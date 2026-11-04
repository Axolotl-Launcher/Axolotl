//! One swappable service and its rebuild protocol (steps in the module above).

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::task::JoinSet;

use super::criteria::Swappable;
use crate::scheduler::tag::Level;

/// Waits before the next rebuild after the first, second and later failures.
pub const REBUILD_BACKOFF: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(4),
    Duration::from_secs(16),
];
/// Failed rebuilds in all before the slot is revoked.
pub const REBUILD_ATTEMPTS_TOTAL: u32 = 9;
/// Time since the first failure after which the slot is revoked.
pub const REBUILD_GIVE_UP_AFTER: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Live,
    Evicting,
    Evicted,
    Rebuilding,
    /// Rebuilding gave up; the work runs with L2 semantics and the slot
    /// reports a degraded state.
    Revoked,
}

/// What a rebuild entry hands back: the service and the tasks it started.
pub struct Built<S> {
    pub service: S,
    pub tasks: JoinSet<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildOutcome {
    Live,
    AlreadyLive,
    /// Still backing off; try again after this long.
    NotYet(Duration),
    /// Another caller is rebuilding right now; this one changed nothing.
    Busy,
    /// This rebuild failed; the next one may start after this long.
    RetryAfter(Duration),
    Revoked,
}

struct Inner<S> {
    state: SlotState,
    level: Level,
    service: Option<S>,
    tasks: JoinSet<()>,
    failures: u32,
    first_failure: Option<Instant>,
    retry_at: Option<Instant>,
}

/// A swappable service. The mutex around it is the per-tag rebuild lock:
/// eviction and rebuild both run under it, so a rebuild never starts while
/// the old instance's tasks are still draining.
pub struct EvictionSlot<S> {
    /// The scheduling tag this slot serves; the map owner keys slots by it.
    key: u64,
    inner: Mutex<Inner<S>>,
    /// Set when the winning attempt actually entered its entry, cleared when
    /// that entry returns. A `Rebuilding` slot with this clear never reached
    /// the entry, so the attempt was dropped before it started.
    entry_entered: AtomicBool,
    /// Set when an entry was dropped before it returned.
    entry_abandoned: AtomicBool,
}

/// How long the drain waits for aborted descendants before giving up on the
/// swap-out. A descendant stuck in `spawn_blocking` cannot be aborted, and
/// waiting forever would hold the rebuild lock and never return the memory.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Longest a rebuild entry may run before it counts as a failed attempt. A
/// dependency that never answers must not leave the slot `Rebuilding` forever.
pub const REBUILD_ENTRY_TIMEOUT: Duration = Duration::from_secs(60);

/// Marks a running rebuild entry. While it lives the entry is running; if it
/// is dropped before `settle` — a panic, a cancelled future, a cancellation
/// while waiting for the slot lock — the entry was abandoned, and the flag it
/// leaves says so, so the next caller settles the attempt instead of leaving
/// `Rebuilding` set forever.
struct Unfinished<'a> {
    running: &'a AtomicBool,
    abandoned: &'a AtomicBool,
    disarmed: bool,
}

impl<'a> Unfinished<'a> {
    fn new(running: &'a AtomicBool, abandoned: &'a AtomicBool) -> Self {
        running.store(true, Ordering::Release);
        abandoned.store(false, Ordering::Release);
        Self {
            running,
            abandoned,
            disarmed: false,
        }
    }

    /// The result is committed under the slot lock: this attempt is settled,
    /// so a later cancellation cannot make it look abandoned.
    fn settle(&mut self) {
        self.running.store(false, Ordering::Release);
        self.disarmed = true;
    }
}

impl Drop for Unfinished<'_> {
    fn drop(&mut self) {
        if !self.disarmed {
            // Dropped while the entry was still running (panic or cancel).
            self.running.store(false, Ordering::Release);
            self.abandoned.store(true, Ordering::Release);
        }
    }
}

impl<S> EvictionSlot<S> {
    /// A live slot for the tag `key`, created through the registry so one tag
    /// never gets two slots. The eligibility token comes from the service's
    /// [`crate::scheduler::eviction::criteria::Assessment`]: an assessment
    /// that fails any criterion yields `None` and no token, so an ineligible
    /// service cannot be put into a slot at all.
    // Named rather than `Self`: the resource generator reads `-> Self {` as
    // a struct literal outside a function (a known false positive that keeps
    // fields registered; fixing it moves reviewed ranks, spec §11.8 R2).
    fn new(key: u64, _eligible: Swappable, built: Built<S>) -> EvictionSlot<S> {
        EvictionSlot {
            key,
            entry_entered: AtomicBool::new(false),
            entry_abandoned: AtomicBool::new(false),
            inner: Mutex::new(Inner {
                state: SlotState::Live,
                level: Level::L1,
                service: Some(built.service),
                tasks: built.tasks,
                failures: 0,
                first_failure: None,
                retry_at: None,
            }),
        }
    }

