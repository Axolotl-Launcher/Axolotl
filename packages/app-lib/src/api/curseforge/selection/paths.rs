//! Install target folders and manual download materialization.

use super::downloads::curseforge_file_page_url;
use crate::api::curseforge::dto::{
    CurseForgeFile, CurseForgeInstallResult, CurseForgeInstalledFile,
    CurseForgeManualDownload, CurseForgeProject, CurseForgeWorldInstallResult,
};
use crate::api::curseforge::install::install_world_file;
use crate::api::curseforge::validation::{
    validate_file_name, validate_world_archive_name,
};
use crate::install::InstallProgressReporter;
use crate::state::{ContentProvider, ProjectType};
use crate::{ErrorKind, State};
use std::path::{Component, Path};

pub(crate) fn manual_download_target_folder(
    download: &CurseForgeManualDownload,
    project_type: ProjectType,
) -> crate::Result<String> {
    let target_folder = if download.target_folder.is_empty() {
        project_type.get_folder()
    } else {
        &download.target_folder
    };
    if target_folder == project_type.get_folder() {
        return Ok(target_folder.to_string());
    }
    let components = Path::new(target_folder).components().collect::<Vec<_>>();
    if project_type == ProjectType::DataPack
        && components.len() == 3
        && matches!(components[0], Component::Normal(value) if value == "saves")
        && matches!(components[1], Component::Normal(value) if !value.is_empty())
        && matches!(components[2], Component::Normal(value) if value == "datapacks")
    {
        return Ok(target_folder.to_string());
    }
    Err(ErrorKind::InputError(
        "CurseForge manual download has an invalid target folder".to_string(),
    )
    .into())
}

pub(crate) fn modpack_target_folder(
    project_type: &str,
) -> crate::Result<String> {
    if project_type == "world" {
        Ok("saves".into())
    } else {
        content_target_folder(managed_project_type(project_type)?, None)
    }
}

pub(crate) async fn install_modpack_world_file(
    instance_id: &str,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    reporter: Option<&InstallProgressReporter>,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> crate::Result<CurseForgeInstallResult> {
    validate_world_archive_name(&file.file_name)?;
    if project.class_id != Some(17) || file.mod_id != project.id {
        return Err(ErrorKind::InputError(
            "World file does not belong to the selected world project".into(),
        )
        .into());
    }
    let world_name = Path::new(&file.file_name)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let path = format!("saves/{world_name}");
    let target = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&path);
    let world = if target.join("level.dat").is_file() {
        // Resuming a pack or changing its version must preserve the player's save.
        CurseForgeWorldInstallResult {
            world_name: Some(world_name.into_owned()),
            manual_download: None,
        }
    } else {
        install_world_file(
            instance_id,
            project,
            file,
            reporter,
            crate::state::instances::ContentOwnershipKind::UserAdded,
            operation,
        )
        .await?
    };
    Ok(CurseForgeInstallResult {
        installed: world
            .world_name
            .map(|name| CurseForgeInstalledFile {
                project_id: project.id,
                file_id: file.id,
                relative_path: format!("saves/{name}"),
                dependency: false,
            })
            .into_iter()
            .collect(),
        manual_downloads: world.manual_download.into_iter().collect(),
        ..Default::default()
    })
}

pub(crate) fn managed_project_type(value: &str) -> crate::Result<ProjectType> {
    match value {
        "mod" => Ok(ProjectType::Mod),
        "datapack" => Ok(ProjectType::DataPack),
        "resourcepack" => Ok(ProjectType::ResourcePack),
        "shader" | "shaderpack" => Ok(ProjectType::ShaderPack),
        other => Err(ErrorKind::InputError(format!(
            "CurseForge project type {other} uses its dedicated installer"
        ))
        .into()),
    }
}

pub(crate) fn content_target_folder(
    project_type: ProjectType,
    world_name: Option<&str>,
) -> crate::Result<String> {
    if project_type == ProjectType::DataPack
        && let Some(world_name) = world_name
    {
        validate_file_name(world_name)?;
        Ok(format!("saves/{world_name}/datapacks"))
    } else {
        Ok(project_type.get_folder().to_string())
    }
}

pub(crate) fn manual_download_from_file(
    project_id: u32,
    file_id: u32,
    file: &CurseForgeFile,
    project: &CurseForgeProject,
    project_type: &str,
    target_folder: String,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    operation_kind: crate::state::instances::ManualDownloadOperationKind,
) -> CurseForgeManualDownload {
    CurseForgeManualDownload {
        project_id,
        file_id,
        file_name: file.file_name.clone(),
        ownership_kind,
        operation_kind,
        website_url: curseforge_file_page_url(
            project.links.website_url.as_deref(),
            file_id,
        ),
        project_type: project_type.to_string(),
        project_slug: project.slug.clone(),
        target_folder,
        hashes: file.hashes.clone(),
        file_length: file.file_length,
        file_fingerprint: file.file_fingerprint,
    }
}

