use crate::util::download::constants::DOWNLOAD_INITIAL_SPEED_FLOOR as INITIAL_SPEED_FLOOR;
use crate::util::download::constants::DOWNLOAD_SPEED_SAMPLE_HISTORY as SAMPLE_HISTORY;
use crate::util::download::constants::DOWNLOAD_SPEED_SAMPLE_INTERVAL_MS as SAMPLE_INTERVAL_MS;
use crate::util::download::constants::{
    DOWNLOAD_PERCENT_BASE, DOWNLOAD_SPEED_FLOOR_PERCENT,
    DOWNLOAD_SPEED_MIN_SAMPLES, DOWNLOAD_SPEED_RECENT_SAMPLES,
};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
pub struct SpeedSnapshot {
    pub aggregate_speed: u64,
    pub recent_average: u64,
    pub speed_floor: u64,
    pub sample_count: usize,
}

struct SpeedState {
    last_sample_at: Instant,
    last_sample_bytes: u64,
    history: VecDeque<u64>,
    current_speed: u64,
    speed_floor: u64,
}

impl Default for SpeedState {
    fn default() -> Self {
        Self {
            last_sample_at: Instant::now(),
            last_sample_bytes: 0,
            history: VecDeque::new(),
            current_speed: 0,
            speed_floor: INITIAL_SPEED_FLOOR,
        }
    }
}

pub struct DownloadSpeedTracker {
    completed_bytes: AtomicU64,
    speed: Mutex<SpeedState>,
}

impl Default for DownloadSpeedTracker {
    fn default() -> Self {
        Self {
            completed_bytes: AtomicU64::new(0),
            speed: Mutex::new(SpeedState::default()),
        }
    }
}

impl DownloadSpeedTracker {
    pub fn record_bytes(&self, bytes: u64) {
        self.completed_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn speed_snapshot(&self) -> SpeedSnapshot {
        let now = Instant::now();
        let total = self.completed_bytes.load(Ordering::Relaxed);
        let mut state = self.speed.lock();
        let elapsed = now.duration_since(state.last_sample_at);
        if elapsed.as_millis() >= SAMPLE_INTERVAL_MS {
            let downloaded = total.saturating_sub(state.last_sample_bytes);
            let instantaneous = (downloaded as f64
                / elapsed.as_secs_f64().max(f64::MIN_POSITIVE))
                as u64;
            state.history.push_front(instantaneous);
            if state.history.len() > SAMPLE_HISTORY {
                state.history.pop_back();
            }
            state.last_sample_at = now;
            state.last_sample_bytes = total;

            let mut weight = state.history.len() as u64;
            let mut weighted_total = 0_u64;
            let mut weight_total = 0_u64;
            for sample in &state.history {
                weighted_total = weighted_total
                    .saturating_add(sample.saturating_mul(weight));
                weight_total += weight;
                weight = weight.saturating_sub(1);
            }
            state.current_speed = weighted_total / weight_total.max(1);

            if state.history.len() >= DOWNLOAD_SPEED_MIN_SAMPLES {
                let recent_count =
                    state.history.len().min(DOWNLOAD_SPEED_RECENT_SAMPLES);
                let recent_average =
                    state.history.iter().take(recent_count).sum::<u64>()
                        / recent_count as u64;
                let next_floor = recent_average
                    .saturating_mul(DOWNLOAD_SPEED_FLOOR_PERCENT)
                    / DOWNLOAD_PERCENT_BASE;
                state.speed_floor = state.speed_floor.max(next_floor);
            }
        }
        let recent_count =
            state.history.len().min(DOWNLOAD_SPEED_RECENT_SAMPLES);
        let recent_average = if recent_count == 0 {
            0
        } else {
            state.history.iter().take(recent_count).sum::<u64>()
                / recent_count as u64
        };
        SpeedSnapshot {
            aggregate_speed: state.current_speed,
            recent_average,
            speed_floor: state.speed_floor,
            sample_count: state.history.len(),
        }
    }
}
