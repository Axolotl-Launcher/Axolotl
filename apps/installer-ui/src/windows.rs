use std::{
    cell::Cell,
    env, fs,
    io::{self, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use native_dialog::DialogBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};
use tempfile::TempDir;
use webview2_com::{
    BrowserProcessExitedEventHandler,
    Microsoft::Web::WebView2::Win32::ICoreWebView2Environment5,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{GetExitCodeProcess, WaitForSingleObject},
        UI::Shell::{
            SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
        },
        UI::WindowsAndMessaging::SW_HIDE,
    },
    core::{Interface, PCWSTR},
};
use wry::{
    NewWindowResponse, WebContext, WebView, WebViewBuilder, WebViewExtWindows,
    http::Request,
};

const HTML: &str = include_str!("installer.html");
const LOGO: &[u8] = include_bytes!("../../app/icons/128x128.png");
const MAIN_BINARY_NAME: &str = "Axolotl Launcher.exe";
const WEBVIEW_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    version: String,
    install_dir: String,
    resource_dir: String,
    fresh_install: bool,
    language: Language,
    logo_data_url: String,
    uninstall: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Language {
    En,
    ZhCn,
}

#[derive(Debug)]
struct Arguments {
    installer: PathBuf,
    bootstrap: Bootstrap,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRequest {
    install_dir: String,
    resource_dir: String,
    desktop_shortcut: bool,
    launch_after: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UninstallRequest {
    delete_app_data: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum PathTarget {
    Install,
    Resource,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "camelCase")]
enum UiCommand {
    Minimize,
    DragWindow,
    Close,
    Browse { target: PathTarget, current: String },
    Install(InstallRequest),
    Uninstall(UninstallRequest),
    Finish { launch: bool },
}

#[derive(Debug)]
enum UserEvent {
    BrowserProcessExited,
    Minimize,
    DragWindow,
    Close,
    Browse { target: PathTarget, current: String },
    Install(InstallRequest),
    Uninstall(UninstallRequest),
    Progress(u8),
    Finished(Result<(), InstallFailure>),
    Finish { launch: bool },
}

#[derive(Debug)]
struct InstallFailure {
    exit_code: Option<i32>,
    message: String,
}

enum InstallerProcess {
    Direct(std::process::Child),
    Elevated(HANDLE),
}

impl InstallerProcess {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        match self {
            Self::Direct(child) => child
                .try_wait()
                .map(|status| status.map(|status| status.code().unwrap_or(1))),
            Self::Elevated(process) => {
                match unsafe { WaitForSingleObject(*process, 0) } {
                    WAIT_OBJECT_0 => {
                        let mut exit_code = 0;
                        unsafe {
                            GetExitCodeProcess(*process, &raw mut exit_code)
                        }
                        .map_err(windows_error)?;
                        let process = std::mem::take(process);
                        let _ = unsafe { CloseHandle(process) };
                        Ok(Some(exit_code as i32))
                    }
                    WAIT_TIMEOUT => Ok(None),
                    _ => Err(std::io::Error::last_os_error()),
                }
            }
        }
    }
}

impl Drop for InstallerProcess {
    fn drop(&mut self) {
        if let Self::Elevated(process) = self
            && !process.is_invalid()
        {
            let _ = unsafe { CloseHandle(*process) };
        }
    }
}

pub fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let mut event_loop =
        EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Axolotl Launcher")
        .with_inner_size(LogicalSize::new(940.0, 620.0))
        .with_min_inner_size(LogicalSize::new(940.0, 620.0))
        .with_max_inner_size(LogicalSize::new(940.0, 620.0))
        .with_resizable(false)
        .with_decorations(false)
        .with_theme(Some(Theme::Dark))
        .with_visible(false)
        .build(&event_loop)
        .map_err(|error| {
            format!("creating installer window failed: {error}")
        })?;

    if let Some(monitor) = window.current_monitor() {
        let screen = monitor.size().to_logical::<f64>(monitor.scale_factor());
        window.set_outer_position(LogicalPosition::new(
            (screen.width - 940.0).max(0.0) / 2.0,
            (screen.height - 620.0).max(0.0) / 2.0,
        ));
    }

