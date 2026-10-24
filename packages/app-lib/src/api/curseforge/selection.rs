//! Dependency selection, target folders, and progress helpers.

use super::catalog::get_files;
use super::dto::{
    CurseForgeFile, CurseForgeFilesRequest, CurseForgeInstallRequest,
    CurseForgeInstallResult, CurseForgeInstalledFile, CurseForgeManualDownload,
    CurseForgeProject, CurseForgeWorldInstallResult,
};
use super::install::{
    CurseForgeModrinthFallbackPreview, dependency_policy, install_world_file,
};
use super::mapping::mod_loader_to_slug;
use super::validation::{validate_file_name, validate_world_archive_name};
use super::{
    CachedDependencyResolutionPlan, DEPENDENCY_PLAN_TTL,
    DEPENDENCY_RESOLUTION_PLANS, MAX_PAGE_SIZE,
};
use crate::event::emit::emit_loading;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProvider, ContentProviderRef,
    CurseForgeFileId, CurseForgeProjectId, DependencyResolutionEdge,
    DependencyResolutionIssue, DependencyResolutionNode,
    DependencyResolutionPlan, DependencySelectionReason, ModLoader,
    ModrinthProjectId, ModrinthVersionId, ProjectType, ReleaseChannel,
};
use crate::util::download::constants::{
    CURSEFORGE_H2_RANGE_THRESHOLD, CURSEFORGE_REQUESTED_H2_RANGES,
};
use crate::util::fetch::{ContentValidation, Integrity};
use crate::{ErrorKind, State};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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

/// Why a dependency file was selected. This makes fallback provenance
/// explicit rather than allowing it to look like a native declaration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurseForgeDependencySelectionReason {
    NativeStrictMatch,
    Sha1VerifiedModrinthFallback,
}

/// A CurseForge file selected for a dependency after local target validation.
pub(super) struct SelectedDependencyFile {
    pub(super) file: CurseForgeFile,
    pub(super) reason: CurseForgeDependencySelectionReason,
}

#[cfg(debug_assertions)]
fn log_dependency_file_selection(
    project_id: u32,
    target_game_version: &str,
    target_mod_loader_type: Option<u32>,
    source: &'static str,
    file: &CurseForgeFile,
) {
    tracing::warn!(
        project_id,
        target_game_version,
        ?target_mod_loader_type,
        source,
        selected_file_id = file.id,
        selected_file_name = %file.file_name,
        game_versions = ?file.game_versions,
        sortable_game_versions = ?file.sortable_game_versions,
        "CurseForge dependency file selected"
    );
}

pub(super) fn dependency_target_game_version(
    game_version: Option<&str>,
) -> Option<&str> {
    game_version.filter(|version| !version.trim().is_empty())
}

pub(super) async fn select_dependency_file(
    project_id: u32,
    game_version: Option<String>,
    mod_loader_type: Option<u32>,
) -> crate::Result<Option<SelectedDependencyFile>> {
    let Some(target_game_version) =
        dependency_target_game_version(game_version.as_deref())
    else {
        #[cfg(debug_assertions)]
        tracing::warn!(
            project_id,
            ?mod_loader_type,
            "CurseForge dependency target has no Minecraft version"
        );
        return Ok(None);
    };
    let filtered = get_files(
        project_id,
        CurseForgeFilesRequest {
            game_version: game_version.clone(),
            mod_loader_type,
            game_version_type_id: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        },
    )
    .await?
    .files;
    if let Some(file) = select_best_dependency_file(
        filtered,
        Some(target_game_version),
        mod_loader_type,
    ) {
        #[cfg(debug_assertions)]
        log_dependency_file_selection(
            project_id,
            target_game_version,
            mod_loader_type,
            "filtered",
            &file,
        );
        return Ok(Some(SelectedDependencyFile {
            file,
            reason: CurseForgeDependencySelectionReason::NativeStrictMatch,
        }));
    }
    let unfiltered = get_files(
        project_id,
        CurseForgeFilesRequest {
            game_version: None,
            mod_loader_type: None,
            game_version_type_id: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        },
    )
    .await?
    .files;
    let selected = select_best_dependency_file(
        unfiltered,
        Some(target_game_version),
        mod_loader_type,
    );
    #[cfg(debug_assertions)]
    if let Some(file) = selected.as_ref() {
        log_dependency_file_selection(
            project_id,
            target_game_version,
            mod_loader_type,
            "unfiltered",
            file,
        );
    }
    Ok(selected.map(|file| SelectedDependencyFile {
        file,
        reason: CurseForgeDependencySelectionReason::NativeStrictMatch,
    }))
}

