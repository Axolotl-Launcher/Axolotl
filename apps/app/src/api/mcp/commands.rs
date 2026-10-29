use super::{
    config::{Preferences, save_preferences},
    service::{EVENT, McpService, McpState, initialize, next_generation},
};
use tauri::{Emitter, Manager};
#[tauri::command]
pub async fn mcp_get_state(app: tauri::AppHandle) -> Result<McpState, String> {
    initialize(&app).await;
    Ok(app.state::<McpService>().0.lock().await.state())
}

#[tauri::command]
pub async fn mcp_configure(
    app: tauri::AppHandle,
    enabled: bool,
    full_access: bool,
    require_auth: bool,
) -> Result<McpState, String> {
    initialize(&app).await;
    let service = app.state::<McpService>();
    let mut controller = service.0.lock().await;
    let preferences = Preferences {
        enabled,
        full_access,
        require_auth,
    };
    save_preferences(&app, preferences)?;
    if let Some(server) = controller.server.take() {
        server.stop().await;
    }
    controller.preferences = preferences;
    controller.revision_base = next_generation();
    controller.token = None;
    controller.error = None;
    if enabled && let Err(error) = controller.start(&app).await {
        controller.error = Some(error);
    }
    let state = controller.state();
    let _ = app.emit_to("main", EVENT, &state);
    Ok(state)
}

#[tauri::command]
pub async fn mcp_respond(
    app: tauri::AppHandle,
    connection_id: String,
    allow: bool,
) -> Result<McpState, String> {
    let service = app.state::<McpService>();
    let controller = service.0.lock().await;
    if let Some(server) = &controller.server {
        if allow {
            if !server.approve(&connection_id) {
                return Err("Connection request expired".into());
            }
        } else {
            server.deny(&connection_id);
        }
    }
    Ok(controller.state())
}

#[tauri::command]
pub async fn mcp_disconnect(
    app: tauri::AppHandle,
    connection_id: String,
) -> Result<McpState, String> {
    let service = app.state::<McpService>();
    let controller = service.0.lock().await;
    if let Some(server) = &controller.server {
        server.disconnect(&connection_id).await;
    }
    Ok(controller.state())
}
