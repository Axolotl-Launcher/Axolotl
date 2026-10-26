//! Manual download import helpers.

use super::*;
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::api::curseforge::dto::{
    CurseForgeManualDownload, CurseForgeManualDownloadImport,
    CurseForgeManualDownloadImportError, CurseForgeManualDownloadScanResult,
};
use crate::api::curseforge::selection::{
    complete_manual_world_download, managed_project_type,
    manual_download_target_folder,
};
use crate::api::curseforge::validation::validate_world_archive_name;
use crate::state::{
    ContentSourceKind, CurseForgeFileId, CurseForgeProjectId, ProjectType,
};
use crate::{ErrorKind, State};
use std::path::{Path, PathBuf};

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

pub(crate) async fn import_pending_manual_download_file_with_integrity_resolver<
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

pub(super) async fn finalize_curseforge_manual_download_import(
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

pub(super) async fn install_manual_download(
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
