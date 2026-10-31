use agent_window::{AgentWindow, Appearance};
use serde::Deserialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

#[tauri::command]
async fn open_agent_window(
    app: AppHandle,
    window: WebviewWindow,
    agent: State<'_, AgentWindow>,
    appearance: Option<Appearance>,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the launcher window can open Agent Window".into());
    }
    agent
        .open_with_appearance(&app, appearance)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum WindowAction {
    Minimize,
    ToggleMaximize,
    Close,
    StartDragging,
}

fn require_agent_window(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == agent_window::WINDOW_LABEL {
        Ok(())
    } else {
        Err("Agent window controls only operate on their own window".into())
    }
}

#[tauri::command]
async fn agent_window_action(
    window: WebviewWindow,
    action: WindowAction,
) -> Result<(), String> {
    require_agent_window(&window)?;
    let result = match action {
        WindowAction::Minimize => window.minimize(),
        WindowAction::ToggleMaximize => {
            if window.is_maximized().map_err(|error| error.to_string())? {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        WindowAction::Close => window.close(),
        WindowAction::StartDragging => window.start_dragging(),
    };
    result.map_err(|error| error.to_string())
}

#[tauri::command]
async fn agent_window_is_maximized(
    window: WebviewWindow,
) -> Result<bool, String> {
    require_agent_window(&window)?;
    window.is_maximized().map_err(|error| error.to_string())
}

#[tauri::command]
async fn agent_window_get_appearance(
    window: WebviewWindow,
    agent: State<'_, AgentWindow>,
) -> Result<Appearance, String> {
    require_agent_window(&window)?;
    Ok(agent.appearance().await)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("agent-window")
        .invoke_handler(tauri::generate_handler![
            open_agent_window,
            agent_window_action,
            agent_window_is_maximized,
            agent_window_get_appearance
        ])
        .register_uri_scheme_protocol(
            agent_window::URI_SCHEME,
            |context, request| {
                agent_window::asset_response(context.webview_label(), &request)
            },
        )
        .setup(|app, _| {
            app.manage(AgentWindow::default());
            Ok(())
        })
        .build()
}

pub fn restrict_launcher_commands<F>(
    handler: F,
) -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
where
    F: Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        // App-level commands without an app ACL manifest bypass capabilities.
        // The independent blank window must not initialize or control the app.
        if invoke.message.webview_ref().label() == agent_window::WINDOW_LABEL {
            invoke
                .resolver
                .reject("Agent Window has no launcher command access");
            return true;
        }
        handler(invoke)
    }
}
