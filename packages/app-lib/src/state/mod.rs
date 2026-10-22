//! Theseus state management system
use crate::util::download::constants::MAX_CONCURRENT_INSTALL_JOBS;
use crate::util::download::constants::{
    AUTO_DOWNLOAD_CONCURRENCY_INITIAL, AUTO_DOWNLOAD_CONCURRENCY_MAX,
    AUTO_DOWNLOAD_CONCURRENCY_MIN, AUTO_DOWNLOAD_CONCURRENCY_STEP,
    AUTO_DOWNLOAD_EXPANSION_ERROR_RATE, AUTO_DOWNLOAD_PRESSURE_ERROR_RATE,
    AUTO_DOWNLOAD_PRESSURE_WINDOWS, AUTO_DOWNLOAD_PROBE_COOLDOWN,
    AUTO_DOWNLOAD_SAMPLE_INTERVAL, AUTO_DOWNLOAD_TRIAL_WINDOWS,
    AUTO_DOWNLOAD_UTILIZATION_WINDOWS, DOWNLOAD_CONCURRENCY_HARD_MAX,
};
use crate::util::download::constants::{
    AUTO_DOWNLOAD_MIN_GAIN_PERCENT, AUTO_DOWNLOAD_PERCENT_BASE,
};
use crate::util::fetch::{FetchSemaphore, IoSemaphore};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering,
};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::{OnceCell, Semaphore, watch};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::state::instances::watcher::FileWatcher;
use sqlx::SqlitePool;

// Submodules
mod dirs;
pub use self::dirs::*;

mod instance_types;
pub use self::instance_types::*;

pub(crate) mod instances;
pub use self::instances::*;

mod settings;
pub use self::settings::*;

mod game_options;
pub(crate) use self::game_options::*;

mod proxy_settings;

pub(crate) mod curseforge_settings;

mod installer_settings;

mod process;
pub use self::process::*;

pub(crate) async fn instance_has_running_process(
    instance_id: &str,
    state: &State,
) -> crate::Result<bool> {
    Ok(state.process_manager.has_instance_process(instance_id))
}

mod java_globals;
pub use self::java_globals::*;

mod discovered_javas;
pub use self::discovered_javas::*;

mod discord;
pub use self::discord::*;

mod minecraft_auth;
pub use self::minecraft_auth::*;

pub mod minecraft_skins;

mod cache;
pub use self::cache::*;

pub mod content_favorites;
pub mod resources;
pub use self::content_favorites::*;

mod friends;
pub use self::friends::*;

mod tunnel;
pub use self::tunnel::*;

pub mod db;
pub(crate) mod db_backup;
mod mr_auth;

pub use self::mr_auth::*;

mod legacy_converter;

pub mod attached_world_data;
pub(crate) mod content_store;
pub mod instance_groups;
pub mod server_join_log;

// Global state
// RwLock on state only has concurrent reads, except for config dir change which takes control of the State
static LAUNCHER_STATE: OnceCell<Arc<State>> = OnceCell::const_new();
/// Pauses maintenance loops that only serve the launcher UI while the main
/// window is destroyed in lightweight mode.
pub(crate) struct MaintenanceGate {
    paused: watch::Sender<bool>,
}

impl MaintenanceGate {
    fn new() -> Self {
        let (paused, _) = watch::channel(false);
        Self { paused }
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        let _ = self.paused.send_replace(paused);
    }

    pub(crate) fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Waits while the gate is paused. Returns whether this call observed a
    /// pause, so callers can catch up on work skipped in the meantime.
    pub(crate) async fn wait_until_resumed(&self) -> bool {
        let mut receiver = self.paused.subscribe();
        let mut was_paused = false;
        while *receiver.borrow_and_update() {
            was_paused = true;
            if receiver.changed().await.is_err() {
                break;
            }
        }
        was_paused
    }
}

/// The CurseForge API key set in the launcher, if any. Returns `None` while the
/// launcher state is still initializing, which lets synchronous callers fall
/// back to the key from the environment or the build without waiting.
pub(crate) fn curseforge_api_key_override_if_ready() -> Option<String> {
    State::get_if_initialized()
        .and_then(|state| state.curseforge_api_key_override())
}

pub struct State {
    /// Information on the location of files used in the launcher
    pub directories: DirectoryInfo,

    /// Semaphore used to limit concurrent network requests and avoid errors
    pub fetch_semaphore: FetchSemaphore,
    /// Global capacity for file transfers. Metadata and API requests use their
    /// own semaphores so they cannot delay an active installation.
    pub download_semaphore: FetchSemaphore,
    /// Semaphore used to limit concurrent I/O and avoid errors
    pub io_semaphore: IoSemaphore,
    /// Semaphore to limit concurrent API requests. This is separate from the fetch semaphore
    /// to keep API functionality while the app is performing intensive tasks.
    pub api_semaphore: FetchSemaphore,
    minecraft_metadata_source: AtomicU8,
    minecraft_file_source: AtomicU8,
    modrinth_source: AtomicU8,
    curseforge_source: AtomicU8,
    bypass_curseforge_download_restrictions: AtomicBool,
    mojang_auth_use_mirror: AtomicBool,
    auto_prefers_mirror: AtomicBool,
    download_concurrency_target: AtomicUsize,
    download_concurrency_limit: AtomicUsize,
    fetch_concurrency_limit: AtomicUsize,
    api_concurrency_limit: AtomicUsize,
    auto_concurrent_downloads: AtomicBool,
    auto_concurrency_ceiling: AtomicUsize,
    download_active_connections: AtomicUsize,
    download_sample_bytes: AtomicU64,
    download_sample_requests: AtomicU64,
    download_sample_errors: AtomicU64,
    download_sample_throttles: AtomicU64,
    pub(crate) install_job_semaphore: Semaphore,
    pub(crate) install_db_semaphore: Semaphore,
    pub(crate) install_job_cancellations: DashMap<Uuid, CancellationToken>,
    pub(crate) install_job_operation_locks:
        DashMap<Uuid, Arc<AsyncMutex<InstallJobOperationState>>>,

    /// Discord RPC
    pub discord_rpc: DiscordGuard,

    /// Process manager
    pub process_manager: ProcessManager,

    /// Gate pausing maintenance loops that only serve the launcher UI.
    pub(crate) maintenance_gate: MaintenanceGate,
    /// Handles for the long-running maintenance loops, aborted on shutdown.
    pub(crate) background_tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,

    // NOTE: we explicitly must NOT store the app identifier in the state object,
    // because creating the state object is fallible (e.g. database missing),
    // but we rely on the app identifier to create the state (data dir).
    //
    // /// App identifier string (like com.modrinth.AxolotlLauncher)
    // pub app_identifier: String,
    /// Friends socket
    pub friends_socket: FriendsSocket,

