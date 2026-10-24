//! Content installation and dependency resolution.

use super::catalog::{get_file, get_project};
use super::download::resolve_download_url as resolve_curseforge_download_url;
use super::dto::{
    CurseForgeCrossSourceDependency, CurseForgeFailedDownload, CurseForgeFile,
    CurseForgeInstallRequest, CurseForgeInstallResult, CurseForgeInstalledFile,
    CurseForgeProject, CurseForgeSkippedDependency,
    CurseForgeWorldInstallRequest, CurseForgeWorldInstallResult, default_true,
};
use super::mapping::{mod_loader_to_slug, recognized_project_type};
use super::metrics::CurseForgeDownloadMetrics;
use super::selection::{
    content_target_folder, curseforge_content_ref,
    curseforge_content_validation, curseforge_file_sha1,
    instance_content_revision, managed_project_type, manual_download_from_file,
    persist_manual_download, resolve_modrinth_fallback_plan,
    select_dependency_file,
};
use super::validation::{validate_file_name, validate_world_archive_name};
use super::verify::{
    DownloadInstalledFileRequest, download_curseforge_path,
    download_installed_file,
};
use super::{
    CURSEFORGE_LOADER_QUILT, DEPENDENCY_RELATION_EMBEDDED,
    DEPENDENCY_RELATION_INCLUDE, DEPENDENCY_RELATION_INCOMPATIBLE,
    DEPENDENCY_RELATION_OPTIONAL, DEPENDENCY_RELATION_REQUIRED,
    DEPENDENCY_RELATION_TOOL, DEPENDENCY_RESOLUTION_PLANS,
    FABRIC_API_CURSEFORGE_PROJECT_ID, MAX_DEPENDENCY_DEPTH,
    QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
};
use crate::install::InstallProgressReporter;
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProvider, ContentProviderRef,
    CurseForgeFileId, DependencyResolutionNode, DependencyResolutionPlan,
    DependencyResolutionTarget, InstanceInstallStage, ProjectType,
};
use crate::{ErrorKind, State};
use content_management::{DependencyPolicy, DependencyType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub async fn install_file(
    request: CurseForgeInstallRequest,
) -> crate::Result<CurseForgeInstallResult> {
    install_file_with_metrics(request, None).await
}

pub async fn install_file_with_reporter(
    request: CurseForgeInstallRequest,
    reporter: InstallProgressReporter,
) -> crate::Result<CurseForgeInstallResult> {
    let metrics = CurseForgeDownloadMetrics::with_reporter(reporter.clone());
    let result = install_file_with_metrics(request, Some(&metrics)).await?;
    metrics.finish(&reporter).await?;
    Ok(result)
}

pub async fn install_world_with_reporter(
    request: CurseForgeWorldInstallRequest,
    reporter: InstallProgressReporter,
) -> crate::Result<CurseForgeWorldInstallResult> {
    let state = State::get().await?;
    let instance =
        crate::state::get_instance(&request.instance_id, &state.pool)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError("Unknown instance".to_string())
            })?;
    if instance.instance.install_stage != InstanceInstallStage::Installed {
        return Err(ErrorKind::InputError(
            "Maps can only be added to an installed instance".to_string(),
        )
        .into());
    }

    let project = get_project(request.project_id).await?;
    if project.id != request.project_id || project.class_id != Some(17) {
        return Err(ErrorKind::InputError(
            "The selected CurseForge project is not a Minecraft world"
                .to_string(),
        )
        .into());
    }

    let file = get_file(request.project_id, request.file_id).await?;
    if file.mod_id != request.project_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned a file for a different project".to_string(),
        )
        .into());
    }
    validate_world_archive_name(&file.file_name)?;

    install_world_file(
        &request.instance_id,
        &project,
        &file,
        Some(&reporter),
        crate::state::instances::ContentOwnershipKind::UserAdded,
        crate::state::instances::ManualDownloadOperationKind::ContentInstall,
    )
    .await
}

