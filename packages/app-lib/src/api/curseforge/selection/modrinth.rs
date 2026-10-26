//! Modrinth fallback planning for CurseForge dependencies.

use crate::State;
use crate::api::curseforge::dto::{CurseForgeFile, CurseForgeInstallRequest};
use crate::api::curseforge::install::{
    CurseForgeModrinthFallbackPreview, dependency_policy,
};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProvider, ContentProviderRef,
    DependencyResolutionEdge, DependencyResolutionIssue,
    DependencyResolutionNode, DependencyResolutionPlan,
    DependencySelectionReason, ModLoader, ModrinthProjectId, ModrinthVersionId,
    ProjectType,
};

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
