//! CurseForge modpack update managed.

use crate::api::curseforge::archive::{loader_type, looks_like_game_version};
use crate::api::curseforge::catalog::get_file;
use crate::api::curseforge::dto::{
    CurseForgeInstallResult, CurseForgeInstalledFile,
    CurseForgeModpackInstallRequest, CurseForgeModpackInstallResult,
};
use crate::api::curseforge::modpack::{
    get_modpack_expected_members_with_reporter, install_modpack_with_reporter,
};
use crate::api::curseforge::selection::persist_manual_download;
use crate::install::{InstallJobEventKind, InstallProgressReporter};
use crate::state::{
    ContentProviderRef, ContentSourceKind, CurseForgeFileId,
    CurseForgeProjectId, EditInstance, InstanceLink,
};
use crate::util::fetch::sha1_file_async;
use crate::{ErrorKind, State};
use std::collections::{HashMap, HashSet};

pub async fn update_managed_modpack(
    instance_id: &str,
    file_id: u32,
) -> crate::Result<CurseForgeModpackInstallResult> {
    update_managed_modpack_with_reporter(instance_id, file_id, None).await
}

pub(crate) async fn update_managed_modpack_with_reporter(
    instance_id: &str,
    file_id: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let state = State::get().await?;
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| ErrorKind::InputError("Unknown instance".to_string()))?;
    let project_id = match &metadata.link {
        InstanceLink::CurseForgeModpack { project_id, .. } => {
            project_id.parse::<u32>().map_err(|_| {
                ErrorKind::InputError(
                    "Linked CurseForge project ID is invalid".to_string(),
                )
            })?
        }
        _ => {
            return Err(ErrorKind::InputError(format!(
                "Instance {instance_id} is not a managed CurseForge pack, or has been disconnected."
            ))
            .into());
        }
    };
    let members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
        &metadata.applied_content_set.id,
        &state.pool,
    )
    .await?;
    if members.iter().any(|member| !member.reconciled) {
        return Err(ErrorKind::InputError(
            "CurseForge pack membership is not calibrated yet; refresh the content page while online before updating"
                .to_string(),
        )
        .into());
    }
    let expected = get_modpack_expected_members_with_reporter(
        project_id,
        file_id,
        reporter.as_ref(),
    )
    .await?;
    let pack_file = get_file(project_id, file_id).await?;
    let game_version = pack_file
        .game_versions
        .iter()
        .find(|value| looks_like_game_version(value))
        .cloned()
        .unwrap_or_else(|| metadata.applied_content_set.game_version.clone());
    let loader = pack_file
        .game_versions
        .iter()
        .find_map(|value| {
            loader_type(value).map(|_| value.to_ascii_lowercase())
        })
        .or_else(|| {
            Some(metadata.applied_content_set.loader.as_str().to_string())
        });
    let installed_releases = members
        .iter()
        .filter(|member| {
            member.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
        })
        .filter_map(|member| {
            Some((
                member.provider_project_id.clone()?,
                member.provider_release_id.clone()?,
            ))
        })
        .collect::<HashSet<_>>();
    let manual_downloads = expected
        .members
        .iter()
        .filter(|member| member.required)
        .filter(|member| {
            !installed_releases.contains(&(
                member.project_id.to_string(),
                member.file_id.to_string(),
            ))
        })
        .filter_map(|member| member.manual_download.clone())
        .collect::<Vec<_>>();
    if !manual_downloads.is_empty() {
        for download in &manual_downloads {
            persist_manual_download(instance_id, download).await?;
        }
        if let Some(reporter) = &reporter {
            let events = manual_downloads
                .iter()
                .map(|download| InstallJobEventKind::ContentFileSkipped {
                    path: download.file_name.clone(),
                    reason: "CurseForge requires manual download".to_string(),
                    project_id: Some(download.project_id.to_string()),
                    version_id: Some(download.file_id.to_string()),
                    manual_url: download.website_url.clone(),
                })
                .collect();
            reporter.record_events(events).await?;
        }
        return Ok(CurseForgeModpackInstallResult {
            content: CurseForgeInstallResult {
                manual_downloads,
                ..CurseForgeInstallResult::default()
            },
            overrides_written: 0,
            minecraft_version: game_version,
            loader,
        });
    }

    let entries = crate::state::instances::adapters::sqlite::content_rows::get_content_entries(
        &metadata.applied_content_set.id,
        &state.pool,
    )
    .await?
    .into_iter()
    .filter_map(|entry| entry.file_id.map(|file_id| (entry.id, file_id)))
    .collect::<HashMap<_, _>>();
    let files = crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
        instance_id,
        &state.pool,
    )
    .await?
    .into_iter()
    .map(|file| (file.id.clone(), file.relative_path))
    .collect::<HashMap<_, _>>();
    let backup = create_curseforge_update_backup(
        &metadata, &members, &entries, &files, &state,
    )
    .await?;
    let result = match install_modpack_with_reporter(
        CurseForgeModpackInstallRequest {
            instance_id: instance_id.to_string(),
            project_id,
            file_id,
            install_optional: false,
            allow_target_change: true,
        },
        reporter,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            rollback_curseforge_update(
                instance_id,
                &metadata,
                &members,
                &backup,
                &[],
                &state,
            )
            .await?;
            return Err(error);
        }
    };
    if !result.content.manual_downloads.is_empty()
        || !result.content.failed_downloads.is_empty()
    {
        rollback_curseforge_update(
            instance_id,
            &metadata,
            &members,
            &backup,
            &result.content.installed,
            &state,
        )
        .await?;
        return Ok(result);
    }

    // `install_modpack_with_reporter` resolves the target manifest and writes
    // the resulting Minecraft/loader metadata before returning. Read that
    // committed content set so the reconciliation edit cannot overwrite a
    // newly selected loader version with the pre-update instance metadata.
    let updated_content_set =
        crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Instance has no applied content set after CurseForge update"
                    .to_string(),
            )
        })?;

    let expected_keys = expected
        .members
        .iter()
        .map(|member| {
            format!(
                "curseforge:{}:{}",
                member.project_id,
                member.project_type.get_name()
            )
        })
        .collect::<HashSet<_>>();
    for member in &members {
        if expected_keys.contains(&member.member_key)
            || member.override_kind
                == crate::state::instances::PackMemberOverrideKind::Version
        {
            continue;
        }
        let Some(entry_id) = member.content_entry_id.as_ref() else {
            continue;
        };
        let Some(file_id) = entries.get(entry_id) else {
            continue;
        };
        let Some(relative_path) = files.get(file_id) else {
            continue;
        };
        crate::state::instances::commands::remove_project(
            instance_id,
            relative_path,
            &state,
        )
        .await?;
    }
    crate::state::sync_content_files(instance_id, &state).await?;
    crate::state::instances::commands::reconcile_curseforge_members(
        instance_id,
        &metadata.applied_content_set.id,
        &expected,
        &state,
    )
    .await?;
    crate::api::instance::edit(
        instance_id,
        EditInstance {
            link: Some(InstanceLink::CurseForgeModpack {
                project_id: project_id.to_string(),
                version_id: file_id.to_string(),
            }),
            content_set_patch: Some(crate::state::AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::CurseForge),
                game_version: Some(updated_content_set.game_version),
                protocol_version: Some(None),
                loader: Some(updated_content_set.loader),
                loader_version: Some(updated_content_set.loader_version),
            }),
            ..EditInstance::default()
        },
    )
    .await?;
    Ok(result)
}