pub(crate) async fn install_world_file(
    instance_id: &str,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    reporter: Option<&InstallProgressReporter>,
    ownership: crate::state::instances::ContentOwnershipKind,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> crate::Result<CurseForgeWorldInstallResult> {
    validate_world_archive_name(&file.file_name)?;
    if project.class_id != Some(17) || file.mod_id != project.id {
        return Err(ErrorKind::InputError(
            "World file does not belong to the selected world project".into(),
        )
        .into());
    }
    let state = State::get().await?;
    let download_url =
        resolve_curseforge_download_url(project.id, file.id, project, file)
            .await?;
    let Some(download_url) = download_url else {
        let manual_download = manual_download_from_file(
            project.id,
            file.id,
            file,
            project,
            "world",
            "saves".to_string(),
            ownership,
            operation,
        );
        persist_manual_download(instance_id, &manual_download).await?;
        return Ok(CurseForgeWorldInstallResult {
            manual_download: Some(manual_download),
            ..Default::default()
        });
    };

    let staging_directory = tempfile::tempdir()?;
    let staging_path = staging_directory.path().join(
        crate::util::io::sanitize_download_file_name(&file.file_name)?,
    );
    let tracking_path = format!("worlds/{}/{}", project.id, file.id);
    download_curseforge_path(
        &download_url,
        file,
        &staging_path,
        curseforge_content_validation(&file.file_name),
        None,
        reporter.map(|reporter| (reporter, tracking_path.as_str())),
        None,
        true,
    )
    .await?;
    let _critical = crate::install::critical_section::enter();
    let world_name = crate::state::instances::commands::import_world_save(
        &state,
        instance_id,
        &staging_path,
        None,
    )
    .await?;

    Ok(CurseForgeWorldInstallResult {
        world_name: Some(world_name),
        ..Default::default()
    })
}

#[derive(Clone, Copy)]
pub(super) struct PendingCurseForgeFile {
    pub(super) project_id: u32,
    pub(super) file_id: u32,
    pub(super) item_type: ProjectType,
    pub(super) dependency: bool,
    pub(super) parent_project_id: Option<u32>,
    pub(super) parent_file_id: Option<u32>,
    pub(super) dependency_kind:
        Option<crate::state::instances::ContentDependencyKind>,
    pub(super) depth: usize,
}

#[derive(Clone)]
pub(crate) struct PreviewPendingCurseForgeFile {
    pub(super) project_id: u32,
    pub(super) file_id: u32,
    pub(super) item_type: ProjectType,
    pub(super) depth: usize,
    pub(super) ancestors: HashSet<(u32, u32)>,
}

#[derive(Clone)]
pub(super) struct CurseForgeDependencyEdgeCandidate {
    pub(super) parent_project_id: u32,
    pub(super) parent_file_id: u32,
    pub(super) child_project_id: u32,
    pub(super) child_file_id: u32,
    pub(super) dependency_kind: crate::state::instances::ContentDependencyKind,
}

#[derive(Clone)]
pub(super) struct ModrinthFallbackPlan {
    pub(super) parent_project_id: u32,
    pub(super) parent_file_id: u32,
    pub(super) plan: content_management::ResolveContentPlan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModrinthFallbackPreview {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub parent_project_id: u32,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

pub(super) async fn install_file_with_metrics(
    request: CurseForgeInstallRequest,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
) -> crate::Result<CurseForgeInstallResult> {
    let state = State::get().await?;
    let project_type = managed_project_type(&request.project_type)?;
    if let Some(plan) =
        load_dependency_resolution_plan(&request, &state).await?
    {
        let result = install_file_from_resolution_plan(
            &request,
            &plan,
            download_metrics,
            &state,
        )
        .await;
        if result.is_ok() {
            DEPENDENCY_RESOLUTION_PLANS.remove(&plan.id);
        }
        return result;
    }
    let mut result = CurseForgeInstallResult::default();
    let mut visited = HashSet::new();
    let mut projects = HashMap::<u32, CurseForgeProject>::new();
    let mut edge_candidates = Vec::new();
    let mut modrinth_fallbacks = Vec::new();
    let mut fallback_parents = HashSet::new();
    let installed_project_ids =
        crate::state::get_installed_project_ids_for_instance(
            &request.instance_id,
            None,
            &state,
        )
        .await?;
    let mut pending = vec![PendingCurseForgeFile {
        project_id: request.project_id,
        file_id: request.file_id,
        item_type: project_type,
        dependency: false,
        parent_project_id: None,
        parent_file_id: None,
        dependency_kind: None,
        depth: 0,
    }];

    while let Some(pending_file) = pending.pop() {
        if !visited.insert((pending_file.project_id, pending_file.file_id)) {
            continue;
        }
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        let dependency = pending_file.dependency;

        let file = get_file(project_id, file_id).await?;
        let project = match projects.get(&project_id) {
            Some(project) => project.clone(),
            None => {
                let project = get_project(project_id).await?;
                projects.insert(project_id, project.clone());
                project
            }
        };
        enqueue_curseforge_dependencies(
            &pending_file,
            &file,
            project_id,
            file_id,
            item_type,
            &request,
            &state,
            &installed_project_ids,
            &mut projects,
            &mut pending,
            &mut result,
            &mut modrinth_fallbacks,
            &mut fallback_parents,
        )
        .await?;

        let download_url = resolve_curseforge_download_url(
            project_id, file_id, &project, &file,
        )
        .await?;
        let Some(download_url) = download_url else {
            let target_folder = content_target_folder(
                item_type,
                request.world_name.as_deref(),
            )?;
            let manual_download = manual_download_from_file(
                project_id,
                file_id,
                &file,
                &project,
                item_type.get_name(),
                target_folder,
                request.ownership_kind,
                request.manual_operation_kind,
            );
            persist_manual_download(&request.instance_id, &manual_download)
                .await?;
            result.manual_downloads.push(manual_download);
            continue;
        };

        validate_file_name(&file.file_name)?;
        let downloaded =
            download_installed_file(DownloadInstalledFileRequest {
                instance_id: &request.instance_id,
                url: &download_url,
                file: &file,
                project_type: item_type,
                world_name: request.world_name.as_deref(),
                project_id,
                file_id,
                project_slug: &project.slug,
                ownership_kind: request.ownership_kind,
                download_metrics,
                defer_persistence: request.defer_persistence,
                verification_tx: request.verification_tx.as_ref(),
                pre_resolved_relative_path: request
                    .pre_resolved_relative_path
                    .as_deref(),
                expected_file_name: request.expected_file_name.as_deref(),
            })
            .await?;
        let relative_path = downloaded.relative_path.clone();
        result.installed.push(CurseForgeInstalledFile {
            project_id,
            file_id,
            relative_path: relative_path.clone(),
            dependency,
        });
        if let (
            Some(parent_project_id),
            Some(parent_file_id),
            Some(dependency_kind),
        ) = (
            pending_file.parent_project_id,
            pending_file.parent_file_id,
            pending_file.dependency_kind,
        ) {
            edge_candidates.push(CurseForgeDependencyEdgeCandidate {
                parent_project_id,
                parent_file_id,
                child_project_id: project_id,
                child_file_id: file_id,
                dependency_kind,
            });
        }
    }

    finalize_install_result(
        &request.instance_id,
        &modrinth_fallbacks,
        &mut result,
        &edge_candidates,
        &state,
    )
    .await?;
    Ok(result)
}

pub(crate) fn dependency_policy(
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> DependencyPolicy {
    if operation
        == crate::state::instances::ManualDownloadOperationKind::ContentUpdate
    {
        DependencyPolicy::RequiredOnly
    } else {
        DependencyPolicy::IncludeOptional
    }
}

pub(crate) fn should_resolve_dependency(
    relation_type: u32,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> bool {
    let dependency_type = match relation_type {
        DEPENDENCY_RELATION_REQUIRED => DependencyType::Required,
        DEPENDENCY_RELATION_OPTIONAL | DEPENDENCY_RELATION_INCLUDE => {
            DependencyType::Optional
        }
        _ => return false,
    };
    dependency_policy(operation).allows(dependency_type)
}

async fn enqueue_curseforge_dependencies(
    pending_file: &PendingCurseForgeFile,
    file: &CurseForgeFile,
    project_id: u32,
    file_id: u32,
    item_type: ProjectType,
    request: &CurseForgeInstallRequest,
    state: &State,
    installed_project_ids: &[String],
    projects: &mut HashMap<u32, CurseForgeProject>,
    pending: &mut Vec<PendingCurseForgeFile>,
    result: &mut CurseForgeInstallResult,
    modrinth_fallbacks: &mut Vec<ModrinthFallbackPlan>,
    fallback_parents: &mut HashSet<(u32, u32)>,
) -> crate::Result<()> {
    if !request.install_dependencies {
        return Ok(());
    }
    if pending_file.depth >= MAX_DEPENDENCY_DEPTH {
        result
            .skipped_dependencies
            .push(CurseForgeSkippedDependency {
                project_id,
                file_id: Some(file_id),
                reason: "dependency_depth_exceeded".to_string(),
            });
        return Ok(());
    }

    for dependency_ref in &file.dependencies {
        match dependency_ref.relation_type {
            DEPENDENCY_RELATION_EMBEDDED | DEPENDENCY_RELATION_TOOL => {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: dependency_ref.mod_id,
                        file_id: None,
                        reason: if dependency_ref.relation_type
                            == DEPENDENCY_RELATION_EMBEDDED
                        {
                            "embedded"
                        } else {
                            "tool"
                        }
                        .to_string(),
                    });
            }
            DEPENDENCY_RELATION_INCOMPATIBLE => {
                result.incompatible_dependencies.push(dependency_ref.mod_id)
            }
            DEPENDENCY_RELATION_OPTIONAL
            | DEPENDENCY_RELATION_INCLUDE
            | DEPENDENCY_RELATION_REQUIRED => {
                if !should_resolve_dependency(
                    dependency_ref.relation_type,
                    request.manual_operation_kind,
                ) {
                    continue;
                }
                let dependency_project_id = if request.mod_loader_type
                    == Some(CURSEFORGE_LOADER_QUILT)
                    && dependency_ref.mod_id == FABRIC_API_CURSEFORGE_PROJECT_ID
                {
                    QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
                } else {
                    dependency_ref.mod_id
                };
                if request
                    .excluded_dependency_project_ids
                    .contains(&dependency_project_id)
                {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: "excluded_by_user".to_string(),
                        },
                    );
                    continue;
                }
                if installed_project_ids
                    .contains(&format!("curseforge:{dependency_project_id}"))
                    && !request
                        .force_dependency_project_ids
                        .contains(&dependency_project_id)
                {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: "already_installed".to_string(),
                        },
                    );
                    continue;
                }
                let dependency_project = match projects
                    .get(&dependency_project_id)
                {
                    Some(project) => project.clone(),
                    None => {
                        let project =
                            get_project(dependency_project_id).await?;
                        projects.insert(dependency_project_id, project.clone());
                        project
                    }
                };
                let Some(dependency_type) =
                    recognized_project_type(dependency_project.class_id)
                else {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id: dependency_project_id,
                        file_id: 0,
                        file_name: dependency_project.name.clone(),
                        reason: "The dependency project type is not supported"
                            .to_string(),
                    });
                    continue;
                };
                if let Some(selected) = select_dependency_file(
                    dependency_project_id,
                    request.game_version.clone(),
                    request.mod_loader_type,
                )
                .await?
                {
                    pending.push(PendingCurseForgeFile {
                        project_id: dependency_project_id,
                        file_id: selected.file.id,
                        item_type: dependency_type,
                        dependency: true,
                        parent_project_id: Some(project_id),
                        parent_file_id: Some(file_id),
                        dependency_kind: Some(if dependency_ref.relation_type
                            == DEPENDENCY_RELATION_REQUIRED
                        {
                            crate::state::instances::ContentDependencyKind::Required
                        } else {
                            crate::state::instances::ContentDependencyKind::Include
                        }),
                        depth: pending_file.depth + 1,
                    });
                } else if fallback_parents.insert((project_id, file_id)) {
                    match resolve_modrinth_fallback_plan(
                        file, item_type, request, state,
                    )
                    .await
                    {
                        Ok(Some(plan)) if !plan.dependencies.is_empty() => {
                            modrinth_fallbacks.push(ModrinthFallbackPlan {
                                parent_project_id: project_id,
                                parent_file_id: file_id,
                                plan,
                            });
                        }
                        Ok(Some(_) | None) => {
                            result.skipped_dependencies.push(
                                CurseForgeSkippedDependency {
                                    project_id: dependency_project_id,
                                    file_id: None,
                                    reason: "no_compatible_version".to_string(),
                                },
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                project_id = dependency_project_id,
                                parent_project_id = project_id,
                                parent_file_id = file_id,
                                "Modrinth SHA-1 dependency fallback failed: {error}"
                            );
                            result.skipped_dependencies.push(
                                CurseForgeSkippedDependency {
                                    project_id: dependency_project_id,
                                    file_id: None,
                                    reason: "modrinth_lookup_failed"
                                        .to_string(),
                                },
                            );
                        }
                    }
                } else {
                    result.skipped_dependencies.push(
                        CurseForgeSkippedDependency {
                            project_id: dependency_project_id,
                            file_id: None,
                            reason: "no_compatible_version".to_string(),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Completes the common post-processing for a CurseForge installation.
///
/// Keeping fallback installation, result normalization, and dependency-edge
/// persistence together makes the main dependency walk easier to reason about
/// and gives preview/resolution-plan flows a single place to reuse later.
async fn finalize_install_result(
    instance_id: &str,
    modrinth_fallbacks: &[ModrinthFallbackPlan],
    result: &mut CurseForgeInstallResult,
    edge_candidates: &[CurseForgeDependencyEdgeCandidate],
    state: &State,
) -> crate::Result<()> {
    install_modrinth_fallbacks(instance_id, modrinth_fallbacks, result, state)
        .await?;

    result.optional_dependencies.sort_unstable();
    result.optional_dependencies.dedup();
    result.incompatible_dependencies.sort_unstable();
    result.incompatible_dependencies.dedup();
    result.skipped_dependencies.sort_unstable();
    result.skipped_dependencies.dedup();

    persist_curseforge_dependency_edges(instance_id, edge_candidates, state)
        .await?;
    Ok(())
}

async fn load_dependency_resolution_plan(
    request: &CurseForgeInstallRequest,
    state: &State,
) -> crate::Result<Option<DependencyResolutionPlan>> {
    let Some(plan_id) = request.dependency_plan_id.as_deref() else {
        return Ok(None);
    };
    let Some(cached) = DEPENDENCY_RESOLUTION_PLANS
        .get(plan_id)
        .map(|cached| cached.clone())
    else {
        return Err(ErrorKind::InputError(
            "The dependency resolution plan has expired".to_string(),
        )
        .into());
    };
    if cached.expires_at <= Instant::now() {
        DEPENDENCY_RESOLUTION_PLANS.remove(plan_id);
        return Err(ErrorKind::InputError(
            "The dependency resolution plan has expired".to_string(),
        )
        .into());
    }
    let expected_primary =
        curseforge_content_ref(request.project_id, request.file_id)?;
    if cached.plan.instance_id != request.instance_id
        || cached.plan.primary != expected_primary
        || cached.plan.target.minecraft_version != request.game_version
        || cached.plan.target.loader
            != request
                .mod_loader_type
                .map(mod_loader_to_slug)
                .map(str::to_string)
        || cached.plan.target.content_type.as_deref()
            != Some(&request.project_type)
    {
        return Err(ErrorKind::InputError(
			"The dependency resolution plan does not match this install request"
				.to_string(),
		)
		.into());
    }
    let current_revision =
        instance_content_revision(&request.instance_id, state).await?;
    if cached.plan.instance_revision != current_revision {
        DEPENDENCY_RESOLUTION_PLANS.remove(plan_id);
        return Err(ErrorKind::InputError(
			"The instance content changed after dependency preview; preview again"
				.to_string(),
		)
		.into());
    }
    Ok(Some(cached.plan))
}

async fn install_file_from_resolution_plan(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    state: &State,
) -> crate::Result<CurseForgeInstallResult> {
    let primary_type = managed_project_type(&request.project_type)?;
    let mut result = CurseForgeInstallResult::default();
    let mut installed = HashSet::new();
    let mut failed = HashSet::new();

    match install_fixed_curseforge_content(
        request,
        request.project_id,
        request.file_id,
        primary_type,
        false,
        plan.primary_expected_sha1.as_deref(),
        plan.primary_expected_size,
        download_metrics,
        &mut result,
    )
    .await
    {
        Ok(true) => {
            installed.insert(plan.primary.clone());
        }
        Ok(false) => {
            failed.insert(plan.primary.clone());
        }
        Err(error) => {
            result.failed_downloads.push(CurseForgeFailedDownload {
                project_id: request.project_id,
                file_id: request.file_id,
                file_name: format!(
                    "CurseForge {}:{}",
                    request.project_id, request.file_id
                ),
                reason: error.to_string(),
            });
            failed.insert(plan.primary.clone());
        }
    }

    let known_refs = std::iter::once(plan.primary.clone())
        .chain(plan.nodes.iter().map(|node| node.content.clone()))
        .collect::<HashSet<_>>();
    let excluded_project_ids = request
        .excluded_dependency_project_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut pending = Vec::new();
    for node in plan.nodes.clone() {
        let excluded = match &node.content {
            ContentProviderRef::CurseForge { project_id, .. } => {
                excluded_project_ids.contains(&project_id.get())
            }
            ContentProviderRef::Modrinth { .. } => node
                .parent
                .as_ref()
                .and_then(curseforge_project_id)
                .is_some_and(|project_id| {
                    excluded_project_ids.contains(&project_id)
                }),
            ContentProviderRef::McArchive { .. } => false,
        };
        if excluded {
            result
                .skipped_dependencies
                .push(CurseForgeSkippedDependency {
                    project_id: curseforge_project_id(&node.content)
                        .or_else(|| {
                            node.parent.as_ref().and_then(curseforge_project_id)
                        })
                        .unwrap_or_default(),
                    file_id: curseforge_file_id(&node.content).or_else(|| {
                        node.parent.as_ref().and_then(curseforge_file_id)
                    }),
                    reason: "excluded_by_user".to_string(),
                });
            failed.insert(node.content);
        } else {
            pending.push(node);
        }
    }
    while !pending.is_empty() {
        let mut progressed = false;
        let mut index = 0;
        while index < pending.len() {
            let node = &pending[index];
            let parent_ready = node
                .parent
                .as_ref()
                .is_none_or(|parent| installed.contains(parent));
            let parent_failed = node.parent.as_ref().is_some_and(|parent| {
                failed.contains(parent) || !known_refs.contains(parent)
            });
            if parent_failed {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: curseforge_project_id(&node.content)
                            .unwrap_or_default(),
                        file_id: curseforge_file_id(&node.content),
                        reason: "parent_not_installed".to_string(),
                    });
                failed.insert(node.content.clone());
                pending.remove(index);
                progressed = true;
                continue;
            }
            if !parent_ready {
                index += 1;
                continue;
            }

            let node = pending.remove(index);
            let installed_node = install_resolution_plan_node(
                request,
                plan,
                &node,
                download_metrics,
                state,
                &mut result,
            )
            .await?;
            if installed_node {
                if let Some(ContentProviderRef::CurseForge {
                    project_id,
                    file_id,
                }) = node.parent.as_ref()
                    && let ContentProviderRef::Modrinth {
                        project_id: child_project,
                        version_id: Some(child_version),
                    } = &node.content
                {
                    result.cross_source_dependencies.push(
                        CurseForgeCrossSourceDependency {
                            parent_project_id: project_id.get(),
                            parent_file_id: file_id
                                .map(CurseForgeFileId::get)
                                .unwrap_or_default(),
                            project_id: child_project.to_string(),
                            version_id: child_version.to_string(),
                        },
                    );
                }
                installed.insert(node.content);
            } else {
                failed.insert(node.content);
            }
            progressed = true;
        }
        if !progressed {
            for node in pending.drain(..) {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: curseforge_project_id(&node.content)
                            .unwrap_or_default(),
                        file_id: curseforge_file_id(&node.content),
                        reason: "dependency_cycle".to_string(),
                    });
            }
        }
    }

    result.skipped_dependencies.sort_unstable();
    result.skipped_dependencies.dedup();
    persist_resolution_plan_dependency_edges(
        &request.instance_id,
        plan,
        &installed,
        state,
    )
    .await?;
    Ok(result)
}

