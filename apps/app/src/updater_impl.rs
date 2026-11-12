use crate::api::Result;
use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::http::HeaderValue;
use tauri::http::header::ACCEPT;
use tauri::{Manager, ResourceId, Runtime, Webview};
use tauri_plugin_http::reqwest;
use tauri_plugin_updater::{Error, Update, UpdaterExt};
use theseus::fetch::{
    DownloadRequest, FetchProgressFn, Integrity, ResourceClass,
    download_to_path,
};
use theseus::{
    LoadingBarType, emit_loading, init_loading, launcher_user_agent,
};
use tokio::time::Instant;
use url::Url;

const UPDATE_SERVER_LATEST_URL: &str = "https://update.axlmc.org/latest";
const UPDATE_SERVER_API: &str = "https://update.axlmc.org/api/versions";
const UPDATE_ARTIFACT_BASE: &str = "https://dist.axlmc.org/";

// Bound the whole transfer, including retries and range requests.
const UPDATE_DOWNLOAD_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(15 * 60);

// ── Shared types ─────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    rid: ResourceId,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    force_update: bool,
    raw_json: serde_json::Value,
}

#[derive(Default)]
pub struct PendingUpdateData(pub Mutex<Option<PendingUpdate>>);

struct CheckedUpdate {
    update: Update,
    portable: Option<UpdateArtifact>,
}

impl tauri::Resource for CheckedUpdate {}

pub enum PendingUpdate {
    Installer(Arc<Update>, Vec<u8>),
    #[cfg(windows)]
    Portable {
        version: String,
        downloaded_bytes: usize,
        prepared: crate::portable_update::PreparedUpdate,
    },
}

impl PendingUpdate {
    pub fn downloaded_bytes(&self) -> usize {
        match self {
            Self::Installer(_, data) => data.len(),
            #[cfg(windows)]
            Self::Portable {
                downloaded_bytes, ..
            } => *downloaded_bytes,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Installer(update, _) => &update.version,
            #[cfg(windows)]
            Self::Portable { version, .. } => version,
        }
    }

    pub fn is_portable(&self) -> bool {
        match self {
            #[cfg(windows)]
            Self::Portable { .. } => true,
            _ => false,
        }
    }

    pub fn install(self, restart: bool) -> std::result::Result<(), String> {
        match self {
            Self::Installer(update, data) if is_apt_linux() => {
                tauri::async_runtime::block_on(install_apt_package(
                    &update.version,
                    &data,
                ))
                .map_err(|error| error.to_string())
            }
            Self::Installer(update, data) => (*update)
                .clone()
                .restart_after_install(restart)
                .install(data)
                .map_err(|error| error.to_string()),
            #[cfg(windows)]
            Self::Portable { prepared, .. } => prepared.start(restart),
        }
    }
}

// ── Update Server API types ─────────────────────────────────────

#[derive(Deserialize)]
struct VersionsResponse {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    version: String,
    artifacts: Vec<ArtifactEntry>,
}

#[derive(Deserialize)]
struct ArtifactEntry {
    kind: String,
    #[serde(default)]
    variant: Option<String>,
    platform: String,
    architecture: String,
    relative_path: String,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    signature: Option<String>,
}

/// Catalog integrity also allows the shared downloader to recover ranges.
#[derive(Clone)]
struct UpdateArtifact {
    url: Url,
    sha256: String,
    size: u64,
    signature: Option<String>,
}

fn apt_deb_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        arch => Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            format!("Unsupported architecture for apt updates: {arch}"),
        ))
        .into()),
    }
}

async fn fetch_update_artifact(
    version: &str,
    download_url: Option<&Url>,
) -> Result<UpdateArtifact> {
    let release = fetch_update_release(version).await?;
    select_update_artifact(&release, download_url)
}

