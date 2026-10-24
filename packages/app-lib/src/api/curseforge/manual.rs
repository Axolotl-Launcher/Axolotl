//! Manual download recognition and reconciliation.

use super::MANUAL_IMPORT_SCAN_LOCK;
use super::auth::capability;
use super::catalog::{get_file, get_projects, match_fingerprints};
use super::dto::{
    CurseForgeCapabilityStatus, CurseForgeFile, CurseForgeFileHash,
    CurseForgeInstalledFile, CurseForgeManualDownload,
    CurseForgeManualDownloadImport, CurseForgeManualDownloadImportError,
    CurseForgeManualDownloadScanResult, CurseForgeRecognitionResult,
};
use super::mapping::recognized_project_type;
use super::selection::{
    complete_manual_world_download, managed_project_type,
    manual_download_target_folder,
};
use super::validation::{validate_file_name, validate_world_archive_name};
use super::verify::{compute_fingerprint, fingerprint_and_sha1_file};
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::install::{InstallJobEventKind, InstallProgressReporter};
use crate::state::{
    ContentProvider, ContentSourceKind, CurseForgeFileId, CurseForgeProjectId,
    ProjectType,
};
use crate::util::fetch::sha1_file_async;
use crate::{ErrorKind, State};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub async fn recognize_instance_files(
    instance_id: &str,
) -> crate::Result<CurseForgeRecognitionResult> {
    let mut result = CurseForgeRecognitionResult::default();
    if capability().status != CurseForgeCapabilityStatus::Ready {
        return Ok(result);
    }

    let state = State::get().await?;
    let instance_files =
        crate::api::instance::sync_content_files(instance_id).await?;
    let tracked_file_ids = match crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    {
        Some(content_set) => {
            crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
                &content_set.id,
                &state.pool,
            )
            .await?
            .into_iter()
            .filter_map(|entry| entry.file_id)
            .collect::<HashSet<_>>()
        }
        None => HashSet::new(),
    };
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let mut fingerprints = Vec::new();
    let mut paths_by_fingerprint =
        HashMap::<u64, Vec<(String, ProjectType)>>::new();

    for file in instance_files.into_iter().filter(|file| !file.missing) {
        if tracked_file_ids.contains(&file.id) {
            continue;
        }
        let Some(project_type) =
            crate::state::instances::adapters::filesystem::project_type_from_relative_path(
                &file.relative_path,
            )
        else {
            continue;
        };
        let full_path = instance_path.join(&file.relative_path);
        let Ok(bytes) = tokio::fs::read(&full_path).await else {
            continue;
        };
        let fingerprint = compute_fingerprint(&bytes) as u64;
        fingerprints.push(fingerprint);
        paths_by_fingerprint
            .entry(fingerprint)
            .or_default()
            .push((file.relative_path, project_type));
    }

    let mut matches = HashMap::new();
    for chunk in fingerprints.chunks(1000) {
        let response = match_fingerprints(chunk.to_vec()).await?;
        for matched in response.exact_matches {
            matches.insert(matched.file.file_fingerprint, matched.file);
        }
    }

    result.scanned = fingerprints.len() as u32;
    let matched_project_ids =
        matches.values().map(|file| file.mod_id).collect::<Vec<_>>();
    let matched_projects = if matched_project_ids.is_empty() {
        HashMap::new()
    } else {
        match get_projects(matched_project_ids).await {
            Ok(projects) => projects
                .into_iter()
                .map(|project| (project.id, project))
                .collect(),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "Failed to resolve CurseForge project types while recognizing instance files"
                );
                HashMap::new()
            }
        }
    };

    for (fingerprint, paths) in paths_by_fingerprint {
        if let Some(file) = matches.get(&fingerprint) {
            for (path, path_project_type) in paths {
                let project_type = matched_projects
                    .get(&file.mod_id)
                    .and_then(|project| {
                        recognized_project_type(project.class_id)
                    })
                    .unwrap_or(path_project_type);
                let full_path = instance_path.join(&path);
                let Some((size, sha1)) = verify_recognized_curseforge_file(
                    &full_path,
                    file.file_fingerprint,
                )
                .await?
                else {
                    continue;
                };
                crate::state::record_verified_curseforge_project_file_atomic(
                    instance_id,
                    &path,
                    &sha1,
                    size,
                    project_type,
                    ContentSourceKind::CurseForge,
                    crate::state::instances::ContentOwnershipKind::UserAdded,
                    CurseForgeProjectId::new(file.mod_id)?,
                    CurseForgeFileId::new(file.id)?,
                    true,
                    &state,
                )
                .await?;
                result.linked.push(CurseForgeInstalledFile {
                    project_id: file.mod_id,
                    file_id: file.id,
                    relative_path: path,
                    dependency: false,
                });
                result.matched += 1;
            }
        } else {
            result
                .unmatched_paths
                .extend(paths.into_iter().map(|(path, _)| path));
        }
    }
    result.unmatched_paths.sort();
    Ok(result)
}

