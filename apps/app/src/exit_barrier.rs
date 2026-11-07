//! The exit barrier (spec §8.6). Every `ExitRequested` re-arms the deadline,
//! so an exit interrupted earlier never leaves a stale one behind. Steps 1–5
//! run in `ExitRequested`, steps 6–7 in `Exit`:
//!
//! 1. re-arm `EXIT_DEADLINE_MS` (now + the hard cap) and reset `POOL_CLOSED`;
//! 2. flush the pending skin change;
//! 3. stop multiplayer and MCP services within the soft window (5 s);
//! 4. stop dedicated servers (their own 5 s budget);
//! 5. cancel and abort what is left below and above L3 — never L3 itself —
//!    and record the losses;
//! 6. close the database pool (3 s); from here nothing is aborted any more,
//!    only recorded;
//! 7. exit, or restart through `request_restart` once the pool is closed.
//!
//! Budget: 5 s + 5 s + 3 s = 13 s, under the 15 s hard cap. At the hard cap
//! the barrier stops waiting, records the losses and exits the process; L3
//! work is never killed by the barrier, it ends with the process and is
//! settled as interrupted on the next start. Installing an update waits on
//! the user by design, so the cap is lifted while it runs.
//!
//! Exit paths: the frontend's `exit_app`, the tray's quit, the frontend's
//! `restart_app` and the updater's restart all go through the barrier. The
//! NSIS installer's own `exit(0)` and an OS shutdown or forced kill cannot be
//! intercepted.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use theseus::scheduler::tag::Level;

pub const SOFT_WINDOW: Duration = Duration::from_secs(5);
pub const HARD_CAP: Duration = Duration::from_secs(15);
/// Steps 3, 4 and 6.
pub const STEP_BUDGETS: [Duration; 3] = [
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(3),
];

/// Milliseconds since the Unix epoch; 0 while unarmed. Re-armed on every
/// `ExitRequested`, so it must never be a write-once cell.
static EXIT_DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
static POOL_CLOSED: AtomicBool = AtomicBool::new(false);
/// The re-entrancy guard: set while one barrier runs.
static EXIT_RUNNING: AtomicBool = AtomicBool::new(false);
static WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// One run of steps 1–5. Dropping it, finished or interrupted, releases the
/// guard, so the next `ExitRequested` runs the whole barrier again.
#[derive(Debug)]
#[must_use = "the barrier is released when this is dropped"]
pub struct ExitPass(());

impl Drop for ExitPass {
    fn drop(&mut self) {
        EXIT_RUNNING.store(false, Ordering::Release);
    }
}

/// Step 1. `None` while another barrier runs, which a nested
/// `ExitRequested` must not start again.
pub fn begin(now_ms: u64) -> Option<ExitPass> {
    if EXIT_RUNNING.swap(true, Ordering::AcqRel) {
        return None;
    }
    EXIT_DEADLINE_MS
        .store(now_ms + HARD_CAP.as_millis() as u64, Ordering::Release);
    POOL_CLOSED.store(false, Ordering::Release);
    Some(ExitPass(()))
}

pub fn deadline_ms() -> u64 {
    EXIT_DEADLINE_MS.load(Ordering::Acquire)
}

/// Lifts the hard cap, for waits that are by design (an update install).
pub fn disarm() {
    EXIT_DEADLINE_MS.store(0, Ordering::Release);
}

pub fn mark_pool_closed() {
    POOL_CLOSED.store(true, Ordering::Release);
}

pub fn pool_closed() -> bool {
    POOL_CLOSED.load(Ordering::Acquire)
}

/// What the hard cap does once the deadline passes.
#[derive(Debug, PartialEq, Eq)]
pub enum CapCheck {
    /// Unarmed: nothing to watch.
    Disarmed,
    /// Check again after this long.
    Wait(Duration),
    Reached,
}

