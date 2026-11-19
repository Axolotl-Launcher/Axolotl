//! The task contract: what a unit of work promises, independent of who runs it.
//!
//! A task declares its kind, how it is made idempotent, how often it may be
//! retried, how long it may take, and what must happen to it at exit. The
//! scheduler owns retry and backoff ([`execute`]); the task owns its effect
//! ([`Task::run`]) and, for at-least-once redelivery, how to tell whether that
//! effect already landed ([`Task::probe`]).
//!
//! Nothing here writes `install_jobs` or `resource_operation_plans`: `probe`
//! only reads them. That keeps the durable record single-truth and leaves the
//! storage compatibility rules of the launcher database intact.

use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::time::{Duration, Instant};

use content_management::resources::JobId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::install::model::InstallJobKind;
use crate::scheduler::l0::quiesce::QuiesceToken;

/// A unit of work's identity. Reuses the resource layer's `JobId` vocabulary
/// rather than introducing a second one.
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TaskId(JobId);

impl TaskId {
    pub fn new(job: JobId) -> Self {
        Self(job)
    }

    pub fn as_job(&self) -> &JobId {
        &self.0
    }
}

/// What a task does, coarse enough to group work without naming a queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskKind {
    InstallJob(InstallJobKind),
    ContentRefresh,
    PlanExecution,
    Backup,
    Export,
    ServerSupervision,
    Maintenance,
}

/// How a task is made idempotent: `domain:scoped-id`.
///
/// The key is validated once, at construction. It never replaces the durable
/// record's own uniqueness constraint; it names the constraint to the
/// scheduler.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IdempotencyKey(String);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InvalidKey {
    #[error("an idempotency key may not be empty")]
    Empty,
    #[error("an idempotency key must be 'domain:scoped-id'")]
    Shape,
    #[error("an idempotency key may not contain control characters")]
    Control,
}

impl IdempotencyKey {
    pub fn parse(raw: impl Into<String>) -> Result<Self, InvalidKey> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(InvalidKey::Empty);
        }
        if raw.chars().any(char::is_control) {
            return Err(InvalidKey::Control);
        }
        let (domain, scoped) = raw.split_once(':').ok_or(InvalidKey::Shape)?;
        if domain.is_empty() || scoped.is_empty() || scoped.contains(':') {
            return Err(InvalidKey::Shape);
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which attempt this is; `n` is 1-based, matching the install job model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attempt {
    pub n: u32,
    pub max: u32,
}

impl Attempt {
    pub const fn first(max: u32) -> Self {
        Self { n: 1, max }
    }

    pub const fn is_last(self) -> bool {
        self.n >= self.max
    }

    pub fn next(self) -> Option<Self> {
        (!self.is_last()).then_some(Self {
            n: self.n + 1,
            max: self.max,
        })
    }
}

/// A wall-clock instant past which the task should not keep going.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deadline(Instant);

impl Deadline {
    pub fn after(duration: Duration) -> Self {
        Self(Instant::now() + duration)
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.0.checked_duration_since(Instant::now())
    }

    pub fn expired(&self) -> bool {
        self.remaining().is_none()
    }
}

/// How often a task may be retried, and how long to wait between tries.
///
/// `backoff`'s last element repeats, so a task with one entry waits the same
/// time before every retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff: &'static [Duration],
    pub give_up_after: Option<Duration>,
}

impl RetryPolicy {
    /// One attempt, no retry.
    pub const ONCE: Self = Self {
        max_attempts: 1,
        backoff: &[],
        give_up_after: None,
    };

    /// The house ladder: 1s, 4s, 16s, then 16s until the attempt bound.
    pub const HOUSE: Self = Self {
        max_attempts: 9,
        backoff: &[
            Duration::from_secs(1),
            Duration::from_secs(4),
            Duration::from_secs(16),
        ],
        give_up_after: Some(Duration::from_secs(30 * 60)),
    };

