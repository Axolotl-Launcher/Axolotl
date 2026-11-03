mod backend;
pub(crate) mod interface;

use launcher_tools::{LauncherInfo, ToolRuntime};
use std::sync::Arc;
use tauri::Manager;

/// App-owned runtime survives external MCP listener restarts and is shared by future internal callers.
pub(crate) struct LauncherTools(
    pub Arc<ToolRuntime>,
    Arc<backend::LauncherBackend>,
);

impl LauncherTools {
    pub(crate) async fn describe_change(
        &self,
        name: &str,
        arguments: &serde_json::Value,
        scope: &str,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<Option<serde_json::Value>, launcher_tools::ToolError> {
        self.1.describe_change(name, arguments, scope, cancel).await
    }
}

pub(crate) fn initialize(app: &tauri::AppHandle) {
    let backend = Arc::new(backend::LauncherBackend::new(app.clone()));
    app.manage(LauncherTools(
        Arc::new(ToolRuntime::new(
            LauncherInfo::new(env!("CARGO_PKG_VERSION")),
            backend.clone(),
        )),
        backend,
    ));
    app.manage(interface::UiBridge::default());
}
