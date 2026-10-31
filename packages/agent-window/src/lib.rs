use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry,
};
use tokio::sync::{Mutex, RwLock, watch};

pub const WINDOW_LABEL: &str = "agent-window";
pub const URI_SCHEME: &str = "axolotl-agent";

const WINDOW_URL: &str = "axolotl-agent://localhost/";
const HTML: &str = include_str!("index.html");
const CSS: &str = include_str!("window.css");
const JS: &str = include_str!("window.js");
const APP_JS: &str = include_str!("app.js");
const STATE_JS: &str = include_str!("state.js");
const MESSAGES_JS: &str = include_str!("messages.js");
const ICON: &[u8] = include_bytes!("../../assets/branding/axolotl.png");

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WindowLabels {
    pub title: String,
    pub minimize: String,
    pub maximize: String,
    pub restore: String,
    pub close: String,
}

impl Default for WindowLabels {
    fn default() -> Self {
        Self {
            title: "Axolotl Agent Window".into(),
            minimize: "Minimize".into(),
            maximize: "Maximize".into(),
            restore: "Restore".into(),
            close: "Close".into(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Appearance {
    pub locale: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
    pub scrollbar: String,
    pub scrollbar_track: String,
    pub scrollbar_radius: String,
    pub labels: WindowLabels,
    pub messages: BTreeMap<String, String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            background: "#ffffff".into(),
            surface: "#f4f4f5".into(),
            text: "#1f2937".into(),
            accent: "#1bd96a".into(),
            scrollbar: "#96a2b0".into(),
            scrollbar_track: "#ffffff".into(),
            scrollbar_radius: "16px".into(),
            labels: WindowLabels::default(),
            messages: BTreeMap::new(),
        }
    }
}

/// Preferences for future internal execution; they do not grant MCP control.
/// Shared by windows for this launcher process only; a restart resets them.
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Permissions {
    pub full_access: bool,
    pub computer_use: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowPhase {
    Open,
    Closing,
    Destroying,
    Destroyed,
}

struct WindowState(watch::Sender<WindowPhase>);

impl tauri::Resource for WindowState {}

fn window_state(
    resources: &tauri::ResourceTable,
) -> tauri::Result<Arc<WindowState>> {
    resources
        .names()
        .find_map(|(id, _)| resources.get::<WindowState>(id).ok())
        .ok_or_else(|| {
            tauri::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "The calling webview has no Agent Window session",
            ))
        })
}

impl WindowState {
    fn new() -> Self {
        Self(watch::channel(WindowPhase::Open).0)
    }

    fn phase(&self) -> WindowPhase {
        *self.0.borrow()
    }

    fn request_close(&self) -> bool {
        self.0.send_if_modified(|phase| {
            if *phase == WindowPhase::Open {
                *phase = WindowPhase::Closing;
                true
            } else {
                false
            }
        })
    }

    fn destroyed(&self) {
        self.0.send_replace(WindowPhase::Destroyed);
    }

    fn begin_destroy(&self) -> bool {
        self.0.send_if_modified(|phase| {
            if matches!(*phase, WindowPhase::Open | WindowPhase::Closing) {
                *phase = WindowPhase::Destroying;
                true
            } else {
                false
            }
        })
    }

    fn destroy_failed(&self) {
        self.0.send_if_modified(|phase| {
            if *phase == WindowPhase::Destroying {
                *phase = WindowPhase::Open;
                true
            } else {
                false
            }
        });
    }
}

trait WindowOperations: Clone {
    fn reveal(&self) -> tauri::Result<()>;
    fn destroy(&self) -> tauri::Result<()>;
}

impl WindowOperations for WebviewWindow<Wry> {
    fn reveal(&self) -> tauri::Result<()> {
        self.eval(
            "window.dispatchEvent(new Event('agent-appearance-changed'))",
        )?;
        self.unminimize()?;
        self.show()?;
        self.set_focus()
    }

    fn destroy(&self) -> tauri::Result<()> {
        WebviewWindow::destroy(self)
    }
}

#[derive(Clone)]
struct WindowSession<W> {
    window: W,
    state: Arc<WindowState>,
}

struct WindowLifecycle<W> {
    operation: Mutex<()>,
    // Event callbacks can identify and mark their own session without blocking
    // the UI thread on an asynchronous open/close operation.
    current: std::sync::Mutex<Option<WindowSession<W>>>,
    close_timeout: Duration,
}

impl<W> Default for WindowLifecycle<W> {
    fn default() -> Self {
        Self {
            operation: Mutex::new(()),
            current: std::sync::Mutex::new(None),
            close_timeout: Duration::from_secs(5),
        }
    }
}

impl<W: WindowOperations> WindowLifecycle<W> {
    fn current(&self) -> Option<WindowSession<W>> {
        self.current.lock().unwrap().clone()
    }