pub(super) async fn verify_recognized_curseforge_file(
    path: &Path,
    expected_fingerprint: u64,
) -> crate::Result<Option<(u64, String)>> {
    if expected_fingerprint == 0 {
        return Ok(None);
    }
    let (size, sha1, fingerprint) =
        fingerprint_and_sha1_file(path, None).await?;
    if fingerprint as u64 != expected_fingerprint {
        return Ok(None);
    }
    Ok(Some((size, sha1)))
}

pub async fn import_manual_downloads(
    instance_id: &str,
    scan_directory: Option<PathBuf>,
) -> crate::Result<CurseForgeManualDownloadScanResult> {
    let Some(download_directory) = scan_directory.or_else(dirs::download_dir)
    else {
        finalize_curseforge_manual_download_import(instance_id, false).await?;
        return Ok(CurseForgeManualDownloadScanResult::default());
    };
    let mut result = CurseForgeManualDownloadScanResult {
        download_directory: Some(download_directory.to_string_lossy().into()),
        ..Default::default()
    };
    let downloads = list_pending_manual_downloads(instance_id).await?;

    for download in downloads {
        let candidate = match find_manual_download_candidate(
            &download_directory,
            &download,
        )
        .await
        {
            Ok(candidate) => candidate,
            Err(error) => {
                result.errors.push(CurseForgeManualDownloadImportError {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    message: error.to_string(),
                });
                continue;
            }
        };
        let Some((source_path, size, sha1)) = candidate else {
            continue;
        };

        match install_manual_download(
            instance_id,
            &download,
            &source_path,
            size,
            &sha1,
        )
        .await
        {
            Ok(relative_path) => {
                result.imported.push(CurseForgeManualDownloadImport {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    relative_path,
                });
            }
            Err(error) => {
                result.errors.push(CurseForgeManualDownloadImportError {
                    project_id: download.project_id,
                    file_id: download.file_id,
                    message: error.to_string(),
                });
            }
        }
    }

    finalize_curseforge_manual_download_import(
        instance_id,
        !result.imported.is_empty(),
    )
    .await?;

    Ok(result)
}

pub async fn configure_manual_download_watcher(
    enabled: bool,
    scan_directory: Option<PathBuf>,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let directory = if enabled {
        let Some(directory) = scan_directory.or_else(dirs::download_dir) else {
            state
                .file_watcher
                .configure_manual_import_directory(None)
                .await?;
            return Ok(None);
        };
        let directory = tokio::fs::canonicalize(directory).await?;
        if !tokio::fs::metadata(&directory).await?.is_dir() {
            return Err(ErrorKind::InputError(
                "Manual import watch path is not a directory".to_string(),
            )
            .into());
        }
        Some(directory)
    } else {
        None
    };

    state
        .file_watcher
        .configure_manual_import_directory(directory.clone())
        .await?;
    Ok(directory.map(|path| path.to_string_lossy().into_owned()))
}

pub(crate) async fn scan_pending_manual_downloads_in(
    download_directory: &Path,
) -> crate::Result<()> {
    let _scan_guard = MANUAL_IMPORT_SCAN_LOCK.lock().await;
    let state = State::get().await?;
    let instance_ids = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT instance_id
         FROM instance_pending_manual_downloads
         WHERE provider = 'curseforge'
            AND state IN ('waiting', 'matched', 'error')",
    )
    .fetch_all(&state.pool)
    .await?;

    for instance_id in instance_ids {
        let result = import_manual_downloads(
            &instance_id,
            Some(download_directory.to_path_buf()),
        )
        .await?;
        for error in result.errors {
            tracing::debug!(
                instance_id,
                project_id = error.project_id,
                file_id = error.file_id,
                "Pending manual download not ready: {}",
                error.message
            );
        }
    }

    let waiting_instances = crate::install::store::list(false, &state)
        .await?
        .into_iter()
        .filter(|job| {
            job.status == crate::install::InstallJobStatus::WaitingForUser
                && is_curseforge_manual_download_job(&job.state)
        })
        .filter_map(|job| job.instance_id)
        .collect::<HashSet<_>>();
    for instance_id in waiting_instances {
        reconcile_curseforge_waiting_jobs_for_instance(&instance_id).await?;
    }

    Ok(())
}

