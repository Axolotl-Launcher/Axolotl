//! The exit barrier (spec §8.6). Every `ExitRequested` starts a pass, so an
//! exit interrupted earlier never leaves a stale one behind. Steps 1–5 run in
//! `ExitRequested`, steps 6–7 in `Exit`:
//!
//! 1. arm `EXIT_DEADLINE_MS` with the cap this exit deserves and reset
//!    `POOL_CLOSED`;
//! 2. flush the pending skin change;
//! 3. stop multiplayer and MCP services within the soft window (5 s);
//! 4. stop dedicated servers (their own 5 s budget);
//! 5. cancel and abort what is left below and above L3 — never L3 itself —
//!    and record the losses;
//! 6. mark the pool closed, then close the database pool (3 s); from here
//!    nothing is aborted any more, only recorded;
//! 7. exit, or restart once the pool is closed: an installed update asks
//!    through `request_restart` here, and the frontend's `restart_app` has
//!    asked before step 1, so Tauri relaunches only after `Exit` returns.
//!
//! Budget: step 2 gets 1 s, and steps 3, 4 and 6 total 5 + 5 + 3 = 13 s,
//! leaving 1 s of the 15 s hard cap for the exit handler and database-close
//! overhead. A user-requested restart or a pending update gets [`RESTART_CAP`]
//! for steps 1–5; a restart then gets the ordinary cap again for steps 6–7,
//! while an update disarms it for the install and [`rearm`] bounds the steps
//! after it returns. At the cap the barrier records the losses and exits the
//! process; L3 work is never killed by the barrier, it ends with the process
//! and is settled as interrupted on the next start.
//!
//! Exit paths: the frontend's `exit_app`, the tray's quit, the frontend's
//! `restart_app` and the updater's restart go through the barrier. macOS
//! Cmd+Q/Dock Quit reaches `Exit` directly; the exit handler runs the missing
//! steps once there. The Windows updater closes the database through the
//! plugin's `on_before_exit` hook. The NSIS installer's own `exit(0)` and an OS
//! shutdown or forced kill cannot be intercepted.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use theseus::scheduler::tag::Level;

/// Steps 3, 4 and 6.
pub const STEP_BUDGETS: [Duration; 3] = [
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(3),
];
pub const SOFT_WINDOW: Duration = STEP_BUDGETS[0];
/// Step 2: a pending skin change may call the Mojang API. Bound it to 1 s so
/// the later steps and database cleanup keep a margin under the hard cap.
pub const SKIN_FLUSH_TIMEOUT: Duration = Duration::from_secs(1);
pub const HARD_CAP: Duration = Duration::from_secs(15);
/// A user-requested restart or a pending update gets one extended pass for
/// steps 1–5. The update then disarms the cap for the install, which waits on
/// the user by design, and [`rearm`] bounds the steps after it returns.
pub const RESTART_CAP: Duration = Duration::from_secs(30);

/// What the current exit request is for; it selects the cap for steps 1–5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitIntent {
    Exit,
    Restart,
    Update,
}

pub fn cap_for(intent: ExitIntent) -> Option<Duration> {
    match intent {
        ExitIntent::Exit => Some(HARD_CAP),
        ExitIntent::Restart | ExitIntent::Update => Some(RESTART_CAP),
    }
}

/// Milliseconds on the exit clock (`now_millis`); 0 while unarmed. Re-armed
/// on every `ExitRequested`, so it must never be a write-once cell.
static EXIT_DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
/// Where the exit clock starts. The hard cap is an elapsed-time limit, so it
/// is measured on a monotonic clock: a wall-clock step during shutdown must
/// neither end the barrier early nor keep it waiting past the cap.
static CLOCK_ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
static POOL_CLOSED: AtomicBool = AtomicBool::new(false);
/// The re-entrancy guard: set while one barrier runs.
static EXIT_RUNNING: AtomicBool = AtomicBool::new(false);
static WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);
/// Whether any barrier pass has run for this process. A macOS Quit reaches
/// `RunEvent::Exit` without `ExitRequested`; the exit handler uses this to run
/// the missing steps once.
static PASS_RAN: AtomicBool = AtomicBool::new(false);

