use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct CriticalSections {
    active: AtomicUsize,
    finished: Notify,
}

tokio::task_local! {
    static SECTIONS: Arc<CriticalSections>;
}

pub(crate) struct CriticalSectionGuard(Arc<CriticalSections>);

impl Drop for CriticalSectionGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
        self.0.finished.notify_one();
    }
}

/// Keeps the current install future alive through file publication and rollback.
pub(crate) fn enter() -> Option<CriticalSectionGuard> {
    SECTIONS
        .try_with(|sections| {
            sections.active.fetch_add(1, Ordering::AcqRel);
            CriticalSectionGuard(sections.clone())
        })
        .ok()
}

/// Cancellation drops ordinary work, but continues polling entered critical sections.
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
                loop {
                    let finished = sections.finished.notified();
                    if sections.active.load(Ordering::Acquire) == 0 { return None; }
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
                {
                    let _critical = enter();
                    tokio::fs::write(&target, b"new").await.unwrap();
                    entered_work.notify_one();
                    resume_work.notified().await;
                    tokio::fs::write(&target, b"old").await.unwrap();
                }
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
}
