//! Managed modpack updates, backups, and overrides.

use super::catalog::get_file;
use super::content_change::install_selected_file;
use super::dto::{
    CurseForgeInstallResult, CurseForgeInstalledFile,
    CurseForgeModpackInstallRequest, CurseForgeModpackInstallResult,
    CurseForgeModpackManifest, CurseForgeModpackTarget,
    CurseForgePackExpectedOverride, get_modpack_expected_members_with_reporter,
};
use super::modpack::install_modpack_with_reporter;
use super::selection::persist_manual_download;
use crate::install::{InstallJobEventKind, InstallProgressReporter};
use crate::state::{
    ContentProviderRef, ContentSourceKind, CurseForgeFileId,
    CurseForgeProjectId, EditInstance, InstanceLink, ModLoader,
};
use crate::util::download::constants::OVERRIDE_EXTRACTION_CONCURRENCY;
use crate::util::fetch::sha1_file_async;
use crate::{ErrorKind, State};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

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

pub(super) struct CurseForgeUpdateBackup {
    pub(super) _directory: tempfile::TempDir,
    pub(super) files: Vec<(String, String, std::path::PathBuf)>,
}

async fn create_curseforge_update_backup(
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

async fn rollback_curseforge_update(
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

pub(crate) async fn cache_instance_icon_from_url(
    icon_url: &str,
) -> crate::Result<std::path::PathBuf> {
    let normalized =
        crate::util::download::retired_sources::normalize(icon_url)?;
    let icon_url = normalized.as_ref();
    let state = State::get().await?;
    // CurseForge avatar/CDN assets are frequently broken via local system
    // proxies, so always download icons with a direct client.
    let permit = state.fetch_semaphore.0.acquire().await?;
    let response = crate::util::download::native_request::send_cdn_request(
        crate::util::fetch::DIRECT_REQWEST_CLIENT.get(icon_url),
    )
    .await?;
    drop(permit);
    if !response.status().is_success() {
        return Err(ErrorKind::OtherError(format!(
            "CurseForge icon download failed with HTTP {}",
            response.status().as_u16()
        ))
        .into());
    }
    let icon_bytes = response.bytes().await?;
    let filename = icon_url.rsplit('/').next().unwrap_or("icon.png");
    crate::util::fetch::write_cached_icon(
        filename,
        &state.directories.caches_dir(),
        icon_bytes,
        &state.io_semaphore,
    )
    .await
}

pub(super) struct ModpackOverrideTask {
    pub(super) index: usize,
    pub(super) target: PathBuf,
    pub(super) size: u64,
}

pub(super) fn collect_modpack_override_tasks<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &CurseForgeModpackManifest,
    instance_path: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<Vec<ModpackOverrideTask>> {
    let prefix = format!("{}/", manifest.overrides.trim_matches('/'));
    let mut tasks_by_target = HashMap::<PathBuf, ModpackOverrideTask>::new();
    for index in 0..archive.len() {
        crate::api::pack::archive_util::check_cancellation(cancellation)?;
        let entry = archive.by_index(index).map_err(modpack_zip_error)?;
        let entry_name =
            crate::pack::detect::decode_zip_entry_name(entry.name_raw());
        if entry.is_dir() || !entry_name.starts_with(&prefix) {
            continue;
        }
        let relative = &entry_name[prefix.len()..];
        let safe_path = safe_archive_relative_path(relative)?;
        let target = instance_path.join(safe_path);
        // Preserve archive order semantics for duplicate targets: the last
        // entry wins, while unique targets can be extracted independently.
        tasks_by_target.insert(
            target.clone(),
            ModpackOverrideTask {
                index,
                target,
                size: entry.size(),
            },
        );
    }
    let total_size = tasks_by_target
        .values()
        .fold(0_u64, |total, task| total.saturating_add(task.size));
    let limit = crate::api::pack::archive_util::EXTRACTION_SIZE_LIMIT;
    if total_size > limit {
        return Err(ErrorKind::InputError(format!(
			"CurseForge modpack overrides exceed the extraction limit: {total_size} unpacked bytes required, limit is {limit} bytes"
		))
		.into());
    }
    Ok(tasks_by_target.into_values().collect())
}

pub(crate) fn materialize_modpack_overrides(
    archive_path: &Path,
    instance_path: &Path,
    cancellation: Option<&tokio_util::sync::CancellationToken>,
) -> crate::Result<(
    u32,
    crate::api::pack::archive_util::StagedArchiveReplacements,
)> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
    let manifest = read_modpack_manifest(&mut archive)?;
    let tasks = collect_modpack_override_tasks(
        &mut archive,
        &manifest,
        instance_path,
        cancellation,
    )?;
    drop(archive);
    if tasks.is_empty() {
        return Ok((
            0,
            crate::api::pack::archive_util::materialize_staged_archive_entries(
                &[],
                cancellation,
            )?,
        ));
    }
    let targets = tasks
        .iter()
        .map(|task| task.target.clone())
        .collect::<Vec<_>>();
    let worker_count = OVERRIDE_EXTRACTION_CONCURRENCY.min(tasks.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let extraction_result = std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            workers.push(scope.spawn(move || -> crate::Result<u32> {
                let file = std::fs::File::open(archive_path).map_err(|error| {
                    crate::util::io::IOError::with_path(error, archive_path)
                })?;
                let mut archive =
                    zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
                let mut files_written = 0_u32;
                loop {
                    crate::api::pack::archive_util::check_cancellation(
                        cancellation,
                    )?;
                    let Some(task) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let mut entry = archive
                        .by_index(task.index)
                        .map_err(modpack_zip_error)?;
                    let written = crate::api::pack::archive_util::write_archive_entry_to_staging(
                        &mut entry,
                        &task.target,
                        cancellation,
                    )?;
                    if written != entry.size() {
                        return Err(ErrorKind::InputError(
                            "CurseForge modpack override was truncated during extraction"
                                .to_string(),
                        )
                        .into());
                    }
                    files_written = files_written.checked_add(1).ok_or_else(|| {
                        ErrorKind::InputError(
                            "CurseForge modpack contains too many override files"
                                .to_string(),
                        )
                    })?;
                }
                Ok(files_written)
            }));
        }
        let mut files_written = 0_u32;
        for worker in workers {
            let count = worker.join().map_err(|_| {
                ErrorKind::OtherError(
                    "CurseForge override extraction worker panicked"
                        .to_string(),
                )
            })??;
            files_written =
                files_written.checked_add(count).ok_or_else(|| {
                    ErrorKind::InputError(
                        "CurseForge modpack contains too many override files"
                            .to_string(),
                    )
                })?;
        }
        Ok(files_written)
    });
    match extraction_result {
        Ok(files_written) => {
            let replacements = crate::api::pack::archive_util::materialize_staged_archive_entries(
                &targets,
                cancellation,
            )?;
            Ok((files_written, replacements))
        }
        Err(error) => {
            if let Err(cleanup_error) =
                crate::api::pack::archive_util::discard_staged_archive_entries(
                    &targets,
                )
            {
                return Err(ErrorKind::OtherError(format!(
                    "{error}; failed to clean staged CurseForge overrides: {cleanup_error}"
                ))
                .into());
            }
            Err(error)
        }
    }
}