async fn fetch_update_release(version: &str) -> Result<VersionEntry> {
    let response = theseus::http_client_builder()
        .user_agent(launcher_user_agent())
        .timeout(UPDATE_DOWNLOAD_TIMEOUT)
        .build()?
        .get(UPDATE_SERVER_API)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Error::Network(format!(
            "Failed to fetch update catalog: {}",
            response.status()
        ))
        .into());
    }

    let catalog: VersionsResponse = response.json().await?;
    let release = catalog
        .versions
        .into_iter()
        .find(|entry| entry.version == version)
        .ok_or_else(|| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Update catalog has no entry for version {version}"
            )))
        })?;

    Ok(release)
}

fn select_update_artifact(
    release: &VersionEntry,
    download_url: Option<&Url>,
) -> Result<UpdateArtifact> {
    let version = &release.version;
    let arch = std::env::consts::ARCH;
    let artifact = release
        .artifacts
        .iter()
        .find(|entry| {
            if let Some(download_url) = download_url {
                return update_artifact_url(&entry.relative_path)
                    .is_ok_and(|url| url == *download_url);
            }
            entry.kind == "installer"
                && entry.variant.as_deref() == Some("deb")
                && entry.platform == "linux"
                && entry.architecture == arch
        })
        .ok_or_else(|| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Update catalog has no matching artifact for {version} on {arch}"
            )))
        })?;

    let sha256 = artifact.sha256.clone().ok_or_else(|| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Update catalog has no sha256 for the artifact of {version}"
        )))
    })?;

    let url = update_artifact_url(&artifact.relative_path)?;

    Ok(UpdateArtifact {
        url,
        sha256,
        size: artifact.size,
        signature: artifact.signature.clone(),
    })
}

fn select_portable_artifact(
    release: &VersionEntry,
    arch: &str,
) -> Result<UpdateArtifact> {
    let expected =
        format!("Axolotl_Launcher_{}_x64_portable.zip", release.version);
    let mut matches = release.artifacts.iter().filter(|entry| {
        arch == "x86_64"
            && entry.kind == "portable"
            && entry.variant.as_deref() == Some("portable")
            && entry.platform == "windows"
            && entry.architecture == arch
            && entry.relative_path.trim_start_matches('/')
                == format!("dist/{}/{}", release.version, expected)
    });
    let entry = matches.next().ok_or_else(|| Error::Network("This release has no matching portable update. Please update the portable package manually.".into()))?;
    if matches.next().is_some()
        || entry.size == 0
        || !entry.sha256.as_ref().is_some_and(|hash| {
            hash.len() == 64
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(Error::Network(
            "Invalid portable update integrity metadata".into(),
        )
        .into());
    }
    Ok(UpdateArtifact {
        url: update_artifact_url(&entry.relative_path)?,
        sha256: entry.sha256.clone().unwrap(),
        size: entry.size,
        signature: entry.signature.clone(),
    })
}

async fn portable_signature(asset: &UpdateArtifact) -> Result<String> {
    if let Some(signature) = &asset.signature {
        if signature.trim().len() >= 32 {
            return Ok(signature.trim().to_owned());
        }
        return Err(
            Error::Network("Invalid portable update signature".into()).into()
        );
    }
    let mut response = theseus::http_client_builder()
        .user_agent(launcher_user_agent())
        .timeout(UPDATE_DOWNLOAD_TIMEOUT)
        .build()?
        .get(format!("{}.sig", asset.url))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Network("This release has no signed portable package. Please update the portable package manually.".into()).into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > 16_384 {
            return Err(Error::Network(
                "Portable update signature is too large".into(),
            )
            .into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes)
        .map(|value| value.trim().to_owned())
        .map_err(|_| {
            Error::Network("Invalid portable update signature encoding".into())
                .into()
        })
}

// ── Updater plugin helpers ───────────────────────────────────────

fn update_channel(channel: &str) -> Result<&str> {
    match channel {
        "release" | "beta" => Ok(channel),
        _ => Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            format!("Unknown update channel: {channel}"),
        ))
        .into()),
    }
}

