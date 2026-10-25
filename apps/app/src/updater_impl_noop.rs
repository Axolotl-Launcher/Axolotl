use crate::api::Result;

#[derive(Default)]
pub struct PendingUpdateData(());

#[tauri::command]
pub fn check_app_update() -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn get_update_size() -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn enqueue_update_for_installation() -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn remove_enqueued_update() {}

#[tauri::command]
pub fn is_apt_linux() -> bool {
    false
}