    pub restart_after_pending_update: AtomicBool,

    /// Per-instance locks serializing content writes against instance
    /// deletion, so a delete can never commit between a command loading an
    /// instance and writing rows that reference it.
    pub(crate) instance_locks: Arc<InstanceLockManager>,

    pub(crate) pool: SqlitePool,
    resource_service: OnceCell<Arc<crate::resources::ResourceService>>,

    /// CurseForge API key set in the launcher. Takes precedence over the key
    /// from the environment or the build, which stays the fallback until the
    /// user sets one here.
    curseforge_api_key: RwLock<Option<String>>,
    curseforge_api_key_update: AsyncMutex<()>,

    // Cloning reqwest::Client retains its underlying connection pool.
    configured_http_client: RwLock<crate::util::fetch::DownloadClients>,
    configured_http_client_update: AsyncMutex<()>,

    pub(crate) file_watcher: FileWatcher,
	pub(crate) screenshot_locks: DashMap<String, Arc<AsyncMutex<()>>>,
	pub(crate) synced_options_lock: Arc<AsyncMutex<()>>,
	pub(crate) game_locale_indexer: crate::api::instance::synced_options::game_options::locales::GameLocaleIndexer,
}

impl State {
    pub async fn maintain_database_backups(&self) -> crate::Result<()> {
        db_backup::maintain_app_db_backups(
            self.pool.connect_options().get_filename(),
        )
        .await
    }

    /// Stops further database writes before recording a clean-shutdown fingerprint.
    pub async fn finish_database_session(&self) -> crate::Result<()> {
        let database = self.pool.connect_options().get_filename().to_path_buf();
        tokio::time::timeout(Duration::from_secs(3), self.pool.close()).await.map_err(|_| {
            crate::ErrorKind::OtherError("Database connections did not close before shutdown; next startup will run a full integrity check".into())
        })?;
        db_backup::mark_clean_shutdown(&database).await
    }

    pub async fn resources(
        &self,
    ) -> crate::resources::ResourceResult<&Arc<crate::resources::ResourceService>>
    {
        self.resource_service
            .get_or_try_init(|| async {
                let path: String = sqlx::query_scalar(
                    "SELECT file FROM pragma_database_list WHERE name = 'main'",
                )
                .fetch_one(&self.pool)
                .await
                .map_err(|error| {
                    crate::resources::ResourceError::Cache(error.to_string())
                })?;
                let channel = std::path::Path::new(&path)
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .filter(|name| matches!(*name, "release" | "beta"))
                    .unwrap_or_else(|| db::default_update_channel());
                let path = self
                    .directories
                    .resource_cache_db(channel)
                    .map_err(|_| {
                        crate::resources::ResourceError::InvalidRequest(
                            "invalid cache channel".into(),
                        )
                    })?;
                let cache = Arc::new(
                    crate::resources::cache::ResourceCache::open(path).await,
                );
                if let Err(error) = cache.import_legacy(&self.pool).await {
                    tracing::warn!(error = %error, "Legacy resource cache import incomplete");
                }
                crate::resources::ResourceService::with_store(
                    cache,
                    crate::resources::providers::default_providers(),
                    resources::ResourceStore::new(self.pool.clone()),
                )
            })
            .await
    }

    pub(crate) async fn lock_instance_screenshots(
        &self,
        instance_id: &str,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        self.screenshot_locks
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
            .lock_owned()
            .await
    }

    pub(crate) async fn lock_synced_options(
        &self,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        self.synced_options_lock.clone().lock_owned().await
    }
}

#[derive(Default)]
pub(crate) struct InstallJobOperationState {
    pub(crate) cache_repair_started: bool,
}

pub(crate) struct DownloadConnectionActivity {
    state: Arc<State>,
}

impl Drop for DownloadConnectionActivity {
    fn drop(&mut self) {
        self.state
            .download_active_connections
            .fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct AutoConcurrencyProbe {
    previous_target: usize,
    baseline_throughput: u64,
    windows: usize,
    throughput: u64,
}

#[derive(Debug, Default)]
struct AutoConcurrencyController {
    high_utilization_windows: usize,
    pressure_windows: usize,
    probe: Option<AutoConcurrencyProbe>,
    cooldown_until: Option<Instant>,
}

#[derive(Clone, Copy, Debug)]
struct AutoConcurrencySample {
    active: usize,
    bytes: u64,
    requests: u64,
    errors: u64,
    throttles: u64,
}

/// Per-instance lock registry with task-local reentrancy.
///
/// Instance deletion removes the `instances` row (and cascades through content
/// files, entries, provider refs and update checks), while content commands
/// load the instance first and write those rows later. Without serialization a
/// concurrent delete can commit between the load and the write, surfacing as a
/// raw `SQLITE_CONSTRAINT_FOREIGNKEY` (code 787) error. The lock is reentrant
/// for the task that already holds it, since commands compose (update →
/// check → sync → record), while concurrent tasks are serialized per instance.
#[derive(Default)]
pub(crate) struct InstanceLockManager {
    locks: DashMap<String, Arc<AsyncMutex<()>>>,
    held_by_owner: std::sync::Mutex<HashMap<LockOwner, HashSet<String>>>,
}

impl InstanceLockManager {
    pub(crate) async fn lock(
        self: &Arc<Self>,
        instance_id: &str,
    ) -> InstanceLockGuard {
        let lock = self
            .locks
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();
        let owner = current_lock_owner();

        if self
            .held_by_owner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(&owner)
            .is_some_and(|held| held.contains(instance_id))
        {
            return InstanceLockGuard {
                manager: Arc::clone(self),
                inner: None,
                owner: Some(owner),
                instance_id: instance_id.to_string(),
            };
        }

        let inner = lock.lock_owned().await;
        self.held_by_owner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .entry(owner)
            .or_default()
            .insert(instance_id.to_string());

        InstanceLockGuard {
            manager: Arc::clone(self),
            inner: Some(inner),
            owner: Some(owner),
            instance_id: instance_id.to_string(),
        }
    }

    pub(crate) async fn lock_exclusive(
        self: &Arc<Self>,
        instance_id: &str,
    ) -> InstanceLockGuard {
        let lock = self
            .locks
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();
        let inner = lock.lock_owned().await;

        InstanceLockGuard {
            manager: Arc::clone(self),
            inner: Some(inner),
            owner: None,
            instance_id: instance_id.to_string(),
        }
    }

    pub(crate) async fn lock_exclusive_cancellable(
        self: &Arc<Self>,
        instance_id: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> crate::Result<InstanceLockGuard> {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(crate::ErrorKind::OtherError(
                format!("Install was canceled while waiting for the content lock for {instance_id}"),
            ).into()),
            guard = self.lock_exclusive(instance_id) => Ok(guard),
        }
    }
}

/// Identity of the async execution context holding an instance lock.
///
/// Task IDs distinguish concurrent tasks on a multi-threaded runtime; when no
/// task context exists (for example the main test future) the thread ID is used
/// so re-entrant calls within the same context are still recognized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum LockOwner {
    Task(tokio::task::Id),
    Thread(std::thread::ThreadId),
}

fn current_lock_owner() -> LockOwner {
    tokio::task::try_id()
        .map(LockOwner::Task)
        .unwrap_or_else(|| LockOwner::Thread(std::thread::current().id()))
}

/// RAII guard for an instance lock. Only the outermost holder releases the
/// underlying mutex and removes the owner from the reentrancy registry.
pub(crate) struct InstanceLockGuard {
    manager: Arc<InstanceLockManager>,
    inner: Option<tokio::sync::OwnedMutexGuard<()>>,
    owner: Option<LockOwner>,
    instance_id: String,
}

impl Drop for InstanceLockGuard {
    fn drop(&mut self) {
        if self.inner.is_some()
            && let Some(owner) = self.owner
        {
            let mut held = self
                .manager
                .held_by_owner
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let mut remove_owner = false;
            if let Some(held_instances) = held.get_mut(&owner) {
                held_instances.remove(&self.instance_id);
                remove_owner = held_instances.is_empty();
            }
            if remove_owner {
                held.remove(&owner);
            }
        }
    }
}

fn grow_semaphore(
    semaphore: &Semaphore,
    current_limit: &AtomicUsize,
    target: usize,
) {
    let mut current = current_limit.load(Ordering::Acquire);

    while current < target {
        match current_limit.compare_exchange(
            current,
            target,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                semaphore.add_permits(target - current);
                return;
            }
            Err(updated) => current = updated,
        }
    }
}

