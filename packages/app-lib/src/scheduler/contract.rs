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

    /// The earlier of two deadlines, which is the one that binds.
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
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

/// Why a task was stopped before it could finish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    /// The caller's cancellation token fired.
    Canceled,
    /// L0 asked the task to reach its next safety point and return.
    Quiesced,
    /// The task's deadline expired.
    Deadline,
}

impl StopReason {
    /// The terminal state a stop settles into.
    ///
    /// A cancellation is `Canceled`, not `Interrupted`: the install-job model
    /// persists the two separately, so collapsing them would report the wrong
    /// state for a job the user stopped.
    pub fn terminal_state(self) -> TerminalState {
        match self {
            Self::Canceled => TerminalState::Canceled,
            Self::Quiesced | Self::Deadline => TerminalState::Interrupted,
        }
    }

    fn error(self) -> TaskError {
        match self {
            Self::Canceled => {
                TaskError::Interrupted("the task was canceled".into())
            }
            Self::Quiesced => {
                TaskError::Interrupted("the task was quiesced".into())
            }
            Self::Deadline => {
                TaskError::Interrupted("the task's deadline expired".into())
            }
        }
    }
}

/// What a task is given when it runs.
pub struct TaskContext<'a> {
    pub attempt: Attempt,
    pub deadline: Option<Deadline>,
    pub cancel: &'a CancellationToken,
    pub quiesce: Option<&'a QuiesceToken>,
}