async fn install_resolution_plan_node(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    node: &DependencyResolutionNode,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    state: &State,
    result: &mut CurseForgeInstallResult,
) -> crate::Result<bool> {
    match &node.content {
        ContentProviderRef::CurseForge {
            project_id,
            file_id,
        } => {
            let Some(file_id) = file_id else {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id: project_id.get(),
                        file_id: None,
                        reason: "missing_version".to_string(),
                    });
                return Ok(false);
            };
            let project_id = project_id.get();
            let project = get_project(project_id).await?;
            let Some(project_type) = recognized_project_type(project.class_id)
            else {
                result
                    .skipped_dependencies
                    .push(CurseForgeSkippedDependency {
                        project_id,
                        file_id: Some(file_id.get()),
                        reason: "unsupported_project_type".to_string(),
                    });
                return Ok(false);
            };
            match install_fixed_curseforge_content(
                request,
                project_id,
                file_id.get(),
                project_type,
                true,
                node.expected_sha1.as_deref(),
                node.expected_size,
                download_metrics,
                result,
            )
            .await
            {
                Ok(installed) => Ok(installed),
                Err(error) => {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id,
                        file_id: file_id.get(),
                        file_name: project.name,
                        reason: error.to_string(),
                    });
                    Ok(false)
                }
            }
        }
        ContentProviderRef::Modrinth { .. } => {
            match install_fixed_modrinth_content(request, plan, node, state)
                .await
            {
                Ok(()) => Ok(true),
                Err(error) => {
                    result.failed_downloads.push(CurseForgeFailedDownload {
                        project_id: curseforge_project_id(
                            node.parent.as_ref().unwrap_or(&plan.primary),
                        )
                        .unwrap_or_default(),
                        file_id: curseforge_file_id(
                            node.parent.as_ref().unwrap_or(&plan.primary),
                        )
                        .unwrap_or_default(),
                        file_name: format!(
                            "Modrinth {}",
                            node.content.database_project_id()
                        ),
                        reason: error.to_string(),
                    });
                    Ok(false)
                }
            }
        }
        ContentProviderRef::McArchive { .. } => {
            result
                .skipped_dependencies
                .push(CurseForgeSkippedDependency {
                    project_id: 0,
                    file_id: None,
                    reason: "unsupported_provider".to_string(),
                });
            Ok(false)
        }
    }
}

