use super::{
    config::{Preferences, config_path},
    source::LauncherCapture,
};
use launcher_observability_mcp::{Server, Snapshot};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

pub(super) const EVENT: &str = "observability-mcp-state-changed";
/// Fixed loopback port, distinct from the launcher tools MCP on 5219.
pub(super) const PORT: u16 = 5220;
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
pub(crate) struct ObservabilityMcpState {
    #[serde(flatten)]
    runtime: Snapshot,
    enabled: bool,
    pub(super) token: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Default)]
pub(crate) struct ObservabilityMcpService(pub(super) Mutex<Controller>);

#[derive(Default)]
pub(super) struct Controller {
    pub(super) loaded: bool,
    pub(super) preferences: Preferences,
    pub(super) server: Option<Server>,
    pub(super) token: Option<String>,
    pub(super) error: Option<String>,
}

impl Controller {
    pub(super) fn state(&self) -> ObservabilityMcpState {
        let runtime = self
            .server
            .as_ref()
            .map(Server::snapshot)
            .unwrap_or_else(|| Snapshot {
                running: false,
                require_auth: self.preferences.require_auth,
                endpoint: None,
            });
        ObservabilityMcpState {
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
        let token = if self.preferences.require_auth {
            let credential = keyring::Entry::new(
                "Axolotl Launcher Observability MCP",
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

        // Capture starts before the listener so a client that connects
        // immediately already sees data.
        theseus::observability::start_capture();
        match Server::start(
            PORT,
            token.clone().unwrap_or_default(),
            self.preferences.require_auth,
            app.package_info().version.to_string(),
            Arc::new(LauncherCapture),
        )
        .await
        {
            Ok(server) => {
                self.server = Some(server);
                self.token = token;
                Ok(())
            }
            Err(error) => {
                theseus::observability::stop_capture();
                self.token = None;
                Err(error.to_string())
            }
        }
    }

    pub(super) async fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.stop().await;
        }
        theseus::observability::stop_capture();
        self.token = None;
    }
}

fn next_generation() -> u64 {
    (GENERATION.fetch_add(1, Ordering::SeqCst) + 1) << 32
}

pub async fn initialize(app: &tauri::AppHandle) {
    let service = app.state::<ObservabilityMcpService>();
    let mut controller = service.0.lock().await;
    if controller.loaded {
        return;
    }
    controller.loaded = true;
    let _ = next_generation();
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
    let service = app.state::<ObservabilityMcpService>();
    let mut controller = service.0.lock().await;
    controller.stop().await;
}
