use crate::State;
use crate::api::content_search::{
    chinese_file_title_for_modrinth_slug, localized_content_relative_path,
    original_content_relative_path,
};
use crate::event::emit::loading_try_for_each_concurrent;
use crate::install::model::{
    InstallPauseReason, MissingModpackContentState, MissingModpackFileState,
    OptionalModpackContentState, OptionalModpackFileState,
};
use crate::install::{
    DownloadItemStatus, InstallErrorContext, InstallJobEventKind,
    InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::pack::install_from::{
    EnvType, PackFile, PackFileHash, set_instance_information,
};
use crate::state::instances::ContentSourceKind;
use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::{
    CachedEntry, ContentProviderRef, ModrinthHashMatch, ModrinthProjectId,
    ModrinthVersionId, Settings, SideType,
};
use crate::util::download::constants::AGGREGATE_FAILURE_PATH_CHAR_LIMIT;
use crate::util::download::constants::AGGREGATE_FAILURE_PATH_LIMIT;
use crate::util::download::constants::AUTO_RETRY_PASSES;
use crate::util::download::constants::CONTENT_DATABASE_BATCH_SIZE;
use crate::util::download::constants::CONTENT_DATABASE_FLUSH_INTERVAL;
use crate::util::download::constants::FINALIZE_WAIT_TIMEOUT;
use crate::util::download::constants::ITEM_FAILURE_REASON_CHAR_LIMIT;
use crate::util::download::constants::NATIVE_CONTENT_FINALIZE_CONCURRENCY;
use crate::util::download::constants::OVERRIDE_EXTRACTION_CONCURRENCY;
use crate::util::fetch::{
    ContentValidation, DownloadMeta, DownloadReason, DownloadRequest,
    Integrity, ResourceClass, download_to_path,
};
use crate::util::io;
use async_zip::base::read::seek::ZipFileReader as SeekZipFileReader;
use async_zip::base::read::{WithEntry, ZipEntryReader};
use async_zip::tokio::read::fs::ZipFileReader as FsZipFileReader;
use futures::StreamExt;
use path_util::SafeRelativeUtf8UnixPathBuf;
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(test)]
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
#[cfg(test)]
use std::time::Duration;

use super::install_from::{CreatePack, CreatePackFile, PackFormat};
use super::parallel_minecraft_install::ParallelMinecraftInstall;
use crate::data::ProjectType;
use std::io::{Cursor, ErrorKind};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore, mpsc};

type ExtractProgressFn<'a> = dyn FnMut(u64) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + 'a>>
    + Send
    + 'a;

struct MrpackVerificationTask {
    project: PackFile,
    project_path: String,
    download_path: PathBuf,
    target_path: PathBuf,
    downloaded_bytes: u64,
    attempts: u32,
    finalize_semaphore: Option<Arc<Semaphore>>,
}

impl Drop for MrpackVerificationTask {
    fn drop(&mut self) {
        // A verifier can be dropped when another pipeline stage fails. The
        // final target is never stored here until materialization succeeds,
        // so removing this staged file cannot damage the installed instance.
        let _ = std::fs::remove_file(&self.download_path);
    }
}

struct MrpackDatabaseTask {
    record: crate::state::instances::commands::ProjectFileRecord,
    settled_bytes: u64,
    event: InstallJobEventKind,
    target_path: PathBuf,
    previous_path: Option<PathBuf>,
}

fn mrpack_staged_download_path(target_path: &Path) -> PathBuf {
    let mut path = target_path.as_os_str().to_os_string();
    path.push(".installing.download");
    PathBuf::from(path)
}

async fn seed_mrpack_staged_download(target_path: &Path, download_path: &Path) {
    if download_path.exists() || !target_path.exists() {
        return;
    }
    match tokio::fs::hard_link(target_path, download_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => tracing::debug!(
            path = %target_path.display(),
            staged_path = %download_path.display(),
            %error,
            "Could not seed staged MRPack download from existing file"
        ),
    }
}

async fn remove_mrpack_staged_file(path: &Path) {
    if tokio::fs::try_exists(path).await.unwrap_or(false)
        && let Err(error) = crate::util::io::remove_file(path).await
    {
        tracing::warn!(
            path = %path.display(),
            %error,
            "Failed to remove staged MRPack download"
        );
    }
}

async fn restore_mrpack_materialization(
    instance_id: &str,
    target_path: &Path,
    previous_path: Option<&Path>,
) {
    let Ok(state) = State::get().await else {
        return;
    };
    let _instance_lock = state.lock_instance_content(instance_id).await;
    if let Err(error) = crate::state::restore_project_materialization(
        target_path,
        previous_path,
    )
    .await
    {
        tracing::error!(
            instance_id,
            path = %target_path.display(),
            %error,
            "Failed to restore MRPack file after database failure"
        );
    }
}

async fn restore_mrpack_materializations(
    instance_id: &str,
    tasks: &[MrpackDatabaseTask],
) {
    for task in tasks.iter().rev() {
        restore_mrpack_materialization(
            instance_id,
            &task.target_path,
            task.previous_path.as_deref(),
        )
        .await;
    }
}

async fn finalize_mrpack_materializations(tasks: &[MrpackDatabaseTask]) {
    for task in tasks {
        if let Err(error) = crate::state::finalize_project_materialization(
            task.previous_path.as_deref(),
        )
        .await
        {
            tracing::warn!(
                path = %task.target_path.display(),
                %error,
                "Failed to remove previous MRPack file backup"
            );
        }
    }
}

pub(crate) enum MrpackInstallOutcome {
    #[allow(dead_code)]
    Completed(String),
    WaitingForUser(InstallPauseReason),
}

#[derive(Clone)]
struct OverrideExtractionSpec {
    index: usize,
    entry_name: String,
    crc32: u32,
    uncompressed_size: u64,
    relative_path: String,
    target_path: PathBuf,
}

struct ExtractedOverride {
    spec: OverrideExtractionSpec,
    size: u64,
    hash: String,
}

fn override_extraction_groups(
    specs: Vec<OverrideExtractionSpec>,
) -> Vec<Vec<OverrideExtractionSpec>> {
    let mut groups: Vec<Vec<OverrideExtractionSpec>> = Vec::new();
    let mut positions = HashMap::<PathBuf, usize>::new();
    for spec in specs {
        if let Some(&index) = positions.get(&spec.target_path) {
            groups[index].push(spec);
        } else {
            positions.insert(spec.target_path.clone(), groups.len());
            groups.push(vec![spec]);
        }
    }
    groups
}

fn mrpack_override_staging_path(target_path: &Path) -> PathBuf {
    let mut path = target_path.as_os_str().to_os_string();
    path.push(".installing");
    PathBuf::from(path)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequiredFileFailure {
    manifest_index: usize,
    path: String,
    reason: String,
}

async fn persist_modpack_record_batch(
    instance_id: &str,
    batch: &[MrpackDatabaseTask],
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let state = State::get().await?;
    if cancellation.is_cancelled() {
        return Err(crate::ErrorKind::OtherError(
            "modpack database registration canceled".to_string(),
        )
        .into());
    }
    let records = batch
        .iter()
        .map(|task| task.record.clone())
        .collect::<Vec<_>>();
    // Cancellation remains responsive while waiting for the database permit,
    // but an atomic write that has started must be driven to a definitive
    // result. Dropping the SQL future while COMMIT is in flight makes it
    // impossible to know whether the files should be finalized or restored.
    let database_permit = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(crate::ErrorKind::OtherError(
                "modpack database registration canceled".to_string(),
            ).into());
        }
        permit = state.acquire_install_db_permit() => permit?,
    };
    crate::state::instances::commands::record_project_files_atomic_with_permit(
        instance_id,
        &records,
        &[],
        Some(database_permit),
        &state,
    )
    .await
}

async fn receive_modpack_database_batch<T>(
    receiver: &mut mpsc::Receiver<T>,
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<Option<Vec<T>>> {
    let first = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(crate::ErrorKind::OtherError(
                "modpack database worker canceled".to_string(),
            ).into());
        }
        task = receiver.recv() => match task {
            Some(task) => task,
            None => return Ok(None),
        },
    };
    let mut batch = Vec::with_capacity(CONTENT_DATABASE_BATCH_SIZE);
    batch.push(first);
    let deadline =
        tokio::time::Instant::now() + CONTENT_DATABASE_FLUSH_INTERVAL;
    while batch.len() < CONTENT_DATABASE_BATCH_SIZE {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                // Return the partial batch to the database worker so it can
                // restore every file already materialized for this batch.
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

impl RequiredFileFailure {
    fn new(manifest_index: usize, path: &str, reason: &str) -> Self {
        Self {
            manifest_index,
            path: bounded_text(path, AGGREGATE_FAILURE_PATH_CHAR_LIMIT),
            reason: bounded_text(reason, ITEM_FAILURE_REASON_CHAR_LIMIT),
        }
    }
}

#[cfg(test)]
struct RequiredFileFailures<'a>(&'a [RequiredFileFailure]);

#[cfg(test)]
impl fmt::Display for RequiredFileFailures<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failures = self.0;
        write!(
            formatter,
            "{} required modpack file{} failed",
            failures.len(),
            if failures.len() == 1 { "" } else { "s" }
        )?;
        let shown = failures
            .iter()
            .take(AGGREGATE_FAILURE_PATH_LIMIT)
            .map(|failure| failure.path.as_str())
            .collect::<Vec<_>>();
        if !shown.is_empty() {
            write!(formatter, ": {}", shown.join(", "))?;
        }
        if failures.len() > shown.len() {
            write!(formatter, " (+{} more)", failures.len() - shown.len())?;
        }
        Ok(())
    }
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}...")
    } else {
        bounded
    }
}

fn estimated_file_max_attempts(file: &PackFile) -> usize {
    file.downloads.len().saturating_mul(2).max(1)
}

