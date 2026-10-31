//! L0's own executor: a `current_thread` runtime driven by one long-lived OS
//! thread, with its own small blocking pool. L0 work therefore neither waits
//! for the shared blocking pool nor competes with the main runtime's workers.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use super::control::ControlTable;

/// How often the driver thread writes `L0_HEARTBEAT_AT`.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
/// The bounded exit window of the driver thread after a quiesce.
pub const EXIT_WINDOW: Duration = Duration::from_secs(5);
/// How long a quiesced L0 task may take to reach its next safety point. The
/// remaining second of [`EXIT_WINDOW`] is the runtime's own shutdown.
const DRAIN_WINDOW: Duration = Duration::from_secs(3);
/// Spec N8: the L0 blocking pool's capacity.
const BLOCKING_THREADS: usize = 4;
pub const DRIVER_THREAD_NAME: &str = "theseus-l0";
pub const BLOCKING_THREAD_NAME: &str = "theseus-l0-blocking";

/// Decrements the pending count even when the task is dropped.
struct PendingGuard(Arc<AtomicUsize>);

impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct L0Executor {
    handle: tokio::runtime::Handle,
    control: Arc<ControlTable>,
    pending: Arc<AtomicUsize>,
    thread: std::thread::JoinHandle<()>,
}

impl L0Executor {
    pub fn start(control: Arc<ControlTable>) -> std::io::Result<Self> {
        let (handle_tx, handle_rx) = std_mpsc::channel();
        let driver_control = control.clone();
        let pending = Arc::new(AtomicUsize::new(0));
        let driver_pending = pending.clone();
        let thread = std::thread::Builder::new()
            .name(DRIVER_THREAD_NAME.into())
            .spawn(move || {
                let runtime =
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .max_blocking_threads(BLOCKING_THREADS)
                        .thread_name(BLOCKING_THREAD_NAME)
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = handle_tx.send(Err(error));
                            return;
                        }
                    };
                let _ = handle_tx.send(Ok(runtime.handle().clone()));
                runtime.block_on(async {
                    while !driver_control.quiesce_requested()
                        && !driver_control.abort_requested()
                    {
                        driver_control.heartbeat();
                        tokio::time::sleep(HEARTBEAT_INTERVAL).await;
                    }
                    // A quiesced task returns at its next safety point; keep
                    // driving the runtime so it can, instead of dropping it
                    // on the spot. Whatever misses the window is dropped by
                    // the bounded runtime shutdown below.
                    let deadline = tokio::time::Instant::now() + DRAIN_WINDOW;
                    while driver_pending.load(Ordering::Acquire) > 0
                        && tokio::time::Instant::now() < deadline
                    {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                });
                runtime.shutdown_timeout(Duration::from_secs(1));
            })?;
        let handle = handle_rx.recv().map_err(|_| {
            std::io::Error::other("L0 driver thread stopped before starting")
        })??;
        Ok(Self {
            handle,
            control,
            pending,
            thread,
        })
    }

    pub fn control(&self) -> &Arc<ControlTable> {
        &self.control
    }

    /// Whether the driver thread is still alive. A finished driver cancels
    /// anything spawned afterwards, so callers must not use this executor
    /// once it is false.
    pub fn is_running(&self) -> bool {
        !self.thread.is_finished()
    }

    /// Runs `task` on the L0 runtime.
    pub fn spawn<F>(&self, task: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.pending.fetch_add(1, Ordering::AcqRel);
        let pending = self.pending.clone();
        self.handle.spawn(async move {
            let _guard = PendingGuard(pending);
            task.await
        })
    }

    /// Runs blocking work on L0's own pool, never the shared one.
    pub fn spawn_blocking<F, R>(&self, work: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.handle.spawn_blocking(work)
    }

    /// Asks L0 to quiesce and waits up to [`EXIT_WINDOW`] for the driver
    /// thread to leave. Returns whether it did; if not, the thread is left
    /// behind and the loss is logged.
    pub fn shutdown(self) -> bool {
        self.control.request_quiesce();
        let deadline = std::time::Instant::now() + EXIT_WINDOW;
        while !self.thread.is_finished() {
            if std::time::Instant::now() >= deadline {
                tracing::warn!(
                    "L0 driver thread did not exit within {EXIT_WINDOW:?}"
                );
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.thread.join();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn l0_work_runs_on_its_own_threads() {
        let executor =
            L0Executor::start(Arc::new(ControlTable::new())).unwrap();
        let task_thread = executor
            .spawn(async { std::thread::current().name().map(str::to_owned) })
            .await
            .unwrap();
        assert_eq!(task_thread.as_deref(), Some(DRIVER_THREAD_NAME));
        let blocking_thread = executor
            .spawn_blocking(|| std::thread::current().name().map(str::to_owned))
            .await
            .unwrap();
        assert_eq!(blocking_thread.as_deref(), Some(BLOCKING_THREAD_NAME));
        assert!(executor.shutdown());
    }

    #[test]
    fn a_saturated_shared_blocking_pool_does_not_stall_l0() {
        // One blocking thread, held by the occupier: if L0's blocking work
        // went to this pool it could not run until the occupier releases.
        let shared = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let release = Arc::new(std::sync::Barrier::new(2));
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let occupier = {
            let release = release.clone();
            let started = started.clone();
            shared.spawn_blocking(move || {
                started.store(true, std::sync::atomic::Ordering::SeqCst);
                release.wait();
            })
        };
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }

        let executor =
            L0Executor::start(Arc::new(ControlTable::new())).unwrap();
        let result = shared.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(1),
                executor.spawn_blocking(|| 7),
            )
            .await
        });
        assert_eq!(
            result.expect("L0 does not queue behind the occupied shared pool"),
            Ok(7)
        );
        release.wait();
        shared.block_on(occupier).unwrap();
        assert!(executor.shutdown());
    }

    #[test]
    fn quiesce_lets_a_pending_task_reach_its_safety_point() {
        let control = Arc::new(ControlTable::new());
        let executor = L0Executor::start(control.clone()).unwrap();
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_started = started.clone();
        let task_finished = finished.clone();
        let task_control = control.clone();
        let _task = executor.spawn(async move {
            task_started.store(true, std::sync::atomic::Ordering::SeqCst);
            while !task_control.quiesce_requested() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            task_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }
        assert!(executor.shutdown());
        assert!(
            finished.load(std::sync::atomic::Ordering::SeqCst),
            "the task reached its safety point before the driver exited"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cross_runtime_round_trip_cost() {
        const TRIPS: u32 = 1_000;
        let executor =
            L0Executor::start(Arc::new(ControlTable::new())).unwrap();
        let started = Instant::now();
        for trip in 0..TRIPS {
            assert_eq!(
                executor.spawn(async move { trip }).await.unwrap(),
                trip
            );
        }
        let average = started.elapsed() / TRIPS;
        println!(
            "cross-runtime round trip: {average:?} on average over {TRIPS} trips"
        );
        assert!(
            average < Duration::from_millis(50),
            "round trip took {average:?}"
        );
        assert!(executor.shutdown());
    }
}
