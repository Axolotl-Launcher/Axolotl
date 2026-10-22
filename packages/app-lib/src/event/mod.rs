//! Theseus state management system
use ariadne::ids::UserId;
use ariadne::users::UserStatus;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
#[cfg(feature = "tauri")]
use tauri::Emitter;
use tokio::sync::OnceCell;
use uuid::Uuid;

pub mod emit;

// Global event state
// Stores the Tauri app handle and other event-related state variables
static EVENT_STATE: OnceCell<Arc<EventState>> = OnceCell::const_new();
pub struct EventState {
    /// Tauri app
    #[cfg(feature = "tauri")]
    pub app: tauri::AppHandle,
    pub loading_bars: DashMap<Uuid, LoadingBar>,
}

impl EventState {
    #[cfg(feature = "tauri")]
    pub async fn init(app: tauri::AppHandle) -> crate::Result<Arc<Self>> {
        EVENT_STATE
            .get_or_try_init(|| async {
                Ok(Arc::new(Self {
                    app,
                    loading_bars: DashMap::new(),
                }))
            })
            .await
            .cloned()
    }

    #[cfg(not(feature = "tauri"))]
    pub async fn init() -> crate::Result<Arc<Self>> {
        EVENT_STATE
            .get_or_try_init(|| async {
                Ok(Arc::new(Self {
                    loading_bars: DashMap::new(),
                }))
            })
            .await
            .cloned()
    }

    pub fn get() -> crate::Result<Arc<Self>> {
        Ok(EVENT_STATE.get().ok_or(EventError::NotInitialized)?.clone())
    }

    // Values provided should not be used directly, as they are clones and are not guaranteed to be up-to-date
    pub async fn list_progress_bars() -> crate::Result<DashMap<Uuid, LoadingBar>>
    {
        let value = Self::get()?;
        Ok(value.loading_bars.clone())
    }

