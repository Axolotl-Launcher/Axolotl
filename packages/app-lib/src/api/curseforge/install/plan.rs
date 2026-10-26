//! CurseForge install plan.

use super::*;
use crate::api::curseforge::catalog::get_project;
use crate::api::curseforge::dto::{
    CurseForgeCrossSourceDependency, CurseForgeFailedDownload,
    CurseForgeInstallRequest, CurseForgeInstallResult,
    CurseForgeSkippedDependency,
};
use crate::api::curseforge::mapping::{
    mod_loader_to_slug, recognized_project_type,
};
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::plans::DEPENDENCY_RESOLUTION_PLANS;
use crate::api::curseforge::selection::{
    curseforge_content_ref, instance_content_revision, managed_project_type,
};
use crate::state::{
    ContentProviderRef, CurseForgeFileId, DependencyResolutionNode,
    DependencyResolutionPlan, DependencyResolutionTarget,
};
use crate::{ErrorKind, State};
use std::collections::HashSet;
use std::time::Instant;

pub(crate) async fn load_dependency_resolution_plan(
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

pub(crate) async fn install_file_from_resolution_plan(
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

pub(crate) async fn install_resolution_plan_node(
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

pub(crate) fn modrinth_version_matches_plan_target(
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

pub(crate) fn curseforge_project_id(
    reference: &ContentProviderRef,
) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { project_id, .. } => {
            Some(project_id.get())
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}

pub(crate) fn curseforge_file_id(
    reference: &ContentProviderRef,
) -> Option<u32> {
    match reference {
        ContentProviderRef::CurseForge { file_id, .. } => {
            file_id.map(CurseForgeFileId::get)
        }
        ContentProviderRef::Modrinth { .. } => None,
        ContentProviderRef::McArchive { .. } => None,
    }
}
