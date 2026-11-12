use std::path::Path;

use crate::api::Result;
use tauri::AppHandle;
use tauri::Manager;
use tauri::Runtime;
use theseus::theme_pack::ThemePackStorage;
use theseus::theme_pack::archive::ThemePackLimits;
use theseus::theme_pack::network::ThemeNetworkRequest;
use theseus::theme_pack::network::ThemeNetworkResponse;
use theseus::theme_pack::store::InstalledThemePack;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("theme-pack")
        .invoke_handler(tauri::generate_handler![
            theme_pack_list,
            theme_pack_get,
            theme_pack_install,
            theme_pack_remove,
            theme_pack_limits,
            theme_pack_fetch,
            theme_pack_data_dir,
            theme_pack_resolve_page,
        ])
        .build()
}

fn storage<R: Runtime>(app: &AppHandle<R>) -> Result<ThemePackStorage> {
    Ok(ThemePackStorage::new(&app.path().app_data_dir()?))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackLimitsInfo {
    pub max_archive_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_entries: usize,
}

#[tauri::command]
pub async fn theme_pack_list<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<InstalledThemePack>> {
    let storage = storage(&app)?;
    Ok(theseus::theme_pack::store::list(&storage).await?)
}

#[tauri::command]
pub async fn theme_pack_get<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
) -> Result<Option<InstalledThemePack>> {
    let storage = storage(&app)?;
    Ok(theseus::theme_pack::store::get(&storage, &pack_id).await?)
}

#[tauri::command]
pub async fn theme_pack_install<R: Runtime>(
    app: AppHandle<R>,
    archive_path: String,
    replace: bool,
) -> Result<InstalledThemePack> {
    let storage = storage(&app)?;
    Ok(theseus::theme_pack::store::install(
        &storage,
        Path::new(&archive_path),
        ThemePackLimits::from_env(),
        replace,
    )
    .await?)
}

#[tauri::command]
pub async fn theme_pack_remove<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    remove_data: bool,
) -> Result<bool> {
    let storage = storage(&app)?;
    Ok(
        theseus::theme_pack::store::remove(&storage, &pack_id, remove_data)
            .await?,
    )
}

#[tauri::command]
pub async fn theme_pack_limits() -> Result<ThemePackLimitsInfo> {
    let limits = ThemePackLimits::from_env();
    Ok(ThemePackLimitsInfo {
        max_archive_bytes: limits.max_archive_bytes,
        max_uncompressed_bytes: limits.max_uncompressed_bytes,
        max_entries: limits.max_entries,
    })
}

#[tauri::command]
pub async fn theme_pack_fetch<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    request: ThemeNetworkRequest,
) -> Result<ThemeNetworkResponse> {
    let storage = storage(&app)?;
    Ok(
        theseus::theme_pack::network::fetch(&storage, &pack_id, request)
            .await?,
    )
}

#[tauri::command]
pub async fn theme_pack_data_dir<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
) -> Result<String> {
    let storage = storage(&app)?;
    let directory = storage.pack_data_dir(&pack_id)?;
    tokio::fs::create_dir_all(&directory).await?;
    Ok(directory.to_string_lossy().to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackPageInfo {
    pub pack_id: String,
    pub page_id: String,
    pub title: serde_json::Value,
    pub path: String,
    pub url: String,
}

#[tauri::command]
pub async fn theme_pack_resolve_page<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    page_id: String,
) -> Result<ThemePackPageInfo> {
    let storage = storage(&app)?;
    let resolved =
        theseus::theme_pack::store::resolve_page(&storage, &pack_id, &page_id)?;
    Ok(ThemePackPageInfo {
        pack_id: pack_id.clone(),
        page_id,
        title: serde_json::to_value(&resolved.page.title)
            .unwrap_or(serde_json::Value::Null),
        path: resolved.path.to_string_lossy().to_string(),
        url: format!("{}/{}/{}", theme_origin(), pack_id, resolved.page.entry),
    })
}

/// The protocol origin for theme assets. Windows cannot serve a custom scheme
/// as a standard origin, so it uses the `.localhost` form there.
pub fn theme_origin() -> &'static str {
    if cfg!(windows) {
        "http://axolotl-theme.localhost"
    } else {
        "axolotl-theme://localhost"
    }
}

pub fn protocol_response(
    app: &AppHandle<impl Runtime>,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let Ok(storage) = storage(app) else {
        return protocol_error(tauri::http::StatusCode::INTERNAL_SERVER_ERROR);
    };
    protocol_response_for(&storage, request)
}