    #[cfg(feature = "tauri")]
    pub async fn get_main_window() -> crate::Result<Option<tauri::WebviewWindow>>
    {
        use tauri::Manager;
        let value = Self::get()?;
        Ok(value.app.get_webview_window("main"))
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct LoadingBar {
    // loading_bar_uuid not be used directly by external functions as it may not reflect the current state of the loading bar/hashmap
    pub loading_bar_uuid: Uuid,
    pub message: String,
    pub total: f64,
    pub current: f64,
    #[serde(skip)]
    pub last_sent: f64,
    pub bar_type: LoadingBarType,
    #[cfg(feature = "cli")]
    #[serde(skip)]
    pub cli_progress_bar: indicatif::ProgressBar,
}

#[derive(Serialize, Debug, Clone)]
pub struct LoadingBarId(Uuid);

// When Loading bar id is dropped, we should remove it from the hashmap
impl Drop for LoadingBarId {
    fn drop(&mut self) {
        let loader_uuid = self.0;
        tokio::spawn(async move {
            if let Ok(event_state) = EventState::get() {
                #[cfg(any(feature = "tauri", feature = "cli"))]
                if let Some((_, bar)) =
                    event_state.loading_bars.remove(&loader_uuid)
                {
                    #[cfg(feature = "tauri")]
                    {
                        let loader_uuid = bar.loading_bar_uuid;
                        let event = bar.bar_type.clone();
                        let fraction = bar.current / bar.total;

                        let _ = event_state.app.emit(
                            "loading",
                            LoadingPayload {
                                fraction: None,
                                message: "Completed".to_string(),
                                event,
                                loader_uuid,
                            },
                        );
                        tracing::trace!(
                            "Exited at {fraction} for loading bar: {:?}",
                            loader_uuid
                        );
                    }

                    // Emit event to indicatif progress bar arc
                    #[cfg(feature = "cli")]
                    {
                        let cli_progress_bar = bar.cli_progress_bar;
                        cli_progress_bar.finish();
                    }
                }

                #[cfg(not(any(feature = "tauri", feature = "cli")))]
                event_state.loading_bars.remove(&loader_uuid);
            }
        });
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum LoadingBarType {
    LegacyDataMigration,
    DirectoryMove {
        old: PathBuf,
        new: PathBuf,
    },
    JavaDownload {
        version: u32,
    },
    PackFileDownload {
        instance_id: String,
        pack_name: String,
        icon: Option<String>,
        pack_version: String,
    },
    PackDownload {
        instance_id: String,
        pack_name: String,
        icon: Option<PathBuf>,
        pack_id: Option<String>,
        pack_version: Option<String>,
    },
    MinecraftDownload {
        instance_id: String,
        instance_name: String,
    },
    InstanceUpdate {
        instance_id: String,
        instance_name: String,
    },
    ZipExtract {
        instance_id: String,
        instance_name: String,
    },
    ConfigChange {
        new_path: PathBuf,
    },
    CopyInstance {
        import_location: PathBuf,
        instance_name: String,
    },
    LauncherUpdate {
        version: String,
        current_version: String,
    },
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct LoadingPayload {
    pub event: LoadingBarType,
    pub loader_uuid: Uuid,
    pub fraction: Option<f64>, // by convention, if optional, it means the loading is done
    pub message: String,
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct WarningPayload {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct JavaDiscoveryPayload {
    pub count: usize,
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
#[serde(rename_all = "camelCase")]
pub struct JavaDownloadConfirmationPayload {
    pub request_id: Uuid,
    pub version: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBulkUpdateProgressPayload {
    pub instance_id: String,
    pub stage: InstanceBulkUpdateProgressStage,
    pub current: usize,
    pub total: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum InstanceBulkUpdateProgressStage {
    ResolvingVersions,
    Downloading,
    Finishing,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackupProgressPayload {
    pub operation_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    pub operation_type: InstanceBackupOperationType,
    pub stage: InstanceBackupProgressStage,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_state: Option<InstanceBackupOperationFinalState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum InstanceBackupOperationType {
    Create,
    Restore,
    Delete,
    RepositoryMove,
    RestorePreview,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum InstanceBackupOperationFinalState {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum InstanceBackupProgressStage {
    Scanning,
    Hashing,
    Saving,
    Validating,
    Copying,
    Restoring,
    Deleting,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "event")]
pub enum CommandPayload {
    InstallMod {
        id: String,
    },
    InstallVersion {
        id: String,
    },
    InstallModpack {
        id: String,
    },
    InstallServer {
        id: String,
    },
    LaunchInstance {
        id: String,
        server: Option<String>,
        singleplayer_world: Option<String>,
    },
    RunMRPack {
        // run or install .mrpack
        path: PathBuf,
    },
    OpenRoute {
        path: String,
        query: Option<String>,
    },
    OpenSettings {
        tab: Option<String>,
        entry: Option<String>,
    },
    // 特权动作：仅生成载荷，执行须前端确认弹窗
    UpdateSettings {
        changes: Vec<SettingChange>,
        source: String,
    },
    StopInstance {
        instance_id: String,
        source: String,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SettingChange {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct ProcessPayload {
    pub instance_id: String,
    pub uuid: Uuid,
    pub pid: u32,
    pub maximize_window: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    #[serde(default)]
    pub launch_preparation_timeout: Option<u64>,
    pub event: ProcessPayloadType,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crashed: Option<bool>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ProcessPayloadType {
    Launched,
    Finished,
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct ServerPayload {
    #[serde(rename = "serverId")]
    pub server_id: String,
    #[serde(flatten)]
    pub event: ServerPayloadType,
}

/// Classifies why a server process exited on its own, derived from the tail
/// of its console output so the UI can react (e.g. offer the EULA dialog)
/// instead of just reporting a dead process.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    Eula,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerPayloadType {
    Log {
        line: String,
    },
    ConsoleOutput {
        data: String,
    },
    DownloadProgress {
        downloaded: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    Starting,
    Stopping,
    Started,
    Stopped {
        crashed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<ExitReason>,
    },
    #[allow(dead_code)]
    EulaRequired {
        server_id: String,
        eula_text: String,
    },
}

#[derive(Serialize, Clone)]
#[cfg(feature = "tauri")]
pub struct InstancePayload {
    pub instance_id: String,
    #[serde(flatten)]
    pub event: InstancePayloadType,
}

#[derive(Serialize, Clone)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum InstancePayloadType {
    Created,
    Synced,
    ContentChanged {
        revision: u64,
    },
    ServersUpdated,
    ScreenshotsUpdated,
    WorldUpdated {
        world: String,
    },
    ServerJoined {
        host: String,
        port: u16,
        timestamp: DateTime<Utc>,
    },
    Edited,
    ContentInstallFinished {
        project_ids: Vec<String>,
        #[serde(default)]
        dependency_project_ids: Vec<String>,
    },
    ContentInstallFailed {
        project_ids: Vec<String>,
        message: String,
    },
    Removed,
}

#[derive(Serialize, Clone)]
pub struct InstanceGroupsChangedPayload {
    pub instance_ids: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "event")]
pub enum FriendPayload {
    FriendRequest { from: UserId },
    UserOffline { id: UserId },
    StatusUpdate { user_status: UserStatus },
    StatusSync,
}

#[cfg(feature = "tauri")]
pub use self::log_types::*;

#[cfg(feature = "tauri")]
mod log_types {
    use crate::state::Log4jEvent;
    use serde::Serialize;

    #[derive(Serialize, Clone)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum LogEvent {
        Log4j(Log4jEvent),
        Legacy { message: String },
    }

    #[derive(Serialize, Clone)]
    pub struct LogPayload {
        pub instance_id: String,
        #[serde(flatten)]
        pub event: LogEvent,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Event state was not properly initialized")]
    NotInitialized,

    #[error("Non-existent loading bar of key: {0}")]
    NoLoadingBar(Uuid),

    #[cfg(feature = "tauri")]
    #[error("Tauri error: {0}")]
    TauriError(#[from] tauri::Error),
}
