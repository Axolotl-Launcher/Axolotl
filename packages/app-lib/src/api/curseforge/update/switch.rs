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
    use sqlx::Row;

    let state = State::get().await?;
    let row = sqlx::query(
        "SELECT ref.provider_project_id, ref.provider_release_id, entry.project_type,
                entry.ownership_kind,
                content_set.game_version, content_set.loader
         FROM instance_files file
         INNER JOIN instance_content_entries entry ON entry.file_id = file.id
         INNER JOIN instance_content_provider_refs ref
            ON ref.content_entry_id = entry.id AND ref.provider = 'curseforge'
         INNER JOIN instance_content_sets content_set
            ON content_set.id = entry.content_set_id
         WHERE file.instance_id = ? AND file.relative_path = ?
         ORDER BY entry.modified_at DESC
         LIMIT 1",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected file is not linked to CurseForge".to_string(),
        )
    })?;
    let project_id = row
        .try_get::<String, _>("provider_project_id")?
        .parse::<u32>()
        .map_err(|_| {
            ErrorKind::InputError(
                "Stored CurseForge project ID is invalid".to_string(),
            )
        })?;
    let current_file_id = row
        .try_get::<Option<String>, _>("provider_release_id")?
        .and_then(|value| value.parse::<u32>().ok());
    let project_type = row.try_get::<String, _>("project_type")?;
    let ownership_kind =
        crate::state::instances::ContentOwnershipKind::from_str(
            &row.try_get::<String, _>("ownership_kind")?,
        )?;
    let game_version = row.try_get::<String, _>("game_version")?;
    let loader = row.try_get::<String, _>("loader")?;
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