pub async fn list_pending_manual_downloads(
    instance_id: &str,
) -> crate::Result<Vec<CurseForgeManualDownload>> {
    let state = State::get().await?;
    Ok(crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
		instance_id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter_map(pending_manual_download)
	.collect())
}

fn pending_manual_download(
    item: crate::state::instances::PendingManualDownload,
) -> Option<CurseForgeManualDownload> {
    if item.provider != ContentProvider::CurseForge {
        return None;
    }
    serde_json::from_value::<CurseForgeManualDownload>(item.context.clone())
        .ok()
        .or_else(|| {
            Some(CurseForgeManualDownload {
                project_id: item.provider_project_id.parse().ok()?,
                file_id: item.provider_release_id.parse().ok()?,
                file_name: item.file_name,
                ownership_kind: if item.pack_member_id.is_some() {
                    crate::state::instances::ContentOwnershipKind::PackManaged
                } else {
                    Default::default()
                },
                operation_kind: item.operation_kind,
                website_url: item.website_url,
                project_type: item.project_type.get_name().to_string(),
                project_slug: String::new(),
                target_folder: Path::new(&item.target_relative_path)
                    .parent()
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default(),
                hashes: item
                    .expected_sha1
                    .map(|value| vec![CurseForgeFileHash { value, algo: 1 }])
                    .unwrap_or_default(),
                file_length: item.expected_size.unwrap_or_default(),
                file_fingerprint: item.expected_fingerprint.unwrap_or_default(),
            })
        })
}

#[derive(Clone, Debug)]
pub(super) struct CurseForgeManualDownloadIntegrityMetadata {
    pub(super) project_id: u32,
    pub(super) file_id: u32,
    pub(super) hashes: Vec<CurseForgeFileHash>,
    pub(super) file_length: u64,
    pub(super) file_fingerprint: u64,
}

impl From<CurseForgeFile> for CurseForgeManualDownloadIntegrityMetadata {
    fn from(file: CurseForgeFile) -> Self {
        Self {
            project_id: file.mod_id,
            file_id: file.id,
            hashes: file.hashes,
            file_length: file.file_length,
            file_fingerprint: file.file_fingerprint,
        }
    }
}