    /// Delay before `attempt`; `None` means the policy is exhausted.
    pub fn next_delay(&self, attempt: Attempt) -> Option<Duration> {
        if attempt.n >= self.max_attempts || self.backoff.is_empty() {
            return None;
        }
        let index = (attempt.n as usize).saturating_sub(1);
        Some(self.backoff[index.min(self.backoff.len() - 1)])
    }
}

/// The only states a finished task can be in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalState {
    Succeeded,
    Failed,
    Canceled,
    Interrupted,
    /// Retries ran out. Stored as `Failed` plus the `retry_exhausted` code;
    /// no new status string is introduced.
    Abandoned,
}

/// Why a task did not succeed, classified by what the scheduler may do.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskError {
    #[error("{0}")]
    Retryable(String),
    #[error("{0}")]
    Permanent(String),
    #[error("{0}")]
    NeedsUser(String),
    #[error("{0}")]
    Interrupted(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    Retryable,
    Permanent,
    NeedsUser,
    Interrupted,
}

impl TaskError {
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::Retryable(_) => ErrorClass::Retryable,
            Self::Permanent(_) => ErrorClass::Permanent,
            Self::NeedsUser(_) => ErrorClass::NeedsUser,
            Self::Interrupted(_) => ErrorClass::Interrupted,
        }
    }

    /// The terminal state a failure of this class settles into.
    pub fn terminal_state(&self) -> TerminalState {
        match self.class() {
            ErrorClass::Interrupted => TerminalState::Interrupted,
            ErrorClass::Retryable
            | ErrorClass::Permanent
            | ErrorClass::NeedsUser => TerminalState::Failed,
        }
    }
}

/// What the exit barrier must do about a task that is still running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitPolicy {
    /// Finish before exit; the deadline must fit the drain window.
    MustComplete,
    /// Output may still be publishing; startup reconciliation settles it.
    PublishGuarded,
    /// Dropping it loses nothing that is not recomputable.
    MayBeDropped,
}

/// What a task is given when it runs.
pub struct TaskContext<'a> {
    pub attempt: Attempt,
    pub deadline: Option<Deadline>,
    pub cancel: &'a CancellationToken,
    pub quiesce: Option<&'a QuiesceToken>,
}

impl TaskContext<'_> {
    /// Breaks when the task should stop: cancelled, quiescing, or out of time.
    ///
    /// Quiesce breaks even for `MustComplete` work so a stalled task lands in
    /// the exit loss list instead of blocking the exit window.
    pub fn checkpoint(&self) -> ControlFlow<()> {
        if self.cancel.is_cancelled()
            || self.deadline.is_some_and(|deadline| deadline.expired())
        {
            return ControlFlow::Break(());
        }
        if self.quiesce.is_some_and(|quiesce| quiesce.is_requested()) {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}

/// What a task returns when it stops.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskOutcome {
    pub state: TerminalState,
    pub error: Option<TaskError>,
    pub attempt: Attempt,
}

impl TaskOutcome {
    pub fn succeeded(attempt: Attempt) -> Self {
        Self {
            state: TerminalState::Succeeded,
            error: None,
            attempt,
        }
    }

    pub fn failed(attempt: Attempt, error: TaskError) -> Self {
        Self {
            state: error.terminal_state(),
            error: Some(error),
            attempt,
        }
    }

    /// The delay before the next attempt; `None` settles the task.
    pub fn next_delay(&self, policy: RetryPolicy) -> Option<Duration> {
        if self.state == TerminalState::Succeeded {
            return None;
        }
        if !matches!(
            self.error.as_ref().map(TaskError::class),
            Some(ErrorClass::Retryable) | Some(ErrorClass::Interrupted)
        ) {
            return None;
        }
        policy.next_delay(self.attempt)
    }
}

/// A unit of work the scheduler can run without knowing what it does.
///
/// Object-safe: boxed futures, no generic parameters, so a runner can hold
/// `&dyn Task`.
pub trait Task: Send + Sync {
    fn id(&self) -> TaskId;

    fn kind(&self) -> TaskKind;

    fn idempotency_key(&self) -> Option<&IdempotencyKey>;