    let bootstrap =
        serde_json::to_string(&arguments.bootstrap).map_err(|error| {
            format!("serializing installer settings failed: {error}")
        })?;
    let webview_data_directory = webview_data_directory()?;
    let mut webview_context =
        WebContext::new(Some(webview_data_directory.0.path().to_path_buf()));
    let proxy = event_loop.create_proxy();
    let ipc_proxy = proxy.clone();
    let handler = move |request: Request<String>| {
        if let Ok(command) = serde_json::from_str::<UiCommand>(request.body()) {
            let event = match command {
                UiCommand::Minimize => UserEvent::Minimize,
                UiCommand::DragWindow => UserEvent::DragWindow,
                UiCommand::Close => UserEvent::Close,
                UiCommand::Browse { target, current } => {
                    UserEvent::Browse { target, current }
                }
                UiCommand::Install(request) => UserEvent::Install(request),
                UiCommand::Uninstall(request) => UserEvent::Uninstall(request),
                UiCommand::Finish { launch } => UserEvent::Finish { launch },
            };
            let _ = ipc_proxy.send_event(event);
        }
    };

    let mut webview = Some(
        WebViewBuilder::new_with_web_context(&mut webview_context)
            .with_html(HTML)
            .with_initialization_script(format!(
                "window.__AXOLOTL_INSTALLER__ = {bootstrap};"
            ))
            .with_background_color((22, 24, 28, 255))
            .with_ipc_handler(handler)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_navigation_handler(|url| url.starts_with("data:"))
            .with_devtools(false)
            .build(&window)
            .map_err(|error| format!("starting WebView2 failed: {error}"))?,
    );

    let environment = webview
        .as_ref()
        .expect("WebView2 was just created")
        .environment()
        .cast::<ICoreWebView2Environment5>()
        .map_err(|error| {
            format!("getting WebView2 environment failed: {error}")
        })?;
    let browser_exited = Rc::new(Cell::new(false));
    let browser_exited_callback = browser_exited.clone();
    let browser_exit_proxy = proxy.clone();
    let mut browser_exit_token = 0;
    unsafe {
        environment.add_BrowserProcessExited(
            &BrowserProcessExitedEventHandler::create(Box::new(move |_, _| {
                browser_exited_callback.set(true);
                let _ = browser_exit_proxy
                    .send_event(UserEvent::BrowserProcessExited);
                Ok(())
            })),
            &raw mut browser_exit_token,
        )
    }
    .map_err(|error| format!("watching WebView2 shutdown failed: {error}"))?;

    window.set_visible(true);
    window.set_focus();

    let installer = arguments.installer;
    let fresh_install = arguments.bootstrap.fresh_install;
    let uninstall = arguments.bootstrap.uninstall;
    let mut installing = false;
    let mut install_dir = PathBuf::from(&arguments.bootstrap.install_dir);
    let mut launch_after_install = false;