fn manual_download_has_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> bool {
    download
        .hashes
        .iter()
        .any(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        || download.file_fingerprint != 0
}

async fn ensure_manual_download_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> crate::Result<CurseForgeManualDownload> {
    ensure_manual_download_integrity_metadata_with(
        download,
        resolve_manual_download_integrity_metadata,
    )
    .await
}

async fn resolve_manual_download_integrity_metadata(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeManualDownloadIntegrityMetadata> {
    Ok(get_file(project_id, file_id).await?.into())
}

pub(super) async fn ensure_manual_download_integrity_metadata_with<F, Fut>(
    download: &CurseForgeManualDownload,
    resolve_metadata: F,
) -> crate::Result<CurseForgeManualDownload>
where
    F: FnOnce(u32, u32) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<CurseForgeManualDownloadIntegrityMetadata>,
        >,
{
    if manual_download_has_integrity_metadata(download) {
        return Ok(download.clone());
    }

    let metadata =
        resolve_metadata(download.project_id, download.file_id).await?;
    if metadata.project_id != download.project_id
        || metadata.file_id != download.file_id
    {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }

    let mut hydrated = download.clone();
    hydrated.hashes = metadata.hashes;
    hydrated.file_length = metadata.file_length;
    hydrated.file_fingerprint = metadata.file_fingerprint;
    if !manual_download_has_integrity_metadata(&hydrated) {
        return Err(ErrorKind::InputError(
            "The required CurseForge file has no usable integrity metadata"
                .to_string(),
        )
        .into());
    }
    Ok(hydrated)
}

pub(super) async fn find_manual_download_candidate(
    download_directory: &Path,
    download: &CurseForgeManualDownload,
) -> crate::Result<Option<(std::path::PathBuf, u64, String)>> {
    validate_file_name(&download.file_name)?;
    let mut entries = match tokio::fs::read_dir(download_directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };

    let mut verified_download = None;
    while let Some(entry) = entries.next_entry().await? {
        let actual_file_name = entry.file_name().to_string_lossy().to_string();
        if !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        ) {
            continue;
        }
        let path = entry.path();
        tracing::debug!(
            project_id = download.project_id,
            file_id = download.file_id,
            candidate_path = %path.display(),
            "Found CurseForge manual download filename candidate"
        );
        if verified_download.is_none() {
            verified_download = Some(
                ensure_manual_download_integrity_metadata(download).await?,
            );
        }
        match verify_manual_download_candidate_with_integrity(
            &path,
            verified_download
                .as_ref()
                .expect("integrity metadata resolved"),
            true,
        )
        .await
        {
            Ok(Some((size, sha1))) => {
                return Ok(Some((path, size, sha1)));
            }
            Ok(None) => continue,
            Err(error) => {
                tracing::debug!(
                    project_id = download.project_id,
                    file_id = download.file_id,
                    candidate_path = %path.display(),
                    %error,
                    "Unable to inspect CurseForge manual download candidate"
                );
                continue;
            }
        }
    }

    Ok(None)
}

async fn verify_manual_download_candidate(
    path: &Path,
    download: &CurseForgeManualDownload,
    require_matching_name: bool,
) -> crate::Result<Option<(u64, String)>> {
    let download = ensure_manual_download_integrity_metadata(download).await?;
    verify_manual_download_candidate_with_integrity(
        path,
        &download,
        require_matching_name,
    )
    .await
}

pub(super) async fn verify_manual_download_candidate_with_integrity(
    path: &Path,
    download: &CurseForgeManualDownload,
    require_matching_name: bool,
) -> crate::Result<Option<(u64, String)>> {
    let actual_file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if require_matching_name
        && !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        )
    {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "name_mismatch",
        );
        return Ok(None);
    }
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if crate::util::io::is_symlink_or_reparse(&metadata) {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "symlink_or_reparse",
        );
        return Ok(None);
    }
    if !metadata.is_file() {
        trace_manual_download_candidate_rejection(path, download, "non_file");
        return Ok(None);
    }
    if download.file_length > 0 && metadata.len() != download.file_length {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "size_mismatch",
        );
        return Ok(None);
    }

    if let Some(expected_sha1) = download
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        .map(|hash| hash.value.as_str())
    {
        let (size, sha1) = sha1_file_async(path).await?;
        if !sha1.eq_ignore_ascii_case(expected_sha1) {
            trace_manual_download_candidate_rejection(
                path,
                download,
                "sha1_mismatch",
            );
            return Ok(None);
        }
        return Ok(Some((size, sha1)));
    }
    if download.file_fingerprint == 0 {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "missing_integrity_metadata",
        );
        return Err(ErrorKind::InputError(
            "The required CurseForge file has no usable integrity metadata"
                .to_string(),
        )
        .into());
    }
    let (size, sha1, fingerprint) =
        fingerprint_and_sha1_file(path, None).await?;
    if fingerprint as u64 != download.file_fingerprint {
        trace_manual_download_candidate_rejection(
            path,
            download,
            "fingerprint_mismatch",
        );
        return Ok(None);
    }
    Ok(Some((size, sha1)))
}

fn trace_manual_download_candidate_rejection(
    path: &Path,
    download: &CurseForgeManualDownload,
    reason: &'static str,
) {
    tracing::trace!(
        project_id = download.project_id,
        file_id = download.file_id,
        candidate_path = %path.display(),
        expected_file_name = %download.file_name,
        rejection_reason = reason,
        "Rejected CurseForge manual download candidate"
    );
}

pub(crate) async fn import_pending_manual_download_from_path(
    instance_id: &str,
    source_path: &Path,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        instance_id,
        &state.pool,
    )
    .await?;
    let actual_file_name = source_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut matched_pending_name = false;
    for download in pending.into_iter().filter_map(pending_manual_download) {
        if !crate::util::downloads::browser_download_file_name_matches(
            &actual_file_name,
            &download.file_name,
        ) {
            continue;
        }
        matched_pending_name = true;
        let Some((size, sha1)) =
            verify_manual_download_candidate(source_path, &download, true)
                .await?
        else {
            continue;
        };
        let relative_path = install_manual_download(
            instance_id,
            &download,
            source_path,
            size,
            &sha1,
        )
        .await?;
        finalize_curseforge_manual_download_import(instance_id, false).await?;
        return Ok(Some(relative_path));
    }
    if matched_pending_name {
        return Err(ErrorKind::InputError(
            "The selected file does not match the required CurseForge file"
                .to_string(),
        )
        .into());
    }
    Ok(None)
}

