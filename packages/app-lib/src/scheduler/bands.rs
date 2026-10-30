//! The five priority bands (M3). Bands do not overlap and never preempt
//! within themselves. Preemption is decided only by the base band, which is
//! the tag's level; aging moves the effective band, which decides service
//! order and nothing else.

use super::tag::Level;

/// Whether work at `running` may be preempted by work at `incoming`. Only a
/// strictly more privileged band preempts, and never into L0 or L3: L0 is
/// not preemptible (only a reclaim bypass reaches it) and L3 is never
/// preempted by the scheduler, only stopped by process exit.
pub fn preempts(incoming: Level, running: Level) -> bool {
    if matches!(running, Level::L0 | Level::L3) {
        return false;
    }
    incoming < running
}

/// Whether the scheduler may destroy work at `level` to reclaim memory: only
/// L1 (rebuilt afterwards) and L4 (gone once its tag is released).
pub fn swappable(level: Level) -> bool {
    matches!(level, Level::L1 | Level::L4)
}

pub const ALL: [Level; 5] =
    [Level::L0, Level::L1, Level::L2, Level::L3, Level::L4];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_never_preempts_itself() {
        for level in ALL {
            assert!(
                !preempts(level, level),
                "{level:?} preempted its own band"
            );
        }
    }

    #[test]
    fn preemption_follows_the_level_alone() {
        assert!(preempts(Level::L0, Level::L1));
        assert!(preempts(Level::L1, Level::L2));
        assert!(preempts(Level::L2, Level::L4));
        assert!(
            !preempts(Level::L2, Level::L1),
            "less privileged work waits"
        );
    }

    #[test]
    fn l0_and_l3_are_never_preempted_by_the_scheduler() {
        for incoming in ALL {
            assert!(!preempts(incoming, Level::L0));
            assert!(!preempts(incoming, Level::L3));
        }
    }

    #[test]
    fn only_rebuildable_or_tagless_work_is_swapped_out() {
        assert_eq!(
            ALL.into_iter()
                .filter(|level| swappable(*level))
                .collect::<Vec<_>>(),
            vec![Level::L1, Level::L4]
        );
    }
}