async fn collect_required_file_failures_concurrently<T, F, Fut>(
    items: Vec<T>,
    limit: Option<usize>,
    mut task: F,
) -> crate::Result<Vec<RequiredFileFailure>>
where
    T: Send,
    F: FnMut(T) -> Fut + Send,
    Fut: Future<Output = Result<(), RequiredFileFailure>> + Send,
{
    let failures = Arc::new(Mutex::new(Vec::<RequiredFileFailure>::new()));
    let collected_failures = failures.clone();
    let num_items = items.len();
    loading_try_for_each_concurrent(
        futures::stream::iter(items).map(Ok::<T, crate::Error>),
        limit,
        None,
        70.0,
        num_items,
        None,
        move |item| {
            let future = task(item);
            let failures = failures.clone();
            async move {
                if let Err(failure) = future.await {
                    failures.lock().await.push(failure);
                }
                Ok(())
            }
        },
    )
    .await?;
    let mut failures = collected_failures.lock().await.clone();
    failures.sort_unstable_by_key(|failure| failure.manifest_index);
    Ok(failures)
}

fn missing_required_content_pause(
    failures: &[RequiredFileFailure],
) -> Option<InstallPauseReason> {
    (!failures.is_empty()).then(|| InstallPauseReason::MissingRequiredContent {
        failed_files: failures.len() as u64,
        paths: failures
            .iter()
            .take(AGGREGATE_FAILURE_PATH_LIMIT)
            .map(|failure| failure.path.clone())
            .collect(),
    })
}

#[derive(Clone)]
struct ModpackContentInstallContext {
    instance_id: String,
    instance_full_path: PathBuf,
    download_meta: DownloadMeta,
    pack_version_id: Option<String>,
    pack_project_id: Option<String>,
    reporter: InstallProgressReporter,
    modpack_details: InstallPhaseDetails,
    content_progress: Arc<AtomicU64>,
    content_bytes_progress: Arc<AtomicU64>,
    download_source: Arc<Mutex<Option<String>>>,
    fallback_count: Arc<AtomicU64>,
    file_infos_by_hash: Arc<Mutex<Option<HashMap<String, ModrinthHashMatch>>>>,
    chinese_titles_by_sha1: Arc<HashMap<String, String>>,
    existing_paths_by_original: Arc<HashMap<String, String>>,
    num_files: usize,
    content_total_bytes: u64,
}

/// Resolves Modrinth file metadata by SHA-1 hash. All completions share a
/// single lookup (double-checked locking through `file_infos_loading`);
/// failures are cached as an empty map so a broken lookup is not retried once
/// per remaining file.
async fn load_modpack_file_infos(
    file_infos_by_hash: &Arc<Mutex<Option<HashMap<String, ModrinthHashMatch>>>>,
    file_infos_loading: &Arc<Mutex<()>>,
    file_info_hashes: &Arc<Vec<String>>,
    pool: &sqlx::SqlitePool,
    api_semaphore: &crate::util::fetch::FetchSemaphore,
) {
    {
        let guard = file_infos_by_hash.lock().await;
        if guard.is_some() {
            return;
        }
    }
    let _loading = file_infos_loading.lock().await;
    {
        let guard = file_infos_by_hash.lock().await;
        if guard.is_some() {
            return;
        }
    }
    let hash_refs = file_info_hashes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let loaded =
        CachedEntry::get_file_many(&hash_refs, None, pool, api_semaphore)
            .await
            .map(|files| {
                files
                    .into_iter()
                    .map(|file| (file.hash.clone(), file))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
    *file_infos_by_hash.lock().await = Some(loaded);
}

/// Maps manifest file hashes to sanitized Chinese titles by resolving the
/// files' Modrinth projects from cache. Failures only disable the naming.
async fn resolve_chinese_titles_by_sha1(
    file_infos_by_hash: &HashMap<String, ModrinthHashMatch>,
    state: &State,
) -> HashMap<String, String> {
    let project_ids = file_infos_by_hash
        .values()
        .filter_map(|file| ModrinthProjectId::new(file.project_id.clone()).ok())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if project_ids.is_empty() {
        return HashMap::new();
    }
    let projects = match CachedEntry::get_project_many(
        &project_ids,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(projects) => projects,
        Err(error) => {
            tracing::warn!(
                "Failed to resolve project metadata for Chinese content file names: {error}"
            );
            return HashMap::new();
        }
    };
    let titles_by_project_id = projects
        .into_iter()
        .filter_map(|project| {
            let slug = project.slug?;
            let title = chinese_file_title_for_modrinth_slug(&slug)?;
            Some((project.id, title))
        })
        .collect::<HashMap<_, _>>();
    file_infos_by_hash
        .iter()
        .filter_map(|(sha1, file)| {
            titles_by_project_id
                .get(&file.project_id)
                .map(|title| (sha1.clone(), title.clone()))
        })
        .collect()
}

impl ModpackContentInstallContext {
    /// Picks the instance-relative install path for a pack file. Paths already
    /// recorded for this instance win so repairs and locale switches never
    /// produce duplicate files; new non-mod content may get a `[中文名]` prefix
    /// when Chinese file naming is active. Mod files always keep manifest names.
    fn resolve_install_path(&self, file: &PackFile) -> String {
        let manifest_path = file.path.as_str();
        if let Some(existing) =
            self.existing_paths_by_original.get(manifest_path)
        {
            return existing.clone();
        }
        let Some(project_type) =
            ProjectType::get_from_parent_folder(manifest_path)
        else {
            return manifest_path.to_string();
        };

        if project_type == ProjectType::Mod {
            return manifest_path.to_string();
        }
        let Some(title) = file
            .hashes
            .get(&PackFileHash::Sha1)
            .and_then(|sha1| self.chinese_titles_by_sha1.get(sha1))
        else {
            return manifest_path.to_string();
        };
        localized_content_relative_path(manifest_path, title)
            .unwrap_or_else(|| manifest_path.to_string())
    }

    async fn report_download_attempt(
        &self,
        path: String,
        file_size: u64,
        attempt: usize,
        max_attempts: usize,
    ) -> crate::Result<()> {
        let current_bytes = self
            .content_bytes_progress
            .load(Ordering::Relaxed)
            .min(self.content_total_bytes);
        self.reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: self.content_progress.load(Ordering::Relaxed),
                    total: self.num_files as u64,
                    secondary: (self.content_total_bytes > 0).then_some(
                        InstallProgressSecondary {
                            current: current_bytes,
                            total: self.content_total_bytes,
                        },
                    ),
                }),
                self.modpack_details.clone(),
                vec![InstallJobEventKind::ContentFileDownloadAttempt {
                    path,
                    bytes_total: (file_size > 0).then_some(file_size),
                    attempt: attempt as u32,
                    max_attempts: max_attempts as u32,
                }],
            )
            .await
    }

    async fn mark_file_settled(
        &self,
        settled_bytes: u64,
        event: InstallJobEventKind,
    ) -> crate::Result<()> {
        self.mark_files_settled(1, settled_bytes, vec![event]).await
    }

    async fn mark_files_settled(
        &self,
        settled_files: u64,
        settled_bytes: u64,
        events: Vec<InstallJobEventKind>,
    ) -> crate::Result<()> {
        let current = self
            .content_progress
            .fetch_add(settled_files, Ordering::Relaxed)
            + settled_files;
        let current_bytes = self
            .content_bytes_progress
            .fetch_add(settled_bytes, Ordering::Relaxed)
            + settled_bytes;

        self.reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current,
                    total: self.num_files as u64,
                    secondary: (self.content_total_bytes > 0).then_some(
                        InstallProgressSecondary {
                            current: current_bytes
                                .min(self.content_total_bytes),
                            total: self.content_total_bytes,
                        },
                    ),
                }),
                self.modpack_details.clone(),
                events,
            )
            .await
    }

    async fn record_download_result(
        &self,
        result: &crate::util::fetch::DownloadResult,
    ) {
        if result.attempts > 0 {
            *self.download_source.lock().await =
                Some(result.source.as_str().to_string());
        }
        self.fallback_count
            .fetch_add(result.fallback_count as u64, Ordering::Relaxed);
    }

    async fn finish_download_metrics(&self) -> crate::Result<()> {
        let source = self.download_source.lock().await.clone();
        if let Some(source) = source {
            self.reporter
                .record_download_metrics(
                    source,
                    self.fallback_count.load(Ordering::Relaxed),
                )
                .await?;
        }
        Ok(())
    }
}

enum MrpackZipReader {
    Memory(async_zip::tokio::read::seek::ZipFileReader<Cursor<bytes::Bytes>>),
    // Local imports stay on disk so large .mrpacks do not have to fit in memory.
    File(FsZipFileReader),
}

impl MrpackZipReader {
    async fn new(file: &CreatePackFile) -> crate::Result<Self> {
        match file {
            CreatePackFile::Bytes(file) => Ok(Self::Memory(
                SeekZipFileReader::with_tokio(Cursor::new(file.clone()))
                    .await
                    .map_err(|_| {
                        crate::Error::from(crate::ErrorKind::InputError(
                            "Failed to read input modpack zip".to_string(),
                        ))
                    })?,
            )),
            CreatePackFile::Path(path) => Ok(Self::File(
                FsZipFileReader::new(path).await.map_err(|_| {
                    crate::Error::from(crate::ErrorKind::InputError(
                        "Failed to read input modpack zip".to_string(),
                    ))
                })?,
            )),
        }
    }

    fn file(&self) -> &async_zip::ZipFile {
        match self {
            Self::Memory(reader) => reader.file(),
            Self::File(reader) => reader.file(),
        }
    }

    async fn read_entry_to_string(
        &mut self,
        index: usize,
    ) -> crate::Result<String> {
        let mut value = String::new();
        match self {
            Self::Memory(reader) => {
                let mut reader = reader.reader_with_entry(index).await?;
                reader.read_to_string_checked(&mut value).await?;
            }
            Self::File(reader) => {
                let mut reader = reader.reader_with_entry(index).await?;
                reader.read_to_string_checked(&mut value).await?;
            }
        }

        Ok(value)
    }

    async fn hash_entry(
        &mut self,
        index: usize,
    ) -> crate::Result<(u64, String)> {
        match self {
            Self::Memory(reader) => {
                hash_zip_entry(reader.reader_with_entry(index).await?).await
            }
            Self::File(reader) => {
                hash_zip_entry(reader.reader_with_entry(index).await?).await
            }
        }
    }

    async fn extract_entry(
        &mut self,
        index: usize,
        path: &Path,
        semaphore: &crate::util::fetch::IoSemaphore,
        progress: Option<&mut ExtractProgressFn<'_>>,
    ) -> crate::Result<(u64, String)> {
        match self {
            Self::Memory(reader) => {
                extract_zip_entry(
                    reader.reader_with_entry(index).await?,
                    path,
                    semaphore,
                    progress,
                )
                .await
            }
            Self::File(reader) => {
                extract_zip_entry(
                    reader.reader_with_entry(index).await?,
                    path,
                    semaphore,
                    progress,
                )
                .await
            }
        }
    }

