//! Install preview derived from a dependency plan.

use super::catalog::{get_file, get_project};
use super::dto::{
    CurseForgeInstallRequest, CurseForgeProject, CurseForgeSkippedDependency,
    default_true,
};
use super::install::{
    CurseForgeModrinthFallbackPreview, PreviewPendingCurseForgeFile,
    should_resolve_dependency,
};
use super::mapping::{mod_loader_to_slug, recognized_project_type};
use super::selection::{
    CurseForgeDependencySelectionReason, append_modrinth_fallback_plan,
    curseforge_content_ref, curseforge_file_sha1, instance_content_revision,
    managed_project_type, preview_modrinth_fallbacks,
    resolve_modrinth_fallback_plan, select_dependency_file,
    store_dependency_resolution_plan,
};
use super::{
    CURSEFORGE_LOADER_QUILT, DEPENDENCY_RELATION_EMBEDDED,
    DEPENDENCY_RELATION_INCLUDE, DEPENDENCY_RELATION_INCOMPATIBLE,
    DEPENDENCY_RELATION_OPTIONAL, DEPENDENCY_RELATION_REQUIRED,
    DEPENDENCY_RELATION_TOOL, FABRIC_API_CURSEFORGE_PROJECT_ID,
    MAX_DEPENDENCY_DEPTH, QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID,
};
use crate::State;
use crate::state::{
    ContentProvider, DependencyResolutionEdge, DependencyResolutionIssue,
    DependencyResolutionNode, DependencyResolutionPlan,
    DependencyResolutionTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgePreviewItem {
    pub project_id: u32,
    pub file_id: u32,
    pub title: String,
    pub version_number: String,
    pub file_name: String,
    pub size: u64,
    pub required_by_project_ids: Vec<u32>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_mismatch: bool,
    #[serde(default)]
    pub selection_reason: Option<CurseForgeDependencySelectionReason>,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeInstallPreview {
    pub plan_id: String,
    pub primary: CurseForgePreviewItem,
    pub dependencies: Vec<CurseForgePreviewItem>,
    #[serde(default)]
    pub modrinth_fallbacks: Vec<CurseForgeModrinthFallbackPreview>,
    pub skipped: Vec<CurseForgeSkippedDependency>,
    pub optional_dependencies: Vec<u32>,
    pub incompatible_dependencies: Vec<u32>,
}

pub async fn preview_install_file(
    request: CurseForgeInstallRequest,
) -> crate::Result<CurseForgeInstallPreview> {
    let state = State::get().await?;
    let primary_type = managed_project_type(&request.project_type)?;
    let primary_file = get_file(request.project_id, request.file_id).await?;
    let primary_project = get_project(request.project_id).await?;
    let primary_ref =
        curseforge_content_ref(request.project_id, request.file_id)?;
    let instance_revision =
        instance_content_revision(&request.instance_id, &state).await?;
    let mut plan = DependencyResolutionPlan {
        id: format!("dependency-resolution-plan:{}", uuid::Uuid::new_v4()),
        instance_id: request.instance_id.clone(),
        instance_revision,
        target: DependencyResolutionTarget {
            minecraft_version: request.game_version.clone(),
            loader: request
                .mod_loader_type
                .map(mod_loader_to_slug)
                .map(str::to_string),
            content_type: Some(request.project_type.clone()),
        },
        primary: primary_ref.clone(),
        primary_expected_sha1: curseforge_file_sha1(&primary_file),
        primary_expected_size: Some(primary_file.file_length),
        nodes: Vec::new(),
        edges: Vec::new(),
        issues: Vec::new(),
    };
    let installed_project_ids = if request.instance_id.is_empty() {
        Vec::new()
    } else {
        crate::state::get_installed_project_ids_for_instance(
            &request.instance_id,
            None,
            &state,
        )
        .await?
    };
    let mut visited = HashSet::new();
    let mut projects = HashMap::<u32, CurseForgeProject>::new();
    projects.insert(request.project_id, primary_project.clone());
    let mut dependencies: Vec<CurseForgePreviewItem> = Vec::new();
    let mut modrinth_fallbacks = Vec::new();
    let mut fallback_parents = HashSet::new();
    let mut skipped = Vec::new();
    let mut optional = Vec::new();
    let mut incompatible = Vec::new();
    let mut pending = vec![PreviewPendingCurseForgeFile {
        project_id: request.project_id,
        file_id: request.file_id,
        item_type: primary_type,
        depth: 0,
        ancestors: HashSet::new(),
    }];

    while let Some(pending_file) = pending.pop() {
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        if !visited.insert((project_id, file_id)) {
            continue;
        }
        let file =
            if project_id == request.project_id && file_id == request.file_id {
                primary_file.clone()
            } else {
                get_file(project_id, file_id).await?
            };
        if request.install_dependencies
            && pending_file.depth < MAX_DEPENDENCY_DEPTH
        {
            for dependency_ref in &file.dependencies {
                let dependency_project_id = if request.mod_loader_type
                    == Some(CURSEFORGE_LOADER_QUILT)
                    && dependency_ref.mod_id == FABRIC_API_CURSEFORGE_PROJECT_ID
                {
                    QUILTED_FABRIC_API_CURSEFORGE_PROJECT_ID
                } else {
                    dependency_ref.mod_id
                };
                match dependency_ref.relation_type {
                    DEPENDENCY_RELATION_INCOMPATIBLE => {
                        incompatible.push(dependency_ref.mod_id)
                    }
                    DEPENDENCY_RELATION_EMBEDDED | DEPENDENCY_RELATION_TOOL => {
                        skipped.push(CurseForgeSkippedDependency {
                            project_id: dependency_ref.mod_id,
                            file_id: None,
                            reason: if dependency_ref.relation_type
                                == DEPENDENCY_RELATION_EMBEDDED
                            {
                                "embedded".to_string()
                            } else {
                                "tool".to_string()
                            },
                        })
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
                        if request
                            .excluded_dependency_project_ids
                            .contains(&dependency_project_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "excluded_by_user".to_string(),
                            });
                            continue;
                        }
                        if installed_project_ids.contains(&format!(
                            "curseforge:{dependency_project_id}"
                        )) && !request
                            .force_dependency_project_ids
                            .contains(&dependency_project_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "already_installed".to_string(),
                            });
                            continue;
                        }
                        let dependency_project = match projects
                            .get(&dependency_project_id)
                        {
                            Some(project) => project.clone(),
                            None => {
                                let project =
                                    get_project(dependency_project_id).await?;
                                projects.insert(
                                    dependency_project_id,
                                    project.clone(),
                                );
                                project
                            }
                        };
                        let Some(dependency_type) = recognized_project_type(
                            dependency_project.class_id,
                        ) else {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "unsupported_project_type".to_string(),
                            });
                            continue;
                        };
                        let Some(selected) = select_dependency_file(
                            dependency_project_id,
                            request.game_version.clone(),
                            request.mod_loader_type,
                        )
                        .await?
                        else {
                            if fallback_parents.insert((project_id, file_id)) {
                                match resolve_modrinth_fallback_plan(
                                    &file, item_type, &request, &state,
                                )
                                .await
                                {
                                    Ok(Some(fallback_plan))
                                        if !fallback_plan
                                            .dependencies
                                            .is_empty() =>
                                    {
                                        append_modrinth_fallback_plan(
                                            &mut plan,
                                            &fallback_plan,
                                            &curseforge_content_ref(
                                                project_id, file_id,
                                            )?,
                                            &state,
                                        )
                                        .await?;
                                        modrinth_fallbacks.extend(
                                            preview_modrinth_fallbacks(
                                                &fallback_plan,
                                                project_id,
                                                &state,
                                            )
                                            .await?,
                                        );
                                        continue;
                                    }
                                    Ok(Some(_) | None) => {}
                                    Err(error) => {
                                        tracing::warn!(
                                            project_id = dependency_project_id,
                                            parent_project_id = project_id,
                                            parent_file_id = file_id,
                                            "Modrinth SHA-1 dependency fallback failed: {error}"
                                        );
                                        skipped.push(
                                            CurseForgeSkippedDependency {
                                                project_id:
                                                    dependency_project_id,
                                                file_id: None,
                                                reason:
                                                    "modrinth_lookup_failed"
                                                        .to_string(),
                                            },
                                        );
                                        continue;
                                    }
                                }
                            }
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: None,
                                reason: "no_compatible_version".to_string(),
                            });
                            continue;
                        };
                        let dependency_file = selected.file;
                        let child_ref = curseforge_content_ref(
                            dependency_project_id,
                            dependency_file.id,
                        )?;
                        if pending_file.ancestors.contains(&(
                            dependency_project_id,
                            dependency_file.id,
                        )) || (dependency_project_id, dependency_file.id)
                            == (project_id, file_id)
                        {
                            skipped.push(CurseForgeSkippedDependency {
                                project_id: dependency_project_id,
                                file_id: Some(dependency_file.id),
                                reason: "dependency_cycle".to_string(),
                            });
                            continue;
                        }
                        let parent_ref =
                            curseforge_content_ref(project_id, file_id)?;
                        plan.edges.push(DependencyResolutionEdge {
							parent: parent_ref.clone(),
							child: child_ref.clone(),
							relation: if dependency_ref.relation_type
								== DEPENDENCY_RELATION_REQUIRED
							{
								crate::state::instances::ContentDependencyKind::Required
							} else {
								crate::state::instances::ContentDependencyKind::Include
							},
							evidence_provider: ContentProvider::CurseForge,
						});
                        if !plan
                            .nodes
                            .iter()
                            .any(|node| node.content == child_ref)
                        {
                            plan.nodes.push(DependencyResolutionNode {
								content: child_ref,
								parent: Some(parent_ref),
							relation: if dependency_ref.relation_type
								== DEPENDENCY_RELATION_REQUIRED
							{
								crate::state::instances::ContentDependencyKind::Required
							} else {
								crate::state::instances::ContentDependencyKind::Include
							},
								source: ContentProvider::CurseForge,
								selection_reason: selected.reason.into(),
								expected_sha1: curseforge_file_sha1(&dependency_file),
								expected_size: Some(dependency_file.file_length),
							});
                        }
                        let existing = dependencies.iter_mut().find(|item| {
                            item.project_id == dependency_project_id
                        });
                        if let Some(existing) = existing {
                            existing.required |= dependency_ref.relation_type
                                == DEPENDENCY_RELATION_REQUIRED;
                            if !existing
                                .required_by_project_ids
                                .contains(&project_id)
                            {
                                existing
                                    .required_by_project_ids
                                    .push(project_id);
                            }
                            continue;
                        }
                        dependencies.push(CurseForgePreviewItem {
                            project_id: dependency_project_id,
                            file_id: dependency_file.id,
                            title: dependency_project.name.clone(),
                            version_number: dependency_file
                                .display_name
                                .clone(),
                            file_name: dependency_file.file_name.clone(),
                            size: dependency_file.file_length,
                            required_by_project_ids: vec![project_id],
                            icon_url: dependency_project
                                .logo
                                .as_ref()
                                .map(|logo| logo.thumbnail_url.clone()),
                            version_mismatch: false,
                            selection_reason: Some(selected.reason),
                            required: dependency_ref.relation_type
                                == DEPENDENCY_RELATION_REQUIRED,
                        });
                        let mut ancestors = pending_file.ancestors.clone();
                        ancestors.insert((project_id, file_id));
                        pending.push(PreviewPendingCurseForgeFile {
                            project_id: dependency_project_id,
                            file_id: dependency_file.id,
                            item_type: dependency_type,
                            depth: pending_file.depth + 1,
                            ancestors,
                        });
                    }
                    _ => {}
                }
            }
        } else if request.install_dependencies {
            skipped.push(CurseForgeSkippedDependency {
                project_id,
                file_id: Some(file_id),
                reason: "dependency_depth_exceeded".to_string(),
            });
        }
    }

    optional.sort_unstable();
    optional.dedup();
    incompatible.sort_unstable();
    incompatible.dedup();
    skipped.sort_unstable();
    skipped.dedup();
    modrinth_fallbacks.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.version_id.cmp(&right.version_id))
    });
    modrinth_fallbacks.dedup_by(|left, right| {
        left.project_id == right.project_id
            && left.version_id == right.version_id
    });
    plan.issues.extend(skipped.iter().map(|skipped| {
        DependencyResolutionIssue {
            provider: ContentProvider::CurseForge,
            project_id: skipped.project_id.to_string(),
            parent: None,
            relation: Some(
                crate::state::instances::ContentDependencyKind::Required,
            ),
            reason: skipped.reason.clone(),
        }
    }));
    store_dependency_resolution_plan(plan.clone());

    Ok(CurseForgeInstallPreview {
        plan_id: plan.id,
        primary: CurseForgePreviewItem {
            project_id: request.project_id,
            file_id: request.file_id,
            title: primary_project.name,
            version_number: primary_file.display_name,
            file_name: primary_file.file_name,
            size: primary_file.file_length,
            required_by_project_ids: Vec::new(),
            icon_url: primary_project
                .logo
                .as_ref()
                .map(|logo| logo.thumbnail_url.clone()),
            version_mismatch: false,
            selection_reason: None,
            required: true,
        },
        dependencies,
        modrinth_fallbacks,
        skipped,
        optional_dependencies: optional,
        incompatible_dependencies: incompatible,
    })
}
