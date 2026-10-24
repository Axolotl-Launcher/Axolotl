//! Download verification, fingerprinting, and database batching.

use super::dto::{CurseForgeFile, CurseForgeProject, UnifiedSearchHit};
use super::mapping::{mod_loader_to_slug, project_type_for_class};
use super::metrics::CurseForgeDownloadMetrics;
use super::selection::{
    content_target_folder, curseforge_candidate_urls,
    curseforge_content_validation, curseforge_integrity,
    curseforge_modpack_h2_range_concurrency, format_bytes, is_forge_cdn_url,
};
use super::transport::api_key;
use super::validation::validate_file_name;
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::event::emit::emit_loading;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    ContentProvider, ContentProviderRef, ContentSourceKind, CurseForgeFileId,
    CurseForgeProjectId, ProjectType,
};
use crate::util::download::constants::{
    MODPACK_DATABASE_BATCH_SIZE, MODPACK_DATABASE_FLUSH_INTERVAL,
    MODPACK_VERIFICATION_CONCURRENCY,
};
use crate::util::fetch::{
    ContentValidation, DownloadRequest, FetchProgressFn, ResourceClass,
    download_to_path, sha1_file_async, sha1_file_cancellable,
};
use crate::{ErrorKind, State};
use futures::stream;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) async fn download_curseforge_path(
    url: &str,
    file: &CurseForgeFile,
    destination: &Path,
    validation: ContentValidation,
    progress: Option<&mut FetchProgressFn<'_>>,
    tracking: Option<(&InstallProgressReporter, &str)>,
    h2_range_concurrency: Option<usize>,
    allow_http1_segmented_download: bool,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    let state = State::get().await?;
    let mut request = DownloadRequest::new(url, ResourceClass::CurseForge)
        .with_candidate_urls(curseforge_candidate_urls(url)?)
        .with_integrity(curseforge_integrity(file, validation))
        .with_http1_segmented_download(allow_http1_segmented_download);
    if let Some(concurrency) = h2_range_concurrency {
        request = request.with_h2_range_concurrency(concurrency);
    }
    let parsed = reqwest::Url::parse(url)?;
    if is_forge_cdn_url(&parsed)
        && let Some(key) = api_key()
    {
        request = request.with_header("x-api-key", key);
    }
    if let Some((reporter, item_id)) = tracking {
        request = request.with_install_tracking(
            reporter.clone(),
            item_id,
            file.file_name.clone(),
        );
    }
    download_to_path(
        request,
        destination,
        &state.download_semaphore,
        &state.pool,
        progress,
    )
    .await
}

pub(crate) async fn download_curseforge_archive(
    project_id: u32,
    file_id: u32,
    file: &CurseForgeFile,
    url: &str,
    progress: Option<&mut FetchProgressFn<'_>>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    validate_file_name(&file.file_name)?;
    let state = State::get().await?;
    let path = state
        .directories
        .caches_dir()
        .join("curseforge")
        .join("modpacks")
        .join(project_id.to_string())
        .join(file_id.to_string())
        .join(crate::util::io::sanitize_download_file_name(
            &file.file_name,
        )?);
    let tracking_item_id = path.display().to_string();
    download_curseforge_path(
        url,
        file,
        &path,
        ContentValidation::Jar,
        progress,
        reporter.map(|reporter| (reporter, tracking_item_id.as_str())),
        curseforge_modpack_h2_range_concurrency(file.file_length),
        true,
    )
    .await
}

pub(crate) struct DownloadInstalledFileRequest<'a> {
    pub(super) instance_id: &'a str,
    pub(super) url: &'a str,
    pub(super) file: &'a CurseForgeFile,
    pub(super) project_type: ProjectType,
    pub(super) world_name: Option<&'a str>,
    pub(super) project_id: u32,
    pub(super) file_id: u32,
    pub(super) project_slug: &'a str,
    pub(super) ownership_kind: crate::state::instances::ContentOwnershipKind,
    pub(super) download_metrics: Option<&'a CurseForgeDownloadMetrics>,
    pub(super) defer_persistence: bool,
    pub(super) verification_tx:
        Option<&'a mpsc::Sender<CurseForgeVerificationTask>>,
    pub(super) pre_resolved_relative_path: Option<&'a str>,
    pub(super) expected_file_name: Option<&'a str>,
}

