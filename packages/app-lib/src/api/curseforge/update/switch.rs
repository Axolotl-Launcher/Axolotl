//! CurseForge installed file version switching.

//! CurseForge modpack update switch.

use crate::api::curseforge::content_change::install_selected_file;
use crate::api::curseforge::dto::CurseForgeInstallResult;
use crate::install::InstallProgressReporter;
use crate::{ErrorKind, State};

pub async fn switch_installed_file_version(
    instance_id: &str,
    relative_path: &str,
    file_id: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeInstallResult> {
    let state = State::get().await?;
    let row = crate::state::instances::adapters::sqlite::content_entries::get_curseforge_linked_file(
        instance_id,
        relative_path,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to CurseForge".to_string(),
        )
    })?;
    let project_id = row.provider_project_id.parse::<u32>().map_err(|_| {
        ErrorKind::InputError(
            "Stored CurseForge project ID is invalid".to_string(),
        )
    })?;
    let current_file_id = row
        .provider_release_id
        .and_then(|value| value.parse::<u32>().ok());
    let project_type = row.project_type;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.ownership_kind,
        )?;
    let game_version = row.game_version;
    let loader = row.loader;
    let mod_loader_type = match loader.as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    };

    if current_file_id == Some(file_id) {
        return Ok(CurseForgeInstallResult::default());
    }

    install_selected_file(
        instance_id,
        relative_path,
        project_id,
        current_file_id,
        file_id,
        project_type,
        ownership_kind,
        game_version,
        mod_loader_type,
        reporter,
    )
    .await
}