    // Tao's run() exits the process without running the profile guard's drop.
    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::UserEvent(UserEvent::Close)
                if !installing =>
            {
                webview.take();
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Minimize) => window.set_minimized(true),
            Event::UserEvent(UserEvent::DragWindow) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(UserEvent::Browse { target, current }) => {
                let title = match target {
                    PathTarget::Install => {
                        "Select the program installation location"
                    }
                    PathTarget::Resource => "Select the application directory",
                };
                let mut dialog =
                    DialogBuilder::file().set_title(title).set_owner(&window);
                let current_path = PathBuf::from(current);
                if let Some(location) = dialog_initial_location(&current_path) {
                    dialog = dialog.set_location(&location);
                }
                if let Ok(Some(path)) = dialog.open_single_dir().show() {
                    send_to_webview(
                        webview.as_ref(),
                        json!({
                            "type": "pathSelected",
                            "target": target,
                            "path": path.to_string_lossy(),
                        }),
                    );
                }
            }
            Event::UserEvent(UserEvent::Install(request)) => {
                if installing || uninstall {
                    return;
                }
                match validate_request(&request, fresh_install) {
                    Ok(()) => {
                        install_dir = PathBuf::from(&request.install_dir);
                        launch_after_install = request.launch_after;
                        installing = true;
                        send_to_webview(
                            webview.as_ref(),
                            json!({ "type": "installStarted" }),
                        );
                        start_install(
                            installer.clone(),
                            request,
                            proxy.clone(),
                        );
                    }
                    Err((field, code)) => send_to_webview(
                        webview.as_ref(),
                        json!({
                            "type": "validationError",
                            "field": field,
                            "code": code,
                        }),
                    ),
                }
            }
            Event::UserEvent(UserEvent::Uninstall(request)) => {
                if installing || !uninstall {
                    return;
                }
                installing = true;
                send_to_webview(
                    webview.as_ref(),
                    json!({ "type": "installStarted" }),
                );
                start_uninstall(
                    installer.clone(),
                    install_dir.clone(),
                    request,
                    proxy.clone(),
                );
            }
            Event::UserEvent(UserEvent::Progress(progress)) => {
                send_to_webview(
                    webview.as_ref(),
                    json!({ "type": "progress", "value": progress }),
                );
            }
            Event::UserEvent(UserEvent::Finished(result)) => {
                installing = false;
                match result {
                    Ok(()) if launch_after_install => {
                        match launch_main_process(&install_dir) {
                            Ok(()) => {
                                webview.take();
                                *control_flow = ControlFlow::Exit;
                            }
                            Err(error) => send_to_webview(
                                webview.as_ref(),
                                json!({
                                    "type": "launchFailed",
                                    "message": error,
                                }),
                            ),
                        }
                    }
                    Ok(()) => send_to_webview(
                        webview.as_ref(),
                        json!({
                            "type": "installFinished",
                            "launchAfter": launch_after_install,
                        }),
                    ),
                    Err(error) => send_to_webview(
                        webview.as_ref(),
                        json!({
                            "type": "installFailed",
                            "exitCode": error.exit_code,
                            "message": error.message,
                        }),
                    ),
                }
            }
            Event::UserEvent(UserEvent::Finish { launch }) => {
                if installing {
                    return;
                }
                if launch
                    && !uninstall
                    && let Err(error) = launch_main_process(&install_dir)
                {
                    send_to_webview(
                        webview.as_ref(),
                        json!({
                            "type": "launchFailed",
                            "message": error,
                        }),
                    );
                    return;
                }
                webview.take();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });

    drop(webview_context);
    // The first loop has dropped the window and WebView. Keep dispatching COM
    // callbacks until the browser and its children release the profile files.
    let deadline = Instant::now() + WEBVIEW_SHUTDOWN_TIMEOUT;
    if !browser_exited.get() {
        event_loop.run_return(|_, _, control_flow| {
            *control_flow =
                if browser_exited.get() || Instant::now() >= deadline {
                    ControlFlow::Exit
                } else {
                    ControlFlow::WaitUntil(deadline)
                };
        });
    }
    let _ =
        unsafe { environment.remove_BrowserProcessExited(browser_exit_token) };
    drop(environment);

    Ok(())
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut args = env::args_os().skip(1);
    let mut installer = None;
    let mut version = None;
    let mut install_dir = None;
    let mut resource_dir = None;
    let mut fresh_install = true;
    let mut uninstall = false;
    let mut language = Language::En;

    while let Some(argument) = args.next() {
        let value = args.next().ok_or_else(|| {
            format!("missing value for {}", argument.to_string_lossy())
        })?;
        match argument.to_string_lossy().as_ref() {
            "--installer" => installer = Some(PathBuf::from(value)),
            "--uninstaller" => {
                installer = Some(PathBuf::from(value));
                uninstall = true;
            }
            "--version" => version = Some(value.to_string_lossy().into_owned()),
            "--install-dir" => {
                install_dir = Some(value.to_string_lossy().into_owned())
            }
            "--resource-dir" => {
                resource_dir = Some(value.to_string_lossy().into_owned())
            }
            "--fresh-install" => fresh_install = value != "0",
            "--language" => {
                language = if value == "2052" {
                    Language::ZhCn
                } else {
                    Language::En
                };
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }

    let installer =
        installer.ok_or_else(|| "missing --installer".to_string())?;
    if !installer.is_file() {
        return Err("installer executable does not exist".to_string());
    }
    if uninstall {
        validate_uninstall_directory(Path::new(
            install_dir.as_deref().unwrap_or_default(),
        ))
        .map_err(|error| error.message)?;
    }

    Ok(Arguments {
        installer,
        bootstrap: Bootstrap {
            version: version.unwrap_or_default(),
            install_dir: install_dir.unwrap_or_default(),
            resource_dir: resource_dir.unwrap_or_default(),
            fresh_install,
            language,
            logo_data_url: format!(
                "data:image/png;base64,{}",
                BASE64.encode(LOGO)
            ),
            uninstall,
        },
    })
}

struct WebViewDataDirectory(TempDir);

impl Drop for WebViewDataDirectory {
    fn drop(&mut self) {
        if let Err(error) = remove_webview_data_directory(
            self.0.path(),
            WEBVIEW_SHUTDOWN_TIMEOUT,
        ) {
            eprintln!(
                "removing temporary WebView2 data directory '{}' failed: {error}",
                self.0.path().display()
            );
        }
    }
}

fn webview_data_directory() -> Result<WebViewDataDirectory, String> {
    tempfile::Builder::new()
        .prefix("axolotl-installer-webview-")
        .tempdir()
        .map(WebViewDataDirectory)
        .map_err(|error| {
            format!(
                "creating temporary WebView2 data directory failed: {error}"
            )
        })
}

fn remove_webview_data_directory(
    directory: &Path,
    timeout: Duration,
) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match fs::remove_dir_all(directory) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => {
                // Startup failures can leave a browser releasing files without
                // an exit subscription. Retry briefly without killing it.
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn validate_request(
    request: &InstallRequest,
    fresh_install: bool,
) -> Result<(), (&'static str, &'static str)> {
    let install_dir = PathBuf::from(request.install_dir.trim());
    if !install_dir.is_absolute() || request.install_dir.contains('"') {
        return Err(("install", "absolutePath"));
    }

    if !fresh_install {
        return Ok(());
    }

    let resource_dir = PathBuf::from(request.resource_dir.trim());
    if !resource_dir.is_absolute() || request.resource_dir.contains('"') {
        return Err(("resource", "absolutePath"));
    }
    if resource_dir.parent().is_none() {
        return Err(("resource", "driveRoot"));
    }

    let install_key = normalized_path_key(&install_dir);
    let resource_key = normalized_path_key(&resource_dir);
    if resource_key == install_key
        || resource_key.starts_with(&format!("{install_key}\\"))
    {
        return Err(("resource", "insideInstall"));
    }

    if fs::create_dir_all(&resource_dir).is_err() {
        return Err(("resource", "notWritable"));
    }
    let write_test = resource_dir
        .join(format!(".axolotl-write-test-{}", std::process::id()));
    if fs::write(&write_test, b"Axolotl Launcher").is_err() {
        return Err(("resource", "notWritable"));
    }
    let _ = fs::remove_file(write_test);

    Ok(())
}

fn normalized_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_lowercase()
}

fn dialog_initial_location(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|candidate| candidate.is_dir())
        .map(Path::to_path_buf)
}