fn update_platform() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("windows-x86_64"),
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("linux", "aarch64") => Ok("linux-aarch64"),
        ("macos", "x86_64") => Ok("darwin-x86_64"),
        ("macos", "aarch64") => Ok("darwin-aarch64"),
        (os, arch) => {
            Err(theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Unsupported updater platform: {os}-{arch}"
            )))
            .into())
        }
    }
}

fn update_endpoint() -> Result<Url> {
    Url::parse(UPDATE_SERVER_LATEST_URL).map_err(|error| {
        theseus::Error::from(theseus::ErrorKind::OtherError(error.to_string()))
            .into()
    })
}

fn update_artifact_url(relative_path: &str) -> Result<Url> {
    let relative_path =
        relative_path.strip_prefix('/').unwrap_or(relative_path);
    if !relative_path.starts_with("dist/") {
        return Err(
            Error::Network("Invalid update artifact path".to_string()).into()
        );
    }
    let base = Url::parse(UPDATE_ARTIFACT_BASE)
        .map_err(|error| Error::Network(error.to_string()))?;
    let url = base
        .join(relative_path)
        .map_err(|error| Error::Network(error.to_string()))?;
    if url.origin() != base.origin()
        || !url.path().starts_with("/dist/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            Error::Network("Invalid update artifact path".to_string()).into()
        );
    }
    Ok(url)
}

/// The Windows updater launches the NSIS installer and exits with
/// `std::process::exit(0)` without returning to the `Exit` handler. The
/// plugin's hook is the last point where the database can be closed cleanly.
fn close_database_before_installer() {
    if let Some(state) = theseus::State::get_if_initialized()
        && let Err(error) =
            tauri::async_runtime::block_on(state.finish_database_session())
    {
        tracing::warn!(
            %error,
            "Could not record a clean database shutdown before the Windows installer"
        );
    }
}

/// Build the platform-updater with the given endpoints and run a check.
async fn check_with_endpoints<R: Runtime>(
    webview: &Webview<R>,
    channel: &str,
) -> Result<Option<Update>> {
    let channel = update_channel(channel)?;
    let platform = update_platform()?;
    let current_version =
        webview.app_handle().package_info().version.to_string();
    let app_handle = webview.app_handle().clone();
    let mut updater = webview
        .updater_builder()
        .endpoints(vec![update_endpoint()?])?
        .header("Accept", "application/json")?
        .header("X-Axolotl-Channel", channel)?
        .header("X-Axolotl-Platform", platform)?
        .header("X-Axolotl-Version", current_version)?
        .on_before_exit(move || {
            close_database_before_installer();
            // The plugin's default hook clears Tauri's resource tables before
            // the installer takes over. Replacing it to close the database
            // would drop that cleanup; run it last, after which no Tauri API
            // may be used.
            app_handle.cleanup_before_exit();
        });

    #[cfg(target_os = "windows")]
    {
        let install_dir = std::env::current_exe()
            .map_err(|error| {
                theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                    "Failed to resolve current executable: {error}"
                )))
            })?
            .parent()
            .ok_or_else(|| {
                theseus::Error::from(theseus::ErrorKind::OtherError(
                    "Current executable has no parent directory".to_string(),
                ))
            })?
            .to_path_buf();

        tracing::debug!(
            install_dir = %install_dir.display(),
            "Using current executable directory for Windows app updates"
        );
        updater = updater.installer_arg(format!(
            "/INSTALL_DIR=\"{}\"",
            install_dir.display()
        ));
    }

    let updater = updater.build()?;
    updater.check().await.map_err(Into::into)
}

