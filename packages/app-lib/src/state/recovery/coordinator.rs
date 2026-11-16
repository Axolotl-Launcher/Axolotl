//! Record interpretation has a separate budget from file recovery execution.
use crate::state::State;
use dashmap::DashSet;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
const RECORD_BUDGET: Duration = Duration::from_secs(15);
static INITIAL_STARTED_AT: OnceLock<tokio::time::Instant> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);
static IMPORT_DONE: AtomicBool = AtomicBool::new(false);
static IMPORT_FAILED: AtomicBool = AtomicBool::new(false);
static IMPORT_READY: tokio::sync::Notify = tokio::sync::Notify::const_new();
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
    ensure_started(Arc::clone(&state));
    let deadline = interpretation_deadline(
        IMPORT_DONE.load(Ordering::Acquire),
        INITIAL_STARTED_AT.get().copied(),
        tokio::time::Instant::now(),
    );
    let id = instance_id.to_owned();
    let (send, receive) = tokio::sync::oneshot::channel();
    let worker = Arc::clone(&state);
    state.spawn_background(async move {
        phases(async {
            loop {let notified=IMPORT_READY.notified();if IMPORT_DONE.load(Ordering::Acquire){break}notified.await;}
            let result=if IMPORT_FAILED.load(Ordering::Acquire){Err(crate::ErrorKind::InputError("Legacy recovery interpretation remains unavailable".into()).into())}else{crate::state::store::recovery_documents::interpret_instance(&worker.pool,&id).await};
            if let Err(error)=&result {tracing::warn!(instance_id=%id,%error,"Recovery record unavailable; launch allowed and original retained");release(&id);}
            result
        },send,async {}).await;
    });
    match tokio::time::timeout_at(deadline, receive).await {
        Ok(Ok(Ok(()))) => {
            FAIL_OK.get_or_init(DashSet::new).remove(instance_id);
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
    ensure_started(state);
    let deadline = interpretation_deadline(
        IMPORT_DONE.load(Ordering::Acquire),
        INITIAL_STARTED_AT.get().copied(),
        tokio::time::Instant::now(),
    );
    if tokio::time::timeout_at(deadline, wait_ready())
        .await
        .is_err()
    {
        tracing::warn!(
            "Recovery record interpretation continues in background; startup allowed"
        );
    }
}

async fn wait_ready() {
    loop {
        let notified = IMPORT_READY.notified();
        if IMPORT_DONE.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }
}
fn ensure_started(state: Arc<State>) {
    if STARTED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let _ = INITIAL_STARTED_AT.set(tokio::time::Instant::now());
    let (send, _receive) = tokio::sync::oneshot::channel();
    let worker = Arc::clone(&state);
    state.spawn_background(async move {
        struct Completion;
        impl Drop for Completion {fn drop(&mut self){if !IMPORT_DONE.load(Ordering::Acquire){IMPORT_FAILED.store(true,Ordering::Release);IMPORT_DONE.store(true,Ordering::Release);IMPORT_READY.notify_waiters();}}}
        let _completion=Completion;
        phases(async {
            if let Err(error)=crate::state::store::recovery_documents::import_legacy(&worker.pool,&worker.directories.settings_dir).await {IMPORT_FAILED.store(true,Ordering::Release);tracing::warn!(%error,"Legacy recovery records retained for later retry");}
            if let Err(error)=crate::state::store::recovery_documents::interpret_all(&worker.pool).await{tracing::warn!(%error,"Some recovery records remain opaque; other records continue");}
            IMPORT_DONE.store(true,Ordering::Release);IMPORT_READY.notify_waiters();
        },send,async {
            // Hashing, downloads and rollback start after interpretation is
            // reported. They are never subject to the record budget.
            if let Err(error)=crate::state::instances::commands::recover_content_file_operations(&worker).await {tracing::warn!(%error,"Some file recoveries remain pending");}
            if let Err(error)=crate::install::recovery::recover_interrupted_jobs(&worker).await {tracing::warn!(%error,"Some install recoveries remain pending");}
        }).await;
    });
}

async fn phases<T, F, G>(
    interpret: F,
    report: tokio::sync::oneshot::Sender<T>,
    execute: G,
) where
    F: std::future::Future<Output = T>,
    G: std::future::Future<Output = ()>,
{
    let result = interpret.await;
    let _ = report.send(result);
    execute.await;
}

#[cfg(test)]
async fn record_phase<F: std::future::Future>(
    records: F,
) -> Result<F::Output, tokio::time::error::Elapsed> {
    tokio::time::timeout(RECORD_BUDGET, records).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn later_launch_gets_a_new_record_attempt_after_initial_completion() {
        let initial = tokio::time::Instant::now();
        tokio::time::sleep(Duration::from_secs(60)).await;
        let now = tokio::time::Instant::now();
        assert_eq!(
            interpretation_deadline(true, Some(initial), now) - now,
            RECORD_BUDGET
        );
        assert!(interpretation_deadline(false, Some(initial), now) < now);
    }
    #[tokio::test(start_paused = true)]
    async fn interpretation_budget_does_not_time_out_later_execution() {
        let start = tokio::time::Instant::now();
        let (send, receive) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(phases(
            async { tokio::time::sleep(Duration::from_secs(5)).await },
            send,
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
            send,
            async { tokio::time::sleep(Duration::from_secs(20)).await },
        ));
        assert!(record_phase(receive).await.is_err());
        worker.await.unwrap();
    }
}

fn interpretation_deadline(
    initial_complete: bool,
    initial: Option<tokio::time::Instant>,
    now: tokio::time::Instant,
) -> tokio::time::Instant {
    if initial_complete {
        now + RECORD_BUDGET
    } else {
        initial.unwrap_or(now) + RECORD_BUDGET
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
        if let Err(error)=crate::state::instances::commands::recover_legacy_rollbacks(&worker).await{tracing::warn!(%error,"Legacy recovery remains pending after game exit");}
        if let Err(error)=crate::state::instances::commands::recover_content_file_operations_for_instance(&worker,&id).await{tracing::warn!(instance_id=%id,%error,"File recovery remains pending after game exit");}
        if let Err(error)=crate::install::recovery::recover_interrupted_jobs_for_instance(&worker,&id).await{tracing::warn!(instance_id=%id,%error,"Install recovery remains pending after game exit");}
    });
}
