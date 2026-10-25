//! CurseForge install dependencies.

use super::*;
use crate::State;
use crate::api::curseforge::catalog::get_project;
use crate::api::curseforge::constants::{
    CURSEFORGE_LOADER_QUILT, MAX_DEPENDENCY_DEPTH,
};
use crate::api::curseforge::dto::{
    CurseForgeFailedDownload, CurseForgeFile, CurseForgeFileDependency,
    CurseForgeInstallRequest, CurseForgeInstallResult, CurseForgeProject,
    CurseForgeSkippedDependency,
};
use crate::api::curseforge::mapping::recognized_project_type;
use crate::api::curseforge::selection::{
    resolve_modrinth_fallback_plan, select_dependency_file,
};
use crate::state::ProjectType;
use crate::state::instances::DependencyRelation;
use content_management::{DependencyPolicy, DependencyType};
use std::collections::{HashMap, HashSet};

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
    relation: DependencyRelation,
    operation: crate::state::instances::ManualDownloadOperationKind,
) -> bool {
    let dependency_type = match relation {
        DependencyRelation::Required => DependencyType::Required,
        DependencyRelation::Optional | DependencyRelation::Include => {
            DependencyType::Optional
        }
        _ => return false,
    };
    dependency_policy(operation).allows(dependency_type)
}

/// The outcome of evaluating one declared dependency against a request.
pub(crate) enum DependencyAction {
    /// Do not resolve it; `reason` is what the results report, and
    /// `target_project_id` is the project they name: the resolved target,
    /// because exclusions and installed checks are keyed by it.
    Skip {
        reason: &'static str,
        target_project_id: u32,
    },
    /// Declared incompatible with the requested content.
    Incompatible,
    /// The operation does not resolve this kind of dependency at all.
    Filtered,
    /// Resolve this project as a dependency.
    Resolve { target_project_id: u32 },
}

/// Decides what a dependency pass should do with one manifest entry.
///
/// Preview and installation walk the same graph; keeping the decision here
/// avoids two copies of the relation dispatch and its skip reasons.
pub(crate) fn dependency_action(
    dependency: &CurseForgeFileDependency,
    request: &CurseForgeInstallRequest,
    installed_project_ids: &[String],
) -> DependencyAction {
    let relation = dependency.relation();
    let target_project_id = dependency.target_project_id(
        request.mod_loader_type == Some(CURSEFORGE_LOADER_QUILT),
    );
    match relation {
        DependencyRelation::Embedded => DependencyAction::Skip {
            reason: "embedded",
            target_project_id,
        },
        DependencyRelation::Tool => DependencyAction::Skip {
            reason: "tool",
            target_project_id,
        },
        DependencyRelation::Incompatible => DependencyAction::Incompatible,
        DependencyRelation::Optional
        | DependencyRelation::Include
        | DependencyRelation::Required => {
            if !should_resolve_dependency(
                relation,
                request.manual_operation_kind,
            ) {
                return DependencyAction::Filtered;
            }
            if request
                .excluded_dependency_project_ids
                .contains(&target_project_id)
            {
                return DependencyAction::Skip {
                    reason: "excluded_by_user",
                    target_project_id,
                };
            }
            if installed_project_ids
                .contains(&format!("curseforge:{target_project_id}"))
                && !request
                    .force_dependency_project_ids
                    .contains(&target_project_id)
            {
                return DependencyAction::Skip {
                    reason: "already_installed",
                    target_project_id,
                };
            }
            DependencyAction::Resolve { target_project_id }
        }
        DependencyRelation::Unknown(_) => DependencyAction::Filtered,
    }
}

/// What a caller must record for one dependency decision.
///
/// Keeping the reporting here means the entry points cannot disagree about
/// which project a record names.
pub(crate) enum DependencyStep {
    /// Nothing to record and nothing to resolve.
    Continue,
    /// Record the skip; it already names the resolved target project.
    Skipped(CurseForgeSkippedDependency),
    /// Record the declared project the parent marked incompatible.
    Incompatible(u32),
    /// Resolve this project as a dependency.
    Resolve { target_project_id: u32 },
}

pub(crate) fn dependency_step(
    dependency: &CurseForgeFileDependency,
    action: DependencyAction,
) -> DependencyStep {
    match action {
        DependencyAction::Skip {
            reason,
            target_project_id,
        } => DependencyStep::Skipped(CurseForgeSkippedDependency {
            project_id: target_project_id,
            file_id: None,
            reason: reason.to_string(),
        }),
        DependencyAction::Incompatible => {
            DependencyStep::Incompatible(dependency.mod_id)
        }
        DependencyAction::Filtered => DependencyStep::Continue,
        DependencyAction::Resolve { target_project_id } => {
            DependencyStep::Resolve { target_project_id }
        }
    }
}

/// The dependency kind a manifest relation implies for the plan and results.
pub(crate) fn content_dependency_kind(
    relation: DependencyRelation,
) -> crate::state::instances::ContentDependencyKind {
    use crate::state::instances::ContentDependencyKind;
    if relation.is_required() {
        ContentDependencyKind::Required
    } else {
        ContentDependencyKind::Include
    }
}

pub(crate) async fn enqueue_curseforge_dependencies(
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
        let dependency_project_id = match dependency_step(
            dependency_ref,
            dependency_action(dependency_ref, request, installed_project_ids),
        ) {
            DependencyStep::Skipped(skipped) => {
                result.skipped_dependencies.push(skipped);
                continue;
            }
            DependencyStep::Incompatible(project_id) => {
                result.incompatible_dependencies.push(project_id);
                continue;
            }
            DependencyStep::Continue => continue,
            DependencyStep::Resolve { target_project_id } => target_project_id,
        };
        let dependency_project = match projects.get(&dependency_project_id) {
            Some(project) => project.clone(),
            None => {
                let project = get_project(dependency_project_id).await?;
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
                dependency_kind: Some(content_dependency_kind(
                    dependency_ref.relation(),
                )),
                depth: pending_file.depth + 1,
                file: Some(selected.file.clone()),
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
                            reason: "modrinth_lookup_failed".to_string(),
                        },
                    );
                }
            }
        } else {
            result
                .skipped_dependencies
                .push(CurseForgeSkippedDependency {
                    project_id: dependency_project_id,
                    file_id: None,
                    reason: "no_compatible_version".to_string(),
                });
        }
    }
    Ok(())
}

/// Completes the common post-processing for a CurseForge installation.
///
/// Keeping fallback installation, result normalization, and dependency-edge
/// persistence together makes the main dependency walk easier to reason about
/// and gives preview/resolution-plan flows a single place to reuse later.
pub(crate) async fn finalize_install_result(
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