fn launch_main_process(install_dir: &Path) -> Result<(), String> {
    Command::new(install_dir.join(MAIN_BINARY_NAME))
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn start_install(
    installer: PathBuf,
    request: InstallRequest,
    proxy: EventLoopProxy<UserEvent>,
) {
    thread::spawn(move || {
        let status_path = env::temp_dir().join(format!(
            "axolotl-installer-{}-{}.status",
            std::process::id(),
            thread_id_suffix()
        ));
        let _ = fs::remove_file(&status_path);

        let result =
            match spawn_installer(&installer, &request, &status_path) {
                Ok(mut child) => {
                    wait_for_installer(&mut child, &status_path, &proxy)
                }
                Err(error) => Err(InstallFailure {
                    exit_code: None,
                    message: error.to_string(),
                }),
            }
            .and_then(|()| {
                crate::executable_install::check(
                    &Path::new(&request.install_dir).join(MAIN_BINARY_NAME),
                )
                .map_err(|message| InstallFailure {
                    exit_code: Some(0),
                    message,
                })
            });
        let _ = fs::remove_file(status_path);
        let _ = proxy.send_event(UserEvent::Finished(result));
    });
}

fn start_uninstall(
    uninstaller: PathBuf,
    install_dir: PathBuf,
    request: UninstallRequest,
    proxy: EventLoopProxy<UserEvent>,
) {
    thread::spawn(move || {
        let status_path = env::temp_dir().join(format!(
            "axolotl-uninstaller-{}-{}.status",
            std::process::id(),
            thread_id_suffix()
        ));
        let _ = fs::remove_file(&status_path);
        let result = spawn_uninstaller(
            &uninstaller,
            &install_dir,
            &request,
            &status_path,
        )
        .and_then(|mut process| {
            match wait_for_installer(&mut process, &status_path, &proxy) {
                Ok(()) => uninstall_result(0, &status_path, &install_dir),
                Err(error) => match error.exit_code {
                    Some(code) => {
                        uninstall_result(code, &status_path, &install_dir)
                    }
                    None => Err(error),
                },
            }
        });
        let _ = fs::remove_file(status_path);
        let _ = proxy.send_event(UserEvent::Finished(result));
    });
}

fn spawn_uninstaller(
    uninstaller: &Path,
    install_dir: &Path,
    request: &UninstallRequest,
    status_path: &Path,
) -> Result<InstallerProcess, InstallFailure> {
    let args = uninstaller_arguments(install_dir, request, status_path)?;
    let result = if install_dir_requires_elevation(install_dir) {
        elevated_installer_process(uninstaller, &args)
            .map(InstallerProcess::Elevated)
    } else {
        let mut command = Command::new(uninstaller);
        for argument in args {
            command.raw_arg(argument);
        }
        command.spawn().map(InstallerProcess::Direct)
    };
    result.map_err(|error| InstallFailure {
        exit_code: None,
        message: error.to_string(),
    })
}

fn validate_uninstall_directory(
    install_dir: &Path,
) -> Result<(), InstallFailure> {
    if !install_dir.is_absolute()
        || install_dir.parent().is_none()
        || install_dir
            .to_string_lossy()
            .contains(['"', '\r', '\n', '\0'])
    {
        return Err(InstallFailure {
            exit_code: None,
            message:
                "The uninstaller did not receive a valid installation directory"
                    .to_string(),
        });
    }
    Ok(())
}

fn uninstaller_arguments(
    install_dir: &Path,
    request: &UninstallRequest,
    status_path: &Path,
) -> Result<Vec<String>, InstallFailure> {
    validate_uninstall_directory(install_dir)?;
    let mut args = vec![
        "/S".to_string(),
        "/UI_CHILD".to_string(),
        nsis_value_option("STATUS_FILE", &status_path.to_string_lossy()),
    ];
    if request.delete_app_data {
        args.push("/DELETE_APP_DATA".to_string());
    }
    // NSIS consumes the entire remaining command line as this unquoted path.
    // It also disables the copy-and-relaunch step, so we wait for the actual core.
    args.push(format!("_?={}", install_dir.display()));
    Ok(args)
}

fn uninstall_result(
    exit_code: i32,
    status_path: &Path,
    install_dir: &Path,
) -> Result<(), InstallFailure> {
    let status = read_installer_status(status_path).unwrap_or_default();
    if exit_code != 0 || status.trim() != "100" {
        return Err(InstallFailure {
            exit_code: Some(exit_code),
            message: if let Some(message) = status.trim().strip_prefix("error:")
            {
                message.to_string()
            } else {
                "The uninstallation core did not confirm completion".to_string()
            },
        });
    }
    for filename in [MAIN_BINARY_NAME, "uninstall.exe"] {
        let path = install_dir.join(filename);
        match path.try_exists() {
            Ok(false) => {}
            Ok(true) => {
                return Err(InstallFailure {
                    exit_code: Some(exit_code),
                    message: format!(
                        "Uninstallation left '{}' on disk",
                        path.display()
                    ),
                });
            }
            Err(error) => {
                return Err(InstallFailure {
                    exit_code: Some(exit_code),
                    message: format!(
                        "Could not verify '{}': {error}",
                        path.display()
                    ),
                });
            }
        }
    }
    Ok(())
}

fn spawn_installer(
    installer: &Path,
    request: &InstallRequest,
    status_path: &Path,
) -> std::io::Result<InstallerProcess> {
    let arguments = installer_arguments(request, status_path);
    if install_dir_requires_elevation(Path::new(request.install_dir.trim())) {
        return elevated_installer_process(installer, &arguments)
            .map(InstallerProcess::Elevated);
    }

    let mut command = Command::new(installer);
    for argument in arguments {
        command.raw_arg(argument);
    }
    command.spawn().map(InstallerProcess::Direct)
}

fn installer_arguments(
    request: &InstallRequest,
    status_path: &Path,
) -> Vec<String> {
    let mut arguments = vec![
        "/S".to_string(),
        nsis_value_option("INSTALL_DIR", request.install_dir.trim()),
        nsis_value_option("RESOURCE_DIR", request.resource_dir.trim()),
        nsis_value_option("STATUS_FILE", &status_path.to_string_lossy()),
        "/NO_RUN_AFTER_INSTALL".to_string(),
    ];
    if !request.desktop_shortcut {
        arguments.push("/NO_DESKTOP_SHORTCUT".to_string());
    }
    arguments
}

fn install_dir_requires_elevation(install_dir: &Path) -> bool {
    let Some(existing_directory) =
        install_dir.ancestors().find(|candidate| candidate.is_dir())
    else {
        return true;
    };

    let write_test = existing_directory.join(format!(
        ".axolotl-install-write-test-{}-{}",
        std::process::id(),
        thread_id_suffix(),
    ));
    let can_write = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&write_test)
        .and_then(|mut file| file.write_all(b"Axolotl Launcher"))
        .is_ok();
    let removed = fs::remove_file(&write_test).is_ok();

    !(can_write && removed)
}

