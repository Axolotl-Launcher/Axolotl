#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![recursion_limit = "256"]

use native_dialog::{DialogBuilder, MessageLevel};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::{
    env, fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
use tauri::{
    Emitter, Listener, Manager,
    http::{Response, StatusCode, header},
};
use tauri_plugin_fs::FsExt;
use theseus::prelude::*;

mod api;
mod error;
mod lightweight_mode;
mod mod_translation;
mod portable;
mod seed_map;
mod window_frame;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePreferences {
    immediate_update_fetch: bool,
    updates_paused: bool,
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(feature = "updater")]
mod updater_impl;
#[cfg(not(feature = "updater"))]
mod updater_impl_noop;

const BLOCKBENCH_SKIN_RESOURCE_DIR: &str = "resources/blockbench-skin";

fn blockbench_skin_response(
    path: &str,
    resource_dir: &Path,
) -> Response<Vec<u8>> {
    let requested_path = path.trim_start_matches('/');
    let requested_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };
    let relative_path = Path::new(requested_path);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Vec::new())
            .expect("failed to build Blockbench skin response");
    }

    let is_compressed_bundle = requested_path == "dist/skin.bundle.js";
    let file_path = resource_dir.join(if is_compressed_bundle {
        PathBuf::from("dist/skin.bundle.js.gz")
    } else {
        relative_path.to_path_buf()
    });
    let contents = match fs::read(file_path) {
        Ok(contents) => contents,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .expect("failed to build Blockbench skin response");
        }
    };
    let contents = if is_compressed_bundle {
        let mut decompressed = Vec::new();
        if flate2::read::GzDecoder::new(contents.as_slice())
            .read_to_end(&mut decompressed)
            .is_err()
        {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Vec::new())
                .expect("failed to build Blockbench skin response");
        }
        decompressed
    } else {
        contents
    };

    let content_type = match relative_path
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("ico") => "image/x-icon",
        _ => "text/html; charset=utf-8",
    };
    let response =
        Response::builder().header(header::CONTENT_TYPE, content_type);
    response
        .body(contents)
        .expect("failed to build Blockbench skin response")
}

fn is_allowed_blockbench_skin_request(
    request: &tauri::http::Request<Vec<u8>>,
) -> bool {
    if !matches!(
        request.uri().host(),
        Some("localhost" | "axolotl-skin.localhost")
    ) {
        return false;
    }

    const ALLOWED_ORIGINS: [&str; 6] = [
        "http://127.0.0.1:5201",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
        "axolotl-skin://localhost",
        "http://axolotl-skin.localhost",
    ];
    let is_allowed_source = |value: &str| {
        ALLOWED_ORIGINS.iter().any(|origin| {
            value == *origin || value.starts_with(&format!("{origin}/"))
        })
    };

    if let Some(origin) = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        if !is_allowed_source(origin) {
            return false;
        }

        if let Some(referer) = request
            .headers()
            .get(header::REFERER)
            .and_then(|value| value.to_str().ok())
        {
            return is_skin_editor_referer(referer, &is_allowed_source);
        }

        return true;
    }

    let Some(referer) = request
        .headers()
        .get(header::REFERER)
        .and_then(|value| value.to_str().ok())
    else {
        // WKWebView omits both Origin and Referer for custom-scheme iframe
        // navigations and their subresources. The handler only exposes packaged
        // editor assets, so a host-validated request remains safe to serve.
        return true;
    };

    is_skin_editor_referer(referer, &is_allowed_source)
}

fn is_skin_editor_referer(
    referer: &str,
    is_allowed_source: &impl Fn(&str) -> bool,
) -> bool {
    let Ok(url) = referer.parse::<url::Url>() else {
        return false;
    };
    if !is_allowed_source(url.origin().ascii_serialization().as_str()) {
        return false;
    }
    if url.path() == "/index.html" {
        return url
            .query_pairs()
            .any(|(key, value)| key == "embed" && value == "skin");
    }
    // Top-level iframe navigation initiated by the launcher, whose route
    // path (e.g. `/` or `/lab/skin-editor`) is never `/index.html`.
    true
}