pub(crate) async fn resolve_modrinth_fallback_plan(
    file: &CurseForgeFile,
    item_type: ProjectType,
    request: &CurseForgeInstallRequest,
    state: &State,
) -> crate::Result<Option<content_management::ResolveContentPlan>> {
    let Some(sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.as_str())
    else {
        return Ok(None);
    };
    let (Some(game_version), Some(loader_type)) =
        (request.game_version.as_deref(), request.mod_loader_type)
    else {
        return Ok(None);
    };
    let Some(loader) = mod_loader_from_curseforge_type(loader_type) else {
        return Ok(None);
    };

    let matches = CachedEntry::get_file_many(
        &[sha1],
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let Some(file_match) = matches.into_iter().next() else {
        return Ok(None);
    };
    let version = CachedEntry::get_version(
        &ModrinthVersionId::new(file_match.version_id)?,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?;
    let Some(version) = version else {
        return Ok(None);
    };
    if !modrinth_version_matches_target(&version, game_version, loader) {
        return Ok(None);
    }

    crate::state::instances::commands::resolve_install_plan_for_target_with_policy(
        crate::state::instances::commands::InstanceInstallProjectRequest {
            project_id: version.project_id,
            version_id: Some(version.id),
            content_type: item_type.into(),
            selected: Default::default(),
            excluded_project_ids: Vec::new(),
            force_project_ids: Vec::new(),
        },
        game_version.to_string(),
        loader,
        dependency_policy(request.manual_operation_kind),
        state,
    )
    .await
    .map(Some)
}

fn modrinth_version_matches_target(
    version: &crate::state::Version,
    game_version: &str,
    loader: ModLoader,
) -> bool {
    version
        .game_versions
        .iter()
        .any(|value| value == game_version)
        && version
            .loaders
            .iter()
            .any(|value| value.eq_ignore_ascii_case(loader.as_str()))
}

fn mod_loader_from_curseforge_type(mod_loader_type: u32) -> Option<ModLoader> {
    match mod_loader_type {
        1 => Some(ModLoader::Forge),
        4 => Some(ModLoader::Fabric),
        5 => Some(ModLoader::Quilt),
        6 => Some(ModLoader::NeoForge),
        _ => None,
    }
}

pub(crate) async fn preview_modrinth_fallbacks(
    plan: &content_management::ResolveContentPlan,
    parent_project_id: u32,
    state: &State,
) -> crate::Result<Vec<CurseForgeModrinthFallbackPreview>> {
    let mut preview = Vec::new();
    for dependency in &plan.dependencies {
        let version = CachedEntry::get_version(
            &ModrinthVersionId::new(dependency.version_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        let Some(version) = version else {
            continue;
        };
        let project = CachedEntry::get_project(
            &ModrinthProjectId::new(dependency.project_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        preview.push(CurseForgeModrinthFallbackPreview {
            project_id: dependency.project_id.clone(),
            version_id: dependency.version_id.clone(),
            title: project
                .as_ref()
                .map(|project| project.title.clone())
                .unwrap_or_else(|| dependency.project_id.clone()),
            version_number: version.version_number,
            parent_project_id,
            icon_url: project.and_then(|project| project.icon_url),
            required: dependency.required,
        });
    }
    Ok(preview)
}

pub(crate) async fn append_modrinth_fallback_plan(
    resolution_plan: &mut DependencyResolutionPlan,
    fallback_plan: &content_management::ResolveContentPlan,
    curseforge_parent: &ContentProviderRef,
    state: &State,
) -> crate::Result<()> {
    for dependency in &fallback_plan.dependencies {
        let content = ContentProviderRef::Modrinth {
            project_id: ModrinthProjectId::new(dependency.project_id.clone())?,
            version_id: Some(ModrinthVersionId::new(
                dependency.version_id.clone(),
            )?),
        };
        let parent = match dependency.dependent_on_version_id.as_deref() {
            Some(version_id)
                if version_id == fallback_plan.primary.version_id =>
            {
                curseforge_parent.clone()
            }
            Some(version_id) => {
                let Some(parent) = fallback_plan
                    .dependencies
                    .iter()
                    .find(|candidate| candidate.version_id == version_id)
                else {
                    resolution_plan.issues.push(DependencyResolutionIssue {
						provider: ContentProvider::Modrinth,
						project_id: dependency.project_id.clone(),
						parent: Some(curseforge_parent.clone()),
						relation: Some(
							crate::state::instances::ContentDependencyKind::Required,
						),
						reason: "dependency_parent_unresolved".to_string(),
					});
                    continue;
                };
                ContentProviderRef::Modrinth {
                    project_id: ModrinthProjectId::new(
                        parent.project_id.clone(),
                    )?,
                    version_id: Some(ModrinthVersionId::new(
                        parent.version_id.clone(),
                    )?),
                }
            }
            None => curseforge_parent.clone(),
        };
        let version = CachedEntry::get_version(
            &ModrinthVersionId::new(dependency.version_id.clone())?,
            Some(CacheBehaviour::MustRevalidate),
            &state.pool,
            &state.api_semaphore,
        )
        .await?;
        let Some(version) = version else {
            resolution_plan.issues.push(DependencyResolutionIssue {
                provider: ContentProvider::Modrinth,
                project_id: dependency.project_id.clone(),
                parent: Some(parent),
                relation: Some(
                    crate::state::instances::ContentDependencyKind::Required,
                ),
                reason: "missing_version".to_string(),
            });
            continue;
        };
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first());
        resolution_plan.edges.push(DependencyResolutionEdge {
            parent: parent.clone(),
            child: content.clone(),
            relation: crate::state::instances::ContentDependencyKind::Required,
            evidence_provider: ContentProvider::Modrinth,
        });
        if !resolution_plan
            .nodes
            .iter()
            .any(|node| node.content == content)
        {
            resolution_plan.nodes.push(DependencyResolutionNode {
                content,
                parent: Some(parent),
                relation:
                    crate::state::instances::ContentDependencyKind::Required,
                source: ContentProvider::Modrinth,
                selection_reason:
                    DependencySelectionReason::Sha1VerifiedModrinthFallback,
                expected_sha1: file
                    .and_then(|file| file.hashes.get("sha1").cloned()),
                expected_size: file.map(|file| file.size as u64),
            });
        }
    }
    Ok(())
}

pub(crate) fn curseforge_content_ref(
    project_id: u32,
    file_id: u32,
) -> crate::Result<ContentProviderRef> {
    Ok(ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(project_id)?,
        file_id: Some(CurseForgeFileId::new(file_id)?),
    })
}

pub(crate) fn curseforge_file_sha1(file: &CurseForgeFile) -> Option<String> {
    file.hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone())
}

pub(crate) async fn instance_content_revision(
    instance_id: &str,
    state: &State,
) -> crate::Result<Option<u64>> {
    if instance_id.is_empty() {
        return Ok(None);
    }
    Ok(
		crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
			instance_id,
			&state.pool,
		)
		.await?
		.map(|content_set| content_set.revision),
	)
}

pub(crate) fn store_dependency_resolution_plan(plan: DependencyResolutionPlan) {
    let now = Instant::now();
    DEPENDENCY_RESOLUTION_PLANS.retain(|_, cached| cached.expires_at > now);
    DEPENDENCY_RESOLUTION_PLANS.insert(
        plan.id.clone(),
        CachedDependencyResolutionPlan {
            plan,
            expires_at: now + DEPENDENCY_PLAN_TTL,
        },
    );
}

impl From<CurseForgeDependencySelectionReason> for DependencySelectionReason {
    fn from(reason: CurseForgeDependencySelectionReason) -> Self {
        match reason {
			CurseForgeDependencySelectionReason::NativeStrictMatch => {
				Self::NativeStrictMatch
			}
			CurseForgeDependencySelectionReason::Sha1VerifiedModrinthFallback => {
				Self::Sha1VerifiedModrinthFallback
			}
		}
    }
}

pub(super) fn select_best_dependency_file(
    files: Vec<CurseForgeFile>,
    game_version: Option<&str>,
    mod_loader_type: Option<u32>,
) -> Option<CurseForgeFile> {
    files
        .into_iter()
        .filter(|file| {
            file.is_available
                && file_matches_dependency_target(
                    file,
                    game_version,
                    mod_loader_type,
                )
        })
        .max_by(|left, right| {
            dependency_release_rank(left)
                .cmp(&dependency_release_rank(right))
                .then_with(|| left.file_date.cmp(&right.file_date))
                .then_with(|| left.id.cmp(&right.id))
        })
}

fn dependency_release_rank(file: &CurseForgeFile) -> u8 {
    match file.release_type {
        1 => 3,
        2 => 2,
        3 => 1,
        _ => 0,
    }
}

/// Checks unfiltered fallback candidates against the instance target.
pub(super) fn file_matches_dependency_target(
    file: &CurseForgeFile,
    game_version: Option<&str>,
    mod_loader_type: Option<u32>,
) -> bool {
    let game_version_matches = game_version.is_none_or(|target| {
        let game_versions_match =
            file.game_versions.iter().any(|version| version == target);
        let sortable_game_versions_match =
            file.sortable_game_versions.iter().any(|version| {
                version.game_version.as_deref() == Some(target)
                    || version.game_version_name == target
            });

        match (
            file.game_versions.is_empty(),
            file.sortable_game_versions.is_empty(),
        ) {
            (true, true) => false,
            (false, true) => game_versions_match,
            (true, false) => sortable_game_versions_match,
            (false, false) => {
                game_versions_match && sortable_game_versions_match
            }
        }
    });
    let mod_loader_matches = mod_loader_type.is_none_or(|target| {
        file.game_versions.iter().any(|version| {
            version.eq_ignore_ascii_case(mod_loader_to_slug(target))
        })
    });

    game_version_matches && mod_loader_matches
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

pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(crate) async fn report_modpack_progress(
    loading_bar: Option<&crate::event::LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
    details: InstallPhaseDetails,
    files_done: &AtomicU64,
    bytes_done: &AtomicU64,
    active_downloads: &AtomicU64,
    total_files: u64,
    total_bytes: u64,
    file_bytes: u64,
    event: InstallJobEventKind,
) -> crate::Result<()> {
    let current_files = files_done.fetch_add(1, Ordering::Relaxed) + 1;
    let current_bytes =
        bytes_done.fetch_add(file_bytes, Ordering::Relaxed) + file_bytes;
    let active = active_downloads.load(Ordering::Relaxed);
    let message = if total_bytes > 0 {
        format!(
            "{current_files}/{total_files} files · {} / {} · {active} downloading in parallel",
            format_bytes(current_bytes.min(total_bytes)),
            format_bytes(total_bytes)
        )
    } else {
        format!(
            "{current_files}/{total_files} files · {active} downloading in parallel"
        )
    };
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 1.0, Some(&message))?;
    }
    if let Some(reporter) = reporter {
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: current_files,
                    total: total_files,
                    secondary: Some(InstallProgressSecondary {
                        current: current_bytes.min(total_bytes),
                        total: total_bytes,
                    }),
                }),
                details,
                vec![event],
            )
            .await?;
    }
    Ok(())
}

