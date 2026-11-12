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
            theme_pack_storage_list,
            theme_pack_storage_read,
            theme_pack_storage_write,
            theme_pack_storage_delete,
            theme_pack_instance_list,
            theme_pack_instance_mods,
            theme_pack_instance_logs,
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

const THEME_STORAGE_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const THEME_STORAGE_MAX_FILES: usize = 256;
const THEME_LOG_MAX_CHARS: usize = 256 * 1024;

fn theme_error(
    message: impl Into<String>,
) -> crate::api::TheseusSerializableError {
    theseus::Error::from(theseus::ErrorKind::OtherError(message.into())).into()
}

async fn manifest_with_permission<R: Runtime>(
    app: &AppHandle<R>,
    pack_id: &str,
    permission: theseus::theme_pack::ThemePackPermission,
) -> Result<theseus::theme_pack::ThemePackManifest> {
    let storage = storage(app)?;
    let pack = theseus::theme_pack::store::get(&storage, pack_id)
        .await?
        .ok_or_else(|| theme_error("The theme pack is not installed"))?;
    if !pack.manifest.has_permission(permission) {
        return Err(theme_error(format!(
            "The theme pack does not have the {permission:?} permission"
        )));
    }
    Ok(pack.manifest)
}

fn valid_storage_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && !key.starts_with('.')
        && key.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '-')
        })
}

fn storage_file<R: Runtime>(
    app: &AppHandle<R>,
    pack_id: &str,
    key: &str,
) -> Result<std::path::PathBuf> {
    if !valid_storage_key(key) {
        return Err(theme_error("The theme pack storage key is invalid"));
    }
    Ok(storage(app)?.pack_data_dir(pack_id)?.join(key))
}

#[tauri::command]
pub async fn theme_pack_storage_list<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
) -> Result<Vec<String>> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::Storage,
    )
    .await?;
    let directory = storage(&app)?.pack_data_dir(&pack_id)?;
    tokio::fs::create_dir_all(&directory).await?;
    let mut keys = Vec::new();
    let mut entries = tokio::fs::read_dir(&directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        if keys.len() >= THEME_STORAGE_MAX_FILES {
            break;
        }
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if valid_storage_key(&name) {
            keys.push(name);
        }
    }
    keys.sort();
    Ok(keys)
}

#[tauri::command]
pub async fn theme_pack_storage_read<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    key: String,
) -> Result<Option<String>> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::Storage,
    )
    .await?;
    let path = storage_file(&app, &pack_id, &key)?;
    let Ok(metadata) = tokio::fs::metadata(&path).await else {
        return Ok(None);
    };
    if !metadata.is_file() || metadata.len() > THEME_STORAGE_MAX_FILE_BYTES {
        return Err(theme_error("The theme pack storage entry is too large"));
    }
    Ok(Some(tokio::fs::read_to_string(&path).await?))
}

#[tauri::command]
pub async fn theme_pack_storage_write<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    key: String,
    contents: String,
) -> Result<()> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::Storage,
    )
    .await?;
    if contents.len() as u64 > THEME_STORAGE_MAX_FILE_BYTES {
        return Err(theme_error("The theme pack storage entry is too large"));
    }
    let path = storage_file(&app, &pack_id, &key)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let temporary =
        path.with_extension(format!("{}-{stamp}.tmp", std::process::id()));
    tokio::fs::write(&temporary, contents).await?;
    tokio::fs::rename(&temporary, &path).await?;
    Ok(())
}

#[tauri::command]
pub async fn theme_pack_storage_delete<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    key: String,
) -> Result<bool> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::Storage,
    )
    .await?;
    let path = storage_file(&app, &pack_id, &key)?;
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(false);
    }
    tokio::fs::remove_file(&path).await?;
    Ok(true)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackInstanceSummary {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: serde_json::Value,
    pub loader_version: Option<String>,
    pub last_played: Option<String>,
    pub time_played: u64,
}

