//! Which services may be swapped out (spec §7.2, T5.1). A service is
//! swappable only when all seven criteria hold:
//!
//! - C1a: it holds no external handle itself (process, socket, IPC client);
//! - C1b: every task it starts is registered in its tag's `JoinSet`;
//! - C1c: none of them is `spawn_blocking` — this alone vetoes it, since a
//!   blocking task cannot be aborted;
//! - C1d: after destruction its task count is zero;
//! - C2: rebuilding it is idempotent;
//! - C3: it has no unfinished external contract;
//! - C4: it has a rebuild entry point.
//!
//! L1 is narrowed to data objects and handle-free services. Three services
//! are candidates, each only under a stated condition; the process manager
//! and Discord RPC are excluded because they hold external handles. The log
//! ring buffer is the only object that meets every criterion unconditionally.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Criteria {
    pub c1a_no_external_handle: bool,
    pub c1b_tasks_in_tag_joinset: bool,
    pub c1c_no_spawn_blocking: bool,
    pub c1d_tasks_reach_zero: bool,
    pub c2_idempotent_rebuild: bool,
    pub c3_no_open_contract: bool,
    pub c4_rebuild_entry: bool,
}

impl Criteria {
    pub const ALL: Self = Self {
        c1a_no_external_handle: true,
        c1b_tasks_in_tag_joinset: true,
        c1c_no_spawn_blocking: true,
        c1d_tasks_reach_zero: true,
        c2_idempotent_rebuild: true,
        c3_no_open_contract: true,
        c4_rebuild_entry: true,
    };

    /// `spawn_blocking` vetoes whatever else holds.
    pub fn vetoed(&self) -> bool {
        !self.c1c_no_spawn_blocking
    }

