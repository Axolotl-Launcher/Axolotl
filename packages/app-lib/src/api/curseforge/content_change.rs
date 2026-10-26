//! Content version changes.

use super::catalog::{get_file, get_modpack_files, get_modpack_projects};
use super::download::resolve_download_url as resolve_curseforge_download_url;
use super::dto::{CurseForgeInstallRequest, CurseForgeInstallResult};
use super::install::{install_file, install_file_with_reporter};
use super::manual::mark_pack_member_version_override;
use super::preview::preview_install_file;
use super::selection::curseforge_file_sha1;
use crate::install::InstallProgressReporter;
use crate::state::ContentProvider;
use crate::{ErrorKind, State};

pub(crate) async fn prepare_curseforge_content_change_action(
    instance_id: &str,
    action: &mut crate::install::ContentChangeAction,
) -> crate::Result<()> {
    use sqlx::Row;

    let state = State::get().await?;
    let relative_path = action.relative_path.as_deref().ok_or_else(|| {
        ErrorKind::InputError(format!(
            "Content {} is not present on disk",
            action.content_id
        ))
    })?;
    let row = sqlx::query(
        "SELECT entry.project_type, entry.ownership_kind,
                content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instances instance ON instance.id = entry.instance_id
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
            AND entry.content_set_id = instance.applied_content_set_id
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to the active content set"
                .to_string(),
        )
    })?;
    let project_id = action
        .project_id
        .as_deref()
        .ok_or_else(|| {
            ErrorKind::InputError("Missing CurseForge project ID".to_string())
        })?
        .parse::<u32>()
        .map_err(|_| {
            ErrorKind::InputError("Invalid CurseForge project ID".to_string())
        })?;
    let file_id = action.target_release_id.parse::<u32>().map_err(|_| {
        ErrorKind::InputError("Invalid CurseForge file ID".to_string())
    })?;
    let project_type = row.try_get::<String, _>("project_type")?;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.try_get::<String, _>("ownership_kind")?,
        )?;
    let loader = row.try_get::<String, _>("loader")?;
    let mod_loader_type = match loader.as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    };
    let preview = preview_install_file(CurseForgeInstallRequest {
        instance_id: instance_id.to_string(),
        project_id,
        file_id,
        project_type,
        ownership_kind,
        manual_operation_kind:
            crate::state::instances::ManualDownloadOperationKind::ContentUpdate,
        game_version: Some(row.try_get::<String, _>("game_version")?),
        mod_loader_type,
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: Vec::new(),
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    })
    .await?;
    let old_provider_file_name = match action.expected_release_id.as_deref() {
        Some(file_id) => Some(
            get_file(
                project_id,
                file_id.parse::<u32>().map_err(|_| {
                    ErrorKind::InputError(
                        "Invalid stored CurseForge file ID".to_string(),
                    )
                })?,
            )
            .await?
            .file_name,
        ),
        None => None,
    };
    action.final_relative_path = Some(
        crate::state::instances::commands::preserved_update_relative_path(
            relative_path,
            old_provider_file_name.as_deref(),
            &preview.primary.file_name,
        )?,
    );
    action.current_provider_file_name = old_provider_file_name;
    action.target_provider_file_name = Some(preview.primary.file_name.clone());
    action.files.clear();
    action.dependencies.clear();

    let items: Vec<_> = std::iter::once(&preview.primary)
        .chain(preview.dependencies.iter())
        .collect();
    let files =
        get_modpack_files(items.iter().map(|item| item.file_id).collect())
            .await?;
    let projects = get_modpack_projects(
        items.iter().map(|item| item.project_id).collect(),
    )
    .await?;

    for (index, item) in items.into_iter().enumerate() {
        let fetched;
        let file = match files.get(&item.file_id) {
            Some(file) => file,
            None => {
                fetched = get_file(item.project_id, item.file_id).await?;
                &fetched
            }
        };
        let Some(project) = projects.get(&item.project_id) else {
            return Err(ErrorKind::OtherError(format!(
                "CurseForge project {} was not returned by the metadata batch",
                item.project_id
            ))
            .into());
        };
        let url = resolve_curseforge_download_url(
            item.project_id,
            item.file_id,
            &project,
            &file,
        )
        .await?;
        let cache_path = state
            .directories
            .caches_dir()
            .join("content")
            .join("curseforge")
            .join(item.project_id.to_string())
            .join(item.file_id.to_string())
            .join(crate::util::io::sanitize_download_file_name(
                &item.file_name,
            )?);
        let id = format!("curseforge:{}:{}", item.project_id, item.file_id);
        action.files.push(crate::install::ContentChangeFile {
            id: id.clone(),
            role: if index == 0 {
                crate::install::ContentChangeFileRole::Primary
            } else {
                crate::install::ContentChangeFileRole::Dependency
            },
            provider: ContentProvider::CurseForge,
            project_id: item.project_id.to_string(),
            release_id: item.file_id.to_string(),
            file_name: item.file_name.clone(),
            target_relative_path: cache_path.display().to_string(),
            urls: url.clone().into_iter().collect(),
            manual_download_url: url.is_none().then(|| {
                format!(
                    "https://www.curseforge.com/minecraft/mc-mods/{}/download/{}/file",
                    project.slug, item.file_id
                )
            }),
            integrity: crate::install::ContentChangeFileIntegrity {
                size: Some(item.size),
                sha1: curseforge_file_sha1(&file),
                sha512: None,
                sha256: None,
                md5: file
                    .hashes
                    .iter()
                    .find(|hash| hash.algo == 2)
                    .map(|hash| hash.value.clone()),
            },
        });
        for parent in &item.required_by_project_ids {
            action
                .dependencies
                .push(crate::install::ContentChangeDependency {
                    parent_file_id: format!("curseforge:{parent}:unknown"),
                    child_file_id: id.clone(),
                    provider: ContentProvider::CurseForge,
                    project_id: item.project_id.to_string(),
                    release_id: item.file_id.to_string(),
                    kind: Some(if item.required {
                        crate::state::instances::ContentDependencyKind::Required
                    } else {
                        crate::state::instances::ContentDependencyKind::Include
                    }),
                });
        }
    }
    for fallback in preview.modrinth_fallbacks {
        action
            .dependencies
            .push(crate::install::ContentChangeDependency {
                parent_file_id: format!(
                    "curseforge:{}:unknown",
                    fallback.parent_project_id
                ),
                child_file_id: format!("modrinth:{}", fallback.version_id),
                provider: ContentProvider::Modrinth,
                project_id: fallback.project_id.clone(),
                release_id: fallback.version_id.clone(),
                kind: Some(if fallback.required {
                    crate::state::instances::ContentDependencyKind::Required
                } else {
                    crate::state::instances::ContentDependencyKind::Include
                }),
            });
        let prepared =
            crate::state::instances::commands::prepare_version_download(
                instance_id,
                &fallback.version_id,
                crate::util::fetch::DownloadReason::Dependency,
                None,
                &state,
            )
            .await?;
        action.files.push(crate::install::ContentChangeFile {
            id: format!("modrinth:{}", fallback.version_id),
            role: crate::install::ContentChangeFileRole::Dependency,
            provider: ContentProvider::Modrinth,
            project_id: fallback.project_id,
            release_id: fallback.version_id,
            file_name: prepared.file_name.clone(),
            target_relative_path: prepared.path.display().to_string(),
            urls: vec![prepared.url.clone()],
            manual_download_url: None,
            integrity: crate::install::ContentChangeFileIntegrity {
                size: prepared.integrity.size,
                sha1: prepared.integrity.sha1.clone(),
                sha512: prepared.integrity.sha512.clone(),
                sha256: None,
                md5: None,
            },
        });
    }
    action.set_status(crate::install::ContentChangeActionStatus::Prepared);
    Ok(())
}

