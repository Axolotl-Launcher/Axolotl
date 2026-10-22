use super::events::InstallProgressReporter;
use super::model::{
    DownloadItemStatus, InstallJobEventKind, InstallJobSnapshot,
    InstallJobState, InstallJobStatus, InstallRequest, InstallTarget,
    MissingModpackFileState,
};
use super::{runner, store};
use crate::State;
use crate::util::download::constants::DOWNLOAD_SCAN_CACHE_TTL;
use crate::util::download::constants::DOWNLOAD_STABILITY_WINDOW;
use crate::util::fetch::{
    ContentValidation, DownloadRequest, Integrity, ResourceClass,
    download_to_path, verify_file,
};
use futures::{StreamExt, stream};
use parking_lot::Mutex;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
#[cfg(test)]
use std::time::Duration;
use std::time::{Instant, SystemTime};
use uuid::Uuid;

static DOWNLOAD_SCAN_CACHE: LazyLock<Mutex<DownloadsScanCache>> =
    LazyLock::new(|| Mutex::new(DownloadsScanCache::default()));

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingModpackContentView {
    pub remaining: usize,
    pub files: Vec<MissingModpackFileView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingModpackFileView {
    pub item_id: String,
    pub path: String,
    pub expected_size: u64,
    pub status: DownloadItemStatus,
    pub last_error: Option<String>,
    pub browser_urls: Vec<String>,
    pub attempt: Option<u32>,
    pub max_attempts: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingModpackScanResult {
    pub download_directory: Option<String>,
    pub content: MissingModpackContentView,
    pub imported_item_ids: Vec<String>,
    pub mismatched_item_ids: Vec<String>,
    pub rejected_item_ids: Vec<String>,
    pub checked_candidates: usize,
    pub pending_candidates: usize,
    pub errors: Vec<MissingModpackScanError>,
    pub job: InstallJobSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingModpackScanError {
    pub item_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CandidateStamp {
    size: u64,
    modified: Option<SystemTime>,
    identity: Option<CandidateFileIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CandidateFileIdentity {
    #[cfg(windows)]
    Windows { volume_serial: u32, file_index: u64 },
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
}

struct CandidateObservation {
    stamp: CandidateStamp,
    first_seen: Instant,
}

#[derive(Default)]
struct JobDownloadsScanCache {
    observations: HashMap<PathBuf, CandidateObservation>,
    rejected: HashMap<(String, PathBuf), CandidateStamp>,
    last_used: Option<Instant>,
}

#[derive(Default)]
struct DownloadsScanCache {
    jobs: HashMap<Uuid, JobDownloadsScanCache>,
}

struct MissingFileCandidateSpec {
    item_id: String,
    file_name: String,
    expected_size: u64,
}

struct StableDownloadCandidate {
    item_id: String,
    path: PathBuf,
    expected_size: u64,
    stamp: CandidateStamp,
}

struct CandidateCollection {
    stable: Vec<StableDownloadCandidate>,
    pending: usize,
}

struct CandidateGroup {
    item_id: String,
    candidates: Vec<StableDownloadCandidate>,
}

struct CandidateGroupResult {
    item_id: String,
    imported: bool,
    mismatched: bool,
    rejected: bool,
    checked_candidates: usize,
    error: Option<String>,
}

pub async fn list_missing_modpack_files(
    job_id: Uuid,
) -> crate::Result<MissingModpackContentView> {
    let state = State::get().await?;
    let job = waiting_job(job_id, &state).await?;
    missing_content_view(&job.state)
}

pub async fn scan_missing_modpack_files(
    job_id: Uuid,
    scan_directory: Option<PathBuf>,
) -> crate::Result<MissingModpackScanResult> {
    let state = State::get().await?;
    let job = waiting_job(job_id, &state).await?;
    let Some(download_directory) = scan_directory.or_else(dirs::download_dir)
    else {
        return unavailable_scan_result(job);
    };
    let Ok(download_directory) =
        tokio::fs::canonicalize(download_directory).await
    else {
        return unavailable_scan_result(job);
    };
    if !tokio::fs::metadata(&download_directory)
        .await
        .is_ok_and(|metadata| metadata.is_dir())
    {
        return unavailable_scan_result(job);
    }
    scan_missing_modpack_files_in(job_id, &download_directory).await
}

fn unavailable_scan_result(
    job: store::InstallJobRecord,
) -> crate::Result<MissingModpackScanResult> {
    Ok(MissingModpackScanResult {
        download_directory: None,
        content: missing_content_view(&job.state)?,
        imported_item_ids: Vec::new(),
        mismatched_item_ids: Vec::new(),
        rejected_item_ids: Vec::new(),
        checked_candidates: 0,
        pending_candidates: 0,
        errors: Vec::new(),
        job: job.snapshot(),
    })
}

async fn scan_missing_modpack_files_in(
    job_id: Uuid,
    download_directory: &Path,
) -> crate::Result<MissingModpackScanResult> {
    scan_missing_modpack_files_in_at(job_id, download_directory, Instant::now())
        .await
}

async fn scan_missing_modpack_files_in_at(
    job_id: Uuid,
    download_directory: &Path,
    now: Instant,
) -> crate::Result<MissingModpackScanResult> {
    let state = State::get().await?;
    let job = waiting_job(job_id, &state).await?;
    let specs = pending_candidate_specs(&job.state)?;
    let candidates =
        collect_download_candidates(job_id, download_directory, &specs, now)
            .await?;
    let pending_candidates = candidates.pending;
    let mut imported_item_ids = Vec::new();
    let mut mismatched_item_ids = Vec::new();
    let mut rejected_item_ids = Vec::new();
    let mut checked_candidates = 0;
    let mut errors = Vec::new();
    let groups = group_candidates_by_item(candidates.stable);
    let _permit = state.install_job_semaphore.acquire().await?;
    let current = store::get_required(job_id, &state).await?;
    if current.status == InstallJobStatus::WaitingForUser {
        let concurrency = crate::util::download::task_concurrency_limit(&state)
            .unwrap_or_else(|| groups.len().max(1));
        let results =
            process_candidate_groups_concurrently(
                groups,
                concurrency,
                |group| {
                    let state = state.clone();
                    async move {
                        process_candidate_group(job_id, group, &state).await
                    }
                },
            )
            .await;

        for result in results {
            let result = result?;
            checked_candidates += result.checked_candidates;
            if result.imported {
                imported_item_ids.push(result.item_id.clone());
            }
            if result.mismatched {
                mismatched_item_ids.push(result.item_id.clone());
            }
            if result.rejected {
                rejected_item_ids.push(result.item_id.clone());
            }
            if let Some(message) = result.error {
                errors.push(MissingModpackScanError {
                    item_id: result.item_id,
                    message,
                });
            }
        }
        if !imported_item_ids.is_empty() {
            let _ = resume_if_complete(job_id, &state).await?;
        }
    }

    let latest = store::get_required(job_id, &state).await?;
    Ok(MissingModpackScanResult {
        download_directory: Some(
            download_directory.to_string_lossy().into_owned(),
        ),
        content: if latest.state.missing_content.is_some() {
            missing_content_view(&latest.state)?
        } else {
            MissingModpackContentView {
                remaining: 0,
                files: Vec::new(),
            }
        },
        imported_item_ids,
        mismatched_item_ids,
        rejected_item_ids,
        checked_candidates,
        pending_candidates,
        errors,
        job: latest.snapshot(),
    })
}

fn group_candidates_by_item(
    candidates: Vec<StableDownloadCandidate>,
) -> Vec<CandidateGroup> {
    let mut groups = Vec::<CandidateGroup>::new();
    for candidate in candidates {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.item_id == candidate.item_id)
        {
            group.candidates.push(candidate);
        } else {
            groups.push(CandidateGroup {
                item_id: candidate.item_id.clone(),
                candidates: vec![candidate],
            });
        }
    }
    groups
}

async fn process_candidate_groups_concurrently<F, Fut>(
    groups: Vec<CandidateGroup>,
    concurrency: usize,
    process: F,
) -> Vec<crate::Result<CandidateGroupResult>>
where
    F: Fn(CandidateGroup) -> Fut,
    Fut: Future<Output = crate::Result<CandidateGroupResult>>,
{
    stream::iter(groups)
        .map(process)
        .buffered(concurrency.max(1))
        .collect()
        .await
}

async fn process_candidate_group(
    job_id: Uuid,
    group: CandidateGroup,
    state: &State,
) -> crate::Result<CandidateGroupResult> {
    let mut checked_candidates = 0;
    let mut mismatched = false;
    let mut rejected = false;
    let mut error = None;
    for candidate in group.candidates {
        if candidate_was_rejected(
            job_id,
            &candidate.item_id,
            &candidate.path,
            &candidate.stamp,
        ) {
            rejected = true;
            continue;
        }
        if candidate.stamp.size != candidate.expected_size {
            mark_candidate_rejected(
                job_id,
                &candidate.item_id,
                &candidate.path,
                candidate.stamp,
            );
            mismatched = true;
            rejected = true;
            continue;
        }
        let current = store::get_required(job_id, state).await?;
        match item_resolution_state(&current, &candidate.item_id) {
            ItemResolutionState::Resolved | ItemResolutionState::JobResumed => {
                break;
            }
            ItemResolutionState::Pending => {}
        }
        let file = pending_file(&current.state, &candidate.item_id)?.clone();
        let integrity = required_integrity(&file)?;
        let reporter = InstallProgressReporter::new(job_id, current.state);
        reporter
            .record_events(vec![
                InstallJobEventKind::ContentFileVerificationStarted {
                    path: candidate.item_id.clone(),
                },
            ])
            .await?;
        let _io_permit = state.io_semaphore.0.acquire().await?;
        checked_candidates += 1;
        if verify_file(&candidate.path, &integrity).await.is_err() {
            mark_candidate_rejected(
                job_id,
                &candidate.item_id,
                &candidate.path,
                candidate.stamp,
            );
            reporter
                .record_events(vec![InstallJobEventKind::ContentFileFailed {
                    path: candidate.item_id.clone(),
                    reason: "The downloaded file does not match this modpack"
                        .to_string(),
                    project_id: None,
                    version_id: None,
                }])
                .await?;
            mismatched = true;
            rejected = true;
            continue;
        }

        match import_missing_modpack_file_locked(
            job_id,
            candidate.item_id.clone(),
            candidate.path.clone(),
            state,
            true,
        )
        .await
        {
            Ok(()) => {
                return Ok(CandidateGroupResult {
                    item_id: group.item_id,
                    imported: true,
                    mismatched: false,
                    rejected: false,
                    checked_candidates,
                    error: None,
                });
            }
            Err(import_error) => {
                mark_candidate_rejected(
                    job_id,
                    &candidate.item_id,
                    &candidate.path,
                    candidate.stamp,
                );
                mismatched = true;
                rejected = true;
                error = Some(user_import_error(&import_error));
            }
        }
    }

    Ok(CandidateGroupResult {
        item_id: group.item_id,
        imported: false,
        mismatched,
        rejected,
        checked_candidates,
        error,
    })
}

fn pending_candidate_specs(
    job_state: &InstallJobState,
) -> crate::Result<Vec<MissingFileCandidateSpec>> {
    let view = missing_content_view(job_state)?;
    view.files
        .into_iter()
        .map(|file| {
            let file_name = Path::new(&file.path)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Required modpack file has no safe file name"
                            .to_string(),
                    )
                })?;
            Ok(MissingFileCandidateSpec {
                item_id: file.item_id,
                file_name: file_name.to_string(),
                expected_size: file.expected_size,
            })
        })
        .collect()
}

async fn collect_download_candidates(
    job_id: Uuid,
    download_directory: &Path,
    specs: &[MissingFileCandidateSpec],
    now: Instant,
) -> crate::Result<CandidateCollection> {
    let mut entries = match tokio::fs::read_dir(download_directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CandidateCollection {
                stable: Vec::new(),
                pending: 0,
            });
        }
        Err(error) => return Err(error.into()),
    };
    let mut discovered = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let actual_name = entry.file_name().to_string_lossy().into_owned();
        if crate::util::downloads::is_incomplete_browser_download(&actual_name)
        {
            continue;
        }
        let matched_specs = specs
            .iter()
            .filter(|spec| {
                crate::util::downloads::browser_download_file_name_matches(
                    &actual_name,
                    &spec.file_name,
                )
            })
            .collect::<Vec<_>>();
        if matched_specs.is_empty() {
            continue;
        }
        let path = entry.path();
        let metadata = match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata)
                if metadata.is_file()
                    && !crate::util::io::is_symlink_or_reparse(&metadata) =>
            {
                metadata
            }
            _ => continue,
        };
        let stamp = CandidateStamp {
            size: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(windows)]
            identity: candidate_file_identity(&path),
            #[cfg(not(windows))]
            identity: candidate_file_identity(&metadata),
        };
        for spec in matched_specs {
            discovered.push(StableDownloadCandidate {
                item_id: spec.item_id.clone(),
                path: path.clone(),
                expected_size: spec.expected_size,
                stamp: stamp.clone(),
            });
        }
    }

    let mut cache = DOWNLOAD_SCAN_CACHE.lock();
    cache.jobs.retain(|_, job| {
        job.last_used.is_some_and(|last_used| {
            now.saturating_duration_since(last_used) < DOWNLOAD_SCAN_CACHE_TTL
        })
    });
    let job_cache = cache.jobs.entry(job_id).or_default();
    job_cache.last_used = Some(now);
    let discovered_paths = discovered
        .iter()
        .map(|candidate| candidate.path.as_path())
        .collect::<HashSet<_>>();
    job_cache
        .observations
        .retain(|path, _| discovered_paths.contains(path.as_path()));
    job_cache
        .rejected
        .retain(|(_, path), _| discovered_paths.contains(path.as_path()));
    let mut stable = Vec::new();
    let mut pending_paths = HashSet::new();
    for candidate in discovered {
        if candidate_is_stable(
            job_cache,
            &candidate.path,
            &candidate.stamp,
            now,
        ) {
            stable.push(candidate);
        } else {
            pending_paths.insert(candidate.path);
        }
    }
    Ok(CandidateCollection {
        stable,
        pending: pending_paths.len(),
    })
}

