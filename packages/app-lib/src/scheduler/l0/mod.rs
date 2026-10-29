//! The L0 level: resident detection work (process and server monitoring, log
//! and crash collection, database session maintenance, the supervisor). L0
//! detects and dispatches; every reaction that needs a lock, a database write,
//! blocking I/O or an await on other work runs at another level.

pub mod control;
pub mod executor;
pub mod queue;
pub mod quiesce;

use std::future::Future;
use std::sync::{Arc, OnceLock};

use control::ControlTable;
use executor::L0Executor;

static EXECUTOR: OnceLock<Option<L0Executor>> = OnceLock::new();

/// The launcher's L0 executor, started once on first use. `None` if its
/// driver thread could not start; callers then run the task on the current
/// runtime.
fn executor() -> Option<&'static L0Executor> {
    EXECUTOR
        .get_or_init(|| {
            L0Executor::start(Arc::new(ControlTable::new()))
                .inspect_err(|error| {
                    tracing::error!(%error, "Could not start the L0 executor");
                })
                .ok()
        })
        .as_ref()
}

/// Runs an L0 task on the L0 executor, or on the current runtime when the
/// executor is unavailable.
pub fn spawn<F>(task: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match executor() {
        Some(executor) => executor.spawn(task),
        None => tokio::spawn(task),
    }
}

/// The L0 executor's control table, for the main runtime's shutdown path.
pub fn control() -> Option<&'static Arc<ControlTable>> {
    EXECUTOR.get()?.as_ref().map(L0Executor::control)
}

#[cfg(test)]
mod heartbeat_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_driver_beats_and_exits_within_its_window() {
        let control = Arc::new(ControlTable::new());
        let l0 = L0Executor::start(control.clone()).unwrap();
        let first = wait_for_beat(&control, 0);
        let second = wait_for_beat(&control, first);
        assert!(second > first, "the heartbeat advances");
        let started = std::time::Instant::now();
        assert!(l0.shutdown(), "the driver exits after a quiesce");
        assert!(started.elapsed() <= executor::EXIT_WINDOW);
    }

    fn wait_for_beat(control: &ControlTable, after: u64) -> u64 {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let beat = control.heartbeat_at();
            if beat > after {
                return beat;
            }
            assert!(std::time::Instant::now() < deadline, "no heartbeat");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