    /// The scheduling tag this slot was created for.
    pub fn key(&self) -> u64 {
        self.key
    }

    pub async fn state(&self) -> SlotState {
        self.inner.lock().await.state
    }

    /// L1 until the slot is revoked, then L2.
    pub async fn level(&self) -> Level {
        self.inner.lock().await.level
    }

    /// The user-visible degraded state.
    pub async fn degraded(&self) -> bool {
        self.inner.lock().await.state == SlotState::Revoked
    }

    pub async fn with_service<R>(&self, f: impl FnOnce(&S) -> R) -> Option<R> {
        self.inner.lock().await.service.as_ref().map(f)
    }

    /// Steps 1–6. Returns `true` when the slot is `Evicted` afterwards, which
    /// includes finishing an eviction that was cut short (its future dropped
    /// while draining). A slot that is already evicted, rebuilding or revoked
    /// returns `false` and is left alone.
    pub async fn evict(&self) -> bool {
        let mut inner = self.inner.lock().await;
        if !matches!(inner.state, SlotState::Live | SlotState::Evicting) {
            return false;
        }
        drain(&mut inner).await;
        inner.state == SlotState::Evicted
    }

    /// Steps 7–10, with `now` as the clock.
    ///
    /// The slot lock is not held while `entry` runs. A rebuild entry is
    /// ordinary service code: it may read the slot it is rebuilding (to log
    /// the attempt, to check a flag), and holding the lock across the await
    /// made that a self-deadlock. What serialises rebuilds is the
    /// `Rebuilding` state: exactly one caller wins the transition, and a
    /// second one sees the slot busy and returns without calling `entry`.
    pub async fn rebuild<F, Fut, E>(
        &self,
        now: Instant,
        entry: F,
    ) -> RebuildOutcome
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Built<S>, E>>,
        E: std::fmt::Display,
    {
        {
            let mut inner = self.inner.lock().await;
            match inner.state {
                SlotState::Live => return RebuildOutcome::AlreadyLive,
                SlotState::Revoked => return RebuildOutcome::Revoked,
                // An eviction cut short: the old tasks drain to zero before
                // a new instance may start.
                SlotState::Evicting => drain(&mut inner).await,
                SlotState::Evicted | SlotState::Rebuilding => {}
            }
            if inner.state == SlotState::Rebuilding {
                // Someone won the transition. `entry_entered` is set by the
                // guard right before the entry is awaited and cleared when it
                // returns: set means an entry is running right now (`Busy`),
                // clear means the winner was dropped before reaching its
                // entry, which is the abandoned case and is settled here as
                // a failure.
                if self.entry_entered.load(Ordering::Acquire) {
                    return RebuildOutcome::Busy;
                }
                if !self.entry_abandoned.swap(false, Ordering::AcqRel) {
                    // The winner holds `Rebuilding` and is on its way into
                    // the entry: this caller is simply early. Report busy so
                    // it retries; the winner is alive.
                    return RebuildOutcome::Busy;
                }
                return match count_failure(
                    &mut inner,
                    now,
                    "the rebuild was abandoned",
                ) {
                    Some(wait) => RebuildOutcome::RetryAfter(wait),
                    None => RebuildOutcome::Revoked,
                };
            }
            // The give-up deadline holds whether or not this attempt would
            // work.
            if let Some(first) = inner.first_failure
                && now.saturating_duration_since(first) >= REBUILD_GIVE_UP_AFTER
            {
                revoke(&mut inner, "the rebuild deadline passed");
                return RebuildOutcome::Revoked;
            }
            if let Some(retry_at) = inner.retry_at
                && now < retry_at
            {
                return RebuildOutcome::NotYet(retry_at - now);
            }
            inner.state = SlotState::Rebuilding;
        }
        // A panicking, cancelled or timed-out entry must not strand the slot
        // in `Rebuilding`. The guard is armed until the result is committed
        // under the lock: until then the attempt is still "not settled", so a
        // cancellation while waiting for the mutex is settled by the next
        // caller exactly like a panic.
        let mut unfinished =
            Unfinished::new(&self.entry_entered, &self.entry_abandoned);
        let entry_started = Instant::now();
        let result = match tokio::time::timeout(REBUILD_ENTRY_TIMEOUT, entry())
            .await
        {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!(
                    timeout = ?REBUILD_ENTRY_TIMEOUT,
                    "A rebuild entry did not return in time; counting it as a failed attempt"
                );
                let mut inner = self.inner.lock().await;
                unfinished.settle();
                let failed_at =
                    now + entry_started.elapsed().min(REBUILD_ENTRY_TIMEOUT);
                return match count_failure(
                    &mut inner,
                    failed_at,
                    "the rebuild entry timed out",
                ) {
                    Some(wait) => RebuildOutcome::RetryAfter(wait),
                    None => RebuildOutcome::Revoked,
                };
            }
        };
        let mut inner = self.inner.lock().await;
        unfinished.settle();
        // `now` is the caller's clock and stays authoritative: a test injects
        // it, and a production caller passes `Instant::now()` at the point it
        // calls `rebuild`. The elapsed time of the entry is added when it is
        // measurable, so a slow entry's failure is recorded when it failed
        // rather than when it started.
        let failed_at = now + entry_started.elapsed();
        match result {
            Ok(built) => {
                inner.service = Some(built.service);
                inner.tasks = built.tasks;
                inner.failures = 0;
                inner.first_failure = None;
                inner.retry_at = None;
                inner.state = SlotState::Live;
                RebuildOutcome::Live
            }
            Err(error) => {
                tracing::warn!(
                    failures = inner.failures + 1,
                    error = %error,
                    "A rebuild attempt failed"
                );
                match count_failure(&mut inner, failed_at, &error.to_string()) {
                    Some(wait) => RebuildOutcome::RetryAfter(wait),
                    None => RebuildOutcome::Revoked,
                }
            }
        }
    }
}