// Should be called in launcher initialization
#[tracing::instrument(skip_all)]
#[tauri::command]
async fn initialize_state(app: tauri::AppHandle) -> api::Result<()> {
    tracing::info!("Initializing app event state...");
    theseus::EventState::init(app.clone()).await?;

    tracing::info!("Initializing app state...");
    State::init(app.config().identifier.clone()).await?;
    let anti_piracy_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = theseus::anti_piracy::check_region().await;
        if let Err(error) =
            anti_piracy_app.emit("anti-piracy-status-changed", status)
        {
            tracing::warn!(%error, "Could not notify frontend of offline account eligibility");
        }
    });
    // A recreated webview must not recover or interrupt backup operations a second time.
    static BACKUP_STARTUP: tokio::sync::OnceCell<()> =
        tokio::sync::OnceCell::const_new();
    BACKUP_STARTUP.get_or_try_init(|| async {
        theseus::instance::recover_backup_repository().await?;
        tauri::async_runtime::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if let Some(state) = State::get_if_initialized()
                && let Err(error) = state.maintain_database_backups().await
            {
                tracing::warn!(%error, "Failed to prune old database backups");
            }
            if let Err(error) = theseus::instance::maintain_backup_repository().await {
                tracing::warn!(%error, "Failed to maintain the instance backup repository");
            }
        });
        Ok::<(), theseus::Error>(())
    }).await?;

    // The logger starts before the database is available, so the stored level
    // is applied here once settings can be read. Beta keeps enough detail for
    // diagnostics by requiring DEBUG or TRACE.
    match theseus::settings::get().await {
        Ok(mut settings) => {
            match resolve_update_channel(&app).await {
                Ok(channel) => {
                    let log_level = api::settings::log_level_for_channel(
                        &channel,
                        &settings.log_level,
                    );
                    if log_level != settings.log_level {
                        settings.log_level = log_level;
                        if let Err(error) = theseus::settings::patch(
                                serde_json::json!({ "log_level": settings.log_level }),
                        )
                        .await
                        {
                            tracing::warn!(
                                "Could not save the Beta log level: {error}"
                            );
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        "Could not resolve the update channel: {error}"
                    );
                }
            }

            if std::env::var_os("RUST_LOG").is_none()
                && let Err(error) = theseus::set_log_level(&settings.log_level)
            {
                tracing::warn!("Keeping the default log level: {error}");
            }
        }
        Err(error) => {
            tracing::warn!("Could not read the stored log level: {error}");
        }
    }

    let state = State::get().await?;
    // Do not start accepting group writes until the legacy import succeeds.
    // The JSON store becomes authoritative after the first write; swallowing
    // this error would let a fallback Favorites-only store overwrite groups
    // that still exist in SQLite.
    theseus::state::instance_groups::ensure_imported(&state).await?;

    app.asset_protocol_scope()
        .allow_directory(state.directories.caches_dir(), true)?;
    app.asset_protocol_scope()
        .allow_directory(state.directories.caches_dir().join("icons"), true)?;
    app.asset_protocol_scope()
        .allow_directory(state.directories.instances_dir(), true)?;
    app.asset_protocol_scope()
        .allow_directory(state.directories.servers_dir(), true)?;
    app.fs_scope()
        .allow_directory(state.directories.instances_dir(), true)?;
    app.asset_protocol_scope()
        .allow_directory(state.directories.servers_dir(), true)?;
    app.fs_scope()
        .allow_directory(state.directories.servers_dir(), true)?;

    Ok(())
}

#[tauri::command]
async fn get_update_channel(app: tauri::AppHandle) -> api::Result<String> {
    resolve_update_channel(&app).await
}

async fn resolve_update_channel<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> api::Result<String> {
    let settings_dir = update_channel_settings_dir(app)?;
    let state = theseus::read_update_channel_state(&settings_dir).await?;
    let channel = state
        .active_channel
        .unwrap_or_else(|| theseus::default_update_channel().to_string());

    match channel.as_str() {
        "release" | "beta" => Ok(channel),
        _ => Err(theseus::Error::from(theseus::ErrorKind::FSError(
            "Update channel settings are invalid".to_string(),
        ))
        .into()),
    }
}