#[cfg(windows)]
fn candidate_file_identity(path: &Path) -> Option<CandidateFileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let file = std::fs::File::open(path).ok()?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            &raw mut information,
        )
        .ok()?;
    }
    Some(CandidateFileIdentity::Windows {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: ((information.nFileIndexHigh as u64) << 32)
            | information.nFileIndexLow as u64,
    })
}

#[cfg(not(windows))]
fn candidate_file_identity(
    metadata: &std::fs::Metadata,
) -> Option<CandidateFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(CandidateFileIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn candidate_is_stable(
    cache: &mut JobDownloadsScanCache,
    path: &Path,
    stamp: &CandidateStamp,
    now: Instant,
) -> bool {
    match cache.observations.get(path) {
        Some(observation)
            if observation.stamp == *stamp
                && now.saturating_duration_since(observation.first_seen)
                    >= DOWNLOAD_STABILITY_WINDOW =>
        {
            true
        }
        Some(observation) if observation.stamp == *stamp => false,
        _ => {
            cache.observations.insert(
                path.to_path_buf(),
                CandidateObservation {
                    stamp: stamp.clone(),
                    first_seen: now,
                },
            );
            false
        }
    }
}

fn candidate_was_rejected(
    job_id: Uuid,
    item_id: &str,
    path: &Path,
    stamp: &CandidateStamp,
) -> bool {
    DOWNLOAD_SCAN_CACHE
        .lock()
        .jobs
        .get(&job_id)
        .and_then(|job| {
            job.rejected.get(&(item_id.to_string(), path.to_path_buf()))
        })
        == Some(stamp)
}

fn mark_candidate_rejected(
    job_id: Uuid,
    item_id: &str,
    path: &Path,
    stamp: CandidateStamp,
) {
    DOWNLOAD_SCAN_CACHE
        .lock()
        .jobs
        .entry(job_id)
        .or_default()
        .rejected
        .insert((item_id.to_string(), path.to_path_buf()), stamp);
}

enum ItemResolutionState {
    Pending,
    Resolved,
    JobResumed,
}

fn item_resolution_state(
    job: &store::InstallJobRecord,
    item_id: &str,
) -> ItemResolutionState {
    if job.status != InstallJobStatus::WaitingForUser {
        return ItemResolutionState::JobResumed;
    }
    if job.state.download_items().into_iter().any(|item| {
        item.id == item_id
            && matches!(
                item.status,
                DownloadItemStatus::Completed | DownloadItemStatus::Skipped
            )
    }) {
        ItemResolutionState::Resolved
    } else {
        ItemResolutionState::Pending
    }
}

fn missing_item_is_resolved(
    job: &store::InstallJobRecord,
    item_id: &str,
) -> crate::Result<bool> {
    SafeRelativeUtf8UnixPathBuf::try_from(item_id.to_string())?;
    if !matches!(
        &job.state.request,
        InstallRequest::CreateModpackInstance { .. }
            | InstallRequest::InstallPackToExistingInstance { .. }
    ) {
        return Ok(false);
    }
    Ok(job.state.download_items().into_iter().any(|item| {
        item.id == item_id
            && matches!(
                item.status,
                DownloadItemStatus::Completed | DownloadItemStatus::Skipped
            )
    }))
}

pub async fn retry_missing_modpack_file(
    job_id: Uuid,
    item_id: String,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let _permit = state.install_job_semaphore.acquire().await?;
    let current = store::get_required(job_id, &state).await?;
    if missing_item_is_resolved(&current, &item_id)? {
        return Ok(current.snapshot());
    }
    let job = waiting_job(job_id, &state).await?;
    let file = pending_file(&job.state, &item_id)?.clone();
    let instance_base = instance_base(&job.state, &state).await?;
    let target = super::recovery::checked_instance_path(
        &instance_base,
        &file.target_path,
    )?;
    let integrity = required_integrity(&file)?;
    let Some(primary_url) = file.download_urls.first() else {
        return Err(crate::ErrorKind::InputError(
            "This modpack file has no automatic download URL".to_string(),
        )
        .into());
    };
    let current = job
        .state
        .download_items()
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| unknown_item_error(&item_id))?;
    let attempt = current.attempt.unwrap_or(0).saturating_add(1);
    let max_attempts = current.max_attempts.unwrap_or(1).max(attempt);
    let reporter = InstallProgressReporter::new(job_id, job.state.clone());
    reporter
        .record_events(vec![InstallJobEventKind::ContentFileDownloadAttempt {
            path: item_id.clone(),
            bytes_total: Some(file.expected_size),
            attempt,
            max_attempts,
        }])
        .await?;

    let result = download_to_path(
        DownloadRequest::new(primary_url, ResourceClass::Modpack)
            .with_candidate_urls(file.download_urls.iter().skip(1).cloned())
            .with_integrity(integrity)
            .with_install_tracking(
                reporter.clone(),
                item_id.clone(),
                file.target_path.clone(),
            ),
        &target,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await;
    match result {
        Ok(download) => {
            reporter
                .record_events(vec![
                    InstallJobEventKind::ContentFileRecovered {
                        path: item_id,
                        bytes: download.size,
                    },
                ])
                .await?;
            resume_if_complete(job_id, &state).await
        }
        Err(error) => {
            tracing::warn!(job_id = %job_id, item_id, %error, "Manual required-file retry failed");
            reporter
                .record_events(vec![InstallJobEventKind::ContentFileFailed {
                    path: item_id,
                    reason: "Automatic download failed. Use browser download or choose the required file locally."
                        .to_string(),
                    project_id: None,
                    version_id: None,
                }])
                .await?;
            Err(crate::ErrorKind::InputError(
                "Automatic download failed. Try browser download or choose a local file."
                    .to_string(),
            )
            .into())
        }
    }
}

pub async fn import_missing_modpack_file(
    job_id: Uuid,
    item_id: String,
    selected_file_path: PathBuf,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let _permit = state.install_job_semaphore.acquire().await?;
    let current = store::get_required(job_id, &state).await?;
    if missing_item_is_resolved(&current, &item_id)? {
        return Ok(current.snapshot());
    }
    import_missing_modpack_file_locked(
        job_id,
        item_id,
        selected_file_path,
        &state,
        false,
    )
    .await?;
    resume_if_complete(job_id, &state).await
}

async fn import_missing_modpack_file_locked(
    job_id: Uuid,
    item_id: String,
    selected_file_path: PathBuf,
    state: &State,
    verification_started: bool,
) -> crate::Result<()> {
    let job = waiting_job(job_id, state).await?;
    let file =
        resolvable_file(&job.state, &item_id, verification_started)?.clone();
    let instance_base = instance_base(&job.state, state).await?;
    let target = super::recovery::checked_instance_path(
        &instance_base,
        &file.target_path,
    )?;
    let integrity = required_integrity(&file)?;
    let reporter = InstallProgressReporter::new(job_id, job.state.clone());
    if !verification_started {
        reporter
            .record_events(vec![
                InstallJobEventKind::ContentFileVerificationStarted {
                    path: item_id.clone(),
                },
            ])
            .await?;
    }

    let import = materialize_verified_file(
        &selected_file_path,
        &target,
        &integrity,
        || {
            let reporter = reporter.clone();
            let item_id = item_id.clone();
            async move {
                reporter
                    .record_events(vec![
                        InstallJobEventKind::ContentFileWritingStarted {
                            path: item_id,
                        },
                    ])
                    .await
                    .map(|_| ())
            }
        },
    )
    .await;

    match import {
        Ok(size) => {
            reporter
                .record_events(vec![
                    InstallJobEventKind::ContentFileRecovered {
                        path: item_id,
                        bytes: size,
                    },
                ])
                .await?;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(job_id = %job_id, item_id, selected_path = %selected_file_path.display(), %error, "Selected modpack file was rejected");
            let message = user_import_error(&error);
            reporter
                .record_events(vec![InstallJobEventKind::ContentFileFailed {
                    path: item_id,
                    reason: message.clone(),
                    project_id: None,
                    version_id: None,
                }])
                .await?;
            Err(crate::ErrorKind::InputError(message).into())
        }
    }
}

pub(crate) fn browser_download_urls(downloads: &[String]) -> Vec<String> {
    downloads
        .iter()
        .filter_map(|download| {
            let url = reqwest::Url::parse(download).ok()?;
            if !matches!(url.scheme(), "http" | "https")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query_pairs().any(|(name, _)| {
                    matches!(
                        name.to_ascii_lowercase().as_str(),
                        "auth"
                            | "authorization"
                            | "key"
                            | "api_key"
                            | "apikey"
                            | "access_token"
                            | "sig"
                            | "signature"
                            | "token"
                            | "x-amz-signature"
                            | "x-goog-signature"
                    )
                })
            {
                return None;
            }
            crate::util::download::retired_sources::normalize(download)
                .ok()
                .map(|url| url.into_owned())
        })
        .fold(Vec::new(), |mut urls, url| {
            if !urls.contains(&url) {
                urls.push(url);
            }
            urls
        })
}

fn missing_content_view(
    job_state: &InstallJobState,
) -> crate::Result<MissingModpackContentView> {
    let content = job_state.missing_content.as_ref().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "This install job has no persisted missing-content context"
                .to_string(),
        )
    })?;
    let items = job_state.download_items();
    let files = content
        .files
        .iter()
        .filter_map(|file| {
            let item = items.iter().find(|item| item.id == file.item_id)?;
            (!matches!(
                item.status,
                DownloadItemStatus::Completed | DownloadItemStatus::Skipped
            ))
            .then(|| MissingModpackFileView {
                item_id: file.item_id.clone(),
                path: file.target_path.clone(),
                expected_size: file.expected_size,
                status: item.status,
                last_error: item.error.clone(),
                browser_urls: browser_download_urls(&file.browser_urls),
                attempt: item.attempt,
                max_attempts: item.max_attempts,
            })
        })
        .collect::<Vec<_>>();
    Ok(MissingModpackContentView {
        remaining: files.len(),
        files,
    })
}