async fn shrink_semaphore(
    semaphore: &Semaphore,
    current_limit: &AtomicUsize,
    target: &AtomicUsize,
) {
    loop {
        if current_limit.load(Ordering::Acquire)
            <= target.load(Ordering::Acquire)
        {
            return;
        }

        let Ok(permit) = semaphore.acquire().await else {
            return;
        };

        loop {
            let current = current_limit.load(Ordering::Acquire);
            if current <= target.load(Ordering::Acquire) {
                drop(permit);
                return;
            }

            if current_limit
                .compare_exchange(
                    current,
                    current - 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                permit.forget();
                break;
            }
        }
    }
}

impl AutoConcurrencyController {
    fn discard_contaminated_sample(&mut self, current: usize) -> usize {
        self.high_utilization_windows = 0;
        self.probe
            .take()
            .map_or(current, |probe| probe.previous_target)
    }

    fn next_target(
        &mut self,
        now: Instant,
        current: usize,
        ceiling: usize,
        sample: AutoConcurrencySample,
    ) -> usize {
        let error_rate = if sample.requests == 0 {
            0.0
        } else {
            sample.errors as f64 / sample.requests as f64
        };
        // A single throttled window is handled by the per-host limiter; the
        // global concurrency only backs off when pressure persists across
        // several samples, and then in small steps instead of a big cut.
        let pressured = sample.throttles > 0
            || error_rate >= AUTO_DOWNLOAD_PRESSURE_ERROR_RATE;
        if pressured {
            self.high_utilization_windows = 0;
            self.probe = None;
            if self.cooldown_until.is_none_or(|until| until <= now) {
                self.pressure_windows += 1;
                if self.pressure_windows >= AUTO_DOWNLOAD_PRESSURE_WINDOWS {
                    self.cooldown_until =
                        Some(now + AUTO_DOWNLOAD_PROBE_COOLDOWN);
                    return current
                        .saturating_sub(AUTO_DOWNLOAD_CONCURRENCY_STEP)
                        .max(AUTO_DOWNLOAD_CONCURRENCY_MIN);
                }
            }
            return current;
        }
        self.pressure_windows = 0;

        if let Some(probe) = &mut self.probe {
            probe.windows += 1;
            probe.throughput = probe.throughput.saturating_add(sample.bytes);
            if probe.windows >= AUTO_DOWNLOAD_TRIAL_WINDOWS {
                let average = probe.throughput / probe.windows as u64;
                let productive = probe.baseline_throughput > 0
                    && average.saturating_mul(AUTO_DOWNLOAD_PERCENT_BASE)
                        >= probe
                            .baseline_throughput
                            .saturating_mul(AUTO_DOWNLOAD_MIN_GAIN_PERCENT);
                let previous = probe.previous_target;
                self.probe = None;
                if !productive {
                    self.cooldown_until =
                        Some(now + AUTO_DOWNLOAD_PROBE_COOLDOWN);
                    return previous;
                }
            }
            return current;
        }

        if self.cooldown_until.is_some_and(|until| until > now)
            || error_rate >= AUTO_DOWNLOAD_EXPANSION_ERROR_RATE
            || sample.throttles > 0
        {
            self.high_utilization_windows = 0;
            return current;
        }

        if sample.bytes > 0
            && sample.active.saturating_mul(10) >= current.saturating_mul(9)
        {
            self.high_utilization_windows += 1;
        } else {
            self.high_utilization_windows = 0;
        }
        if self.high_utilization_windows < AUTO_DOWNLOAD_UTILIZATION_WINDOWS
            || current >= ceiling
        {
            return current;
        }

        self.high_utilization_windows = 0;
        let target = (current + AUTO_DOWNLOAD_CONCURRENCY_STEP).min(ceiling);
        self.probe = Some(AutoConcurrencyProbe {
            previous_target: current,
            baseline_throughput: sample.bytes,
            windows: 0,
            throughput: 0,
        });
        target
    }
}