impl TaskContext<'_> {
    /// The first stop signal that applies, in priority order: an explicit
    /// cancellation outranks a quiesce, which outranks an expired deadline.
    pub fn stop_reason(&self) -> Option<StopReason> {
        if self.cancel.is_cancelled() {
            return Some(StopReason::Canceled);
        }
        if self.quiesce.is_some_and(|quiesce| quiesce.is_requested()) {
            return Some(StopReason::Quiesced);
        }
        if self.deadline.is_some_and(|deadline| deadline.expired()) {
            return Some(StopReason::Deadline);
        }
        None
    }

    /// Breaks when the task should stop: cancelled, quiescing, or out of time.
    ///
    /// Quiesce breaks even for `MustComplete` work so a stalled task lands in
    /// the exit loss list instead of blocking the exit window.
    pub fn checkpoint(&self) -> ControlFlow<()> {
        match self.stop_reason() {
            Some(_) => ControlFlow::Break(()),
            None => ControlFlow::Continue(()),
        }
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
///
/// Retry authority belongs to the task's declared policy. The caller's
/// [`Attempt::max`] is only an outer bound on the first attempt's numbering;
/// it does not shorten the policy. A policy that never retries reports the
/// failure it saw rather than an exhausted-retry state.
///
/// Exit paths are distinguished: a cancellation settles as `Canceled`, while
/// a quiesce or an expired deadline settles as `Interrupted`. Retryable work
/// that used up its policy settles as `Abandoned` with a `retry_exhausted`
/// error, whichever of `Failed`/`Interrupted` it stopped at.
pub async fn execute(
    task: &dyn Task,
    ctx_base: TaskContext<'_>,
) -> TaskOutcome {
    let policy = task.retry();
    // The task's own deadline applies when the caller supplied none, and the
    // earlier of the two applies when both exist.
    let deadline = match (ctx_base.deadline, task.deadline()) {
        (Some(caller), Some(declared)) => Some(caller.min(declared)),
        (Some(caller), None) => Some(caller),
        (None, declared) => declared,
    };
    // The other half of #5: the attempt bound is the policy's, not the
    // caller's. A caller that passes `Attempt::first(1)` with a nine-attempt
    // policy must still get nine attempts.
    let mut attempt = Attempt {
        n: ctx_base.attempt.n.max(1),
        max: policy.max_attempts.max(ctx_base.attempt.n.max(1)),
    };
    let started = Instant::now();

    loop {
        let stopped = || {
            let ctx = TaskContext {
                attempt,
                deadline,
                cancel: ctx_base.cancel,
                quiesce: ctx_base.quiesce,
            };
            ctx.stop_reason()
        };

        if let Some(stop) = stopped() {
            return TaskOutcome {
                state: stop.terminal_state(),
                error: Some(stop.error()),
                attempt,
            };
        }

        let probe_ctx = TaskContext {
            attempt,
            deadline,
            cancel: ctx_base.cancel,
            quiesce: ctx_base.quiesce,
        };
        if let Some(state) = task.probe(probe_ctx).await {
            return TaskOutcome {
                state,
                error: None,
                attempt,
            };
        }

        // A stop that arrived while the probe ran must not let the effect
        // start; re-check before committing to it.
        if let Some(stop) = stopped() {
            return TaskOutcome {
                state: stop.terminal_state(),
                error: Some(stop.error()),
                attempt,
            };
        }

        let outcome = task
            .run(TaskContext {
                attempt,
                deadline,
                cancel: ctx_base.cancel,
                quiesce: ctx_base.quiesce,
            })
            .await;

        if outcome.state == TerminalState::Succeeded {
            return outcome;
        }

        // `give_up_after` bounds the whole retry sequence, not one attempt: a
        // policy whose attempts have already outlived it stops here.
        let out_of_time = policy
            .give_up_after
            .is_some_and(|limit| started.elapsed() >= limit);
        if out_of_time && retryable(&outcome) {
            return exhausted(outcome);
        }

        let Some(delay) = outcome.next_delay(policy) else {
            return settle(outcome, policy);
        };

        // A delay that cannot fit in the remaining time is not a retry.
        let delay_fits = deadline
            .and_then(|deadline| deadline.remaining())
            .is_none_or(|left| left > delay);
        if !delay_fits {
            return settle(
                TaskOutcome {
                    state: TerminalState::Interrupted,
                    error: Some(TaskError::Interrupted(
                        "the deadline leaves no room for another attempt"
                            .into(),
                    )),
                    attempt,
                },
                policy,
            );
        }

        let Some(next) = attempt.next() else {
            return settle(outcome, policy);
        };

        // Backoff must be interruptible: a stop during the wait settles now
        // instead of after the sleep ends.
        let wait_ctx = TaskContext {
            attempt,
            deadline,
            cancel: ctx_base.cancel,
            quiesce: ctx_base.quiesce,
        };
        match interruptible_sleep(delay, &wait_ctx).await {
            Ok(()) => attempt = next,
            Err(stop) => {
                return TaskOutcome {
                    state: stop.terminal_state(),
                    error: Some(stop.error()),
                    attempt,
                };
            }
        }
    }
}

fn retryable(outcome: &TaskOutcome) -> bool {
    matches!(
        outcome.error.as_ref().map(TaskError::class),
        Some(ErrorClass::Retryable) | Some(ErrorClass::Interrupted)
    )
}

/// Settle a failed attempt against its policy.
///
/// `Abandoned` is reserved for retryable work that actually used up its
/// retries, and it carries the `retry_exhausted` code. Anything else reports
/// the state it reached, so a `Permanent` failure stays `Failed` and a policy
/// that never retried is not relabelled.
fn settle(outcome: TaskOutcome, policy: RetryPolicy) -> TaskOutcome {
    let budget_exhausted =
        policy.max_attempts > 1 && outcome.attempt.n >= policy.max_attempts;
    if retryable(&outcome) && budget_exhausted {
        return exhausted(outcome);
    }
    outcome
}

fn exhausted(outcome: TaskOutcome) -> TaskOutcome {
    TaskOutcome {
        state: TerminalState::Abandoned,
        error: Some(TaskError::Retryable(RETRY_EXHAUSTED.into())),
        attempt: outcome.attempt,
    }
}

/// The code an exhausted retry sequence reports. The install model stores it
/// in `InstallErrorView::code`, so a settled `Abandoned` outcome carries the
/// distinguishing code rather than the last attempt's message.
pub const RETRY_EXHAUSTED: &str = "retry_exhausted";

/// Sleep for `delay`, returning early when a stop signal arrives.
async fn interruptible_sleep(
    delay: Duration,
    ctx: &TaskContext<'_>,
) -> Result<(), StopReason> {
    if let Some(stop) = ctx.stop_reason() {
        return Err(stop);
    }
    let quiesced = async {
        match ctx.quiesce {
            Some(quiesce) => quiesce.requested().await,
            None => std::future::pending().await,
        }
    };
    let expired = async {
        match ctx.deadline.and_then(|deadline| deadline.remaining()) {
            Some(left) => tokio::time::sleep(left).await,
            None => std::future::pending().await,
        }
    };
    let elapsed = tokio::time::sleep(delay);
    tokio::pin!(quiesced, expired, elapsed);
    tokio::select! {
        biased;
        () = ctx.cancel.cancelled() => Err(StopReason::Canceled),
        () = quiesced => Err(StopReason::Quiesced),
        () = expired => Err(StopReason::Deadline),
        () = elapsed => Ok(()),
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
        // A cancellation is `Canceled`; the install model persists it
        // separately from `Interrupted`, so reporting either for the other
        // would misdescribe a job the user stopped.
        assert_eq!(outcome.state, TerminalState::Canceled);
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

    // ---- regressions for the 8 review findings ----

    /// A task whose retry sequence has outlived `give_up_after` must stop
    /// instead of retrying to the attempt bound.
    struct SlowRetry {
        id: TaskId,
        runs: Arc<AtomicU32>,
    }

    impl Task for SlowRetry {
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
                max_attempts: 9,
                backoff: &ONE_MILLI,
                give_up_after: Some(Duration::ZERO),
            }
        }
        fn run<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>> {
            Box::pin(async move {
                self.runs.fetch_add(1, Ordering::AcqRel);
                TaskOutcome::failed(
                    ctx.attempt,
                    TaskError::Retryable("still failing".into()),
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

    /// Finding 1: `give_up_after` bounds the whole retry sequence.
    #[tokio::test]
    async fn give_up_after_stops_the_retry_sequence() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = SlowRetry {
            id: task_id(),
            runs: Arc::clone(&runs),
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Abandoned);
        assert_eq!(
            outcome.error,
            Some(TaskError::Retryable(RETRY_EXHAUSTED.into()))
        );
        assert_eq!(
            runs.load(Ordering::Acquire),
            1,
            "a zero give_up_after must not allow a second attempt"
        );
    }

    /// Finding 2: a task's declared deadline applies even when the caller
    /// passed none, and an effect never starts after it expires.
    struct DeclaresDeadline {
        id: TaskId,
        runs: Arc<AtomicU32>,
    }

    impl Task for DeclaresDeadline {
        fn id(&self) -> TaskId {
            self.id.clone()
        }
        fn kind(&self) -> TaskKind {
            TaskKind::Maintenance
        }
        fn idempotency_key(&self) -> Option<&IdempotencyKey> {
            None
        }
        fn deadline(&self) -> Option<Deadline> {
            Some(Deadline::after(Duration::ZERO))
        }
        fn run<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>> {
            Box::pin(async move {
                self.runs.fetch_add(1, Ordering::AcqRel);
                TaskOutcome::succeeded(ctx.attempt)
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

    #[tokio::test]
    async fn a_task_declared_deadline_applies_without_a_caller_one() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = DeclaresDeadline {
            id: task_id(),
            runs: Arc::clone(&runs),
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Interrupted);
        assert_eq!(
            runs.load(Ordering::Acquire),
            0,
            "an expired declared deadline must stop the effect"
        );
    }

    const LONG: [Duration; 1] = [Duration::from_secs(30)];

    struct SlowBackoff {
        inner: Flaky,
    }

    impl Task for SlowBackoff {
        fn id(&self) -> TaskId {
            self.inner.id()
        }
        fn kind(&self) -> TaskKind {
            self.inner.kind()
        }
        fn idempotency_key(&self) -> Option<&IdempotencyKey> {
            None
        }
        fn retry(&self) -> RetryPolicy {
            RetryPolicy {
                max_attempts: 9,
                backoff: &LONG,
                give_up_after: None,
            }
        }
        fn run<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'a>> {
            self.inner.run(ctx)
        }
        fn probe<'a>(
            &'a self,
            ctx: TaskContext<'a>,
        ) -> Pin<Box<dyn Future<Output = Option<TerminalState>> + Send + 'a>>
        {
            self.inner.probe(ctx)
        }
    }

    /// Finding 4: a stop during backoff settles now, not after the delay.
    #[tokio::test]
    async fn cancellation_during_backoff_settles_without_waiting() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = SlowBackoff {
            inner: Flaky {
                id: task_id(),
                runs: Arc::clone(&runs),
                fail_until: u32::MAX,
            },
        };
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            trigger.cancel();
        });
        let started = Instant::now();
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Canceled);
        assert_eq!(runs.load(Ordering::Acquire), 1);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the 30-second backoff must not have been awaited"
        );
    }

    /// Finding 5: a caller's small `Attempt` must not shorten the policy.
    #[tokio::test]
    async fn the_policy_not_the_callers_attempt_bound_governs_retries() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs: Arc::clone(&runs),
            fail_until: 3,
        };
        let ctx = TaskContext {
            attempt: Attempt::first(1),
            deadline: None,
            cancel: &cancel,
            quiesce: None,
        };
        let outcome = execute(&task, ctx).await;
        assert_eq!(outcome.state, TerminalState::Succeeded);
        assert_eq!(
            runs.load(Ordering::Acquire),
            3,
            "the task's own three-attempt policy must run, not the caller's one"
        );
    }

    /// Finding 6: exhaustion carries the `retry_exhausted` code rather than
    /// the last attempt's message.
    #[tokio::test]
    async fn exhaustion_reports_the_retry_exhausted_code() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs: Arc::clone(&runs),
            fail_until: u32::MAX,
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Abandoned);
        assert_eq!(
            outcome.error,
            Some(TaskError::Retryable(RETRY_EXHAUSTED.into()))
        );
    }

    /// Finding 7: a canceled task settles as `Canceled`, not `Interrupted`.
    #[tokio::test]
    async fn cancellation_settles_as_canceled_not_interrupted() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let runs = Arc::new(AtomicU32::new(0));
        let task = Flaky {
            id: task_id(),
            runs,
            fail_until: 1,
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Canceled);
    }

    struct AlwaysInterrupted {
        id: TaskId,
        runs: Arc<AtomicU32>,
    }

    impl Task for AlwaysInterrupted {
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
                TaskOutcome::failed(
                    ctx.attempt,
                    TaskError::Interrupted("the work was interrupted".into()),
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

    /// Finding 8: interrupted work that used up its policy settles as
    /// `Abandoned`, not `Interrupted`.
    #[tokio::test]
    async fn exhausted_interrupted_work_settles_as_abandoned() {
        let cancel = CancellationToken::new();
        let runs = Arc::new(AtomicU32::new(0));
        let task = AlwaysInterrupted {
            id: task_id(),
            runs: Arc::clone(&runs),
        };
        let outcome = execute(&task, run_ctx(&cancel)).await;
        assert_eq!(outcome.state, TerminalState::Abandoned);
        assert_eq!(runs.load(Ordering::Acquire), 3);
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
