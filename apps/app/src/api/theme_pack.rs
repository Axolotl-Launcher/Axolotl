use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::api::Result;
use tauri::AppHandle;
use tauri::LogicalPosition;
use tauri::LogicalSize;
use tauri::Manager;
use tauri::Runtime;
use tauri::State;
use tauri::Webview;
use tauri::WebviewUrl;
use tauri::webview::WebviewBuilder;
use theseus::theme_pack::ThemePackStorage;
use theseus::theme_pack::archive::ORIGIN_FILE;
use theseus::theme_pack::archive::ThemePackLimits;
use theseus::theme_pack::archive::now_seconds;
use theseus::theme_pack::manifest::MANIFEST_FILE;
use theseus::theme_pack::manifest::ThemePackManifest;
use theseus::theme_pack::manifest::ThemePackPermission;
use theseus::theme_pack::manifest::is_windows_device_name;
use theseus::theme_pack::network::ThemeNetworkRequest;
use theseus::theme_pack::network::ThemeNetworkResponse;
use theseus::theme_pack::store::InstalledThemePack;

/// Child webview labels are the only webviews granted the page bridge
/// permission. The label prefix is also used by the `theme-pages` capability.
pub const THEME_PAGE_LABEL_PREFIX: &str = "theme-page-";

/// Serializes storage accounting with the write it authorizes so concurrent
/// pages cannot each pass the quota check against stale usage.
static THEME_STORAGE_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

/// Tracks which theme pack owns each page webview. The bridge command derives
/// the pack id from this map instead of trusting the page, so a page cannot
/// request another pack's storage or network allowlist.
#[derive(Default)]
pub struct ThemePackPageState {
    pages: Mutex<HashMap<String, ThemePageOwner>>,
    counter: AtomicU64,
}

/// The resolved identity of an open page. Requirements are captured when the
/// page opens so navigation cannot widen what the page declared.
#[derive(Clone)]
struct ThemePageOwner {
    pack_id: String,
    requires: Vec<ThemePackPermission>,
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("theme-pack")
        .setup(|app, _api| {
            app.manage(ThemePackPageState::default());
            Ok(())
        })
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
            theme_pack_export_snapshot,
            theme_pack_open_page,
            theme_pack_page_bounds,
            theme_pack_close_page,
            theme_pack_page_request,
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
        url: format!(
            "{}/{}/{}",
            theme_origin_for(&pack_id),
            encode_url_segment(&pack_id),
            resolved
                .page
                .entry
                .split('/')
                .map(encode_url_segment)
                .collect::<Vec<_>>()
                .join("/")
        ),
    })
}

/// The bridge exposed to pack pages. It only calls the page bridge command;
/// the child webview's capability grants nothing else.
const THEME_PAGE_API: &str = r#"
(function () {
  const call = (method, params) =>
    window.__TAURI_INTERNALS__.invoke('plugin:theme-pack|theme_pack_page_request', {
      request: { method: method, params: params || {} },
    });
  Object.defineProperty(window, 'axolotlTheme', {
    value: Object.freeze({
      request: call,
      network: {
        fetch: (request) => call('network.fetch', request),
      },
      storage: {
        list: () => call('storage.list'),
        read: (key) => call('storage.read', { key: key }),
        write: (key, contents) => call('storage.write', { key: key, contents: contents }),
        delete: (key) => call('storage.delete', { key: key }),
      },
      instances: {
        list: () => call('instances.list'),
        mods: (instanceId) => call('instances.mods', { instanceId: instanceId }),
        logs: (instanceId) => call('instances.logs', { instanceId: instanceId }),
      },
    }),
  });
})();
"#;

fn lock_pages(
    state: &ThemePackPageState,
) -> Result<std::sync::MutexGuard<'_, HashMap<String, ThemePageOwner>>> {
    state
        .pages
        .lock()
        .map_err(|_| theme_error("The theme page state is unavailable"))
}

fn page_owner(
    state: &ThemePackPageState,
    label: &str,
) -> Result<ThemePageOwner> {
    lock_pages(state)?
        .get(label)
        .cloned()
        .ok_or_else(|| theme_error("The theme page is not open"))
}

fn page_pack_id(state: &ThemePackPageState, label: &str) -> Result<String> {
    page_owner(state, label).map(|owner| owner.pack_id)
}

fn permission_name(permission: ThemePackPermission) -> &'static str {
    match permission {
        ThemePackPermission::Storage => "storage",
        ThemePackPermission::InstancesRead => "instances.read",
        ThemePackPermission::InstancesLogs => "instances.logs",
        ThemePackPermission::Network => "network",
    }
}

fn require_page_permission(
    owner: &ThemePageOwner,
    permission: ThemePackPermission,
) -> Result<()> {
    if owner.requires.contains(&permission) {
        Ok(())
    } else {
        Err(theme_error(&format!(
            "The theme page did not declare the {} permission",
            permission_name(permission)
        )))
    }
}