pub(super) struct DownloadedCurseForgeFile {
    pub(super) relative_path: String,
}

pub(crate) struct CurseForgeVerificationTask {
    pub(super) instance_id: String,
    pub(super) relative_path: String,
    pub(super) download_path: PathBuf,
    pub(super) full_path: PathBuf,
    pub(super) file: CurseForgeFile,
    pub(super) project_type: ProjectType,
    pub(super) ownership_kind: crate::state::instances::ContentOwnershipKind,
    pub(super) expected_bytes: u64,
    pub(super) download_result: Option<crate::util::fetch::DownloadResult>,
    pub(super) cancellation: CancellationToken,
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
    pub(super) reporter: Option<InstallProgressReporter>,
    pub(super) loading_bar: Option<Arc<crate::event::LoadingBarId>>,
    pub(super) details: InstallPhaseDetails,
    pub(super) files_done: Arc<AtomicU64>,
    pub(super) bytes_done: Arc<AtomicU64>,
    pub(super) active_downloads: Arc<AtomicU64>,
    pub(super) total_files: u64,
    pub(super) total_bytes: u64,
    pub(super) cancellation: CancellationToken,
}

pub(crate) struct CurseForgeDatabaseTask {
    pub(super) instance_id: String,
    pub(super) record: crate::state::instances::commands::ProjectFileRecord,
    pub(super) verified_pending:
        Option<(CurseForgeProjectId, CurseForgeFileId)>,
    pub(super) expected_bytes: u64,
    pub(super) full_path: PathBuf,
    pub(super) previous_path: Option<PathBuf>,
}

pub(crate) struct ActiveCurseForgeDownload {
    pub(super) counter: Arc<AtomicU64>,
    pub(super) active: bool,
}

impl ActiveCurseForgeDownload {
    pub(super) fn start(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self {
            counter,
            active: true,
        }
    }

    pub(super) fn finish(&mut self) {
        if self.active {
            self.counter.fetch_sub(1, Ordering::Relaxed);
            self.active = false;
        }
    }
}

impl Drop for ActiveCurseForgeDownload {
    fn drop(&mut self) {
        self.finish();
    }
}

async fn remove_curseforge_staged_file(path: &Path) {
    if tokio::fs::try_exists(path).await.unwrap_or(false)
        && let Err(error) = crate::util::io::remove_file(path).await
    {
        tracing::warn!(
            path = %path.display(),
            %error,
            "Failed to remove staged CurseForge download"
        );
    }
}