pub(crate) struct CurseForgeUpdateBackup {
    pub(super) _directory: tempfile::TempDir,
    pub(super) files: Vec<(String, String, std::path::PathBuf)>,
}

pub(super) async fn create_curseforge_update_backup(
    metadata: &crate::state::InstanceMetadata,
    members: &[crate::state::instances::PackMember],
    entries: &HashMap<String, String>,
    files: &HashMap<String, String>,
    state: &State,
) -> crate::Result<CurseForgeUpdateBackup> {
    crate::util::io::create_dir_all(&state.directories.caches_dir()).await?;
    let directory = tempfile::Builder::new()
        .prefix("curseforge-pack-update-")
        .tempdir_in(state.directories.caches_dir())?;
    let instance_path = state.directories.instance_game_dir(&metadata.instance);
    let mut backups = Vec::new();
    for member in members {
        let Some(entry_id) = member.content_entry_id.as_ref() else {
            continue;
        };
        let Some(file_id) = entries.get(entry_id) else {
            continue;
        };
        let Some(relative_path) = files.get(file_id) else {
            continue;
        };
        let source = instance_path.join(relative_path);
        if !source.is_file() {
            continue;
        }
        let backup_path = directory
            .path()
            .join(format!("{}.backup", uuid::Uuid::new_v4()));
        tokio::fs::copy(&source, &backup_path).await?;
        backups.push((
            member.member_key.clone(),
            relative_path.clone(),
            backup_path,
        ));
    }
    Ok(CurseForgeUpdateBackup {
        _directory: directory,
        files: backups,
    })
}