fn protocol_error(
    status: tauri::http::StatusCode,
) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .body(Vec::new())
        .expect("failed to build theme pack response")
}

pub(crate) fn protocol_response_for(
    storage: &ThemePackStorage,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::StatusCode;
    use tauri::http::header;

    if !is_allowed_theme_request(request) {
        return protocol_error(StatusCode::FORBIDDEN);
    }
    let path = request.uri().path().trim_start_matches('/');
    let Ok(path) = urlencoding::decode(path) else {
        return protocol_error(StatusCode::BAD_REQUEST);
    };
    let Some((pack_id, relative)) = path.split_once('/') else {
        return protocol_error(StatusCode::NOT_FOUND);
    };
    let Ok(file_path) =
        theseus::theme_pack::store::asset_path(storage, pack_id, relative)
    else {
        return protocol_error(StatusCode::NOT_FOUND);
    };
    let Ok(contents) = std::fs::read(&file_path) else {
        return protocol_error(StatusCode::NOT_FOUND);
    };
    let content_type = content_type_for(&file_path);
    let mut builder = tauri::http::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    if content_type.starts_with("text/html") {
        builder = builder.header(
            "Content-Security-Policy",
            "sandbox allow-scripts; default-src 'self'; script-src 'self' 'unsafe-inline'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; \
             connect-src 'none'; media-src 'self'; frame-ancestors http://tauri.localhost \
             tauri://localhost http://axolotl-theme.localhost axolotl-theme://localhost",
        );
    }
    builder
        .body(contents)
        .unwrap_or_else(|_| protocol_error(StatusCode::INTERNAL_SERVER_ERROR))
}

fn is_allowed_theme_request(request: &tauri::http::Request<Vec<u8>>) -> bool {
    if !matches!(
        request.uri().host(),
        Some("localhost" | "axolotl-theme.localhost")
    ) {
        return false;
    }
    const ALLOWED_ORIGINS: [&str; 7] = [
        "http://127.0.0.1:5201",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
        "axolotl-theme://localhost",
        "http://axolotl-theme.localhost",
        "axolotl-skin://localhost",
    ];
    match request
        .headers()
        .get(tauri::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        Some(origin) => ALLOWED_ORIGINS.iter().any(|allowed| {
            origin == *allowed || origin.starts_with(&format!("{allowed}/"))
        }),
        // Same-origin subresource loads (a stylesheet's own font, an image in
        // pack HTML) do not carry an Origin header.
        None => true,
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::http::Request;
    use tauri::http::StatusCode;

    fn request(uri: &str, origin: Option<&str>) -> Request<Vec<u8>> {
        let mut builder = Request::builder().uri(uri);
        if let Some(origin) = origin {
            builder = builder.header("Origin", origin);
        }
        builder.body(Vec::new()).unwrap()
    }

    fn storage_with_css() -> (tempfile::TempDir, ThemePackStorage) {
        let directory = tempfile::tempdir().unwrap();
        let styles = directory.path().join("themes/demo/styles");
        std::fs::create_dir_all(&styles).unwrap();
        std::fs::write(styles.join("theme.css"), ".x { color: red; }").unwrap();
        let storage = ThemePackStorage::new(directory.path());
        (directory, storage)
    }

    #[test]
    fn serves_a_pack_asset_without_an_origin() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(
                "http://axolotl-theme.localhost/demo/styles/theme.css",
                None,
            ),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/css; charset=utf-8"
        );
        assert_eq!(response.body(), b".x { color: red; }");
    }

    #[test]
    fn refuses_a_foreign_origin() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(
                "http://axolotl-theme.localhost/demo/styles/theme.css",
                Some("https://tracker.example"),
            ),
        );
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn refuses_encoded_traversal() {
        let (_directory, storage) = storage_with_css();
        for path in [
            "/demo/%2e%2e/manifest.json",
            "/demo/styles/%2e%2e/%2e%2e/secret",
            "/demo/%2Fetc%2Fpasswd",
        ] {
            let response = protocol_response_for(
                &storage,
                &request(
                    &format!("http://axolotl-theme.localhost{path}"),
                    None,
                ),
            );
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{path} should not resolve"
            );
        }
    }

    #[test]
    fn refuses_an_invalid_pack_id() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(
                "http://axolotl-theme.localhost/../styles/theme.css",
                None,
            ),
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
