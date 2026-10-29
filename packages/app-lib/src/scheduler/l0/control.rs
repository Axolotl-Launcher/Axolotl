//! The cross-runtime control table between the main runtime and the L0
//! executor. Only atomics cross the boundary: the main runtime asks for a
//! quiesce or an abort by setting a flag, and reads L0's liveness and last
//! safety point from timestamps. Aborting stays the supervisor's own power.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct ControlTable {
    quiesce_requested: AtomicBool,
    abort_requested: AtomicBool,
    safety_point_at: AtomicU64,
    heartbeat_at: AtomicU64,
}

impl ControlTable {
    pub const fn new() -> Self {
        Self {
            quiesce_requested: AtomicBool::new(false),
            abort_requested: AtomicBool::new(false),
            safety_point_at: AtomicU64::new(0),
            heartbeat_at: AtomicU64::new(0),
        }
    }

    /// `QUIESCE_REQUESTED`: L0 finishes its current unit and returns.
    pub fn request_quiesce(&self) {
        self.quiesce_requested.store(true, Ordering::Release);
    }

    pub fn quiesce_requested(&self) -> bool {
        self.quiesce_requested.load(Ordering::Acquire)
    }

    /// `L0_ABORT_REQUESTED`: the main runtime asks; the supervisor aborts.
    pub fn request_abort(&self) {
        self.abort_requested.store(true, Ordering::Release);
    }

    pub fn abort_requested(&self) -> bool {
        self.abort_requested.load(Ordering::Acquire)
    }

    /// `L0_SAFETY_POINT_AT`: when an L0 task last stood at a safety point.
    pub fn record_safety_point(&self) {
        self.safety_point_at.store(now_millis(), Ordering::Release);
    }

    pub fn safety_point_at(&self) -> u64 {
        self.safety_point_at.load(Ordering::Acquire)
    }

    /// `L0_HEARTBEAT_AT`: L0 liveness, updated by the driver thread.
    pub fn heartbeat(&self) {
        self.heartbeat_at.store(now_millis(), Ordering::Release);
    }

    pub fn heartbeat_at(&self) -> u64 {
        self.heartbeat_at.load(Ordering::Acquire)
    }
}

/// Milliseconds since the Unix epoch; 0 if the clock is before it.
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
