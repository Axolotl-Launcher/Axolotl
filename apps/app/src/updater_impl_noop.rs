use crate::api::Result;

#[derive(Default)]
pub struct PendingUpdateData(());

/// Builds without the updater never apply launcher updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateMethod {
    Updater,
}

impl UpdateMethod {
    pub fn detect() -> Self {
        Self::Updater
    }
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn check_app_update() -> Result<()> {
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn get_update_size() -> Result<()> {
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn enqueue_update_for_installation() -> Result<()> {
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn remove_enqueued_update() {}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn is_apt_linux() -> bool {
    false
}
