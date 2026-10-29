//! One swappable service and its rebuild protocol (steps in the module above).

use std::future::Future;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::task::JoinSet;

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
    inner: Mutex<Inner<S>>,
}

impl<S> EvictionSlot<S> {
    // Named rather than `Self`: the resource generator reads `-> Self {` as
    // a struct literal outside a function (a known false positive that keeps
    // fields registered; fixing it moves reviewed ranks, spec §11.8 R2).
    pub fn live(built: Built<S>) -> EvictionSlot<S> {
        EvictionSlot {
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

    /// Steps 1–6. Returns whether a live service was evicted.
    pub async fn evict(&self) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.state != SlotState::Live {
            return false;
        }
        inner.state = SlotState::Evicting;
        inner.tasks.abort_all();
        // A panicked or aborted task has terminated as much as a finished one.
        while inner.tasks.join_next().await.is_some() {}
        debug_assert_eq!(inner.tasks.len(), 0);
        inner.service = None;
        inner.state = SlotState::Evicted;
        true
    }

    /// Steps 7–10, with `now` as the clock.
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
        let mut inner = self.inner.lock().await;
        match inner.state {
            SlotState::Live => return RebuildOutcome::AlreadyLive,
            SlotState::Revoked => return RebuildOutcome::Revoked,
            _ => {}
        }
        if let Some(retry_at) = inner.retry_at
            && now < retry_at
        {
            return RebuildOutcome::NotYet(retry_at - now);
        }
        inner.state = SlotState::Rebuilding;
        match entry().await {
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
                inner.failures += 1;
                let first = *inner.first_failure.get_or_insert(now);
                if inner.failures >= REBUILD_ATTEMPTS_TOTAL
                    || now.saturating_duration_since(first)
                        >= REBUILD_GIVE_UP_AFTER
                {
                    tracing::warn!(
                        failures = inner.failures,
                        %error,
                        "An evicted service could not be rebuilt; it stays degraded"
                    );
                    inner.state = SlotState::Revoked;
                    inner.level = Level::L2;
                    return RebuildOutcome::Revoked;
                }
                let wait = REBUILD_BACKOFF[(inner.failures as usize - 1)
                    .min(REBUILD_BACKOFF.len() - 1)];
                inner.retry_at = Some(now + wait);
                inner.state = SlotState::Evicted;
                RebuildOutcome::RetryAfter(wait)
            }
        }
    }
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

    async fn failing() -> Result<Built<u32>, String> {
        Err("rebuild failed".into())
    }

    #[tokio::test]
    async fn evicting_drops_the_service_and_its_tasks() {
        let mut tasks = JoinSet::new();
        tasks.spawn(std::future::pending::<()>());
        tasks.spawn(async { panic!("a panicked descendant still terminates") });
        let slot = EvictionSlot::live(Built { service: 7, tasks });
        assert!(slot.evict().await);
        assert_eq!(slot.state().await, SlotState::Evicted);
        assert_eq!(slot.with_service(|value| *value).await, None);
        assert!(!slot.evict().await, "only a live slot is evicted");
    }

    #[tokio::test]
    async fn a_failed_rebuild_backs_off_one_four_then_sixteen_seconds() {
        let slot = EvictionSlot::live(built(1));
        slot.evict().await;
        let mut now = Instant::now();
        for wait in [1, 4, 16, 16] {
            let wait = Duration::from_secs(wait);
            assert_eq!(
                slot.rebuild(now, failing).await,
                RebuildOutcome::RetryAfter(wait)
            );
            assert_eq!(
                slot.rebuild(now, failing).await,
                RebuildOutcome::NotYet(wait),
                "no rebuild before the backoff ends"
            );
            now += wait;
        }
    }

    #[tokio::test]
    async fn eviction_gives_up_with_cumulative_failures() {
        let slot = EvictionSlot::live(built(1));
        slot.evict().await;
        let mut now = Instant::now();
        let mut outcome = RebuildOutcome::Live;
        for _ in 0..REBUILD_ATTEMPTS_TOTAL {
            outcome = slot.rebuild(now, failing).await;
            now += REBUILD_BACKOFF[2];
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
        let slot = EvictionSlot::live(built(1));
        slot.evict().await;
        let first = Instant::now();
        slot.rebuild(first, failing).await;
        assert_eq!(
            slot.rebuild(first + REBUILD_GIVE_UP_AFTER, failing).await,
            RebuildOutcome::Revoked
        );
    }

    #[tokio::test]
    async fn only_a_successful_rebuild_resets_the_counters() {
        let slot = EvictionSlot::live(built(1));
        slot.evict().await;
        let mut now = Instant::now();
        for _ in 0..5 {
            slot.rebuild(now, failing).await;
            now += REBUILD_BACKOFF[2];
        }
        assert_eq!(
            slot.rebuild(now, || async { Ok::<_, String>(built(2)) })
                .await,
            RebuildOutcome::Live
        );
        assert_eq!(slot.with_service(|value| *value).await, Some(2));
        slot.evict().await;
        assert_eq!(
            slot.rebuild(now, failing).await,
            RebuildOutcome::RetryAfter(REBUILD_BACKOFF[0]),
            "the count starts over after success"
        );
    }
}
