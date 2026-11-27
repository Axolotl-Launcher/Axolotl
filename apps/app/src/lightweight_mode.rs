use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    sync::Mutex,
};
use tauri::{
    AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::WindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
use theseus::prelude::CommandPayload;

mod tray_menu;
use tray_menu::{TrayChange, TrayRefresh, schedule_tray_menu_update};

const MAIN_WINDOW_LABEL: &str = "main";
const LIGHTWEIGHT_HOST_WINDOW_LABEL: &str = "lightweight-host";
const TRAY_ID: &str = "main";
const LAUNCH_INSTANCE_PREFIX: &str = "launch-instance:";

#[derive(Clone, Copy, PartialEq, Eq)]
struct TrayLabels {
    show_launcher: &'static str,
    launch_instance: &'static str,
    launch_failed: &'static str,
    lightweight_mode: &'static str,
    quit: &'static str,
    running_prefix: &'static str,
}

fn tray_labels(locale: &str) -> TrayLabels {
    if locale.eq_ignore_ascii_case("zh-CN") {
        TrayLabels {
            show_launcher: "显示 Axolotl 启动器",
            launch_instance: "启动实例",
            launch_failed: "无法启动实例",
            lightweight_mode: "轻量模式",
            quit: "退出",
            running_prefix: "正在运行：",
        }
    } else if locale.eq_ignore_ascii_case("zh-TW") {
        TrayLabels {
            show_launcher: "顯示 Axolotl 啟動器",
            launch_instance: "啟動實例",
            launch_failed: "無法啟動實例",
            lightweight_mode: "輕量模式",
            quit: "結束",
            running_prefix: "正在執行：",
        }
    } else {
        TrayLabels {
            show_launcher: "Show Axolotl Launcher",
            launch_instance: "Launch instance",
            launch_failed: "Could not launch instance",
            lightweight_mode: "Lightweight mode",
            quit: "Quit",
            running_prefix: "Running: ",
        }
    }
}

struct LightweightModeState {
    active: bool,
    route: String,
    running_processes: usize,
    pending_crashes: VecDeque<PendingCrash>,
    pending_commands: VecDeque<CommandPayload>,
    frontend_ready: bool,
    restoring: bool,
    running_instance_ids: HashSet<String>,
}

impl Default for LightweightModeState {
    fn default() -> Self {
        Self {
            active: false,
            route: "/".to_string(),
            running_processes: 0,
            pending_crashes: VecDeque::new(),
            pending_commands: VecDeque::new(),
            frontend_ready: false,
            restoring: false,
            running_instance_ids: HashSet::new(),
        }
    }
}

#[derive(Serialize)]
pub struct PendingCrash {
    pub instance_id: String,
    pub uuid: String,
}

#[derive(Default)]
pub struct LightweightMode(
    Mutex<LightweightModeState>,
    tokio::sync::Mutex<()>,
    TrayRefresh,
);

fn keep_running_on_exit(
    code: Option<i32>,
    active: bool,
    transitioning: bool,
) -> bool {
    code.is_none() && (active || transitioning)
}

pub(crate) fn prevent_implicit_tray_exit(
    app: &AppHandle,
    code: Option<i32>,
) -> bool {
    app.try_state::<LightweightMode>().is_some_and(|mode| {
        keep_running_on_exit(code, mode.is_active(), mode.1.try_lock().is_err())
    })
}

impl LightweightMode {
    async fn enter(&self, app: &AppHandle) -> Result<(), String> {
        self.enter_internal(app, true).await
    }

    async fn enter_for_close(&self, app: &AppHandle) -> Result<(), String> {
        save_window_state_on_main_thread(app).await?;
        self.enter_internal(app, false).await
    }

    async fn enter_internal(
        &self,
        app: &AppHandle,
        require_running: bool,
    ) -> Result<(), String> {
        let _transition = self.1.lock().await;
        let state = self.0.lock().map_err(|error| error.to_string())?;
        if state.active {
            return Ok(());
        }
        if require_running && state.running_processes == 0 {
            return Err(
                "Lightweight mode requires a running Minecraft instance"
                    .to_string(),
            );
        }
        let idle = state.running_processes == 0;
        drop(state);
        // Keep the native, webview-free host for the process lifetime once
        // created. Rapid restores must not remove the tray's last window.
        create_lightweight_host_window(app)?;
        let mut state = self.0.lock().map_err(|error| error.to_string())?;
        let was_frontend_ready = state.frontend_ready;
        state.active = true;
        state.frontend_ready = false;
        if let Some(theseus_state) = theseus::State::get_if_initialized() {
            theseus_state.pause_background_services();
        }
        drop(state);
        if let Err(error) = destroy_main_window(app) {
            if let Ok(mut state) = self.0.lock() {
                state.active = false;
                state.frontend_ready = was_frontend_ready;
            }
            if let Some(theseus_state) = theseus::State::get_if_initialized() {
                theseus_state.resume_background_services();
            }
            return Err(error);
        }
        schedule_lightweight_maintenance(idle);
        schedule_tray_menu_update(app, TrayChange::Presentation);
        Ok(())
    }

    pub async fn exit(&self, app: &AppHandle) -> Result<(), String> {
        let _transition = self.1.lock().await;
        let has_main_window = app.get_window(MAIN_WINDOW_LABEL).is_some();
        let has_tray_host =
            app.get_window(LIGHTWEIGHT_HOST_WINDOW_LABEL).is_some();

        let route = {
            let mut state = self.0.lock().map_err(|error| error.to_string())?;
            if !state.active && (has_main_window || !has_tray_host) {
                drop(state);
                return show_main_window(app);
            }
            state.frontend_ready = false;
            state.restoring = true;
            state.route.clone()
        };
        let result = wait_for_main_window_destruction(app)
            .await
            .and_then(|()| create_main_window(app, &route));
        if let Err(error) = result {
            if let Ok(mut state) = self.0.lock() {
                state.restoring = false;
            }
            return Err(error);
        }
        if let Ok(mut state) = self.0.lock() {
            state.active = false;
            if let Some(theseus_state) = theseus::State::get_if_initialized() {
                theseus_state.resume_background_services();
            }
        }
        schedule_tray_menu_update(app, TrayChange::Presentation);
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.0.lock().map(|state| state.active).unwrap_or(false)
    }

    fn process_event(&self, app: &AppHandle, payload: ProcessEventPayload) {
        if payload.lightweight_replay {
            return;
        }

        let restore_window = {
            let mut state = match self.0.lock() {
                Ok(state) => state,
                Err(error) => {
                    tracing::error!(
                        "Failed to lock lightweight mode state: {error}"
                    );
                    return;
                }
            };
            match payload.event.as_str() {
                "launched" => {
                    state.running_processes += 1;
                    state.running_instance_ids.insert(payload.instance_id);
                    false
                }
                "finished" => {
                    state.running_processes =
                        state.running_processes.saturating_sub(1);
                    state.running_instance_ids.remove(&payload.instance_id);
                    let crashed = payload.crashed == Some(true);
                    if state.active && crashed {
                        state.pending_crashes.push_back(PendingCrash {
                            instance_id: payload.instance_id,
                            uuid: payload.uuid,
                        });
                    }
                    state.running_processes == 0 || (state.active && crashed)
                }
                _ => return,
            }
        };

        schedule_tray_menu_update(app, TrayChange::Presentation);
        if restore_window {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) =
                    app.state::<LightweightMode>().exit(&app).await
                {
                    tracing::error!(%error, "Failed to restore launcher after Minecraft exited");
                }
            });
        } else if payload.event == "launched" {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if payload.maximize_window {
                    maximize_minecraft_window(
                        payload.pid,
                        payload.launch_preparation_timeout,
                    )
                    .await;
                }
                if let Some(title) = payload
                    .window_title
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                {
                    rename_minecraft_window(
                        payload.pid,
                        title,
                        payload.launch_preparation_timeout,
                    )
                    .await;
                }
                let settings = match theseus::settings::get().await {
                    Ok(settings) => settings,
                    Err(error) => {
                        tracing::warn!(
                            "Failed to read lightweight mode setting: {error}"
                        );
                        return;
                    }
                };
                if settings.enter_lightweight_mode_on_game_launch {
                    let state = app.state::<LightweightMode>();
                    if let Err(error) = state.enter(&app).await {
                        tracing::warn!(
                            "Failed to enter lightweight mode: {error}"
                        );
                    }
                } else if settings.hide_on_process_start
                    && let Some(window) =
                        app.get_webview_window(MAIN_WINDOW_LABEL)
                    && let Err(error) = window.minimize()
                {
                    tracing::warn!(
                        "Failed to minimize launcher after Minecraft started: {error}"
                    );
                }
            });
        }
    }

    fn set_route(&self, route: String) {
        if route.starts_with('/')
            && let Ok(mut state) = self.0.lock()
        {
            state.route = route;
        }
    }

    fn mark_frontend_ready(&self) -> (Vec<PendingCrash>, Vec<CommandPayload>) {
        self.0
            .lock()
            .map(|mut state| {
                state.frontend_ready = true;
                state.restoring = false;
                (
                    state.pending_crashes.drain(..).collect(),
                    state.pending_commands.drain(..).collect(),
                )
            })
            .unwrap_or_else(|_| (Vec::new(), Vec::new()))
    }

    fn queue_command(&self, command: CommandPayload) {
        if let Ok(mut state) = self.0.lock() {
            state.pending_commands.push_back(command);
        }
    }

    fn is_frontend_ready(&self) -> bool {
        self.0
            .lock()
            .map(|state| state.frontend_ready)
            .unwrap_or(false)
    }

    fn is_restoring(&self) -> bool {
        self.0.lock().map(|state| state.restoring).unwrap_or(false)
    }
}

