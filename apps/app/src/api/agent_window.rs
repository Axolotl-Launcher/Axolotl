use agent_window::{AgentWindow, Appearance, Permissions};
use serde::{Deserialize, Serialize};
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

#[tauri::command]
async fn update_agent_window_appearance(
    app: AppHandle,
    window: WebviewWindow,
    agent: State<'_, AgentWindow>,
    appearance: Appearance,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the launcher window can update Agent Window".into());
    }
    agent
        .update_appearance(&app, appearance)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum WindowAction {
    Show,
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
    agent: State<'_, AgentWindow>,
    action: WindowAction,
) -> Result<(), String> {
    require_agent_window(&window)?;
    let result = match action {
        WindowAction::Show => window.show().and_then(|()| window.set_focus()),
        WindowAction::Minimize => window.minimize(),
        WindowAction::ToggleMaximize => {
            if window.is_maximized().map_err(|error| error.to_string())? {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        WindowAction::Close => agent.close(&window).await,
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

#[tauri::command]
async fn agent_window_get_permissions(
    window: WebviewWindow,
    agent: State<'_, AgentWindow>,
) -> Result<Permissions, String> {
    require_agent_window(&window)?;
    Ok(agent.permissions().await)
}

#[tauri::command]
async fn agent_window_set_permissions(
    window: WebviewWindow,
    agent: State<'_, AgentWindow>,
    permissions: Permissions,
) -> Result<Permissions, String> {
    require_agent_window(&window)?;
    Ok(agent.set_permissions(permissions).await)
}

#[derive(Serialize)]
struct AgentModel {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct AgentProvider {
    provider_id: String,
    name: String,
    models: Vec<AgentModel>,
}

#[derive(Serialize)]
struct AgentInstance {
    id: String,
    name: String,
    game_version: String,
    loader: String,
}

#[derive(Serialize)]
struct AgentContext {
    ai_enabled: bool,
    providers: Vec<AgentProvider>,
    instances: Vec<AgentInstance>,
    execution_available: bool,
}

#[tauri::command]
async fn agent_window_get_context(
    window: WebviewWindow,
) -> Result<AgentContext, String> {
    require_agent_window(&window)?;
    if theseus::State::get_if_initialized().is_none() {
        return Err("Launcher is still initializing".into());
    }
    let ai = theseus::ai::get_state().await.map_err(|e| e.to_string())?;
    let catalog = theseus::ai::catalog();
    let providers = ai
        .providers
        .into_iter()
        .filter(|provider| ai.settings.enabled && provider.enabled)
        .filter_map(|provider| {
            let models: Vec<_> = provider
                .models
                .into_iter()
                .filter(|model| model.enabled)
                .map(|model| AgentModel {
                    name: if model.name.is_empty() {
                        model.id.clone()
                    } else {
                        model.name
                    },
                    id: model.id,
                })
                .collect();
            if models.is_empty() {
                return None;
            }
            let name = provider
                .custom_name
                .filter(|name| !name.trim().is_empty())
                .or_else(|| {
                    catalog
                        .iter()
                        .find(|definition| {
                            definition.id == provider.provider_id
                        })
                        .map(|definition| definition.name.to_string())
                })
                .unwrap_or_else(|| provider.provider_id.clone());
            Some(AgentProvider {
                provider_id: provider.provider_id,
                name,
                models,
            })
        })
        .collect();
    let mut instances: Vec<_> = theseus::instance::list()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|metadata| AgentInstance {
            id: metadata.instance.id,
            name: metadata.instance.name,
            game_version: metadata.applied_content_set.game_version,
            loader: metadata.applied_content_set.loader.as_str().to_string(),
        })
        .collect();
    instances.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(AgentContext {
        ai_enabled: ai.settings.enabled,
        providers,
        instances,
        execution_available: false,
    })
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("agent-window")
        .invoke_handler(tauri::generate_handler![
            open_agent_window,
            update_agent_window_appearance,
            agent_window_action,
            agent_window_is_maximized,
            agent_window_get_appearance,
            agent_window_get_permissions,
            agent_window_set_permissions,
            agent_window_get_context
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
