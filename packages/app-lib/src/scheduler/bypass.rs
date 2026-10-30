//! The explicit reclaim bypass (M4). A unit waiting on a resource whose
//! holder has stopped making progress may be let past that holder for a short
//! lease, into any band including L0. The grant is decided from observable
//! state only, never changes the waiting tag's level, and is never granted at
//! the waiter's own request. The four conditions (tasks.md T4.4):
//!
//! - B-1: the unit has waited longer than [`BYPASS_MIN_WAIT`];
//! - B-2: its tag was granted at most once in the last [`BYPASS_GRANT_INTERVAL`];
//! - B-3: the holder is [`HolderState::StalledWhileRunning`], not paused by design;
//! - B-4: every decision leaves an [`AuditRecord`] (tag id, condition,
//!   timestamp), and a request initiated by the waiter itself is refused.
//!   Who initiates is never a value a caller fills in: the scheduler grants
//!   through [`ReclaimBypass::grant`], visible only inside the scheduler, and a
//!   task asks through [`ReclaimBypass::request`] with its own tag. A tag
//!   cannot be cloned, so asking with the waiter's tag is asking for itself.
//!
//! The audit trail is kept in [`ReclaimBypass::audit`] and logged under the
//! `theseus::scheduler::bypass` target, which lands in the launcher log.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::l0::control::now_millis;
use super::tag::{Level, Tag};

/// B-1.
pub const BYPASS_MIN_WAIT: Duration = Duration::from_millis(200);
/// B-2: one grant per tag per interval.
pub const BYPASS_GRANT_INTERVAL: Duration = Duration::from_secs(60);
pub const BYPASS_LEASE_TTL: Duration = Duration::from_secs(5);
/// Audit records kept in memory; older ones remain in the log.
pub const AUDIT_CAPACITY: usize = 256;

/// What the holder of the contended resource is doing, as observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum HolderState {
    Progressing,
    /// Waiting on purpose (a user prompt, a paused download).
    PausedByDesign,
    /// Running but making no progress.
    StalledWhileRunning,
}

pub struct BypassRequest<'a> {
    pub waiter: &'a Tag,
    pub resource: &'static str,
    pub waited: Duration,
    pub holder: HolderState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Denial {
    /// B-1.
    WaitTooShort,
    /// B-2.
    GrantedRecently,
    /// B-3.
    HolderNotStalled(HolderState),
    /// B-4.
    SelfInitiated,
}

impl Denial {
    fn condition(self) -> &'static str {
        match self {
            Self::WaitTooShort => "B-1 wait too short",
            Self::GrantedRecently => "B-2 granted within the interval",
            Self::HolderNotStalled(_) => "B-3 holder not stalled",
            Self::SelfInitiated => "B-4 self-initiated",
        }
    }
}

/// One bypass decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuditRecord {
    pub tag: u64,
    pub resource: &'static str,
    pub granted: bool,
    pub condition: &'static str,
    /// Milliseconds since the Unix epoch.
    pub at: u64,
}

/// Permission to pass the stalled holder of one resource until it expires.
/// It carries the tag's level unchanged.
#[derive(Debug, PartialEq, Eq)]
pub struct Lease {
    tag: u64,
    level: Level,
    resource: &'static str,
    granted_at: Instant,
}

impl Lease {
    pub fn tag(&self) -> u64 {
        self.tag
    }

    pub fn level(&self) -> Level {
        self.level
    }

    pub fn resource(&self) -> &'static str {
        self.resource
    }

    pub fn valid_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.granted_at) < BYPASS_LEASE_TTL
    }

    /// A lease may pass a holder in any band, L0 included; this is the only
    /// way past an L0 holder (`bands::preempts` never is).
    pub fn reaches(&self, holder: Level, now: Instant) -> bool {
        let _ = holder;
        self.valid_at(now)
    }
}

#[derive(Debug, Default)]
pub struct ReclaimBypass {
    last_grant: HashMap<u64, Instant>,
    audit: VecDeque<AuditRecord>,
    granted: usize,
    denied: usize,
}

impl ReclaimBypass {
    pub fn new() -> Self {
        Self::default()
    }

    /// A grant the scheduler decides from what it observed.
    pub(in crate::scheduler) fn grant(
        &mut self,
        request: BypassRequest<'_>,
        now: Instant,
    ) -> Result<Lease, Denial> {
        self.settle(request, false, now)
    }

    /// A grant a task asks for on the waiter's behalf, proving who it is
    /// with its own tag. The waiter's own tag is refused (B-4).
    pub fn request(
        &mut self,
        asker: &Tag,
        request: BypassRequest<'_>,
        now: Instant,
    ) -> Result<Lease, Denial> {
        let self_initiated = std::ptr::eq(asker, request.waiter);
        self.settle(request, self_initiated, now)
    }

    fn settle(
        &mut self,
        request: BypassRequest<'_>,
        self_initiated: bool,
        now: Instant,
    ) -> Result<Lease, Denial> {
        let tag = request.waiter.id();
        let decision = self.decide(&request, self_initiated, tag, now);
        let condition = match decision {
            Ok(()) => "B-1..B-4 met",
            Err(denial) => denial.condition(),
        };
        self.record(AuditRecord {
            tag,
            resource: request.resource,
            granted: decision.is_ok(),
            condition,
            at: now_millis(),
        });
        decision?;
        self.last_grant.insert(tag, now);
        Ok(Lease {
            tag,
            level: request.waiter.level(),
            resource: request.resource,
            granted_at: now,
        })
    }

