pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("telemetry")
        .invoke_handler(tauri::generate_handler![notify_online,])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn notify_online() {
    theseus::telemetry::notify_online();
}
