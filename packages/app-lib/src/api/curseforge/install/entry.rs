//! CurseForge install entry.

use super::*;
use crate::api::curseforge::catalog::{get_file, get_project};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::{
    CurseForgeFile, CurseForgeInstallRequest, CurseForgeInstallResult,
    CurseForgeInstalledFile, CurseForgeProject, CurseForgeWorldInstallRequest,
    CurseForgeWorldInstallResult, default_true,
};
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::plans::DEPENDENCY_RESOLUTION_PLANS;
use crate::api::curseforge::selection::{
    content_target_folder, curseforge_content_validation, managed_project_type,
    manual_download_from_file, persist_manual_download,
};
use crate::api::curseforge::validation::{
    validate_file_name, validate_world_archive_name,
};
use crate::api::curseforge::verify::{
    DownloadInstalledFileRequest, download_curseforge_path,
    download_installed_file,
};
use crate::install::InstallProgressReporter;
use crate::state::{InstanceInstallStage, ProjectType};
use crate::{ErrorKind, State};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

#[derive(Clone)]
pub(crate) struct PendingCurseForgeFile {
    pub(crate) project_id: u32,
    pub(crate) file_id: u32,
    pub(crate) item_type: ProjectType,
    pub(crate) dependency: bool,
    pub(crate) parent_project_id: Option<u32>,
    pub(crate) parent_file_id: Option<u32>,
    pub(crate) dependency_kind:
        Option<crate::state::instances::ContentDependencyKind>,
    pub(crate) depth: usize,
    /// Set when the caller already fetched this file, so the walk does not
    /// read it again.
    pub(crate) file: Option<CurseForgeFile>,
}

#[derive(Clone)]
pub(crate) struct PreviewPendingCurseForgeFile {
    pub(crate) project_id: u32,
    pub(crate) file_id: u32,
    pub(crate) item_type: ProjectType,
    pub(crate) depth: usize,
    pub(crate) ancestors: HashSet<(u32, u32)>,
    /// Set when the caller already fetched this file, so the walk does not
    /// read it again.
    pub(crate) file: Option<CurseForgeFile>,
}

#[derive(Clone)]
pub(crate) struct CurseForgeDependencyEdgeCandidate {
    pub(crate) parent_project_id: u32,
    pub(crate) parent_file_id: u32,
    pub(crate) child_project_id: u32,
    pub(crate) child_file_id: u32,
    pub(crate) dependency_kind: crate::state::instances::ContentDependencyKind,
}

#[derive(Clone)]
pub(crate) struct ModrinthFallbackPlan {
    pub(crate) parent_project_id: u32,
    pub(crate) parent_file_id: u32,
    pub(crate) plan: content_management::ResolveContentPlan,
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

pub(crate) async fn install_file_with_metrics(
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
        file: None,
    }];

    while let Some(mut pending_file) = pending.pop() {
        if !visited.insert((pending_file.project_id, pending_file.file_id)) {
            continue;
        }
        let project_id = pending_file.project_id;
        let file_id = pending_file.file_id;
        let item_type = pending_file.item_type;
        let dependency = pending_file.dependency;

        let file = match pending_file.file.take() {
            Some(file) => file,
            None => get_file(project_id, file_id).await?,
        };
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