/// Runs the post-enter maintenance work off the caller's thread: stop idle
/// multiplayer tunnels, then trim the launcher's working set once the
/// destroyed webview's processes had a moment to exit.
fn schedule_lightweight_maintenance(idle: bool) {
    tauri::async_runtime::spawn(async move {
        if idle && let Err(error) = theseus::multiplayer::stop().await {
            tracing::debug!(
                "Failed to stop multiplayer tunnels entering lightweight mode: {error}"
            );
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Some((before, after)) =
            theseus::memory::trim_launcher_working_set().await
        {
            tracing::info!(
                before_bytes = before,
                after_bytes = after,
                "Trimmed the launcher working set for lightweight mode"
            );
        }
    });
}

#[derive(serde::Deserialize)]
struct ProcessEventPayload {
    instance_id: String,
    uuid: String,
    #[serde(default)]
    pid: u32,
    #[serde(default)]
    maximize_window: bool,
    #[serde(default)]
    window_title: Option<String>,
    #[serde(default)]
    launch_preparation_timeout: Option<u64>,
    event: String,
    crashed: Option<bool>,
    #[serde(default)]
    lightweight_replay: bool,
}

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

#[cfg(target_os = "windows")]
static MAXIMIZE_PROCESS_ID: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "windows")]
static MAXIMIZE_WINDOW_HANDLE: AtomicIsize = AtomicIsize::new(0);
#[cfg(target_os = "windows")]
static MAXIMIZE_WINDOW_ENUMERATION: Mutex<()> = Mutex::new(());