    fn forget(&self, state: &Arc<WindowState>) {
        let mut current = self.current.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|session| Arc::ptr_eq(&session.state, state))
        {
            *current = None;
        }
    }

    async fn finish_close(
        &self,
        session: &WindowSession<W>,
    ) -> tauri::Result<()> {
        if session.state.phase() == WindowPhase::Destroyed {
            self.forget(&session.state);
            return Ok(());
        }
        if session.state.begin_destroy() {
            if let Err(error) = session.window.destroy() {
                session.state.destroy_failed();
                return Err(error);
            }
        }
        // destroy() only enqueues a native event. Keep the lifecycle locked
        // until Tauri has removed the old label before another open can run.
        // Cancellation/timeout preserves Destroying so later opens resume this
        // wait instead of reusing a dying window or creating a duplicate label.
        let mut phase = session.state.0.subscribe();
        tokio::time::timeout(
            self.close_timeout,
            phase.wait_for(|phase| *phase == WindowPhase::Destroyed),
        )
        .await
        .map_err(|_| {
            tauri::Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Agent Window did not finish closing. Please retry opening it.",
            ))
        })?
        .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
        self.forget(&session.state);
        Ok(())
    }

    async fn open(
        &self,
        create: impl FnOnce(Arc<WindowState>) -> tauri::Result<W>,
    ) -> tauri::Result<()> {
        let _operation = self.operation.lock().await;
        if let Some(session) = self.current() {
            if session.state.phase() == WindowPhase::Open {
                return session.window.reveal();
            }
            self.finish_close(&session).await?;
        }
        let state = Arc::new(WindowState::new());
        let window = create(state.clone())?;
        *self.current.lock().unwrap() = Some(WindowSession {
            window: window.clone(),
            state,
        });
        // Show/focus errors must reach the launcher caller, not only the
        // independent window's script (which could still be hidden).
        window.reveal()
    }

    async fn close(&self, target: Arc<WindowState>) -> tauri::Result<()> {
        target.request_close();
        let _operation = self.operation.lock().await;
        if let Some(session) = self.current() {
            if Arc::ptr_eq(&session.state, &target) {
                self.finish_close(&session).await?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    async fn close_current(&self) -> tauri::Result<()> {
        if let Some(session) = self.current() {
            self.close(session.state).await?;
        }
        Ok(())
    }
}

/// Owns window creation independently of the launcher's frontend and runtime.
#[derive(Default)]
pub struct AgentWindow {
    lifecycle: WindowLifecycle<WebviewWindow<Wry>>,
    appearance: RwLock<Appearance>,
    permissions: RwLock<Permissions>,
}

impl AgentWindow {
    pub async fn open(&self, app: &AppHandle<Wry>) -> tauri::Result<()> {
        self.open_with_appearance(app, None).await
    }

    pub async fn appearance(&self) -> Appearance {
        self.appearance.read().await.clone()
    }

    pub async fn permissions(&self) -> Permissions {
        *self.permissions.read().await
    }

    pub async fn close(
        &self,
        window: &WebviewWindow<Wry>,
    ) -> tauri::Result<()> {
        // The label can be reused by a replacement while an older IPC task is
        // still pending. Resolve its target from the calling webview itself.
        let state = window_state(&window.resources_table())?;
        self.lifecycle.close(state).await
    }

    pub async fn set_permissions(
        &self,
        mut permissions: Permissions,
    ) -> Permissions {
        permissions.computer_use &= permissions.full_access;
        *self.permissions.write().await = permissions;
        permissions
    }

    pub async fn update_appearance(
        &self,
        app: &AppHandle<Wry>,
        appearance: Appearance,
    ) -> tauri::Result<()> {
        *self.appearance.write().await = appearance;
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window.eval(
                "window.dispatchEvent(new Event('agent-appearance-changed'))",
            )?;
        }
        Ok(())
    }

    pub async fn open_with_appearance(
        &self,
        app: &AppHandle<Wry>,
        appearance: Option<Appearance>,
    ) -> tauri::Result<()> {
        if let Some(appearance) = appearance {
            *self.appearance.write().await = appearance;
        }
        self.lifecycle
            .open(|state| {
                let window = WebviewWindowBuilder::new(
                    app,
                    WINDOW_LABEL,
                    WebviewUrl::CustomProtocol(
                        WINDOW_URL.parse().expect("valid Agent Window URL"),
                    ),
                )
                .title("Axolotl Agent Window")
                .inner_size(1100.0, 760.0)
                .min_inner_size(640.0, 480.0)
                .decorations(false)
                .visible(false)
                .resizable(true)
                .center()
                .disable_drag_drop_handler()
                .on_navigation(is_window_url)
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
                window.resources_table().add_arc(state.clone());
                let app = app.clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if state.request_close() {
                            let app = app.clone();
                            let state = state.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(error) = app
                                    .state::<AgentWindow>()
                                    .lifecycle
                                    .close(state)
                                    .await
                                {
                                    eprintln!(
                                        "Could not close Agent Window: {error}"
                                    );
                                }
                            });
                        }
                    }
                    tauri::WindowEvent::Destroyed => state.destroyed(),
                    _ => {}
                });
                Ok(window)
            })
            .await
    }
}