    async fn extract_override_entry(
        &mut self,
        spec: &OverrideExtractionSpec,
        path: &Path,
        semaphore: &crate::util::fetch::IoSemaphore,
        progress: Option<&mut ExtractProgressFn<'_>>,
    ) -> crate::Result<(u64, String)> {
        let entry = self.file().entries().get(spec.index).ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Override entry {} is missing from reopened modpack archive",
                spec.entry_name
            ))
        })?;
        let entry_name = entry.filename().as_str().map_err(|_| {
            crate::ErrorKind::InputError(format!(
                "Override entry {} has an invalid filename in reopened modpack archive",
                spec.entry_name
            ))
        })?;
        if entry_name != spec.entry_name
            || entry.crc32() != spec.crc32
            || entry.uncompressed_size() != spec.uncompressed_size
        {
            return Err(crate::ErrorKind::InputError(format!(
                "Override entry {} does not match the original modpack archive layout",
                spec.entry_name
            ))
            .into());
        }
        self.extract_entry(spec.index, path, semaphore, progress)
            .await
    }
}

async fn hash_zip_entry<R>(
    mut reader: ZipEntryReader<'_, R, WithEntry<'_>>,
) -> crate::Result<(u64, String)>
where
    R: futures_lite::io::AsyncBufRead + Unpin,
{
    let expected_crc32 = reader.entry().crc32();
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; 262144];

    loop {
        let bytes_read =
            futures_lite::io::AsyncReadExt::read(&mut reader, &mut buffer)
                .await?;
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }

    if reader.compute_hash() != expected_crc32 {
        return Err(async_zip::error::ZipError::CRC32CheckError.into());
    }

    Ok((size, hasher.digest().to_string()))
}

async fn extract_zip_entry<R>(
    mut reader: ZipEntryReader<'_, R, WithEntry<'_>>,
    path: &Path,
    semaphore: &crate::util::fetch::IoSemaphore,
    mut progress: Option<&mut ExtractProgressFn<'_>>,
) -> crate::Result<(u64, String)>
where
    R: futures_lite::io::AsyncBufRead + Unpin,
{
    let _permit = semaphore.0.acquire().await?;

    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }

    let parent = path.parent().ok_or_else(|| {
        io::IOError::from(std::io::Error::other(
            "could not get parent directory for temporary file",
        ))
    })?;
    let temp_path = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| io::IOError::with_path(e, parent))?
        .into_temp_path();

    // Only replace the profile file after the ZIP entry has passed its CRC check.
    let expected_crc32 = reader.entry().crc32();
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| io::IOError::with_path(e, &temp_path))?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; 262144];
    let mut pending_progress = 0_u64;

    use crate::util::download::constants::PROGRESS_GRANULARITY;

    loop {
        let bytes_read =
            futures_lite::io::AsyncReadExt::read(&mut reader, &mut buffer)
                .await?;
        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])
            .await
            .map_err(|e| io::IOError::with_path(e, &temp_path))?;
        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
        pending_progress += bytes_read as u64;
        if let Some(progress) = progress.as_mut()
            && pending_progress >= PROGRESS_GRANULARITY
        {
            progress(pending_progress).await?;
            pending_progress = 0;
        }
    }
    if let Some(progress) = progress.as_mut()
        && pending_progress > 0
    {
        progress(pending_progress).await?;
    }
    drop(file);

    if reader.compute_hash() != expected_crc32 {
        return Err(async_zip::error::ZipError::CRC32CheckError.into());
    }

    temp_path.persist(path).map_err(|e| {
        let tempfile::PathPersistError { error, .. } = e;
        io::IOError::with_path(error, path)
    })?;

    Ok((size, hasher.digest().to_string()))
}