async fn restore_curseforge_materialization(
    instance_id: &str,
    full_path: &Path,
    previous_path: Option<&Path>,
) {
    let Ok(state) = State::get().await else {
        return;
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;
    if let Err(error) =
        crate::state::restore_project_materialization(full_path, previous_path)
            .await
    {
        tracing::error!(
            instance_id,
            path = %full_path.display(),
            %error,
            "Failed to restore CurseForge file after database failure"
        );
    }
}

async fn restore_curseforge_materializations(
    instance_id: &str,
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks.iter().rev() {
        restore_curseforge_materialization(
            instance_id,
            &task.full_path,
            task.previous_path.as_deref(),
        )
        .await;
    }
}

async fn finalize_curseforge_materializations(
    tasks: &[CurseForgeDatabaseTask],
) {
    for task in tasks {
        if let Err(error) = crate::state::finalize_project_materialization(
            task.previous_path.as_deref(),
        )
        .await
        {
            tracing::warn!(
                path = %task.full_path.display(),
                %error,
                "Failed to remove previous CurseForge file backup"
            );
        }
    }
}

pub(super) async fn verify_and_record_curseforge_modpack_file(
    task: CurseForgeVerificationTask,
    database_tx: mpsc::Sender<CurseForgeDatabaseTask>,
) -> crate::Result<()> {
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }

    let state = State::get().await?;
    let verified = match verify_installed_curseforge_file(
        &task.download_path,
        &task.file,
        Some(&task.cancellation),
        task.download_result.as_ref(),
    )
    .await
    {
        Ok(verified) => verified,
        Err(error) => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    if task.cancellation.is_cancelled() {
        remove_curseforge_staged_file(&task.download_path).await;
        return Err(ErrorKind::OtherError(
            "CurseForge modpack verification canceled".to_string(),
        )
        .into());
    }
    // Only materialization needs the per-instance lock. Hashing and database
    // persistence run in separate workers so verification does not serialize
    // unrelated content operations for the whole hash duration.
    let instance_lock = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => {
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(ErrorKind::OtherError(
                "CurseForge materialization canceled".to_string(),
            ).into());
        }
        lock = state.lock_instance_content(&task.instance_id) => lock,
    };
    let previous_path = match crate::state::materialize_project_download(
        &task.download_path,
        &task.full_path,
    )
    .await
    {
        Ok(previous_path) => previous_path,
        Err(error) => {
            drop(instance_lock);
            remove_curseforge_staged_file(&task.download_path).await;
            return Err(error);
        }
    };
    drop(instance_lock);
    remove_curseforge_staged_file(&task.download_path).await;

    let project_id = CurseForgeProjectId::new(task.file.mod_id)?;
    let file_id = CurseForgeFileId::new(task.file.id)?;
    let record = crate::state::instances::commands::ProjectFileRecord {
        relative_path: task.relative_path.clone(),
        sha1: verified.sha1,
        size: verified.size,
        project_type: task.project_type,
        source_kind: ContentSourceKind::CurseForge,
        ownership_kind: task.ownership_kind,
        provider_ref: Some(ContentProviderRef::CurseForge {
            project_id,
            file_id: Some(file_id),
        }),
        origin: true,
        known_modrinth_project_id: None,
        known_modrinth_version_id: None,
    };
    let verified_pending = match verified.pending_completion {
        CurseForgePendingCompletionProof::None => None,
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            Some((project_id, file_id))
        }
    };
    let database_task = CurseForgeDatabaseTask {
        instance_id: task.instance_id.clone(),
        record,
        verified_pending,
        expected_bytes: task.expected_bytes,
        full_path: task.full_path.clone(),
        previous_path,
    };
    let restore_full_path = database_task.full_path.clone();
    let restore_previous_path = database_task.previous_path.clone();
    let send_result = tokio::select! {
        biased;
        _ = task.cancellation.cancelled() => Err(ErrorKind::OtherError(
            "CurseForge modpack database registration canceled".to_string(),
        ).into()),
        result = database_tx.send(database_task) => result.map_err(|_| ErrorKind::OtherError(
            "CurseForge modpack database worker stopped".to_string(),
        ).into()),
    };
    if let Err(error) = send_result {
        restore_curseforge_materialization(
            &task.instance_id,
            &restore_full_path,
            restore_previous_path.as_deref(),
        )
        .await;
        return Err(error);
    }
    Ok(())
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