#[cfg(target_os = "windows")]
unsafe extern "system" fn maximize_if_owned_by_process(
    hwnd: windows::Win32::Foundation::HWND,
    _: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };
    use windows::core::BOOL;

    let mut class_name = [0_u16; 256];
    let class_name_length = unsafe { GetClassNameW(hwnd, &mut class_name) };
    let class_name =
        String::from_utf16_lossy(&class_name[..class_name_length as usize]);
    if !matches!(class_name.as_str(), "GLFW30" | "LWJGL" | "SunAwtFrame") {
        return BOOL(1);
    }

    let mut window_title = [0_u16; 512];
    let window_title_length =
        unsafe { GetWindowTextW(hwnd, &mut window_title) };
    let window_title =
        String::from_utf16_lossy(&window_title[..window_title_length as usize]);
    if !(window_title.starts_with("FML")
        || (window_title != "PopupMessageWindow"
            && !window_title.starts_with("GLFW")))
    {
        return BOOL(1);
    }

    // FML and Quilt Loader windows are transitional launcher windows. Keep
    // enumerating until the actual Minecraft window appears.
    if window_title.starts_with("FML")
        || window_title.starts_with("Quilt Loader")
    {
        return BOOL(1);
    }

    let mut window_pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut window_pid)) };
    if window_pid == MAXIMIZE_PROCESS_ID.load(Ordering::Relaxed)
        && unsafe { IsWindowVisible(hwnd).as_bool() }
    {
        MAXIMIZE_WINDOW_HANDLE.store(hwnd.0 as isize, Ordering::Relaxed);
        return BOOL(0);
    }
    BOOL(1)
}

#[cfg(target_os = "windows")]
async fn maximize_minecraft_window(
    pid: u32,
    launch_preparation_timeout: Option<u64>,
) {
    if pid == 0 {
        return;
    }

    let timeout = std::time::Duration::from_secs(
        launch_preparation_timeout.unwrap_or(60),
    );
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let found = {
            let _guard = MAXIMIZE_WINDOW_ENUMERATION.lock();
            MAXIMIZE_PROCESS_ID.store(pid, Ordering::Relaxed);
            MAXIMIZE_WINDOW_HANDLE.store(0, Ordering::Relaxed);
            unsafe {
                use windows::Win32::Foundation::LPARAM;
                use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
                let _ =
                    EnumWindows(Some(maximize_if_owned_by_process), LPARAM(0));
            }
            MAXIMIZE_WINDOW_HANDLE.load(Ordering::Relaxed)
        };
        if found != 0 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let hwnd = windows::Win32::Foundation::HWND(found as *mut _);
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{
                    SW_MAXIMIZE, ShowWindow,
                };
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            }
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

