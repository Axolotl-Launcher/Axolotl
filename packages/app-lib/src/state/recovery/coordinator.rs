//! Record interpretation has a separate budget from file recovery execution.
use crate::install::recovery::recover_published_jobs;
use crate::state::State;
use crate::state::instances::commands::{
    recover_content_file_operations,
    recover_content_file_operations_for_instance, recover_legacy_rollbacks,
};
use crate::state::store::recovery_documents;
use dashmap::DashSet;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
const RECORD_BUDGET: Duration = Duration::from_secs(15);
static STARTUP_INSPECTION: OnceLock<tokio::sync::watch::Receiver<bool>> =
    OnceLock::new();
static IMPORT_DONE: AtomicBool = AtomicBool::new(false);
static FAIL_OK: OnceLock<DashSet<String>> = OnceLock::new();
pub(crate) fn fail_ok(instance_id: &str) -> bool {
    FAIL_OK.get_or_init(DashSet::new).contains(instance_id)
}
fn release(instance_id: &str) {
    FAIL_OK.get_or_init(DashSet::new).insert(instance_id.into());
}

/// A timed-out interpretation keeps running; only its record decoding is timed.
/// No instance lock is held here, so the game may acquire its maintenance lock.
pub(crate) async fn before_launch(state: Arc<State>, instance_id: &str) {
    if crate::state::store::recovery_mode::root(&state.pool).is_some() {
        return;
    }
    let deadline = tokio::time::Instant::now() + RECORD_BUDGET;
    let id = instance_id.to_owned();
    let (send, receive) = tokio::sync::oneshot::channel();
    let worker = Arc::clone(&state);
    state.spawn_background(async move {
        let result =
            recovery_documents::interpret_instance(&worker.pool, &id).await;
        let _ = send.send(result);
    });
    match tokio::time::timeout_at(deadline, receive).await {
        Ok(Ok(Ok(()))) => {
            FAIL_OK.get_or_init(DashSet::new).remove(instance_id);
        }
        Ok(Ok(Err(error))) => {
            release(instance_id);
            tracing::warn!(instance_id, %error, "Recovery record unavailable; launch allowed and original retained");
        }
        _ => {
            release(instance_id);
            tracing::warn!(
                instance_id,
                "Record interpretation budget exhausted or unavailable; launch allowed"
            );
        }
    }
}

pub(crate) async fn start(state: Arc<State>) {
    let mut inspection = STARTUP_INSPECTION
        .get_or_init(|| {
            let (complete, inspection) = tokio::sync::watch::channel(false);
            let worker = Arc::clone(&state);
            state.spawn_background(async move {
                phases(
                    async {
                        if let Err(error) = recovery_documents::rebind_copied_history(
                            &worker.pool, &worker.directories.settings_dir, &worker.directories.config_dir,
                        ).await {
                            tracing::warn!(%error, "Could not rebind history copied with the launcher data directory");
                        }
                        if let Err(error) = recovery_documents::interpret_all(&worker.pool).await {
                            tracing::warn!(%error, "Some recovery records remain opaque; other records continue");
                        }
                    },
                    move |()| { complete.send_replace(true); },
                    async { execute_pending(&worker).await },
                ).await;
            });
            inspection
        })
        .clone();
    match record_phase(inspection.wait_for(|done| *done)).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::warn!(%error, "Startup recovery record inspection stopped")
        }
        Err(_) => {
            tracing::warn!(
                "Recovery record inspection exceeded the startup budget; continuing in background"
            );
        }
    }
}

async fn execute_pending(state: &State) {
    IMPORT_DONE.store(true, Ordering::Release);
    // File recovery and rollback have their own execution semantics. The
    // 15-second budget applies only to reading the records that select them.
    if let Err(error) = recover_content_file_operations(state).await {
        tracing::warn!(%error, "Some file recoveries remain pending");
    }
    if let Err(error) = recover_legacy_rollbacks(state).await {
        tracing::warn!(%error, "Some legacy recovery records remain pending");
    }
    if let Err(error) =
        recover_published_jobs(state, &state.startup_install_jobs, None).await
    {
        tracing::warn!(%error, "Some install recoveries remain pending");
    }
}

async fn phases<T, F, G>(interpret: F, report: impl FnOnce(T), execute: G)
where
    F: std::future::Future<Output = T>,
    G: std::future::Future<Output = ()>,
{
    let result = interpret.await;
    report(result);
    execute.await;
}

async fn record_phase<F: std::future::Future>(
    records: F,
) -> Result<F::Output, tokio::time::error::Elapsed> {
    tokio::time::timeout(RECORD_BUDGET, records).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn interpretation_budget_does_not_time_out_later_execution() {
        let start = tokio::time::Instant::now();
        let (send, receive) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(phases(
            async { tokio::time::sleep(Duration::from_secs(5)).await },
            |result| {
                let _ = send.send(result);
            },
            async { tokio::time::sleep(Duration::from_secs(20)).await },
        ));
        record_phase(receive).await.unwrap().unwrap();
        assert_eq!(start.elapsed(), Duration::from_secs(5));
        worker.await.unwrap();
        assert_eq!(start.elapsed(), Duration::from_secs(25));
    }
    #[tokio::test(start_paused = true)]
    async fn expired_interpretation_does_not_cancel_the_background_record() {
        let (send, receive) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(phases(
            async { tokio::time::sleep(Duration::from_secs(30)).await },
            |result| {
                let _ = send.send(result);
            },
            async { tokio::time::sleep(Duration::from_secs(20)).await },
        ));
        assert!(record_phase(receive).await.is_err());
        worker.await.unwrap();
    }
}

/// One event-driven retry after the game leaves the process registry. The
/// executors acquire maintenance and content locks and re-read observations.
pub(crate) fn after_exit(state: Arc<State>, instance_id: &str) {
    if crate::state::store::recovery_mode::root(&state.pool).is_some()
        || !IMPORT_DONE.load(Ordering::Acquire)
    {
        return;
    }
    let id = instance_id.to_owned();
    let worker = Arc::clone(&state);
    state.spawn_background(async move {
        if let Err(error) = recover_legacy_rollbacks(&worker).await {
            tracing::warn!(%error, "Legacy recovery remains pending after game exit");
        }
        if let Err(error) = recover_content_file_operations_for_instance(&worker, &id).await {
            tracing::warn!(instance_id=%id, %error, "File recovery remains pending after game exit");
        }
        if let Err(error) = recover_published_jobs(&worker, &worker.startup_install_jobs, Some(&id)).await {
            tracing::warn!(instance_id=%id, %error, "Install recovery remains pending after game exit");
        }
    });
}