fn is_window_url(url: &tauri::Url) -> bool {
    let local_origin = (url.scheme() == URI_SCHEME
        && url.host_str() == Some("localhost"))
        || (url.scheme() == "http"
            && url.host_str() == Some("axolotl-agent.localhost"));
    local_origin
        && matches!(url.path(), "/" | "/index.html")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

/// Serves only the embedded entry document; no filesystem or network access.
pub fn asset_response(
    webview_label: &str,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Method, StatusCode, header};

    let (status, content_type, body) = if webview_label != WINDOW_LABEL {
        (StatusCode::FORBIDDEN, "text/plain", Vec::new())
    } else if request.method() != Method::GET {
        (StatusCode::METHOD_NOT_ALLOWED, "text/plain", Vec::new())
    } else {
        match request.uri().path() {
            "/" | "/index.html" => {
                (StatusCode::OK, "text/html", HTML.as_bytes().to_vec())
            }
            "/window.css" => {
                (StatusCode::OK, "text/css", CSS.as_bytes().to_vec())
            }
            "/window.js" => {
                (StatusCode::OK, "text/javascript", JS.as_bytes().to_vec())
            }
            "/app.js" => (
                StatusCode::OK,
                "text/javascript",
                APP_JS.as_bytes().to_vec(),
            ),
            "/state.js" => (
                StatusCode::OK,
                "text/javascript",
                STATE_JS.as_bytes().to_vec(),
            ),
            "/messages.js" => (
                StatusCode::OK,
                "text/javascript",
                MESSAGES_JS.as_bytes().to_vec(),
            ),
            "/axolotl.png" => (StatusCode::OK, "image/png", ICON.to_vec()),
            _ => (StatusCode::NOT_FOUND, "text/plain", Vec::new()),
        }
    };

    tauri::http::Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, format!("{content_type}; charset=utf-8"))
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; connect-src ipc: http://ipc.localhost; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        )
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("valid Agent Window response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        task::Poll,
    };
    use tauri::http::{Request, StatusCode};

    #[derive(Clone)]
    struct FakeWindow(Arc<FakeWindowInner>);

    struct FakeWindowInner {
        state: Arc<WindowState>,
        destroys: AtomicUsize,
        reveals: AtomicUsize,
        fail_destroy: AtomicBool,
        fail_reveal: AtomicBool,
    }

    impl FakeWindow {
        fn new(state: Arc<WindowState>) -> Self {
            Self(Arc::new(FakeWindowInner {
                state,
                destroys: AtomicUsize::new(0),
                reveals: AtomicUsize::new(0),
                fail_destroy: AtomicBool::new(false),
                fail_reveal: AtomicBool::new(false),
            }))
        }
    }

    impl WindowOperations for FakeWindow {
        fn reveal(&self) -> tauri::Result<()> {
            self.0.reveals.fetch_add(1, Ordering::SeqCst);
            if self.0.fail_reveal.load(Ordering::SeqCst) {
                return Err(tauri::Error::Io(std::io::Error::other(
                    "show failed",
                )));
            }
            Ok(())
        }

        fn destroy(&self) -> tauri::Result<()> {
            self.0.destroys.fetch_add(1, Ordering::SeqCst);
            if self.0.fail_destroy.load(Ordering::SeqCst) {
                return Err(tauri::Error::Io(std::io::Error::other(
                    "destroy failed",
                )));
            }
            Ok(())
        }
    }

    async fn pending<F: Future>(mut future: Pin<&mut F>) -> bool {
        std::future::poll_fn(|cx| {
            Poll::Ready(future.as_mut().poll(cx).is_pending())
        })
        .await
    }

    async fn open_fake(lifecycle: &WindowLifecycle<FakeWindow>) -> FakeWindow {
        lifecycle
            .open(|state| Ok(FakeWindow::new(state)))
            .await
            .unwrap();
        lifecycle.current().unwrap().window
    }

    #[test]
    fn destroyed_event_cannot_be_overwritten_by_a_late_close_or_failure() {
        let state = WindowState::new();
        assert!(state.begin_destroy());
        state.destroyed();
        state.destroy_failed();
        assert!(!state.request_close());
        assert!(!state.begin_destroy());
        assert!(state.phase() == WindowPhase::Destroyed);
    }

    #[test]
    fn open_waits_for_close_to_finish_before_creating_a_new_window() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            let old = open_fake(&lifecycle).await;
            let mut close = std::pin::pin!(lifecycle.close_current());
            assert!(pending(close.as_mut()).await);
            assert_eq!(old.0.destroys.load(Ordering::SeqCst), 1);
            let mut open = std::pin::pin!(open_fake(&lifecycle));
            assert!(pending(open.as_mut()).await);
            assert_eq!(old.0.reveals.load(Ordering::SeqCst), 1);
            old.0.state.destroyed();
            close.await.unwrap();
            let new = open.await;
            assert!(!Arc::ptr_eq(&old.0, &new.0));
            assert_eq!(new.0.reveals.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn system_close_is_completed_by_open_and_stale_close_cannot_close_its_replacement()
     {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            let old = open_fake(&lifecycle).await;
            assert!(old.0.state.request_close());
            let mut open = std::pin::pin!(open_fake(&lifecycle));
            assert!(pending(open.as_mut()).await);
            assert_eq!(old.0.destroys.load(Ordering::SeqCst), 1);
            old.0.state.destroyed();
            let new = open.await;
            lifecycle.close(old.0.state.clone()).await.unwrap();
            assert_eq!(new.0.destroys.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn delayed_ipc_close_uses_its_webviews_resources_not_the_replacement() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            let old = open_fake(&lifecycle).await;
            let mut old_resources = tauri::ResourceTable::default();
            assert!(window_state(&old_resources).is_err());
            old_resources.add_arc(old.0.state.clone());

            let mut close = std::pin::pin!(
                lifecycle.close(window_state(&old_resources).unwrap())
            );
            assert!(pending(close.as_mut()).await);
            old.0.state.destroyed();
            close.await.unwrap();

            let new = open_fake(&lifecycle).await;
            let mut new_resources = tauri::ResourceTable::default();
            new_resources.add_arc(new.0.state.clone());
            lifecycle
                .close(window_state(&old_resources).unwrap())
                .await
                .unwrap();
            assert_eq!(new.0.destroys.load(Ordering::SeqCst), 0);

            let mut close = std::pin::pin!(
                lifecycle.close(window_state(&new_resources).unwrap())
            );
            assert!(pending(close.as_mut()).await);
            assert_eq!(new.0.destroys.load(Ordering::SeqCst), 1);
            new.0.state.destroyed();
            close.await.unwrap();
            assert!(lifecycle.current().is_none());
        });
    }

    #[test]
    fn cancelling_close_keeps_the_pending_destruction_visible_to_open() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            let old = open_fake(&lifecycle).await;
            {
                let mut close = std::pin::pin!(lifecycle.close_current());
                assert!(pending(close.as_mut()).await);
            }
            let mut open = std::pin::pin!(open_fake(&lifecycle));
            assert!(pending(open.as_mut()).await);
            assert_eq!(old.0.destroys.load(Ordering::SeqCst), 1);
            old.0.state.destroyed();
            let new = open.await;
            assert!(!Arc::ptr_eq(&old.0, &new.0));
        });
    }

    #[test]
    fn failed_destroy_releases_the_lifecycle_for_retry() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            let old = open_fake(&lifecycle).await;
            old.0.fail_destroy.store(true, Ordering::SeqCst);
            assert!(lifecycle.close_current().await.is_err());
            let reopened = open_fake(&lifecycle).await;
            assert!(Arc::ptr_eq(&old.0, &reopened.0));
            old.0.fail_destroy.store(false, Ordering::SeqCst);
            let mut close = std::pin::pin!(lifecycle.close_current());
            assert!(pending(close.as_mut()).await);
            old.0.state.destroyed();
            close.await.unwrap();
        });
    }

    #[test]
    fn timed_out_close_can_recover_after_the_destroyed_event_arrives() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle {
                close_timeout: Duration::ZERO,
                ..WindowLifecycle::default()
            };
            let old = open_fake(&lifecycle).await;
            assert!(lifecycle.close_current().await.is_err());
            assert!(
                lifecycle
                    .open(|_| panic!("must not reuse the old label"))
                    .await
                    .is_err()
            );
            old.0.state.destroyed();
            let new = open_fake(&lifecycle).await;
            assert!(!Arc::ptr_eq(&old.0, &new.0));
        });
    }

    #[test]
    fn initial_show_failure_reaches_the_caller_and_can_be_retried() {
        tauri::async_runtime::block_on(async {
            let lifecycle = WindowLifecycle::default();
            assert!(
                lifecycle
                    .open(|state| {
                        let window = FakeWindow::new(state);
                        window.0.fail_reveal.store(true, Ordering::SeqCst);
                        Ok(window)
                    })
                    .await
                    .is_err()
            );
            let original = lifecycle.current().unwrap().window;
            original.0.fail_reveal.store(false, Ordering::SeqCst);
            let retried = open_fake(&lifecycle).await;
            assert!(Arc::ptr_eq(&original.0, &retried.0));
            assert_eq!(retried.0.reveals.load(Ordering::SeqCst), 2);
        });
    }

    #[test]
    fn interface_permission_requires_full_access() {
        tauri::async_runtime::block_on(async {
            let agent = AgentWindow::default();
            assert!(!agent.permissions().await.full_access);
            let permissions = agent
                .set_permissions(Permissions {
                    full_access: false,
                    computer_use: true,
                })
                .await;
            assert!(!permissions.computer_use);
            let permissions = agent
                .set_permissions(Permissions {
                    full_access: true,
                    computer_use: true,
                })
                .await;
            assert!(permissions.computer_use);
            let permissions =
                agent.set_permissions(Permissions::default()).await;
            assert!(!permissions.full_access && !permissions.computer_use);
        });
    }

    #[test]
    fn permissions_reset_with_launcher_state() {
        tauri::async_runtime::block_on(async {
            let agent = AgentWindow::default();
            agent
                .set_permissions(Permissions {
                    full_access: true,
                    computer_use: true,
                })
                .await;
            assert!(agent.permissions().await.computer_use);
            let restarted = AgentWindow::default();
            assert!(!restarted.permissions().await.full_access);
            assert!(!restarted.permissions().await.computer_use);
        });
    }

    #[test]
    fn embedded_entry_is_only_served_to_the_agent_window() {
        let request =
            Request::builder().uri(WINDOW_URL).body(Vec::new()).unwrap();
        let response = asset_response(WINDOW_LABEL, &request);
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().ends_with(b"</html>\n"));
        assert_eq!(
            asset_response("main", &request).status(),
            StatusCode::FORBIDDEN
        );
        for (path, mime) in [
            ("/window.css", "text/css; charset=utf-8"),
            ("/window.js", "text/javascript; charset=utf-8"),
            ("/app.js", "text/javascript; charset=utf-8"),
            ("/state.js", "text/javascript; charset=utf-8"),
            ("/messages.js", "text/javascript; charset=utf-8"),
            ("/axolotl.png", "image/png; charset=utf-8"),
        ] {
            let request =
                Request::builder().uri(path).body(Vec::new()).unwrap();
            let response = asset_response(WINDOW_LABEL, &request);
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], mime);
            assert!(!response.body().is_empty());
        }
        for (method, path, status) in [
            ("GET", "/missing.html", StatusCode::NOT_FOUND),
            ("POST", "/", StatusCode::METHOD_NOT_ALLOWED),
        ] {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .body(Vec::new())
                .unwrap();
            assert_eq!(asset_response(WINDOW_LABEL, &request).status(), status);
        }
    }

    #[test]
    fn navigation_stays_on_the_embedded_document_on_every_platform() {
        for url in [
            WINDOW_URL,
            "axolotl-agent://localhost/index.html",
            "http://axolotl-agent.localhost/",
        ] {
            assert!(is_window_url(&url.parse().unwrap()), "{url}");
        }
        for url in [
            "https://example.com/",
            "http://localhost/",
            "tauri://localhost/",
            "axolotl-agent://other/",
            "axolotl-agent://localhost/missing.html",
            "axolotl-agent://localhost/?redirect=https://example.com",
            "axolotl-agent://user@localhost/",
            "axolotl-agent://localhost:1234/",
        ] {
            assert!(!is_window_url(&url.parse().unwrap()), "{url}");
        }
    }
}