    fn decide(
        &self,
        request: &BypassRequest<'_>,
        self_initiated: bool,
        tag: u64,
        now: Instant,
    ) -> Result<(), Denial> {
        if self_initiated {
            return Err(Denial::SelfInitiated);
        }
        if request.waited <= BYPASS_MIN_WAIT {
            return Err(Denial::WaitTooShort);
        }
        if request.holder != HolderState::StalledWhileRunning {
            return Err(Denial::HolderNotStalled(request.holder));
        }
        if self.last_grant.get(&tag).is_some_and(|last| {
            now.saturating_duration_since(*last) < BYPASS_GRANT_INTERVAL
        }) {
            return Err(Denial::GrantedRecently);
        }
        Ok(())
    }

    fn record(&mut self, record: AuditRecord) {
        tracing::info!(
            target: "theseus::scheduler::bypass",
            tag = record.tag,
            resource = record.resource,
            granted = record.granted,
            condition = record.condition,
            "Reclaim bypass decision"
        );
        if record.granted {
            self.granted += 1;
        } else {
            self.denied += 1;
        }
        if self.audit.len() == AUDIT_CAPACITY {
            self.audit.pop_front();
        }
        self.audit.push_back(record);
    }

    pub fn audit(&self) -> impl Iterator<Item = &AuditRecord> {
        self.audit.iter()
    }

    pub fn granted(&self) -> usize {
        self.granted
    }

    pub fn denied(&self) -> usize {
        self.denied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::tag::TagRegistry;

    const RESOURCE: &str = "theseus::state::State.install_db_semaphore";

    fn request(waiter: &Tag) -> BypassRequest<'_> {
        BypassRequest {
            waiter,
            resource: RESOURCE,
            waited: BYPASS_MIN_WAIT + Duration::from_millis(1),
            holder: HolderState::StalledWhileRunning,
        }
    }

    #[test]
    fn a_stalled_holder_is_bypassed_for_one_lease() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        let now = Instant::now();
        let lease = bypass.grant(request(&waiter), now).unwrap();
        assert_eq!(lease.tag(), waiter.id());
        assert!(
            lease.valid_at(now + BYPASS_LEASE_TTL - Duration::from_millis(1))
        );
        assert!(!lease.valid_at(now + BYPASS_LEASE_TTL), "ttl = 5 s");
    }

    #[test]
    fn a_lease_reaches_into_l0_and_keeps_the_level() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        let now = Instant::now();
        let lease = bypass.grant(request(&waiter), now).unwrap();
        assert!(lease.reaches(Level::L0, now));
        assert!(!crate::scheduler::bands::preempts(Level::L4, Level::L0));
        assert_eq!(lease.level(), Level::L4);
        assert_eq!(
            waiter.level(),
            Level::L4,
            "the grant does not raise the tag"
        );
    }

    #[test]
    fn b1_a_short_wait_is_refused() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        let short = BypassRequest {
            waited: BYPASS_MIN_WAIT,
            ..request(&waiter)
        };
        assert_eq!(
            bypass.grant(short, Instant::now()),
            Err(Denial::WaitTooShort)
        );
    }

    #[test]
    fn b2_a_tag_gets_one_grant_per_interval() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let other = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        let now = Instant::now();
        bypass.grant(request(&waiter), now).unwrap();
        assert_eq!(
            bypass.grant(request(&waiter), now + Duration::from_secs(59)),
            Err(Denial::GrantedRecently)
        );
        assert!(
            bypass
                .grant(request(&other), now + Duration::from_secs(59))
                .is_ok(),
            "the limit is per tag"
        );
        assert!(
            bypass
                .grant(request(&waiter), now + BYPASS_GRANT_INTERVAL)
                .is_ok()
        );
    }

    #[test]
    fn b3_only_a_stalled_holder_is_passed() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        for holder in [HolderState::PausedByDesign, HolderState::Progressing] {
            let blocked = BypassRequest {
                holder,
                ..request(&waiter)
            };
            assert_eq!(
                bypass.grant(blocked, Instant::now()),
                Err(Denial::HolderNotStalled(holder))
            );
        }
    }

    #[test]
    fn b4_a_waiter_cannot_grant_itself_and_every_decision_is_audited() {
        let registry = TagRegistry::new(1);
        let waiter = registry.mint(Level::L4, "reclaim");
        let asker = registry.mint(Level::L4, "reclaim");
        let mut bypass = ReclaimBypass::new();
        assert_eq!(
            bypass.request(&waiter, request(&waiter), Instant::now()),
            Err(Denial::SelfInitiated)
        );
        bypass
            .request(&asker, request(&waiter), Instant::now())
            .expect("another task may ask on the waiter's behalf");
        let audit: Vec<_> = bypass.audit().collect();
        assert_eq!(audit.len(), 2);
        assert!(!audit[0].granted);
        assert_eq!(audit[0].condition, "B-4 self-initiated");
        assert!(audit[1].granted);
        assert!(audit.iter().all(|record| record.tag == waiter.id()
            && record.resource == RESOURCE
            && record.at > 0));
        assert_eq!((bypass.granted(), bypass.denied()), (1, 1));
    }
}
