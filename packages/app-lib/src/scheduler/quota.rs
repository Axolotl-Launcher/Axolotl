//! Admission quotas (spec §8.3). Quotas belong to admission, not to
//! scheduling: each class reserves a minimum and is capped at a maximum of
//! concurrent units, and the minimums together must fit inside the global
//! maximum, which is the install job semaphore's capacity.

use super::tag::Level;
use crate::util::download::constants::MAX_CONCURRENT_INSTALL_JOBS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quota {
    pub class: &'static str,
    pub level: Level,
    pub max: usize,
    pub min: usize,
}

/// `LevelQuota { l3_install: {2,1}, l3_launch: {1,0}, l2: {2,0}, l4: {4,0} }`.
pub const LEVEL_QUOTAS: [Quota; 4] = [
    Quota {
        class: "l3_install",
        level: Level::L3,
        max: 2,
        min: 1,
    },
    Quota {
        class: "l3_launch",
        level: Level::L3,
        max: 1,
        min: 0,
    },
    Quota {
        class: "l2",
        level: Level::L2,
        max: 2,
        min: 0,
    },
    Quota {
        class: "l4",
        level: Level::L4,
        max: 4,
        min: 0,
    },
];

/// `global: { max: 2 }`.
pub const GLOBAL_MAX: usize = MAX_CONCURRENT_INSTALL_JOBS;

/// `sum(min) <= global.max`, and no class reserves more than it may use.
///
/// A class `max` above `global_max` is not an error: the global semaphore
/// already caps every class, so the local maximum only documents an intent
/// the global slot cannot satisfy. [`classes_capped_by_global`] reports it.
pub fn quotas_fit(quotas: &[Quota], global_max: usize) -> bool {
    quotas.iter().all(|quota| quota.min <= quota.max)
        && quotas.iter().map(|quota| quota.min).sum::<usize>() <= global_max
}

/// Classes whose own maximum exceeds what the global semaphore can hand out.
/// Not a failure — the global cap wins — but worth a startup warning, because
/// the class will never see its stated maximum.
pub fn classes_capped_by_global(
    quotas: &[Quota],
    global_max: usize,
) -> Vec<&'static str> {
    quotas
        .iter()
        .filter(|quota| quota.max > global_max)
        .map(|quota| quota.class)
        .collect()
}

/// The startup assertions of this module and the queue bound (spec §11.6
/// N12). They log instead of panicking, so a misconfiguration degrades
/// admission rather than stopping the launcher.
pub fn check_at_startup() -> bool {
    let quotas = quotas_fit(&LEVEL_QUOTAS, GLOBAL_MAX);
    if !quotas {
        tracing::error!(
            "Scheduler minimum quotas exceed the global install concurrency"
        );
    }
    let queue = super::fairness::queue_bound_fits_stall_timeout();
    if !queue {
        tracing::error!("Scheduler queue wait bound exceeds the stall timeout");
    }
    let capped = classes_capped_by_global(&LEVEL_QUOTAS, GLOBAL_MAX);
    if !capped.is_empty() {
        tracing::warn!(
            classes = ?capped,
            global_max = GLOBAL_MAX,
            "Scheduler class maxima exceed the global install concurrency; the global cap applies"
        );
    }
    // Reported, not enforced: the spec says a misconfiguration degrades
    // admission rather than stopping the launcher, so the startup path logs
    // and continues. The `debug_assert!` makes a developer build fail loudly.
    debug_assert!(
        quotas && queue,
        "scheduler startup checks must hold: quotas={quotas} queue={queue}"
    );
    quotas && queue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_min_le_global_max() {
        assert!(check_at_startup());
        assert!(quotas_fit(&LEVEL_QUOTAS, GLOBAL_MAX));
        assert!(quotas_fit(&LEVEL_QUOTAS, 1), "only one slot is reserved");
        assert!(!quotas_fit(&LEVEL_QUOTAS, 0));
    }

    #[test]
    fn a_class_cannot_reserve_more_than_its_maximum() {
        let broken = [Quota {
            class: "broken",
            level: Level::L2,
            max: 1,
            min: 2,
        }];
        assert!(!quotas_fit(&broken, 10));
    }
}