pub fn check_cap(now_ms: u64) -> CapCheck {
    match deadline_ms() {
        0 => CapCheck::Disarmed,
        deadline if now_ms >= deadline => CapCheck::Reached,
        deadline => {
            CapCheck::Wait(Duration::from_millis((deadline - now_ms).min(250)))
        }
    }
}

/// Starts the hard-cap watchdog once. It follows the deadline as it is
/// re-armed; when the cap is reached it records the losses and exits.
pub fn watch_hard_cap(losses: fn() -> Vec<Loss>) {
    if WATCHDOG_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            match check_cap(now_millis()) {
                CapCheck::Disarmed => {
                    // Re-armed by a later exit; keep following it.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                CapCheck::Wait(wait) => tokio::time::sleep(wait).await,
                CapCheck::Reached => {
                    tracing::error!(
                        "Exit did not finish within the {}s hard cap; exiting without waiting further",
                        HARD_CAP.as_secs()
                    );
                    record(&losses());
                    std::process::exit(0);
                }
            }
        }
    });
}

/// One entry of the loss list (spec §8.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loss {
    /// ① An L3 job that had not settled; the next start settles it as
    /// interrupted by the app closing (④).
    L3Interrupted { label: String },
    /// ② L1 state, which may be lost and is rebuilt.
    L1Dropped { label: String },
    /// ⑤ Abandoned work below or above L3.
    Abandoned { level: Level, label: String },
    /// ⑥ An L3 job in its file-publishing stage: the operation journal keeps
    /// its staged files and publish point, and startup reconciliation
    /// finishes or rolls back the publish.
    L3Publishing { label: String },
}

/// Work still running when step 5 or the hard cap is reached.
pub struct Leftover {
    pub level: Level,
    pub label: String,
    pub publishing: bool,
    pub abort: Box<dyn FnOnce() + Send>,
}

/// The action table of step 5: abort leftovers other than L3 while the pool
/// is open, never L3, and return what was lost. After the pool closed,
/// nothing is aborted; every leftover is only recorded.
pub fn settle_leftovers(leftovers: Vec<Leftover>) -> Vec<Loss> {
    let closed = pool_closed();
    leftovers
        .into_iter()
        .map(|leftover| match leftover.level {
            Level::L3 if leftover.publishing => Loss::L3Publishing {
                label: leftover.label,
            },
            Level::L3 => Loss::L3Interrupted {
                label: leftover.label,
            },
            level => {
                if !closed {
                    (leftover.abort)();
                }
                if level == Level::L1 {
                    Loss::L1Dropped {
                        label: leftover.label,
                    }
                } else {
                    Loss::Abandoned {
                        level,
                        label: leftover.label,
                    }
                }
            }
        })
        .collect()
}

pub fn record(losses: &[Loss]) {
    for loss in losses {
        tracing::warn!(?loss, "Work lost at exit");
    }
}

/// Restarting the app after the barrier.
pub trait Restarter {
    /// Asks for an exit with restart; never restarts on the spot.
    fn request_restart(&self);
}