#[cfg(not(target_os = "windows"))]
async fn maximize_minecraft_window(
    _pid: u32,
    _launch_preparation_timeout: Option<u64>,
) {
}

#[cfg(target_os = "windows")]
async fn rename_minecraft_window(
    pid: u32,
    title: &str,
    launch_preparation_timeout: Option<u64>,
) {
    if pid == 0 || title.is_empty() {
        return;
    }

    // PCL keeps re-applying the title: FML/Quilt and some mods reset it
    // while the game boots, so a one-shot rename is not enough.
    let timeout = std::time::Duration::from_secs(
        launch_preparation_timeout.unwrap_or(60),
    );
    let deadline = tokio::time::Instant::now() + timeout;
    let title = title.to_string();
    let mut applied = false;
    let mut hold_until: Option<tokio::time::Instant> = None;
    loop {
        let found = {
            let _guard = MAXIMIZE_WINDOW_ENUMERATION.lock();
            MAXIMIZE_PROCESS_ID.store(pid, Ordering::Relaxed);
            MAXIMIZE_WINDOW_HANDLE.store(0, Ordering::Relaxed);
            unsafe {
                use windows::Win32::Foundation::LPARAM;
                use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
                let _ =
                    EnumWindows(Some(maximize_if_owned_by_process), LPARAM(0));
            }
            MAXIMIZE_WINDOW_HANDLE.load(Ordering::Relaxed)
        };
        if found != 0 {
            let hwnd = windows::Win32::Foundation::HWND(found as *mut _);
            let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::SetWindowTextW;
                let _ =
                    SetWindowTextW(hwnd, windows::core::PCWSTR(wide.as_ptr()));
            }
            if !applied {
                applied = true;
                // Keep watching for a bit after the first success so loader
                // splash windows cannot leave the default title behind.
                hold_until = Some(
                    tokio::time::Instant::now()
                        + std::time::Duration::from_secs(45),
                );
            }
        }
        let now = tokio::time::Instant::now();
        if let Some(hold) = hold_until
            && now >= hold
        {
            return;
        }
        if hold_until.is_none() && now >= deadline {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
}

#[cfg(not(target_os = "windows"))]
async fn rename_minecraft_window(
    _pid: u32,
    _title: &str,
    _launch_preparation_timeout: Option<u64>,
) {
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lightweight_mode_frontend_ready(
    app: AppHandle,
    route: String,
) -> Result<FrontendReadyPayload, String> {
    let state = app.state::<LightweightMode>();
    state.set_route(route);
    let (pending_crashes, pending_commands) = state.mark_frontend_ready();
    schedule_tray_menu_update(&app, TrayChange::Presentation);
    Ok(FrontendReadyPayload {
        pending_crashes,
        pending_commands,
    })
}

#[derive(Serialize)]
pub struct FrontendReadyPayload {
    pub pending_crashes: Vec<PendingCrash>,
    pub pending_commands: Vec<CommandPayload>,
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lightweight_mode_set_route(app: AppHandle, route: String) {
    app.state::<LightweightMode>().set_route(route);
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn lightweight_mode_enter(app: AppHandle) -> Result<(), String> {
    // Schedule the window destruction after returning from the IPC command.
    // Destroying the webview synchronously would leave the invoke caller
    // waiting for a response from a window that is already being destroyed.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Err(error) =
            app.state::<LightweightMode>().enter_for_close(&app).await
        {
            tracing::warn!(
                "Failed to enter lightweight mode on close: {error}"
            );
            let _ = app.emit("lightweight-mode-error", error);
        }
    });
    Ok(())
}

async fn save_window_state_on_main_thread(
    app: &AppHandle,
) -> Result<(), String> {
    let (send, saved) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    // The plugin holds its state cache lock while querying native windows.
    // Running the entire operation on the event loop avoids waiting on it
    // while a window event is trying to acquire that same lock.
    app.run_on_main_thread(move || {
        let _ = send.send(handle.save_window_state(StateFlags::all()));
    })
    .map_err(|error| error.to_string())?;
    saved
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

fn destroy_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.destroy().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn create_lightweight_host_window(app: &AppHandle) -> Result<(), String> {
    if app.get_window(LIGHTWEIGHT_HOST_WINDOW_LABEL).is_none() {
        WindowBuilder::new(app, LIGHTWEIGHT_HOST_WINDOW_LABEL)
            .title("Axolotl Launcher")
            .visible(false)
            .focused(false)
            .focusable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn wait_for_main_window_destruction(
    app: &AppHandle,
) -> Result<(), String> {
    // Tauri queues Destroy on the native event loop. The old window can still
    // occupy its label when a restore request arrives immediately afterward.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while app.get_window(MAIN_WINDOW_LABEL).is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| "The previous launcher window has not closed yet".to_string())
}

fn create_main_window(app: &AppHandle, route: &str) -> Result<(), String> {
    if app.get_webview_window(MAIN_WINDOW_LABEL).is_none() {
        let mut builder = WebviewWindowBuilder::new(
            app,
            MAIN_WINDOW_LABEL,
            WebviewUrl::App(route.into()),
        )
        .title("Axolotl Launcher")
        .inner_size(1280.0, 800.0)
        .min_inner_size(1100.0, 700.0)
        .resizable(true)
        .transparent(true)
        .zoom_hotkeys_enabled(false)
        .visible(false);
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.decorations(false);
        }
        #[cfg(target_os = "macos")]
        {
            builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(tauri::LogicalPosition::new(
                    15.0, 22.0,
                ));
        }
        builder.build().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_window(MAIN_WINDOW_LABEL) {
        crate::window_frame::present_main_window(&window, true)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show-launcher" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = app.state::<LightweightMode>().exit(&app).await;
            });
        }
        "lightweight-mode" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<LightweightMode>();
                if state.is_active() {
                    let _ = state.exit(&app).await;
                } else if let Err(error) = state.enter(&app).await {
                    tracing::debug!(
                        "Lightweight mode was not entered from tray: {error}"
                    );
                }
            });
        }
        "quit" => app.exit(0),
        instance_id if instance_id.starts_with(LAUNCH_INSTANCE_PREFIX) => {
            let instance_id = instance_id
                .trim_start_matches(LAUNCH_INSTANCE_PREFIX)
                .to_string();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = theseus::instance::run(
                    &instance_id,
                    theseus::instance::QuickPlayType::None,
                    false,
                )
                .await
                {
                    tracing::error!(
                        "Failed to launch tray instance {instance_id}: {error}"
                    );
                    let locale = theseus::settings::get()
                        .await
                        .map(|settings| settings.locale)
                        .unwrap_or_default();
                    app.dialog()
                        .message(error.to_string())
                        .title(tray_labels(&locale).launch_failed)
                        .kind(MessageDialogKind::Error)
                        .show(|_| {});
                }
            });
        }
        _ => {}
    }
}