pub(crate) async fn install_zipped_mrpack_files_with_reporter(
    create_pack: CreatePack,
    ignore_lock: bool,
    reason: DownloadReason,
    reporter: InstallProgressReporter,
) -> crate::Result<MrpackInstallOutcome> {
    let state = &State::get().await?;

    let file = create_pack.file;
    let description = create_pack.description.clone();
    let icon = create_pack.description.icon;
    let project_id = create_pack.description.project_id;
    let version_id = create_pack.description.version_id;
    let instance_id = create_pack.description.instance_id;
    let mut icon_exists = icon.is_some();
    let source_path = pack_source_path(&file);

    reporter
        .set_context(
            InstallErrorContext::new("read modpack archive")
                .maybe_project_id(project_id.clone())
                .maybe_version_id(version_id.clone())
                .source_path(source_path.clone())
                .build(),
        )
        .await?;
    let mut zip_reader = MrpackZipReader::new(&file).await?;
    let instance_full_path =
        crate::api::instance::get_full_path(&instance_id).await?;
    let modpack_details = InstallPhaseDetails::Modpack {
        project_id: project_id.clone(),
        version_id: version_id.clone(),
        title: description.override_title.clone(),
    };
    reporter
        .update(
            InstallPhaseId::ReadingPackManifest,
            None,
            modpack_details.clone(),
        )
        .await?;
    reporter
        .set_context(
            InstallErrorContext::new("read modpack manifest")
                .maybe_project_id(project_id.clone())
                .maybe_version_id(version_id.clone())
                .source_path(source_path.clone())
                .entry_path("modrinth.index.json")
                .build(),
        )
        .await?;

    // Extract index of modrinth.index.json, tolerating packs zipped inside a
    // single wrapping folder. A root-level manifest always wins so override
    // files that happen to share the name cannot hijack the base folder.
    let Some((manifest_idx, archive_base)) =
        locate_mrpack_manifest(zip_reader.file())
    else {
        return Err(crate::Error::from(crate::ErrorKind::InputError(
            "No pack manifest found in mrpack".to_string(),
        )));
    };
    let overrides_prefix = format!("{archive_base}overrides/");
    let client_overrides_prefix = format!("{archive_base}client-overrides/");
    let server_overrides_prefix = format!("{archive_base}server-overrides/");
    let mut manifest = String::new();
    manifest.push_str(&zip_reader.read_entry_to_string(manifest_idx).await?);

    let pack: PackFormat = serde_json::from_str(&manifest)?;
    if &*pack.game != "minecraft" {
        return Err(crate::ErrorKind::InputError(
            "Pack does not support Minecraft".to_string(),
        )
        .into());
    }

    reporter
        .update(InstallPhaseId::ResolvingPack, None, modpack_details.clone())
        .await?;

    if !icon_exists {
        let icon_entry =
            zip_reader.file().entries().iter().enumerate().find_map(
                |(index, entry)| {
                    let Ok(name) = entry.filename().as_str() else {
                        return None;
                    };
                    (name == format!("{archive_base}icon.png")
                        || name == format!("{overrides_prefix}icon.png")
                        || name == format!("{client_overrides_prefix}icon.png"))
                    .then_some(index)
                },
            );

        if let Some(icon_entry) = icon_entry {
            let icon_path = instance_full_path.join("icon.png");
            zip_reader
                .extract_entry(
                    icon_entry,
                    &icon_path,
                    &state.io_semaphore,
                    None,
                )
                .await?;
            crate::api::instance::edit_icon(&instance_id, Some(&icon_path))
                .await?;
            icon_exists = true;
        }
    }

    set_instance_information(
        instance_id.clone(),
        &description,
        &pack.name,
        Some(&pack.version_id),
        &pack.dependencies,
        ignore_lock,
    )
    .await?;

    let metadata =
        crate::api::instance::get(&instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let download_meta = DownloadMeta {
        reason,
        game_version: metadata.applied_content_set.game_version.clone(),
        loader: metadata.applied_content_set.loader.as_str().to_string(),
        dependent_on: version_id.clone(),
    };

    let num_files = pack.files.len();
    let content_total_bytes =
        checked_download_size(pack.files.iter().map(|file| file.file_size))?;
    let mut cached_pack_hashes = pack
        .files
        .iter()
        .filter_map(|file| file.hashes.get(&PackFileHash::Sha1).cloned())
        .collect::<Vec<_>>();
    let cached_pack_project_ids = pack
        .files
        .iter()
        .filter_map(|file| {
            file.downloads.iter().find_map(|url| {
                let parts = url.split('/').collect::<Vec<_>>();
                let data_index =
                    parts.iter().position(|part| *part == "data")?;
                parts.get(data_index + 1).map(|id| (*id).to_string())
            })
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let content_progress = Arc::new(AtomicU64::new(0));
    let content_bytes_progress = Arc::new(AtomicU64::new(0));
    let file_info_hashes = Arc::new(
        pack.files
            .iter()
            .filter_map(|file| file.hashes.get(&PackFileHash::Sha1).cloned())
            .collect::<Vec<_>>(),
    );
    let file_infos_by_hash =
        Arc::new(Mutex::new(None::<HashMap<String, ModrinthHashMatch>>));
    let file_infos_loading = Arc::new(Mutex::new(()));
    let chinese_naming_enabled =
        Settings::get(&state.pool).await?.locale == "zh-CN";
    let chinese_titles_by_sha1 = Arc::new(if chinese_naming_enabled {
        // Chinese titles shape the on-disk file names, so they must be known
        // before any download starts. All other installs resolve them lazily
        // on the first completed file.
        load_modpack_file_infos(
            &file_infos_by_hash,
            &file_infos_loading,
            &file_info_hashes,
            &state.pool,
            &state.api_semaphore,
        )
        .await;
        let infos = file_infos_by_hash.lock().await;
        let infos = infos.as_ref().expect("file metadata was just loaded");
        resolve_chinese_titles_by_sha1(infos, state).await
    } else {
        HashMap::new()
    });
    // Resolve and cache the shared hash metadata before the Minecraft install
    // and content database writes begin competing for SQLite's single writer.
    load_modpack_file_infos(
        &file_infos_by_hash,
        &file_infos_loading,
        &file_info_hashes,
        &state.pool,
        &state.api_semaphore,
    )
    .await;
    let existing_paths_by_original = Arc::new(
        content_rows::get_instance_files(&instance_id, &state.pool)
            .await?
            .into_iter()
            .map(|file| {
                (
                    original_content_relative_path(&file.relative_path),
                    file.relative_path,
                )
            })
            .collect::<HashMap<_, _>>(),
    );
    let content_context = ModpackContentInstallContext {
        instance_id: instance_id.clone(),
        instance_full_path: instance_full_path.clone(),
        download_meta,
        pack_version_id: version_id.clone(),
        pack_project_id: project_id.clone(),
        reporter: reporter.clone().with_pipeline_cancellation(),
        modpack_details: modpack_details.clone(),
        content_progress,
        content_bytes_progress,
        download_source: Arc::new(Mutex::new(None)),
        fallback_count: Arc::new(AtomicU64::new(0)),
        file_infos_by_hash,
        chinese_titles_by_sha1,
        existing_paths_by_original,
        num_files,
        content_total_bytes,
    };
    for file in &pack.files {
        io::join_within_root(
            &content_context.instance_full_path,
            &content_context.resolve_install_path(file),
        )?;
    }
    let current_job_state = reporter.current_state().await?;
    let skipped_missing_content_paths = Arc::new(
        current_job_state
            .skipped_missing_content_paths
            .clone()
            .into_iter()
            .collect::<HashSet<_>>(),
    );
    let skipped_optional_content_paths = Arc::new(
        current_job_state
            .skipped_optional_content_paths
            .clone()
            .into_iter()
            .collect::<HashSet<_>>(),
    );
    let optional_content_decided = current_job_state.optional_content_decided;
    let mut queued_events = Vec::with_capacity(num_files.saturating_add(1));
    queued_events.push(InstallJobEventKind::ContentDownloadStarted {
        files: num_files as u64,
        bytes: (content_total_bytes > 0).then_some(content_total_bytes),
    });
    queued_events.extend(pack.files.iter().map(|file| {
        InstallJobEventKind::ContentFileQueued {
            path: content_context.resolve_install_path(file),
            bytes_total: (file.file_size > 0).then_some(file.file_size),
            max_attempts: estimated_file_max_attempts(file) as u32,
        }
    }));
    queued_events.extend(pack.files.iter().filter_map(|file| {
        let urls = crate::install::missing_content::browser_download_urls(
            &file.downloads,
        );
        (!urls.is_empty()).then(|| {
            InstallJobEventKind::ContentFileBrowserOptions {
                path: content_context.resolve_install_path(file),
                urls,
            }
        })
    }));
    reporter
        .update_with_events(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: num_files as u64,
                secondary: (content_total_bytes > 0).then_some(
                    InstallProgressSecondary {
                        current: 0,
                        total: content_total_bytes,
                    },
                ),
            }),
            modpack_details.clone(),
            queued_events,
        )
        .await?;
    reporter
        .set_missing_content(Some(MissingModpackContentState {
            files: pack
                .files
                .iter()
                .map(|file| {
                    let target_path =
                        content_context.resolve_install_path(file);
                    MissingModpackFileState {
                        item_id: target_path.clone(),
                        manifest_path: file.path.as_str().to_string(),
                        target_path,
                        expected_size: file.file_size,
                        sha1: file.hashes.get(&PackFileHash::Sha1).cloned(),
                        sha512: file
                            .hashes
                            .get(&PackFileHash::Sha512)
                            .cloned(),
                        download_urls: file.downloads.clone(),
                        browser_urls: crate::install::missing_content::browser_download_urls(
                            &file.downloads,
                        ),
                        validate_as_jar: file
                            .path
                            .as_str()
                            .to_ascii_lowercase()
                            .ends_with(".jar"),
                    }
                })
                .collect(),
        }))
        .await?;
    let optional_files: Vec<OptionalModpackFileState> = pack
        .files
        .iter()
        .filter(|file| {
            file.env.as_ref().is_some_and(|env| {
                env.get(&EnvType::Client) == Some(&SideType::Optional)
            })
        })
        .map(|file| {
            let target_path = content_context.resolve_install_path(file);
            OptionalModpackFileState {
                item_id: target_path.clone(),
                manifest_path: file.path.as_str().to_string(),
                target_path,
                expected_size: file.file_size,
                sha1: file.hashes.get(&PackFileHash::Sha1).cloned(),
                title: None,
                source: None,
                recommended: true,
            }
        })
        .collect();
    if !optional_files.is_empty() && !optional_content_decided {
        let optional_files_count = optional_files.len() as u64;
        reporter
            .set_optional_content(Some(OptionalModpackContentState {
                files: optional_files,
            }))
            .await?;
        return Ok(MrpackInstallOutcome::WaitingForUser(
            InstallPauseReason::OptionalContentSelection {
                optional_files: optional_files_count,
            },
        ));
    }
    reporter
        .track_rollback_paths(
            pack.files
                .iter()
                .map(|file| content_context.resolve_install_path(file))
                .collect(),
        )
        .await?;
    // Prewarm exactly the hosts this pack can use. Do not introduce a mirror
    // route or rewrite an official CDN host merely for DNS cache warming.
    let dns_hosts = pack
        .files
        .iter()
        .flat_map(|file| file.downloads.iter())
        .filter_map(|url| url::Url::parse(url).ok())
        .filter_map(|url| url.host_str().map(str::to_string))
        .collect::<HashSet<_>>();
    let dns_hosts = dns_hosts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::util::fetch::prewarm_download_dns(&dns_hosts).await;
    // Start the Minecraft core install concurrently with the content
    // download so both progress bars advance at once. The Minecraft install
    // reports on the parallel track (phase + bytes); content stays the main
    // phase. It is aborted if the content flow pauses for the user or fails.
    let minecraft_install =
        ParallelMinecraftInstall::start(instance_id.clone(), reporter.clone());
    let pack_files = pack.files;
    let mut retry_indices = (0..pack_files.len()).collect::<Vec<_>>();
    let mut required_file_failures = Vec::new();
    for pass in 0..=AUTO_RETRY_PASSES {
        if retry_indices.is_empty() {
            break;
        }
        let tasks = retry_indices
            .iter()
            .map(|&index| (index, pack_files[index].clone()))
            .collect::<Vec<_>>();
        let finalize_semaphore =
            Arc::new(Semaphore::new(NATIVE_CONTENT_FINALIZE_CONCURRENCY));
        let (completion_tx, mut completion_rx) =
            mpsc::channel::<MrpackDatabaseTask>(128);
        let completion_instance_id = content_context.instance_id.clone();
        let completion_context = content_context.clone();
        let completion_worker = tokio::spawn(async move {
            let cancellation = completion_context.reporter.cancellation_token();
            let result: crate::Result<()> = async {
                while let Some(batch) = receive_modpack_database_batch(
                    &mut completion_rx,
                    &cancellation,
                )
                .await?
                {
                    if let Err(error) = persist_modpack_record_batch(
                        &completion_instance_id,
                        &batch,
                        &cancellation,
                    )
                    .await
                    {
                        restore_mrpack_materializations(
                            &completion_instance_id,
                            &batch,
                        )
                        .await;
                        return Err(error);
                    }
                    // The database transaction is the commit point. Once it
                    // succeeds, the newly materialized files must remain even
                    // if cancellation arrives before progress is reported.
                    finalize_mrpack_materializations(&batch).await;
                    if cancellation.is_cancelled() {
                        return Err(crate::ErrorKind::OtherError(
                            "modpack database worker canceled".to_string(),
                        )
                        .into());
                    }
                    completion_context
                        .mark_files_settled(
                            batch.len() as u64,
                            batch.iter().map(|task| task.settled_bytes).sum(),
                            batch
                                .iter()
                                .map(|task| task.event.clone())
                                .collect(),
                        )
                        .await?;
                }
                Ok(())
            }
            .await;
            if result.is_err() {
                while let Ok(task) = completion_rx.try_recv() {
                    restore_mrpack_materialization(
                        &completion_instance_id,
                        &task.target_path,
                        task.previous_path.as_deref(),
                    )
                    .await;
                }
                if let Err(error) = &result {
                    completion_context.reporter.fail_pipeline(error);
                }
                cancellation.cancel();
            }
            result
        });
        let (verification_tx, mut verification_rx) =
            mpsc::channel::<MrpackVerificationTask>(128);
        let verification_context = content_context.clone();
        let verification_completion_tx = completion_tx.clone();
        let verification_worker = tokio::spawn(async move {
            let cancellation =
                verification_context.reporter.cancellation_token();
            let worker_context = verification_context.clone();
            let worker_completion_tx = verification_completion_tx.clone();
            let result = crate::install::try_for_each_concurrent_draining(
                futures::stream::poll_fn(move |cx| {
                    verification_rx.poll_recv(cx)
                }),
                Some(NATIVE_CONTENT_FINALIZE_CONCURRENCY),
                cancellation.clone(),
                move |task| {
                    let verification_context = worker_context.clone();
                    let verification_completion_tx =
                        worker_completion_tx.clone();
                    async move {
                let result: crate::Result<()> = async {
                    let cancellation = verification_context.reporter.cancellation_token();
                    if cancellation.is_cancelled() {
                        remove_mrpack_staged_file(&task.download_path).await;
                        return Err(crate::ErrorKind::OtherError(
                            "modpack verification canceled".to_string(),
                        ).into());
                    }
                    let finalize_permit = if let Some(semaphore) = task.finalize_semaphore.clone() {
                        Some(tokio::select! {
                            _ = cancellation.cancelled() => {
                                remove_mrpack_staged_file(&task.download_path).await;
                                return Err(crate::ErrorKind::OtherError(
                                    "modpack finalization canceled while waiting for worker".to_string(),
                                ).into());
                            }
                            permit = tokio::time::timeout(
                                FINALIZE_WAIT_TIMEOUT,
                                semaphore.acquire_owned(),
                            ) => permit.map_err(|_| crate::ErrorKind::NetworkError(
                                "timed out waiting for modpack finalization worker".to_string(),
                            ))??,
                        })
                    } else {
                        None
                    };
                    verification_context
                        .reporter
                        .record_download_stage(
                            task.project_path.clone(),
                            DownloadItemStatus::Finalizing,
                        )
                        .await?;
                    verification_context
                        .reporter
                        .record_download_stage(
                            task.project_path.clone(),
                            DownloadItemStatus::Metadata,
                        )
                        .await?;
                    let sha1 = if let Some(hash) = task.project.hashes.get(&PackFileHash::Sha1) {
                        hash.clone()
                    } else {
                        crate::util::fetch::sha1_file_cancellable(
                            &task.download_path,
                            &cancellation,
                        )
                        .await?.1
                    };
                    let file_info = verification_context
                        .file_infos_by_hash
                        .lock()
                        .await
                        .as_ref()
                        .and_then(|infos| infos.get(&sha1))
                        .cloned();
                    drop(finalize_permit);
                    let provider_ref = file_info
                        .as_ref()
                        .map(|file| {
                            crate::Result::Ok(ContentProviderRef::Modrinth {
                                project_id: ModrinthProjectId::new(file.project_id.clone())?,
                                version_id: Some(ModrinthVersionId::new(file.version_id.clone())?),
                            })
                        })
                        .transpose()?;
                    let record = ProjectType::get_from_parent_folder(task.project.path.as_str())
                        .map(|project_type| crate::state::instances::commands::ProjectFileRecord {
                            relative_path: task.project_path.clone(),
                            sha1,
                            size: task.downloaded_bytes,
                            project_type,
                            source_kind: modpack_source_kind(
                                verification_context.pack_version_id.as_deref(),
                            ),
                            ownership_kind: crate::state::instances::ContentOwnershipKind::PackManaged,
                            provider_ref,
                            origin: false,
                            known_modrinth_project_id: file_info.as_ref().map(|file| file.project_id.clone()),
                            known_modrinth_version_id: file_info.as_ref().map(|file| file.version_id.clone()),
                        });
                    let event = if task.attempts == 0 {
                        InstallJobEventKind::ContentFileRecovered {
                            path: task.project_path.clone(),
                            bytes: task.downloaded_bytes,
                        }
                    } else {
                        InstallJobEventKind::ContentFileCompleted {
                            path: task.project_path.clone(),
                            bytes: task.downloaded_bytes,
                        }
                    };
                    let state = State::get().await?;
                    let instance_lock = tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => {
                            remove_mrpack_staged_file(&task.download_path).await;
                            return Err(crate::ErrorKind::OtherError(
                                "modpack materialization canceled".to_string(),
                            ).into());
                        }
                        lock = state.lock_instance_content(&verification_context.instance_id) => lock,
                    };
                    let previous_path = match crate::state::materialize_project_download(
                        &task.download_path,
                        &task.target_path,
                    )
                    .await
                    {
                        Ok(previous_path) => previous_path,
                        Err(error) => {
                            drop(instance_lock);
                            remove_mrpack_staged_file(&task.download_path).await;
                            return Err(error);
                        }
                    };
                    drop(instance_lock);
                    remove_mrpack_staged_file(&task.download_path).await;
                    if let Some(record) = record {
                        let database_task = MrpackDatabaseTask {
                            record,
                            settled_bytes: task.downloaded_bytes,
                            event,
                            target_path: task.target_path.clone(),
                            previous_path,
                        };
                        let restore_target_path = database_task.target_path.clone();
                        let restore_previous_path = database_task.previous_path.clone();
                        let send_result = tokio::select! {
                            _ = cancellation.cancelled() => {
                                Err(crate::ErrorKind::OtherError(
                                    "modpack database registration canceled".to_string(),
                                ).into())
                            }
                            result = verification_completion_tx.send(database_task) => result.map_err(|_| crate::ErrorKind::OtherError(
                                "modpack database worker stopped".to_string(),
                            ).into()),
                        };
                        if let Err(error) = send_result {
                            restore_mrpack_materialization(
                                &verification_context.instance_id,
                                &restore_target_path,
                                restore_previous_path.as_deref(),
                            )
                            .await;
                            return Err(error);
                        }
                    } else {
                        let report_result = verification_context
                            .mark_file_settled(task.downloaded_bytes, event)
                            .await;
                        if let Err(error) = report_result {
                            restore_mrpack_materialization(
                                &verification_context.instance_id,
                                &task.target_path,
                                previous_path.as_deref(),
                            )
                            .await;
                            return Err(error);
                        }
                        crate::state::finalize_project_materialization(
                            previous_path.as_deref(),
                        )
                        .await?;
                    }
                    Ok(())
                }.await;
                if let Err(error) = &result { verification_context.reporter.fail_pipeline(error); }
                result?;
                Ok(())
                    }
                },
            )
            .await;
            if result.is_err() {
                if let Err(error) = &result {
                    verification_context.reporter.fail_pipeline(error);
                }
                cancellation.cancel();
            }
            result
        });
        let pass_failures =
            collect_required_file_failures_concurrently(
        tasks,
        Some(state.download_concurrency()),
        |(manifest_index, project)| {
            let content_context = content_context.clone();
            let skipped_missing_content_paths =
                skipped_missing_content_paths.clone();
            let skipped_optional_content_paths =
                skipped_optional_content_paths.clone();
            let finalize_semaphore = finalize_semaphore.clone();
            let verification_tx = verification_tx.clone();
             async move {
                let project_size = project.file_size;
                let project_path =
                    content_context.resolve_install_path(&project);
                let target_path = io::join_within_root(
                    &content_context.instance_full_path,
                    &project_path,
                ).map_err(|error| RequiredFileFailure {
                    manifest_index,
                    path: project_path.clone(),
                    reason: error.to_string(),
                })?;
                let download_path =
                    mrpack_staged_download_path(&target_path);
                let context =
                    InstallErrorContext::new("download modpack content file")
                        .maybe_project_id(
                            content_context.pack_project_id.clone(),
                        )
                        .maybe_version_id(
                            content_context.pack_version_id.clone(),
                        )
                        .file_path(project_path.clone())
                        .target_path(target_path.display().to_string())
                        .urls(project.downloads.clone())
                        .maybe_expected_hash(
                            project.hashes.get(&PackFileHash::Sha1).cloned(),
                        )
                        .expected_size(project_size)
                        .build();
                let result: crate::Result<()> = async {

                content_context
                    .reporter
                    .record_download_stage(
                        project_path.clone(),
                        DownloadItemStatus::WorkerStarted,
                    )
                    .await?;

                if skipped_missing_content_paths.contains(&project_path) {
                    content_context
                        .mark_file_settled(
                            0,
                            InstallJobEventKind::ContentFileSkipped {
                                path: project_path.clone(),
                                reason: "skipped by user".to_string(),
                                project_id: None,
                                version_id: None,
                                manual_url: None,
                            },
                        )
                        .await?;
                    return Ok(());
                }

                if skipped_optional_content_paths.contains(&project_path) {
                    content_context
                        .mark_file_settled(
                            0,
                            InstallJobEventKind::ContentFileSkipped {
                                path: project_path.clone(),
                                reason: "optional file skipped by user".to_string(),
                                project_id: None,
                                version_id: None,
                                manual_url: None,
                            },
                        )
                        .await?;
                    return Ok(());
                }

                if let Some(env) = project.env.as_ref()
                    && env
                        .get(&EnvType::Client)
                        .is_some_and(|x| x == &SideType::Unsupported)
                {
                    content_context
                        .mark_file_settled(
                            0,
                            InstallJobEventKind::ContentFileSkipped {
                                path: project_path.clone(),
                                reason: "unsupported on client".to_string(),
                                project_id: None,
                                version_id: None,
                                manual_url: None,
                            },
                        )
                        .await?;
                    return Ok(());
                }

                content_context
                    .reporter
                    .set_transient_context(context.clone())
                    .await?;

                content_context
                    .report_download_attempt(
                        project_path.clone(),
                        project_size,
                        1,
                        estimated_file_max_attempts(&project),
                    )
                    .await?;
                if tokio::fs::try_exists(&target_path).await.unwrap_or(false) {
                    content_context
                        .reporter
                        .record_events(vec![
                            InstallJobEventKind::ContentFileVerificationStarted {
                                path: project_path.clone(),
                            },
                        ])
                        .await?;
                }
                let Some(primary_url) = project.downloads.first() else {
                    return Err(crate::ErrorKind::InputError(format!(
                        "Modpack file {} has no download URL",
                        project.path
                    ))
                    .into());
                };
                let integrity = Integrity {
                    size: Some(project_size),
                    sha1: project.hashes.get(&PackFileHash::Sha1).cloned(),
                    sha512: project
                        .hashes
                        .get(&PackFileHash::Sha512)
                        .cloned(),
                    content: if project
                        .path
                        .as_str()
                        .to_ascii_lowercase()
                        .ends_with(".jar")
                    {
                        ContentValidation::Jar
                    } else {
                        ContentValidation::None
                    },
                    ..Integrity::default()
                };
                // Only the transfer owns a network worker. Metadata and DB
                // finalization run in a separate bounded stage so a slow
                // SQLite write cannot stop subsequent file transfers.
                content_context
                    .reporter
                    .record_download_stage(
                        project_path.clone(),
                        DownloadItemStatus::Connecting,
                    )
                    .await?;
                // Seed the staging path with an existing installation when
                // possible. The downloader can then verify/reuse it without
                // ever deleting the live target when validation fails.
                seed_mrpack_staged_download(&target_path, &download_path).await;
                let download = match download_to_path(
                    DownloadRequest::new(primary_url, ResourceClass::Modpack)
                        .with_candidate_urls(
                            project.downloads.iter().skip(1).cloned(),
                        )
                        .with_integrity(integrity)
                        .with_download_meta(
                            content_context.download_meta.clone(),
                        )
                        .with_segmented_download(true)
                        .with_http1_segmented_download(false)
                        .with_install_tracking(
                            content_context.reporter.clone(),
                            project_path.clone(),
                            project_path.clone(),
                        ),
                    &download_path,
                    &state.download_semaphore,
                    &state.pool,
                    None,
                )
                .await
                {
                    Ok(download) => download,
                    Err(error) => return Err(error),
                };
                let downloaded_bytes = download.size;
                content_context.record_download_result(&download).await;
                let verification_task = MrpackVerificationTask {
                    project: project.clone(),
                    project_path: project_path.clone(),
                    download_path,
                    target_path,
                    downloaded_bytes,
                    attempts: download.attempts as u32,
                    finalize_semaphore: Some(Arc::clone(&finalize_semaphore)),
                };
                let enqueue_cancellation =
                    content_context.reporter.cancellation_token();
                tokio::select! {
                    biased;
                    _ = enqueue_cancellation.cancelled() => {
                        return Err(crate::ErrorKind::OtherError(
                            "modpack verification enqueue canceled".to_string(),
                        ).into());
                    }
                    result = verification_tx.send(verification_task) => {
                        result.map_err(|_| crate::ErrorKind::OtherError(
                            "modpack verification worker stopped".to_string(),
                        ))?;
                    }
                }
                Ok(())
                }
                .await;
                match result {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        if pass == AUTO_RETRY_PASSES {
                            content_context
                                .reporter
                                .persist_failure_context(context)
                                .await;
                            let failure = RequiredFileFailure::new(
                                manifest_index,
                                &project_path,
                                &error.to_string(),
                            );
                            tracing::warn!(
                                path = %project_path,
                                reason = %failure.reason,
                                "Failed to install required Modrinth pack file"
                            );
                            if let Err(report_error) = content_context
                                .mark_file_settled(
                                    0,
                                    InstallJobEventKind::ContentFileFailed {
                                        path: project_path.clone(),
                                        reason: failure.reason.clone(),
                                        project_id: None,
                                        version_id: None,
                                    },
                                )
                                .await
                            {
                                return Err(RequiredFileFailure::new(
                                    manifest_index,
                                    &project_path,
                                    &format!(
                                        "{}; failed to record item failure: {report_error}",
                                        failure.reason
                                    ),
                                ));
                            }
                            Err(failure)
                        } else {
                            tracing::warn!(
                                path = %project_path,
                                pass = pass + 1,
                                retries_left =
                                    AUTO_RETRY_PASSES.saturating_sub(pass),
                                reason = %error,
                                "Modpack file failed; scheduling an automatic retry"
                            );
                            Err(RequiredFileFailure::new(
                                manifest_index,
                                &project_path,
                                &error.to_string(),
                            ))
                        }
                    }
                }
            }
        },
    )
    .await?;
        drop(completion_tx);
        drop(verification_tx);
        let verification_result = verification_worker
            .await
            .map_err(|error| {
                crate::Error::from(crate::ErrorKind::OtherError(format!(
                    "modpack verification worker failed: {error}"
                )))
            })
            .and_then(|result| result);
        let completion_result = completion_worker
            .await
            .map_err(|error| {
                crate::Error::from(crate::ErrorKind::OtherError(format!(
                    "modpack database worker failed: {error}"
                )))
            })
            .and_then(|result| result);
        if let Some(error) = content_context.reporter.pipeline_failure() {
            return Err(error);
        }
        verification_result?;
        completion_result?;
        required_file_failures = pass_failures;
        if required_file_failures.is_empty() {
            break;
        }
        if pass < AUTO_RETRY_PASSES {
            tracing::warn!(
                files = required_file_failures.len(),
                pass = pass + 1,
                retries_left = AUTO_RETRY_PASSES.saturating_sub(pass),
                "Automatic retry pass for failed modpack files"
            );
        }
        retry_indices = required_file_failures
            .iter()
            .map(|failure| failure.manifest_index)
            .collect();
    }
    content_context.finish_download_metrics().await?;
    if let Some(reason) =
        missing_required_content_pause(&required_file_failures)
    {
        minecraft_install.abort().await;
        return Ok(MrpackInstallOutcome::WaitingForUser(reason));
    }

    let override_specs = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            let filename = file.filename().as_str().unwrap_or_default();
            ((filename.starts_with(&overrides_prefix)
                || filename.starts_with(&client_overrides_prefix))
                && !filename.ends_with('/'))
            .then(|| {
                let relative_path =
                    override_relative_path(filename, &archive_base)?;
                Ok(OverrideExtractionSpec {
                    index,
                    entry_name: filename.to_string(),
                    crc32: file.crc32(),
                    uncompressed_size: file.uncompressed_size(),
                    target_path: io::safe_relative_join(
                        &instance_full_path,
                        &relative_path,
                    )?,
                    relative_path,
                })
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let override_total_bytes = override_specs
        .iter()
        .map(|spec| zip_reader.file().entries()[spec.index].uncompressed_size())
        .sum::<u64>();
    reporter
        .track_rollback_paths(
            override_specs
                .iter()
                .map(|spec| spec.relative_path.clone())
                .collect(),
        )
        .await?;
    let progress = (override_total_bytes > 0).then_some(InstallProgress {
        current: 0,
        total: override_total_bytes,
        secondary: None,
    });
    reporter
        .update(
            InstallPhaseId::ExtractingOverrides,
            progress,
            modpack_details.clone(),
        )
        .await?;

    let extracted_override_bytes = Arc::new(AtomicU64::new(0));
    let reported_override_bucket = Arc::new(AtomicU64::new(0));
    let override_progress_delta = (override_total_bytes / 200).max(256 * 1024);
    let mut override_parents = override_specs
        .iter()
        .filter_map(|spec| spec.target_path.parent().map(Path::to_path_buf))
        .collect::<HashSet<_>>();
    for parent in override_parents.drain() {
        io::create_dir_all(&parent).await?;
    }
    let mut seen_override_targets = HashSet::new();
    let override_targets = override_specs
        .iter()
        .filter(|&spec| seen_override_targets.insert(spec.target_path.clone()))
        .map(|spec| spec.target_path.clone())
        .collect::<Vec<_>>();
    let override_groups = Arc::new(Mutex::new(VecDeque::from(
        override_extraction_groups(override_specs),
    )));
    let extraction_result =
        futures::stream::iter(0..OVERRIDE_EXTRACTION_CONCURRENCY)
            .map(|_| {
                let file = file.clone();
                let reporter = reporter.clone();
                let project_id = project_id.clone();
                let version_id = version_id.clone();
                let source_path = source_path.clone();
                let modpack_details = modpack_details.clone();
                let extracted_override_bytes =
                    Arc::clone(&extracted_override_bytes);
                let reported_override_bucket =
                    Arc::clone(&reported_override_bucket);
                let override_groups = Arc::clone(&override_groups);
                async move {
                    let mut extracted = Vec::new();
                    let mut reader = MrpackZipReader::new(&file).await?;
                    loop {
                        let Some(group) =
                            override_groups.lock().await.pop_front()
                        else {
                            break;
                        };
                        for spec in group {
                            let override_context = InstallErrorContext::new(
                                "extract modpack override",
                            )
                            .maybe_project_id(project_id.clone())
                            .maybe_version_id(version_id.clone())
                            .source_path(source_path.clone())
                            .entry_path(spec.entry_name.clone())
                            .target_path(spec.target_path.display().to_string())
                            .build();
                            reporter
                                .set_transient_context(override_context.clone())
                                .await?;
                            let mut report_progress =
                                |bytes_read: u64| -> Pin<
                                    Box<
                                        dyn Future<Output = crate::Result<()>>
                                            + Send,
                                    >,
                                > {
                                    let current =
                                        extracted_override_bytes.fetch_add(
                                            bytes_read,
                                            Ordering::Relaxed,
                                        ) + bytes_read;
                                    let bucket =
                                        current / override_progress_delta;
                                    let previous = reported_override_bucket
                                        .fetch_max(bucket, Ordering::Relaxed);
                                    if current < override_total_bytes
                                        && bucket <= previous
                                    {
                                        return Box::pin(async { Ok(()) });
                                    }
                                    let reporter = reporter.clone();
                                    let details = modpack_details.clone();
                                    Box::pin(async move {
                                        reporter
                                    .update(
                                        InstallPhaseId::ExtractingOverrides,
                                        Some(InstallProgress {
                                            current: current
                                                .min(override_total_bytes),
                                            total: override_total_bytes,
                                            secondary: None,
                                        }),
                                        details,
                                    )
                                    .await
                                    })
                                };
                            let progress = (override_total_bytes > 0)
                                .then_some(
                                    &mut report_progress
                                        as &mut ExtractProgressFn<'_>,
                                );
                            let staging_path =
                                mrpack_override_staging_path(&spec.target_path);
                            let extract_result = reader
                                .extract_override_entry(
                                    &spec,
                                    &staging_path,
                                    &state.io_semaphore,
                                    progress,
                                )
                                .await;
                            let (size, hash) = reporter
                                .preserve_failure_context(
                                    override_context,
                                    extract_result,
                                )
                                .await?;
                            extracted.push(ExtractedOverride {
                                spec,
                                size,
                                hash,
                            });
                        }
                    }
                    Ok::<_, crate::Error>(extracted)
                }
            })
            .buffer_unordered(OVERRIDE_EXTRACTION_CONCURRENCY)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<crate::Result<Vec<_>>>();
    let mut extracted_overrides = match extraction_result {
        Ok(extracted) => extracted.into_iter().flatten().collect::<Vec<_>>(),
        Err(error) => {
            let cleanup_targets = override_targets.clone();
            let cleanup_result = tokio::task::spawn_blocking(move || {
                crate::api::pack::archive_util::discard_staged_archive_entries(
                    &cleanup_targets,
                )
            })
            .await?;
            if let Err(cleanup_error) = cleanup_result {
                return Err(crate::ErrorKind::OtherError(format!(
                    "{error}; failed to clean staged MRPack overrides: {cleanup_error}"
                ))
                .into());
            }
            return Err(error);
        }
    };
    let materialization_result =
		crate::api::pack::archive_util::run_cancellable_blocking_instance_write(
        instance_id.clone(),
        reporter.cancellation_token(),
        {
            let override_targets = override_targets.clone();
            move |cancellation| {
                crate::api::pack::archive_util::materialize_staged_archive_entries(
                    &override_targets,
                    Some(cancellation),
                )
            }
        },
    )
	.await;
    let override_replacements = match materialization_result {
        Ok(replacements) => replacements,
        Err(error) => {
            let cleanup_targets = override_targets.clone();
            let cleanup_result = tokio::task::spawn_blocking(move || {
                crate::api::pack::archive_util::discard_staged_archive_entries(
                    &cleanup_targets,
                )
            })
            .await?;
            if let Err(cleanup_error) = cleanup_result {
                return Err(crate::ErrorKind::OtherError(format!("{error}; failed to clean staged MRPack overrides: {cleanup_error}")).into());
            }
            return Err(error);
        }
    };
    extracted_overrides.sort_unstable_by_key(|extracted| extracted.spec.index);

    let mut override_records = Vec::new();
    for extracted in extracted_overrides {
        cached_pack_hashes.push(extracted.hash.clone());
        if let Some(project_type) =
            ProjectType::get_from_parent_folder(&extracted.spec.relative_path)
        {
            override_records
                .push(crate::state::instances::commands::ProjectFileRecord {
                relative_path: extracted.spec.relative_path,
                sha1: extracted.hash,
                size: extracted.size,
                project_type,
                source_kind: modpack_source_kind(version_id.as_deref()),
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                provider_ref: None,
                origin: false,
                known_modrinth_project_id: None,
                known_modrinth_version_id: None,
            });
        }
    }
    let override_registration_result: crate::Result<()> = async {
        if override_records.is_empty() {
            return Ok(());
        }
        let cancellation = reporter.cancellation_token();
        if cancellation.is_cancelled() {
            return Err(crate::ErrorKind::OtherError(
                "modpack override registration canceled".to_string(),
            )
            .into());
        }
        let record_context =
            InstallErrorContext::new("record modpack overrides")
                .maybe_project_id(project_id.clone())
                .maybe_version_id(version_id.clone())
                .source_path(source_path.clone())
                .build();
        // Do not race an in-flight SQLite transaction against cancellation.
        // The permit wait above is cancelable; after the write begins its
        // outcome must be observed. Release the permit before persisting a
        // failure context because the install-job store uses the same
        // semaphore and would otherwise deadlock on an error.
        let database_permit = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(crate::ErrorKind::OtherError(
                    "modpack override registration canceled".to_string(),
                ).into());
            }
            permit = state.acquire_install_db_permit() => permit?,
        };
        let record_result =
            crate::state::instances::commands::record_project_files_atomic_with_permit(
                &instance_id,
                &override_records,
                &[],
                Some(database_permit),
                state,
            )
            .await;
        reporter
            .preserve_failure_context(record_context, record_result)
            .await?;
        Ok(())
    }
    .await;
    let replacement_result = match override_registration_result {
        Ok(()) => {
            crate::api::pack::archive_util::run_blocking_instance_write(
                instance_id.clone(),
                reporter.cancellation_token(),
                move |_| override_replacements.finalize(),
            )
            .await
        }
        Err(error) => {
            let rollback_result =
                crate::api::pack::archive_util::run_blocking_instance_write(
                    instance_id.clone(),
                    reporter.cancellation_token(),
                    move |_| override_replacements.rollback(),
                )
                .await;
            if let Err(rollback_error) = rollback_result {
                return Err(crate::ErrorKind::OtherError(format!(
                    "{error}; failed to restore MRPack overrides: {rollback_error}"
                ))
                .into());
            }
            return Err(error);
        }
    };
    replacement_result?;

    if let Some(ref version_id) = version_id {
        let server_override_entries = zip_reader
            .file()
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let filename = entry.filename().as_str().ok()?;
                (filename.starts_with(&server_overrides_prefix)
                    && !filename.ends_with('/'))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        for index in server_override_entries {
            cached_pack_hashes.push(zip_reader.hash_entry(index).await?.1);
        }
        CachedEntry::cache_modpack_files_best_effort(
            version_id,
            cached_pack_hashes,
            cached_pack_project_ids,
            &state.pool,
        )
        .await;
    } else {
        tracing::warn!(
            "No version_id available, skipping modpack file hash caching"
        );
    }

    // If the icon doesn't exist, we expect icon.png to be a potential icon.
    // If it doesn't exist, and an override to icon.png exists, cache and use that
    let potential_icon = instance_full_path.join("icon.png");
    if !icon_exists && potential_icon.exists() {
        crate::api::instance::edit_icon(&instance_id, Some(&potential_icon))
            .await?;
    }

    // Join the parallel Minecraft core install: if content finished first,
    // this waits for the Minecraft download to complete so the instance is
    // fully installed; if Minecraft finished first, this returns at once.
    minecraft_install.join().await?;
    reporter.clear_context().await?;

    Ok(MrpackInstallOutcome::Completed(instance_id.clone()))
}

