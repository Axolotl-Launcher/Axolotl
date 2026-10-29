//! The read-only scheduler view (M6): one snapshot of what the inversion
//! guards see, so they can be verified. Building a view only reads; it holds
//! nothing past its own return and changes nothing.
//!
//! Observed items ([`METRICS`]): live tags per band, the order scope and its
//! listed exceptions, bypass grants, denials and the latest audit records, the
//! L0 control table (heartbeat, safety point, quiesce and abort requests), the
//! L0 queue's dropped and overflowed counts, and the aging and queue wait
//! bounds.

use std::collections::BTreeMap;

use serde::Serialize;

use super::aging::{AGING_INTERVAL, max_wait_bound};
use super::bypass::{AuditRecord, ReclaimBypass};
use super::fairness::queue_max_wait_upper_bound;
use super::l0::control::ControlTable;
use super::tag::{Level, TagRegistry};

/// The names of every observed item, as serialized.
pub const METRICS: [&str; 15] = [
    "live_tags",
    "order_scope",
    "order_exceptions",
    "bypass_granted",
    "bypass_denied",
    "bypass_recent",
    "l0_heartbeat_at",
    "l0_safety_point_at",
    "l0_quiesce_requested",
    "l0_abort_requested",
    "l0_queue_dropped",
    "l0_queue_overflowed",
    "aging_interval_ms",
    "aging_max_wait_ms",
    "queue_max_wait_ms",
];

/// Audit records a view carries.
pub const RECENT_BYPASS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SchedulerView {
    pub live_tags: BTreeMap<Level, usize>,
    pub order_scope: &'static str,
    /// `held -> acquired` pairs allowed to descend (scheduler policy).
    pub order_exceptions: usize,
    pub bypass_granted: usize,
    pub bypass_denied: usize,
    pub bypass_recent: Vec<AuditRecord>,
    pub l0_heartbeat_at: u64,
    pub l0_safety_point_at: u64,
    pub l0_quiesce_requested: bool,
    pub l0_abort_requested: bool,
    pub l0_queue_dropped: u64,
    pub l0_queue_overflowed: u64,
    pub aging_interval_ms: u64,
    pub aging_max_wait_ms: u64,
    pub queue_max_wait_ms: u64,
}

/// What a view reads from. The L0 parts are absent when no L0 executor runs.
pub struct Sources<'a> {
    pub tags: &'a TagRegistry,
    pub bypass: &'a ReclaimBypass,
    pub order_exceptions: usize,
    pub control: Option<&'a ControlTable>,
    /// `(dropped, overflowed)` of the L0 event queue.
    pub l0_queue: Option<(u64, u64)>,
}

pub fn snapshot(sources: &Sources<'_>) -> SchedulerView {
    let recent: Vec<_> = sources.bypass.audit().cloned().collect();
    let control = sources.control;
    let (dropped, overflowed) = sources.l0_queue.unwrap_or_default();
    SchedulerView {
        live_tags: sources.tags.live_by_level(),
        order_scope: "family",
        order_exceptions: sources.order_exceptions,
        bypass_granted: sources.bypass.granted(),
        bypass_denied: sources.bypass.denied(),
        bypass_recent: recent[recent.len().saturating_sub(RECENT_BYPASS)..]
            .to_vec(),
        l0_heartbeat_at: control.map_or(0, ControlTable::heartbeat_at),
        l0_safety_point_at: control.map_or(0, ControlTable::safety_point_at),
        l0_quiesce_requested: control
            .is_some_and(ControlTable::quiesce_requested),
        l0_abort_requested: control.is_some_and(ControlTable::abort_requested),
        l0_queue_dropped: dropped,
        l0_queue_overflowed: overflowed,
        aging_interval_ms: AGING_INTERVAL.as_millis() as u64,
        aging_max_wait_ms: max_wait_bound().as_millis() as u64,
        queue_max_wait_ms: queue_max_wait_upper_bound().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::bypass::{BypassRequest, HolderState, Initiator};
    use std::time::{Duration, Instant};

    #[test]
    fn the_view_reports_each_guard_and_changes_nothing() {
        let tags = TagRegistry::new(1);
        let waiter = tags.mint(Level::L4, "view");
        let _l1 = tags.mint(Level::L1, "view");
        let _other_l1 = tags.mint(Level::L1, "view");
        let mut bypass = ReclaimBypass::new();
        let now = Instant::now();
        for waited in [Duration::from_millis(10), Duration::from_secs(1)] {
            let _ = bypass.request(
                BypassRequest {
                    waiter: &waiter,
                    resource: "theseus::state::State.install_db_semaphore",
                    waited,
                    holder: HolderState::StalledWhileRunning,
                    initiator: Initiator::Scheduler,
                },
                now,
            );
        }
        let control = ControlTable::new();
        control.heartbeat();
        control.request_quiesce();
        let sources = Sources {
            tags: &tags,
            bypass: &bypass,
            order_exceptions: 3,
            control: Some(&control),
            l0_queue: Some((4, 1)),
        };

        let view = snapshot(&sources);
        assert_eq!(view, snapshot(&sources), "taking a view changes nothing");
        assert_eq!(
            view.live_tags,
            BTreeMap::from([(Level::L1, 2), (Level::L4, 1)])
        );
        assert_eq!((view.bypass_granted, view.bypass_denied), (1, 1));
        assert_eq!(view.bypass_recent.len(), 2);
        assert!(view.l0_heartbeat_at > 0);
        assert!(view.l0_quiesce_requested && !view.l0_abort_requested);
        assert_eq!((view.l0_queue_dropped, view.l0_queue_overflowed), (4, 1));
        assert_eq!(view.aging_interval_ms, 250);
        assert_eq!(view.queue_max_wait_ms, 8_000);
        assert!(tags.is_live(&waiter));

        let serialized = serde_json::to_value(&view).unwrap();
        let keys: Vec<_> =
            serialized.as_object().unwrap().keys().cloned().collect();
        let mut expected: Vec<_> =
            METRICS.iter().map(|name| name.to_string()).collect();
        expected.sort();
        let mut keys = keys;
        keys.sort();
        assert_eq!(keys, expected, "METRICS lists every observed item");
    }

    #[test]
    fn without_l0_the_view_reports_zeroes_instead_of_failing() {
        let tags = TagRegistry::new(1);
        let bypass = ReclaimBypass::new();
        let view = snapshot(&Sources {
            tags: &tags,
            bypass: &bypass,
            order_exceptions: 0,
            control: None,
            l0_queue: None,
        });
        assert!(view.live_tags.is_empty());
        assert_eq!(view.l0_heartbeat_at, 0);
        assert!(!view.l0_quiesce_requested);
    }
}