pub async fn import_pending_manual_download_file(
    instance_id: &str,
    project_id: u32,
    file_id: u32,
    source_path: PathBuf,
) -> crate::Result<CurseForgeManualDownloadImport> {
    import_pending_manual_download_file_with_integrity_resolver(
        instance_id,
        project_id,
        file_id,
        source_path,
        resolve_manual_download_integrity_metadata,
    )
    .await
}

pub(super) async fn import_pending_manual_download_file_with_integrity_resolver<
    F,
    Fut,
>(
    instance_id: &str,
    project_id: u32,
    file_id: u32,
    source_path: PathBuf,
    resolve_metadata: F,
) -> crate::Result<CurseForgeManualDownloadImport>
where
    F: FnOnce(u32, u32) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<CurseForgeManualDownloadIntegrityMetadata>,
        >,
{
    tracing::debug!(
        instance_id,
        project_id,
        file_id,
        source_path = %source_path.display(),
        "Importing selected CurseForge manual download file"
    );
    let pending = list_pending_manual_downloads(instance_id).await?;
    let pending_count = pending.len();
    let download = pending.into_iter().find(|download| {
        download.project_id == project_id && download.file_id == file_id
    });
    tracing::debug!(
        instance_id,
        project_id,
        file_id,
        pending_count,
        pending_found = download.is_some(),
        "Looked up selected CurseForge pending manual download"
    );
    let download = download.ok_or_else(|| {
        ErrorKind::InputError(
            "The selected CurseForge file is not pending for this instance"
                .to_string(),
        )
    })?;
    let download = ensure_manual_download_integrity_metadata_with(
        &download,
        resolve_metadata,
    )
    .await?;
    let Some((size, sha1)) = verify_manual_download_candidate_with_integrity(
        &source_path,
        &download,
        false,
    )
    .await?
    else {
        return Err(ErrorKind::InputError(
            "The selected file does not match the required CurseForge file"
                .to_string(),
        )
        .into());
    };
    let relative_path = install_manual_download(
        instance_id,
        &download,
        &source_path,
        size,
        &sha1,
    )
    .await?;
    finalize_curseforge_manual_download_import(instance_id, true).await?;
    Ok(CurseForgeManualDownloadImport {
        project_id,
        file_id,
        relative_path,
    })
}

async fn finalize_curseforge_manual_download_import(
    instance_id: &str,
    emit_content_changed: bool,
) -> crate::Result<()> {
    let reconciliation =
        reconcile_curseforge_waiting_jobs_for_instance(instance_id).await;
    let content_changed = if emit_content_changed {
        crate::api::instance::emit_content_changed(instance_id).await
    } else {
        Ok(())
    };
    reconciliation?;
    content_changed
}

pub(crate) async fn reconcile_curseforge_waiting_jobs_for_instance(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        instance_id,
        &state,
    )
    .await
}

pub(crate) async fn reconcile_persisted_curseforge_waiting_jobs(
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_persisted_curseforge_waiting_jobs_with_resume(
        state,
        &mut resume_job,
    )
    .await
}

pub(super) async fn reconcile_persisted_curseforge_waiting_jobs_with_resume<
    F,
    Fut,
>(
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let jobs = crate::install::store::list(false, state).await?;
    for instance_id in curseforge_waiting_job_instance_ids(&jobs) {
        reconcile_curseforge_waiting_jobs_for_instance_with_resume(
            &instance_id,
            state,
            resume_job,
        )
        .await?;
    }
    Ok(())
}