pub fn init(app: &AppHandle) {
    app.manage(LightweightMode::default());
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .expect("missing default app icon")
                .clone(),
        )
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let app = tray.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = app.state::<LightweightMode>().exit(&app).await;
                });
            }
        })
        .build(app)
        .expect("failed to create system tray");
    let _tray = tray;
    tray_menu::start(app);
    let app_handle = app.clone();
    app.listen("process", move |event| {
        let Ok(payload) =
            serde_json::from_str::<ProcessEventPayload>(event.payload())
        else {
            return;
        };
        app_handle
            .state::<LightweightMode>()
            .process_event(&app_handle, payload);
    });
    let app_handle = app.clone();
    app.listen("instance", move |event| {
        if tray_menu::changes_instance_menu(event.payload()) {
            schedule_tray_menu_update(&app_handle, TrayChange::Instances);
        }
    });
    let app_handle = app.clone();
    app.listen("settings", move |_| {
        schedule_tray_menu_update(&app_handle, TrayChange::Settings);
    });
    let app_handle = app.clone();
    app.listen("command", move |event| {
        let Ok(command) =
            serde_json::from_str::<CommandPayload>(event.payload())
        else {
            return;
        };
        let state = app_handle.state::<LightweightMode>();
        if state.is_frontend_ready()
            || (!state.is_active() && !state.is_restoring())
        {
            return;
        }
        state.queue_command(command);
        if !state.is_active() {
            return;
        }
        let app = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let _ = app.state::<LightweightMode>().exit(&app).await;
        });
    });
}

#[cfg(test)]
mod tests {
    use super::keep_running_on_exit;

    #[test]
    fn implicit_exit_during_tray_mode_keeps_the_backend_alive() {
        assert!(keep_running_on_exit(None, true, false));
        assert!(keep_running_on_exit(None, false, true));
        assert!(!keep_running_on_exit(None, false, false));
    }

    #[test]
    fn explicit_quit_and_restart_can_exit_from_the_tray() {
        for code in [0, tauri::RESTART_EXIT_CODE] {
            assert!(!keep_running_on_exit(Some(code), true, true));
        }
    }
}