impl State {
    pub async fn init(app_identifier: String) -> crate::Result<()> {
        let state = LAUNCHER_STATE
            .get_or_try_init(move || Self::initialize_state(app_identifier))
            .await?;

        if let Err(e) =
            crate::install::recovery::recover_interrupted_jobs(state).await
        {
            tracing::error!("Error recovering interrupted install jobs: {e}");
        }
        if let Err(e) =
            crate::api::curseforge::reconcile_persisted_curseforge_waiting_jobs(
                state,
            )
            .await
        {
            tracing::error!(
                "Error reconciling persisted CurseForge waiting install jobs: {e}"
            );
        }

        let config_sync_task = tokio::task::spawn({
            let state = Arc::clone(state);
            async move {
                instances::config_sync::run(state).await;
            }
        });
        state.background_tasks.lock().push(config_sync_task);

        let concurrency_task = tokio::spawn({
            let state = Arc::clone(state);
            async move {
                state.run_auto_concurrency_controller().await;
            }
        });
        state.background_tasks.lock().push(concurrency_task);

        crate::telemetry::start(Arc::clone(state));

        let preload_task = tokio::spawn(async move {
            crate::google_ip::preload().await;
        });
        state.background_tasks.lock().push(preload_task);

        let maintenance_task = tokio::spawn({
            let state = Arc::clone(state);
            async move {
                crate::util::fetch::cleanup_stale_partial_downloads(vec![
                    state.directories.metadata_dir(),
                    state.directories.caches_dir(),
                ]);

                instances::watcher::watch_instances_init(
                    &state.file_watcher,
                    &state.directories,
                    &state.pool,
                )
                .await;

                let res = tokio::try_join!(
                    state.discord_rpc.clear_to_default(true),
                    instances::refresh_all_instances(),
                    Settings::migrate(&state.pool),
                    ModrinthCredentials::refresh_all(),
                );

                if let Err(e) = res {
                    tracing::error!("Error running discord RPC: {e}");
                }

                // Axolotl does not connect to Modrinth's private friends socket.
            }
        });
        state.background_tasks.lock().push(maintenance_task);

        Ok(())
    }

    #[cfg(all(test, not(feature = "tauri")))]
    pub(crate) async fn init_for_test(
        app_identifier: String,
    ) -> crate::Result<Arc<Self>> {
        LAUNCHER_STATE
            .get_or_try_init(move || Self::initialize_state(app_identifier))
            .await
            .cloned()
    }