/// Check the updater manifest through the configured Update Server endpoint.
async fn check_with_updater<R: Runtime>(
    webview: &Webview<R>,
    channel: &str,
) -> Result<Option<UpdateMetadata>> {
    let Some(mut update) = check_with_endpoints(webview, channel).await? else {
        return Ok(None);
    };
    update.timeout = Some(UPDATE_DOWNLOAD_TIMEOUT);

    // On Debian and derivatives the plugin's minisign signature check cannot
    // validate the unsigned .deb, so point the download at the deb from the
    // Update Server catalog instead of the AppImage artifact. Its integrity
    // is verified with the catalog's sha256/size during the download.
    if is_apt_linux() {
        update.download_url =
            fetch_update_artifact(&update.version, None).await?.url;
    } else {
        update.download_url = update_artifact_url(update.download_url.path())?;
    }

    let published_at = update
        .raw_json
        .get("published_at")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let force_update = update
        .raw_json
        .get("force_update")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let portable = if cfg!(windows) && crate::portable::is_portable_mode() {
        Some(select_portable_artifact(
            &fetch_update_release(&update.version).await?,
            std::env::consts::ARCH,
        )?)
    } else {
        None
    };
    let metadata = UpdateMetadata {
        rid: webview.resources_table().add(CheckedUpdate {
            update: update.clone(),
            portable,
        }),
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date: None,
        body: update.body.clone(),
        published_at,
        force_update,
        raw_json: update.raw_json,
    };

    Ok(Some(metadata))
}

// ── Tauri commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn check_app_update<R: Runtime>(
    webview: Webview<R>,
    channel: String,
) -> Result<Option<UpdateMetadata>> {
    check_with_updater(&webview, &channel).await
}

// Reimplementation of Update::download mostly, minus the actual download part
#[tauri::command]
pub async fn get_update_size<R: Runtime>(
    webview: Webview<R>,
    rid: ResourceId,
) -> Result<Option<u64>> {
    let checked = webview.resources_table().get::<CheckedUpdate>(rid)?;
    if let Some(asset) = &checked.portable {
        return Ok(Some(asset.size));
    }
    let update = &checked.update;

    let mut headers = update.headers.clone();
    if !headers.contains_key(ACCEPT) {
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/octet-stream"),
        );
    }

    let mut request =
        theseus::http_client_builder().user_agent(launcher_user_agent());
    if let Some(timeout) = update.timeout {
        request = request.timeout(timeout);
    }
    if let Some(ref proxy) = update.proxy {
        let proxy = reqwest::Proxy::all(proxy.as_str())?;
        request = request.proxy(proxy);
    }
    let response = request
        .build()?
        .head(update.download_url.clone())
        .headers(headers)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Error::Network(format!(
            "Download request failed with status: {}",
            response.status()
        ))
        .into());
    }

    let content_length = response
        .headers()
        .get("Content-Length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok());

    Ok(content_length)
}

async fn download_update_artifact(
    asset: &UpdateArtifact,
    progress: &mut FetchProgressFn<'_>,
) -> Result<Vec<u8>> {
    let state = theseus::State::get().await?;
    let directory = tempfile::tempdir().map_err(Error::Io)?;
    let destination = directory.path().join("update");
    let request =
        DownloadRequest::new(asset.url.as_str(), ResourceClass::Other)
            .with_integrity(Integrity {
                size: Some(asset.size),
                sha256: Some(asset.sha256.clone()),
                ..Integrity::default()
            })
            .with_large_file_download_policy();
    tokio::time::timeout(
        UPDATE_DOWNLOAD_TIMEOUT,
        download_to_path(
            request,
            &destination,
            &state.download_semaphore,
            Some(progress),
        ),
    )
    .await
    .map_err(|_| Error::Network("Update download timed out".into()))??;
    let bytes = tokio::fs::read(&destination).await.map_err(Error::Io)?;
    // Catalog size is an exact update artifact constraint, even when a hash
    // matches (the shared downloader permits stale sizes for other resources).
    if bytes.len() as u64 != asset.size {
        return Err(Error::Network(format!(
            "Downloaded update size mismatch: expected {}, got {}",
            asset.size,
            bytes.len()
        ))
        .into());
    }
    Ok(bytes)
}