/// A navigation target may only differ from the exact entry URL by nothing:
/// a scheme, host, or port change would leave the pack origin and turn the
/// page into a generic browser window with access to the new host.
fn is_allowed_page_location(
    url: &url::Url,
    origin: &url::Url,
    path: &str,
) -> bool {
    url.scheme() == origin.scheme()
        && url.host_str() == origin.host_str()
        && url.port_or_known_default() == origin.port_or_known_default()
        && url.path() == path
        && url.query().is_none()
        && url.fragment().is_none()
}

/// Opens a pack page in a child webview. Pack pages never share the main
/// webview: their capability only allows `theme_pack_page_request`, so the
/// page cannot reach filesystem, install, or process commands.
#[tauri::command]
pub async fn theme_pack_open_page<R: Runtime>(
    webview: Webview<R>,
    state: State<'_, ThemePackPageState>,
    pack_id: String,
    page_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<String> {
    let app = webview.app_handle();
    let storage = storage(app)?;
    let resolved =
        theseus::theme_pack::store::resolve_page(&storage, &pack_id, &page_id)?;
    let entry = resolved
        .page
        .entry
        .split('/')
        .map(encode_url_segment)
        .collect::<Vec<_>>()
        .join("/");
    let origin = theme_origin_for(&pack_id);
    let origin_url = url::Url::parse(&origin).map_err(|_| {
        theme_error("The theme page origin could not be resolved")
    })?;
    let page_url =
        format!("{}/{}/{}", origin, encode_url_segment(&pack_id), entry);
    let page_url: url::Url = page_url
        .parse()
        .map_err(|_| theme_error("The theme page URL could not be resolved"))?;
    let label = format!(
        "{}{}",
        THEME_PAGE_LABEL_PREFIX,
        state.counter.fetch_add(1, Ordering::Relaxed)
    );
    // Only the declared entry may load in this webview. Allowing navigation
    // within the pack would let a page with fewer declared requirements take
    // over a bridge session opened for a page with more requirements.
    let allowed_path = format!("/{}/{}", encode_url_segment(&pack_id), entry);
    let builder =
        WebviewBuilder::new(label.clone(), WebviewUrl::External(page_url))
            .initialization_script(THEME_PAGE_API)
            .on_navigation(move |url| {
                is_allowed_page_location(url, &origin_url, &allowed_path)
            });
    lock_pages(&state)?.insert(
        label.clone(),
        ThemePageOwner {
            pack_id,
            requires: resolved.page.requires.clone(),
        },
    );
    if let Err(error) = webview.window().add_child(
        builder,
        LogicalPosition::new(x, y),
        LogicalSize::new(width.max(1.0), height.max(1.0)),
    ) {
        lock_pages(&state)?.remove(&label);
        return Err(error.into());
    }
    Ok(label)
}

#[tauri::command]
pub async fn theme_pack_page_bounds<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ThemePackPageState>,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<()> {
    page_pack_id(&state, &label)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| theme_error("The theme page is not open"))?;
    webview.set_position(LogicalPosition::new(x, y))?;
    webview.set_size(LogicalSize::new(width.max(1.0), height.max(1.0)))?;
    Ok(())
}