    /// Get the current launcher state, waiting for initialization
    pub async fn get() -> crate::Result<Arc<Self>> {
        if !LAUNCHER_STATE.initialized() {
            // Frontend commands routinely race initialize_state; wait instead of
            // treating the first poll as a hard error.
            tracing::debug!(
                "State not ready yet; waiting for launcher initialization before serving command"
            );
            while !LAUNCHER_STATE.initialized() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        Ok(Arc::clone(
            LAUNCHER_STATE.get().expect("State is not initialized!"),
        ))
    }

    pub fn initialized() -> bool {
        LAUNCHER_STATE.initialized()
    }

    pub(crate) fn minecraft_metadata_source(&self) -> DownloadSourceMode {
        DownloadSourceMode::from_u8(
            self.minecraft_metadata_source.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn minecraft_file_source(&self) -> DownloadSourceMode {
        DownloadSourceMode::from_u8(
            self.minecraft_file_source.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn modrinth_source(&self) -> DownloadSourceMode {
        DownloadSourceMode::from_u8(
            self.modrinth_source.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn curseforge_source(&self) -> DownloadSourceMode {
        DownloadSourceMode::from_u8(
            self.curseforge_source.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn bypass_curseforge_download_restrictions(&self) -> bool {
        self.bypass_curseforge_download_restrictions
            .load(Ordering::Relaxed)
    }

    pub fn mojang_auth_use_mirror(&self) -> bool {
        self.mojang_auth_use_mirror.load(Ordering::Relaxed)
    }

    pub fn set_mojang_auth_use_mirror(&self, use_mirror: bool) {
        self.mojang_auth_use_mirror
            .store(use_mirror, Ordering::Relaxed);
    }

    pub async fn proxy_config(
        &self,
    ) -> crate::Result<crate::util::proxy::ProxyConfig> {
        crate::state::proxy_settings::get(&self.pool).await
    }

    pub fn curseforge_api_key_override(&self) -> Option<String> {
        self.curseforge_api_key.read().clone()
    }

    pub async fn update_curseforge_api_key(
        &self,
        api_key: Option<String>,
    ) -> crate::Result<()> {
        let _update = self.curseforge_api_key_update.lock().await;
        let api_key = curseforge_settings::normalize(api_key);
        curseforge_settings::set(&self.pool, api_key.as_deref()).await?;
        *self.curseforge_api_key.write() = api_key;
        Ok(())
    }

    pub async fn update_proxy_config(
        &self,
        config: &crate::util::proxy::ProxyConfig,
    ) -> crate::Result<()> {
        let _update = self.configured_http_client_update.lock().await;
        let settings = Settings::get(&self.pool).await?;
        let client = crate::util::fetch::DownloadClients::build(
            config,
            settings.ignore_ssl_errors,
            settings.doh_enabled,
        )?;
        crate::state::proxy_settings::set(&self.pool, config).await?;
        let mut current = self.configured_http_client.write();
        current.endpoints.close();
        *current = client;
        Ok(())
    }

    pub(crate) async fn update_http_client_for_settings(
        &self,
        settings: &Settings,
    ) -> crate::Result<()> {
        let _update = self.configured_http_client_update.lock().await;
        let proxy = crate::state::proxy_settings::get(&self.pool).await?;
        let res = {
            let current = self.configured_http_client.read();
            current.proxy == proxy
                && current.ignore_ssl_errors == settings.ignore_ssl_errors
                && current.doh_enabled == settings.doh_enabled
        };
        if res {
            return Ok(());
        }
        let client = crate::util::fetch::DownloadClients::build(
            &proxy,
            settings.ignore_ssl_errors,
            settings.doh_enabled,
        )?;
        let mut current = self.configured_http_client.write();
        current.endpoints.close();
        *current = client;
        Ok(())
    }

    pub fn network_scope(&self) -> String {
        self.download_clients().scope
    }

    pub(crate) fn configured_http_client(&self) -> reqwest::Client {
        self.download_clients().metadata
    }

    pub(crate) fn download_clients(
        &self,
    ) -> crate::util::fetch::DownloadClients {
        // Try the existing settings-update lock: a concurrent settings writer
        // owns replacement, while active requests keep their original snapshot.
        if let Ok(_update) = self.configured_http_client_update.try_lock() {
            let current = self.configured_http_client.read().clone();
            match current.refresh_system_proxy() {
                Ok(Some(rebuilt)) => {
                    *self.configured_http_client.write() = rebuilt;
                    tracing::info!(
                        "Rebuilt network clients after system proxy configuration changed"
                    );
                }
                Ok(None) => {}
                Err(_) => tracing::warn!(
                    "Could not rebuild network clients after system proxy configuration changed"
                ),
            }
        }
        self.configured_http_client.read().clone()
    }

    pub(crate) fn download_concurrency(&self) -> usize {
        self.download_concurrency_target.load(Ordering::Acquire)
    }

    pub(crate) fn begin_download_connection(
        self: &Arc<Self>,
    ) -> DownloadConnectionActivity {
        self.download_active_connections
            .fetch_add(1, Ordering::AcqRel);
        self.download_sample_requests.fetch_add(1, Ordering::AcqRel);
        DownloadConnectionActivity {
            state: Arc::clone(self),
        }
    }

    pub(crate) fn record_download_bytes(&self, bytes: u64) {
        self.download_sample_bytes
            .fetch_add(bytes, Ordering::AcqRel);
    }

    pub(crate) fn record_download_error(&self) {
        self.download_sample_errors.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn record_download_throttle(&self) {
        self.download_sample_throttles
            .fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn update_download_settings(
        self: &Arc<Self>,
        settings: &Settings,
    ) {
        self.minecraft_metadata_source
            .store(settings.minecraft_metadata_source as u8, Ordering::Relaxed);
        self.minecraft_file_source
            .store(settings.minecraft_file_source as u8, Ordering::Relaxed);
        self.modrinth_source
            .store(settings.modrinth_source as u8, Ordering::Relaxed);
        self.curseforge_source
            .store(settings.curseforge_source as u8, Ordering::Relaxed);
        self.bypass_curseforge_download_restrictions.store(
            settings.bypass_curseforge_download_restrictions,
            Ordering::Relaxed,
        );
        match settings.mojang_auth_source {
            DownloadSourceMode::MirrorPreferred => {
                self.mojang_auth_use_mirror.store(true, Ordering::Relaxed);
            }
            DownloadSourceMode::OfficialOnly => {
                self.mojang_auth_use_mirror.store(false, Ordering::Relaxed);
            }
            DownloadSourceMode::Auto
            | DownloadSourceMode::OfficialPreferred => {}
        }
        self.auto_prefers_mirror
            .store(settings.auto_prefers_mirror(), Ordering::Relaxed);
        let was_auto = self
            .auto_concurrent_downloads
            .swap(settings.auto_concurrent_downloads, Ordering::AcqRel);
        self.auto_concurrency_ceiling
            .store(AUTO_DOWNLOAD_CONCURRENCY_MAX, Ordering::Release);
        if settings.auto_concurrent_downloads {
            if !was_auto {
                self.resize_download_concurrency(
                    AUTO_DOWNLOAD_CONCURRENCY_INITIAL,
                );
            }
        } else {
            self.resize_download_concurrency(
                settings.effective_max_concurrent_downloads(),
            );
        }
    }

    async fn run_auto_concurrency_controller(self: Arc<Self>) {
        let mut interval = tokio::time::interval(AUTO_DOWNLOAD_SAMPLE_INTERVAL);
        interval
            .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        let mut controller = AutoConcurrencyController::default();
        let mut trial_lease = None;
        let mut trial_revision =
            crate::util::download::endpoint_selection::tuning::TRIALS
                .snapshot(Instant::now())
                .0;
        loop {
            interval.tick().await;
            if self.maintenance_gate.is_paused() && self.download_work_is_idle()
            {
                self.maintenance_gate.wait_until_resumed().await;
            }
            crate::util::download::local_resources::sample_cpu().await;
            crate::util::download::h2_pool::observe_adaptive_connections()
                .await;
            if !self.auto_concurrent_downloads.load(Ordering::Acquire) {
                controller = AutoConcurrencyController::default();
                trial_lease = None;
                self.download_sample_bytes.swap(0, Ordering::AcqRel);
                self.download_sample_requests.swap(0, Ordering::AcqRel);
                self.download_sample_errors.swap(0, Ordering::AcqRel);
                self.download_sample_throttles.swap(0, Ordering::AcqRel);
                continue;
            }
            let sample = AutoConcurrencySample {
                active: self
                    .download_active_connections
                    .load(Ordering::Acquire),
                bytes: self.download_sample_bytes.swap(0, Ordering::AcqRel),
                requests: self
                    .download_sample_requests
                    .swap(0, Ordering::AcqRel),
                errors: self.download_sample_errors.swap(0, Ordering::AcqRel),
                throttles: self
                    .download_sample_throttles
                    .swap(0, Ordering::AcqRel),
            };
            let current = self.download_concurrency();
            let ceiling =
                self.auto_concurrency_ceiling.load(Ordering::Acquire).clamp(
                    AUTO_DOWNLOAD_CONCURRENCY_MIN,
                    AUTO_DOWNLOAD_CONCURRENCY_MAX,
                );
            use crate::util::download::endpoint_selection::tuning::{
                TRIALS, TrialKind,
            };
            let now = Instant::now();
            let (revision, owner) = TRIALS.snapshot(now);
            if trial_lease.as_ref().is_some_and(|lease: &crate::util::download::endpoint_selection::tuning::TrialLease| !lease.valid(now))
                || owner.is_some_and(|owner| owner != TrialKind::Files)
                || (trial_lease.is_none() && revision != trial_revision)
            {
                trial_lease = None;
                let previous = controller.discard_contaminated_sample(current);
                if previous != current { self.resize_download_concurrency(previous); }
                trial_revision = TRIALS.snapshot(now).0;
                continue;
            }
            let mut target = controller.next_target(
                Instant::now(),
                current,
                ceiling,
                sample,
            );
            if target > current && trial_lease.is_none() {
                trial_lease = TRIALS.acquire(
                    TrialKind::Files,
                    now,
                    Duration::from_secs(15),
                );
                if trial_lease.is_none() {
                    controller.probe = None;
                    target = current;
                }
            }
            if controller.probe.is_none() {
                trial_lease = None;
            }
            trial_revision = TRIALS.snapshot(now).0;
            if target != current {
                tracing::info!(
                    current,
                    target,
                    active = sample.active,
                    bytes = sample.bytes,
                    requests = sample.requests,
                    errors = sample.errors,
                    throttles = sample.throttles,
                    "Adjusted automatic download concurrency"
                );
                self.resize_download_concurrency(target);
            }
        }
    }

    fn download_work_is_idle(&self) -> bool {
        self.download_active_connections.load(Ordering::Acquire) == 0
            && self.install_job_cancellations.is_empty()
    }

    fn resize_download_concurrency(self: &Arc<Self>, target: usize) {
        let target = target.clamp(1, DOWNLOAD_CONCURRENCY_HARD_MAX);
        self.download_concurrency_target
            .store(target, Ordering::Release);

        grow_semaphore(
            &self.fetch_semaphore.0,
            &self.fetch_concurrency_limit,
            target,
        );
        grow_semaphore(
            &self.download_semaphore.0,
            &self.download_concurrency_limit,
            target,
        );
        grow_semaphore(
            &self.api_semaphore.0,
            &self.api_concurrency_limit,
            target,
        );

        if self.fetch_concurrency_limit.load(Ordering::Acquire) > target {
            let state = Arc::clone(self);
            tokio::spawn(async move {
                shrink_semaphore(
                    &state.fetch_semaphore.0,
                    &state.fetch_concurrency_limit,
                    &state.download_concurrency_target,
                )
                .await;
            });
        }

        if self.download_concurrency_limit.load(Ordering::Acquire) > target {
            let state = Arc::clone(self);
            tokio::spawn(async move {
                shrink_semaphore(
                    &state.download_semaphore.0,
                    &state.download_concurrency_limit,
                    &state.download_concurrency_target,
                )
                .await;
            });
        }

        if self.api_concurrency_limit.load(Ordering::Acquire) > target {
            let state = Arc::clone(self);
            tokio::spawn(async move {
                shrink_semaphore(
                    &state.api_semaphore.0,
                    &state.api_concurrency_limit,
                    &state.download_concurrency_target,
                )
                .await;
            });
        }
    }

    /// Pauses maintenance loops that only feed the launcher UI and drops
    /// caches that are stale while the main window is destroyed.
    pub fn pause_background_services(&self) {
        self.maintenance_gate.set_paused(true);
        self.trim_idle_caches();
    }

    /// Resumes the maintenance loops paused for lightweight mode.
    pub fn resume_background_services(&self) {
        self.maintenance_gate.set_paused(false);
    }

    /// Aborts the maintenance loops and closes the Discord IPC connection;
    /// intended for the app-exit path.
    pub async fn shutdown(&self) {
        for task in self.background_tasks.lock().drain(..) {
            task.abort();
        }
        self.discord_rpc.disconnect().await;
        self.trim_idle_caches();
    }

    fn trim_idle_caches(&self) {
        let mut active_log_ids: Vec<String> = self
            .process_manager
            .get_all()
            .into_iter()
            .map(|metadata| metadata.instance_id)
            .collect();
        active_log_ids.extend(crate::api::servers::running_server_ids());
        process::remove_inactive_log_buffers(&active_log_ids);
    }

    pub fn get_if_initialized() -> Option<Arc<Self>> {
        LAUNCHER_STATE.get().map(Arc::clone)
    }

    #[tracing::instrument]
    async fn initialize_state(
        app_identifier: String,
    ) -> crate::Result<Arc<Self>> {
        tracing::info!("Connecting to app database");
        let pool = db::connect(&app_identifier).await?;

        legacy_converter::migrate_legacy_data(&pool).await?;

        tracing::info!("Fetching app settings");
        let mut settings = Settings::get(&pool).await?;
        installer_settings::apply_pending_installer_directory(
            &mut settings,
            &pool,
            &app_identifier,
        )
        .await?;
        let download_concurrency =
            settings.effective_max_concurrent_downloads();
        let fetch_semaphore =
            FetchSemaphore(Semaphore::new(download_concurrency));
        let download_semaphore =
            FetchSemaphore(Semaphore::new(download_concurrency));
        let io_semaphore =
            IoSemaphore(Semaphore::new(settings.max_concurrent_writes.max(1)));
        let api_semaphore =
            FetchSemaphore(Semaphore::new(download_concurrency));
        let auto_prefers_mirror = settings.auto_prefers_mirror();
        let curseforge_api_key = curseforge_settings::get(&pool).await?;
        let proxy_config = proxy_settings::get(&pool).await?;
        let configured_http_client =
            crate::util::fetch::DownloadClients::build(
                &proxy_config,
                settings.ignore_ssl_errors,
                settings.doh_enabled,
            )?;

        tracing::info!("Initializing directories");
        DirectoryInfo::move_launcher_directory(
            &mut settings,
            &pool,
            &io_semaphore,
            &app_identifier,
        )
        .await?;

        let directories =
            DirectoryInfo::init(settings.custom_dir, &app_identifier).await?;

        let discord_rpc = DiscordGuard::init()?;

        tracing::info!("Initializing file watcher");
        let file_watcher = instances::watcher::init_watcher().await?;

        let process_manager = ProcessManager::new();

        let friends_socket = FriendsSocket::new();

        Ok(Arc::new(Self {
            directories,
            fetch_semaphore,
            download_semaphore,
            io_semaphore,
            api_semaphore,
            minecraft_metadata_source: AtomicU8::new(
                settings.minecraft_metadata_source as u8,
            ),
            minecraft_file_source: AtomicU8::new(
                settings.minecraft_file_source as u8,
            ),
            modrinth_source: AtomicU8::new(settings.modrinth_source as u8),
            curseforge_source: AtomicU8::new(settings.curseforge_source as u8),
            bypass_curseforge_download_restrictions: AtomicBool::new(
                settings.bypass_curseforge_download_restrictions,
            ),
            mojang_auth_use_mirror: AtomicBool::new(
                match settings.mojang_auth_source {
                    DownloadSourceMode::MirrorPreferred => true,
                    DownloadSourceMode::Auto => auto_prefers_mirror,
                    DownloadSourceMode::OfficialOnly
                    | DownloadSourceMode::OfficialPreferred => false,
                },
            ),
            auto_prefers_mirror: AtomicBool::new(auto_prefers_mirror),
            download_concurrency_target: AtomicUsize::new(download_concurrency),
            download_concurrency_limit: AtomicUsize::new(download_concurrency),
            fetch_concurrency_limit: AtomicUsize::new(download_concurrency),
            api_concurrency_limit: AtomicUsize::new(download_concurrency),
            auto_concurrent_downloads: AtomicBool::new(
                settings.auto_concurrent_downloads,
            ),
            auto_concurrency_ceiling: AtomicUsize::new(
                AUTO_DOWNLOAD_CONCURRENCY_MAX,
            ),
            download_active_connections: AtomicUsize::new(0),
            download_sample_bytes: AtomicU64::new(0),
            download_sample_requests: AtomicU64::new(0),
            download_sample_errors: AtomicU64::new(0),
            download_sample_throttles: AtomicU64::new(0),
            install_job_semaphore: Semaphore::new(MAX_CONCURRENT_INSTALL_JOBS),
            install_db_semaphore: Semaphore::new(1),
            install_job_cancellations: DashMap::new(),
            install_job_operation_locks: DashMap::new(),
            discord_rpc,
            process_manager,
            maintenance_gate: MaintenanceGate::new(),
            background_tasks: Mutex::new(Vec::new()),
            friends_socket,
            restart_after_pending_update: AtomicBool::new(false),
            instance_locks: Arc::new(InstanceLockManager::default()),
            pool,
            resource_service: OnceCell::new(),
            curseforge_api_key: RwLock::new(curseforge_api_key),
            curseforge_api_key_update: AsyncMutex::new(()),
            configured_http_client: RwLock::new(configured_http_client),
            configured_http_client_update: AsyncMutex::new(()),
            file_watcher,
            screenshot_locks: DashMap::new(),
            synced_options_lock: Arc::new(AsyncMutex::new(())),
            game_locale_indexer: Default::default(),
            // app_identifier,
        }))
    }

    /// Acquire the lock serializing content writes and instance deletion for
    /// the given instance. Reentrant within the task that already holds it.
    pub(crate) async fn lock_instance_content(
        &self,
        instance_id: &str,
    ) -> InstanceLockGuard {
        self.instance_locks.lock(instance_id).await
    }

    /// Acquire the content lock without allowing a UI operation to wait forever.
    pub(crate) async fn lock_instance_content_with_timeout(
        &self,
        instance_id: &str,
        timeout: std::time::Duration,
    ) -> crate::Result<InstanceLockGuard> {
        tokio::time::timeout(timeout, self.lock_instance_content(instance_id))
            .await
            .map_err(|_| {
                crate::ErrorKind::InputError(
                    "Timed out waiting for another content operation to finish"
                        .to_string(),
                )
                .into()
            })
    }

    /// Acquire the single writer permit without allowing a content operation
    /// to wait forever behind a stale install checkpoint.
    pub(crate) async fn acquire_install_db_permit(
        &self,
    ) -> crate::Result<tokio::sync::SemaphorePermit<'_>> {
        let permit = tokio::time::timeout(
            Duration::from_secs(20),
            self.install_db_semaphore.acquire(),
        )
        .await
        .map_err(|_| {
            crate::ErrorKind::InputError(
                "Timed out waiting for the install database writer".to_string(),
            )
        })?
        .map_err(|_| {
            crate::ErrorKind::OtherError(
                "install database semaphore closed".to_string(),
            )
        })?;
        Ok(permit)
    }

    pub(crate) async fn lock_instance_content_exclusive(
        &self,
        instance_id: &str,
    ) -> InstanceLockGuard {
        self.instance_locks.lock_exclusive(instance_id).await
    }
}

#[cfg(test)]
pub(crate) async fn test_state(
    directories: DirectoryInfo,
    pool: SqlitePool,
) -> crate::Result<Arc<State>> {
    let file_watcher = instances::watcher::init_watcher().await?;
    let curseforge_api_key = curseforge_settings::get(&pool).await?;
    let proxy_config = proxy_settings::get(&pool).await?;
    let settings = Settings::get(&pool).await?;
    let configured_http_client = crate::util::fetch::DownloadClients::build(
        &proxy_config,
        settings.ignore_ssl_errors,
        settings.doh_enabled,
    )?;

    Ok(Arc::new(State {
        directories,
        fetch_semaphore: FetchSemaphore(Semaphore::new(8)),
        download_semaphore: FetchSemaphore(Semaphore::new(8)),
        io_semaphore: IoSemaphore(Semaphore::new(8)),
        api_semaphore: FetchSemaphore(Semaphore::new(8)),
        minecraft_metadata_source: AtomicU8::new(0),
        minecraft_file_source: AtomicU8::new(0),
        modrinth_source: AtomicU8::new(0),
        curseforge_source: AtomicU8::new(0),
        bypass_curseforge_download_restrictions: AtomicBool::new(true),
        mojang_auth_use_mirror: AtomicBool::new(false),
        auto_prefers_mirror: AtomicBool::new(false),
        download_concurrency_target: AtomicUsize::new(8),
        download_concurrency_limit: AtomicUsize::new(8),
        fetch_concurrency_limit: AtomicUsize::new(8),
        api_concurrency_limit: AtomicUsize::new(8),
        auto_concurrent_downloads: AtomicBool::new(false),
        auto_concurrency_ceiling: AtomicUsize::new(8),
        download_active_connections: AtomicUsize::new(0),
        download_sample_bytes: AtomicU64::new(0),
        download_sample_requests: AtomicU64::new(0),
        download_sample_errors: AtomicU64::new(0),
        download_sample_throttles: AtomicU64::new(0),
        install_job_semaphore: Semaphore::new(1),
        install_db_semaphore: Semaphore::new(1),
        install_job_cancellations: DashMap::new(),
        install_job_operation_locks: DashMap::new(),
        discord_rpc: DiscordGuard::init()?,
        process_manager: ProcessManager::new(),
        maintenance_gate: MaintenanceGate::new(),
        background_tasks: Mutex::new(Vec::new()),
        friends_socket: FriendsSocket::new(),
        restart_after_pending_update: AtomicBool::new(false),
        instance_locks: Arc::new(InstanceLockManager::default()),
        pool,
        resource_service: OnceCell::new(),
        curseforge_api_key: RwLock::new(curseforge_api_key),
        curseforge_api_key_update: AsyncMutex::new(()),
        configured_http_client: RwLock::new(configured_http_client),
        configured_http_client_update: AsyncMutex::new(()),
        file_watcher,
        screenshot_locks: DashMap::new(),
        synced_options_lock: Arc::new(AsyncMutex::new(())),
        game_locale_indexer: Default::default(),
    }))
}

#[cfg(test)]
mod maintenance_gate_tests {
    use super::*;

    #[tokio::test]
    async fn gate_resolves_immediately_when_not_paused() {
        let gate = MaintenanceGate::new();
        assert!(!gate.wait_until_resumed().await);
    }

    #[tokio::test]
    async fn gate_waits_while_paused_and_reports_the_pause() {
        let gate = MaintenanceGate::new();
        gate.set_paused(true);
        assert!(gate.is_paused());

        assert!(
            tokio::time::timeout(
                Duration::from_millis(50),
                gate.wait_until_resumed()
            )
            .await
            .is_err(),
            "the waiter must stay pending while the gate is paused"
        );

        let (was_paused, ()) = tokio::join!(gate.wait_until_resumed(), async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            gate.set_paused(false);
        });
        assert!(was_paused);
        assert!(!gate.is_paused());
    }

    #[tokio::test]
    async fn repeated_pauses_resolve_with_a_single_resume() {
        let gate = MaintenanceGate::new();
        gate.set_paused(true);
        gate.set_paused(true);

        let (was_paused, ()) = tokio::join!(gate.wait_until_resumed(), async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            gate.set_paused(false);
        });
        assert!(was_paused);
        assert!(!gate.is_paused());
    }
}

#[cfg(test)]
mod auto_concurrency_tests {
    use super::*;

    fn healthy(active: usize, bytes: u64) -> AutoConcurrencySample {
        AutoConcurrencySample {
            active,
            bytes,
            requests: 100,
            errors: 0,
            throttles: 0,
        }
    }

    #[test]
    fn contaminated_trial_restores_previous_target_and_requires_new_baseline() {
        let now = Instant::now();
        let mut controller = AutoConcurrencyController::default();
        controller.next_target(now, 64, 128, healthy(64, 100));
        assert_eq!(controller.next_target(now, 64, 128, healthy(64, 100)), 72);
        assert_eq!(controller.discard_contaminated_sample(72), 64);
        assert!(controller.probe.is_none());
        assert_eq!(controller.next_target(now, 64, 128, healthy(64, 100)), 64);
    }

    #[test]
    fn probes_up_after_two_saturated_windows_and_respects_ceiling() {
        let mut controller = AutoConcurrencyController::default();
        let now = Instant::now();
        assert_eq!(controller.next_target(now, 64, 128, healthy(64, 100)), 64);
        assert_eq!(controller.next_target(now, 64, 128, healthy(64, 100)), 72);

        let mut capped = AutoConcurrencyController::default();
        assert_eq!(capped.next_target(now, 128, 128, healthy(128, 100)), 128);
        assert_eq!(capped.next_target(now, 128, 128, healthy(128, 100)), 128);
    }

    #[test]
    fn unproductive_probe_reverts_and_throttle_reacts_to_sustained_pressure() {
        let mut controller = AutoConcurrencyController::default();
        let now = Instant::now();
        controller.next_target(now, 64, 128, healthy(64, 100));
        assert_eq!(controller.next_target(now, 64, 128, healthy(64, 100)), 72);
        assert_eq!(controller.next_target(now, 72, 128, healthy(72, 100)), 72);
        assert_eq!(controller.next_target(now, 72, 128, healthy(72, 100)), 64);

        let throttled = AutoConcurrencySample {
            throttles: 1,
            ..healthy(96, 100)
        };
        // A single throttled window is absorbed by the per-host limiter.
        let mut throttled_controller = AutoConcurrencyController::default();
        assert_eq!(
            throttled_controller.next_target(now, 96, 128, throttled),
            96
        );
        // Sustained pressure backs off by one step, not a quarter.
        assert_eq!(
            throttled_controller.next_target(now, 96, 128, throttled),
            88
        );
    }

    #[test]
    fn sustained_pressure_steps_down_across_cooldowns_and_recovers() {
        let mut controller = AutoConcurrencyController::default();
        let now = Instant::now();
        let throttled = AutoConcurrencySample {
            throttles: 1,
            ..healthy(96, 100)
        };
        assert_eq!(controller.next_target(now, 96, 128, throttled), 96);
        assert_eq!(controller.next_target(now, 96, 128, throttled), 88);
        // Backing off starts a cooldown that absorbs further pressure.
        assert_eq!(controller.next_target(now, 88, 128, throttled), 88);
        // After the cooldown, continued pressure steps down again.
        let later = now + Duration::from_secs(31);
        assert_eq!(controller.next_target(later, 88, 128, throttled), 80);
        // Clean samples reset pressure and allow growth after the cooldown.
        let later_2 = now + Duration::from_secs(62);
        assert_eq!(
            controller.next_target(later_2, 80, 128, healthy(80, 100)),
            80
        );
        assert_eq!(
            controller.next_target(later_2, 80, 128, healthy(80, 100)),
            88
        );
    }

    #[test]
    fn high_error_rate_drops_but_never_below_minimum() {
        let mut controller = AutoConcurrencyController::default();
        let sample = AutoConcurrencySample {
            active: 16,
            bytes: 0,
            requests: 100,
            errors: 5,
            throttles: 0,
        };
        assert_eq!(controller.next_target(Instant::now(), 16, 128, sample), 16);
    }
}

#[cfg(test)]
mod instance_lock_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn canceled_archive_writer_does_not_wait_for_busy_instance() {
        let manager = Arc::new(InstanceLockManager::default());
        let holder = manager.lock_exclusive("instance-1").await;
        let cancellation = tokio_util::sync::CancellationToken::new();
        let mut waiter = Box::pin(
            manager.lock_exclusive_cancellable("instance-1", &cancellation),
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut waiter)
                .await
                .is_err()
        );
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_millis(500), waiter)
            .await
            .unwrap();
        assert!(result.err().unwrap().to_string().contains("canceled"));
        drop(holder);
        tokio::time::timeout(
            Duration::from_millis(500),
            manager.lock_exclusive("instance-1"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn canceled_archive_writer_does_not_acquire_available_instance() {
        let manager = Arc::new(InstanceLockManager::default());
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        assert!(
            manager
                .lock_exclusive_cancellable("instance-1", &cancellation)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn serializes_concurrent_tasks_for_the_same_instance() {
        let manager = Arc::new(InstanceLockManager::default());
        let first = manager.lock("instance-1").await;

        let manager_for_task = Arc::clone(&manager);
        let mut contender = tokio::spawn(async move {
            let _guard = manager_for_task.lock("instance-1").await;
        });

        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut contender)
                .await
                .is_err(),
            "a second task must wait for the instance lock"
        );

        drop(first);
        tokio::time::timeout(Duration::from_millis(500), contender)
            .await
            .expect("contender acquires the lock after the holder drops")
            .unwrap();
    }

    #[tokio::test]
    async fn does_not_serialize_different_instances() {
        let manager = Arc::new(InstanceLockManager::default());
        let first = manager.lock("instance-1").await;
        let second = tokio::time::timeout(
            Duration::from_millis(200),
            manager.lock("instance-2"),
        )
        .await
        .expect("a different instance lock must be acquirable immediately");
        drop(first);
        drop(second);
    }

    #[tokio::test]
    async fn is_reentrant_within_the_same_task() {
        let manager = Arc::new(InstanceLockManager::default());
        let outer = manager.lock("instance-1").await;
        let inner = tokio::time::timeout(
            Duration::from_millis(200),
            manager.lock("instance-1"),
        )
        .await
        .expect("re-entering the same task must not deadlock");
        drop(inner);
        drop(outer);
    }

    #[tokio::test]
    async fn releases_the_lock_for_other_tasks_after_drop() {
        let manager = Arc::new(InstanceLockManager::default());
        let outer = manager.lock("instance-1").await;
        drop(outer);

        let acquired = tokio::time::timeout(
            Duration::from_millis(200),
            manager.lock("instance-1"),
        )
        .await
        .expect("the lock must be free after the guard drops");
        drop(acquired);
    }

    #[tokio::test]
    async fn detached_blocking_writer_keeps_instance_locked_until_exit() {
        let manager = Arc::new(InstanceLockManager::default());
        let (acquired_tx, acquired_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let guard = manager.lock_exclusive("instance-1").await;
        let writer = tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let _ = acquired_tx.send(());
            let _ = release_rx.recv();
            let _ = done_tx.send(());
        });

        tokio::time::timeout(Duration::from_millis(500), acquired_rx)
            .await
            .expect("blocking writer acquires the instance lock")
            .unwrap();
        drop(writer);

        let mut contender = Box::pin(manager.lock("instance-1"));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut contender)
                .await
                .is_err(),
            "cleanup in the original task must wait for the detached writer"
        );

        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_millis(500), done_rx)
            .await
            .expect("blocking writer exits after release")
            .unwrap();
        let guard = tokio::time::timeout(Duration::from_millis(500), contender)
            .await
            .expect("cleanup can acquire the lock after the writer exits");
        drop(guard);
    }
}