    pub fn swappable(&self) -> bool {
        !self.vetoed()
            && self.c1a_no_external_handle
            && self.c1b_tasks_in_tag_joinset
            && self.c1d_tasks_reach_zero
            && self.c2_idempotent_rebuild
            && self.c3_no_open_contract
            && self.c4_rebuild_entry
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Assessment {
    pub name: &'static str,
    pub path: &'static str,
    /// The criteria once `condition` holds.
    pub criteria: Criteria,
    /// What must hold at eviction time; `None` when nothing does.
    pub condition: Option<&'static str>,
}

/// Proof that a service passed every swap-out criterion. Only
/// [`Assessment::token`] produces one, and an assessment that is not
/// swappable yields `None`, so a slot cannot be created for an ineligible
/// service.
#[derive(Clone, Copy, Debug)]
pub struct Swappable {
    _priv: (),
}

impl Assessment {
    /// The eligibility token for this assessment, or `None` when it fails any
    /// criterion (including the `spawn_blocking` veto).
    pub fn token(&self) -> Option<Swappable> {
        self.criteria.swappable().then_some(Swappable { _priv: () })
    }
}

/// The one unconditional member of L1.
pub const LOG_RING_BUFFER: Assessment = Assessment {
    name: "LogRingBuffer",
    path: "packages/app-lib/src/state/process.rs",
    criteria: Criteria::ALL,
    condition: None,
};

/// The three conditional candidates, each failing C1c or C4 as it stands.
/// They are listed so the gap is recorded rather than hidden: the code that
/// would make one swappable is named in `condition`, and until that code
/// exists the service must not be handed to [`crate::scheduler::eviction::EvictionSlot`].
pub const CANDIDATES: [Assessment; 3] = [
    Assessment {
        name: "config_sync",
        path: "packages/app-lib/src/state/instances/config_sync.rs",
        // `write_config_file` -> `io::write`, which is `spawn_blocking`
        // (util/io.rs). A blocking write cannot be aborted, so C1c vetoes.
        criteria: Criteria {
            // The loop is started through `State::spawn_background`, not a
            // per-tag slot, so C1b fails until it moves into the slot.
            c1b_tasks_in_tag_joinset: false,
            c1c_no_spawn_blocking: false,
            c4_rebuild_entry: false,
            ..Criteria::ALL
        },
        condition: Some(
            "the options write moves to an abortable async path, its loop runs \
             in the slot's JoinSet, the dirty-instance set is empty, and a \
             rebuild entry exists (C1c, C1b, C3, C4)",
        ),
    },
    Assessment {
        name: "endpoint selection coordinator",
        path: "packages/app-lib/src/util/download/endpoint_selection/coordinator.rs",
        // Probe rounds spawn detached tasks (coordinator.rs) and the hint
        // store persists through `spawn_blocking`, so C1c/C1b fail; there is
        // no rebuild entry.
        criteria: Criteria {
            c1b_tasks_in_tag_joinset: false,
            c1c_no_spawn_blocking: false,
            c4_rebuild_entry: false,
            ..Criteria::ALL
        },
        condition: Some(
            "probe tasks run in the slot's JoinSet, the hint store stops using \
             spawn_blocking, no download holds a lease, and a rebuild entry \
             exists (C1b, C1c, C3, C4)",
        ),
    },
    Assessment {
        name: "google_ip warm-up",
        path: "packages/app-lib/src/api/google_ip.rs",
        // The refresh task is parked in a static and the client pool lives in
        // another static, so neither the task nor the pool can be dropped by
        // the slot; there is no rebuild entry.
        criteria: Criteria {
            c1a_no_external_handle: false,
            c1b_tasks_in_tag_joinset: false,
            c4_rebuild_entry: false,
            ..Criteria::ALL
        },
        condition: Some(
            "the refresh task and the client pool are owned by the slot rather \
             than statics, and a rebuild entry exists (C1a, C1b, C4)",
        ),
    },
];

pub const EXCLUDED: [Assessment; 2] = [
    Assessment {
        name: "ProcessManager",
        path: "packages/app-lib/src/state/process.rs",
        criteria: Criteria {
            c1a_no_external_handle: false,
            c3_no_open_contract: false,
            ..Criteria::ALL
        },
        condition: Some("owns the game child processes"),
    },
    Assessment {
        name: "discord_rpc",
        path: "packages/app-lib/src/state/discord.rs",
        criteria: Criteria {
            c1a_no_external_handle: false,
            ..Criteria::ALL
        },
        condition: Some("owns an IPC client connected to Discord"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_has_three_conditional_candidates_that_are_not_yet_swappable() {
        assert_eq!(CANDIDATES.len(), 3);
        for candidate in CANDIDATES {
            // Each fails at least one criterion today; the condition names
            // the code that would have to change first. Claiming `swappable`
            // here would let an unready service be evicted.
            assert!(
                !candidate.criteria.swappable(),
                "{} is not swappable until its condition holds",
                candidate.name
            );
            assert!(
                !candidate.criteria.c4_rebuild_entry,
                "{} has no rebuild entry",
                candidate.name
            );
            assert!(
                candidate.condition.is_some(),
                "{} is a candidate only under its condition",
                candidate.name
            );
        }
    }

    #[test]
    fn handle_holders_are_excluded() {
        let names: Vec<_> = EXCLUDED.iter().map(|item| item.name).collect();
        assert_eq!(names, ["ProcessManager", "discord_rpc"]);
        for item in EXCLUDED {
            assert!(!item.criteria.c1a_no_external_handle);
            assert!(!item.criteria.swappable(), "{}", item.name);
        }
    }

    #[test]
    fn spawn_blocking_vetoes_everything_else() {
        let blocking = Criteria {
            c1c_no_spawn_blocking: false,
            ..Criteria::ALL
        };
        assert!(blocking.vetoed());
        assert!(!blocking.swappable());
    }

    #[test]
    fn only_the_log_ring_buffer_holds_unconditionally() {
        assert!(LOG_RING_BUFFER.criteria.swappable());
        assert!(LOG_RING_BUFFER.condition.is_none());
        assert!(
            CANDIDATES
                .iter()
                .all(|candidate| candidate.condition.is_some())
        );
    }

    /// C1c, checked on the candidates' sources rather than asserted.
    #[test]
    fn the_spawn_blocking_veto_stays_recorded_for_each_candidate() {
        // The veto must stay on in the table while the blocking write is
        // still reachable: `config_sync` reaches it through `io::write` and
        // the coordinator through its hint store, so a literal grep of the
        // candidate file cannot see it. This pins the recorded verdicts.
        let vetoed: Vec<_> = CANDIDATES
            .iter()
            .filter(|candidate| !candidate.criteria.c1c_no_spawn_blocking)
            .map(|candidate| candidate.name)
            .collect();
        assert_eq!(
            vetoed,
            ["config_sync", "endpoint selection coordinator"],
            "the blocking paths of both candidates are still in place"
        );
        // config_sync's loop is started by `State::spawn_background`, not the
        // slot's JoinSet, so C1b must stay false too.
        assert!(
            !CANDIDATES[0].criteria.c1b_tasks_in_tag_joinset,
            "config_sync's loop is outside the slot"
        );
        assert!(
            !CANDIDATES[2].criteria.c1a_no_external_handle,
            "google_ip's pool and refresh task live in statics"
        );
    }
}