#[tauri::command]
pub async fn theme_pack_instance_list<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
) -> Result<Vec<ThemePackInstanceSummary>> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::InstancesRead,
    )
    .await?;
    let instances = theseus::instance::list().await?;
    Ok(instances
        .into_iter()
        .map(|metadata| {
            let instance = metadata.instance;
            let content = metadata.applied_content_set;
            ThemePackInstanceSummary {
                id: instance.id,
                name: instance.name,
                game_version: content.game_version,
                loader: serde_json::to_value(content.loader)
                    .unwrap_or(serde_json::Value::Null),
                loader_version: content.loader_version,
                last_played: instance
                    .last_played
                    .map(|played| played.to_rfc3339()),
                time_played: instance
                    .submitted_time_played
                    .saturating_add(instance.recent_time_played),
            }
        })
        .collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackModSummary {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub project_type: serde_json::Value,
    pub project_id: Option<String>,
}

#[tauri::command]
pub async fn theme_pack_instance_mods<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    instance_id: String,
) -> Result<Vec<ThemePackModSummary>> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::InstancesRead,
    )
    .await?;
    let projects = theseus::instance::get_projects(&instance_id, None).await?;
    let mut mods: Vec<ThemePackModSummary> = projects
        .iter()
        .map(|entry| {
            let file = entry.value();
            ThemePackModSummary {
                file_name: file.file_name.clone(),
                enabled: file.enabled,
                size: file.size,
                project_type: serde_json::to_value(file.project_type)
                    .unwrap_or(serde_json::Value::Null),
                project_id: file
                    .modrinth
                    .as_ref()
                    .map(|matched| matched.project_id.to_string()),
            }
        })
        .collect();
    mods.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(mods)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackLogSummary {
    pub log_type: serde_json::Value,
    pub file_name: String,
    pub age: u64,
    pub output: String,
    pub truncated: bool,
}

#[tauri::command]
pub async fn theme_pack_instance_logs<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    instance_id: String,
) -> Result<Vec<ThemePackLogSummary>> {
    manifest_with_permission(
        &app,
        &pack_id,
        theseus::theme_pack::ThemePackPermission::InstancesLogs,
    )
    .await?;
    let logs = theseus::logs::get_logs(&instance_id, None).await?;
    Ok(logs
        .into_iter()
        .map(|log| {
            let output = log.output.as_ref().map(|text| text.as_str());
            let output = output.unwrap_or_default();
            let length = output.chars().count();
            let truncated = length > THEME_LOG_MAX_CHARS;
            let output = if truncated {
                output.chars().skip(length - THEME_LOG_MAX_CHARS).collect()
            } else {
                output.to_string()
            };
            ThemePackLogSummary {
                log_type: serde_json::to_value(log.log_type)
                    .unwrap_or(serde_json::Value::Null),
                file_name: log.filename,
                age: log.age,
                output,
                truncated,
            }
        })
        .collect())
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
        .header(header::VARY, "Origin");
    if let Some(origin) = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        // Only echo an origin that passed `is_allowed_theme_request`; `null`
        // is the opaque origin of a sandboxed pack page fetching its own font.
        if origin == "null" || is_allowed_origin(origin) {
            builder =
                builder.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
    }
    if content_type.starts_with("text/html")
        || content_type.starts_with("image/svg+xml")
    {
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

const ALLOWED_THEME_ORIGINS: [&str; 7] = [
    "http://127.0.0.1:5201",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "tauri://localhost",
    "axolotl-theme://localhost",
    "http://axolotl-theme.localhost",
    "axolotl-skin://localhost",
];

fn is_allowed_origin(origin: &str) -> bool {
    ALLOWED_THEME_ORIGINS.iter().any(|allowed| {
        origin == *allowed || origin.starts_with(&format!("{allowed}/"))
    })
}

fn is_allowed_theme_request(request: &tauri::http::Request<Vec<u8>>) -> bool {
    if !matches!(
        request.uri().host(),
        Some("localhost" | "axolotl-theme.localhost")
    ) {
        return false;
    }
    match request
        .headers()
        .get(tauri::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        Some(origin) => is_allowed_origin(origin),
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