pub(super) fn curseforge_waiting_job_instance_ids(
    jobs: &[crate::install::store::InstallJobRecord],
) -> Vec<String> {
    let mut instance_ids = jobs
        .iter()
        .filter(|job| is_reconcilable_curseforge_waiting_job(job))
        .filter_map(|job| job.instance_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    instance_ids.sort_unstable();
    instance_ids
}

pub(super) async fn reconcile_curseforge_waiting_jobs_for_instance_with_state(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_curseforge_waiting_jobs_for_instance_with_resume(
        instance_id,
        state,
        &mut resume_job,
    )
    .await
}

async fn resume_curseforge_install_job(
    job_id: uuid::Uuid,
) -> crate::Result<()> {
    crate::install::runner::resume_job(job_id).await?;
    Ok(())
}

async fn reconcile_curseforge_waiting_jobs_for_instance_with_resume<F, Fut>(
    instance_id: &str,
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
		instance_id,
		&state.pool,
	)
	.await?
	.into_iter()
	.filter(|download| download.provider == ContentProvider::CurseForge)
	.map(|download| {
		(download.provider_project_id, download.provider_release_id)
	})
	.collect::<HashSet<_>>();
    let materialized = crate::state::instances::adapters::sqlite::content_rows::get_materialized_curseforge_downloads(
        instance_id,
        &state.pool,
    )
    .await?
    .into_iter()
    .collect::<HashSet<_>>();

    let jobs = crate::install::store::list(false, state).await?;
    let waiting_job_count = jobs
        .iter()
        .filter(|job| {
            job.instance_id.as_deref() == Some(instance_id)
                && is_reconcilable_curseforge_waiting_job(job)
        })
        .count();
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        "Starting CurseForge waiting install job reconciliation"
    );
    let mut recovered_item_count = 0usize;
    let mut resume_job_called = false;
    for job in jobs {
        if job.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&job)
        {
            continue;
        }

        let operation_lock = state
            .install_job_operation_locks
            .entry(job.id)
            .or_default()
            .clone();
        let _operation_guard = operation_lock.lock().await;
        let current =
            crate::install::store::get_required(job.id, state).await?;
        if current.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&current)
        {
            continue;
        }

        let mut reconciled_materialized = materialized.clone();
        reconciled_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &current.state,
            )
            .await?,
        );
        let reconciliation = curseforge_manual_download_reconciliation(
            &current.state.download_items(),
            &pending,
            &reconciled_materialized,
        );
        for inconsistent in &reconciliation.inconsistent {
            tracing::warn!(
                job_id = %current.id,
                instance_id,
                project_id = %inconsistent.project_id,
                file_id = %inconsistent.file_id,
                item_path = %inconsistent.path,
                reason = inconsistent.reason,
                "CurseForge manual download job state is inconsistent"
            );
        }

        let recovered_count = reconciliation.recovered.len();
        recovered_item_count += recovered_count;
        if recovered_count > 0 {
            InstallProgressReporter::new(current.id, current.state)
                .record_events(reconciliation.recovered)
                .await?;
        }

        let latest = crate::install::store::get_required(job.id, state).await?;
        if latest.status != crate::install::InstallJobStatus::WaitingForUser {
            continue;
        }
        let mut latest_materialized = materialized.clone();
        latest_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &latest.state,
            )
            .await?,
        );
        let latest_reconciliation = curseforge_manual_download_reconciliation(
            &latest.state.download_items(),
            &pending,
            &latest_materialized,
        );
        tracing::debug!(
            job_id = %latest.id,
            instance_id,
            pending_count = pending.len(),
            manual_skipped_count = reconciliation.manual_skipped_count,
            materialized_exact_match_count =
                reconciliation.materialized_exact_match_count,
            recovered_count,
            unresolved_pending_count =
                latest_reconciliation.unresolved_pending_count,
            inconsistent_count = latest_reconciliation.inconsistent.len(),
            "Reconciled CurseForge waiting install job"
        );

        if latest_reconciliation.should_resume() {
            resume_job_called = true;
            match resume_job(latest.id).await {
                Ok(_) => {
                    tracing::info!(
                        job_id = %latest.id,
                        instance_id,
                        pending_count = pending.len(),
                        manual_skipped_count =
                            reconciliation.manual_skipped_count,
                        materialized_exact_match_count =
                            reconciliation.materialized_exact_match_count,
                        recovered_count,
                        unresolved_pending_count = 0,
                        inconsistent_count = 0,
                        "Reconciled completed CurseForge manual downloads and resuming install job"
                    );
                }
                Err(error) => {
                    let current =
                        crate::install::store::get_required(latest.id, state)
                            .await?;
                    if current.status
                        == crate::install::InstallJobStatus::WaitingForUser
                    {
                        return Err(error);
                    }
                }
            }
        }
    }
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        recovered_item_count,
        resume_job_called,
        "Completed CurseForge waiting install job reconciliation"
    );
    Ok(())
}