async fn waiting_job(
    job_id: Uuid,
    state: &State,
) -> crate::Result<store::InstallJobRecord> {
    let job = store::get_required(job_id, state).await?;
    if job.status != InstallJobStatus::WaitingForUser {
        return Err(crate::ErrorKind::InputError(
            "Missing modpack files can only be resolved while the job is waiting for user action"
                .to_string(),
        )
        .into());
    }
    if job.state.missing_content.is_none() {
        return Err(crate::ErrorKind::InputError(
            "This job is not waiting for required Modrinth pack content"
                .to_string(),
        )
        .into());
    }
    if !matches!(
        job.state.request,
        InstallRequest::CreateModpackInstance { .. }
            | InstallRequest::InstallPackToExistingInstance { .. }
    ) {
        return Err(crate::ErrorKind::InputError(
            "Missing-content resolution is only available for modpack install jobs"
                .to_string(),
        )
        .into());
    }
    Ok(job)
}

fn pending_file<'a>(
    job_state: &'a InstallJobState,
    item_id: &str,
) -> crate::Result<&'a MissingModpackFileState> {
    resolvable_file(job_state, item_id, false)
}

fn resolvable_file<'a>(
    job_state: &'a InstallJobState,
    item_id: &str,
    allow_verifying: bool,
) -> crate::Result<&'a MissingModpackFileState> {
    SafeRelativeUtf8UnixPathBuf::try_from(item_id.to_string())?;
    let file = job_state
        .missing_content
        .as_ref()
        .and_then(|content| {
            content.files.iter().find(|file| file.item_id == item_id)
        })
        .ok_or_else(|| unknown_item_error(item_id))?;
    let item = job_state
        .download_items()
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| unknown_item_error(item_id))?;
    if item.status != DownloadItemStatus::Failed
        && !(allow_verifying && item.status == DownloadItemStatus::Verifying)
    {
        return Err(crate::ErrorKind::InputError(
            "Only failed required modpack files can be resolved".to_string(),
        )
        .into());
    }
    Ok(file)
}

