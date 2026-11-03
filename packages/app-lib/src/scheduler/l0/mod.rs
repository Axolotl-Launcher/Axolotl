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
use quiesce::QuiesceToken;

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
/// executor is unavailable or has already exited.
pub fn spawn<F>(task: F) -> tokio::sync::oneshot::Receiver<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    dispatch(executor(), task)
}

/// The decision `spawn` makes, with the executor passed in so both branches
/// are testable without the process-wide `EXECUTOR`.
fn dispatch<F>(
    executor: Option<&L0Executor>,
    task: F,
) -> tokio::sync::oneshot::Receiver<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match executor {
        Some(executor) if executor.is_running() => executor.spawn(task),
        Some(_) => {
            // A finished driver binds later tasks into a closed runtime and
            // cancels them. Running the work on the current runtime is worse
            // for isolation than the L0 executor, but it is not silently
            // dropped; phase 7's exit barrier re-creates the executor.
            tracing::warn!(
                "The L0 executor has exited; running L0 work on the current runtime"
            );
            spawn_on_current(task)
        }
        None => spawn_on_current(task),
    }
}

fn spawn_on_current<F>(task: F) -> tokio::sync::oneshot::Receiver<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (output, received) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = output.send(task.await);
    });
    received
}

/// A quiesce token over the live executor's control table, so a detector sees
/// the same quiesce request as the executor's shutdown path.
pub fn quiesce_token() -> QuiesceToken {
    token_for(executor())
}

/// The token `quiesce_token` builds, with the executor passed in.
fn token_for(executor: Option<&L0Executor>) -> QuiesceToken {
    match executor {
        Some(executor) if executor.is_running() => {
            QuiesceToken::new(Arc::clone(executor.control()))
        }
        // No live executor: the task runs on the current runtime, where the
        // app's own shutdown cancels it. A token over the dead executor's
        // table would already be quiesced and the task would return at once.
        _ => QuiesceToken::new(Arc::new(ControlTable::new())),
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

    #[tokio::test]
    async fn work_runs_on_the_l0_executor_and_on_the_caller_without_one() {
        let control = Arc::new(ControlTable::new());
        let l0 = L0Executor::start(control.clone()).unwrap();
        let received = dispatch(Some(&l0), async { 7_u32 });
        assert_eq!(received.await.unwrap(), 7);
        assert!(l0.shutdown());

        // No executor: the same work runs on the current runtime.
        let received = dispatch(None, async { 9_u32 });
        assert_eq!(received.await.unwrap(), 9);
    }

    #[tokio::test]
    async fn a_token_follows_the_executor_it_is_given() {
        let control = Arc::new(ControlTable::new());
        let l0 = L0Executor::start(control.clone()).unwrap();
        let live = token_for(Some(&l0));
        assert!(!live.is_requested());
        control.request_quiesce();
        assert!(live.is_requested(), "the token reads the executor's table");
        assert!(l0.shutdown());

        // Without an executor the token has its own table, so it is not
        // already quiesced by a dead executor's flags.
        let detached = token_for(None);
        assert!(!detached.is_requested());
    }

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
