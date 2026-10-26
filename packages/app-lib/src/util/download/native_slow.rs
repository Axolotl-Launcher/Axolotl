//! Slow-transfer and optimal-stop policy for the native download engine.

use crate::util::download::constants::NATIVE_SLOW_ABSOLUTE_SPEED_FLOOR as ABSOLUTE_SPEED_FLOOR;
use crate::util::download::constants::NATIVE_SLOW_COLD_SPEED_FLOOR as COLD_SPEED_FLOOR;
#[cfg(not(test))]
use crate::util::download::constants::NATIVE_SLOW_IDLE_TIMEOUT as IDLE_TIMEOUT;
#[cfg(test)]
use crate::util::download::constants::NATIVE_SLOW_IDLE_TIMEOUT as IDLE_TIMEOUT;
use crate::util::download::constants::NATIVE_SLOW_MIN_FLOW as MIN_FLOW;
use crate::util::download::constants::NATIVE_SLOW_MIN_SAVINGS as MIN_SAVINGS;
use crate::util::download::constants::NATIVE_SLOW_RECONNECT_OVERHEAD as RECONNECT_OVERHEAD;
use crate::util::download::constants::NATIVE_SLOW_REQUIRED_WINDOWS as REQUIRED_SLOW_WINDOWS;
use crate::util::download::constants::NATIVE_SLOW_WINDOW as WINDOW;
use crate::util::download::constants::{
    DOWNLOAD_PERCENT_BASE, NATIVE_SLOW_EXPECTED_PERCENT,
    NATIVE_SWITCH_MIN_SAVINGS_RATIO, NATIVE_SWITCH_PERCENT_BASE,
    NATIVE_SWITCH_SPEED_PERCENT, RECONNECT_OVERHEAD_MAX,
    RECONNECT_OVERHEAD_MIN,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SlowDecision {
    Continue,
    Probe { bytes_per_second: u64 },
    Idle { elapsed: Duration },
    Commit,
}

pub(crate) struct NativeSlowPolicy {
    window: Duration,
    min_flow: Duration,
    started_at: Instant,
    window_started_at: Instant,
    window_start_bytes: u64,
    slow_windows: u8,
    committed: bool,
    expected_speed: Option<u64>,
    observed_speed: u64,
    probe_small_transfers: bool,
    last_observed_bytes: u64,
    last_data_at: Instant,
}

impl NativeSlowPolicy {
    pub(crate) fn observe_with_pressure(
        &mut self,
        downloaded: u64,
        remaining: u64,
        pressure: super::local_resources::Pressure,
    ) -> SlowDecision {
        if pressure.blocks_expansion() {
            self.observed_speed = 0;
            self.window_started_at = Instant::now();
            self.window_start_bytes = downloaded;
            self.slow_windows = 0;
            self.last_data_at = Instant::now();
            self.last_observed_bytes = downloaded;
            SlowDecision::Continue
        } else {
            self.observe(downloaded, remaining)
        }
    }
    pub(crate) fn new(
        starting_bytes: u64,
        expected_speed: Option<u64>,
    ) -> Self {
        let now = Instant::now();
        #[cfg(test)]
        let (window, min_flow) = TEST_TIMING
            .try_with(|timing| *timing)
            .unwrap_or((WINDOW, MIN_FLOW));
        #[cfg(not(test))]
        let (window, min_flow) = (WINDOW, MIN_FLOW);
        Self {
            window,
            min_flow,
            started_at: now,
            window_started_at: now,
            window_start_bytes: starting_bytes,
            slow_windows: 0,
            committed: false,
            expected_speed,
            observed_speed: 0,
            probe_small_transfers: false,
            last_observed_bytes: starting_bytes,
            last_data_at: now,
        }
    }

    pub(crate) fn for_http2(
        starting_bytes: u64,
        expected_speed: Option<u64>,
    ) -> Self {
        Self {
            probe_small_transfers: true,
            ..Self::new(starting_bytes, expected_speed)
        }
    }

    pub(crate) fn observe(
        &mut self,
        downloaded: u64,
        remaining: u64,
    ) -> SlowDecision {
        if downloaded != self.last_observed_bytes {
            self.last_observed_bytes = downloaded;
            self.last_data_at = Instant::now();
        }
        let idle_elapsed = self.last_data_at.elapsed();
        if idle_elapsed >= IDLE_TIMEOUT {
            return SlowDecision::Idle {
                elapsed: idle_elapsed,
            };
        }
        if self.committed {
            return SlowDecision::Continue;
        }
        let elapsed = self.window_started_at.elapsed();
        if elapsed < self.window {
            // The minimum flow age can mature between full windows. Reuse
            // the already-confirmed slow windows instead of delaying another
            // three seconds while a slow tail still has recoverable work.
            if !elapsed.is_zero()
                && self.started_at.elapsed() >= self.min_flow
                && self.slow_windows >= REQUIRED_SLOW_WINDOWS
            {
                let speed = (downloaded.saturating_sub(self.window_start_bytes)
                    as f64
                    / elapsed.as_secs_f64()) as u64;
                if speed < self.speed_floor()
                    && (remaining == 0
                        || estimated_duration(remaining, speed)
                            > RECONNECT_OVERHEAD + MIN_SAVINGS)
                {
                    self.slow_windows = 0;
                    return SlowDecision::Probe {
                        bytes_per_second: speed,
                    };
                }
            }
            return SlowDecision::Continue;
        }
        let speed = (downloaded.saturating_sub(self.window_start_bytes) as f64
            / elapsed.as_secs_f64()) as u64;
        self.window_started_at = Instant::now();
        self.window_start_bytes = downloaded;
        // Learn from complete windows of this transfer, so a cold download
        // can detect a fast prefix followed by a throttled tail. H2 streams
        // share connection bandwidth and retain their conservative floor.
        if !self.probe_small_transfers {
            self.observed_speed = self.observed_speed.max(speed);
        }

        // Zero is also used for unknown length by legacy callers. It must
        // not permanently commit an otherwise stalled or slow transfer.
        if remaining > 0
            && estimated_duration(remaining, speed)
                <= RECONNECT_OVERHEAD + MIN_SAVINGS
        {
            self.slow_windows = 0;
            return SlowDecision::Continue;
        }
        let expected_floor = self.speed_floor();
        if speed < expected_floor {
            self.slow_windows = self.slow_windows.saturating_add(1);
        } else {
            self.slow_windows = 0;
        }
        if self.started_at.elapsed() >= self.min_flow
            && self.slow_windows >= REQUIRED_SLOW_WINDOWS
        {
            self.slow_windows = 0;
            SlowDecision::Probe {
                bytes_per_second: speed,
            }
        } else {
            SlowDecision::Continue
        }
    }

    fn speed_floor(&self) -> u64 {
        let expected_floor = self
            .expected_speed
            .map(|expected| {
                expected.saturating_mul(NATIVE_SLOW_EXPECTED_PERCENT)
                    / DOWNLOAD_PERCENT_BASE
            })
            .unwrap_or(COLD_SPEED_FLOOR)
            .max(ABSOLUTE_SPEED_FLOOR)
            .max(
                self.observed_speed
                    .saturating_mul(NATIVE_SLOW_EXPECTED_PERCENT)
                    / DOWNLOAD_PERCENT_BASE,
            );
        // A multiplexed stream shares bandwidth with other downloads, so a
        // historical single-file peak alone must not trigger H2 retirement.
        if self.probe_small_transfers {
            expected_floor.min(COLD_SPEED_FLOOR)
        } else {
            expected_floor
        }
    }

    pub(crate) fn commit(&mut self) {
        self.committed = true;
    }
}

#[cfg(test)]
tokio::task_local! { static TEST_TIMING: (Duration, Duration); }

#[cfg(test)]
pub(crate) async fn with_test_timing<F: std::future::Future>(
    window: Duration,
    min_flow: Duration,
    future: F,
) -> F::Output {
    TEST_TIMING.scope((window, min_flow), future).await
}

pub(crate) fn should_switch(
    current_speed: u64,
    candidate_speed: u64,
    remaining_bytes: u64,
    restart_bytes: u64,
) -> bool {
    should_switch_with_overhead(
        current_speed,
        candidate_speed,
        remaining_bytes,
        restart_bytes,
        RECONNECT_OVERHEAD,
    )
}

pub(crate) fn should_switch_with_overhead(
    current_speed: u64,
    candidate_speed: u64,
    remaining_bytes: u64,
    restart_bytes: u64,
    reconnect_overhead: Duration,
) -> bool {
    if current_speed == 0 || candidate_speed == 0 {
        return false;
    }
    if candidate_speed.saturating_mul(NATIVE_SWITCH_PERCENT_BASE)
        < current_speed.saturating_mul(NATIVE_SWITCH_SPEED_PERCENT)
    {
        return false;
    }
    let stay = estimated_duration(remaining_bytes, current_speed);
    let switch = reconnect_overhead
        .clamp(RECONNECT_OVERHEAD_MIN, RECONNECT_OVERHEAD_MAX)
        .saturating_add(estimated_duration(restart_bytes, candidate_speed));
    let required_savings =
        MIN_SAVINGS.max(stay.mul_f64(NATIVE_SWITCH_MIN_SAVINGS_RATIO));
    switch.saturating_add(required_savings) < stay
}

pub(crate) fn estimated_duration(bytes: u64, speed: u64) -> Duration {
    if speed == 0 {
        return Duration::MAX;
    }
    Duration::try_from_secs_f64(bytes as f64 / speed as f64)
        .unwrap_or(Duration::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_http1_transfer_detects_fast_prefix_then_500_kib_tail() {
        let mut policy = NativeSlowPolicy::new(0, None);
        let mut downloaded = 5 * 1024 * 1024 * WINDOW.as_secs();
        policy.window_started_at = Instant::now() - WINDOW;
        assert_eq!(
            policy.observe(downloaded, 128 * 1024 * 1024),
            SlowDecision::Continue
        );
        assert!(policy.speed_floor() > 500 * 1024);
        policy.started_at = Instant::now() - MIN_FLOW;
        for index in 0..REQUIRED_SLOW_WINDOWS {
            policy.window_started_at = Instant::now() - WINDOW;
            downloaded += 500 * 1024 * WINDOW.as_secs();
            assert_eq!(
                matches!(
                    policy.observe(downloaded, 64 * 1024 * 1024),
                    SlowDecision::Probe { .. }
                ),
                index + 1 == REQUIRED_SLOW_WINDOWS
            );
        }
    }

    #[test]
    fn partial_windows_and_local_pressure_do_not_teach_a_speed_peak() {
        let mut policy = NativeSlowPolicy::new(0, None);
        policy.observe(5 * 1024 * 1024, 128 * 1024 * 1024);
        assert_eq!(policy.observed_speed, 0);
        policy.observed_speed = 5 * 1024 * 1024;
        policy.observe_with_pressure(
            5 * 1024 * 1024,
            128 * 1024 * 1024,
            super::super::local_resources::Pressure {
                cpu_percent: crate::util::download::constants::LOCAL_CPU_PRESSURE_PERCENT,
                write_latency_ms: 0.0,
            },
        );
        assert_eq!(policy.observed_speed, 0);
        assert_eq!(policy.speed_floor(), COLD_SPEED_FLOOR);
    }

    #[test]
    fn extreme_content_lengths_saturate_eta_and_reconnect_cost_without_panicking()
     {
        assert_eq!(estimated_duration(u64::MAX, 1), Duration::MAX);
        assert!(!should_switch(1, 1, u64::MAX, u64::MAX));
        assert!(should_switch(1, 10, u64::MAX, u64::MAX));
        assert!(!should_switch(1, 10, 1, u64::MAX));
    }

    #[test]
    fn slow_cold_connection_cost_can_outweigh_tail_speed_gain() {
        assert!(should_switch_with_overhead(
            100_000,
            500_000,
            512_000,
            512_000,
            Duration::from_millis(100)
        ));
        assert!(!should_switch_with_overhead(
            100_000,
            500_000,
            512_000,
            512_000,
            Duration::from_secs(3)
        ));
    }

    #[test]
    fn confirmed_slow_windows_do_not_wait_for_an_extra_window_when_minimum_age_matures()
     {
        let mut policy = NativeSlowPolicy::new(0, None);
        policy.started_at = Instant::now() - MIN_FLOW;
        policy.window_started_at = Instant::now() - Duration::from_secs(1);
        policy.slow_windows = REQUIRED_SLOW_WINDOWS;
        assert!(matches!(
            policy.observe(45 * 1024, 512 * 1024),
            SlowDecision::Probe { .. }
        ));
        assert_eq!(policy.slow_windows, 0);
    }

    #[test]
    fn ordinary_small_tail_probes_by_eta_and_keeps_monitoring_fast_tail() {
        for (speed, remaining, expected) in [
            (45 * 1024, 512 * 1024, true),
            (30 * 1024 * 1024, 512 * 1024, false),
        ] {
            let mut policy = NativeSlowPolicy::new(0, None);
            policy.started_at = Instant::now() - MIN_FLOW;
            policy.window_started_at = Instant::now() - WINDOW;
            policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
            let decision = policy.observe(speed * WINDOW.as_secs(), remaining);
            assert_eq!(
                matches!(decision, SlowDecision::Probe { .. }),
                expected
            );
            assert!(!policy.committed);
        }
    }

    #[test]
    fn unknown_remaining_does_not_commit_a_slow_stream() {
        let mut policy = NativeSlowPolicy::new(0, None);
        policy.started_at = Instant::now() - MIN_FLOW;
        policy.window_started_at = Instant::now() - WINDOW;
        policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
        assert!(matches!(
            policy.observe(45 * 1024 * WINDOW.as_secs(), 0),
            SlowDecision::Probe { .. }
        ));
        assert!(!policy.committed);
    }

    #[test]
    fn healthy_http2_stream_is_not_downgraded_against_single_file_peak() {
        for speed in [1024 * 1024, 5 * 1024 * 1024] {
            let mut policy =
                NativeSlowPolicy::for_http2(0, Some(30 * 1024 * 1024));
            policy.started_at = Instant::now() - MIN_FLOW;
            policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
            policy.window_started_at = Instant::now() - WINDOW;
            assert_eq!(
                policy.observe(speed * WINDOW.as_secs(), 128 * 1024 * 1024),
                SlowDecision::Continue,
            );
            assert_eq!(policy.slow_windows, 0);
        }
    }

    #[test]
    fn abnormally_slow_http2_stream_still_recovers_with_fast_history() {
        let mut policy = NativeSlowPolicy::for_http2(0, Some(30 * 1024 * 1024));
        policy.started_at = Instant::now() - MIN_FLOW;
        policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
        policy.window_started_at = Instant::now() - WINDOW;
        assert!(matches!(
            policy.observe(4 * 1024 * WINDOW.as_secs(), 512 * 1024),
            SlowDecision::Probe { .. }
        ));
    }

    #[test]
    fn small_http2_transfer_probes_only_when_reconnect_can_pay_off() {
        for (remaining, expected) in [(512 * 1024, true), (1024, false)] {
            let mut policy = NativeSlowPolicy::for_http2(0, None);
            policy.started_at = Instant::now() - MIN_FLOW;
            policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
            policy.window_started_at = Instant::now() - WINDOW;
            let decision = policy.observe(12 * 1024, remaining);
            assert_eq!(
                matches!(decision, SlowDecision::Probe { .. }),
                expected
            );
        }
    }

    #[test]
    fn small_http2_transfer_keeps_monitoring_after_a_short_eta() {
        let mut policy = NativeSlowPolicy::for_http2(0, None);
        policy.started_at = Instant::now() - MIN_FLOW;
        policy.window_started_at = Instant::now() - WINDOW;
        assert_eq!(policy.observe(12 * 1024, 1024), SlowDecision::Continue);
        assert!(!policy.committed);
        policy.window_started_at = Instant::now() - WINDOW;
        policy.slow_windows = REQUIRED_SLOW_WINDOWS - 1;
        assert!(matches!(
            policy.observe(12 * 1024 + 1, 1023),
            SlowDecision::Probe { .. }
        ));
    }

    #[test]
    fn committed_transfer_still_recovers_from_idle() {
        let mut policy = NativeSlowPolicy::for_http2(0, None);
        policy.commit();
        policy.last_data_at = Instant::now() - IDLE_TIMEOUT;
        assert!(matches!(policy.observe(0, 1024), SlowDecision::Idle { .. }));
    }

    #[test]
    fn local_pressure_does_not_trigger_route_switch_or_idle_failure() {
        let mut policy = NativeSlowPolicy::new(0, None);
        policy.window_started_at = Instant::now() - Duration::from_secs(60);
        assert_eq!(
            policy.observe_with_pressure(
                0,
                64 * 1024 * 1024,
                super::super::local_resources::Pressure {
                    cpu_percent: crate::util::download::constants::LOCAL_CPU_PRESSURE_PERCENT,
                    write_latency_ms: 0.0
                }
            ),
            SlowDecision::Continue
        );
        assert_eq!(policy.observe(0, 64 * 1024 * 1024), SlowDecision::Continue);
        policy.window_started_at = Instant::now() - Duration::from_secs(60);
        assert_eq!(
            policy.observe_with_pressure(
                0,
                64 * 1024 * 1024,
                super::super::local_resources::Pressure {
                    cpu_percent: 0,
                    write_latency_ms: 80.0
                }
            ),
            SlowDecision::Continue
        );
    }

    #[test]
    fn switch_requires_repayment_of_restarted_bytes() {
        assert!(!should_switch(1024, 2048, 1024, 1024 * 1024));
    }

    #[test]
    fn faster_candidate_wins_early_in_a_large_download() {
        assert!(should_switch(
            256 * 1024,
            2 * 1024 * 1024,
            64 * 1024 * 1024,
            64 * 1024 * 1024,
        ));
    }

    #[test]
    fn idle_body_is_a_terminal_recovery_decision() {
        let mut policy = NativeSlowPolicy::new(0, None);
        std::thread::sleep(IDLE_TIMEOUT + Duration::from_millis(10));
        assert!(matches!(
            policy.observe(0, 64 * 1024 * 1024),
            SlowDecision::Idle { .. }
        ));
    }
}