pub(crate) fn is_forge_cdn_url(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host == "forgecdn.net" || host.ends_with(".forgecdn.net")
}

pub(crate) fn curseforge_file_page_url(
    website_url: Option<&str>,
    file_id: u32,
) -> Option<String> {
    let website_url = website_url?;
    let Ok(mut url) = reqwest::Url::parse(website_url) else {
        return Some(website_url.to_owned());
    };
    if !matches!(
        url.host_str(),
        Some("curseforge.com" | "www.curseforge.com" | "legacy.curseforge.com")
    ) {
        return Some(website_url.to_owned());
    }

    let mut path = url.path().trim_end_matches('/').to_string();
    for marker in ["/files/", "/download/"] {
        if let Some(index) = path.rfind(marker) {
            path.truncate(index);
            break;
        }
    }
    url.set_path(&format!("{path}/download/{file_id}"));
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}

pub(super) fn validate_cdn_url(url: &reqwest::Url) -> crate::Result<()> {
    #[cfg(debug_assertions)]
    if url.scheme() == "http"
        && matches!(
            url.host_str()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str(),
            "127.0.0.1" | "localhost"
        )
    {
        return Ok(());
    }
    if url.scheme() != "https" || !is_forge_cdn_url(url) {
        return Err(ErrorKind::InputError(
            "CurseForge returned a download URL outside its CDN".to_string(),
        )
        .into());
    }
    Ok(())
}

