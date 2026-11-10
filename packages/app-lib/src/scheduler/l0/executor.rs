//! L0's own executor: a `current_thread` runtime driven by one long-lived OS
//! thread, with its own small blocking pool. L0 work therefore neither waits
//! for the shared blocking pool nor competes with the main runtime's workers.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::control::ControlTable;
use crate::scheduler::supervisor::Supervisor;

/// A task handed to the driver's supervisor.
type BoxedTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// How many tasks may wait for the driver to enter them into the supervisor.
const SUBMISSION_QUEUE: usize = 64;

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

/// Counts tasks that could not enter the supervisor because the submission
/// queue was full; the drain must wait for them too.
struct DetachedGuard(Arc<AtomicUsize>);

impl Drop for DetachedGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct L0Executor {
    handle: tokio::runtime::Handle,
    control: Arc<ControlTable>,
    submit: mpsc::Sender<BoxedTask>,
    detached: Arc<AtomicUsize>,
    thread: std::thread::JoinHandle<()>,
}

impl L0Executor {
    pub fn start(control: Arc<ControlTable>) -> std::io::Result<Self> {
        let (handle_tx, handle_rx) = std_mpsc::channel();
        let (submit, mut submissions) = mpsc::channel(SUBMISSION_QUEUE);
        let detached = Arc::new(AtomicUsize::new(0));
        let driver_detached = detached.clone();
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
                runtime.block_on(async move {
                    // The driver owns the supervisor: every L0 task is spawned
                    // into one JoinSet, so a panic is reported and the cancel
                    // authority (abort_all) stays here, not on the caller.
                    let mut supervisor = Supervisor::new();
                    let mut heartbeat =
                        tokio::time::interval(HEARTBEAT_INTERVAL);
                    loop {
                        let has_tasks = !supervisor.is_empty();
                        tokio::select! {
                            _ = heartbeat.tick() => driver_control.heartbeat(),
                            Some(task) = submissions.recv() => {
                                supervisor.spawn("l0-task", task);
                            }
                            joined = supervisor.join_next(), if has_tasks => {
                                if let Some(exit) = joined {
                                    tracing::debug!(?exit, "L0 task ended");
                                }
                            }
                        }
                        if driver_control.quiesce_requested()
                            || driver_control.abort_requested()
                        {
                            break;
                        }
                    }
                    // A quiesced task returns at its next safety point; keep
                    // driving and reaping so it can, instead of dropping it on
                    // the spot. Abort is the cancel path and stays immediate.
                    if driver_control.abort_requested() {
                        supervisor.abort_all();
                    } else {
                        let deadline =
                            tokio::time::Instant::now() + DRAIN_WINDOW;
                        while (!supervisor.is_empty()
                            || !submissions.is_empty()
                            || driver_detached.load(Ordering::Acquire) > 0)
                            && tokio::time::Instant::now() < deadline
                        {
                            // An abort that lands during the drain escalates:
                            // cancel is immediate, not a safety-point wait.
                            if driver_control.abort_requested() {
                                break;
                            }
                            // Keep liveness visible while draining: the main
                            // loop's heartbeat stops at the quiesce.
                            driver_control.heartbeat();
                            let has_tasks = !supervisor.is_empty();
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_millis(10)) => {},
                                Some(task) = submissions.recv() => {
                                    supervisor.spawn("l0-task", task);
                                }
                                joined = supervisor.join_next(), if has_tasks => {
                                    let _ = joined;
                                }
                            }
                        }
                        supervisor.abort_all();
                    }
                });
                // The drain above is the grace for async tasks; this drops
                // any task that missed it and waits up to one second for L0
                // blocking work.
                runtime.shutdown_timeout(Duration::from_secs(1));
            })?;
        let handle = handle_rx.recv().map_err(|_| {
            std::io::Error::other("L0 driver thread stopped before starting")
        })??;
        Ok(Self {
            handle,
            control,
            submit,
            detached,
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

    /// Runs `task` on the L0 runtime, supervised by the driver's JoinSet.
    pub fn spawn<F>(&self, task: F) -> oneshot::Receiver<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (output, received) = oneshot::channel();
        let task: BoxedTask = Box::pin(async move {
            let _ = output.send(task.await);
        });
        match self.submit.try_send(task) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(task)) => {
                // The submission queue is bounded; L0 never blocks its
                // caller. The task still runs on L0, just outside the
                // supervisor's JoinSet for this submission; the detached
                // counter keeps the quiesce drain waiting for it.
                tracing::warn!(
                    "L0 submission queue is full; running the task outside the supervisor"
                );
                self.detached.fetch_add(1, Ordering::AcqRel);
                let detached = self.detached.clone();
                let _ = self.handle.spawn(async move {
                    let _guard = DetachedGuard(detached);
                    task.await;
                });
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::warn!(
                    "The L0 executor is shutting down; the task was not run"
                );
            }
        }
        received
    }

    /// Asks the supervisor to abort every pending L0 task (cancel semantics,
    /// no safety-point drain). The driver owns the abort; callers only set the
    /// request flag.
    pub fn abort_all(&self) {
        self.control.request_abort();
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
        let deadline = Instant::now() + Duration::from_secs(2);
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "the occupier did not start");
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
        // Release and join before asserting, so a timeout fails the test
        // instead of leaving the occupier blocked and the runtime drop hung.
        release.wait();
        shared.block_on(occupier).unwrap();
        assert_eq!(
            result
                .expect("L0 does not queue behind the occupied shared pool")
                .expect("the L0 blocking task ran"),
            7
        );
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
            // Stay past the driver's 250 ms heartbeat park: without the
            // bounded drain the runtime would drop this task before it
            // finishes, so the test actually proves the drain.
            tokio::time::sleep(Duration::from_millis(700)).await;
            task_finished.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "the task did not start");
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