pub(super) async fn rollback_curseforge_update(
    instance_id: &str,
    metadata: &crate::state::InstanceMetadata,
    old_members: &[crate::state::instances::PackMember],
    backup: &CurseForgeUpdateBackup,
    installed: &[CurseForgeInstalledFile],
    state: &State,
) -> crate::Result<()> {
    let instance_path = state.directories.instance_game_dir(&metadata.instance);
    let old_paths = backup
        .files
        .iter()
        .map(|(_, relative_path, _)| relative_path.as_str())
        .collect::<HashSet<_>>();
    for installed_file in installed {
        if old_paths.contains(installed_file.relative_path.as_str()) {
            continue;
        }
        crate::state::instances::commands::remove_project(
            instance_id,
            &installed_file.relative_path,
            state,
        )
        .await?;
    }
    for (member_key, relative_path, backup_path) in &backup.files {
        let destination = instance_path.join(relative_path);
        if let Some(parent) = destination.parent() {
            crate::util::io::create_dir_all(parent).await?;
        }
        tokio::fs::copy(backup_path, &destination).await?;
        let Some(member) = old_members
            .iter()
            .find(|member| member.member_key == *member_key)
        else {
            continue;
        };
        let (Some(project_id), Some(file_id)) = (
            member.provider_project_id.as_deref(),
            member.provider_release_id.as_deref(),
        ) else {
            continue;
        };
        let provider_ref = ContentProviderRef::CurseForge {
            project_id: CurseForgeProjectId::new(project_id.parse().map_err(
                |_| {
                    ErrorKind::InputError(
                        "Stored CurseForge project ID is invalid".to_string(),
                    )
                },
            )?)?,
            file_id: Some(CurseForgeFileId::new(file_id.parse().map_err(
                |_| {
                    ErrorKind::InputError(
                        "Stored CurseForge file ID is invalid".to_string(),
                    )
                },
            )?)?),
        };
        let (size, sha1) = sha1_file_async(&destination).await?;
        crate::state::record_project_file_atomic(
            instance_id,
            relative_path,
            &sha1,
            size,
            member.project_type,
            ContentSourceKind::CurseForge,
            crate::state::instances::ContentOwnershipKind::PackManaged,
            Some(&provider_ref),
            true,
            None,
            state,
        )
        .await?;
    }

    let current_members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
        &metadata.applied_content_set.id,
        &state.pool,
    )
    .await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
    let old_keys = old_members
        .iter()
        .map(|member| member.member_key.as_str())
        .collect::<HashSet<_>>();
    for member in current_members {
        if !old_keys.contains(member.member_key.as_str()) {
            sqlx::query("DELETE FROM instance_pack_members WHERE id = ?")
                .bind(member.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    for old_member in old_members {
        let current_entry_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT content_entry_id FROM instance_pack_members
             WHERE content_set_id = ? AND member_key = ?",
        )
        .bind(&metadata.applied_content_set.id)
        .bind(&old_member.member_key)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        let mut restored_member = old_member.clone();
        if current_entry_id.is_some() {
            restored_member.content_entry_id = current_entry_id;
        }
        crate::state::instances::adapters::sqlite::content_rows::upsert_pack_member_in_transaction(
            &restored_member,
            &mut tx,
        )
        .await?;
    }
    crate::state::instances::adapters::sqlite::content_rows::bump_content_set_revision_in_transaction(
        &metadata.applied_content_set.id,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
