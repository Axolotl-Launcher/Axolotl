//! Verification and database worker plumbing.

use super::record::{
    finalize_curseforge_materializations, restore_curseforge_materialization,
    restore_curseforge_materializations,
    verify_and_record_curseforge_modpack_file,
};
use crate::api::curseforge::dto::CurseForgeFile;
use crate::api::curseforge::selection::format_bytes;
use crate::event::emit::emit_loading;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{CurseForgeFileId, CurseForgeProjectId, ProjectType};
use crate::util::download::constants::{
    MODPACK_DATABASE_BATCH_SIZE, MODPACK_DATABASE_FLUSH_INTERVAL,
    MODPACK_VERIFICATION_CONCURRENCY,
};
use crate::{ErrorKind, State};
use futures::stream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) struct CurseForgeVerificationTask {
    pub(crate) instance_id: String,
    pub(crate) relative_path: String,
    pub(crate) download_path: PathBuf,
    pub(crate) full_path: PathBuf,
    pub(crate) file: CurseForgeFile,
    pub(crate) project_type: ProjectType,
    pub(crate) ownership_kind: crate::state::instances::ContentOwnershipKind,
    pub(crate) expected_bytes: u64,
    pub(crate) download_result: Option<crate::util::fetch::DownloadResult>,
    pub(crate) cancellation: CancellationToken,
}

impl Drop for CurseForgeVerificationTask {
    fn drop(&mut self) {
        // A worker may stop after another file fails, leaving tasks buffered
        // in the channel. Those staged files are safe to remove and must not
        // accumulate in an existing instance after cancellation.
        let _ = std::fs::remove_file(&self.download_path);
    }
}

#[derive(Clone)]
pub(crate) struct CurseForgeVerificationContext {
    pub(crate) reporter: Option<InstallProgressReporter>,
    pub(crate) loading_bar: Option<Arc<crate::event::LoadingBarId>>,
    pub(crate) details: InstallPhaseDetails,
    pub(crate) files_done: Arc<AtomicU64>,
    pub(crate) bytes_done: Arc<AtomicU64>,
    pub(crate) active_downloads: Arc<AtomicU64>,
    pub(crate) total_files: u64,
    pub(crate) total_bytes: u64,
    pub(crate) cancellation: CancellationToken,
}

pub(crate) struct CurseForgeDatabaseTask {
    pub(crate) instance_id: String,
    pub(crate) record: crate::state::instances::commands::ProjectFileRecord,
    pub(crate) verified_pending:
        Option<(CurseForgeProjectId, CurseForgeFileId)>,
    pub(crate) expected_bytes: u64,
    pub(crate) full_path: PathBuf,
    pub(crate) previous_path: Option<PathBuf>,
}

pub(crate) fn spawn_curseforge_verification_worker(
    mut receiver: mpsc::Receiver<CurseForgeVerificationTask>,
    context: CurseForgeVerificationContext,
    database_tx: mpsc::Sender<CurseForgeDatabaseTask>,
) -> tokio::task::JoinHandle<crate::Result<()>> {
    tokio::spawn(async move {
        let cancellation = context.cancellation.clone();
        let worker_database_tx = database_tx.clone();
        let worker_reporter = context.reporter.clone();
        let result = crate::install::try_for_each_concurrent_draining(
            stream::poll_fn(move |cx| receiver.poll_recv(cx)),
            Some(MODPACK_VERIFICATION_CONCURRENCY),
            cancellation.clone(),
            move |task| {
                let reporter = worker_reporter.clone();
                let database_tx = worker_database_tx.clone();
                async move {
                    let result = verify_and_record_curseforge_modpack_file(
                        task,
                        database_tx,
                    )
                    .await;
                    if let (Some(reporter), Err(error)) = (&reporter, &result) {
                        reporter.fail_pipeline(error);
                    }
                    result
                }
            },
        )
        .await;
        if result.is_err() {
            // Stop network transfers and peers at the first verifier/SQLite
            // failure. This prevents a closed queue from turning every
            // remaining file into an independent retry/error storm.
            if let (Some(reporter), Err(error)) = (&context.reporter, &result) {
                reporter.fail_pipeline(error);
            }
            cancellation.cancel();
        }
        result
    })
}