/// The one slot per tag. Creating a second slot for the same key returns the
/// existing one, so two callers can never evict or rebuild the same tag
/// independently.
pub struct SlotRegistry<S> {
    slots: std::sync::Mutex<
        std::collections::HashMap<u64, std::sync::Arc<EvictionSlot<S>>>,
    >,
}

impl<S> Default for SlotRegistry<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SlotRegistry<S> {
    pub fn new() -> Self {
        Self {
            slots: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// The slot for `key`, creating it on first use. The first caller's
    /// service is used; later callers get the same slot and their `built` is
    /// dropped.
    pub fn slot(
        &self,
        key: u64,
        eligible: Swappable,
        built: Built<S>,
    ) -> std::sync::Arc<EvictionSlot<S>> {
        let mut slots =
            self.slots.lock().unwrap_or_else(|error| error.into_inner());
        slots
            .entry(key)
            .or_insert_with(|| {
                std::sync::Arc::new(EvictionSlot::new(key, eligible, built))
            })
            .clone()
    }

    /// The slot for `key` if one exists.
    pub fn get(&self, key: u64) -> Option<std::sync::Arc<EvictionSlot<S>>> {
        self.slots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&key)
            .cloned()
    }

    pub fn len(&self) -> usize {
        self.slots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Steps 3–6: aborts the slot's tasks, drains them to zero and drops the
/// service. A panicked or aborted task has terminated as much as a finished
/// one. Draining is safe to resume: cut short, it leaves the slot
/// `Evicting` with the remaining tasks still in its `JoinSet`.
///
/// A descendant in `spawn_blocking` cannot be aborted. The drain is bounded,
/// and a descendant that outlives the deadline revokes the slot: the module
/// cannot observe when detached work really ends, so it must not start a
/// replacement next to it. Revoked is the safe state — the work keeps its L2
/// semantics and no second instance is built.
async fn drain<S>(inner: &mut Inner<S>) {
    inner.state = SlotState::Evicting;
    inner.tasks.abort_all();
    let deadline = Instant::now() + DRAIN_TIMEOUT;
    let mut stranded = false;
    while Instant::now() < deadline {
        match tokio::time::timeout_at(deadline.into(), inner.tasks.join_next())
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => {
                stranded = true;
                tracing::warn!(
                    remaining = inner.tasks.len(),
                    "Some evicted tasks outlived the drain timeout; the slot will not rebuild a second instance"
                );
                break;
            }
        }
    }
    inner.tasks.detach_all();
    inner.service = None;
    if stranded {
        revoke(inner, "evicted work outlived the drain window");
        return;
    }
    inner.state = SlotState::Evicted;
}

/// Counts a failed rebuild attempt. Returns the backoff that now applies, or
/// `None` when the slot was revoked (attempts or deadline spent).
fn count_failure<S>(
    inner: &mut Inner<S>,
    now: Instant,
    reason: &str,
) -> Option<Duration> {
    inner.failures += 1;
    let first = *inner.first_failure.get_or_insert(now);
    if inner.failures >= REBUILD_ATTEMPTS_TOTAL
        || now.saturating_duration_since(first) >= REBUILD_GIVE_UP_AFTER
    {
        revoke(inner, reason);
        return None;
    }
    let wait = REBUILD_BACKOFF
        [(inner.failures as usize - 1).min(REBUILD_BACKOFF.len() - 1)];
    inner.retry_at = Some(now + wait);
    inner.state = SlotState::Evicted;
    Some(wait)
}

fn revoke<S>(inner: &mut Inner<S>, reason: &str) {
    tracing::warn!(
        failures = inner.failures,
        reason,
        "An evicted service could not be rebuilt; it stays degraded"
    );
    inner.state = SlotState::Revoked;
    inner.level = Level::L2;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(value: u32) -> Built<u32> {
        Built {
            service: value,
            tasks: JoinSet::new(),
        }
    }

    /// The eligibility token every test slot needs; a real service gets one
    /// from `Assessment::token` after passing every criterion.
    fn eligible() -> Swappable {
        super::super::criteria::LOG_RING_BUFFER
            .token()
            .expect("the log ring buffer is the unconditional L1 member")
    }

    fn registry() -> SlotRegistry<u32> {
        SlotRegistry::new()
    }

    async fn failing() -> Result<Built<u32>, String> {
        Err("rebuild failed".into())
    }

    #[tokio::test]
    async fn a_rebuild_entry_that_touches_the_slot_does_not_deadlock() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let probe = std::sync::Arc::clone(&slot);
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            slot.rebuild(Instant::now(), || async move {
                // A real rebuild entry reads the slot it is rebuilding.
                let _ = probe.state().await;
                Ok::<_, String>(built(2))
            }),
        )
        .await
        .expect("the rebuild entry must not wait on the slot's own lock");
        assert_eq!(outcome, RebuildOutcome::Live);
    }