async fn install_fixed_curseforge_content(
    request: &CurseForgeInstallRequest,
    project_id: u32,
    file_id: u32,
    project_type: ProjectType,
    dependency: bool,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    result: &mut CurseForgeInstallResult,
) -> crate::Result<bool> {
    let file = get_file(project_id, file_id).await?;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different planned file"
                .to_string(),
        )
        .into());
    }
    if expected_sha1.is_some_and(|expected| {
        curseforge_file_sha1(&file).as_deref() != Some(expected)
    }) || expected_size.is_some_and(|expected| file.file_length != expected)
    {
        return Err(ErrorKind::InputError(
            "CurseForge file metadata changed after dependency preview"
                .to_string(),
        )
        .into());
    }
    let project = get_project(project_id).await?;
    let Some(download_url) =
        resolve_curseforge_download_url(project_id, file_id, &project, &file)
            .await?
    else {
        let target_folder =
            content_target_folder(project_type, request.world_name.as_deref())?;
        let manual_download = manual_download_from_file(
            project_id,
            file_id,
            &file,
            &project,
            project_type.get_name(),
            target_folder,
            request.ownership_kind,
            request.manual_operation_kind,
        );
        persist_manual_download(&request.instance_id, &manual_download).await?;
        result.manual_downloads.push(manual_download);
        return Ok(false);
    };
    validate_file_name(&file.file_name)?;
    let relative_path = download_installed_file(DownloadInstalledFileRequest {
        instance_id: &request.instance_id,
        url: &download_url,
        file: &file,
        project_type,
        world_name: request.world_name.as_deref(),
        project_id,
        file_id,
        project_slug: &project.slug,
        ownership_kind: request.ownership_kind,
        download_metrics,
        defer_persistence: request.defer_persistence,
        verification_tx: None,
        pre_resolved_relative_path: None,
        expected_file_name: request.expected_file_name.as_deref(),
    })
    .await?;
    result.installed.push(CurseForgeInstalledFile {
        project_id,
        file_id,
        relative_path: relative_path.relative_path,
        dependency,
    });
    Ok(true)
}