#[derive(Debug)]
pub(super) struct CurseForgeManualDownloadInconsistency {
    pub(super) path: String,
    pub(super) project_id: String,
    pub(super) file_id: String,
    pub(super) reason: &'static str,
}

#[derive(Debug, Default)]
pub(super) struct CurseForgeManualDownloadReconciliation {
    pub(super) recovered: Vec<InstallJobEventKind>,
    pub(super) inconsistent: Vec<CurseForgeManualDownloadInconsistency>,
    pub(super) manual_skipped_count: usize,
    pub(super) materialized_exact_match_count: usize,
    pub(super) unresolved_pending_count: usize,
}

impl CurseForgeManualDownloadReconciliation {
    pub(super) fn should_resume(&self) -> bool {
        self.unresolved_pending_count == 0 && self.inconsistent.is_empty()
    }
}

pub(super) fn curseforge_manual_download_reconciliation(
    items: &[crate::install::model::DownloadItemSnapshot],
    pending: &HashSet<(String, String)>,
    materialized: &HashSet<(String, String)>,
) -> CurseForgeManualDownloadReconciliation {
    let mut result = CurseForgeManualDownloadReconciliation::default();
    for item in items.iter().filter(|item| {
        item.status == crate::install::model::DownloadItemStatus::Skipped
            && item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    }) {
        result.manual_skipped_count += 1;
        let project_id = item.project_id.as_ref().expect("filtered project ID");
        let version_id = item.version_id.as_ref().expect("filtered file ID");
        if pending.contains(&(project_id.clone(), version_id.clone())) {
            result.unresolved_pending_count += 1;
        } else if materialized
            .contains(&(project_id.clone(), version_id.clone()))
        {
            result.materialized_exact_match_count += 1;
            result
                .recovered
                .push(InstallJobEventKind::ContentFileRecovered {
                    path: item.id.clone(),
                    bytes: item.bytes_total.unwrap_or(0),
                });
        } else {
            result
                .inconsistent
                .push(CurseForgeManualDownloadInconsistency {
                    path: item.id.clone(),
                    project_id: project_id.clone(),
                    file_id: version_id.clone(),
                    reason: "pending_missing_but_not_materialized",
                });
        }
    }
    result
}

fn is_reconcilable_curseforge_waiting_job(
    job: &crate::install::store::InstallJobRecord,
) -> bool {
    job.status == crate::install::InstallJobStatus::WaitingForUser
        && job.instance_id.is_some()
        && is_curseforge_manual_download_job(&job.state)
        && matches!(
            &job.state.pause_reason,
            Some(crate::install::model::InstallPauseReason::MissingRequiredContent { .. })
        )
        && job.state.download_items().iter().any(|item| {
            item.status == crate::install::model::DownloadItemStatus::Skipped
                && item.manual_url.is_some()
                && item.project_id.is_some()
                && item.version_id.is_some()
        })
}

fn is_curseforge_manual_download_job(
    job: &crate::install::model::InstallJobState,
) -> bool {
    if !matches!(
        job.provider(),
        crate::install::model::InstallJobProvider::CurseForge
            | crate::install::model::InstallJobProvider::Local
    ) {
        return false;
    }
    job.download_items().iter().any(|item| {
        item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    })
}

async fn manually_imported_curseforge_world_downloads(
    instance_id: &str,
    job_state: &crate::install::model::InstallJobState,
) -> crate::Result<HashSet<(String, String)>> {
    let crate::install::model::InstallRequest::InstallCurseForgeWorld {
        request,
        ..
    } = &job_state.request
    else {
        return Ok(HashSet::new());
    };
    let project_id = request.project_id.to_string();
    let file_id = request.file_id.to_string();
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let imported = job_state.download_items().into_iter().any(|item| {
        if item.status != crate::install::model::DownloadItemStatus::Skipped
            || item.project_id.as_deref() != Some(project_id.as_str())
            || item.version_id.as_deref() != Some(file_id.as_str())
        {
            return false;
        }
        let Some(file_name) = Path::new(&item.id).file_name() else {
            return false;
        };
        let Some(world_name) = Path::new(file_name).file_stem() else {
            return false;
        };
        instance_path
            .join("saves")
            .join(world_name)
            .join("level.dat")
            .is_file()
    });
    Ok(if imported {
        HashSet::from([(project_id, file_id)])
    } else {
        Default::default()
    })
}