pub(super) async fn persist_curseforge_database_batch(
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

pub(super) async fn receive_curseforge_database_batch<T>(
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

pub(crate) async fn download_installed_file(
    request: DownloadInstalledFileRequest<'_>,
) -> crate::Result<DownloadedCurseForgeFile> {
    let DownloadInstalledFileRequest {
        instance_id,
        url,
        file,
        project_type,
        world_name,
        project_id,
        file_id,
        project_slug,
        ownership_kind,
        download_metrics,
        defer_persistence,
        verification_tx,
        pre_resolved_relative_path,
        expected_file_name,
    } = request;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    let file_name =
        crate::util::io::sanitize_download_file_name(&file.file_name)?;
    let folder = content_target_folder(project_type, world_name)?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        chinese_file_title_for_curseforge_slug(project_slug)
            .and_then(|title| localized_content_file_name(&file_name, &title))
            .map(|file_name| format!("{folder}/{file_name}"))
    };
    let relative_path = match pre_resolved_relative_path {
        Some(path) => path.to_string(),
        None => {
            crate::state::resolve_content_install_relative_path(
                instance_id,
                format!("{folder}/{file_name}"),
                localized_candidate,
                &state.pool,
            )
            .await?
        }
    };
    let full_path = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&relative_path);
    if project_type == ProjectType::Mod
        && let Some(expected_file_name) = expected_file_name
        && Path::new(&relative_path).file_name()
            != Some(std::ffi::OsStr::new(
                &crate::util::io::sanitize_download_file_name(
                    expected_file_name,
                )?,
            ))
    {
        return Err(ErrorKind::OtherError(format!(
            "CurseForge install context mismatch before download: expected_file={expected_file_name} installed_path={relative_path}",
        ))
        .into());
    }
    let mut download_path = full_path.as_os_str().to_os_string();
    download_path.push(".installing.download");
    let download_path = Path::new(&download_path);
    let result = download_curseforge_path(
        url,
        file,
        download_path,
        curseforge_content_validation(&file.file_name),
        None,
        download_metrics
            .and_then(|metrics| metrics.reporter.as_ref())
            .map(|reporter| (reporter, relative_path.as_str())),
        None,
        false,
    )
    .await?;
    if let Some(download_metrics) = download_metrics {
        download_metrics.record(&result);
    }
    if defer_persistence {
        if let Some(sender) = verification_tx {
            let cancellation = download_metrics
                .and_then(|metrics| metrics.reporter.as_ref())
                .map(InstallProgressReporter::cancellation_token)
                .unwrap_or_default();
            let verification_task = CurseForgeVerificationTask {
                instance_id: instance_id.to_string(),
                relative_path: relative_path.clone(),
                download_path: download_path.to_path_buf(),
                full_path: full_path.clone(),
                file: file.clone(),
                project_type,
                ownership_kind,
                expected_bytes: file.file_length,
                download_result: Some(result.clone()),
                cancellation: cancellation.clone(),
            };
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => {
                    return Err(ErrorKind::OtherError(
                        "curseforge verification enqueue canceled".to_string(),
                    ).into());
                }
                result = sender.send(verification_task) => {
                    result.map_err(|_| ErrorKind::OtherError(
                        "curseforge verification worker stopped".to_string(),
                    ))?;
                }
            }
        }
        return Ok(DownloadedCurseForgeFile { relative_path });
    }
    let verified = verify_installed_curseforge_file(
        download_path,
        file,
        None,
        Some(&result),
    )
    .await?;
    // Acquire the writer before the instance lock so a progress checkpoint
    // cannot leave this materialization holding the lock indefinitely.
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let previous_path =
        crate::state::materialize_project_download(download_path, &full_path)
            .await?;
    crate::util::io::remove_file(download_path).await?;
    let record_result = record_verified_curseforge_file_with_permit(
        instance_id,
        &relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        &state,
        verified,
    )
    .await;
    match record_result {
        Ok(()) => {
            crate::state::finalize_project_materialization(
                previous_path.as_deref(),
            )
            .await?;
        }
        Err(error) => {
            crate::state::restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(DownloadedCurseForgeFile { relative_path })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CurseForgePendingCompletionProof {
    None,
    AuthoritativeSha1,
    AuthoritativeFingerprint,
}

pub(super) struct VerifiedInstalledCurseForgeFile {
    pub(super) size: u64,
    pub(super) sha1: String,
    pub(super) pending_completion: CurseForgePendingCompletionProof,
}

async fn read_curseforge_verification_chunk(
    file: &mut tokio::fs::File,
    buffer: &mut [u8],
    cancellation: Option<&CancellationToken>,
) -> crate::Result<usize> {
    match cancellation {
        Some(cancellation) => tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(ErrorKind::OtherError(
                "CurseForge fingerprint verification canceled".to_string(),
            ).into()),
            result = file.read(buffer) => Ok(result?),
        },
        None => Ok(file.read(buffer).await?),
    }
}

pub(crate) async fn fingerprint_and_sha1_file(
    path: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<(u64, String, u32)> {
    #[cfg(test)]
    crate::util::download::verified_file::record_scan();
    let started = std::time::Instant::now();
    let acquire = crate::util::fetch::acquire_native_validation_permit();
    let _permit = if let Some(cancellation) = cancellation {
        tokio::select! { biased; _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("fingerprint verification canceled while waiting for resources".into()).into()), permit = acquire => permit? }
    } else {
        acquire.await?
    };

    use crate::util::download::constants::CURSEFORGE_VERIFY_BUFFER_SIZE as BUFFER_SIZE;
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut sha1 = sha1_smol::Sha1::new();
    let mut size = 0_u64;
    let mut normalized_size = 0_u64;
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        let bytes = &buffer[..read];
        sha1.update(bytes);
        size += read as u64;
        normalized_size += bytes
            .iter()
            .filter(|byte| !is_curseforge_fingerprint_whitespace(**byte))
            .count() as u64;
    }

    let mut file = tokio::fs::File::open(path).await?;
    let mut fingerprint =
        CurseForgeFingerprintHasher::new(normalized_size as u32);
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        fingerprint.update(&buffer[..read]);
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed CurseForge fingerprint verification");
    Ok((size, sha1.digest().to_string(), fingerprint.finish()))
}