fn pack_source_path(file: &CreatePackFile) -> String {
    match file {
        CreatePackFile::Bytes(_) => "downloaded mrpack bytes".to_string(),
        CreatePackFile::Path(path) => path.display().to_string(),
    }
}

/// Finds the `modrinth.index.json` entry and the archive base folder it lives
/// in. The exact root entry is preferred; a manifest one wrapping folder deep
/// is accepted as a fallback.
fn locate_mrpack_manifest(
    zip_file: &async_zip::ZipFile,
) -> Option<(usize, String)> {
    let entries = zip_file.entries();
    entries
        .iter()
        .position(|f| {
            matches!(f.filename().as_str(), Ok("modrinth.index.json"))
        })
        .map(|index| (index, String::new()))
        .or_else(|| {
            entries.iter().enumerate().find_map(|(index, f)| {
                let name = f.filename().as_str().ok()?;
                let (base, rest) = name.split_once('/')?;
                (rest == "modrinth.index.json")
                    .then(|| (index, format!("{base}/")))
            })
        })
}

fn override_relative_path(
    entry_name: &str,
    archive_base: &str,
) -> crate::Result<String> {
    let entry_name =
        entry_name.strip_prefix(archive_base).unwrap_or(entry_name);
    let relative =
        SafeRelativeUtf8UnixPathBuf::try_from(entry_name.to_string())?;
    let relative = relative
        .strip_prefix("overrides")
        .or_else(|_| relative.strip_prefix("client-overrides"))
        .map_err(|_| {
            crate::ErrorKind::InputError(format!(
                "Invalid modpack override path: {relative}"
            ))
        })?;
    io::safe_relative_join(Path::new(""), relative.as_str())?;
    Ok(relative.as_str().to_string())
}

