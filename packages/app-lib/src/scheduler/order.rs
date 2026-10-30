//! Debug check of the single global resource order: a task may only acquire a
//! managed resource whose rank is higher than every rank it already holds.
//! Ranks come from the generated resource inventory; descending acquisition is
//! a structural violation (`V_struct`).
//!
//! Not wired in yet: no production acquisition goes through an
//! [`OrderTracker`] until acquisition points carry `sched-order` annotations
//! (scheduler phase 4). Until then this is the tested primitive, not an
//! active guard.

use std::cell::RefCell;

/// A managed resource as it appears in the generated registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rank {
    pub rank: u32,
    pub ident: &'static str,
}

/// An acquisition that does not ascend past what the task already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderViolation {
    pub held: Rank,
    pub acquired: Rank,
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
    /// acquisition at or below a held rank is refused with the violation;
    /// release builds record without checking.
    pub fn enter(
        &self,
        resource: Rank,
    ) -> Result<OrderGuard<'_>, OrderViolation> {
        let mut held = self.held.borrow_mut();
        if cfg!(debug_assertions)
            && let Some(top) = held.iter().max_by_key(|rank| rank.rank)
            && top.rank >= resource.rank
        {
            return Err(OrderViolation {
                held: *top,
                acquired: resource,
            });
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
        ident: "theseus::state::State.install_admission_locks",
    };
    const DB_PERMIT: Rank = Rank {
        rank: 5,
        ident: "theseus::state::State.install_db_semaphore",
    };
    const CONTENT: Rank = Rank {
        rank: 27,
        ident: "theseus::state::InstanceLockManager.locks",
    };

    #[test]
    fn ascending_acquisition_is_accepted() {
        let tracker = OrderTracker::new();
        let _admission = tracker.enter(ADMISSION).unwrap();
        let _permit = tracker.enter(DB_PERMIT).unwrap();
        let _content = tracker.enter(CONTENT).unwrap();
        assert_eq!(tracker.held(), vec![ADMISSION, DB_PERMIT, CONTENT]);
    }

    // Release builds record without checking, so the violations below only
    // exist in debug builds.
    #[cfg(debug_assertions)]
    #[test]
    fn descending_acquisition_is_a_violation() {
        let tracker = OrderTracker::new();
        let _content = tracker.enter(CONTENT).unwrap();
        assert_eq!(
            tracker.enter(DB_PERMIT).unwrap_err(),
            OrderViolation {
                held: CONTENT,
                acquired: DB_PERMIT,
            }
        );
        assert_eq!(tracker.held(), vec![CONTENT]);
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
        drop(tracker.enter(CONTENT).unwrap());
        let _permit = tracker.enter(DB_PERMIT).unwrap();
        assert_eq!(tracker.held(), vec![DB_PERMIT]);
    }
}
