mod commands;
mod config;
use super::tools::interface;
mod service;

use service::McpService;
pub use service::{initialize, shutdown};
use tauri::Manager;

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("mcp")
        .invoke_handler(tauri::generate_handler![
            commands::mcp_get_state,
            commands::mcp_configure,
            commands::mcp_respond,
            commands::mcp_disconnect,
            interface::mcp_ui_claim,
            interface::mcp_ui_respond
        ])
        .setup(|app, _| {
            app.manage(McpService::default());
            super::tools::initialize(app);
            Ok(())
        })
        .build()
}