async fn install_fixed_modrinth_content(
    request: &CurseForgeInstallRequest,
    plan: &DependencyResolutionPlan,
    node: &DependencyResolutionNode,
    state: &State,
) -> crate::Result<()> {
    let ContentProviderRef::Modrinth {
        project_id,
        version_id: Some(version_id),
    } = &node.content
    else {
        return Err(ErrorKind::InputError(
            "The planned Modrinth dependency has no exact version".to_string(),
        )
        .into());
    };
    let version = CachedEntry::get_version(
        version_id,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The planned Modrinth dependency is no longer available"
                .to_string(),
        )
    })?;
    if version.project_id != project_id.as_str()
        || !modrinth_version_matches_plan_target(&version, &plan.target)
    {
        return Err(ErrorKind::InputError(
			"The planned Modrinth dependency no longer matches the instance target"
				.to_string(),
		)
		.into());
    }
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            ErrorKind::InputError(
                "The planned Modrinth dependency has no downloadable file"
                    .to_string(),
            )
        })?;
    if node.expected_sha1.as_deref()
        != file.hashes.get("sha1").map(String::as_str)
        || node
            .expected_size
            .is_some_and(|size| file.size as u64 != size)
    {
        return Err(ErrorKind::InputError(
			"The planned Modrinth file metadata changed after dependency preview"
				.to_string(),
		)
		.into());
    }
    crate::state::instances::commands::install_resolved_dependency(
        &request.instance_id,
        &content_management::ResolvedContent {
            project_id: project_id.to_string(),
            version_id: version_id.to_string(),
            dependent_on_version_id: None,
            required: true,
            metadata: None,
        },
        state,
    )
    .await?;
    Ok(())
}