fn unknown_item_error(item_id: &str) -> crate::Error {
    crate::ErrorKind::InputError(format!(
        "Required modpack item does not belong to this job: {item_id}"
    ))
    .into()
}

fn required_integrity(
    file: &MissingModpackFileState,
) -> crate::Result<Integrity> {
    if file
        .sha1
        .as_ref()
        .is_some_and(|hash| !valid_hex_hash(hash, 40))
        || file
            .sha512
            .as_ref()
            .is_some_and(|hash| !valid_hex_hash(hash, 128))
        || file.sha1.is_none() && file.sha512.is_none()
    {
        return Err(crate::ErrorKind::InputError(
            "Modpack file has invalid or missing cryptographic integrity metadata"
                .to_string(),
        )
        .into());
    }
    Ok(Integrity {
        size: Some(file.expected_size),
        sha1: file.sha1.clone(),
        sha512: file.sha512.clone(),
        content: if file.validate_as_jar {
            ContentValidation::Jar
        } else {
            ContentValidation::None
        },
        ..Integrity::default()
    })
}

fn valid_hex_hash(hash: &str, length: usize) -> bool {
    hash.len() == length && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn instance_base(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<PathBuf> {
    let instance_id = match &job_state.target {
        InstallTarget::NewInstance {
            instance_id: Some(instance_id),
        }
        | InstallTarget::ExistingInstance { instance_id } => instance_id,
        InstallTarget::NewInstance { instance_id: None } => {
            return Err(crate::ErrorKind::InputError(
                "Install job has no target instance".to_string(),
            )
            .into());
        }
    };
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unknown target instance {instance_id}"
            ))
        })?;
    Ok(state.directories.instance_game_dir(&instance.instance))
}

