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
//! are candidates, each only under a stated condition; the process manager,
//! the friends socket and Discord RPC are excluded because they hold external
//! handles. The log ring buffer is the only object that meets every criterion
//! unconditionally.

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

/// The one unconditional member of L1.
pub const LOG_RING_BUFFER: Assessment = Assessment {
    name: "LogRingBuffer",
    path: "packages/app-lib/src/state/process.rs",
    criteria: Criteria::ALL,
    condition: None,
};

pub const CANDIDATES: [Assessment; 3] = [
    Assessment {
        name: "config_sync",
        path: "packages/app-lib/src/state/instances/config_sync.rs",
        criteria: Criteria::ALL,
        condition: Some(
            "its loop runs in the slot's JoinSet instead of the state's background \
             tracker, and the dirty-instance set is empty, so no options write is \
             in flight (C1b, C3)",
        ),
    },
    Assessment {
        name: "endpoint selection coordinator",
        path: "packages/app-lib/src/util/download/endpoint_selection/coordinator.rs",
        criteria: Criteria::ALL,
        condition: Some(
            "no download holds a lease from it and its probe tasks run in the \
             slot's JoinSet (C1b, C3)",
        ),
    },
    Assessment {
        name: "google_ip warm-up",
        path: "packages/app-lib/src/api/google_ip.rs",
        criteria: Criteria::ALL,
        condition: Some(
            "no IP refresh task is running; the pinned translation client's pooled \
             connections close when the pool is dropped (C1a, C3)",
        ),
    },
];

pub const EXCLUDED: [Assessment; 3] = [
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
        name: "friends_socket",
        path: "packages/app-lib/src/state/friends.rs",
        criteria: Criteria {
            c1a_no_external_handle: false,
            c3_no_open_contract: false,
            ..Criteria::ALL
        },
        condition: Some("owns a live websocket to the friends service"),
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
    fn l1_has_three_conditional_candidates() {
        assert_eq!(CANDIDATES.len(), 3);
        for candidate in CANDIDATES {
            assert!(candidate.criteria.swappable(), "{}", candidate.name);
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
        assert_eq!(names, ["ProcessManager", "friends_socket", "discord_rpc"]);
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
    fn no_candidate_uses_spawn_blocking() {
        let sources = [
            include_str!("../../state/instances/config_sync.rs"),
            include_str!(
                "../../util/download/endpoint_selection/coordinator.rs"
            ),
            include_str!("../../api/google_ip.rs"),
        ];
        for (candidate, source) in CANDIDATES.iter().zip(sources) {
            assert!(
                !source.contains("spawn_blocking"),
                "{} uses spawn_blocking",
                candidate.name
            );
        }
    }
}