pub(crate) fn curseforge_content_validation(
    file_name: &str,
) -> ContentValidation {
    match Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jar" | "zip" | "mrpack") => ContentValidation::Jar,
        _ => ContentValidation::None,
    }
}

pub(crate) fn curseforge_integrity(
    file: &CurseForgeFile,
    validation: ContentValidation,
) -> Integrity {
    Integrity {
        size: Some(file.file_length),
        sha1: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone()),
        md5: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 2)
            .map(|hash| hash.value.clone()),
        content: validation,
        ..Integrity::default()
    }
}

pub(crate) const fn curseforge_modpack_h2_range_concurrency(
    size: u64,
) -> Option<usize> {
    if size >= CURSEFORGE_H2_RANGE_THRESHOLD {
        Some(CURSEFORGE_REQUESTED_H2_RANGES)
    } else {
        None
    }
}

pub(crate) fn curseforge_candidate_urls(
    url: &str,
) -> crate::Result<Vec<String>> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        ErrorKind::InputError(
            "CurseForge returned an invalid download URL".to_string(),
        )
    })?;
    validate_cdn_url(&parsed)?;
    if !is_forge_cdn_url(&parsed) {
        return Ok(Vec::new());
    }

    let original_host = parsed.host_str().unwrap_or_default();
    let mut candidates = Vec::new();
    for host in [
        "edge.forgecdn.net",
        "media.forgecdn.net",
        "mediafilez.forgecdn.net",
    ] {
        if host == original_host {
            continue;
        }
        let mut candidate = parsed.clone();
        candidate.set_host(Some(host)).map_err(|_| {
            ErrorKind::InputError(
                "CurseForge returned an invalid CDN URL".to_string(),
            )
        })?;
        candidates.push(candidate.to_string());
    }
    Ok(candidates)
}