    #[tokio::test]
    async fn the_slot_names_the_tag_it_serves() {
        let registry = registry();
        let slot = registry.slot(42, eligible(), built(1));
        assert_eq!(slot.key(), 42);
    }

    #[tokio::test]
    async fn an_abandoned_rebuild_is_settled_and_retried() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        // Drop the entry future before it completes: this is the panic or
        // cancel path.
        let mut pending = Box::pin(slot.rebuild(Instant::now(), || async {
            std::future::pending::<Result<Built<u32>, String>>().await
        }));
        let _ = futures::poll!(&mut pending);
        drop(pending);
        assert_eq!(slot.state().await, SlotState::Rebuilding);
        let wait = match slot.rebuild(Instant::now(), failing).await {
            RebuildOutcome::RetryAfter(wait) => wait,
            other => panic!(
                "the abandoned attempt must count as a failure: {other:?}"
            ),
        };
        assert_eq!(wait, REBUILD_BACKOFF[0]);
        assert_eq!(slot.state().await, SlotState::Evicted);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_drain_never_waits_past_its_deadline() {
        // `JoinSet::abort_all` ends an async task at its next await point, so
        // the reachable slow case is a task that keeps working after the
        // abort (no await point inside its loop). The drain must still return
        // within its window and must not leave a rebuildable slot behind
        // while that work runs.
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let (started_sender, started_receiver) =
            tokio::sync::oneshot::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let mut tasks = JoinSet::new();
        tasks.spawn({
            let stop = Arc::clone(&stop);
            let finished = Arc::clone(&finished);
            async move {
                let _ = started_sender.send(());
                // No await point: an abort cannot interrupt this loop, which
                // is exactly the descendant the drain deadline exists for.
                while !stop.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
                finished.store(true, Ordering::Release);
            }
        });
        // A sleep only gives the runtime the chance to poll; it does not prove
        // that it did, and an unpolled task is cancelled before it starts.
        started_receiver
            .await
            .expect("the descendant must start before eviction");

        let registry = registry();
        let slot = registry.slot(1, eligible(), Built { service: 7, tasks });
        let started = Instant::now();
        let evicted = tokio::time::timeout(
            DRAIN_TIMEOUT + Duration::from_secs(10),
            slot.evict(),
        )
        .await
        .expect("the drain must return within its window");
        // The outer timeout is the boundedness check; a wall-clock upper bound
        // here fails on a runner that deschedules the test around the
        // deadline. The lower bound is what proves the drain waited at all.
        assert!(
            started.elapsed() >= DRAIN_TIMEOUT,
            "the drain must wait out its deadline, returned after {:?}",
            started.elapsed()
        );
        // The descendant outlived the window, so the slot must never start a
        // second instance beside it.
        assert!(!evicted, "a stranded descendant must revoke the slot");
        assert_eq!(slot.state().await, SlotState::Revoked);
        assert_eq!(
            slot.rebuild(Instant::now(), || async {
                Ok::<_, String>(built(2))
            })
            .await,
            RebuildOutcome::Revoked
        );
        assert_eq!(slot.with_service(|value| *value).await, None);

        // Release the detached descendant so it cannot outlive the test.
        stop.store(true, Ordering::Release);
        tokio::time::timeout(Duration::from_secs(10), async {
            while !finished.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the descendant must stop once released");
    }

    #[tokio::test]
    async fn a_second_rebuild_while_one_runs_reports_busy() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let (built_entered, entered) = tokio::sync::oneshot::channel::<()>();
        let (release, released) = tokio::sync::oneshot::channel::<()>();
        // The first attempt must stay alive while the second one runs, so it
        // is a separate task; `join!` on one task would poll both in step.
        let slot = std::sync::Arc::new(slot);
        let slow = {
            let slot = std::sync::Arc::clone(&slot);
            tokio::spawn(async move {
                slot.rebuild(Instant::now(), || async move {
                    let _ = built_entered.send(());
                    let _ = released.await;
                    Ok::<_, String>(built(2))
                })
                .await
            })
        };
        let _ = entered.await;
        let second = slot
            .rebuild(Instant::now(), || async { Ok::<_, String>(built(3)) })
            .await;
        assert_eq!(second, RebuildOutcome::Busy, "one attempt at a time");
        release.send(()).ok();
        assert_eq!(slow.await.unwrap(), RebuildOutcome::Live);
        assert_eq!(slot.with_service(|value| *value).await, Some(2));
    }

    #[tokio::test]
    async fn evicting_drops_the_service_and_its_tasks() {
        let mut tasks = JoinSet::new();
        tasks.spawn(std::future::pending::<()>());
        tasks.spawn(async { panic!("a panicked descendant still terminates") });
        let registry = registry();
        let slot = registry.slot(1, eligible(), Built { service: 7, tasks });
        assert!(slot.evict().await);
        assert_eq!(slot.state().await, SlotState::Evicted);
        assert_eq!(slot.with_service(|value| *value).await, None);
        assert!(!slot.evict().await, "only a live slot is evicted");
    }

    #[tokio::test]
    async fn a_failed_rebuild_backs_off_one_four_then_sixteen_seconds() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let mut now = Instant::now();
        for wait in [1, 4, 16, 16] {
            let wait = Duration::from_secs(wait);
            // The failure is recorded at the entry's completion, which is a
            // few microseconds after `now`: the returned backoff includes
            // them, and the next attempt must advance by at least that much.
            let outcome = slot.rebuild(now, failing).await;
            let RebuildOutcome::RetryAfter(retry_in) = outcome else {
                panic!("expected a retry, got {outcome:?}");
            };
            assert!(
                retry_in >= wait
                    && retry_in < wait + Duration::from_millis(200),
                "backoff should be about {wait:?}, got {retry_in:?}"
            );
            assert!(
                matches!(
                    slot.rebuild(now, failing).await,
                    RebuildOutcome::NotYet(_)
                ),
                "no rebuild before the backoff ends"
            );
            // Step just past the recorded failure time (a few microseconds
            // after `now`) plus the backoff.
            now += retry_in + Duration::from_millis(1);
        }
    }

