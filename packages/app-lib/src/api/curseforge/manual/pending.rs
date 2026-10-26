//! Manual download pending helpers.

use super::*;
use crate::api::curseforge::dto::{
    CurseForgeFileHash, CurseForgeManualDownload,
};
use crate::state::ContentProvider;
use crate::{ErrorKind, State};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub(super) static MANUAL_IMPORT_SCAN_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

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

pub(super) fn pending_manual_download(
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

pub(super) async fn manually_imported_curseforge_world_downloads(
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