async fn install_manual_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
    source_path: &Path,
    size: u64,
    sha1: &str,
) -> crate::Result<String> {
    if download.project_type == "modpack" {
        let verified_directory = tempfile::tempdir()?;
        let verified_source =
            verified_directory.path().join("manual-download.pack");
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &verified_source,
            size,
            sha1,
        )
        .await?;
        crate::install::install_pack_to_existing_instance(
            instance_id.to_string(),
            crate::api::pack::install_from::CreatePackLocation::FromFile {
                path: verified_source,
            },
            None,
        )
        .await?;
        let state = State::get().await?;
        let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			instance_id,
			&state.pool,
		)
		.await?
		.ok_or_else(|| {
			ErrorKind::InputError(
				"Instance has no applied content set".to_string(),
			)
		})?;
        let _instance_lock = state.lock_instance_content(instance_id).await;
        let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
        crate::state::instances::adapters::sqlite::content_rows::complete_pending_manual_download(
			instance_id,
			&download.project_id.to_string(),
			&download.file_id.to_string(),
			None,
			&mut tx,
		)
		.await?;
        crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
			&content_set.id,
			&mut tx,
        )
		.await?;
        tx.commit().await?;
        let relative_path = source_path.to_string_lossy().to_string();
        tracing::info!(
            instance_id,
            project_id = download.project_id,
            file_id = download.file_id,
            relative_path = %relative_path,
            "Completed CurseForge pending manual download record"
        );
        return Ok(relative_path);
    }
    if download.project_type == "world" {
        validate_world_archive_name(&download.file_name)?;
        let verified_directory = tempfile::tempdir()?;
        let verified_source = verified_directory.path().join(
            crate::util::io::sanitize_download_file_name(&download.file_name)?,
        );
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &verified_source,
            size,
            sha1,
        )
        .await?;
        let state = State::get().await?;
        let world_name = crate::state::instances::commands::import_world_save(
            &state,
            instance_id,
            &verified_source,
            None,
        )
        .await?;
        complete_manual_world_download(instance_id, download).await?;
        return Ok(format!("saves/{world_name}"));
    }
    let project_type = managed_project_type(&download.project_type)?;
    let target_folder = manual_download_target_folder(download, project_type)?;
    let file_name =
        crate::util::io::sanitize_download_file_name(&download.file_name)?;
    let state = State::get().await?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        chinese_file_title_for_curseforge_slug(&download.project_slug)
            .and_then(|title| localized_content_file_name(&file_name, &title))
            .map(|file_name| format!("{target_folder}/{file_name}"))
    };
    let relative_path = crate::state::resolve_content_install_relative_path(
        instance_id,
        format!("{target_folder}/{file_name}"),
        localized_candidate,
        &state.pool,
    )
    .await?;
    let full_path = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&relative_path);
    let previous_path =
        crate::state::materialize_verified_project_download_copy(
            source_path,
            &full_path,
            size,
            sha1,
        )
        .await?;
    let record_result =
        crate::state::record_verified_curseforge_project_file_atomic(
            instance_id,
            &relative_path,
            sha1,
            size,
            project_type,
            ContentSourceKind::CurseForge,
            download.ownership_kind,
            CurseForgeProjectId::new(download.project_id)?,
            CurseForgeFileId::new(download.file_id)?,
            true,
            &state,
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
    tracing::info!(
        instance_id,
        project_id = download.project_id,
        file_id = download.file_id,
        relative_path = %relative_path,
        "Completed CurseForge pending manual download record"
    );
    if download.ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
        && download.operation_kind
            == crate::state::instances::ManualDownloadOperationKind::ContentUpdate
    {
        mark_pack_member_version_override(instance_id, &relative_path).await?;
    }
    Ok(relative_path)
}

pub(crate) async fn mark_pack_member_version_override(
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let updated = sqlx::query(
        "UPDATE instance_pack_members
         SET override_kind = 'version', modified_at = ?
         WHERE content_entry_id IN (
            SELECT entry.id
            FROM instance_content_entries entry
            INNER JOIN instance_files file ON file.id = entry.file_id
            WHERE entry.content_set_id = ? AND file.relative_path = ?
         )",
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(&content_set.id)
    .bind(relative_path)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
            &content_set.id,
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
