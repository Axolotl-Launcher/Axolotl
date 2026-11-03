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
    table_of(EXECUTOR.get())
}

/// The table `control` reports, with the installed cell passed in.
fn table_of(
    installed: Option<&Option<L0Executor>>,
) -> Option<&Arc<ControlTable>> {
    installed.and_then(Option::as_ref).map(L0Executor::control)
}

/// Exit: asks L0 to quiesce; its driver leaves at the next safety point.
/// The executor stays in place (it is static), so this never joins it.
pub fn request_exit() {
    if let Some(control) = control() {
        control.request_quiesce();
    }
}

/// Waits until `deadline` for the L0 driver to leave after
/// [`request_exit`]. `None` when it left or never started; otherwise the
/// time of L0's last safety point, for the exit loss list.
pub fn wait_exit(deadline: std::time::Instant) -> Option<u64> {
    let executor = EXECUTOR.get()?.as_ref()?;
    if executor.wait_for_exit(deadline) {
        None
    } else {
        Some(executor.control().safety_point_at())
    }
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

    #[tokio::test]
    async fn an_exited_executor_falls_back_to_the_current_runtime() {
        // `shutdown` consumes the executor, and the branch under test needs a
        // reference to a stopped one: leak this one and quiesce it in place.
        let control = Arc::new(ControlTable::new());
        let stopped: &'static L0Executor =
            Box::leak(Box::new(L0Executor::start(control.clone()).unwrap()));
        let receiver = stopped.spawn(async { 1_u32 });
        assert_eq!(
            receiver.await.unwrap(),
            1,
            "the live executor runs the work"
        );

        control.request_quiesce();
        let deadline = std::time::Instant::now() + executor::EXIT_WINDOW;
        while stopped.is_running() {
            assert!(
                std::time::Instant::now() < deadline,
                "the driver did not leave within its exit window"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        // Binding work into the closed runtime would cancel it, so it runs on
        // this runtime instead of being dropped.
        let received = dispatch(Some(stopped), async { 11_u32 });
        assert_eq!(received.await.unwrap(), 11);
        // And the token must not come from the stopped executor's quiesced
        // table: the work it guards now runs here, under this shutdown.
        assert!(!token_for(Some(stopped)).is_requested());
    }

    #[test]
    fn the_control_table_is_the_executors_own_and_absent_without_one() {
        // The cell is passed in, so both states of the process-wide lookup are
        // covered without starting the real executor: `control` only reads
        // this cell.
        assert!(table_of(None).is_none(), "a missing cell has no table");
        assert!(
            table_of(Some(&None)).is_none(),
            "a cell whose driver could not start has no table"
        );

        let control = Arc::new(ControlTable::new());
        let l0 = L0Executor::start(control.clone()).unwrap();
        let installed = Some(l0);
        let table =
            table_of(Some(&installed)).expect("a live executor has a table");
        assert!(
            Arc::ptr_eq(table, &control),
            "the reported table is the executor's own"
        );
        assert!(installed.unwrap().shutdown());
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
