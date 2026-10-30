//! Critical sections keep an install future alive through file publication
//! and rollback after its job is canceled.
//!
//! The only entry is [`with_critical_section`], which takes all three
//! branches as arguments: there is no gate value a caller could match and
//! silently skip, and leaving a branch out does not compile. Writing APIs ask
//! for a [`WriteAccess`], which only the protected branch (a guard), the
//! unprotected branch (an [`Unscoped`] token) or a named [`WhitelistSite`]
//! can supply.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct CriticalSections {
    active: AtomicUsize,
    closing: AtomicBool,
    finished: Notify,
}

tokio::task_local! {
    static SECTIONS: Arc<CriticalSections>;
}

/// Proof that the current install future is inside an entered critical
/// section. Its field is private, so only [`with_critical_section`] makes one.
pub(crate) struct CriticalSectionGuard {
    sections: Arc<CriticalSections>,
}

impl Drop for CriticalSectionGuard {
    fn drop(&mut self) {
        self.sections.active.fetch_sub(1, Ordering::AcqRel);
        self.sections.finished.notify_one();
    }
}

/// Handed to the unprotected branch: no cancelable install scope exists, so
/// nothing would cut the work short.
#[derive(Clone, Copy)]
pub(crate) struct Unscoped {
    _priv: (),
}

/// Handed to the closing branch: the job was canceled before this section
/// started, so it must not begin writing.
pub(crate) struct ClosingToken {
    _priv: (),
}

/// What a writing API needs: a guard or an unscoped token.
#[derive(Clone, Copy)]
pub(crate) enum WriteAccess<'a> {
    // Never read: borrowing the guard is the proof, and it ties the access to
    // the section's lifetime.
    Guarded(#[allow(dead_code)] &'a CriticalSectionGuard),
    Unscoped(Unscoped),
}

impl<'a> From<&'a CriticalSectionGuard> for WriteAccess<'a> {
    fn from(guard: &'a CriticalSectionGuard) -> Self {
        Self::Guarded(guard)
    }
}

impl From<Unscoped> for WriteAccess<'_> {
    fn from(token: Unscoped) -> Self {
        Self::Unscoped(token)
    }
}

/// Call sites that write outside any critical section. Each one names who
/// protects the write instead; the list is capped at ten entries.
#[derive(Clone, Copy, Debug)]
pub(crate) enum WhitelistSite {
    /// CurseForge manual import of a world archive; holds the instance
    /// content lock for the whole import.
    CurseForgeManualWorldImport,
    /// `api::instance::import_world_save`, the UI's world import; holds the
    /// instance content lock for the whole import.
    InstanceWorldImport,
    #[cfg(test)]
    Tests,
}

/// Write access for a whitelisted call site outside any critical section.
pub(crate) fn whitelisted(_site: WhitelistSite) -> Unscoped {
    Unscoped { _priv: () }
}

/// The error a closing branch returns: the job was canceled before the
/// section could start, so nothing was written.
pub(crate) fn closing_error() -> crate::Error {
    crate::ErrorKind::InputError(
        "The install task was canceled before it started writing".into(),
    )
    .into()
}

/// Runs exactly one branch. Inside a cancelable install scope that is still
/// running, `protected` gets a guard that keeps the scope alive until it
/// returns; once the scope is closing, `closing` runs instead. Outside any
/// scope, `unprotected` runs.
pub(crate) async fn with_critical_section<R>(
    protected: impl AsyncFnOnce(&CriticalSectionGuard) -> R,
    unprotected: impl AsyncFnOnce(Unscoped) -> R,
    closing: impl AsyncFnOnce(ClosingToken) -> R,
) -> R {
    let Ok(sections) = SECTIONS.try_with(Arc::clone) else {
        return unprotected(Unscoped { _priv: () }).await;
    };
    // Count the section first, then look for closing; cancellation stores
    // closing first, then reads the count. With both sequentially consistent,
    // either the canceller sees this section or this section sees closing.
    sections.active.fetch_add(1, Ordering::SeqCst);
    let guard = CriticalSectionGuard { sections };
    if guard.sections.closing.load(Ordering::SeqCst) {
        drop(guard);
        return closing(ClosingToken { _priv: () }).await;
    }
    protected(&guard).await
}

/// One piece of work shared by the protected and unprotected branches, so
/// both run the same code and differ only in the access they pass.
pub(crate) struct SharedWork<F>(std::sync::Mutex<Option<F>>);

impl<F> SharedWork<F> {
    pub(crate) fn new(work: F) -> Self {
        Self(std::sync::Mutex::new(Some(work)))
    }

