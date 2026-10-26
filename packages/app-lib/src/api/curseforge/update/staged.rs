//! CurseForge modpack update staged.

use crate::api::curseforge::catalog::{get_file, get_project};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::CurseForgeFile;
use crate::api::curseforge::mapping::recognized_project_type;
use crate::api::curseforge::selection::curseforge_content_validation;
use crate::api::curseforge::validation::validate_file_name;
use crate::api::curseforge::verify::{
    download_curseforge_path, record_installed_curseforge_file_with_permit,
    verify_installed_curseforge_file,
};
use crate::install::InstallProgressReporter;
use crate::state::ProjectType;
use crate::{ErrorKind, State};
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct StagedCurseForgeUpgrade {
    pub path: PathBuf,
    pub file: CurseForgeFile,
    pub project_type: ProjectType,
}

pub(crate) async fn stage_curseforge_upgrade_file(
    project_id: u32,
    file_id: u32,
    project_type: Option<ProjectType>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<StagedCurseForgeUpgrade> {
    let file = get_file(project_id, file_id).await?;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }
    validate_file_name(&file.file_name)?;
    let project = get_project(project_id).await?;
    let project_type = project_type
        .or_else(|| recognized_project_type(project.class_id))
        .ok_or_else(|| {
            ErrorKind::InputError(
                "CurseForge upgrade project has an unsupported content type"
                    .to_string(),
            )
        })?;
    let url =
        resolve_curseforge_download_url(project_id, file_id, &project, &file)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError(
            "Selected CurseForge upgrade file requires a manual download"
                .to_string(),
        )
            })?;
    let state = State::get().await?;
    let path = state
        .directories
        .caches_dir()
        .join("content")
        .join("curseforge")
        .join(project_id.to_string())
        .join(file_id.to_string())
        .join(crate::util::io::sanitize_download_file_name(
            &file.file_name,
        )?);
    let tracking = path.display().to_string();
    download_curseforge_path(
        &url,
        &file,
        &path,
        curseforge_content_validation(&file.file_name),
        None,
        reporter.map(|reporter| (reporter, tracking.as_str())),
        None,
        true,
    )
    .await?;
    verify_installed_curseforge_file(&path, &file, None, None).await?;
    Ok(StagedCurseForgeUpgrade {
        path,
        file,
        project_type,
    })
}

pub(crate) async fn apply_staged_curseforge_upgrade_file(
    instance_id: &str,
    staged: StagedCurseForgeUpgrade,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    relative_path: &str,
) -> crate::Result<String> {
    let state = State::get().await?;
    apply_staged_curseforge_upgrade_file_with_state(
        instance_id,
        staged,
        ownership_kind,
        relative_path,
        &state,
    )
    .await
}

pub(crate) async fn apply_staged_curseforge_upgrade_file_with_state(
    instance_id: &str,
    staged: StagedCurseForgeUpgrade,
    ownership_kind: crate::state::instances::ContentOwnershipKind,
    relative_path: &str,
    state: &State,
) -> crate::Result<String> {
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let full_path = crate::state::instances::commands::instance_full_path(
        state,
        &scope.instance,
    )
    .join(relative_path);
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    let previous_path =
        crate::state::materialize_project_download(&staged.path, &full_path)
            .await?;
    let record_result = record_installed_curseforge_file_with_permit(
        instance_id,
        relative_path,
        &full_path,
        &staged.file,
        staged.project_type,
        ownership_kind,
        database_permit,
        state,
    )
    .await;
    match record_result {
        Ok(()) => {
            crate::state::finalize_project_materialization(
                previous_path.as_deref(),
            )
            .await?;
        }
        Err(error) => {
            crate::state::restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(relative_path.to_string())
}