#[cfg(test)]
pub(super) fn extract_modpack_overrides(
    archive_path: &Path,
    instance_path: &Path,
    cancellation: Option<&tokio_util::sync::CancellationToken>,
) -> crate::Result<u32> {
    let (files_written, replacements) = materialize_modpack_overrides(
        archive_path,
        instance_path,
        cancellation,
    )?;
    replacements.finalize()?;
    Ok(files_written)
}

pub(crate) async fn settle_materialized_modpack_overrides(
    instance_id: String,
    cancellation: CancellationToken,
    replacements: crate::api::pack::archive_util::StagedArchiveReplacements,
    commit: bool,
) -> crate::Result<()> {
    crate::api::pack::archive_util::run_blocking_instance_write(
        instance_id,
        cancellation,
        move |_| {
            if commit {
                replacements.finalize()
            } else {
                replacements.rollback()
            }
        },
    )
    .await
}

pub(crate) fn read_modpack_manifest<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> crate::Result<CurseForgeModpackManifest> {
    let mut entry = archive.by_name("manifest.json").map_err(|_| {
        ErrorKind::InputError(
            "CurseForge modpack is missing manifest.json".to_string(),
        )
    })?;
    let mut json = String::new();
    entry.read_to_string(&mut json)?;
    Ok(serde_json::from_str::<CurseForgeModpackManifest>(&json)?)
}

pub(crate) fn read_modpack_override_content<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    manifest: &CurseForgeModpackManifest,
) -> crate::Result<Vec<CurseForgePackExpectedOverride>> {
    let prefix = format!("{}/", manifest.overrides.trim_matches('/'));
    let mut overrides = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(modpack_zip_error)?;
        let entry_name =
            crate::pack::detect::decode_zip_entry_name(entry.name_raw());
        if entry.is_dir() || !entry_name.starts_with(&prefix) {
            continue;
        }
        let Some(override_content) =
            curseforge_override_content(&entry_name[prefix.len()..])?
        else {
            continue;
        };
        overrides.push(override_content);
    }
    overrides.sort_by(|left, right| {
        left.expected_relative_path
            .cmp(&right.expected_relative_path)
    });
    overrides.dedup_by(|left, right| {
        left.expected_relative_path == right.expected_relative_path
    });
    Ok(overrides)
}

pub(super) fn curseforge_override_content(
    relative_path: &str,
) -> crate::Result<Option<CurseForgePackExpectedOverride>> {
    let relative_path = safe_archive_relative_path(relative_path)?;
    let Some(project_type) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(
        &relative_path,
    ) else {
        return Ok(None);
    };
    if !crate::state::instances::adapters::filesystem::is_scannable_project_path(
        project_type,
        &relative_path,
    ) {
        return Ok(None);
    }
    Ok(Some(CurseForgePackExpectedOverride {
        project_type,
        expected_relative_path: relative_path,
    }))
}