#[tauri::command]
pub async fn theme_pack_close_page<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, ThemePackPageState>,
    label: String,
) -> Result<()> {
    page_pack_id(&state, &label)?;
    lock_pages(&state)?.remove(&label);
    if let Some(webview) = app.get_webview(&label) {
        webview.close()?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePageRequest {
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

fn request_string(params: &serde_json::Value, key: &str) -> Result<String> {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            theme_error("The theme page request is missing an argument")
        })
}

#[tauri::command]
pub async fn theme_pack_page_request<R: Runtime>(
    webview: Webview<R>,
    state: State<'_, ThemePackPageState>,
    request: ThemePageRequest,
) -> Result<serde_json::Value> {
    let owner = page_owner(&state, webview.label())?;
    let pack_id = owner.pack_id.clone();
    let app = webview.app_handle().clone();
    match request.method.as_str() {
        "network.fetch" => {
            require_page_permission(&owner, ThemePackPermission::Network)?;
            let request: ThemeNetworkRequest =
                serde_json::from_value(request.params).map_err(|error| {
                    theme_error(&format!(
                        "The network request is invalid: {error}"
                    ))
                })?;
            let storage = storage(&app)?;
            let response = theseus::theme_pack::network::fetch(
                &storage, &pack_id, request,
            )
            .await?;
            Ok(serde_json::to_value(response).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "storage.list" => {
            require_page_permission(&owner, ThemePackPermission::Storage)?;
            let keys = theme_pack_storage_list(app, pack_id).await?;
            Ok(serde_json::to_value(keys).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "storage.read" => {
            require_page_permission(&owner, ThemePackPermission::Storage)?;
            let key = request_string(&request.params, "key")?;
            let contents = theme_pack_storage_read(app, pack_id, key).await?;
            Ok(serde_json::to_value(contents).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "storage.write" => {
            require_page_permission(&owner, ThemePackPermission::Storage)?;
            let key = request_string(&request.params, "key")?;
            let contents = request_string(&request.params, "contents")?;
            theme_pack_storage_write(app, pack_id, key, contents).await?;
            Ok(serde_json::Value::Null)
        }
        "storage.delete" => {
            require_page_permission(&owner, ThemePackPermission::Storage)?;
            let key = request_string(&request.params, "key")?;
            let removed = theme_pack_storage_delete(app, pack_id, key).await?;
            Ok(serde_json::to_value(removed).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "instances.list" => {
            require_page_permission(
                &owner,
                ThemePackPermission::InstancesRead,
            )?;
            let instances = theme_pack_instance_list(app, pack_id).await?;
            Ok(serde_json::to_value(instances).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "instances.mods" => {
            require_page_permission(
                &owner,
                ThemePackPermission::InstancesRead,
            )?;
            let instance_id = request_string(&request.params, "instanceId")?;
            let mods =
                theme_pack_instance_mods(app, pack_id, instance_id).await?;
            Ok(serde_json::to_value(mods).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        "instances.logs" => {
            require_page_permission(
                &owner,
                ThemePackPermission::InstancesLogs,
            )?;
            let instance_id = request_string(&request.params, "instanceId")?;
            let logs =
                theme_pack_instance_logs(app, pack_id, instance_id).await?;
            Ok(serde_json::to_value(logs).map_err(|error| {
                theme_error(&format!(
                    "The response could not be serialized: {error}"
                ))
            })?)
        }
        method => Err(theme_error(&format!(
            "The theme page method {method} is not available"
        ))),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackSnapshot {
    pub schema_version: u32,
    pub source: String,
    pub theme: String,
    #[serde(default)]
    pub accent_color: Option<String>,
    #[serde(default)]
    pub custom_theme_colors: serde_json::Value,
    #[serde(default)]
    pub tokens_light: BTreeMap<String, String>,
    #[serde(default)]
    pub tokens_dark: BTreeMap<String, String>,
    #[serde(default)]
    pub pack_id: Option<String>,
}

#[tauri::command]
pub async fn theme_pack_export_snapshot<R: Runtime>(
    app: AppHandle<R>,
    destination: String,
    snapshot: ThemePackSnapshot,
) -> Result<String> {
    let storage = storage(&app)?;
    let (pack_dir, pack_data_dir) = match snapshot.pack_id.as_deref() {
        Some(pack_id) => {
            let installed = theseus::theme_pack::store::get(&storage, pack_id)
                .await?
                .ok_or_else(|| {
                    theme_error("The theme pack is not installed")
                })?;
            (
                Some(std::path::PathBuf::from(installed.path)),
                Some(storage.pack_data_dir(pack_id)?),
            )
        }
        None => (None, None),
    };
    let destination = std::path::PathBuf::from(destination);
    let destination_for_task = destination.clone();
    tokio::task::spawn_blocking(move || {
        export_snapshot_archive(
            &destination_for_task,
            &snapshot,
            pack_dir.as_deref(),
            pack_data_dir.as_deref(),
            ThemePackLimits::from_env(),
        )
    })
    .await
    .map_err(|error| {
        theme_error(format!("The snapshot export task failed: {error}"))
    })??;
    Ok(destination.to_string_lossy().to_string())
}

fn export_snapshot_archive(
    destination: &Path,
    snapshot: &ThemePackSnapshot,
    pack_dir: Option<&Path>,
    pack_data_dir: Option<&Path>,
    limits: ThemePackLimits,
) -> Result<()> {
    if !destination
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err(theme_error(
            "The appearance snapshot must be saved as a .zip file",
        ));
    }
    let parent = match destination.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.is_dir() {
        return Err(theme_error("The selected folder does not exist"));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = nanos ^ (u128::from(sequence) << 64);
    let temporary = parent.join(format!(
        ".axolotl-theme-snapshot-{}-{nonce}.tmp",
        std::process::id()
    ));
    match write_snapshot_archive(
        &temporary,
        snapshot,
        pack_dir,
        pack_data_dir,
        limits,
        nonce,
    ) {
        Ok(()) => {
            // A save dialog already asked the user to confirm replacing the
            // file. Move the old archive aside first so a failed rename cannot
            // leave the user with neither the old nor the new snapshot.
            let backup = destination
                .exists()
                .then(|| temporary.with_extension("bak"));
            if let Some(backup) = &backup {
                if let Err(error) = std::fs::rename(destination, backup) {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(theme_error(format!(
                        "The existing snapshot could not be replaced: {error}"
                    )));
                }
            }
            if let Err(error) = std::fs::rename(&temporary, destination) {
                let _ = std::fs::remove_file(&temporary);
                if let Some(backup) = &backup {
                    let _ = std::fs::rename(backup, destination);
                }
                return Err(theme_error(format!(
                    "The appearance snapshot could not be saved: {error}"
                )));
            }
            if let Some(backup) = &backup {
                let _ = std::fs::remove_file(backup);
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn write_snapshot_archive(
    temporary: &Path,
    snapshot: &ThemePackSnapshot,
    pack_dir: Option<&Path>,
    pack_data_dir: Option<&Path>,
    limits: ThemePackLimits,
    nonce: u128,
) -> Result<()> {
    let exported_at = now_seconds();
    let file = std::fs::File::create(temporary).map_err(|error| {
        theme_error(format!(
            "The appearance snapshot file could not be created: {error}"
        ))
    })?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let origin = match pack_dir {
        Some(pack_dir) => {
            let manifest = ThemePackManifest::load(pack_dir)?;
            let mut entries = 0usize;
            let mut bytes = 0u64;
            copy_pack_directory(
                &mut archive,
                options,
                pack_dir,
                pack_dir,
                pack_data_dir,
                limits,
                &mut entries,
                &mut bytes,
            )?;
            serde_json::json!({
                "kind": "pack",
                "packId": manifest.id,
                "version": manifest.version,
                "exportedAt": exported_at,
            })
        }
        None => {
            let manifest = snapshot_manifest(snapshot, exported_at, nonce)?;
            let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
                theme_error(format!(
                    "The appearance snapshot manifest could not be written: {error}"
                ))
            })?;
            write_archive_entry(&mut archive, options, MANIFEST_FILE, &bytes)?;
            serde_json::json!({
                "kind": snapshot.source,
                "theme": snapshot.theme,
                "accentColor": snapshot.accent_color,
                "customThemeColors": snapshot.custom_theme_colors,
                "exportedAt": exported_at,
            })
        }
    };
    let origin = serde_json::to_vec_pretty(&origin).map_err(|error| {
        theme_error(format!(
            "The appearance snapshot origin could not be written: {error}"
        ))
    })?;
    write_archive_entry(&mut archive, options, ORIGIN_FILE, &origin)?;
    archive.finish().map_err(|error| {
        theme_error(format!(
            "The appearance snapshot could not be finalized: {error}"
        ))
    })?;
    Ok(())
}

fn snapshot_manifest(
    snapshot: &ThemePackSnapshot,
    exported_at: u64,
    nonce: u128,
) -> Result<ThemePackManifest> {
    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "id": format!("snapshot-{exported_at}-{nonce}"),
        "name": { "en-US": "Appearance snapshot" },
        "version": "1.0.0",
        "engines": { "axolotl": ">=1.0.0" },
        "capabilities": ["tokens"],
        "tokens": {
            "light": &snapshot.tokens_light,
            "dark": &snapshot.tokens_dark,
        },
        "styles": [],
        "permissions": [],
        "network": { "allow": [] },
        "pages": [],
    });
    serde_json::from_value(manifest).map_err(|error| {
        theme_error(format!(
            "The appearance snapshot manifest is invalid: {error}"
        ))
    })
}

fn write_archive_entry(
    archive: &mut zip::ZipWriter<std::fs::File>,
    options: zip::write::SimpleFileOptions,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    archive.start_file(name, options).map_err(|error| {
        theme_error(format!(
            "The appearance snapshot entry {name} could not be written: {error}"
        ))
    })?;
    archive.write_all(bytes).map_err(|error| {
        theme_error(format!(
            "The appearance snapshot entry {name} could not be written: {error}"
        ))
    })
}

#[allow(clippy::too_many_arguments)]
fn copy_pack_directory(
    archive: &mut zip::ZipWriter<std::fs::File>,
    options: zip::write::SimpleFileOptions,
    root: &Path,
    directory: &Path,
    pack_data_dir: Option<&Path>,
    limits: ThemePackLimits,
    entries: &mut usize,
    bytes: &mut u64,
) -> Result<()> {
    let read_dir = std::fs::read_dir(directory).map_err(|error| {
        theme_error(format!("The theme pack folder could not be read: {error}"))
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|error| {
            theme_error(format!(
                "The theme pack folder could not be read: {error}"
            ))
        })?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if pack_data_dir.is_some_and(|data_dir| path.starts_with(data_dir)) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            theme_error(format!(
                "The theme pack file could not be read: {error}"
            ))
        })?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            copy_pack_directory(
                archive,
                options,
                root,
                &path,
                pack_data_dir,
                limits,
                entries,
                bytes,
            )?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let relative = path.strip_prefix(root).map_err(|error| {
            theme_error(format!(
                "The theme pack file could not be exported: {error}"
            ))
        })?;
        let name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if name == ORIGIN_FILE {
            continue;
        }
        *entries += 1;
        if *entries > limits.max_entries {
            return Err(theme_error(format!(
                "The theme pack has more than {} files",
                limits.max_entries
            )));
        }
        *bytes = bytes.checked_add(metadata.len()).ok_or_else(|| {
            theme_error("The theme pack is too large to export")
        })?;
        if *bytes > limits.max_uncompressed_bytes {
            return Err(theme_error(format!(
                "The theme pack is larger than the {} byte export limit",
                limits.max_uncompressed_bytes
            )));
        }
        archive.start_file(&name, options).map_err(|error| {
            theme_error(format!(
                "The appearance snapshot entry {name} could not be written: {error}"
            ))
        })?;
        let mut source = std::fs::File::open(&path).map_err(|error| {
            theme_error(format!(
                "The theme pack file could not be read: {error}"
            ))
        })?;
        std::io::copy(&mut source, archive).map_err(|error| {
            theme_error(format!(
                "The appearance snapshot entry {name} could not be written: {error}"
            ))
        })?;
    }
    Ok(())
}

static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const THEME_STORAGE_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const THEME_STORAGE_MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024;
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
        && !key.ends_with('.')
        && !is_windows_device_name(key)
        && key.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '-')
        })
}

/// Windows resolves the reserved DOS device names in every directory, so a
/// storage key such as `con` or `lpt1.txt` would address a device instead of a
/// file in the pack data directory.
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

async fn storage_usage(directory: &Path) -> Result<(u64, usize)> {
    let mut entries = tokio::fs::read_dir(directory).await?;
    let mut total = 0u64;
    let mut files = 0usize;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
            files += 1;
        }
    }
    Ok((total, files))
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
    let _guard = THEME_STORAGE_LOCK.lock().await;
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
    let parent = path
        .parent()
        .ok_or_else(|| theme_error("The theme pack storage path is invalid"))?;
    tokio::fs::create_dir_all(parent).await?;
    let existing = tokio::fs::metadata(&path).await.ok();
    let exists = existing.as_ref().is_some_and(|metadata| metadata.is_file());
    let previous = existing
        .as_ref()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let (usage, files) = storage_usage(parent).await?;
    if !exists && files >= THEME_STORAGE_MAX_FILES {
        return Err(theme_error("The theme pack storage has too many files"));
    }
    let next = usage
        .saturating_sub(previous)
        .saturating_add(contents.len() as u64);
    if next > THEME_STORAGE_MAX_TOTAL_BYTES {
        return Err(theme_error("The theme pack storage quota is exhausted"));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let temporary =
        path.with_extension(format!("{}-{stamp}.tmp", std::process::id()));
    if let Err(error) = tokio::fs::write(&temporary, contents).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&temporary, &path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    Ok(())
}

#[tauri::command]
pub async fn theme_pack_storage_delete<R: Runtime>(
    app: AppHandle<R>,
    pack_id: String,
    key: String,
) -> Result<bool> {
    let _guard = THEME_STORAGE_LOCK.lock().await;
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

/// Every pack gets its own origin by deriving a stable port from its id. The
/// launcher and the protocol handler share the algorithm, so two packs never
/// share `localStorage`, `sessionStorage`, IndexedDB, or CSSOM access.
pub fn theme_pack_port(pack_id: &str) -> u16 {
    theseus::theme_pack::manifest::origin_port(pack_id)
}

/// The protocol origin for one theme pack. Windows cannot serve a custom
/// scheme as a standard origin, so it uses the `.localhost` form there. The
/// port is virtual: wry intercepts the request before any network access.
pub fn theme_origin_for(pack_id: &str) -> String {
    let port = theme_pack_port(pack_id);
    if cfg!(windows) {
        format!("http://axolotl-theme.localhost:{port}")
    } else {
        format!("axolotl-theme://localhost:{port}")
    }
}

/// Percent-encodes one URL path segment. Manifest paths may contain spaces or
/// `#`, which would otherwise be parsed as separators or fragments.
fn encode_url_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~' => encoded.push(byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
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
    // The port is part of the pack origin. Rejecting a mismatched port stops
    // one pack page from reading another pack's assets through this handler.
    if request.uri().port_u16() != Some(theme_pack_port(pack_id)) {
        return protocol_error(StatusCode::NOT_FOUND);
    }
    let pack_origin = theme_origin_for(pack_id);
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
        // Echo only the requesting pack's own origin or a launcher origin.
        // Another pack's origin must not gain CORS access to these assets.
        if origin == pack_origin || is_allowed_origin(origin) {
            builder =
                builder.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
    }
    if content_type.starts_with("text/html") {
        // Pack pages run in a child webview whose capability only allows the
        // page bridge command. They keep their real theme origin so the bridge
        // capability applies, and the bridge derives the pack id from the
        // webview label rather than from the page.
        let origin = theme_origin_for(pack_id);
        builder = builder.header(
            "Content-Security-Policy",
            format!(
                "default-src {origin}; \
                 script-src {origin} 'unsafe-inline'; \
                 style-src {origin} 'unsafe-inline'; \
                 img-src {origin} data:; \
                 font-src {origin} data:; \
                 connect-src 'none'; media-src {origin}; \
                 object-src 'none'; base-uri 'none'; form-action 'none'; \
                 frame-src 'none'; frame-ancestors 'none'"
            ),
        );
    } else if content_type.starts_with("image/svg+xml") {
        // SVG documents never need the bridge; keep them sandboxed so their
        // scripts cannot run with the theme origin.
        builder = builder.header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; style-src 'unsafe-inline'; \
             img-src data:",
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

/// Matches the per-pack origins (`http://axolotl-theme.localhost:<port>` on
/// Windows, `axolotl-theme://localhost:<port>` elsewhere) without accepting a
/// path or arbitrary suffix.
fn is_theme_origin(origin: &str) -> bool {
    fn valid_port_suffix(rest: &str) -> bool {
        rest.is_empty()
            || rest.strip_prefix(':').is_some_and(|port| {
                !port.is_empty()
                    && port.chars().all(|character| character.is_ascii_digit())
            })
    }
    origin
        .strip_prefix("http://axolotl-theme.localhost")
        .is_some_and(valid_port_suffix)
        || origin
            .strip_prefix("axolotl-theme://localhost")
            .is_some_and(valid_port_suffix)
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
        Some(origin) => is_allowed_origin(origin) || is_theme_origin(origin),
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

    fn pack_url(pack_id: &str, path: &str) -> String {
        format!("{}/{pack_id}/{path}", theme_origin_for(pack_id))
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
    fn rejects_windows_device_and_trailing_dot_storage_keys() {
        for key in [
            "CON", "con.txt", "Prn", "AUX.log", "NUL", "com1", "LPT9.bin",
            "notes.",
        ] {
            assert!(!valid_storage_key(key), "{key}");
        }
        for key in
            ["console", "con-1", "com10", "lpt0", "nul_.txt", "notes.txt"]
        {
            assert!(valid_storage_key(key), "{key}");
        }
    }

    #[test]
    fn serves_a_pack_asset_without_an_origin() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(&pack_url("demo", "styles/theme.css"), None),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/css; charset=utf-8"
        );
        assert_eq!(response.body(), b".x { color: red; }");
    }

    #[test]
    fn rejects_requests_on_another_packs_port() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(
                &format!(
                    "{}/demo/styles/theme.css",
                    theme_origin_for("another-pack")
                ),
                None,
            ),
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn page_permissions_gate_bridge_methods() {
        let owner = ThemePageOwner {
            pack_id: "demo".to_string(),
            requires: vec![ThemePackPermission::Storage],
        };
        assert!(
            require_page_permission(&owner, ThemePackPermission::Storage)
                .is_ok()
        );
        let error =
            require_page_permission(&owner, ThemePackPermission::Network)
                .unwrap_err();
        assert!(error.to_string().contains("network"), "{error}");
    }

    #[test]
    fn every_pack_gets_a_stable_origin() {
        assert_eq!(theme_origin_for("demo"), theme_origin_for("demo"));
        assert_ne!(theme_origin_for("demo"), theme_origin_for("other"));
        let port = theme_pack_port("demo");
        assert!((20_000..60_000).contains(&port));
        // Must match `themePackPort` in theme-packs.ts.
        assert_eq!(port, 49_814);
    }

    #[test]
    fn navigation_must_stay_on_the_pack_origin() {
        let origin = url::Url::parse(&theme_origin_for("demo")).unwrap();
        let page = url::Url::parse(&format!(
            "{}/demo/pages/home.html",
            theme_origin_for("demo")
        ))
        .unwrap();
        assert!(is_allowed_page_location(
            &page,
            &origin,
            "/demo/pages/home.html"
        ));
        let remote =
            url::Url::parse("https://attacker.example/demo/pages/home.html")
                .unwrap();
        assert!(!is_allowed_page_location(
            &remote,
            &origin,
            "/demo/pages/home.html"
        ));
        let other = url::Url::parse(&format!(
            "{}/demo/pages/home.html",
            theme_origin_for("other")
        ))
        .unwrap();
        assert!(!is_allowed_page_location(
            &other,
            &origin,
            "/demo/pages/home.html"
        ));
        let query = url::Url::parse(&format!(
            "{}/demo/pages/home.html?next=https://attacker.example",
            theme_origin_for("demo")
        ))
        .unwrap();
        assert!(!is_allowed_page_location(
            &query,
            &origin,
            "/demo/pages/home.html"
        ));
    }

    #[test]
    fn rejects_opaque_origins_for_pack_assets() {
        let directory = tempfile::tempdir().unwrap();
        let page_dir = directory.path().join("themes/demo/pages");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(page_dir.join("home.html"), "<!doctype html>").unwrap();
        let storage = ThemePackStorage::new(directory.path());
        let response = protocol_response_for(
            &storage,
            &request(&pack_url("demo", "pages/home.html"), Some("null")),
        );
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn serves_pack_pages_with_an_ipc_safe_csp() {
        let directory = tempfile::tempdir().unwrap();
        let page_dir = directory.path().join("themes/demo/pages");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(
            page_dir.join("home.html"),
            "<!doctype html><script src=\"../scripts/home.js\"></script>",
        )
        .unwrap();
        let storage = ThemePackStorage::new(directory.path());
        let origin = theme_origin_for("demo");
        let response = protocol_response_for(
            &storage,
            &request(&pack_url("demo", "pages/home.html"), Some(&origin)),
        );
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains(&format!("default-src {origin};")), "{csp}");
        // Pack pages run in a dedicated child webview whose capability only
        // allows the page bridge, so they keep their real origin instead of
        // relying on an iframe CSP sandbox.
        assert!(!csp.contains("sandbox"), "{csp}");
        assert!(!csp.contains("allow-same-origin"), "{csp}");
        assert!(csp.contains("connect-src 'none'"), "{csp}");
        assert!(csp.contains(&format!("script-src {origin}")), "{csp}");
        assert!(csp.contains("frame-src 'none'"), "{csp}");
        assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            origin.as_str()
        );
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }

    #[test]
    fn sandboxes_svg_documents() {
        let directory = tempfile::tempdir().unwrap();
        let assets = directory.path().join("themes/demo/assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("icon.svg"), "<svg></svg>").unwrap();
        let storage = ThemePackStorage::new(directory.path());
        let response = protocol_response_for(
            &storage,
            &request(&pack_url("demo", "assets/icon.svg"), None),
        );
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("sandbox"), "{csp}");
        assert!(csp.contains("default-src 'none'"), "{csp}");
    }

    #[test]
    fn refuses_a_foreign_origin() {
        let (_directory, storage) = storage_with_css();
        let response = protocol_response_for(
            &storage,
            &request(
                &pack_url("demo", "styles/theme.css"),
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
                    &format!(
                        "{}/{}",
                        theme_origin_for("demo"),
                        path.trim_start_matches('/')
                    ),
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
                &format!("{}/../styles/theme.css", theme_origin_for("demo")),
                None,
            ),
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    fn snapshot() -> ThemePackSnapshot {
        let mut tokens_light = BTreeMap::new();
        tokens_light.insert("--surface-1".to_string(), "#ebebeb".to_string());
        tokens_light.insert("--surface-3".to_string(), "#f8f8f8".to_string());
        tokens_light
            .insert("--color-text-primary".to_string(), "#1a202c".to_string());
        let mut tokens_dark = BTreeMap::new();
        tokens_dark.insert("--surface-1".to_string(), "#101215".to_string());
        tokens_dark.insert("--surface-3".to_string(), "#262b34".to_string());
        tokens_dark
            .insert("--color-text-primary".to_string(), "#ffffff".to_string());
        ThemePackSnapshot {
            schema_version: 1,
            source: "builtin".to_string(),
            theme: "dark".to_string(),
            accent_color: Some("green".to_string()),
            custom_theme_colors: serde_json::json!({
                "light": { "bg": "#ffffff" },
            }),
            tokens_light,
            tokens_dark,
            pack_id: None,
        }
    }

    fn archive_names(path: &Path) -> Vec<String> {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|index| {
                archive
                    .by_index(index)
                    .unwrap()
                    .name()
                    .unwrap()
                    .into_owned()
            })
            .collect()
    }

    fn archive_text(path: &Path, name: &str) -> String {
        use std::io::Read as _;
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }

    #[test]
    fn builds_a_valid_snapshot_manifest() {
        let manifest =
            snapshot_manifest(&snapshot(), 1_700_000_000, 42).unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.id, "snapshot-1700000000-42");
        assert_eq!(
            manifest.name.get("en-US").map(String::as_str),
            Some("Appearance snapshot")
        );
        let next = snapshot_manifest(&snapshot(), 1_700_000_000, 43).unwrap();
        assert_ne!(manifest.id, next.id);
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(
            manifest
                .tokens
                .dark
                .get("--color-text-primary")
                .map(String::as_str),
            Some("#ffffff")
        );
    }

    #[test]
    fn writes_a_snapshot_archive() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("snapshot.zip");
        export_snapshot_archive(
            &destination,
            &snapshot(),
            None,
            None,
            ThemePackLimits::default(),
        )
        .unwrap();
        let names = archive_names(&destination);
        assert!(names.contains(&"manifest.json".to_string()), "{names:?}");
        assert!(names.contains(&"origin.json".to_string()), "{names:?}");
        assert!(
            archive_text(&destination, "manifest.json")
                .contains("\"schemaVersion\": 1")
        );
        let origin = archive_text(&destination, "origin.json");
        assert!(origin.contains("\"kind\": \"builtin\""));
        assert!(origin.contains("\"theme\": \"dark\""));
        assert!(origin.contains("\"exportedAt\""));
    }

    #[test]
    fn replaces_an_existing_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("snapshot.zip");
        std::fs::write(&destination, b"existing").unwrap();
        export_snapshot_archive(
            &destination,
            &snapshot(),
            None,
            None,
            ThemePackLimits::default(),
        )
        .unwrap();
        let names = archive_names(&destination);
        assert!(names.contains(&"manifest.json".to_string()), "{names:?}");
        assert_ne!(std::fs::read(&destination).unwrap(), b"existing");
    }

    #[test]
    fn copies_a_pack_and_skips_staging() {
        let directory = tempfile::tempdir().unwrap();
        let pack = directory.path().join("themes/aurora");
        std::fs::create_dir_all(pack.join("assets")).unwrap();
        std::fs::create_dir_all(pack.join(".staging")).unwrap();
        std::fs::create_dir_all(pack.join("data")).unwrap();
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "id": "aurora",
            "name": { "en-US": "Aurora" },
            "version": "2.1.0",
            "engines": { "axolotl": ">=1.0.0" },
            "capabilities": ["tokens"],
            "tokens": {
                "light": {
                    "--surface-1": "#ffffff",
                    "--surface-3": "#ffffff",
                    "--color-text-primary": "#000000"
                },
                "dark": {
                    "--surface-1": "#000000",
                    "--surface-3": "#000000",
                    "--color-text-primary": "#ffffff"
                }
            }
        });
        std::fs::write(
            pack.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(pack.join("assets/theme.css"), ".x { color: red; }")
            .unwrap();
        std::fs::write(pack.join(".staging/temp.txt"), "staging").unwrap();
        std::fs::write(pack.join("data/data.json"), "{}").unwrap();
        std::fs::write(pack.join("origin.json"), "{\"old\":true}").unwrap();
        let destination = directory.path().join("pack.zip");
        export_snapshot_archive(
            &destination,
            &snapshot(),
            Some(&pack),
            Some(&pack.join("data")),
            ThemePackLimits::default(),
        )
        .unwrap();
        let names = archive_names(&destination);
        assert!(names.contains(&"manifest.json".to_string()), "{names:?}");
        assert!(names.contains(&"assets/theme.css".to_string()), "{names:?}");
        assert!(names.contains(&"origin.json".to_string()), "{names:?}");
        assert!(!names.iter().any(|name| name.starts_with(".staging")));
        assert!(!names.iter().any(|name| name.starts_with("data/")));
        let origin = archive_text(&destination, "origin.json");
        assert!(origin.contains("\"kind\": \"pack\""));
        assert!(origin.contains("\"packId\": \"aurora\""));
        assert!(origin.contains("\"version\": \"2.1.0\""));
        assert!(!origin.contains("\"old\""));
    }

    #[test]
    fn encode_url_segment_escapes_reserved_characters() {
        assert_eq!(encode_url_segment("My Skin#1.html"), "My%20Skin%231.html");
    }

    #[test]
    fn theme_pages_have_an_isolated_capability() {
        for source in [
            include_str!("../../capabilities/app.json"),
            include_str!("../../capabilities/core.json"),
            include_str!("../../capabilities/plugins.json"),
            include_str!("../../capabilities/updater.json"),
        ] {
            let capability: serde_json::Value =
                serde_json::from_str(source).unwrap();
            assert_eq!(capability["webviews"], serde_json::json!(["main"]));
            assert!(capability.get("windows").is_none());
        }

        let app: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/app.json"))
                .unwrap();
        assert_eq!(app["permissions"], serde_json::json!(["default"]));

        let capability: serde_json::Value = serde_json::from_str(include_str!(
            "../../capabilities/theme-pages.json"
        ))
        .unwrap();
        assert_eq!(capability["webviews"], serde_json::json!(["theme-page-*"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["theme-pack:allow-theme-pack-page-request"])
        );

        for source in [
            include_str!("../../tauri.conf.json"),
            include_str!("../../tauri-release.conf.json"),
        ] {
            let config: serde_json::Value =
                serde_json::from_str(source).unwrap();
            let enabled = config["app"]["security"]["capabilities"]
                .as_array()
                .unwrap();
            for capability in ["app", "theme-pages"] {
                assert!(
                    enabled.iter().any(|entry| entry == capability),
                    "the {capability} capability must be enabled in {source}"
                );
            }
        }
    }

    #[test]
    fn main_webview_cannot_frame_theme_pages() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json"))
                .unwrap();
        for key in ["csp", "devCsp"] {
            let frame = config["app"]["security"][key]["frame-src"]
                .as_str()
                .expect("the CSP must declare frame-src");
            assert!(
                !frame.contains("axolotl-theme"),
                "theme pages are hosted in a child webview, not framed: {frame}"
            );
        }
    }
}