pub(crate) async fn install_selected_file(
    instance_id: &str,
    relative_path: &str,
    project_id: u32,
    current_file_id: Option<u32>,
    file_id: u32,
    project_type: String,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    game_version: String,
    mod_loader_type: Option<u32>,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeInstallResult> {
    let old_provider_file_name = match current_file_id {
        Some(current_file_id) => {
            Some(get_file(project_id, current_file_id).await?.file_name)
        }
        None => None,
    };
    let new_provider_file_name = get_file(project_id, file_id).await?.file_name;
    let request = CurseForgeInstallRequest {
        instance_id: instance_id.to_string(),
        project_id,
        file_id,
        project_type,
        ownership_kind,
        manual_operation_kind:
            crate::state::instances::ManualDownloadOperationKind::ContentUpdate,
        game_version: Some(game_version),
        mod_loader_type,
        world_name: None,
        install_dependencies: true,
        excluded_dependency_project_ids: Vec::new(),
        force_dependency_project_ids: Vec::new(),
        dependency_plan_id: None,
        defer_persistence: false,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: None,
    };
    let mut result = match reporter {
        Some(reporter) => install_file_with_reporter(request, reporter).await?,
        None => install_file(request).await?,
    };
    if ownership_kind
        == crate::state::instances::ContentOwnershipKind::PackManaged
        && let Some(installed) = result
            .installed
            .iter()
            .find(|file| !file.dependency && file.project_id == project_id)
    {
        mark_pack_member_version_override(
            instance_id,
            &installed.relative_path,
        )
        .await?;
    }
    if let Some(primary_index) = result.installed.iter().position(|file| {
        !file.dependency
            && file.project_id == project_id
            && file.relative_path != relative_path
    }) {
        let state = State::get().await?;
        let installed_path =
            result.installed[primary_index].relative_path.clone();
        let final_path =
            crate::state::instances::commands::finalize_updated_project_path(
                instance_id,
                relative_path,
                &installed_path,
                old_provider_file_name.as_deref(),
                &new_provider_file_name,
                &state,
            )
            .await?;
        result.installed[primary_index].relative_path = final_path;
    }
    Ok(result)
}