async fn materialize_verified_file<F, Fut>(
    selected_file: &Path,
    target: &Path,
    integrity: &Integrity,
    before_write: F,
) -> crate::Result<u64>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let metadata =
        tokio::fs::symlink_metadata(selected_file)
            .await
            .map_err(|_| {
                crate::ErrorKind::InputError(
                    "Unable to read the selected file".to_string(),
                )
            })?;
    if !metadata.is_file() || crate::util::io::is_symlink_or_reparse(&metadata)
    {
        return Err(crate::ErrorKind::InputError(
            "The selected path is not a regular readable file".to_string(),
        )
        .into());
    }
    if integrity.size.is_some_and(|size| size != metadata.len()) {
        return Err(crate::ErrorKind::InputError(
            "The selected file size does not match the modpack requirement"
                .to_string(),
        )
        .into());
    }
    let parent = target.parent().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Required modpack target has no parent directory".to_string(),
        )
    })?;
    crate::util::io::create_dir_all(parent).await?;
    let staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| crate::util::io::IOError::with_path(error, parent))?
        .into_temp_path();
    crate::util::io::copy(selected_file, &staged).await?;
    let size = verify_file(&staged, integrity).await.map_err(|error| {
        tracing::debug!(%error, selected_path = %selected_file.display(), "Selected file integrity mismatch");
        crate::Error::from(crate::ErrorKind::InputError(
            "The selected file is not the version required by this modpack"
                .to_string(),
        ))
    })?;
    before_write().await?;
    let previous =
        crate::state::materialize_project_download(&staged, target).await?;
    if let Err(error) =
        crate::state::finalize_project_materialization(previous.as_deref())
            .await
    {
        crate::state::restore_project_materialization(
            target,
            previous.as_deref(),
        )
        .await?;
        return Err(error);
    }
    Ok(size)
}

fn user_import_error(error: &crate::Error) -> String {
    match error.raw.as_ref() {
        crate::ErrorKind::InputError(message) => message.clone(),
        _ => "Unable to import the selected file safely".to_string(),
    }
}

async fn resume_if_complete(
    job_id: Uuid,
    state: &State,
) -> crate::Result<InstallJobSnapshot> {
    let job = store::get_required(job_id, state).await?;
    if all_missing_content_resolved(&job.state)? {
        runner::resume_job(job_id).await
    } else {
        Ok(job.snapshot())
    }
}