pub(crate) async fn persist_curseforge_database_batch(
    batch: &[CurseForgeDatabaseTask],
    context: &CurseForgeVerificationContext,
) -> crate::Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let instance_id = &batch[0].instance_id;
    if batch.iter().any(|task| task.instance_id != *instance_id) {
        return Err(ErrorKind::OtherError(
            "CurseForge database batch mixed multiple instances".to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    if context.cancellation.is_cancelled() {
        restore_curseforge_materializations(instance_id, batch).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        )
        .into());
    }
    let records = batch
        .iter()
        .map(|task| task.record.clone())
        .collect::<Vec<_>>();
    let verified_pending = batch
        .iter()
        .filter_map(|task| task.verified_pending)
        .collect::<Vec<_>>();
    // The permit wait is cancelable, but once the transaction starts we must
    // observe whether it committed before deciding to finalize or restore the
    // corresponding files. Dropping a COMMIT future on cancellation leaves
    // the filesystem/database outcome ambiguous.
    let database_permit = tokio::select! {
        biased;
        _ = context.cancellation.cancelled() => {
            restore_curseforge_materializations(instance_id, batch).await;
            return Err(ErrorKind::OtherError(
                "CurseForge modpack database registration canceled".to_string(),
            ).into());
        }
        permit = state.acquire_install_db_permit() => permit?,
    };
    let write_result: crate::Result<()> =
        crate::state::instances::commands::record_project_files_atomic_with_permit(
            instance_id,
            &records,
            &verified_pending,
            Some(database_permit),
            &state,
        )
        .await;
    if let Err(error) = write_result {
        restore_curseforge_materializations(instance_id, batch).await;
        return Err(error);
    }
    finalize_curseforge_materializations(batch).await;

    if context.cancellation.is_cancelled() {
        return Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        )
        .into());
    }

    let batch_bytes = batch.iter().map(|task| task.expected_bytes).sum::<u64>();
    let batch_files = batch.len() as u64;
    let current_files =
        context.files_done.fetch_add(batch_files, Ordering::Relaxed)
            + batch_files;
    let current_bytes =
        context.bytes_done.fetch_add(batch_bytes, Ordering::Relaxed)
            + batch_bytes;
    let active = context.active_downloads.load(Ordering::Relaxed);
    let message = if context.total_bytes > 0 {
        format!(
            "{current_files}/{} files · {} / {} · {active} downloading in parallel",
            context.total_files,
            format_bytes(current_bytes.min(context.total_bytes)),
            format_bytes(context.total_bytes),
        )
    } else {
        format!(
            "{current_files}/{} files · {active} downloading in parallel",
            context.total_files,
        )
    };
    if let Some(loading_bar) = context.loading_bar.as_deref() {
        emit_loading(loading_bar, batch_files as f64, Some(&message))?;
    }
    if let Some(reporter) = context.reporter.as_ref() {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: current_files,
                    total: context.total_files,
                    secondary: Some(InstallProgressSecondary {
                        current: current_bytes.min(context.total_bytes),
                        total: context.total_bytes,
                    }),
                }),
                context.details.clone(),
                batch
                    .iter()
                    .map(|task| InstallJobEventKind::ContentFileCompleted {
                        path: task.record.relative_path.clone(),
                        bytes: task.expected_bytes,
                    })
                    .collect(),
            )
            .await?;
    }
    Ok(())
}

pub(crate) fn spawn_curseforge_database_worker(
    mut receiver: mpsc::Receiver<CurseForgeDatabaseTask>,
    context: CurseForgeVerificationContext,
) -> tokio::task::JoinHandle<crate::Result<()>> {
    tokio::spawn(async move {
        let cancellation = context.cancellation.clone();
        let result: crate::Result<()> = async {
            while let Some(batch) = receive_curseforge_database_batch(
                &mut receiver,
                &context.cancellation,
            )
            .await?
            {
                persist_curseforge_database_batch(&batch, &context).await?;
            }
            Ok(())
        }
        .await;
        if result.is_err() {
            if let (Some(reporter), Err(error)) = (&context.reporter, &result) {
                reporter.fail_pipeline(error);
            }
            while let Ok(task) = receiver.try_recv() {
                restore_curseforge_materialization(
                    &task.instance_id,
                    &task.full_path,
                    task.previous_path.as_deref(),
                )
                .await;
            }
            cancellation.cancel();
        }
        result
    })
}

pub(crate) async fn receive_curseforge_database_batch<T>(
    receiver: &mut mpsc::Receiver<T>,
    cancellation: &CancellationToken,
) -> crate::Result<Option<Vec<T>>> {
    let first = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(ErrorKind::OtherError(
                "CurseForge modpack database worker canceled".to_string(),
            ).into());
        }
        task = receiver.recv() => match task {
            Some(task) => task,
            None => return Ok(None),
        },
    };
    let mut batch = Vec::with_capacity(MODPACK_DATABASE_BATCH_SIZE);
    batch.push(first);
    let deadline =
        tokio::time::Instant::now() + MODPACK_DATABASE_FLUSH_INTERVAL;
    while batch.len() < MODPACK_DATABASE_BATCH_SIZE {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                // Return the partial batch so the persistence layer can
                // restore every file that was already materialized.
                break;
            }
            task = receiver.recv() => match task {
                Some(task) => batch.push(task),
                None => break,
            },
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    Ok(Some(batch))
}