fn verify_update_signature(
    data: &[u8],
    signature: &str,
    public_key: &str,
) -> std::result::Result<(), Error> {
    let decode = |value: &str| -> std::result::Result<String, Error> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(value)?;
        String::from_utf8(bytes)
            .map_err(|_| Error::SignatureUtf8(value.to_owned()))
    };
    let public_key = PublicKey::decode(&decode(public_key)?)?;
    let signature = Signature::decode(&decode(signature)?)?;
    public_key.verify(data, &signature, true)?;
    Ok(())
}

#[tauri::command]
pub async fn enqueue_update_for_installation<R: Runtime>(
    webview: Webview<R>,
    rid: ResourceId,
) -> Result<()> {
    let pending_data = webview.state::<PendingUpdateData>().inner();

    let checked = webview.resources_table().get::<CheckedUpdate>(rid)?;
    let update = &checked.update;

    let progress = init_loading(
        LoadingBarType::LauncherUpdate {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
        },
        1.0,
        "Downloading update...",
    )
    .await?;

    let download_start = Instant::now();
    let apt = is_apt_linux();
    let asset = if let Some(asset) = &checked.portable {
        asset.clone()
    } else {
        fetch_update_artifact(
            &update.version,
            if apt {
                None
            } else {
                Some(&update.download_url)
            },
        )
        .await?
    };
    let signature = if checked.portable.is_some() {
        portable_signature(&asset).await?
    } else {
        update.signature.clone()
    };
    let mut reported_fraction = 0.0_f64;
    let mut download_progress: Box<FetchProgressFn<'_>> =
        Box::new(|bytes, total| {
            // The downloader reports cumulative bytes, including after retries.
            // Keep the bar monotonic and reserve completion for verification.
            let fraction = if total > 0 {
                (bytes as f64 / total as f64).clamp(0.0, 1.0) * 0.95
            } else {
                reported_fraction
            };
            let increment = (fraction - reported_fraction).max(0.0);
            reported_fraction += increment;
            if let Err(error) = emit_loading(&progress, increment, None) {
                tracing::error!(
                    "Failed to update download progress bar: {error}"
                );
            }
            Box::pin(async { Ok(()) })
        });
    let update_data =
        download_update_artifact(&asset, &mut download_progress).await?;
    drop(download_progress);

    if !apt {
        let public_key = webview
            .app_handle()
            .config()
            .plugins
            .0
            .get("updater")
            .and_then(|config| config.get("pubkey"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                Error::Network("Missing updater public key".into())
            })?;
        verify_update_signature(&update_data, &signature, public_key)?;
    }
    let download_duration = download_start.elapsed();
    tracing::info!("Downloaded update in {download_duration:?}");

    #[cfg(windows)]
    let pending = if checked.portable.is_some() {
        let executable = std::env::current_exe().map_err(Error::Io)?;
        let downloaded_bytes = update_data.len();
        let prepared = tokio::task::spawn_blocking(move || {
            crate::portable_update::prepare(&update_data, executable)
        })
        .await
        .map_err(|error| Error::Network(error.to_string()))?
        .map_err(Error::Network)?;
        PendingUpdate::Portable {
            version: update.version.clone(),
            downloaded_bytes,
            prepared,
        }
    } else {
        PendingUpdate::Installer(Arc::new(update.clone()), update_data)
    };
    #[cfg(not(windows))]
    let pending =
        PendingUpdate::Installer(Arc::new(update.clone()), update_data);
    pending_data.0.lock().unwrap().replace(pending);

    Ok(())
}

#[tauri::command]
pub fn remove_enqueued_update<R: Runtime>(webview: Webview<R>) {
    let pending_data = webview.state::<PendingUpdateData>().inner();
    pending_data.0.lock().unwrap().take();
}

// ── Debian / derivatives apt update ─────────────────────────────