fn modrinth_version_matches_plan_target(
    version: &crate::state::Version,
    target: &DependencyResolutionTarget,
) -> bool {
    target.minecraft_version.as_deref().is_none_or(|minecraft| {
        version
            .game_versions
            .iter()
            .any(|candidate| candidate == minecraft)
    }) && target.loader.as_deref().is_none_or(|loader| {
        version
            .loaders
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(loader))
    })
}

fn curseforge_project_id(reference: &ContentProviderRef) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { project_id, .. } => {
            Some(project_id.get())
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}

fn curseforge_file_id(reference: &ContentProviderRef) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { file_id, .. } => {
            file_id.map(CurseForgeFileId::get)
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}

async fn persist_resolution_plan_dependency_edges(
    instance_id: &str,
    plan: &DependencyResolutionPlan,
    installed: &HashSet<ContentProviderRef>,
    state: &State,
) -> crate::Result<()> {
    if plan.edges.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for edge in &plan.edges {
        if !installed.contains(&edge.parent) || !installed.contains(&edge.child)
        {
            continue;
        }
        let (Some(parent_release), Some(child_release)) = (
            edge.parent.database_release_id(),
            edge.child.database_release_id(),
        ) else {
            continue;
        };
        let Some(parent_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
			&scope.content_set_id,
			edge.parent.provider(),
			&edge.parent.database_project_id(),
			&parent_release,
			&state.pool,
		)
		.await?
		else {
			continue;
		};
        let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
			&scope.content_set_id,
			edge.child.provider(),
			&edge.child.database_project_id(),
			&child_release,
			&state.pool,
		)
		.await?
		else {
			continue;
		};
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
			&crate::state::instances::ContentDependencyEdge {
				id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
				content_set_id: scope.content_set_id.clone(),
				parent_entry_id: parent_entry.id,
				child_entry_id: child_entry.id,
				evidence_provider: edge.evidence_provider,
				parent_provider: edge.parent.provider(),
				child_provider: edge.child.provider(),
				dependency_kind: edge.relation,
				parent_project_id: edge.parent.database_project_id(),
				parent_release_id: parent_release,
				child_project_id: edge.child.database_project_id(),
				child_release_id: child_release,
				created_at: now,
				modified_at: now,
			},
			&mut tx,
		)
		.await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

