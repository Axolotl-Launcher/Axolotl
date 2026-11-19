use super::{
    config::{Preferences, save_preferences},
    service::{
        EVENT, ObservabilityMcpService, ObservabilityMcpState, initialize,
    },
};
use tauri::{Emitter, Manager};

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn observability_mcp_get_state(
    app: tauri::AppHandle,
) -> Result<ObservabilityMcpState, String> {
    initialize(&app).await;
    Ok(app
        .state::<ObservabilityMcpService>()
        .0
        .lock()
        .await
        .state())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn observability_mcp_configure(
    app: tauri::AppHandle,
    enabled: bool,
    require_auth: bool,
) -> Result<ObservabilityMcpState, String> {
    initialize(&app).await;
    let service = app.state::<ObservabilityMcpService>();
    let mut controller = service.0.lock().await;
    let preferences = Preferences {
        enabled,
        require_auth,
    };
    save_preferences(&app, preferences)?;
    controller.stop().await;
    controller.preferences = preferences;
    controller.error = None;
    if enabled && let Err(error) = controller.start(&app).await {
        controller.error = Some(error);
    }
    let state = controller.state();
    let _ = app.emit_to("main", EVENT, &state);
    Ok(state)
}
