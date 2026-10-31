use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use tokio::sync::{Mutex, RwLock};

pub const WINDOW_LABEL: &str = "agent-window";
pub const URI_SCHEME: &str = "axolotl-agent";

const WINDOW_URL: &str = "axolotl-agent://localhost/";
const HTML: &str = include_str!("index.html");
const CSS: &str = include_str!("window.css");
const JS: &str = include_str!("window.js");

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
    pub labels: WindowLabels,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            background: "#ffffff".into(),
            surface: "#f4f4f5".into(),
            text: "#1f2937".into(),
            accent: "#1bd96a".into(),
            labels: WindowLabels::default(),
        }
    }
}

/// Owns window creation independently of the launcher's frontend and runtime.
#[derive(Default)]
pub struct AgentWindow {
    opening: Mutex<()>,
    appearance: RwLock<Appearance>,
}

impl AgentWindow {
    pub async fn open(&self, app: &AppHandle<Wry>) -> tauri::Result<()> {
        self.open_with_appearance(app, None).await
    }

    pub async fn appearance(&self) -> Appearance {
        self.appearance.read().await.clone()
    }

    pub async fn open_with_appearance(
        &self,
        app: &AppHandle<Wry>,
        appearance: Option<Appearance>,
    ) -> tauri::Result<()> {
        // Tauri's label check and native window creation must be serialized.
        // This async entry point also keeps creation off the Windows UI thread.
        let _opening = self.opening.lock().await;
        if let Some(appearance) = appearance {
            *self.appearance.write().await = appearance;
        }
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window.eval(
                "window.dispatchEvent(new Event('agent-appearance-changed'))",
            )?;
            window.unminimize()?;
            window.show()?;
            return window.set_focus();
        }

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
        .resizable(true)
        .center()
        .disable_drag_drop_handler()
        .on_navigation(is_window_url)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .build()?;
        window.set_focus()
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
            _ => (StatusCode::NOT_FOUND, "text/plain", Vec::new()),
        }
    };

    tauri::http::Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, format!("{content_type}; charset=utf-8"))
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        )
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("valid Agent Window response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::http::{Request, StatusCode};

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
