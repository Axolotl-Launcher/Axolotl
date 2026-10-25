//! Install preview derived from a dependency plan.

use super::catalog::{get_file, get_project};
use super::constants::MAX_DEPENDENCY_DEPTH;
use super::dto::{
    CurseForgeFile, CurseForgeInstallRequest, CurseForgeProject,
    CurseForgeSkippedDependency, default_true,
};
use super::install::{
    CurseForgeModrinthFallbackPreview, PreviewPendingCurseForgeFile,
};
use super::install::{
    DependencyStep, content_dependency_kind, dependency_action, dependency_step,
};
use super::mapping::{mod_loader_to_slug, recognized_project_type};
use super::selection::{
    CurseForgeDependencySelectionReason, append_modrinth_fallback_plan,
    curseforge_content_ref, curseforge_file_sha1, instance_content_revision,
    managed_project_type, preview_modrinth_fallbacks,
    resolve_modrinth_fallback_plan, select_dependency_file,
    store_dependency_resolution_plan,
};
use crate::State;
use crate::state::{
    ContentProvider, ContentProviderRef, DependencyResolutionEdge,
    DependencyResolutionIssue, DependencyResolutionNode,
    DependencyResolutionPlan, DependencyResolutionTarget,
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

fn base_plan(
    request: &CurseForgeInstallRequest,
    instance_revision: Option<u64>,
    primary_ref: ContentProviderRef,
    primary_file: &CurseForgeFile,
) -> DependencyResolutionPlan {
    DependencyResolutionPlan {
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
        primary: primary_ref,
        primary_expected_sha1: curseforge_file_sha1(primary_file),
        primary_expected_size: Some(primary_file.file_length),
        nodes: Vec::new(),
        edges: Vec::new(),
        issues: Vec::new(),
    }
}

/// Collected dependency decisions of one preview traversal.
struct TraversalOutcome {
    dependencies: Vec<CurseForgePreviewItem>,
    modrinth_fallbacks: Vec<CurseForgeModrinthFallbackPreview>,
    skipped: Vec<CurseForgeSkippedDependency>,
    optional: Vec<u32>,
    incompatible: Vec<u32>,
}

fn assemble_preview(
    mut plan: DependencyResolutionPlan,
    request: &CurseForgeInstallRequest,
    primary_project: &CurseForgeProject,
    primary_file: &CurseForgeFile,
    outcome: TraversalOutcome,
) -> crate::Result<CurseForgeInstallPreview> {
    let TraversalOutcome {
        dependencies,
        mut modrinth_fallbacks,
        mut skipped,
        mut optional,
        mut incompatible,
    } = outcome;
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
            title: primary_project.name.clone(),
            version_number: primary_file.display_name.clone(),
            file_name: primary_file.file_name.clone(),
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
    let mut plan =
        base_plan(&request, instance_revision, primary_ref, &primary_file);
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
    let optional = Vec::new();
    let mut incompatible = Vec::new();
    let mut pending = vec![PreviewPendingCurseForgeFile {
        project_id: request.project_id,
        file_id: request.file_id,
        item_type: primary_type,
        depth: 0,
        ancestors: HashSet::new(),
        file: Some(primary_file.clone()),
    }];

    while let Some(pending_file) = pending.pop() {
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        if !visited.insert((project_id, file_id)) {
            continue;
        }
        let file = match pending_file.file {
            Some(file) => file,
            None => get_file(project_id, file_id).await?,
        };
        if request.install_dependencies
            && pending_file.depth < MAX_DEPENDENCY_DEPTH
        {
            for dependency_ref in &file.dependencies {
                let dependency_project_id = match dependency_step(
                    dependency_ref,
                    dependency_action(
                        dependency_ref,
                        &request,
                        &installed_project_ids,
                    ),
                ) {
                    DependencyStep::Skipped(skipped_dependency) => {
                        skipped.push(skipped_dependency);
                        continue;
                    }
                    DependencyStep::Incompatible(project_id) => {
                        incompatible.push(project_id);
                        continue;
                    }
                    DependencyStep::Continue => continue,
                    DependencyStep::Resolve { target_project_id } => {
                        target_project_id
                    }
                };
                {
                    let dependency_project = match projects
                        .get(&dependency_project_id)
                    {
                        Some(project) => project.clone(),
                        None => {
                            let project =
                                get_project(dependency_project_id).await?;
                            projects
                                .insert(dependency_project_id, project.clone());
                            project
                        }
                    };
                    let Some(dependency_type) =
                        recognized_project_type(dependency_project.class_id)
                    else {
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
                                    skipped.push(CurseForgeSkippedDependency {
                                        project_id: dependency_project_id,
                                        file_id: None,
                                        reason: "modrinth_lookup_failed"
                                            .to_string(),
                                    });
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
                    if pending_file
                        .ancestors
                        .contains(&(dependency_project_id, dependency_file.id))
                        || (dependency_project_id, dependency_file.id)
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
                        relation: content_dependency_kind(
                            dependency_ref.relation(),
                        ),
                        evidence_provider: ContentProvider::CurseForge,
                    });
                    if !plan.nodes.iter().any(|node| node.content == child_ref)
                    {
                        plan.nodes.push(DependencyResolutionNode {
                            content: child_ref,
                            parent: Some(parent_ref),
                            relation: content_dependency_kind(
                                dependency_ref.relation(),
                            ),
                            source: ContentProvider::CurseForge,
                            selection_reason: selected.reason.into(),
                            expected_sha1: curseforge_file_sha1(
                                &dependency_file,
                            ),
                            expected_size: Some(dependency_file.file_length),
                        });
                    }
                    let existing = dependencies
                        .iter_mut()
                        .find(|item| item.project_id == dependency_project_id);
                    if let Some(existing) = existing {
                        existing.required |=
                            dependency_ref.relation().is_required();
                        if !existing
                            .required_by_project_ids
                            .contains(&project_id)
                        {
                            existing.required_by_project_ids.push(project_id);
                        }
                        continue;
                    }
                    dependencies.push(CurseForgePreviewItem {
                        project_id: dependency_project_id,
                        file_id: dependency_file.id,
                        title: dependency_project.name.clone(),
                        version_number: dependency_file.display_name.clone(),
                        file_name: dependency_file.file_name.clone(),
                        size: dependency_file.file_length,
                        required_by_project_ids: vec![project_id],
                        icon_url: dependency_project
                            .logo
                            .as_ref()
                            .map(|logo| logo.thumbnail_url.clone()),
                        version_mismatch: false,
                        selection_reason: Some(selected.reason),
                        required: dependency_ref.relation().is_required(),
                    });
                    let mut ancestors = pending_file.ancestors.clone();
                    ancestors.insert((project_id, file_id));
                    pending.push(PreviewPendingCurseForgeFile {
                        project_id: dependency_project_id,
                        file_id: dependency_file.id,
                        item_type: dependency_type,
                        depth: pending_file.depth + 1,
                        ancestors,
                        file: Some(dependency_file),
                    });
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

    assemble_preview(
        plan,
        &request,
        &primary_project,
        &primary_file,
        TraversalOutcome {
            dependencies,
            modrinth_fallbacks,
            skipped,
            optional,
            incompatible,
        },
    )
}
