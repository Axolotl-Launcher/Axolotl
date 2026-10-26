//! Manual download recognize helpers.

use crate::api::curseforge::auth::capability;
use crate::api::curseforge::catalog::{
    get_file, get_projects, match_fingerprints,
};
use crate::api::curseforge::dto::{
    CurseForgeCapabilityStatus, CurseForgeFile, CurseForgeFileHash,
    CurseForgeInstalledFile, CurseForgeManualDownload,
    CurseForgeRecognitionResult,
};
use crate::api::curseforge::mapping::recognized_project_type;
use crate::api::curseforge::validation::validate_file_name;
use crate::api::curseforge::verify::{
    compute_fingerprint, fingerprint_and_sha1_file,
};
use crate::state::{
    ContentSourceKind, CurseForgeFileId, CurseForgeProjectId, ProjectType,
};
use crate::util::fetch::sha1_file_async;
use crate::{ErrorKind, State};
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

pub(crate) async fn verify_recognized_curseforge_file(
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

#[derive(Clone, Debug)]
pub(crate) struct CurseForgeManualDownloadIntegrityMetadata {
    pub(crate) project_id: u32,
    pub(crate) file_id: u32,
    pub(crate) hashes: Vec<CurseForgeFileHash>,
    pub(crate) file_length: u64,
    pub(crate) file_fingerprint: u64,
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

pub(super) fn manual_download_has_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> bool {
    download
        .hashes
        .iter()
        .any(|hash| hash.algo == 1 && !hash.value.trim().is_empty())
        || download.file_fingerprint != 0
}

pub(super) async fn ensure_manual_download_integrity_metadata(
    download: &CurseForgeManualDownload,
) -> crate::Result<CurseForgeManualDownload> {
    ensure_manual_download_integrity_metadata_with(
        download,
        resolve_manual_download_integrity_metadata,
    )
    .await
}

pub(super) async fn resolve_manual_download_integrity_metadata(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeManualDownloadIntegrityMetadata> {
    Ok(get_file(project_id, file_id).await?.into())
}

pub(crate) async fn ensure_manual_download_integrity_metadata_with<F, Fut>(
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

pub(crate) async fn find_manual_download_candidate(
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

pub(super) async fn verify_manual_download_candidate(
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

pub(crate) async fn verify_manual_download_candidate_with_integrity(
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

pub(super) fn trace_manual_download_candidate_rejection(
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