fn elevated_installer_process(
    installer: &Path,
    arguments: &[String],
) -> std::io::Result<HANDLE> {
    let verb = wide_null("runas");
    let installer = wide_null(installer.as_os_str());
    let arguments = wide_null(arguments.join(" "));
    let mut execute_info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(installer.as_ptr()),
        lpParameters: PCWSTR(arguments.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    unsafe { ShellExecuteExW(&raw mut execute_info) }.map_err(windows_error)?;
    if execute_info.hProcess.is_invalid() {
        return Err(std::io::Error::other(
            "elevated installer did not return a process handle",
        ));
    }

    Ok(execute_info.hProcess)
}

fn wide_null(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.as_ref().encode_wide().chain(Some(0)).collect()
}

fn windows_error(error: windows::core::Error) -> std::io::Error {
    const FACILITY_WIN32: i32 = 7;

    let hr = error.code();
    let raw = hr.0;

    // If this is a Win32 error (FACILITY_WIN32), extract the underlying Win32 error code
    let facility = (raw >> 16) & 0x1fff;
    if facility == FACILITY_WIN32 {
        let win32_code = raw & 0xFFFF;
        std::io::Error::from_raw_os_error(win32_code)
    } else {
        // Fall back to using the HRESULT value as a raw OS error code when representable
        std::io::Error::from_raw_os_error(raw)
    }
}

fn nsis_value_option(name: &str, value: &str) -> String {
    format!(r#"/{name}="{value}""#)
}

fn read_installer_status(status_path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(status_path)?;
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        if bytes.len() % 2 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Incomplete uninstaller status",
            ));
        }
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&words).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
        })
    } else {
        String::from_utf8(bytes).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
        })
    }
}

