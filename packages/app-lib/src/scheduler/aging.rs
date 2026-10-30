//! Aging (M5). A waiting unit's effective band rises one step per
//! `AGING_INTERVAL` of waiting, up to `MAX_AGING_STEPS` steps, and never above
//! its base band's privilege ceiling for service order (aging never grants
//! preemption; see `bands::preempts`).
//!
//! Wait bound: a preemptible unit that has aged to the top of the queue waits
//! at most the aging time plus one service unit,
//! `T_max_wait <= MAX_AGING_STEPS * AGING_INTERVAL + T_max_service`
//! (`<= 1 s + T_max_service` with the defaults). The bound holds only for
//! preemptible units with a bounded service time; L3 long jobs have none.

use std::time::Duration;

use super::tag::Level;

pub const AGING_INTERVAL: Duration = Duration::from_millis(250);
pub const MAX_AGING_STEPS: u32 = 4;
/// Longest service of one preemptible unit (spec §11.6 N11).
pub const T_MAX_SERVICE: Duration = Duration::from_secs(1);

/// Effective band index for service order: lower serves first.
pub fn effective_band(level: Level, waited: Duration) -> u32 {
    let base = level as u32;
    let steps = (waited.as_millis() / AGING_INTERVAL.as_millis())
        .min(u128::from(MAX_AGING_STEPS)) as u32;
    base.saturating_sub(steps)
}

/// The wait bound above, for units that can be preempted.
pub fn max_wait_bound() -> Duration {
    AGING_INTERVAL * MAX_AGING_STEPS + T_MAX_SERVICE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_raises_the_effective_band_one_step_per_interval() {
        assert_eq!(effective_band(Level::L4, Duration::ZERO), 4);
        assert_eq!(effective_band(Level::L4, AGING_INTERVAL), 3);
        assert_eq!(effective_band(Level::L4, AGING_INTERVAL * 2), 2);
    }

    #[test]
    fn aging_is_capped_and_never_goes_below_band_zero() {
        assert_eq!(
            effective_band(Level::L4, AGING_INTERVAL * 100),
            4 - MAX_AGING_STEPS
        );
        assert_eq!(effective_band(Level::L1, AGING_INTERVAL * 100), 0);
    }

    #[test]
    fn the_wait_bound_is_one_second_plus_one_service() {
        assert_eq!(max_wait_bound(), Duration::from_secs(1) + T_MAX_SERVICE);
    }
}