async fn persist_curseforge_dependency_edges(
    instance_id: &str,
    candidates: &[CurseForgeDependencyEdgeCandidate],
    state: &State,
) -> crate::Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    for candidate in candidates {
        let Some(parent_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.parent_project_id.to_string(),
                &candidate.parent_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        let Some(child_entry) =
            crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::CurseForge,
                &candidate.child_project_id.to_string(),
                &candidate.child_file_id.to_string(),
                &state.pool,
            )
            .await?
        else {
            continue;
        };
        crate::state::instances::adapters::sqlite::content_rows::set_content_entry_auto_dependency_in_transaction(
            &child_entry.id,
            true,
            &mut tx,
        )
        .await?;
        let now = chrono::Utc::now();
        crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                content_set_id: scope.content_set_id.clone(),
                parent_entry_id: parent_entry.id,
                child_entry_id: child_entry.id,
                evidence_provider: ContentProvider::CurseForge,
                parent_provider: ContentProvider::CurseForge,
                child_provider: ContentProvider::CurseForge,
                dependency_kind: candidate.dependency_kind,
                parent_project_id: candidate.parent_project_id.to_string(),
                parent_release_id: candidate.parent_file_id.to_string(),
                child_project_id: candidate.child_project_id.to_string(),
                child_release_id: candidate.child_file_id.to_string(),
                created_at: now,
                modified_at: now,
            },
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}