pub(crate) async fn verify_installed_curseforge_file(
    path: &Path,
    file: &CurseForgeFile,
    cancellation: Option<&CancellationToken>,
    download: Option<&crate::util::fetch::DownloadResult>,
) -> crate::Result<VerifiedInstalledCurseForgeFile> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ErrorKind::OtherError(
            "CurseForge verification canceled".into(),
        )
        .into());
    }
    if let Some(expected_sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        .map(|hash| hash.value.as_str())
    {
        if let Some(download) = download
            && download.path == path
            && download
                .verified_sha1
                .as_deref()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(expected_sha1))
            && let Some(proof) = &download.verified_file
            && proof.matches(path, download.size).await
        {
            return Ok(VerifiedInstalledCurseForgeFile {
                size: download.size,
                sha1: expected_sha1.to_string(),
                pending_completion:
                    CurseForgePendingCompletionProof::AuthoritativeSha1,
            });
        }
        let (size, sha1) = match cancellation {
            Some(cancellation) => {
                sha1_file_cancellable(path, cancellation).await?
            }
            None => sha1_file_async(path).await?,
        };
        if !sha1.eq_ignore_ascii_case(expected_sha1) {
            return Err(
                ErrorKind::HashError(expected_sha1.to_string(), sha1).into()
            );
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeSha1,
        });
    }

    if file.file_fingerprint != 0 {
        let (size, sha1, fingerprint) =
            fingerprint_and_sha1_file(path, cancellation).await?;
        if fingerprint as u64 != file.file_fingerprint {
            return Err(ErrorKind::InputError(
                "The downloaded file does not match the required CurseForge fingerprint"
                    .to_string(),
            )
            .into());
        }
        return Ok(VerifiedInstalledCurseForgeFile {
            size,
            sha1,
            pending_completion:
                CurseForgePendingCompletionProof::AuthoritativeFingerprint,
        });
    }

    let (size, sha1) = match cancellation {
        Some(cancellation) => sha1_file_cancellable(path, cancellation).await?,
        None => sha1_file_async(path).await?,
    };
    Ok(VerifiedInstalledCurseForgeFile {
        size,
        sha1,
        pending_completion: CurseForgePendingCompletionProof::None,
    })
}

pub(crate) async fn record_installed_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    full_path: &Path,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
) -> crate::Result<()> {
    let verified =
        verify_installed_curseforge_file(full_path, file, None, None).await?;
    record_verified_curseforge_file_with_permit(
        instance_id,
        relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        state,
        verified,
    )
    .await
}

async fn record_verified_curseforge_file_with_permit(
    instance_id: &str,
    relative_path: &str,
    file: &CurseForgeFile,
    project_type: ProjectType,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    database_permit: Option<tokio::sync::SemaphorePermit<'_>>,
    state: &State,
    verified: VerifiedInstalledCurseForgeFile,
) -> crate::Result<()> {
    match verified.pending_completion {
        CurseForgePendingCompletionProof::None => {
            let provider_ref = ContentProviderRef::CurseForge {
                project_id: CurseForgeProjectId::new(file.mod_id)?,
                file_id: Some(CurseForgeFileId::new(file.id)?),
            };
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(provider_ref),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[],
                database_permit,
                state,
            )
            .await
        }
        CurseForgePendingCompletionProof::AuthoritativeSha1
        | CurseForgePendingCompletionProof::AuthoritativeFingerprint => {
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                instance_id,
                &[crate::state::instances::commands::ProjectFileRecord {
                    relative_path: relative_path.to_string(),
                    sha1: verified.sha1,
                    size: verified.size,
                    project_type,
                    source_kind: ContentSourceKind::CurseForge,
                    ownership_kind,
                    provider_ref: Some(ContentProviderRef::CurseForge {
                        project_id: CurseForgeProjectId::new(file.mod_id)?,
                        file_id: Some(CurseForgeFileId::new(file.id)?),
                    }),
                    origin: true,
                    known_modrinth_project_id: None,
                    known_modrinth_version_id: None,
                }],
                &[(CurseForgeProjectId::new(file.mod_id)?, CurseForgeFileId::new(file.id)?)],
                database_permit,
                state,
            )
            .await
        }
    }
}

