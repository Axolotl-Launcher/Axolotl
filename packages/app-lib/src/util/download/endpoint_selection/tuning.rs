//! Serialize throughput experiments so one trial does not change another's baseline.

use parking_lot::Mutex;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrialKind {
    Endpoint,
    Connections,
    Files,
    Ranges,
}

#[derive(Debug, Default)]
struct State {
    next_id: u64,
    revision: u64,
    active: Option<(u64, TrialKind, Instant)>,
}

#[derive(Debug, Default)]
pub(crate) struct TrialRegistry(Mutex<State>);

pub(crate) static TRIALS: LazyLock<Arc<TrialRegistry>> =
    LazyLock::new(Arc::default);

#[derive(Debug)]
pub(crate) struct TrialLease {
    registry: Arc<TrialRegistry>,
    id: u64,
}

impl TrialRegistry {
    fn expire(state: &mut State, now: Instant) {
        if state.active.is_some_and(|(_, _, deadline)| now >= deadline) {
            state.active = None;
            state.revision += 1;
        }
    }

    pub(crate) fn acquire(
        self: &Arc<Self>,
        kind: TrialKind,
        now: Instant,
        lifetime: Duration,
    ) -> Option<TrialLease> {
        let mut state = self.0.lock();
        Self::expire(&mut state, now);
        if state.active.is_some() {
            return None;
        }
        state.next_id += 1;
        let id = state.next_id;
        state.active = Some((id, kind, now + lifetime));
        state.revision += 1;
        Some(TrialLease {
            registry: self.clone(),
            id,
        })
    }

    pub(crate) fn snapshot(&self, now: Instant) -> (u64, Option<TrialKind>) {
        let mut state = self.0.lock();
        Self::expire(&mut state, now);
        (state.revision, state.active.map(|(_, kind, _)| kind))
    }

    /// Production tail recovery takes priority over throughput experiments.
    /// Invalidate their baselines when replacing completed range connections.
    pub(crate) fn invalidate_for_range_recovery(&self) {
        let mut state = self.0.lock();
        state.active = None;
        state.revision += 1;
    }
}

impl TrialLease {
    pub(crate) fn valid(&self, now: Instant) -> bool {
        let mut state = self.registry.0.lock();
        TrialRegistry::expire(&mut state, now);
        state.active.is_some_and(|(id, _, _)| id == self.id)
    }
}

impl Drop for TrialLease {
    fn drop(&mut self) {
        let mut state = self.registry.0.lock();
        if state.active.is_some_and(|(id, _, _)| id == self.id) {
            state.active = None;
            state.revision += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_tuning_axes_are_mutually_exclusive_and_drop_releases() {
        let registry = Arc::new(TrialRegistry::default());
        let now = Instant::now();
        for kind in [
            TrialKind::Endpoint,
            TrialKind::Connections,
            TrialKind::Files,
            TrialKind::Ranges,
        ] {
            let lease = registry
                .acquire(kind, now, Duration::from_secs(20))
                .unwrap();
            assert!(lease.valid(now));
            assert!(
                registry
                    .acquire(TrialKind::Endpoint, now, Duration::from_secs(1))
                    .is_none()
            );
            drop(lease);
            assert!(registry.snapshot(now).1.is_none());
        }
    }

    #[test]
    fn range_recovery_invalidates_experiments_without_releasing_a_successor() {
        let registry = Arc::new(TrialRegistry::default());
        let now = Instant::now();
        let old = registry
            .acquire(TrialKind::Endpoint, now, Duration::from_secs(20))
            .unwrap();
        let revision = registry.snapshot(now).0;
        registry.invalidate_for_range_recovery();
        assert!(!old.valid(now));
        assert!(registry.snapshot(now).0 > revision);
        let next = registry
            .acquire(TrialKind::Ranges, now, Duration::from_secs(2))
            .unwrap();
        drop(old);
        assert!(next.valid(now));
    }

    #[test]
    fn expired_lease_does_not_block_or_release_its_successor() {
        let registry = Arc::new(TrialRegistry::default());
        let now = Instant::now();
        let old = registry
            .acquire(TrialKind::Endpoint, now, Duration::from_secs(1))
            .unwrap();
        let later = now + Duration::from_secs(2);
        let new = registry
            .acquire(TrialKind::Files, later, Duration::from_secs(10))
            .unwrap();
        assert!(!old.valid(later));
        drop(old);
        assert!(new.valid(later));
    }
}