/// Milliseconds on the monotonic exit clock, counted from its first read. An
/// armed deadline is at least the hard cap, so it never reads as unarmed.
pub fn now_millis() -> u64 {
    CLOCK_ORIGIN.elapsed().as_millis() as u64
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

/// Step 1 with the ordinary hard cap. `None` while another barrier runs, which
/// a nested `ExitRequested` must not start again.
#[cfg_attr(not(test), allow(dead_code))]
pub fn begin(now_ms: u64) -> Option<ExitPass> {
    begin_with(now_ms, Some(HARD_CAP))
}

/// Step 1 with an explicit cap. `None` disarms the watchdog for this pass: the
/// updater's elevation prompt is a wait by design and has no upper bound.
pub fn begin_with(now_ms: u64, cap: Option<Duration>) -> Option<ExitPass> {
    if EXIT_RUNNING.swap(true, Ordering::AcqRel) {
        return None;
    }
    match cap {
        Some(cap) => EXIT_DEADLINE_MS
            .store(now_ms + cap.as_millis() as u64, Ordering::Release),
        None => disarm(),
    }
    POOL_CLOSED.store(false, Ordering::Release);
    PASS_RAN.store(true, Ordering::Release);
    Some(ExitPass(()))
}

/// Whether a barrier pass has run in this process.
pub fn pass_ran() -> bool {
    PASS_RAN.load(Ordering::Acquire)
}

pub fn deadline_ms() -> u64 {
    EXIT_DEADLINE_MS.load(Ordering::Acquire)
}

/// Re-arms the ordinary hard cap after a bounded step, so steps 6–7 still get
/// a full budget even when the pre-install pass used an extended one.
pub fn rearm(now_ms: u64) {
    EXIT_DEADLINE_MS
        .store(now_ms + HARD_CAP.as_millis() as u64, Ordering::Release);
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
                        "Exit did not finish within its hard cap; exiting without waiting further"
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

#[cfg(any(feature = "updater", test))]
#[derive(Debug, PartialEq, Eq)]
pub enum PendingUpdateStatus {
    Installed,
    PortableReady,
    Failed(String),
}

#[cfg(any(feature = "updater", test))]
#[derive(Debug, PartialEq, Eq)]
pub struct PendingUpdateOutcome {
    pub version: String,
    pub status: PendingUpdateStatus,
    pub restart_requested: bool,
}

#[cfg(any(feature = "updater", test))]
impl PendingUpdateOutcome {
    pub fn restart_after_exit(&self) -> bool {
        self.restart_requested && self.status == PendingUpdateStatus::Installed
    }
}

/// Records the install result before database finalization. A portable worker
/// owns its relaunch; failed installs never request a restart here.
#[cfg(any(feature = "updater", test))]
pub fn complete_pending_update(
    version: String,
    portable_worker: bool,
    restart_requested: bool,
    result: Result<(), String>,
    set_toast: impl FnOnce(Option<String>),
    report: impl FnOnce(&PendingUpdateOutcome),
) -> PendingUpdateOutcome {
    let status = match result {
        Ok(()) if portable_worker => PendingUpdateStatus::PortableReady,
        Ok(()) => PendingUpdateStatus::Installed,
        Err(error) => PendingUpdateStatus::Failed(error),
    };
    let outcome = PendingUpdateOutcome {
        version,
        status,
        restart_requested,
    };
    let toast = match &outcome.status {
        PendingUpdateStatus::Failed(_) => None,
        _ => Some(outcome.version.clone()),
    };
    set_toast(toast);
    report(&outcome);
    outcome
}

/// Steps 6–7: close the database session first, then restart if an update
/// asked for it, so a restart never skips the clean-shutdown record.
pub fn finish_exit(
    finish_database: impl FnOnce(),
    restart: bool,
    restarter: &impl Restarter,
) {
    // Mark before closing: a hard cap that fires while the pool is closing
    // must not run the abort arm of the action table.
    mark_pool_closed();
    finish_database();
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
        assert_eq!(
            total + SKIN_FLUSH_TIMEOUT,
            Duration::from_secs(14),
            "the bounded steps leave one second for exit-handler and database-close overhead"
        );
        assert!(total + SKIN_FLUSH_TIMEOUT < HARD_CAP);
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
    fn the_hard_cap_runs_on_the_monotonic_exit_clock() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let _serial = serial();
        let first = now_millis();
        std::thread::sleep(Duration::from_millis(5));
        assert!(now_millis() >= first + 5, "the exit clock moves forward");
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(
            now_millis() < epoch_ms,
            "the exit clock must be monotonic, not wall-clock milliseconds"
        );

        let pass = begin(now_millis()).unwrap();
        assert!(deadline_ms() >= HARD_CAP.as_millis() as u64);
        assert!(
            matches!(check_cap(now_millis()), CapCheck::Wait(_)),
            "a deadline armed now is not reached on the same clock"
        );
        drop(pass);
    }

    #[test]
    fn the_exit_intent_selects_the_cap() {
        assert_eq!(cap_for(ExitIntent::Exit), Some(HARD_CAP));
        assert_eq!(cap_for(ExitIntent::Restart), Some(RESTART_CAP));
        assert_eq!(
            cap_for(ExitIntent::Update),
            Some(RESTART_CAP),
            "the pre-install pass is bounded; the install itself is disarmed"
        );
        assert!(RESTART_CAP > HARD_CAP);
    }

    #[test]
    fn a_disarmed_pass_keeps_the_watchdog_off_until_rearmed() {
        let _serial = serial();
        let pass = begin_with(1_000, None).unwrap();
        assert!(pass_ran());
        assert_eq!(check_cap(u64::MAX), CapCheck::Disarmed);
        rearm(2_000);
        assert_eq!(
            deadline_ms(),
            2_000 + HARD_CAP.as_millis() as u64,
            "rearm restores the ordinary cap"
        );
        assert_eq!(
            check_cap(2_000 + HARD_CAP.as_millis() as u64),
            CapCheck::Reached
        );
        drop(pass);
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

    #[test]
    fn pending_update_outcomes_finalize_the_database_before_any_restart() {
        let _serial = serial();
        for (
            result,
            portable,
            restart_requested,
            expected_status,
            expected_restart,
        ) in [
            (Ok(()), false, true, PendingUpdateStatus::Installed, true),
            (Ok(()), false, false, PendingUpdateStatus::Installed, false),
            (
                Ok(()),
                true,
                true,
                PendingUpdateStatus::PortableReady,
                false,
            ),
            (
                Err("authorization cancelled".to_owned()),
                false,
                true,
                PendingUpdateStatus::Failed("authorization cancelled".into()),
                false,
            ),
            (
                Err("rpm rejected the package".to_owned()),
                false,
                false,
                PendingUpdateStatus::Failed("rpm rejected the package".into()),
                false,
            ),
        ] {
            let app = FakeApp(RefCell::default());
            let toast = RefCell::new(None);
            let reported = RefCell::new(None);
            let outcome = complete_pending_update(
                "1.2.3".into(),
                portable,
                restart_requested,
                result,
                |value| {
                    *toast.borrow_mut() = Some(value);
                    app.0.borrow_mut().push("persist_outcome");
                },
                |outcome| {
                    *reported.borrow_mut() =
                        Some(format!("{:?}", outcome.status));
                    app.0.borrow_mut().push("report_outcome");
                },
            );
            assert_eq!(outcome.version, "1.2.3");
            assert_eq!(outcome.status, expected_status);
            assert_eq!(outcome.restart_after_exit(), expected_restart);
            assert_eq!(
                *reported.borrow(),
                Some(format!("{expected_status:?}"))
            );
            assert_eq!(
                *toast.borrow(),
                Some(
                    if matches!(expected_status, PendingUpdateStatus::Failed(_))
                    {
                        None
                    } else {
                        Some("1.2.3".into())
                    }
                ),
            );
            finish_exit(
                || app.0.borrow_mut().push("finish_database_session"),
                outcome.restart_after_exit(),
                &app,
            );
            let mut expected = vec![
                "persist_outcome",
                "report_outcome",
                "finish_database_session",
            ];
            if expected_restart {
                expected.push("request_restart");
            }
            assert_eq!(*app.0.borrow(), expected);
        }
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
            || {
                assert!(
                    pool_closed(),
                    "the pool is marked before the database starts closing"
                );
                app.0.borrow_mut().push("finish_database_session");
            },
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