    /// Runs the work with `access`. Only one branch of a critical section
    /// runs, so the work is taken exactly once.
    pub(crate) async fn run<'a, R>(&self, access: WriteAccess<'a>) -> R
    where
        F: AsyncFnOnce(WriteAccess<'a>) -> R,
    {
        let work = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .expect("a critical section runs one branch");
        work(access).await
    }
}

/// Cancellation drops ordinary work, but continues polling entered critical
/// sections; sections that have not started yet take their closing branch.
pub(crate) async fn run_cancelable<F: Future>(
    cancellation: &CancellationToken,
    work: F,
) -> Option<F::Output> {
    let sections = Arc::new(CriticalSections::default());
    SECTIONS.scope(sections.clone(), async {
        tokio::pin!(work);
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                sections.closing.store(true, Ordering::SeqCst);
                loop {
                    let finished = sections.finished.notified();
                    if sections.active.load(Ordering::SeqCst) == 0 { return None; }
                    tokio::select! {
                        result = &mut work => return Some(result),
                        _ = finished => {},
                    }
                }
            }
            result = &mut work => Some(result),
        }
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cancellation_waits_for_file_rollback_before_releasing_the_work() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("instance.jar");
        tokio::fs::write(&file, b"old").await.unwrap();
        let entered = Arc::new(Notify::new());
        let resume = Arc::new(Notify::new());
        let cancellation = CancellationToken::new();
        let entered_work = entered.clone();
        let resume_work = resume.clone();
        let token = cancellation.clone();
        let target = file.clone();
        let mut task = tokio::spawn(async move {
            run_cancelable(&token, async move {
                with_critical_section(
                    async |_guard| {
                        tokio::fs::write(&target, b"new").await.unwrap();
                        entered_work.notify_one();
                        resume_work.notified().await;
                        tokio::fs::write(&target, b"old").await.unwrap();
                    },
                    async |_unscoped| unreachable!("runs inside a scope"),
                    async |_closing| unreachable!("entered before cancel"),
                )
                .await;
                std::future::pending::<()>().await;
            })
            .await
        });
        entered.notified().await;
        cancellation.cancel();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut task)
                .await
                .is_err()
        );
        resume.notify_one();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
        assert_eq!(tokio::fs::read(file).await.unwrap(), b"old");
    }

    #[tokio::test]
    async fn ordinary_pending_work_cancels_immediately() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(
            run_cancelable(&token, std::future::pending::<()>())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn outside_a_scope_the_unprotected_branch_runs() {
        let branch = with_critical_section(
            async |_guard| "protected",
            async |_unscoped| "unprotected",
            async |_closing| "closing",
        )
        .await;
        assert_eq!(branch, "unprotected");
    }

    #[tokio::test]
    async fn shared_work_runs_the_same_code_in_either_branch() {
        let work = SharedWork::new(async |access: WriteAccess<'_>| {
            matches!(access, WriteAccess::Unscoped(_))
        });
        let unscoped = with_critical_section(
            async |guard| work.run(guard.into()).await,
            async |token| work.run(token.into()).await,
            async |_closing| false,
        )
        .await;
        assert!(unscoped);
    }
}

#[cfg(test)]
mod closing_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[tokio::test]
    async fn cancel_during_critical_section_skips_publish() {
        let published = Arc::new(AtomicBool::new(false));
        let reached = Arc::new(Notify::new());
        let proceed = Arc::new(Notify::new());
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        let (published_work, reached_work, proceed_work) =
            (published.clone(), reached.clone(), proceed.clone());
        let task = tokio::spawn(async move {
            run_cancelable(&token, async move {
                let first = with_critical_section(
                    async |_guard| {
                        reached_work.notify_one();
                        proceed_work.notified().await;
                        "first section finished"
                    },
                    async |_unscoped| unreachable!(),
                    async |_closing| unreachable!(),
                )
                .await;
                // Canceled while the first section ran: the next one must
                // take its closing branch instead of publishing.
                let second = with_critical_section(
                    async |_guard| {
                        published_work.store(true, Ordering::SeqCst);
                        "published"
                    },
                    async |_unscoped| unreachable!(),
                    async |_closing| "closing",
                )
                .await;
                (first, second)
            })
            .await
        });
        reached.notified().await;
        cancellation.cancel();
        tokio::task::yield_now().await;
        proceed.notify_one();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, Some(("first section finished", "closing")));
        assert!(!published.load(Ordering::SeqCst), "publish never ran");
    }
}