/// Whether this Linux system updates Axolotl through apt (Debian and its
/// derivatives) and has `pkexec` available for a single privileged prompt.
#[tauri::command]
pub fn is_apt_linux() -> bool {
    #[cfg(target_os = "linux")]
    {
        let debian_like = std::path::Path::new("/etc/debian_version").exists()
            || std::path::Path::new("/etc/apt").is_dir()
            || std::path::Path::new("/usr/bin/apt-get").exists();
        let has_pkexec = ["/usr/bin/pkexec", "/bin/pkexec"]
            .iter()
            .any(|path| std::path::Path::new(path).exists());
        debian_like && has_pkexec
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Install the downloaded .deb on Debian and its derivatives, prompting for
/// root once via `pkexec`. The package is installed from the absolute path
/// of a temporary file, which is removed afterwards.
pub async fn install_apt_package(version: &str, data: &[u8]) -> Result<()> {
    if !is_apt_linux() {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "apt updates are only supported on Debian-based Linux systems with pkexec"
                .to_string(),
        ))
        .into());
    }

    let arch = apt_deb_arch()?;
    let deb_path = std::env::temp_dir()
        .join(format!("Axolotl.Launcher_{version}_{arch}.deb"));
    std::fs::write(&deb_path, data).map_err(|io| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to write the downloaded deb: {io}"
        )))
    })?;
    let _deb_cleanup = TempDebFile(deb_path.clone());

    let install_path = deb_path.clone();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("pkexec")
            .arg("apt")
            .arg("install")
            .arg("-y")
            .arg(&install_path)
            .output()
    })
    .await
    .map_err(|join| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to run the apt updater: {join}"
        )))
    })?
    .map_err(|io| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to start pkexec: {io}"
        )))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            format!("apt install failed: {}", stderr.trim()),
        ))
        .into());
    }

    Ok(())
}

/// Removes a temporary Debian package when installation finishes or fails.
///
/// The installer awaits a blocking task and has several fallible operations
/// after creating the file. Keeping cleanup in `Drop` makes every return path
/// (including task and process-launch errors) remove the package.
struct TempDebFile(std::path::PathBuf);

