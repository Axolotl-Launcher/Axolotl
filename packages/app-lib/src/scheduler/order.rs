//! The managed-resource order. Ranks come from the generated resource
//! inventory and are never written in code. The recall-gap gate has moved the
//! design to per-family orders (spec §6.3.4, accepted as R5): a task may only
//! acquire a resource ranked above every resource of the same scope family it
//! already holds; resources of different families are not ordered against each
//! other, so only the 20 s admission timeout bounds a deadlock between them.
//! Descending acquisition within a family is a structural violation
//! (`V_struct`).
//!
//! Two checks use these rules. `scripts/axolotl/sched-order-annotation-check.mjs`
//! judges the sched-order annotations statically; [`instrument`] checks
//! the same annotated sites at run time in test runs with
//! `SCHED_ORDER_GUARD=1`.

use std::cell::RefCell;

pub mod instrument;

#[cfg(test)]
mod consistency;

/// A managed resource as it appears in the generated registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rank {
    pub rank: u32,
    /// The scope class (`S1`–`S10`); ranks compare only within one.
    pub family: &'static str,
    pub ident: &'static str,
}

/// An acquisition that does not ascend past what the task already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderViolation {
    pub held: Rank,
    pub acquired: Rank,
}

/// The held resource that acquiring `acquired` would descend past: one of the
/// same family ranked at or above it, unless `allowed` lists that pair as an
/// exception.
pub fn descends_past(
    held: &[Rank],
    acquired: Rank,
    allowed: impl Fn(&Rank, &Rank) -> bool,
) -> Option<OrderViolation> {
    held.iter()
        .filter(|rank| {
            rank.family == acquired.family
                && rank.rank >= acquired.rank
                && !allowed(rank, &acquired)
        })
        .max_by_key(|rank| rank.rank)
        .map(|held| OrderViolation {
            held: *held,
            acquired,
        })
}

/// The ranks one task currently holds.
#[derive(Debug, Default)]
pub struct OrderTracker {
    held: RefCell<Vec<Rank>>,
}

impl OrderTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `resource` as held until the guard drops. In debug builds an
    /// acquisition at or below a held rank of the same family is refused with
    /// the violation; release builds record without checking.
    pub fn enter(
        &self,
        resource: Rank,
    ) -> Result<OrderGuard<'_>, OrderViolation> {
        let mut held = self.held.borrow_mut();
        if cfg!(debug_assertions)
            && let Some(violation) =
                descends_past(&held, resource, |_, _| false)
        {
            return Err(violation);
        }
        held.push(resource);
        Ok(OrderGuard {
            tracker: self,
            resource,
        })
    }

    pub fn held(&self) -> Vec<Rank> {
        self.held.borrow().clone()
    }
}

/// Holds one rank on its tracker; dropping it releases that rank.
#[derive(Debug)]
pub struct OrderGuard<'a> {
    tracker: &'a OrderTracker,
    resource: Rank,
}

impl Drop for OrderGuard<'_> {
    fn drop(&mut self) {
        let mut held = self.tracker.held.borrow_mut();
        if let Some(index) =
            held.iter().rposition(|rank| *rank == self.resource)
        {
            held.remove(index);
        }
    }
}

#[cfg(test)]
mod order_guard_tests {
    use super::*;

    const ADMISSION: Rank = Rank {
        rank: 2,
        family: "S2",
        ident: "theseus::state::State.install_admission_locks",
    };
    const EXECUTION: Rank = Rank {
        rank: 3,
        family: "S2",
        ident: "theseus::state::State.install_execution_locks",
    };
    const DB_PERMIT: Rank = Rank {
        rank: 5,
        family: "S5",
        ident: "theseus::state::State.install_db_semaphore",
    };
    const CONTENT: Rank = Rank {
        rank: 27,
        family: "S3",
        ident: "theseus::state::InstanceLockManager.locks",
    };

    #[test]
    fn ascending_acquisition_is_accepted() {
        let tracker = OrderTracker::new();
        let _admission = tracker.enter(ADMISSION).unwrap();
        let _execution = tracker.enter(EXECUTION).unwrap();
        let _permit = tracker.enter(DB_PERMIT).unwrap();
        let _content = tracker.enter(CONTENT).unwrap();
        assert_eq!(
            tracker.held(),
            vec![ADMISSION, EXECUTION, DB_PERMIT, CONTENT]
        );
    }

    #[test]
    fn families_are_not_ordered_against_each_other() {
        let tracker = OrderTracker::new();
        let _content = tracker.enter(CONTENT).unwrap();
        let _permit = tracker
            .enter(DB_PERMIT)
            .expect("a lower rank of another family is not a violation");
        assert_eq!(tracker.held(), vec![CONTENT, DB_PERMIT]);
    }

    // Release builds record without checking, so the violations below only
    // exist in debug builds.
    #[cfg(debug_assertions)]
    #[test]
    fn descending_acquisition_within_a_family_is_a_violation() {
        let tracker = OrderTracker::new();
        let _execution = tracker.enter(EXECUTION).unwrap();
        assert_eq!(
            tracker.enter(ADMISSION).unwrap_err(),
            OrderViolation {
                held: EXECUTION,
                acquired: ADMISSION,
            }
        );
        assert_eq!(tracker.held(), vec![EXECUTION]);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn reacquiring_a_held_rank_is_a_violation() {
        let tracker = OrderTracker::new();
        let _permit = tracker.enter(DB_PERMIT).unwrap();
        assert!(tracker.enter(DB_PERMIT).is_err());
    }

    #[test]
    fn released_ranks_no_longer_constrain() {
        let tracker = OrderTracker::new();
        drop(tracker.enter(EXECUTION).unwrap());
        let _admission = tracker.enter(ADMISSION).unwrap();
        assert_eq!(tracker.held(), vec![ADMISSION]);
    }

    #[test]
    fn a_listed_exception_lets_one_pair_descend() {
        let allowed = |held: &Rank, acquired: &Rank| {
            held.ident == EXECUTION.ident && acquired.ident == ADMISSION.ident
        };
        assert_eq!(descends_past(&[EXECUTION], ADMISSION, allowed), None);
        assert!(descends_past(&[EXECUTION], ADMISSION, |_, _| false).is_some());
    }
}
