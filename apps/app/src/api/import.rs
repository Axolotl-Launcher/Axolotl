use std::path::PathBuf;
use std::time::Duration;

use crate::api::Result;
use theseus::pack::import::{ImportLauncherType, ImportableInstance};

use theseus::pack::import;

const IMPORT_SCAN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const IMPORT_VALIDATION_TIMEOUT: Duration = Duration::from_secs(2 * 60);

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("import")
        .invoke_handler(tauri::generate_handler![
            get_importable_instances,
            is_valid_importable_instance,
            get_default_launcher_path,
        ])
        .build()
}

/// Gets a list of importable instances from a launcher type and base path.
/// Each entry includes the display name and the resolved filesystem path.
#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_importable_instances(
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
) -> Result<Vec<ImportableInstance>> {
    Ok(tokio::time::timeout(
        IMPORT_SCAN_TIMEOUT,
        import::get_importable_instances(launcher_type, base_path),
    )
    .await
    .map_err(|_| {
        theseus::Error::from(theseus::ErrorKind::InputError(format!(
            "Launcher instance scan timed out after {} minutes",
            IMPORT_SCAN_TIMEOUT.as_secs() / 60
        )))
    })??)
}

/// Checks if this instance is valid for importing, given a certain launcher type
/// eg: is_valid_importable_instance(PathBuf::from("C:/MultiMC/Instance 1"), ImportLauncherType::MultiMC)
#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn is_valid_importable_instance(
    instance_folder: PathBuf,
    launcher_type: ImportLauncherType,
) -> Result<bool> {
    Ok(tokio::time::timeout(
        IMPORT_VALIDATION_TIMEOUT,
        import::is_valid_importable_instance(instance_folder, launcher_type),
    )
    .await
    .map_err(|_| {
        theseus::Error::from(theseus::ErrorKind::InputError(format!(
            "Launcher instance validation timed out after {} minutes",
            IMPORT_VALIDATION_TIMEOUT.as_secs() / 60
        )))
    })?)
}

/// Returns the default path for the given launcher type
/// None if it can't be found or doesn't exist
#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_default_launcher_path(
    launcher_type: ImportLauncherType,
) -> Result<Option<PathBuf>> {
    Ok(import::get_default_launcher_path(launcher_type))
}