fn wait_for_installer(
    process: &mut InstallerProcess,
    status_path: &Path,
    proxy: &EventLoopProxy<UserEvent>,
) -> Result<(), InstallFailure> {
    let mut last_progress = 0;
    loop {
        if let Ok(value) = read_installer_status(status_path)
            && let Ok(progress) = value.trim().parse::<u8>()
            && progress != last_progress
        {
            last_progress = progress;
            let _ = proxy.send_event(UserEvent::Progress(progress.min(99)));
        }

        match process.try_wait() {
            Ok(Some(exit_code)) => return installer_result(exit_code),
            Ok(None) => thread::sleep(Duration::from_millis(120)),
            Err(error) => {
                return Err(InstallFailure {
                    exit_code: None,
                    message: error.to_string(),
                });
            }
        }
    }
}

fn installer_result(exit_code: i32) -> Result<(), InstallFailure> {
    if exit_code == 0 {
        Ok(())
    } else {
        Err(InstallFailure {
            exit_code: Some(exit_code),
            message: "The NSIS installation core returned an error".to_string(),
        })
    }
}

fn thread_id_suffix() -> String {
    format!("{:?}", thread::current().id())
        .replace("ThreadId(", "")
        .replace(')', "")
}

fn send_to_webview(webview: Option<&WebView>, payload: serde_json::Value) {
    let Some(webview) = webview else {
        return;
    };
    if let Ok(payload) = serde_json::to_string(&payload) {
        let _ = webview.evaluate_script(&format!(
            "window.axolotlInstaller && window.axolotlInstaller.receive({payload});"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAIN_BINARY_NAME, UiCommand, UninstallRequest, dialog_initial_location,
        install_dir_requires_elevation, launch_main_process, nsis_value_option,
        remove_webview_data_directory, uninstall_result, uninstaller_arguments,
        webview_data_directory, wide_null,
    };
    use std::{
        fs,
        os::windows::fs::OpenOptionsExt,
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn dialog_location_keeps_existing_directory() {
        let existing = std::env::temp_dir();

        assert_eq!(dialog_initial_location(&existing), Some(existing));
    }

    #[test]
    fn dialog_location_falls_back_to_existing_parent() {
        let existing = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let missing = existing
            .join(format!("axolotl-installer-ui-{unique}"))
            .join("nested");
        assert!(!missing.exists());

        assert_eq!(dialog_initial_location(&missing), Some(existing));
    }

    #[test]
    fn dialog_location_rejects_relative_path_without_existing_ancestor() {
        assert_eq!(dialog_initial_location(&PathBuf::from("")), None);
    }

    #[test]
    fn install_request_includes_launch_after_choice() {
        let command = serde_json::from_str::<UiCommand>(
            r#"{"command":"install","installDir":"C:\\Axolotl","resourceDir":"C:\\AxolotlData","desktopShortcut":true,"launchAfter":true}"#,
        )
        .expect("install request should deserialize");
        let UiCommand::Install(request) = command else {
            panic!("expected install command");
        };

        assert!(request.launch_after);
    }

    #[test]
    fn nsis_option_keeps_name_outside_quoted_value() {
        assert_eq!(
            nsis_value_option(
                "INSTALL_DIR",
                r"C:\Program Files\Axolotl Launcher",
            ),
            r#"/INSTALL_DIR="C:\Program Files\Axolotl Launcher""#,
        );
    }

    #[test]
    fn uninstall_directory_is_the_last_unquoted_argument() {
        for directory in [
            r"C:\Program Files\Axolotl Launcher",
            "D:\\游戏\\Axolotl Launcher\\",
        ] {
            for delete_app_data in [false, true] {
                let args = uninstaller_arguments(
                    &PathBuf::from(directory),
                    &UninstallRequest { delete_app_data },
                    &PathBuf::from(r"C:\Temp Folder\uninstall.status"),
                )
                .unwrap();
                assert_eq!(args.last().unwrap(), &format!("_?={directory}"));
                assert!(args.contains(&"/UI_CHILD".to_string()));
                assert_eq!(
                    args.contains(&"/DELETE_APP_DATA".to_string()),
                    delete_app_data
                );
            }
        }
    }

    #[test]
    fn uninstall_rejects_missing_relative_root_and_quoted_directories() {
        for directory in ["", "relative", r"C:\", "C:\\bad\"path"] {
            assert!(
                uninstaller_arguments(
                    &PathBuf::from(directory),
                    &UninstallRequest {
                        delete_app_data: false
                    },
                    &PathBuf::from(r"C:\Temp\uninstall.status"),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn uninstall_success_requires_completion_and_removed_executables() {
        let directory = std::env::temp_dir()
            .join(format!("axolotl-uninstall-result-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let status = directory.join("uninstall.status");
        assert!(uninstall_result(0, &status, &directory).is_err());
        fs::write(&status, "82").unwrap();
        assert!(uninstall_result(0, &status, &directory).is_err());
        fs::write(&status, "error:Could not remove a locked file").unwrap();
        assert!(
            uninstall_result(2, &status, &directory)
                .unwrap_err()
                .message
                .contains("locked file")
        );
        fs::write(&status, "100").unwrap();
        assert!(uninstall_result(2, &status, &directory).is_err());
        for filename in [MAIN_BINARY_NAME, "uninstall.exe"] {
            let file = directory.join(filename);
            fs::write(&file, "still installed").unwrap();
            assert!(uninstall_result(0, &status, &directory).is_err());
            fs::remove_file(file).unwrap();
        }
        assert!(uninstall_result(0, &status, &directory).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn uninstall_preserves_unicode_failure_paths() {
        let directory = std::env::temp_dir()
            .join(format!("axolotl-uninstall-unicode-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let status = directory.join("uninstall.status");
        let message = r"error:Could not remove: D:\游戏\启动器.exe";
        let bytes: Vec<u8> = [0xfeff_u16]
            .into_iter()
            .chain(message.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        fs::write(&status, bytes).unwrap();
        assert_eq!(
            uninstall_result(2, &status, &directory)
                .unwrap_err()
                .message,
            &message[6..]
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn uac_cancellation_and_access_denied_keep_the_windows_error_codes() {
        for code in [1223, 5] {
            let error = windows::core::Error::from_hresult(
                windows::core::HRESULT::from_win32(code),
            );
            assert_eq!(
                super::windows_error(error).raw_os_error(),
                Some(code as i32)
            );
        }
    }

    #[test]
    fn writable_install_directory_does_not_require_elevation() {
        let directory = std::env::temp_dir().join(format!(
            "axolotl-installer-ui-elevation-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory)
            .expect("test directory should be created");

        assert!(!install_dir_requires_elevation(
            &directory.join("Axolotl Launcher")
        ));

        fs::remove_dir_all(directory)
            .expect("test directory should be removed");
    }

    #[test]
    fn wide_strings_preserve_windows_paths_and_end_with_null() {
        let value = wide_null(r"C:\Downloads\Axolotl Launcher Setup.exe");

        assert_eq!(value.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&value[..value.len() - 1])
                .expect("test path should be valid UTF-16"),
            r"C:\Downloads\Axolotl Launcher Setup.exe"
        );
    }

    #[test]
    fn launching_from_missing_install_directory_reports_error() {
        let missing = std::env::temp_dir().join(format!(
            "axolotl-installer-ui-missing-{}",
            std::process::id()
        ));
        assert!(!missing.exists());

        assert!(launch_main_process(&missing).is_err());
    }

    #[test]
    fn webview_data_directory_is_unique_and_removed_with_its_contents() {
        let first = webview_data_directory()
            .expect("WebView2 data directory should be created");
        let second = webview_data_directory().unwrap();
        let first_path = first.0.path().to_path_buf();
        let second_path = second.0.path().to_path_buf();
        assert_ne!(first_path, second_path);
        assert_eq!(first_path.parent(), Some(std::env::temp_dir().as_path()));
        let nested = first_path.join("EBWebView/Default");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Preferences"), b"temporary profile").unwrap();

        drop(first);
        assert!(!first_path.exists());
        assert!(second_path.is_dir());
        drop(second);
        assert!(!second_path.exists());
    }

    #[test]
    fn webview_data_directory_cleanup_retries_until_files_are_released() {
        let directory = webview_data_directory().unwrap();
        let path = directory.0.path().to_path_buf();
        let locked_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(path.join("locked-profile"))
            .unwrap();
        assert!(remove_webview_data_directory(&path, Duration::ZERO).is_err());

        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(locked_file);
        });
        drop(directory);
        release.join().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn webview_data_directory_is_removed_after_startup_failure() {
        let mut path = PathBuf::new();
        let result: Result<(), &str> = (|| {
            let directory = webview_data_directory().unwrap();
            path = directory.0.path().to_path_buf();
            fs::write(path.join("partial-profile"), b"startup data").unwrap();
            Err("WebView2 startup failed")
        })();

        assert!(result.is_err());
        assert!(!path.exists());
    }
}
