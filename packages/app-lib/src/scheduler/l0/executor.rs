//! L0's own executor: a `current_thread` runtime driven by one long-lived OS
//! thread, with its own small blocking pool. L0 work therefore neither waits
//! for the shared blocking pool nor competes with the main runtime's workers.

use std::future::Future;
use std::sync::Arc;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use super::control::ControlTable;

/// How often the driver thread writes `L0_HEARTBEAT_AT`.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
/// The bounded exit window of the driver thread after a quiesce.
pub const EXIT_WINDOW: Duration = Duration::from_secs(5);
const BLOCKING_THREADS: usize = 2;
pub const DRIVER_THREAD_NAME: &str = "theseus-l0";
pub const BLOCKING_THREAD_NAME: &str = "theseus-l0-blocking";

pub struct L0Executor {
    handle: tokio::runtime::Handle,
    control: Arc<ControlTable>,
    thread: std::thread::JoinHandle<()>,
}

impl L0Executor {
    pub fn start(control: Arc<ControlTable>) -> std::io::Result<Self> {
        let (handle_tx, handle_rx) = std_mpsc::channel();
        let driver_control = control.clone();
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
                });
                // L0 tasks still pending are dropped here; quiesced tasks
                // have already returned at a safety point.
                runtime.shutdown_timeout(Duration::from_secs(1));
            })?;
        let handle = handle_rx.recv().map_err(|_| {
            std::io::Error::other("L0 driver thread stopped before starting")
        })??;
        Ok(Self {
            handle,
            control,
            thread,
        })
    }

    pub fn control(&self) -> &Arc<ControlTable> {
        &self.control
    }

    /// Runs `task` on the L0 runtime.
    pub fn spawn<F>(&self, task: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle.spawn(task)
    }

    /// Runs blocking work on L0's own pool, never the shared one.
    pub fn spawn_blocking<F, R>(&self, work: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.handle.spawn_blocking(work)
    }

    /// Waits until `deadline` for the driver thread to leave after a
    /// quiesce request; returns whether it did.
    pub fn wait_for_exit(&self, deadline: std::time::Instant) -> bool {
        while !self.thread.is_finished() {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    /// Asks L0 to quiesce and waits up to [`EXIT_WINDOW`] for the driver
    /// thread to leave. Returns whether it did; if not, the thread is left
    /// behind and the loss is logged.
    pub fn shutdown(self) -> bool {
        self.control.request_quiesce();
        if !self.wait_for_exit(std::time::Instant::now() + EXIT_WINDOW) {
            tracing::warn!(
                "L0 driver thread did not exit within {EXIT_WINDOW:?}"
            );
            return false;
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_saturated_shared_blocking_pool_does_not_stall_l0() {
        let executor =
            L0Executor::start(Arc::new(ControlTable::new())).unwrap();
        let release = Arc::new(std::sync::Barrier::new(2));
        let occupier = {
            let release = release.clone();
            tokio::task::spawn_blocking(move || {
                release.wait();
            })
        };
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            executor.spawn_blocking(|| 7),
        )
        .await
        .expect("L0 blocking work does not queue behind the shared pool")
        .unwrap();
        assert_eq!(result, 7);
        release.wait();
        occupier.await.unwrap();
        assert!(executor.shutdown());
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
