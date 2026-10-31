mod backend;
pub(crate) mod interface;

use launcher_tools::{LauncherInfo, ToolRuntime};
use std::sync::Arc;
use tauri::Manager;

/// App-owned runtime survives external MCP listener restarts and is shared by future internal callers.
pub(crate) struct LauncherTools(pub Arc<ToolRuntime>);

pub(crate) fn initialize(app: &tauri::AppHandle) {
    app.manage(LauncherTools(Arc::new(ToolRuntime::new(
        LauncherInfo::new(env!("CARGO_PKG_VERSION")),
        Arc::new(backend::LauncherBackend::new(app.clone())),
    ))));
    app.manage(interface::UiBridge::default());
}