async fn persist_manual_modpack_archive(
    instance_id: &str,
    download: &CurseForgeManualDownload,
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
    let now = chrono::Utc::now();
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
        &crate::state::instances::PendingManualDownload {
            id: format!("manual-download:{}", uuid::Uuid::new_v4()),
            instance_id: instance_id.to_string(),
            pack_member_id: None,
            content_entry_id: None,
            operation_kind: download.operation_kind,
            operation_target_id: Some(download.file_id.to_string()),
            project_type: ProjectType::Mod,
            provider: ContentProvider::CurseForge,
            provider_project_id: download.project_id.to_string(),
            provider_release_id: download.file_id.to_string(),
            file_name: download.file_name.clone(),
            website_url: download.website_url.clone(),
            target_relative_path: download.file_name.clone(),
            expected_sha1: download
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
            expected_size: (download.file_length > 0)
                .then_some(download.file_length),
            expected_fingerprint: (download.file_fingerprint > 0)
                .then_some(download.file_fingerprint),
            state: crate::state::instances::ManualDownloadState::Waiting,
            context: serde_json::to_value(download)?,
            created_at: now,
            modified_at: now,
        },
        &mut tx,
    )
    .await?;
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
        &content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn persist_manual_world_archive(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    validate_world_archive_name(&download.file_name)?;
    let state = State::get().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let now = chrono::Utc::now();
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
        &crate::state::instances::PendingManualDownload {
            id: format!("manual-download:{}", uuid::Uuid::new_v4()),
            instance_id: instance_id.to_string(),
            pack_member_id: None,
            content_entry_id: None,
            operation_kind: download.operation_kind,
            operation_target_id: None,
            project_type: ProjectType::WorldSave,
            provider: ContentProvider::CurseForge,
            provider_project_id: download.project_id.to_string(),
            provider_release_id: download.file_id.to_string(),
            file_name: download.file_name.clone(),
            website_url: download.website_url.clone(),
            target_relative_path: format!("saves/{}", download.file_name),
            expected_sha1: download
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
            expected_size: (download.file_length > 0)
                .then_some(download.file_length),
            expected_fingerprint: (download.file_fingerprint > 0)
                .then_some(download.file_fingerprint),
            state: crate::state::instances::ManualDownloadState::Waiting,
            context: serde_json::to_value(download)?,
            created_at: now,
            modified_at: now,
        },
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn complete_manual_world_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    let state = State::get().await?;
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
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn persist_manual_download(
    instance_id: &str,
    download: &CurseForgeManualDownload,
) -> crate::Result<()> {
    if download.project_type == "modpack" {
        return persist_manual_modpack_archive(instance_id, download).await;
    }
    if download.project_type == "world" {
        return persist_manual_world_archive(instance_id, download).await;
    }
    let project_type = managed_project_type(&download.project_type)?;
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
    let target_relative_path = format!(
        "{}/{}",
        download.target_folder.trim_end_matches('/'),
        download.file_name
    );
    let member_key = format!(
        "curseforge:{}:{}",
        download.project_id,
        project_type.get_name()
    );
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let now = chrono::Utc::now();
    let pack_member_id = if download.ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
    {
        let existing_id = sqlx::query_scalar::<_, String>(
            "SELECT id FROM instance_pack_members
             WHERE content_set_id = ? AND member_key = ?",
        )
        .bind(&content_set.id)
        .bind(&member_key)
        .fetch_optional(&mut *tx)
        .await?;
        if download.operation_kind
            == crate::state::instances::ManualDownloadOperationKind::PackUpdate
        {
            existing_id
        } else {
            let member_id = existing_id.unwrap_or_else(|| {
                format!("pack-member:{}", uuid::Uuid::new_v4())
            });
            crate::state::instances::adapters::sqlite::content_rows::upsert_pack_member_in_transaction(
            &crate::state::instances::PackMember {
                id: member_id.clone(),
                content_set_id: content_set.id.clone(),
                content_entry_id: None,
                member_key,
                project_type,
                expected_relative_path: target_relative_path.clone(),
                provider: Some(ContentProvider::CurseForge),
                provider_project_id: Some(download.project_id.to_string()),
                provider_release_id: Some(download.file_id.to_string()),
                required: true,
                expected_sha1: download
                    .hashes
                    .iter()
                    .find(|hash| hash.algo == 1)
                    .map(|hash| hash.value.clone()),
                expected_size: (download.file_length > 0)
                    .then_some(download.file_length),
                expected_fingerprint: (download.file_fingerprint > 0)
                    .then_some(download.file_fingerprint),
                materialization_state: crate::state::instances::PackMemberMaterializationState::PendingManual,
                override_kind: crate::state::instances::PackMemberOverrideKind::None,
                reconciled: true,
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
            Some(member_id)
        }
    } else {
        None
    };
    crate::state::instances::adapters::sqlite::content_rows::upsert_pending_manual_download_in_transaction(
        &crate::state::instances::PendingManualDownload {
            id: format!("manual-download:{}", uuid::Uuid::new_v4()),
            instance_id: instance_id.to_string(),
            pack_member_id,
            content_entry_id: None,
            operation_kind: download.operation_kind,
            operation_target_id: None,
            project_type,
            provider: ContentProvider::CurseForge,
            provider_project_id: download.project_id.to_string(),
            provider_release_id: download.file_id.to_string(),
            file_name: download.file_name.clone(),
            website_url: download.website_url.clone(),
            target_relative_path,
            expected_sha1: download
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone()),
            expected_size: (download.file_length > 0)
                .then_some(download.file_length),
            expected_fingerprint: (download.file_fingerprint > 0)
                .then_some(download.file_fingerprint),
            state: crate::state::instances::ManualDownloadState::Waiting,
            context: serde_json::to_value(download)?,
            created_at: now,
            modified_at: now,
        },
        &mut tx,
    )
    .await?;
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
        &content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