pub(crate) fn modpack_target(
    manifest: &CurseForgeModpackManifest,
) -> crate::Result<CurseForgeModpackTarget> {
    let Some(manifest_loader) = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
    else {
        return Ok(CurseForgeModpackTarget {
            game_version: manifest.minecraft.version.clone(),
            loader: ModLoader::Vanilla,
            loader_version: None,
        });
    };

    let family = loader_family(&manifest_loader.id);
    let manifest_loader_version = manifest_loader
        .id
        .strip_prefix(family)
        .and_then(|version| version.strip_prefix('-'))
        .filter(|version| !version.is_empty());
    let disguised_cleanroom = family == "forge"
        && manifest.minecraft.version == "1.12.2"
        && manifest_loader_version
            .is_some_and(|version| version.starts_with("0."));
    let loader = match family {
        "cleanroom" => ModLoader::Cleanroom,
        "forge" if disguised_cleanroom => ModLoader::Cleanroom,
        "forge" => ModLoader::Forge,
        "fabric" => ModLoader::Fabric,
        "quilt" => ModLoader::Quilt,
        "neo" | "neoforge" => ModLoader::NeoForge,
        _ => {
            return Err(ErrorKind::InputError(format!(
                "CurseForge modpack uses unsupported loader {}",
                manifest_loader.id
            ))
            .into());
        }
    };
    let loader_version = manifest_loader_version
        .filter(|version| !disguised_cleanroom || version.starts_with("0."))
        .map(str::to_string);

    Ok(CurseForgeModpackTarget {
        game_version: manifest.minecraft.version.clone(),
        loader,
        loader_version,
    })
}

/// Reads the identity fields from a local CurseForge archive before an
/// instance is created, avoiding the temporary default Vanilla metadata.
pub async fn get_local_modpack_target(
    archive_path: &Path,
) -> crate::Result<(String, CurseForgeModpackTarget)> {
    let path = archive_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        let manifest = read_modpack_manifest(&mut archive)?;
        let name = manifest
            .name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "CurseForge Modpack".to_string());
        Ok((name, modpack_target(&manifest)?))
    })
    .await?
}

pub(crate) fn modpack_zip_error(error: zip::result::ZipError) -> crate::Error {
    ErrorKind::InputError(format!(
        "CurseForge modpack archive is invalid: {error}"
    ))
    .into()
}

pub(super) fn safe_archive_relative_path(value: &str) -> crate::Result<String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ErrorKind::InputError(
            "CurseForge modpack contains an invalid override path".to_string(),
        )
        .into());
    }
    Ok(normalized)
}

pub(crate) fn loader_family(loader_id: &str) -> &str {
    loader_id.split('-').next().unwrap_or(loader_id)
}

pub(crate) fn loader_type(loader: &str) -> Option<u32> {
    match loader.trim().to_ascii_lowercase().as_str() {
        "forge" | "cleanroom" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

pub(super) fn looks_like_game_version(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && loader_type(value).is_none()
}

pub(crate) fn merge_install_result(
    target: &mut CurseForgeInstallResult,
    mut source: CurseForgeInstallResult,
) {
    target.installed.append(&mut source.installed);
    target.manual_downloads.append(&mut source.manual_downloads);
    target.failed_downloads.append(&mut source.failed_downloads);
    target
        .optional_dependencies
        .append(&mut source.optional_dependencies);
    target
        .incompatible_dependencies
        .append(&mut source.incompatible_dependencies);
}

pub async fn switch_installed_file_version(
    instance_id: &str,
    relative_path: &str,
    file_id: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeInstallResult> {
    use sqlx::Row;

    let state = State::get().await?;
    let row = sqlx::query(
		"SELECT ref.provider_project_id, ref.provider_release_id, entry.project_type,
				entry.ownership_kind,
				content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id AND ref.provider = 'curseforge'
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to CurseForge".to_string(),
        )
    })?;
    let project_id = row
        .try_get::<String, _>("provider_project_id")?
        .parse::<u32>()
        .map_err(|_| {
            ErrorKind::InputError(
                "Stored CurseForge project ID is invalid".to_string(),
            )
        })?;
    let current_file_id = row
        .try_get::<Option<String>, _>("provider_release_id")?
        .and_then(|value| value.parse::<u32>().ok());
    let project_type = row.try_get::<String, _>("project_type")?;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.try_get::<String, _>("ownership_kind")?,
        )?;
    let game_version = row.try_get::<String, _>("game_version")?;
    let loader = row.try_get::<String, _>("loader")?;
    let mod_loader_type = match loader.as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    };

    if current_file_id == Some(file_id) {
        return Ok(CurseForgeInstallResult::default());
    }

    install_selected_file(
        instance_id,
        relative_path,
        project_id,
        current_file_id,
        file_id,
        project_type,
        ownership_kind,
        game_version,
        mod_loader_type,
        reporter,
    )
    .await
}
