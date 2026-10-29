use super::config::{Preferences, config_path};
use launcher_mcp::{LauncherInfo, Server, Snapshot};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
pub(super) const EVENT: &str = "mcp-state-changed";
static GENERATION: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Serialize)]
pub(crate) struct McpState {
    #[serde(flatten)]
    runtime: Snapshot,
    enabled: bool,
    pub(super) token: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Default)]
pub(crate) struct McpService(pub(super) Mutex<Controller>);

#[derive(Default)]
pub(super) struct Controller {
    pub(super) revision_base: u64,
    pub(super) loaded: bool,
    pub(super) preferences: Preferences,
    pub(super) server: Option<Server>,
    pub(super) token: Option<String>,
    pub(super) error: Option<String>,
}

impl Controller {
    pub(super) fn state(&self) -> McpState {
        let mut runtime = self
            .server
            .as_ref()
            .map(Server::snapshot)
            .unwrap_or_default();
        runtime.full_access = self.preferences.full_access;
        runtime.require_auth = self.preferences.require_auth;
        runtime.revision += self.revision_base;
        McpState {
            runtime,
            enabled: self.preferences.enabled,
            token: self.token.clone(),
            error: self.error.clone(),
        }
    }

    pub(super) async fn start(
        &mut self,
        app: &tauri::AppHandle,
    ) -> Result<(), String> {
        self.revision_base = next_generation();
        let revision_base = self.revision_base;
        let token = if self.preferences.require_auth {
            let credential = keyring::Entry::new(
                "Axolotl Launcher MCP",
                &app.config().identifier,
            )
            .map_err(|error| error.to_string())?;
            let token = match credential.get_password() {
                Ok(token) => token,
                Err(keyring::Error::NoEntry) => {
                    let token = format!(
                        "{}{}",
                        uuid::Uuid::new_v4().simple(),
                        uuid::Uuid::new_v4().simple()
                    );
                    credential
                        .set_password(&token)
                        .map_err(|error| error.to_string())?;
                    token
                }
                Err(error) => return Err(error.to_string()),
            };
            Some(token)
        } else {
            None
        };
        let app_handle = app.clone();
        self.server =
            Some(
                Server::start(
                    5219,
                    token.clone().unwrap_or_default(),
                    self.preferences.full_access,
                    self.preferences.require_auth,
                    LauncherInfo::new(env!("CARGO_PKG_VERSION")),
                    move |mut runtime| {
                        runtime.revision += revision_base;
                        if runtime.connection.as_ref().is_some_and(
                            |connection| connection.awaiting_approval,
                        ) {
                            let _ = app_handle
                            .state::<crate::lightweight_mode::LightweightMode>()
                            .exit(&app_handle);
                        }
                        let _ = app_handle.emit_to(
                            "main",
                            EVENT,
                            McpState {
                                enabled: runtime.running,
                                runtime,
                                token: None,
                                error: None,
                            },
                        );
                    },
                )
                .await
                .map_err(|error| error.to_string())?,
            );
        self.token = token;
        self.error = None;
        Ok(())
    }
}

// Keep state reads and asynchronous events ordered across service restarts.
pub(super) fn next_generation() -> u64 {
    (GENERATION.fetch_add(1, Ordering::SeqCst) + 1) << 32
}

pub async fn initialize(app: &tauri::AppHandle) {
    let service = app.state::<McpService>();
    let mut controller = service.0.lock().await;
    if controller.loaded {
        return;
    }
    controller.loaded = true;
    let result = (|| match std::fs::read(config_path(app)?) {
        Ok(contents) => {
            serde_json::from_slice(&contents).map_err(|error| error.to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Preferences::default())
        }
        Err(error) => Err(error.to_string()),
    })();
    match result {
        Ok(preferences) => controller.preferences = preferences,
        Err(error) => controller.error = Some(error),
    }
    if controller.preferences.enabled
        && let Err(error) = controller.start(app).await
    {
        controller.error = Some(error);
    }
    let _ = app.emit_to("main", EVENT, controller.state());
}

pub async fn shutdown(app: &tauri::AppHandle) {
    let service = app.state::<McpService>();
    let mut controller = service.0.lock().await;
    if let Some(server) = controller.server.take() {
        server.stop().await;
    }
}