    #[tokio::test]
    async fn eviction_gives_up_with_cumulative_failures() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let mut now = Instant::now();
        let mut outcome = RebuildOutcome::Live;
        for _ in 0..REBUILD_ATTEMPTS_TOTAL {
            outcome = slot.rebuild(now, failing).await;
            // Advance past the returned backoff; the recorded failure time is
            // a few microseconds after `now`.
            if let RebuildOutcome::RetryAfter(wait) = outcome {
                now += wait + Duration::from_millis(1);
            } else {
                now += REBUILD_BACKOFF[2];
            }
        }
        assert_eq!(outcome, RebuildOutcome::Revoked);
        assert_eq!(slot.state().await, SlotState::Revoked);
        assert_eq!(slot.level().await, Level::L2, "revoked work runs as L2");
        assert!(slot.degraded().await);
        assert_eq!(
            slot.rebuild(now, || async { Ok::<_, String>(built(2)) })
                .await,
            RebuildOutcome::Revoked,
            "a revoked slot stays revoked"
        );
    }

    #[tokio::test]
    async fn thirty_minutes_of_failures_revoke_before_nine_attempts() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let first = Instant::now();
        slot.rebuild(first, failing).await;
        assert_eq!(
            slot.rebuild(
                first + REBUILD_GIVE_UP_AFTER + Duration::from_secs(1),
                failing
            )
            .await,
            RebuildOutcome::Revoked
        );
    }

    #[tokio::test]
    async fn only_a_successful_rebuild_resets_the_counters() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let mut now = Instant::now();
        for _ in 0..5 {
            slot.rebuild(now, failing).await;
            now += REBUILD_BACKOFF[2] + Duration::from_millis(1);
        }
        assert_eq!(
            slot.rebuild(now, || async { Ok::<_, String>(built(2)) })
                .await,
            RebuildOutcome::Live
        );
        assert_eq!(slot.with_service(|value| *value).await, Some(2));
        slot.evict().await;
        assert!(
            matches!(
                slot.rebuild(now, failing).await,
                RebuildOutcome::RetryAfter(wait) if wait >= REBUILD_BACKOFF[0]
            ),
            "the count starts over after success"
        );
    }

    #[tokio::test]
    async fn a_rebuild_after_the_deadline_is_revoked_even_if_it_would_work() {
        let registry = registry();
        let slot = registry.slot(1, eligible(), built(1));
        slot.evict().await;
        let first = Instant::now();
        slot.rebuild(first, failing).await;
        let called = std::sync::atomic::AtomicBool::new(false);
        let outcome = slot
            .rebuild(
                first + REBUILD_GIVE_UP_AFTER + Duration::from_secs(1),
                || async {
                    called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok::<_, String>(built(2))
                },
            )
            .await;
        assert_eq!(outcome, RebuildOutcome::Revoked);
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(slot.level().await, Level::L2);
    }

    /// An eviction whose future is dropped while it drains leaves the old
    /// tasks running; a rebuild must finish draining them before it builds.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_eviction_cut_short_is_finished_before_a_rebuild() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let (release, wait) = std::sync::mpsc::channel::<()>();
        let (started_tx, started) = std::sync::mpsc::channel::<()>();
        let old_done = Arc::new(AtomicBool::new(false));
        let mut tasks = JoinSet::new();
        {
            let old_done = Arc::clone(&old_done);
            // A blocking task cannot be aborted, so draining waits for it.
            tasks.spawn_blocking(move || {
                let _ = started_tx.send(());
                let _ = wait.recv();
                old_done.store(true, Ordering::SeqCst);
            });
        }
        // Abort reaches a blocking task only before it starts; wait until
        // it runs, so the drain really has to wait for it.
        started.recv().unwrap();
        let registry = registry();
        let slot = registry.slot(1, eligible(), Built { service: 1, tasks });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), slot.evict())
                .await
                .is_err(),
            "the eviction is cut short while it drains"
        );
        assert_eq!(slot.state().await, SlotState::Evicting);

        let rebuild = {
            let slot = Arc::clone(&slot);
            let old_done = Arc::clone(&old_done);
            tokio::spawn(async move {
                slot.rebuild(Instant::now(), || async move {
                    assert!(
                        old_done.load(Ordering::SeqCst),
                        "the old instance's task ended first"
                    );
                    Ok::<_, String>(built(2))
                })
                .await
            })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!rebuild.is_finished(), "the rebuild waits for the drain");
        release.send(()).unwrap();
        assert_eq!(rebuild.await.unwrap(), RebuildOutcome::Live);
        assert_eq!(slot.with_service(|value| *value).await, Some(2));
    }
}