#[tauri::command]
async fn get_current_app_database_path(
    app: tauri::AppHandle,
) -> api::Result<String> {
    Ok(theseus::current_app_database_path(&app.config().identifier)
        .await?
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
async fn set_update_channel(
    app: tauri::AppHandle,
    channel: String,
) -> api::Result<()> {
    if !matches!(channel.as_str(), "release" | "beta") {
        return Err(theseus::Error::from(theseus::ErrorKind::FSError(
            "Invalid update channel".to_string(),
        ))
        .into());
    }

    let settings_dir = update_channel_settings_dir(&app)?;
    let mut state = theseus::read_update_channel_state(&settings_dir).await?;
    state.active_channel = Some(channel);
    write_update_channel_state(&app, &state)
}

#[tauri::command]
async fn get_update_preferences(
    app: tauri::AppHandle,
) -> api::Result<UpdatePreferences> {
    let settings_dir = update_channel_settings_dir(&app)?;
    let state = theseus::read_update_channel_state(&settings_dir).await?;
    let is_beta = match state.active_channel.as_deref() {
        Some("beta") => true,
        Some("release") => false,
        _ => theseus::default_update_channel() == "beta",
    };
    Ok(UpdatePreferences {
        immediate_update_fetch: is_beta
            || state.immediate_update_fetch.unwrap_or(false),
        updates_paused: state.updates_paused.unwrap_or(false),
    })
}

#[tauri::command]
async fn set_update_preferences(
    app: tauri::AppHandle,
    immediate_update_fetch: bool,
    updates_paused: bool,
) -> api::Result<()> {
    let settings_dir = update_channel_settings_dir(&app)?;
    let mut state = theseus::read_update_channel_state(&settings_dir).await?;
    state.immediate_update_fetch = Some(immediate_update_fetch);
    state.updates_paused = Some(updates_paused);
    write_update_channel_state(&app, &state)
}

fn update_channel_settings_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> api::Result<PathBuf> {
    theseus::DirectoryInfo::initial_settings_dir_path(&app.config().identifier)
        .ok_or_else(|| {
            theseus::Error::from(theseus::ErrorKind::FSError(
                "Could not find update channel settings directory".to_string(),
            ))
            .into()
        })
}

fn write_update_channel_state(
    app: &tauri::AppHandle,
    state: &theseus::UpdateChannelState,
) -> api::Result<()> {
    let settings_dir = update_channel_settings_dir(app)?;
    let path = theseus::update_channel_state_file_path(&settings_dir);
    std::fs::create_dir_all(&settings_dir)?;
    let temporary_path = settings_dir.join("update-channel.json.tmp");
    let contents = serde_json::to_vec(state).map_err(|error| {
        theseus::Error::from(theseus::ErrorKind::OtherError(error.to_string()))
    })?;
    std::fs::write(&temporary_path, contents)?;
    std::fs::rename(temporary_path, path)?;
    Ok(())
}

#[tauri::command]
async fn copy_release_database_to_beta(
    app: tauri::AppHandle,
) -> api::Result<()> {
    theseus::copy_release_database_to_beta(&app.config().identifier).await?;
    Ok(())
}

#[tauri::command]
async fn copy_database_between_channels(
    app: tauri::AppHandle,
    source_channel: String,
    target_channel: String,
) -> api::Result<()> {
    theseus::copy_database_between_channels(
        &app.config().identifier,
        &source_channel,
        &target_channel,
    )
    .await?;
    Ok(())
}

#[tauri::command]
async fn beta_database_exists(app: tauri::AppHandle) -> api::Result<bool> {
    Ok(theseus::beta_database_exists(&app.config().identifier).await?)
}

#[tauri::command]
async fn backup_app_db_for_update(
    app: tauri::AppHandle,
    version: String,
) -> api::Result<()> {
    theseus::backup_current_app_db_for_update(
        &app.config().identifier,
        &version,
    )
    .await?;
    Ok(())
}

#[tauri::command]
async fn set_discord_activity(activity: String) -> api::Result<()> {
    let state = State::get().await?;
    state
        .discord_rpc
        .set_launcher_activity(&activity, true)
        .await?;
    Ok(())
}

// Should be call once Vue has mounted the app
#[tracing::instrument(skip_all)]
#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    let win = app.get_window("main").unwrap();
    if let Err(e) = win.show() {
        DialogBuilder::message()
            .set_level(MessageLevel::Error)
            .set_title("Initialization error")
            .set_text(format!(
                "Cannot display application window due to an error:\n{e}"
            ))
            .alert()
            .show()
            .unwrap();
        panic!("cannot display application window")
    } else {
        let _ = win.set_focus();
    }
}

#[tauri::command]
async fn set_transparent_window_frame(
    enabled: bool,
    window: tauri::Window,
) -> Result<window_frame::FrameResult, String> {
    window_frame::apply_frame(enabled, &window).await
}

#[tauri::command]
fn is_dev() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
fn are_updates_enabled() -> bool {
    true
}

#[cfg(feature = "updater")]
pub use updater_impl::*;

#[cfg(not(feature = "updater"))]
pub use updater_impl_noop::*;

// Toggles decorations
#[tauri::command]
async fn toggle_decorations(b: bool, window: tauri::Window) -> api::Result<()> {
    window.set_decorations(b).map_err(|e| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to toggle decorations: {e}"
        )))
    })?;
    Ok(())
}

