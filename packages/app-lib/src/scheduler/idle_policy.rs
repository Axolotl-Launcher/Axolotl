//! When an L2 companion counts as idle, and how waiting is classified
//! (spec §8.4, T7.7). A unit paused by design (waiting for the user, a paused
//! download) waits without limit and never counts against a budget; a unit
//! that is running but makes no progress is stalled, which has a limit and
//! does count. L2 is idle only when its gate is paused, no request is in
//! flight and nothing is queued — all three at once.

use std::time::{Duration, Instant};

use super::bypass::HolderState;

/// Grace after the last activity before an L2 companion may count as idle.
pub const IDLE_GRACE: Duration = Duration::from_secs(30);
/// How long a running unit may make no progress before it is stalled.
pub const STALL_LIMIT: Duration = super::fairness::STALL_TIMEOUT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleCondition {
    GatePaused,
    NoInFlightRequest,
    NoPendingQueue,
}

pub const IDLE_CONDITIONS: [IdleCondition; 3] = [
    IdleCondition::GatePaused,
    IdleCondition::NoInFlightRequest,
    IdleCondition::NoPendingQueue,
];

/// What an L2 companion reports about itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompanionState {
    pub gate_paused: bool,
    pub in_flight_requests: usize,
    pub pending: usize,
    pub last_activity: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdlePolicy {
    pub idle_grace: Duration,
    pub stall_limit: Duration,
}

impl Default for IdlePolicy {
    fn default() -> Self {
        Self {
            idle_grace: IDLE_GRACE,
            stall_limit: STALL_LIMIT,
        }
    }
}

impl IdlePolicy {
    pub fn holds(condition: IdleCondition, state: &CompanionState) -> bool {
        match condition {
            IdleCondition::GatePaused => state.gate_paused,
            IdleCondition::NoInFlightRequest => state.in_flight_requests == 0,
            IdleCondition::NoPendingQueue => state.pending == 0,
        }
    }

    /// Idle only when every condition holds and the grace has passed.
    pub fn is_idle(&self, state: &CompanionState, now: Instant) -> bool {
        IDLE_CONDITIONS
            .iter()
            .all(|condition| Self::holds(*condition, state))
            && now.saturating_duration_since(state.last_activity)
                >= self.idle_grace
    }

    /// Classifies a unit that has made no progress for `quiet`.
    /// `paused_by_design` comes from the unit itself (waiting on the user,
    /// a paused download); nothing else makes a wait by design.
    pub fn classify(
        &self,
        paused_by_design: bool,
        quiet: Duration,
    ) -> HolderState {
        if paused_by_design {
            HolderState::PausedByDesign
        } else if quiet >= self.stall_limit {
            HolderState::StalledWhileRunning
        } else {
            HolderState::Progressing
        }
    }

    /// Whether the wait counts against the unit's restart budget.
    pub fn counts_against_budget(state: HolderState) -> bool {
        state == HolderState::StalledWhileRunning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_state(now: Instant) -> CompanionState {
        CompanionState {
            gate_paused: true,
            in_flight_requests: 0,
            pending: 0,
            last_activity: now - IDLE_GRACE,
        }
    }

    #[test]
    fn l2_is_idle_only_when_all_three_conditions_hold() {
        let now = Instant::now();
        let policy = IdlePolicy::default();
        assert!(policy.is_idle(&idle_state(now), now));
        let breakers = [
            CompanionState {
                gate_paused: false,
                ..idle_state(now)
            },
            CompanionState {
                in_flight_requests: 1,
                ..idle_state(now)
            },
            CompanionState {
                pending: 1,
                ..idle_state(now)
            },
        ];
        for (state, condition) in breakers.iter().zip(IDLE_CONDITIONS) {
            assert!(!IdlePolicy::holds(condition, state));
            assert!(
                !policy.is_idle(state, now),
                "{condition:?} alone breaks idle"
            );
        }
    }

    #[test]
    fn idle_waits_for_the_grace() {
        let now = Instant::now();
        let recent = CompanionState {
            last_activity: now,
            ..idle_state(now)
        };
        assert!(!IdlePolicy::default().is_idle(&recent, now));
    }

    #[test]
    fn a_designed_pause_has_no_limit_and_no_budget_cost() {
        let policy = IdlePolicy::default();
        let state = policy.classify(true, Duration::from_secs(24 * 3600));
        assert_eq!(state, HolderState::PausedByDesign);
        assert!(!IdlePolicy::counts_against_budget(state));
    }

    #[test]
    fn a_stall_is_limited_and_counts_against_the_budget() {
        let policy = IdlePolicy::default();
        assert_eq!(
            policy.classify(false, STALL_LIMIT - Duration::from_millis(1)),
            HolderState::Progressing
        );
        let stalled = policy.classify(false, STALL_LIMIT);
        assert_eq!(stalled, HolderState::StalledWhileRunning);
        assert!(IdlePolicy::counts_against_budget(stalled));
    }
}