fn all_missing_content_resolved(
    job_state: &InstallJobState,
) -> crate::Result<bool> {
    Ok(missing_content_view(job_state)?.remaining == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::pack::install_from::CreatePackLocation;
    use crate::install::model::MissingModpackContentState;
    #[cfg(not(feature = "tauri"))]
    use crate::install::model::{InstallJobExecutionMode, InstallPauseReason};
    #[cfg(not(feature = "tauri"))]
    use crate::state::{InstanceLink, ModLoader};
    use sha1_smol::Sha1;

    fn integrity(bytes: &[u8]) -> Integrity {
        Integrity {
            size: Some(bytes.len() as u64),
            sha1: Some(Sha1::from(bytes).hexdigest()),
            ..Integrity::default()
        }
    }

    #[tokio::test]
    async fn verified_import_is_atomic_and_rejects_wrong_hashes() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("instance/mods/example.bin");
        crate::util::io::create_dir_all(target.parent().unwrap())
            .await
            .unwrap();
        crate::util::io::write(&target, b"existing-bad-file")
            .await
            .unwrap();
        let good = root.path().join("example.bin");
        let wrong_same_name = root.path().join("other/example.bin");
        let wrong_same_size = root.path().join("same-size.bin");
        crate::util::io::create_dir_all(wrong_same_name.parent().unwrap())
            .await
            .unwrap();
        crate::util::io::write(&good, b"required-content")
            .await
            .unwrap();
        crate::util::io::write(&wrong_same_name, b"wrong-content!!!")
            .await
            .unwrap();
        crate::util::io::write(&wrong_same_size, b"wrong-content!!!")
            .await
            .unwrap();

        let expected = integrity(b"required-content");
        assert!(
            materialize_verified_file(
                &wrong_same_name,
                &target,
                &expected,
                || async { Ok(()) }
            )
            .await
            .is_err()
        );
        assert_eq!(
            crate::util::io::read(&target).await.unwrap(),
            b"existing-bad-file"
        );
        assert!(
            materialize_verified_file(
                &wrong_same_size,
                &target,
                &expected,
                || async { Ok(()) }
            )
            .await
            .is_err()
        );
        assert_eq!(
            crate::util::io::read(&target).await.unwrap(),
            b"existing-bad-file"
        );

        let size =
            materialize_verified_file(&good, &target, &expected, || async {
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(size, 16);
        assert_eq!(
            crate::util::io::read(&target).await.unwrap(),
            b"required-content"
        );
    }

    #[test]
    fn browser_urls_only_use_public_manifest_urls() {
        let urls = browser_download_urls(&[
            "https://cdn.modrinth.com/data/project/versions/file.jar"
                .to_string(),
            "https://secret@example.com/file.jar".to_string(),
            "https://mirror.example/file.jar?api_key=secret".to_string(),
            "https://mirror.example/file.jar?x-amz-signature=secret"
                .to_string(),
            "file:///tmp/file.jar".to_string(),
            "https://fallback.example/file.jar".to_string(),
        ]);
        assert_eq!(
            urls,
            vec![
                "https://cdn.modrinth.com/data/project/versions/file.jar",
                "https://fallback.example/file.jar",
            ]
        );
    }

    #[test]
    fn browser_urls_migrate_saved_retired_links_and_deduplicate_them() {
        let suffix = "/data/%e9%87%91/file+name%2B.jar?download=1#file";
        let urls = browser_download_urls(&[
            format!("https://cdn-alt.modrinth.com{suffix}"),
            format!("https://cdn.modrinth.com{suffix}"),
            "https://cdn-alt.modrinth.com/file.jar?token=secret".into(),
        ]);
        assert_eq!(urls, vec![format!("https://cdn.modrinth.com{suffix}")]);
    }

    #[test]
    fn tianpao_browser_urls_restore_known_paths_and_filter_unknown_paths() {
        let urls = browser_download_urls(&[
            "https://mod.tianpao.top/data/file.jar".into(),
            "https://cdn.modrinth.com/data/file.jar".into(),
            "https://mod.tianpao.top/files/1/2/file.jar".into(),
            "https://mod.tianpao.top/media/attachments/icon.png".into(),
            "https://mod.tianpao.top/unknown".into(),
            "https://mod.tianpao.top/data/file.jar?token=secret".into(),
        ]);
        assert_eq!(
            urls,
            vec![
                "https://cdn.modrinth.com/data/file.jar",
                "https://edge.forgecdn.net/files/1/2/file.jar",
                "https://media.forgecdn.net/attachments/icon.png"
            ]
        );
    }

    #[test]
    fn last_recovered_file_is_the_auto_resume_trigger() {
        let file = MissingModpackFileState {
            item_id: "mods/only.bin".to_string(),
            manifest_path: "mods/only.bin".to_string(),
            target_path: "mods/only.bin".to_string(),
            expected_size: 8,
            sha1: Some(Sha1::from(b"required").hexdigest()),
            sha512: None,
            download_urls: vec!["https://cdn.example/only.bin".to_string()],
            browser_urls: vec![
                "https://cdn-alt.modrinth.com/data/only.bin".to_string(),
            ],
            validate_as_jar: false,
        };
        let mut job_state =
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".to_string(),
                version: 21,
            });
        job_state.missing_content = Some(MissingModpackContentState {
            files: vec![file.clone()],
        });
        job_state.record_event(InstallJobEventKind::ContentFileQueued {
            path: file.item_id.clone(),
            bytes_total: Some(file.expected_size),
            max_attempts: 2,
        });
        job_state.record_event(InstallJobEventKind::ContentFileFailed {
            path: file.item_id.clone(),
            reason: "fixture failure".to_string(),
            project_id: None,
            version_id: None,
        });
        assert!(!all_missing_content_resolved(&job_state).unwrap());
        let restored: InstallJobState =
            serde_json::from_str(&serde_json::to_string(&job_state).unwrap())
                .unwrap();
        assert_eq!(
            missing_content_view(&restored).unwrap().files[0].browser_urls,
            vec!["https://cdn.modrinth.com/data/only.bin"]
        );

        job_state.record_event(InstallJobEventKind::ContentFileRecovered {
            path: file.item_id,
            bytes: file.expected_size,
        });
        assert!(all_missing_content_resolved(&job_state).unwrap());
    }

    #[tokio::test]
    async fn downloads_candidates_are_filtered_and_require_stability() {
        let directory = tempfile::tempdir().unwrap();
        let required = directory.path().join("required.bin");
        let duplicate = directory.path().join("required (1).bin");
        let incomplete = directory.path().join("required.bin.crdownload");
        crate::util::io::write(&required, b"required")
            .await
            .unwrap();
        crate::util::io::write(&duplicate, b"mismatch")
            .await
            .unwrap();
        crate::util::io::write(&incomplete, b"required")
            .await
            .unwrap();
        for index in 0..500 {
            crate::util::io::write(
                &directory.path().join(format!("unrelated-{index}.jar")),
                b"required",
            )
            .await
            .unwrap();
        }
        let specs = vec![MissingFileCandidateSpec {
            item_id: "mods/required.bin".to_string(),
            file_name: "required.bin".to_string(),
            expected_size: 8,
        }];
        let job_id = Uuid::new_v4();
        let first_seen = Instant::now();
        let first = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen,
        )
        .await
        .unwrap();
        assert!(first.stable.is_empty());
        assert_eq!(first.pending, 2);

        let stable = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen + DOWNLOAD_STABILITY_WINDOW,
        )
        .await
        .unwrap();
        assert_eq!(stable.stable.len(), 2);
        assert_eq!(stable.pending, 0);
        assert!(
            stable
                .stable
                .iter()
                .all(|candidate| candidate.path != incomplete)
        );
    }

    #[tokio::test]
    async fn unavailable_downloads_directory_returns_no_candidates() {
        let specs = vec![MissingFileCandidateSpec {
            item_id: "mods/required.bin".to_string(),
            file_name: "required.bin".to_string(),
            expected_size: 8,
        }];
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("not-created");
        let candidates = collect_download_candidates(
            Uuid::new_v4(),
            &missing,
            &specs,
            Instant::now(),
        )
        .await
        .unwrap();
        assert!(candidates.stable.is_empty());
        assert_eq!(candidates.pending, 0);
    }

    #[test]
    fn unchanged_hash_mismatches_are_not_checked_again() {
        let job_id = Uuid::new_v4();
        let item_id = "mods/required.bin";
        let path = PathBuf::from("Downloads/required.bin");
        let stamp = CandidateStamp {
            size: 8,
            modified: Some(SystemTime::UNIX_EPOCH),
            identity: None,
        };
        mark_candidate_rejected(job_id, item_id, &path, stamp.clone());
        assert!(candidate_was_rejected(job_id, item_id, &path, &stamp));
        assert!(!candidate_was_rejected(
            job_id,
            item_id,
            &path,
            &CandidateStamp {
                size: 9,
                modified: stamp.modified,
                identity: stamp.identity,
            }
        ));
    }

    #[tokio::test]
    async fn same_path_same_size_and_mtime_replacement_is_a_new_candidate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("required.bin");
        crate::util::io::write(&path, b"wrong123").await.unwrap();
        let original_modified = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        let specs = vec![MissingFileCandidateSpec {
            item_id: "mods/required.bin".to_string(),
            file_name: "required.bin".to_string(),
            expected_size: 8,
        }];
        let job_id = Uuid::new_v4();
        let first_seen = Instant::now();
        let first = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen,
        )
        .await
        .unwrap();
        assert_eq!(first.pending, 1);
        let original = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen + DOWNLOAD_STABILITY_WINDOW,
        )
        .await
        .unwrap();
        let original_stamp = original.stable[0].stamp.clone();
        mark_candidate_rejected(
            job_id,
            "mods/required.bin",
            &path,
            original_stamp.clone(),
        );

        tokio::fs::remove_file(&path).await.unwrap();
        crate::util::io::write(&path, b"correct!").await.unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(original_modified)
            .unwrap();

        let replacement = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 2,
        )
        .await
        .unwrap();
        assert_eq!(replacement.pending, 1);
        assert!(replacement.stable.is_empty());
        let stable_replacement = collect_download_candidates(
            job_id,
            directory.path(),
            &specs,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 3,
        )
        .await
        .unwrap();
        let replacement_stamp = &stable_replacement.stable[0].stamp;
        assert_eq!(replacement_stamp.size, original_stamp.size);
        assert_eq!(replacement_stamp.modified, original_stamp.modified);
        assert_ne!(replacement_stamp.identity, original_stamp.identity);
        assert!(!candidate_was_rejected(
            job_id,
            "mods/required.bin",
            &path,
            replacement_stamp,
        ));
    }

    #[test]
    fn scanner_race_uses_typed_job_and_item_state() {
        let mut job_state =
            InstallJobState::new(InstallRequest::CreateModpackInstance {
                location: CreatePackLocation::FromFile {
                    path: PathBuf::from("test.mrpack"),
                },
                post_install_edit: None,
            });
        job_state.record_event(InstallJobEventKind::ContentFileQueued {
            path: "mods/resolved.bin".to_string(),
            bytes_total: Some(8),
            max_attempts: 2,
        });
        job_state.record_event(InstallJobEventKind::ContentFileRecovered {
            path: "mods/resolved.bin".to_string(),
            bytes: 8,
        });
        let now = chrono::Utc::now();
        let mut record = store::InstallJobRecord {
            id: Uuid::new_v4(),
            instance_id: Some("instance".to_string()),
            kind: job_state.request.kind(),
            status: InstallJobStatus::WaitingForUser,
            state: job_state,
            created: now,
            modified: now,
            finished: None,
            dismissed: false,
        };
        assert!(matches!(
            item_resolution_state(&record, "mods/resolved.bin"),
            ItemResolutionState::Resolved
        ));
        record.status = InstallJobStatus::Running;
        assert!(matches!(
            item_resolution_state(&record, "mods/resolved.bin"),
            ItemResolutionState::JobResumed
        ));
    }

    #[tokio::test]
    async fn candidate_groups_are_processed_concurrently() {
        use std::sync::Arc;
        use tokio::sync::Barrier;

        let barrier = Arc::new(Barrier::new(2));
        let groups = ["mods/one.bin", "mods/two.bin"]
            .into_iter()
            .map(|item_id| CandidateGroup {
                item_id: item_id.to_string(),
                candidates: Vec::new(),
            })
            .collect();
        let results = tokio::time::timeout(
            Duration::from_secs(1),
            process_candidate_groups_concurrently(groups, 2, |group| {
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    Ok(CandidateGroupResult {
                        item_id: group.item_id,
                        imported: false,
                        mismatched: false,
                        rejected: false,
                        checked_candidates: 0,
                        error: None,
                    })
                }
            }),
        )
        .await
        .expect("both candidate groups should run before either completes");

        assert_eq!(results.len(), 2);
        assert!(results.into_iter().all(|result| result.is_ok()));
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn import_api_trusts_persisted_job_context_and_keeps_other_missing_files_waiting()
     {
        crate::event::EventState::init().await.unwrap();
        let root = tempfile::tempdir().unwrap().keep();
        let state = State::init_for_test(root.to_string_lossy().to_string())
            .await
            .unwrap();
        let created = crate::api::instance::create(
            format!("Stage 3 {}", Uuid::new_v4()),
            "1.20.1".to_string(),
            ModLoader::Vanilla,
            None,
            None,
            InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();
        let instance_id = created.instance.id;
        let instance_base = state
            .directories
            .instances_dir()
            .join(created.instance.path);
        let first_bytes = b"required-one";
        let second_bytes = b"required-two";
        let first = MissingModpackFileState {
            item_id: "mods/one.bin".to_string(),
            manifest_path: "mods/one.bin".to_string(),
            target_path: "mods/one.bin".to_string(),
            expected_size: first_bytes.len() as u64,
            sha1: Some(Sha1::from(first_bytes).hexdigest()),
            sha512: None,
            download_urls: vec!["https://cdn.example/one.bin".to_string()],
            browser_urls: vec!["https://cdn.example/one.bin".to_string()],
            validate_as_jar: false,
        };
        let second = MissingModpackFileState {
            item_id: "mods/two.bin".to_string(),
            manifest_path: "mods/two.bin".to_string(),
            target_path: "mods/two.bin".to_string(),
            expected_size: second_bytes.len() as u64,
            sha1: Some(Sha1::from(second_bytes).hexdigest()),
            sha512: None,
            download_urls: vec!["https://cdn.example/two.bin".to_string()],
            browser_urls: vec!["https://cdn.example/two.bin".to_string()],
            validate_as_jar: false,
        };
        let mut job_state = InstallJobState::new(
            InstallRequest::InstallPackToExistingInstance {
                instance_id: instance_id.clone(),
                location: CreatePackLocation::FromFile {
                    path: root.join("stage-3.mrpack"),
                },
                post_install_edit: None,
            },
        );
        job_state.set_progress(
            crate::install::model::InstallPhaseId::DownloadingContent,
            None,
            crate::install::model::InstallPhaseDetails::Empty,
        );
        job_state.missing_content = Some(MissingModpackContentState {
            files: vec![first.clone(), second.clone()],
        });
        job_state.pause_reason =
            Some(InstallPauseReason::MissingRequiredContent {
                failed_files: 2,
                paths: vec![first.item_id.clone(), second.item_id.clone()],
            });
        job_state.record_event(InstallJobEventKind::WaitingForUser {
            reason: job_state.pause_reason.clone().unwrap(),
        });
        for file in [&first, &second] {
            job_state.record_event(InstallJobEventKind::ContentFileQueued {
                path: file.item_id.clone(),
                bytes_total: Some(file.expected_size),
                max_attempts: 2,
            });
            job_state.record_event(
                InstallJobEventKind::ContentFileBrowserOptions {
                    path: file.item_id.clone(),
                    urls: file.browser_urls.clone(),
                },
            );
            job_state.record_event(InstallJobEventKind::ContentFileFailed {
                path: file.item_id.clone(),
                reason: "fixture download failed".to_string(),
                project_id: None,
                version_id: None,
            });
        }
        let job_id = Uuid::new_v4();
        store::insert(
            job_id,
            &job_state,
            InstallJobStatus::WaitingForUser,
            &state,
        )
        .await
        .unwrap();

        let queued_job_id = Uuid::new_v4();
        store::insert(
            queued_job_id,
            &job_state,
            InstallJobStatus::Queued,
            &state,
        )
        .await
        .unwrap();
        assert!(list_missing_modpack_files(queued_job_id).await.is_err());

        let selected = root.join("one-selected.bin");
        crate::util::io::write(&selected, first_bytes)
            .await
            .unwrap();
        assert!(
            import_missing_modpack_file(
                queued_job_id,
                first.item_id.clone(),
                selected.clone(),
            )
            .await
            .is_err()
        );
        let mut resolved_queued_state = job_state.clone();
        resolved_queued_state.record_event(
            InstallJobEventKind::ContentFileRecovered {
                path: first.item_id.clone(),
                bytes: first.expected_size,
            },
        );
        let resolved_queued_job_id = Uuid::new_v4();
        store::insert(
            resolved_queued_job_id,
            &resolved_queued_state,
            InstallJobStatus::Queued,
            &state,
        )
        .await
        .unwrap();
        assert!(
            import_missing_modpack_file(
                resolved_queued_job_id,
                first.item_id.clone(),
                selected.clone(),
            )
            .await
            .is_ok()
        );
        assert!(
            import_missing_modpack_file(
                job_id,
                "mods/not-in-job.bin".to_string(),
                selected.clone(),
            )
            .await
            .is_err()
        );
        assert!(
            import_missing_modpack_file(
                job_id,
                "../outside.bin".to_string(),
                selected.clone(),
            )
            .await
            .is_err()
        );

        let target = instance_base.join(&first.target_path);
        crate::util::io::create_dir_all(target.parent().unwrap())
            .await
            .unwrap();
        crate::util::io::write(&target, b"bad-target")
            .await
            .unwrap();
        let race_downloads = root.join("RaceDownloads");
        crate::util::io::create_dir_all(&race_downloads)
            .await
            .unwrap();
        crate::util::io::write(&race_downloads.join("one.bin"), first_bytes)
            .await
            .unwrap();
        let first_seen = Instant::now();
        let race_specs = vec![MissingFileCandidateSpec {
            item_id: first.item_id.clone(),
            file_name: "one.bin".to_string(),
            expected_size: first.expected_size,
        }];
        let observed = collect_download_candidates(
            job_id,
            &race_downloads,
            &race_specs,
            first_seen,
        )
        .await
        .unwrap();
        assert_eq!(observed.pending, 1);
        let (scan_result, manual_result) = tokio::join!(
            scan_missing_modpack_files_in_at(
                job_id,
                &race_downloads,
                first_seen + DOWNLOAD_STABILITY_WINDOW,
            ),
            import_missing_modpack_file(
                job_id,
                first.item_id.clone(),
                selected,
            )
        );
        let scan_result = scan_result.unwrap();
        assert!(manual_result.is_ok());
        assert!(
            scan_result.imported_item_ids.is_empty()
                || scan_result.imported_item_ids == vec![first.item_id.clone()]
        );
        let snapshot = store::get_required(job_id, &state)
            .await
            .unwrap()
            .snapshot();
        assert_eq!(snapshot.status, InstallJobStatus::WaitingForUser);
        assert_eq!(crate::util::io::read(&target).await.unwrap(), first_bytes);
        assert_eq!(
            snapshot
                .items
                .iter()
                .find(|item| item.id == first.item_id)
                .unwrap()
                .status,
            DownloadItemStatus::Completed
        );
        assert_eq!(
            list_missing_modpack_files(job_id).await.unwrap().remaining,
            1
        );
        assert!(matches!(
            snapshot.pause_reason,
            Some(InstallPauseReason::MissingRequiredContent {
                failed_files: 1,
                ..
            })
        ));
        assert!(
            import_missing_modpack_file(
                job_id,
                first.item_id,
                root.join("one-selected.bin"),
            )
            .await
            .is_ok()
        );

        let second_target = instance_base.join(&second.target_path);
        crate::util::io::write(&second_target, b"keep-this-bad-target")
            .await
            .unwrap();
        let wrong = root.join("wrong-two.bin");
        crate::util::io::write(&wrong, b"wrong--two!!")
            .await
            .unwrap();
        assert!(
            import_missing_modpack_file(job_id, second.item_id.clone(), wrong)
                .await
                .is_err()
        );
        assert_eq!(
            crate::util::io::read(&second_target).await.unwrap(),
            b"keep-this-bad-target"
        );

        let downloads = root.join("Downloads");
        crate::util::io::create_dir_all(&downloads).await.unwrap();
        crate::util::io::write(&downloads.join("two.bin"), b"wrong")
            .await
            .unwrap();
        crate::util::io::write(
            &downloads.join("two.bin.crdownload"),
            second_bytes,
        )
        .await
        .unwrap();
        let first_seen = Instant::now();
        let first_scan =
            scan_missing_modpack_files_in_at(job_id, &downloads, first_seen)
                .await
                .unwrap();
        assert!(first_scan.imported_item_ids.is_empty());
        assert_eq!(first_scan.pending_candidates, 1);

        let wrong_size_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW,
        )
        .await
        .unwrap();
        assert!(wrong_size_scan.imported_item_ids.is_empty());
        assert_eq!(
            wrong_size_scan.mismatched_item_ids,
            vec![second.item_id.clone()]
        );
        assert_eq!(wrong_size_scan.checked_candidates, 0);
        assert_eq!(
            wrong_size_scan.rejected_item_ids,
            vec![second.item_id.clone()]
        );
        assert_eq!(
            wrong_size_scan.job.status,
            InstallJobStatus::WaitingForUser
        );
        assert_eq!(
            crate::util::io::read(&second_target).await.unwrap(),
            b"keep-this-bad-target"
        );

        tokio::fs::remove_file(downloads.join("two.bin"))
            .await
            .unwrap();
        crate::util::io::write(&downloads.join("two.bin"), b"wrong--two!!")
            .await
            .unwrap();
        let rejected_modified = tokio::fs::metadata(downloads.join("two.bin"))
            .await
            .unwrap()
            .modified()
            .unwrap();
        let hash_candidate_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 2,
        )
        .await
        .unwrap();
        assert_eq!(hash_candidate_scan.pending_candidates, 1);
        assert_eq!(hash_candidate_scan.checked_candidates, 0);

        let wrong_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 3,
        )
        .await
        .unwrap();
        assert_eq!(wrong_scan.checked_candidates, 1);
        assert_eq!(wrong_scan.rejected_item_ids, vec![second.item_id.clone()]);

        let unchanged_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 4,
        )
        .await
        .unwrap();
        assert_eq!(unchanged_scan.checked_candidates, 0);
        assert_eq!(
            unchanged_scan.rejected_item_ids,
            vec![second.item_id.clone()]
        );

        tokio::fs::remove_file(downloads.join("two.bin"))
            .await
            .unwrap();
        let disappeared_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 5,
        )
        .await
        .unwrap();
        assert!(disappeared_scan.rejected_item_ids.is_empty());

        crate::util::io::write(&downloads.join("two.bin"), second_bytes)
            .await
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(downloads.join("two.bin"))
            .unwrap()
            .set_modified(rejected_modified)
            .unwrap();
        let new_candidate_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 6,
        )
        .await
        .unwrap();
        assert!(new_candidate_scan.imported_item_ids.is_empty());
        assert_eq!(new_candidate_scan.pending_candidates, 1);
        assert_eq!(new_candidate_scan.checked_candidates, 0);

        let completed_scan = scan_missing_modpack_files_in_at(
            job_id,
            &downloads,
            first_seen + DOWNLOAD_STABILITY_WINDOW * 7,
        )
        .await
        .unwrap();
        assert_eq!(
            completed_scan.imported_item_ids,
            vec![second.item_id.clone()]
        );
        assert_eq!(completed_scan.checked_candidates, 1);
        assert_ne!(completed_scan.job.status, InstallJobStatus::WaitingForUser);
        assert_eq!(
            completed_scan.job.execution_mode,
            InstallJobExecutionMode::RecoveryValidation
        );
        assert_eq!(
            crate::util::io::read(&second_target).await.unwrap(),
            second_bytes
        );
        let resumed = store::get_required(job_id, &state).await.unwrap();
        assert_eq!(
            resumed
                .state
                .events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    InstallJobEventKind::JobQueued { .. }
                ))
                .count(),
            2
        );
    }
}