#[tauri::command]
async fn restart_app(app: tauri::AppHandle) -> api::Result<()> {
    if theseus::State::initialized()
        && theseus::instance::has_active_backup_operations().await?
    {
        return Err(theseus::Error::from(theseus::ErrorKind::InputError(
            "Backup operations are still running".to_string(),
        ))
        .into());
    }
    app.restart()
}

#[tauri::command]
async fn exit_app(app: tauri::AppHandle, force: bool) -> api::Result<()> {
    if theseus::State::initialized()
        && theseus::instance::has_active_backup_operations().await?
    {
        if !force {
            return Err(theseus::Error::from(theseus::ErrorKind::InputError(
                "Backup operations are still running".to_string(),
            ))
            .into());
        }
        if theseus::instance::has_active_backup_repository_move().await? {
            return Err(theseus::Error::from(theseus::ErrorKind::InputError(
                "The launcher cannot force exit while the backup repository is being moved"
                    .to_string(),
            ))
            .into());
        }
        theseus::instance::interrupt_active_backup_operations().await?;
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn check_symlink_capability() -> api::Result<String> {
    let capability = theseus::symlink::check_symlink_capability().await?;
    Ok(match capability {
        theseus::SymlinkCapability::Supported => "supported",
        theseus::SymlinkCapability::RequiresAdmin => "requires_admin",
        theseus::SymlinkCapability::Unsupported => "unsupported",
    }
    .to_string())
}

#[tauri::command]
fn is_elevated() -> bool {
    theseus::is_process_elevated()
}

#[tauri::command]
fn allow_symlink_target(app: tauri::AppHandle, path: String) {
    use tauri_plugin_fs::FsExt;
    let _ = app.fs_scope().allow_directory(&path, true);
}

#[tauri::command]
async fn set_restart_after_pending_update(
    should_restart: bool,
) -> api::Result<()> {
    let state = State::get().await?;
    state
        .restart_after_pending_update
        .store(should_restart, Ordering::Relaxed);
    Ok(())
}

// macOS caps each process at 256 open file descriptors by default
// (RLIMIT_NOFILE soft limit). The launcher's high download concurrency
// (up to 128 concurrent downloads, each split into multiple segments)
// easily exhausts this, causing "Too many open files (os error 24)"
// failures while installing modpacks. Raise the soft limit up to macOS's
// per-process ceiling (kern.maxfilesperproc, ~10240) at startup.
#[cfg(target_os = "macos")]
fn raise_file_descriptor_limit() {
    // SAFETY: Called at the start of main() before any threads or tokio
    // runtime are spawned.
    unsafe {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            eprintln!(
                "Failed to read RLIMIT_NOFILE: {}",
                std::io::Error::last_os_error()
            );
            return;
        }
        // The hard limit is usually unlimited (-1); cap the target so
        // setrlimit is accepted by the kernel.
        const TARGET: libc::rlim_t = 10240;
        let new_cur = if limit.rlim_max == libc::RLIM_INFINITY {
            TARGET
        } else {
            limit.rlim_max.min(TARGET)
        };
        if new_cur <= limit.rlim_cur {
            return;
        }
        limit.rlim_cur = new_cur;
        if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
            eprintln!(
                "Failed to raise RLIMIT_NOFILE to {new_cur}: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

// if Tauri app is called with arguments, then those arguments will be treated as commands
// ie: deep links or filepaths for .mrpacks
fn main() {
    // Initialize portable mode first (checks .Axolotl folder and sets THESEUS_CONFIG_DIR)
    // SAFETY: Called at the start of main() before any threads or tokio runtime are spawned
    let _portable = unsafe { portable::init_portable_mode() };

    // macOS limits the per-process file descriptor count to 256 by default,
    // which the launcher's download concurrency exhausts during installs.
    #[cfg(target_os = "macos")]
    raise_file_descriptor_limit();

    #[cfg(target_os = "windows")]
    if std::env::args_os().any(|argument| argument == "--memory-optimize") {
        std::process::exit(theseus::memory::optimize_current_process_context());
    }

    // Short-lived elevated helper entry: create a directory/file link with
    // administrator privileges, report the outcome through a result file, and
    // exit immediately. The main launcher process is never elevated.
    #[cfg(target_os = "windows")]
    {
        let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
        if let Some(index) = args
            .iter()
            .position(|argument| argument == "--elevated-create-link")
        {
            let payload = args
                .get(index + 1)
                .cloned()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            std::process::exit(theseus::symlink::create_link_elevated_helper(
                &payload,
            ));
        }
    }

    // Workaround: NVIDIA's proprietary EGL driver crashes WebKitGTK's DMA-BUF renderer
    #[cfg(target_os = "linux")]
    if env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        // SAFETY: This is called before any threads are spawned in main()
        unsafe { env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1") }
    }

    /*
        tracing is set basd on the environment variable RUST_LOG=xxx, depending on the amount of logs to show
            ERROR > WARN > INFO > DEBUG > TRACE
        eg. RUST_LOG=info will show info, warn, and error logs
            RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)
            RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)

        Error messages returned to Tauri will display as traced error logs if they return an error.
        This will also include an attached span trace if the error is from a tracing error, and the level is set to info, debug, or trace

        on unix:
            RUST_LOG="theseus=trace" {run command}

    */

    // Configure the tokio runtime with a larger thread stack size to prevent
    // stack overflows on Windows during deep async call chains (e.g. pack
    // import). The /STACK linker flag in .cargo/config.toml only affects the
    // main thread, not tokio worker threads.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(16 * 1024 * 1024)
        .build()
        .expect("failed to build tokio runtime");
    let handle = rt.handle().clone();
    // SAFETY: tauri::async_runtime::set() takes a Handle by value but does not
    // take ownership of the Runtime itself — Tauri expects the runtime to
    // outlive main(). Since the Handle borrows the Runtime internally, we must
    // leak the Runtime here so its worker threads and timer heap live for the
    // process lifetime. The OS will reclaim the leaked pages on exit. The
    // alternative (wrapping in an Arc and passing it to a permanent scope) is
    // not supported by the tauri::async_runtime API.
    std::mem::forget(rt);
    tauri::async_runtime::set(handle);

    let tauri_context = tauri::tauri_build_context!();

    let _log_guard = theseus::start_logger(&tauri_context.config().identifier);

    tracing::info!("Initialized tracing subscriber. Loading Axolotl Launcher!");

    let mut builder = tauri::Builder::default().register_uri_scheme_protocol(
        "axolotl-skin",
        move |context, request| {
            if !is_allowed_blockbench_skin_request(&request) {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Vec::new())
                    .expect(
                        "failed to build Blockbench skin forbidden response",
                    );
            }
            let resource_dir = context
                .app_handle()
                .path()
                .resource_dir()
                .expect("failed to resolve Tauri resource directory")
                .join(BLOCKBENCH_SKIN_RESOURCE_DIR);
            blockbench_skin_response(request.uri().path(), &resource_dir)
        },
    );

    #[cfg(feature = "updater")]
    {
        use tauri_plugin_http::reqwest::header::{HeaderValue, USER_AGENT};
        use theseus::launcher_user_agent;
        builder = builder.plugin(
            tauri_plugin_updater::Builder::new()
                .header(
                    USER_AGENT,
                    HeaderValue::from_str(&launcher_user_agent()).unwrap(),
                )
                .unwrap()
                .build(),
        );
    }

    builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(payload) = args.get(1) {
                tracing::info!("Handling deep link from arg {payload}");
                let payload = payload.clone();
                tauri::async_runtime::spawn(api::utils::handle_command(
                    payload,
                ));
            }

            // A second launch must restore the launcher UI. The main window
            // may be hidden or destroyed when the app is in lightweight mode,
            // so focusing it is not enough (see #490).
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ =
                    app.state::<lightweight_mode::LightweightMode>().exit(&app);
            });
        }))
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin({
            let mut window_state_builder =
                tauri_plugin_window_state::Builder::default()
                    .with_filename("app-window-state.json")
                    // Use *only* POSITION and SIZE state flags, because saving VISIBLE causes the `visible: false` to not take effect
                    .with_state_flags(
                        tauri_plugin_window_state::StateFlags::POSITION
                            | tauri_plugin_window_state::StateFlags::SIZE
                            | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                    );

            // Use THESEUS_CONFIG_DIR for window state if set (portable mode)
            if let Some(config_dir) = std::env::var_os("THESEUS_CONFIG_DIR") {
                window_state_builder =
                    window_state_builder.with_state_dir(config_dir);
            }

            window_state_builder.build()
        })
        .setup(|app| {
            lightweight_mode::init(app.handle());
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                macos::traffic_lights::install(&window)?;
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                if let Some(window) = app_handle.get_window("main")
                    && !window.is_visible().unwrap_or(true)
                {
                    let _ = window.show();
                }
            });

            #[cfg(target_os = "macos")]
            {
                let payload = macos::deep_link::get_or_init_payload(app);

                let mtx_copy = payload.payload;
                app.listen("deep-link://new-url", move |url| {
                    let mtx_copy_copy = mtx_copy.clone();
                    let request = url.payload().to_owned();

                    let actual_request =
                        serde_json::from_str::<Vec<String>>(&request)
                            .ok()
                            .map(|mut x| x.remove(0))
                            .unwrap_or(request);

                    tauri::async_runtime::spawn(async move {
                        tracing::info!("Handling deep link {actual_request}");

                        let mut payload = mtx_copy_copy.lock().await;
                        if payload.is_none() {
                            *payload = Some(actual_request.clone());
                        }

                        let _ =
                            api::utils::handle_command(actual_request).await;
                    });
                });
            };

            #[cfg(not(target_os = "macos"))]
            app.listen("deep-link://new-url", |url| {
                let payload = url.payload().to_owned();
                tracing::info!("Handling deep link {payload}");
                tauri::async_runtime::spawn(api::utils::handle_command(
                    payload,
                ));
            });

            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_window("main")
                && let Err(e) = window.set_shadow(true)
            {
                tracing::warn!("Failed to set window shadow: {e}");
            }

            Ok(())
        });

    builder = builder
        .plugin(api::ai::init())
        .plugin(api::auth::init())
        .plugin(api::mr_auth::init())
        .plugin(api::import::init())
        .plugin(api::install::init())
        .plugin(api::instance::init())
        .plugin(api::logs::init())
        .plugin(api::jre::init())
        .plugin(api::metadata::init())
        .plugin(api::mcarchive::init())
        .plugin(api::minecraft_skins::init())
        .plugin(api::mod_translation::init())
        .plugin(api::process::init())
        .plugin(api::planet_minecraft::init())
        .plugin(api::settings::init())
        .plugin(api::storage::init())
        .plugin(api::system_accent::init())
        .plugin(api::seed_map::init())
        .plugin(api::schematic_preview::init())
        .plugin(api::shortcuts::init())
        .plugin(api::tags::init())
        .plugin(api::telemetry::init())
        .plugin(api::translation::init())
        .plugin(api::utils::init())
        .plugin(api::cache::init())
        .plugin(api::content_favorites::init())
        .plugin(api::content_search::init())
        .plugin(api::resources::init())
        .plugin(api::curseforge::init())
        .plugin(api::datapacks::init())
        .plugin(api::drop::init())
        .plugin(api::files::init())
        .plugin(api::fonts::init())
        .plugin(api::friends::init())
        .plugin(api::worlds::init())
        .plugin(api::terracotta::init())
        .plugin(api::multiplayer::init())
        .manage(api::files::StudioWatchers::default())
        .plugin(api::servers::init())
        .manage(PendingUpdateData::default())
        .invoke_handler(tauri::generate_handler![
            initialize_state,
            get_update_channel,
            get_current_app_database_path,
            set_update_channel,
            get_update_preferences,
            set_update_preferences,
            copy_release_database_to_beta,
            copy_database_between_channels,
            beta_database_exists,
            backup_app_db_for_update,
            set_discord_activity,
            is_dev,
            portable::is_portable_mode,
            are_updates_enabled,
            get_update_size,
            check_app_update,
            enqueue_update_for_installation,
            remove_enqueued_update,
            set_restart_after_pending_update,
            is_apt_linux,
            toggle_decorations,
            set_transparent_window_frame,
            show_window,
            restart_app,
            exit_app,
            check_symlink_capability,
            is_elevated,
            allow_symlink_target,
            lightweight_mode::lightweight_mode_frontend_ready,
            lightweight_mode::lightweight_mode_set_route,
            lightweight_mode::lightweight_mode_enter,
            #[cfg(target_os = "macos")]
            macos::traffic_lights::update_traffic_lights_anchor,
        ]);

    tracing::info!("Initializing app...");
    let app = builder.build(tauri_context);

    match app {
        Ok(app) => {
            app.run(|app, event| {
                #[cfg(not(any(feature = "updater", target_os = "macos")))]
                let _ = app;

                if matches!(&event, tauri::RunEvent::ExitRequested { .. })
                    && let Err(error) = tauri::async_runtime::block_on(
                        theseus::minecraft_skins::flush_pending_skin_change(),
                    )
                {
                    tracing::warn!(
                        "Failed to flush pending Minecraft skin change before exit: {error}"
                    );
                }

                if matches!(&event, tauri::RunEvent::ExitRequested { .. })
                    && let Err(error) = tauri::async_runtime::block_on(
                        theseus::multiplayer::shutdown(),
                    )
                {
                    tracing::warn!(
                        "Failed to stop multiplayer services before exit: {error}"
                    );
                }

                if matches!(&event, tauri::RunEvent::ExitRequested { .. }) {
                    tauri::async_runtime::block_on(async {
                        let killed = theseus::servers::shutdown_all().await;
                        if killed > 0 {
                            tracing::info!(
                                "Killed {killed} dedicated server process(es) after the shutdown budget"
                            );
                        }
                        if let Some(state) = State::get_if_initialized() {
                            state.shutdown().await;
                        }
                    });
                }

                #[cfg(feature = "updater")]
                if matches!(&event, tauri::RunEvent::Exit) {
                    let update_data = app.state::<PendingUpdateData>().inner();
                    let should_restart = State::get_if_initialized()
                        .map(|s| {
                            s.restart_after_pending_update.load(Ordering::Relaxed)
                        })
                        .unwrap_or(false);
                    if let Some((update, data)) = &*update_data.0.lock().unwrap()
                    {
                        fn set_changelog_toast(version: Option<String>) {
                            let toast_result: theseus::Result<()> = tauri::async_runtime::block_on(async move {
                                settings::patch(serde_json::json!({ "pending_update_toast_for_version": version })).await?;
                                Ok(())
                            });
                            if let Err(e) = toast_result {
                                tracing::warn!(
                                    "Failed to set pending_update_toast: {e}"
                                )
                            }
                        }

                        let version = update.version.clone();
                        let install_result = if is_apt_linux() {
                            tauri::async_runtime::block_on(
                                install_apt_package(&version, data),
                            )
                            .map_err(|error| error.to_string())
                        } else {
                            let update = if should_restart {
                                (**update).clone()
                            } else {
                                (**update).clone().restart_after_install(false)
                            };

                            // Persist the trigger before installing: on Windows
                            // the updater plugin launches the NSIS installer and
                            // exits the process via `std::process::exit(0)`
                            // without returning, so the success path below never
                            // runs there.
                            #[cfg(target_os = "windows")]
                            set_changelog_toast(Some(update.version.clone()));

                            update.install(data).map_err(|error| error.to_string())
                        };

                        match install_result {
                            Ok(()) => {
                                set_changelog_toast(Some(version.clone()));
                                if should_restart {
                                    tracing::info!(
                                        "Pending update installed successfully (version {}); restarting because user requested reload",
                                        version
                                    );
                                    app.restart();
                                } else {
                                    tracing::info!(
                                        "Pending update installed successfully (version {}); exiting without relaunch (user did not request reload)",
                                        version
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Pending update install failed (version {}): {e}",
                                    version
                                );
                                set_changelog_toast(None);

                                DialogBuilder::message()
                                    .set_level(MessageLevel::Error)
                                    .set_title("Update error")
                                    .set_text(format!("Failed to install update due to an error:\n{e}"))
                                    .alert()
                                    .show()
                                    .unwrap();
                            }
                        }
                    }
                }
                if matches!(&event, tauri::RunEvent::Exit)
                    && let Some(state) = State::get_if_initialized()
                    && let Err(error) = tauri::async_runtime::block_on(state.finish_database_session())
                {
                    tracing::warn!(%error, "Could not record a clean database shutdown");
                }
                #[cfg(target_os = "macos")]
                if let tauri::RunEvent::Opened { urls } = event {
                    tracing::info!("Handling webview open {urls:?}");

                    let file = urls
                        .into_iter()
                        .find_map(|url| url.to_file_path().ok());

                    if let Some(file) = file {
                        let payload =
                            macos::deep_link::get_or_init_payload(app);

                        let mtx_copy = payload.payload;
                        let request = file.to_string_lossy().to_string();
                        tauri::async_runtime::spawn(async move {
                            let mut payload = mtx_copy.lock().await;
                            if payload.is_none() {
                                *payload = Some(request.clone());
                            }

                            let _ = api::utils::handle_command(request).await;
                        });
                    }
                }
            });
        }
        Err(e) => {
            tracing::error!("Error while running tauri application: {:?}", e);

            #[cfg(target_os = "windows")]
            {
                // tauri doesn't expose runtime errors, so matching a string representation seems like the only solution
                if format!("{e:?}").contains(
                    "Runtime(CreateWebview(WebView2Error(WindowsError",
                ) {
                    DialogBuilder::message()
                        .set_level(MessageLevel::Error)
                        .set_title("Initialization error")
                        .set_text("Your Microsoft Edge WebView2 installation is corrupt.\n\nMicrosoft Edge WebView2 is required to run Axolotl Launcher.\n\nRepair or reinstall the Microsoft Edge WebView2 Runtime, then start Axolotl again.")
                        .alert()
                        .show()
                        .unwrap();

                    panic!("webview2 initialization failed")
                }
            }

            DialogBuilder::message()
                .set_level(MessageLevel::Error)
                .set_title("Initialization error")
                .set_text(format!(
                    "Cannot initialize application due to an error:\n{e:?}"
                ))
                .alert()
                .show()
                .unwrap();

            panic!("{1}: {:?}", e, "error while running tauri application")
        }
    }
}