    fn retry(&self) -> RetryPolicy {
        RetryPolicy::ONCE
    }

    fn deadline(&self) -> Option<Deadline> {
        None
    }

    fn exit_policy(&self) -> ExitPolicy {
        ExitPolicy::MayBeDropped
    }

    /// Perform the effect and report what happened.
    fn run<'a>(
        &'a self,
        ctx: TaskContext<'a>,
    ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>>;

    /// At-least-once redelivery: read the durable record and say whether the
    /// effect already landed. `None` means it cannot be observed and the task
    /// should run again.
    fn probe<'a>(
        &'a self,
        ctx: TaskContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Option<TerminalState>> + Send + 'a>>;
}

/// The scheduler's only entry point. Owns retry and backoff; never owns the
/// effect, and never writes the durable record.
pub async fn execute(
    task: &dyn Task,
    ctx_base: TaskContext<'_>,
) -> TaskOutcome {
    let policy = task.retry();
    let mut attempt = ctx_base.attempt;

    loop {
        let ctx = TaskContext {
            attempt,
            deadline: ctx_base.deadline,
            cancel: ctx_base.cancel,
            quiesce: ctx_base.quiesce,
        };

        if ctx.checkpoint().is_break() {
            return TaskOutcome {
                state: TerminalState::Interrupted,
                error: Some(TaskError::Interrupted(
                    "the task was interrupted before it ran".into(),
                )),
                attempt,
            };
        }

        if let Some(state) = task.probe(ctx).await {
            return TaskOutcome {
                state,
                error: None,
                attempt,
            };
        }

        let outcome = task
            .run(TaskContext {
                attempt,
                deadline: ctx_base.deadline,
                cancel: ctx_base.cancel,
                quiesce: ctx_base.quiesce,
            })
            .await;

        let Some(delay) = outcome.next_delay(policy) else {
            // "Abandoned" means retries actually ran out. A policy that never
            // retried reports the failure it saw instead.
            let retries_ran_out = policy.max_attempts > 1
                && outcome.state == TerminalState::Failed
                && matches!(
                    outcome.error.as_ref().map(TaskError::class),
                    Some(ErrorClass::Retryable)
                );
            return if retries_ran_out {
                TaskOutcome {
                    state: TerminalState::Abandoned,
                    ..outcome
                }
            } else {
                outcome
            };
        };

        let Some(next) = attempt.next() else {
            return TaskOutcome {
                state: TerminalState::Abandoned,
                ..outcome
            };
        };

        tokio::time::sleep(delay).await;
        attempt = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn task_id() -> TaskId {
        TaskId::new(JobId::new("task-1").expect("valid job id"))
    }

    struct Flaky {
        id: TaskId,
        runs: Arc<AtomicU32>,
        fail_until: u32,
    }

    const ONE_MILLI: [Duration; 1] = [Duration::from_millis(1)];

    impl Task for Flaky {
        fn id(&self) -> TaskId {
            self.id.clone()
        }

        fn kind(&self) -> TaskKind {
            TaskKind::Maintenance
        }

        fn idempotency_key(&self) -> Option<&IdempotencyKey> {
            None
        }

        fn retry(&self) -> RetryPolicy {
            RetryPolicy {
                max_attempts: 3,
                backoff: &ONE_MILLI,
                give_up_after: None,
            }
        }

        fn run<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>> {
            Box::pin(async move {
                self.runs.fetch_add(1, Ordering::AcqRel);
                if ctx.attempt.n < self.fail_until {
                    TaskOutcome::failed(
                        ctx.attempt,
                        TaskError::Retryable("try again".into()),
                    )
                } else {
                    TaskOutcome::succeeded(ctx.attempt)
                }
            })
        }

        fn probe<'a>(
            &'a self,
            _ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = Option<TerminalState>> + Send + 'a>>
        {
            Box::pin(async { None })
        }
    }

    fn run_ctx(cancel: &CancellationToken) -> TaskContext<'_> {
        TaskContext {
            attempt: Attempt::first(3),
            deadline: None,
            cancel,
            quiesce: None,
        }
    }

    #[tokio::test]
    async fn retries_until_success() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs: Arc::clone(&runs),
            fail_until: 3,
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Succeeded);
        assert_eq!(runs.load(Ordering::Acquire), 3);
    }

    #[tokio::test]
    async fn exhausting_retries_settles_as_abandoned() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs: Arc::clone(&runs),
            fail_until: u32::MAX,
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Abandoned);
        assert_eq!(runs.load(Ordering::Acquire), 3);
    }

    #[tokio::test]
    async fn cancellation_short_circuits_before_the_effect() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs: Arc::clone(&runs),
            fail_until: 1,
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Interrupted);
        assert_eq!(runs.load(Ordering::Acquire), 0);
    }

    #[test]
    fn idempotency_keys_are_validated_once() {
        assert!(IdempotencyKey::parse("content-plan:abc@3").is_ok());
        assert_eq!(IdempotencyKey::parse(""), Err(InvalidKey::Empty));
        assert_eq!(IdempotencyKey::parse("no-colon"), Err(InvalidKey::Shape));
        assert_eq!(IdempotencyKey::parse("a:b:c"), Err(InvalidKey::Shape));
        assert_eq!(IdempotencyKey::parse("a:\u{7}b"), Err(InvalidKey::Control));
    }

    #[test]
    fn attempt_is_one_based_and_bounded() {
        let first = Attempt::first(2);
        assert_eq!(first.n, 1);
        assert!(!first.is_last());
        assert_eq!(first.next(), Some(Attempt { n: 2, max: 2 }));
        assert!(first.next().unwrap().is_last());
        assert_eq!(first.next().unwrap().next(), None);
    }

    #[test]
    fn the_house_ladder_repeats_its_last_delay() {
        assert_eq!(
            RetryPolicy::HOUSE.next_delay(Attempt { n: 1, max: 9 }),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            RetryPolicy::HOUSE.next_delay(Attempt { n: 2, max: 9 }),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            RetryPolicy::HOUSE.next_delay(Attempt { n: 5, max: 9 }),
            Some(Duration::from_secs(16))
        );
        assert_eq!(
            RetryPolicy::HOUSE.next_delay(Attempt { n: 9, max: 9 }),
            None
        );
    }

    #[tokio::test]
    async fn a_single_attempt_policy_reports_the_failure_it_saw() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = RetryOnce {
            id: task_id(),
            runs: Arc::clone(&runs),
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Failed);
        assert_eq!(runs.load(Ordering::Acquire), 1);
    }

    struct RetryOnce {
        id: TaskId,
        runs: Arc<AtomicU32>,
    }

    impl Task for RetryOnce {
        fn id(&self) -> TaskId {
            self.id.clone()
        }

        fn kind(&self) -> TaskKind {
            TaskKind::Maintenance
        }

        fn idempotency_key(&self) -> Option<&IdempotencyKey> {
            None
        }

        fn run<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>> {
            Box::pin(async move {
                self.runs.fetch_add(1, Ordering::AcqRel);
                TaskOutcome::failed(
                    ctx.attempt,
                    TaskError::Retryable("one shot".into()),
                )
            })
        }

        fn probe<'a>(
            &'a self,
            _ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = Option<TerminalState>> + Send + 'a>>
        {
            Box::pin(async { None })
        }
    }

    #[test]
    fn permanent_failures_are_not_retried() {
        let outcome = TaskOutcome::failed(
            Attempt { n: 1, max: 9 },
            TaskError::Permanent("no".into()),
        );
        assert_eq!(outcome.state, TerminalState::Failed);
        assert_eq!(outcome.next_delay(RetryPolicy::HOUSE), None);
    }

    #[test]
    fn deadline_expiry_breaks_the_checkpoint() {
        let cancel = CancellationToken::new();
        let deadline = Deadline::after(Duration::ZERO);
        let ctx = TaskContext {
            attempt: Attempt::first(1),
            deadline: Some(deadline),
            cancel: &cancel,
            quiesce: None,
        };
        assert!(ctx.checkpoint().is_break());
    }
}