pub(super) fn is_curseforge_fingerprint_whitespace(byte: u8) -> bool {
    matches!(byte, 9 | 10 | 13 | 32)
}

pub(crate) struct CurseForgeFingerprintHasher {
    pub(super) hash: u32,
    pub(super) tail: [u8; 4],
    pub(super) tail_len: usize,
}

impl CurseForgeFingerprintHasher {
    pub(crate) fn new(normalized_size: u32) -> Self {
        Self {
            hash: 1 ^ normalized_size,
            tail: [0; 4],
            tail_len: 0,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if is_curseforge_fingerprint_whitespace(byte) {
                continue;
            }
            self.tail[self.tail_len] = byte;
            self.tail_len += 1;
            if self.tail_len == self.tail.len() {
                self.mix_word(u32::from_le_bytes(self.tail));
                self.tail_len = 0;
            }
        }
    }

    fn mix_word(&mut self, mut value: u32) {
        const M: u32 = 0x5bd1e995;
        const R: u32 = 24;
        value = value.wrapping_mul(M);
        value ^= value >> R;
        value = value.wrapping_mul(M);
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= value;
    }

    pub(crate) fn finish(mut self) -> u32 {
        const M: u32 = 0x5bd1e995;
        match self.tail[..self.tail_len] {
            [a, b, c] => {
                self.hash ^= (c as u32) << 16;
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a, b] => {
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a] => {
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [] => {}
            _ => unreachable!(),
        }
        self.hash ^= self.hash >> 13;
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= self.hash >> 15;
        self.hash
    }
}

pub fn compute_fingerprint(data: &[u8]) -> u32 {
    let normalized_size = data
        .iter()
        .filter(|byte| !is_curseforge_fingerprint_whitespace(**byte))
        .count() as u32;
    let mut hasher = CurseForgeFingerprintHasher::new(normalized_size);
    hasher.update(data);
    hasher.finish()
}

impl From<CurseForgeProject> for UnifiedSearchHit {
    fn from(project: CurseForgeProject) -> Self {
        let mut versions = Vec::new();
        let mut seen_versions = HashSet::new();
        let mut loaders = HashSet::new();
        for index in &project.latest_files_indexes {
            if seen_versions.insert(index.game_version.clone()) {
                versions.push(index.game_version.clone());
            }
            if let Some(mod_loader) = index.mod_loader {
                let loader_slug = mod_loader_to_slug(mod_loader);
                loaders.insert(loader_slug);
            }
        }

        let project_type = project_type_for_class(project.class_id);
        let mut categories: Vec<String> = project
            .categories
            .iter()
            .map(|category| category.slug.clone())
            .collect();
        categories.extend(loaders.iter().map(|s| s.to_string()));

        Self {
            provider: ContentProvider::CurseForge,
            project_id: project.id.to_string(),
            slug: Some(project.slug),
            author: project
                .authors
                .first()
                .map(|author| author.name.clone())
                .unwrap_or_default(),
            author_url: project
                .authors
                .first()
                .map(|author| author.url.clone()),
            title: project.name,
            description: project.summary,
            project_type: project_type.to_string(),
            categories,
            versions,
            downloads: project.download_count,
            icon_url: project.logo.map(|logo| logo.url),
            date_created: project.date_created,
            date_modified: project.date_modified,
            latest_version: project
                .latest_files
                .first()
                .map(|file| file.id.to_string()),
            gallery: project
                .screenshots
                .into_iter()
                .map(|screenshot| screenshot.url)
                .collect(),
            website_url: project.links.website_url,
            source_url: project.links.source_url,
            allow_mod_distribution: project.allow_mod_distribution,
        }
    }
}