async fn install_modrinth_fallbacks(
    instance_id: &str,
    fallbacks: &[ModrinthFallbackPlan],
    result: &mut CurseForgeInstallResult,
    state: &State,
) -> crate::Result<()> {
    let mut installed_versions_by_parent = Vec::new();

    for fallback in fallbacks {
        let mut installed_versions =
            HashSet::from([fallback.plan.primary.version_id.clone()]);
        for dependency in &fallback.plan.dependencies {
            if dependency
                .dependent_on_version_id
                .as_ref()
                .is_some_and(|parent| !installed_versions.contains(parent))
            {
                continue;
            }
            match crate::state::instances::commands::install_resolved_dependency(
                instance_id, dependency, state,
            )
            .await
            {
                Ok(_) => {
                    installed_versions.insert(dependency.version_id.clone());
                    result.cross_source_dependencies.push(
                        CurseForgeCrossSourceDependency {
                            parent_project_id: fallback.parent_project_id,
                            parent_file_id: fallback.parent_file_id,
                            project_id: dependency.project_id.clone(),
                            version_id: dependency.version_id.clone(),
                        },
                    );
                }
                Err(error) => result.failed_downloads.push(
                    CurseForgeFailedDownload {
                        project_id: fallback.parent_project_id,
                        file_id: fallback.parent_file_id,
                        file_name: format!(
                            "Modrinth {} {}",
                            dependency.project_id, dependency.version_id
                        ),
                        reason: error.to_string(),
                    },
                ),
            }
        }
        installed_versions_by_parent.push((fallback, installed_versions));
    }

    persist_modrinth_fallback_dependency_edges(
        instance_id,
        &installed_versions_by_parent,
        state,
    )
    .await
}

async fn persist_modrinth_fallback_dependency_edges(
    instance_id: &str,
    fallbacks: &[(&ModrinthFallbackPlan, HashSet<String>)],
    state: &State,
) -> crate::Result<()> {
    if fallbacks.is_empty() {
        return Ok(());
    }
    let database_permit = state.acquire_install_db_permit().await?;
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await?;

    for (fallback, installed_versions) in fallbacks {
        for dependency in &fallback.plan.dependencies {
            if !installed_versions.contains(&dependency.version_id) {
                continue;
            }
            let Some(parent_version_id) =
                dependency.dependent_on_version_id.as_deref()
            else {
                continue;
            };
            let parent = if parent_version_id
                == fallback.plan.primary.version_id
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::CurseForge,
                    &fallback.parent_project_id.to_string(),
                    &fallback.parent_file_id.to_string(),
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::CurseForge,
                    fallback.parent_project_id.to_string(),
                    fallback.parent_file_id.to_string(),
                ))
            } else if let Some(parent_content) = fallback
                .plan
                .dependencies
                .iter()
                .find(|content| content.version_id == parent_version_id)
            {
                crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    ContentProvider::Modrinth,
                    &parent_content.project_id,
                    &parent_content.version_id,
                    &state.pool,
                )
                .await?
                .map(|entry| (
                    entry,
                    ContentProvider::Modrinth,
                    parent_content.project_id.clone(),
                    parent_content.version_id.clone(),
                ))
            } else {
                None
            };
            let Some((
                parent_entry,
                parent_provider,
                parent_project_id,
                parent_release_id,
            )) = parent
            else {
                continue;
            };
            let Some(child_entry) = crate::state::instances::adapters::sqlite::content_rows::get_content_entry_by_provider_ref(
                &scope.content_set_id,
                ContentProvider::Modrinth,
                &dependency.project_id,
                &dependency.version_id,
                &state.pool,
            )
            .await?
            else {
                continue;
            };
            let now = chrono::Utc::now();
            crate::state::instances::adapters::sqlite::content_rows::upsert_content_dependency_edge_in_transaction(
                &crate::state::instances::ContentDependencyEdge {
                    id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
                    content_set_id: scope.content_set_id.clone(),
                    parent_entry_id: parent_entry.id,
                    child_entry_id: child_entry.id,
                    evidence_provider: ContentProvider::Modrinth,
                    parent_provider,
                    child_provider: ContentProvider::Modrinth,
                    dependency_kind: crate::state::instances::ContentDependencyKind::Required,
                    parent_project_id,
                    parent_release_id,
                    child_project_id: dependency.project_id.clone(),
                    child_release_id: dependency.version_id.clone(),
                    created_at: now,
                    modified_at: now,
                },
                &mut tx,
            )
            .await?;
        }
    }
    tx.commit().await?;
    drop(database_permit);
    Ok(())
}