pub(crate) async fn related_file_paths(
    mrpack_file: &CreatePackFile,
) -> crate::Result<Vec<String>> {
    let mut zip_reader = MrpackZipReader::new(mrpack_file).await?;
    let Some((manifest_idx, archive_base)) =
        locate_mrpack_manifest(zip_reader.file())
    else {
        return Err(crate::ErrorKind::InputError(
            "No pack manifest found in mrpack".to_string(),
        )
        .into());
    };
    let manifest = zip_reader.read_entry_to_string(manifest_idx).await?;
    let pack: PackFormat = serde_json::from_str(&manifest)?;
    let overrides_prefix = format!("{archive_base}overrides/");
    let client_overrides_prefix = format!("{archive_base}client-overrides/");
    let mut paths = pack
        .files
        .into_iter()
        .map(|file| file.path.as_str().to_string())
        .collect::<Vec<_>>();
    for entry in zip_reader.file().entries() {
        let name = entry.filename().as_str().unwrap_or_default();
        if (name.starts_with(&overrides_prefix)
            || name.starts_with(&client_overrides_prefix))
            && !name.ends_with('/')
        {
            paths.push(override_relative_path(name, &archive_base)?);
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn modpack_source_kind(version_id: Option<&str>) -> ContentSourceKind {
    if version_id.is_some() {
        ContentSourceKind::ModrinthModpack
    } else {
        ContentSourceKind::ImportedModpack
    }
}

fn checked_download_size(
    sizes: impl IntoIterator<Item = u64>,
) -> crate::Result<u64> {
    sizes.into_iter().try_fold(0u64, |total, size| {
        total.checked_add(size).ok_or_else(|| {
            crate::ErrorKind::InputError(
                "pack download size exceeds supported range".into(),
            )
            .into()
        })
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_download_size_rejects_overflow() {
        assert!(super::checked_download_size([u64::MAX, 1]).is_err());
        assert_eq!(super::checked_download_size([10, 20]).unwrap(), 30);
    }

    use super::*;
    use async_zip::tokio::write::ZipFileWriter;
    use async_zip::{Compression, ZipEntryBuilder};
    use std::sync::Mutex as StdMutex;

    #[test]
    fn override_paths_reject_portable_traversal_and_windows_aliases() {
        for prefix in ["overrides/", "client-overrides/"] {
            assert_eq!(
                override_relative_path(
                    &format!("{prefix}config/example.toml"),
                    ""
                )
                .unwrap(),
                "config/example.toml"
            );
            for relative in [
                "../outside",
                "config/.. /outside",
                "config/file:stream",
                "C:/outside",
                "C:outside",
                "config/NUL",
                "config/file.",
            ] {
                assert!(
                    override_relative_path(&format!("{prefix}{relative}"), "")
                        .is_err(),
                    "{prefix}{relative}"
                );
            }
        }
    }

    fn override_spec(index: usize, target: &str) -> OverrideExtractionSpec {
        OverrideExtractionSpec {
            index,
            entry_name: format!("overrides/{target}"),
            crc32: 0,
            uncompressed_size: 0,
            relative_path: target.to_string(),
            target_path: PathBuf::from(target),
        }
    }

    #[test]
    fn override_batches_bound_concurrency_and_order_duplicate_targets() {
        let groups = override_extraction_groups(vec![
            override_spec(0, "config/a.toml"),
            override_spec(1, "config/b.toml"),
            override_spec(2, "config/c.toml"),
            override_spec(3, "config/d.toml"),
            override_spec(4, "config/a.toml"),
        ]);

        assert!(groups.iter().all(|group| {
            group
                .windows(2)
                .all(|pair| pair[0].target_path == pair[1].target_path)
        }));
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0][0].index, 0);
        assert_eq!(groups[0][1].index, 4);
    }

    #[tokio::test]
    async fn mrpack_materialization_restores_existing_file_on_failure() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("mods/example.jar");
        let staged = mrpack_staged_download_path(&target);
        tokio::fs::create_dir_all(target.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&target, b"previous").await.unwrap();
        tokio::fs::write(&staged, b"downloaded").await.unwrap();

        let previous =
            crate::state::materialize_project_download(&staged, &target)
                .await
                .unwrap();
        remove_mrpack_staged_file(&staged).await;
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"downloaded");

        crate::state::restore_project_materialization(
            &target,
            previous.as_deref(),
        )
        .await
        .unwrap();

        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"previous");
        assert!(!staged.exists());
        assert!(previous.is_some_and(|path| !path.exists()));
    }

    #[tokio::test]
    async fn mrpack_materialization_keeps_new_file_after_commit() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("mods/example.jar");
        let staged = mrpack_staged_download_path(&target);
        tokio::fs::create_dir_all(target.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&target, b"previous").await.unwrap();
        tokio::fs::write(&staged, b"downloaded").await.unwrap();

        let previous =
            crate::state::materialize_project_download(&staged, &target)
                .await
                .unwrap();
        remove_mrpack_staged_file(&staged).await;
        crate::state::finalize_project_materialization(previous.as_deref())
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"downloaded");
        assert!(previous.is_some_and(|path| !path.exists()));
        assert!(!staged.exists());
    }

    #[tokio::test]
    async fn modpack_database_batch_flushes_when_channel_closes() {
        let (sender, mut receiver) = mpsc::channel(4);
        sender.send(1_u8).await.unwrap();
        sender.send(2_u8).await.unwrap();
        drop(sender);

        let batch = receive_modpack_database_batch(
            &mut receiver,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(batch, vec![1, 2]);
    }

    #[tokio::test]
    async fn modpack_database_batch_flushes_at_size_limit() {
        let (sender, mut receiver) = mpsc::channel(CONTENT_DATABASE_BATCH_SIZE);
        for index in 0..CONTENT_DATABASE_BATCH_SIZE {
            sender.send(index).await.unwrap();
        }

        let batch = tokio::time::timeout(
            Duration::from_millis(100),
            receive_modpack_database_batch(
                &mut receiver,
                &tokio_util::sync::CancellationToken::new(),
            ),
        )
        .await
        .expect("a full database batch should flush immediately")
        .unwrap()
        .unwrap();

        assert_eq!(batch.len(), CONTENT_DATABASE_BATCH_SIZE);
    }

    #[tokio::test]
    async fn modpack_database_batch_wait_is_cancelable() {
        let (_sender, mut receiver) = mpsc::channel::<u8>(1);
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();

        let error = tokio::time::timeout(
            Duration::from_millis(100),
            receive_modpack_database_batch(&mut receiver, &cancellation),
        )
        .await
        .expect("database queue cancellation should be immediate")
        .unwrap_err();

        assert!(error.to_string().contains("canceled"));
    }

    #[tokio::test]
    async fn partial_modpack_database_batch_flushes_on_interval() {
        let (sender, mut receiver) = mpsc::channel(2);
        // Keep the sender alive so only the flush interval can finish the
        // partial batch.
        sender.send(7_u8).await.unwrap();

        let batch = tokio::time::timeout(
            CONTENT_DATABASE_FLUSH_INTERVAL + Duration::from_millis(250),
            receive_modpack_database_batch(
                &mut receiver,
                &tokio_util::sync::CancellationToken::new(),
            ),
        )
        .await
        .expect("a partial database batch should flush on the interval")
        .unwrap()
        .unwrap();

        assert_eq!(batch, vec![7]);
    }

    async fn run_failure_collection(
        failed_indices: &[usize],
    ) -> (Vec<usize>, Vec<RequiredFileFailure>) {
        let attempted = Arc::new(StdMutex::new(Vec::new()));
        let failed_indices = Arc::new(failed_indices.to_vec());
        let failures = collect_required_file_failures_concurrently(
            (0..4).map(|index| (index, index)).collect(),
            Some(1),
            {
                let attempted = attempted.clone();
                move |(manifest_index, value)| {
                    let attempted = attempted.clone();
                    let failed_indices = failed_indices.clone();
                    async move {
                        attempted.lock().unwrap().push(value);
                        if failed_indices.contains(&value) {
                            Err(RequiredFileFailure::new(
                                manifest_index,
                                &format!("mods/{value}.jar"),
                                "test download failure",
                            ))
                        } else {
                            Ok(())
                        }
                    }
                }
            },
        )
        .await
        .unwrap();
        let attempted = attempted.lock().unwrap().clone();
        (attempted, failures)
    }

    #[tokio::test]
    async fn all_required_file_tasks_complete_without_failures() {
        let (attempted, failures) = run_failure_collection(&[]).await;

        assert_eq!(attempted, vec![0, 1, 2, 3]);
        assert!(failures.is_empty());
        assert!(missing_required_content_pause(&failures).is_none());
    }

    #[tokio::test]
    async fn first_required_file_failure_does_not_stop_later_tasks() {
        let (attempted, failures) = run_failure_collection(&[0]).await;

        assert_eq!(attempted, vec![0, 1, 2, 3]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].path, "mods/0.jar");
    }

    #[tokio::test]
    async fn middle_required_file_failure_does_not_stop_later_tasks() {
        let (attempted, failures) = run_failure_collection(&[1]).await;

        assert_eq!(attempted, vec![0, 1, 2, 3]);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].path, "mods/1.jar");
    }

    #[tokio::test]
    async fn multiple_required_file_failures_are_all_collected() {
        let (attempted, failures) = run_failure_collection(&[0, 2, 3]).await;

        assert_eq!(attempted, vec![0, 1, 2, 3]);
        assert_eq!(
            failures
                .iter()
                .map(|failure| failure.path.as_str())
                .collect::<Vec<_>>(),
            vec!["mods/0.jar", "mods/2.jar", "mods/3.jar"]
        );
        let summary = RequiredFileFailures(&failures).to_string();
        assert!(summary.contains("3 required modpack files failed"));
        assert!(summary.contains("mods/0.jar"));

        let pause = missing_required_content_pause(&failures).unwrap();
        assert_eq!(
            pause,
            InstallPauseReason::MissingRequiredContent {
                failed_files: 3,
                paths: vec![
                    "mods/0.jar".to_string(),
                    "mods/2.jar".to_string(),
                    "mods/3.jar".to_string(),
                ],
            }
        );
    }

    #[test]
    fn aggregate_required_file_error_has_bounded_path_details() {
        let failures = (0..10)
            .map(|index| {
                RequiredFileFailure::new(
                    index,
                    &format!("mods/{index}-{}.jar", "x".repeat(500)),
                    "failed",
                )
            })
            .collect::<Vec<_>>();

        let message = RequiredFileFailures(&failures).to_string();
        assert!(message.contains("10 required modpack files failed"));
        assert!(message.contains("(+7 more)"));
        assert!(message.len() < 600);
    }

    #[tokio::test]
    async fn local_mrpack_uses_the_file_backed_reader() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("local.mrpack");
        let mut file = tokio::fs::File::create(&path).await.unwrap();
        let mut writer = ZipFileWriter::with_tokio(&mut file);
        writer
            .write_entry_whole(
                ZipEntryBuilder::new(
                    "modrinth.index.json".to_string().into(),
                    Compression::Stored,
                ),
                br#"{"formatVersion":1,"game":"minecraft","versionId":"test","name":"test","files":[]}"#,
            )
            .await
            .unwrap();
        writer.close().await.unwrap();
        drop(file);

        let mut reader = MrpackZipReader::new(&CreatePackFile::Path(path))
            .await
            .unwrap();
        assert!(matches!(&reader, MrpackZipReader::File(_)));
        let index = reader
            .file()
            .entries()
            .iter()
            .position(|entry| {
                matches!(entry.filename().as_str(), Ok("modrinth.index.json"))
            })
            .unwrap();
        assert_eq!(
            reader.read_entry_to_string(index).await.unwrap(),
            r#"{"formatVersion":1,"game":"minecraft","versionId":"test","name":"test","files":[]}"#
        );
    }

    #[tokio::test]
    async fn parallel_override_readers_extract_the_original_entry_contents()
    -> crate::Result<()> {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("overrides.mrpack");
        let entries = [
            ("overrides/config/first.toml", b"first = true".as_slice()),
            ("overrides/config/second.toml", b"second = true".as_slice()),
            (
                "client-overrides/options.txt",
                b"fancyGraphics:true".as_slice(),
            ),
        ];
        let mut archive = tokio::fs::File::create(&path).await.unwrap();
        let mut writer = ZipFileWriter::with_tokio(&mut archive);
        for (name, contents) in entries {
            writer
                .write_entry_whole(
                    ZipEntryBuilder::new(
                        name.to_string().into(),
                        Compression::Deflate,
                    ),
                    contents,
                )
                .await
                .unwrap();
        }
        writer.close().await.unwrap();
        drop(archive);

        let source = CreatePackFile::Path(path);
        let reader = MrpackZipReader::new(&source).await.unwrap();
        let specs = reader
            .file()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let entry_name = entry.filename().as_str().unwrap().to_string();
                let relative_path =
                    override_relative_path(&entry_name, "").unwrap();
                OverrideExtractionSpec {
                    index,
                    entry_name,
                    crc32: entry.crc32(),
                    uncompressed_size: entry.uncompressed_size(),
                    target_path: directory
                        .path()
                        .join("output")
                        .join(&relative_path),
                    relative_path,
                }
            })
            .collect::<Vec<_>>();
        let semaphore = Arc::new(crate::util::fetch::IoSemaphore(
            tokio::sync::Semaphore::new(OVERRIDE_EXTRACTION_CONCURRENCY),
        ));
        let extracted = futures::stream::iter(specs)
            .map(|spec| {
                let source = source.clone();
                let semaphore = Arc::clone(&semaphore);
                async move {
                    let mut reader = MrpackZipReader::new(&source).await?;
                    let staging_path =
                        mrpack_override_staging_path(&spec.target_path);
                    reader
                        .extract_override_entry(
                            &spec,
                            &staging_path,
                            &semaphore,
                            None,
                        )
                        .await?;
                    Ok::<_, crate::Error>(spec)
                }
            })
            .buffer_unordered(OVERRIDE_EXTRACTION_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        let specs = extracted.into_iter().collect::<crate::Result<Vec<_>>>()?;

        for spec in &specs {
            let expected = entries
                .iter()
                .find(|(name, _)| *name == spec.entry_name)
                .unwrap()
                .1;
            assert!(!spec.target_path.exists());
            assert_eq!(
                tokio::fs::read(mrpack_override_staging_path(
                    &spec.target_path
                ))
                .await
                .unwrap(),
                expected
            );
        }
        let targets = specs
            .iter()
            .map(|spec| spec.target_path.clone())
            .collect::<Vec<_>>();
        tokio::task::spawn_blocking(move || {
            crate::api::pack::archive_util::commit_staged_archive_entries(
                &targets, None,
            )
        })
        .await??;

        for spec in specs {
            let expected = entries
                .iter()
                .find(|(name, _)| *name == spec.entry_name)
                .unwrap()
                .1;
            assert_eq!(
                tokio::fs::read(spec.target_path).await.unwrap(),
                expected
            );
        }
        Ok(())
    }
}
