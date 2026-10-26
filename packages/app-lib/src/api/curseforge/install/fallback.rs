//! CurseForge install fallback.

use super::*;
use crate::api::curseforge::catalog::{get_file, get_project};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::{
    CurseForgeCrossSourceDependency, CurseForgeFailedDownload,
    CurseForgeInstallRequest, CurseForgeInstallResult, CurseForgeInstalledFile,
};
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::selection::{
    content_target_folder, curseforge_file_sha1, manual_download_from_file,
    persist_manual_download,
};
use crate::api::curseforge::validation::validate_file_name;
use crate::api::curseforge::verify::{
    DownloadInstalledFileRequest, download_installed_file,
};
use crate::state::{
    CacheBehaviour, CachedEntry, ContentProviderRef, DependencyResolutionNode,
    DependencyResolutionPlan, ProjectType,
};
use crate::{ErrorKind, State};
use std::collections::HashSet;

pub(crate) async fn install_fixed_curseforge_content(
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

pub(crate) async fn install_fixed_modrinth_content(
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

pub(crate) async fn install_modrinth_fallbacks(
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