/// Steps 6–7: close the database session first, then restart if an update
/// asked for it, so a restart never skips the clean-shutdown record.
pub fn finish_exit(
    finish_database: impl FnOnce(),
    restart: bool,
    restarter: &impl Restarter,
) {
    finish_database();
    mark_pool_closed();
    if restart {
        restarter.request_restart();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    /// The statics are process-wide; the tests that touch them take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn leftover(
        level: Level,
        publishing: bool,
        aborts: &Arc<AtomicUsize>,
    ) -> Leftover {
        let aborts = Arc::clone(aborts);
        Leftover {
            level,
            label: format!("{level:?}"),
            publishing,
            abort: Box::new(move || {
                aborts.fetch_add(1, Ordering::SeqCst);
            }),
        }
    }

    #[test]
    fn the_budget_fits_under_the_hard_cap() {
        let total: Duration = STEP_BUDGETS.iter().sum();
        assert_eq!(total, Duration::from_secs(13));
        assert!(total < HARD_CAP);
        assert_eq!(STEP_BUDGETS[0], SOFT_WINDOW);
    }

    #[test]
    fn second_exit_after_interrupted_first_still_arms_barrier() {
        let _serial = serial();
        let first = begin(1_000).unwrap();
        mark_pool_closed();
        drop(first); // the first exit was interrupted
        let second =
            begin(50_000).expect("a later exit runs the barrier again");
        assert_eq!(deadline_ms(), 50_000 + HARD_CAP.as_millis() as u64);
        assert!(!pool_closed(), "the pool flag is reset with the deadline");
        assert_eq!(
            check_cap(50_000 + 14_999),
            CapCheck::Wait(Duration::from_millis(1))
        );
        assert_eq!(
            check_cap(50_000 + HARD_CAP.as_millis() as u64),
            CapCheck::Reached,
            "the hard cap follows the new deadline"
        );
        drop(second);
    }

    #[test]
    fn exit_reentrancy_guard_skips_a_nested_exit() {
        let _serial = serial();
        let pass = begin(1_000).unwrap();
        assert!(
            begin(2_000).is_none(),
            "a nested ExitRequested starts nothing"
        );
        assert_eq!(deadline_ms(), 1_000 + HARD_CAP.as_millis() as u64);
        drop(pass);
        let again = begin(3_000);
        assert!(again.is_some());
    }

    #[test]
    fn exit_reentrancy_disarming_lifts_the_cap() {
        let _serial = serial();
        let pass = begin(0).unwrap();
        disarm();
        assert_eq!(check_cap(u64::MAX), CapCheck::Disarmed);
        drop(pass);
    }

    #[test]
    fn l3_not_aborted_on_hard_cap() {
        let _serial = serial();
        let pass = begin(0).unwrap();
        let aborts = Arc::new(AtomicUsize::new(0));
        let losses = settle_leftovers(vec![
            leftover(Level::L3, false, &aborts),
            leftover(Level::L3, true, &aborts),
            leftover(Level::L2, false, &aborts),
            leftover(Level::L1, false, &aborts),
        ]);
        assert_eq!(
            aborts.load(Ordering::SeqCst),
            2,
            "only L1 and L2 were aborted"
        );
        assert_eq!(losses[0], Loss::L3Interrupted { label: "L3".into() });
        assert_eq!(losses[1], Loss::L3Publishing { label: "L3".into() });
        assert_eq!(losses[3], Loss::L1Dropped { label: "L1".into() });
        drop(pass);
    }

    #[test]
    fn after_the_pool_closes_leftovers_are_only_recorded() {
        let _serial = serial();
        let pass = begin(0).unwrap();
        mark_pool_closed();
        let aborts = Arc::new(AtomicUsize::new(0));
        let losses =
            settle_leftovers(vec![leftover(Level::L4, false, &aborts)]);
        assert_eq!(aborts.load(Ordering::SeqCst), 0);
        assert_eq!(
            losses,
            [Loss::Abandoned {
                level: Level::L4,
                label: "L4".into()
            }]
        );
        drop(pass);
    }

    struct FakeApp(RefCell<Vec<&'static str>>);

    impl Restarter for FakeApp {
        fn request_restart(&self) {
            self.0.borrow_mut().push("request_restart");
        }
    }

    #[test]
    fn restart_uses_request_restart() {
        let _serial = serial();
        let app = FakeApp(RefCell::default());
        finish_exit(
            || app.0.borrow_mut().push("finish_database_session"),
            true,
            &app,
        );
        assert_eq!(
            *app.0.borrow(),
            ["finish_database_session", "request_restart"],
            "the database session is finished before the restart is requested"
        );
        assert!(pool_closed());

        let app = FakeApp(RefCell::default());
        finish_exit(
            || app.0.borrow_mut().push("finish_database_session"),
            false,
            &app,
        );
        assert_eq!(*app.0.borrow(), ["finish_database_session"]);
    }
}
