mod commands;
mod config;
mod service;
mod source;

use service::ObservabilityMcpService;
pub use service::{initialize, shutdown};
use tauri::Manager;

pub fn init() -> tauri::plugin::TauriPlugin<crate::AppRuntime> {
    tauri::plugin::Builder::new("observability-mcp")
        .invoke_handler(tauri::generate_handler![
            commands::observability_mcp_get_state,
            commands::observability_mcp_configure
        ])
        .setup(|app, _| {
            app.manage(ObservabilityMcpService::default());
            Ok(())
        })
        .build()
}