impl Drop for TempDebFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(
                path = %self.0.display(),
                error = %error,
                "Failed to remove temporary deb file"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UPDATE_SERVER_API, VersionEntry, select_portable_artifact,
        select_update_artifact, update_artifact_url, update_endpoint,
        verify_update_signature,
    };
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[test]
    fn portable_selection_requires_its_own_exact_versioned_package() {
        let portable = serde_json::json!({
            "kind": "portable", "variant": "portable", "platform": "windows", "architecture": "x86_64",
            "relative_path": "dist/1.11.0/Axolotl_Launcher_1.11.0_x64_portable.zip",
            "sha256": "a".repeat(64), "size": 123, "signature": "portable signature"
        });
        let release = |entry| {
            serde_json::from_value::<VersionEntry>(serde_json::json!({
            "version": "1.11.0", "artifacts": [
                { "kind": "updater", "variant": "tauri", "platform": "windows", "architecture": "x86_64",
                  "relative_path": "dist/1.11.0/setup.nsis.zip", "sha256": "b".repeat(64), "size": 5, "signature": "setup signature" }, entry
            ]
        })).unwrap()
        };
        let selected =
            select_portable_artifact(&release(portable.clone()), "x86_64")
                .unwrap();
        assert_eq!(selected.signature.as_deref(), Some("portable signature"));
        assert_eq!(selected.size, 123);
        assert!(selected.url.path().ends_with("_portable.zip"));
        assert!(
            select_portable_artifact(&release(portable.clone()), "aarch64")
                .is_err()
        );
        for (key, value) in [
            ("kind", serde_json::json!("installer")),
            ("variant", serde_json::json!("modern")),
            ("platform", serde_json::json!("linux")),
            ("architecture", serde_json::json!("aarch64")),
            ("sha256", serde_json::json!("not a hash")),
            ("size", serde_json::json!(0)),
            (
                "relative_path",
                serde_json::json!(
                    "dist/1.10.0/Axolotl_Launcher_1.10.0_x64_portable.zip"
                ),
            ),
        ] {
            let mut bad = portable.clone();
            bad[key] = value;
            assert!(
                select_portable_artifact(&release(bad), "x86_64").is_err(),
                "accepted invalid {key}"
            );
        }
        let mut unsigned = portable;
        unsigned.as_object_mut().unwrap().remove("signature");
        let historical =
            select_portable_artifact(&release(unsigned), "x86_64").unwrap();
        assert!(historical.signature.is_none());
        assert!(historical.url.path().ends_with("_portable.zip"));
    }

    #[test]
    fn signed_update_still_requires_a_valid_minisign_signature() {
        // Public test vector from minisign-verify.
        let key = STANDARD.encode(
            "untrusted comment: minisign public key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3",
        );
        let signature = STANDARD.encode(
            "untrusted comment: signature from minisign secret key\n\
             RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\n\
             trusted comment: timestamp:1633700835\tfile:test\tprehashed\n\
             wLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==",
        );
        verify_update_signature(b"test", &signature, &key).unwrap();
        assert!(
            verify_update_signature(b"tampered", &signature, &key).is_err()
        );
        assert!(verify_update_signature(b"test", "invalid", &key).is_err());
        assert!(
            verify_update_signature(b"test", &signature, "invalid").is_err()
        );
    }

    #[test]
    fn update_integrity_is_selected_by_the_exact_distribution_url() {
        let release: VersionEntry = serde_json::from_value(serde_json::json!({
            "version": "1.10.0",
            "artifacts": [
                {"kind": "installer", "platform": "windows", "architecture": "x86_64",
                 "relative_path": "dist/1.10.0/setup.exe", "sha256": "a".repeat(64), "size": 10},
                {"kind": "updater", "platform": "windows", "architecture": "x86_64",
                 "relative_path": "dist/1.10.0/setup.nsis.zip", "sha256": "b".repeat(64), "size": 20}
            ]
        })).unwrap();
        let url = update_artifact_url("dist/1.10.0/setup.nsis.zip").unwrap();
        let selected = select_update_artifact(&release, Some(&url)).unwrap();
        assert_eq!(selected.url, url);
        assert_eq!(selected.size, 20);
        assert_eq!(selected.sha256, "b".repeat(64));
        let missing = update_artifact_url("dist/1.10.0/missing.zip").unwrap();
        assert!(select_update_artifact(&release, Some(&missing)).is_err());
    }

    #[test]
    fn metadata_endpoints_stay_on_update_server() {
        assert_eq!(
            update_endpoint().unwrap().as_str(),
            "https://update.axlmc.org/latest"
        );
        assert_eq!(UPDATE_SERVER_API, "https://update.axlmc.org/api/versions");
    }

    #[test]
    fn relative_artifacts_use_the_distribution_origin() {
        for path in [
            "dist/1.10.0/Axolotl_amd64.deb",
            "/dist/1.10.0/Axolotl_x64-setup.nsis.zip",
        ] {
            let url = update_artifact_url(path).unwrap();
            assert_eq!(url.host_str(), Some("dist.axlmc.org"));
            assert_eq!(url.scheme(), "https");
            assert_eq!(
                url.path().trim_start_matches('/'),
                path.trim_start_matches('/')
            );
        }
    }

    #[test]
    fn manifest_paths_keep_their_encoding_on_distribution() {
        let manifest_url = url::Url::parse(
            "https://update.axlmc.org/dist/1.10.0/Axolotl%20Launcher%25.zip",
        )
        .unwrap();
        assert_eq!(
            update_artifact_url(manifest_url.path()).unwrap().as_str(),
            "https://dist.axlmc.org/dist/1.10.0/Axolotl%20Launcher%25.zip"
        );
    }

    #[test]
    fn artifact_paths_cannot_escape_distribution_or_add_url_parameters() {
        for path in [
            "https://example.com/dist/file.zip",
            "//example.com/dist/file.zip",
            "../file.zip",
            "dist/../../file.zip",
            "dist/1.10.0/file.zip?token=value",
            "dist/1.10.0/file.zip#fragment",
        ] {
            assert!(
                update_artifact_url(path).is_err(),
                "accepted invalid path: {path}"
            );
        }
    }
}
