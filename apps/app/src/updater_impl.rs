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
    install_method: UpdateMethod,
}

#[derive(Default)]
pub struct PendingUpdateData(pub Mutex<Option<PendingUpdate>>);

struct CheckedUpdate {
    update: Update,
    portable: Option<UpdateArtifact>,
    install_method: UpdateMethod,
}

impl tauri::Resource for CheckedUpdate {}

pub enum PendingUpdate {
    Installer(Arc<Update>, Vec<u8>, UpdateMethod),
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
            Self::Installer(_, data, _) => data.len(),
            #[cfg(windows)]
            Self::Portable {
                downloaded_bytes, ..
            } => *downloaded_bytes,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Installer(update, _, _) => &update.version,
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
            Self::Installer(_, _, UpdateMethod::Manual) => {
                Err("This copy of Axolotl Launcher cannot update itself".into())
            }
            Self::Installer(update, data, method) => {
                if let Some(installer) = method.package_installer() {
                    tauri::async_runtime::block_on(install_linux_package(
                        installer, data,
                    ))
                    .map_err(|error| error.to_string())
                } else {
                    (*update)
                        .clone()
                        .restart_after_install(restart)
                        .install(data)
                        .map_err(|error| error.to_string())
                }
            }
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

/// A distribution package that updates an apt or rpm install.
struct PackageUpdate {
    url: Url,
    /// Base64 minisign signature, encoded like the manifest's.
    signature: String,
}

fn catalog_error(message: String) -> crate::api::TheseusSerializableError {
    theseus::Error::from(theseus::ErrorKind::OtherError(message)).into()
}

async fn fetch_update_artifact(
    version: &str,
    download_url: &Url,
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
    download_url: &Url,
) -> Result<UpdateArtifact> {
    let version = &release.version;
    let arch = std::env::consts::ARCH;
    let artifact = release
        .artifacts
        .iter()
        .find(|entry| {
            update_artifact_url(&entry.relative_path)
                .is_ok_and(|url| url == *download_url)
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

/// Selects the package itself; sidecars need not appear in the primary artifact list.
fn select_package_artifact(
    release: &VersionEntry,
    variant: &str,
    arch: &str,
) -> Result<UpdateArtifact> {
    let mut matches = release.artifacts.iter().filter(|entry| {
        entry.kind == "installer"
            && entry.variant.as_deref() == Some(variant)
            && entry.platform == "linux"
            && entry.architecture == arch
    });
    let package = matches.next().ok_or_else(|| {
        catalog_error(format!(
            "Update catalog has no {variant} package of {} for {arch}",
            release.version
        ))
    })?;
    if matches.next().is_some()
        || package.size == 0
        || !package.sha256.as_ref().is_some_and(|hash| {
            hash.len() == 64
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(catalog_error(
            "Invalid Linux package integrity metadata".into(),
        ));
    }
    Ok(UpdateArtifact {
        url: update_artifact_url(&package.relative_path)?,
        sha256: package.sha256.clone().unwrap(),
        size: package.size,
        signature: package.signature.clone(),
    })
}

/// Uses the package's own inline signature or exact distribution sidecar.
async fn fetch_package_update(
    version: &str,
    variant: &str,
) -> Result<PackageUpdate> {
    let release = fetch_update_release(version).await?;
    let package =
        select_package_artifact(&release, variant, std::env::consts::ARCH)?;
    let signature = artifact_signature(&package).await?;
    Ok(PackageUpdate {
        url: package.url,
        signature,
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

const MAX_UPDATE_SIGNATURE_BYTES: usize = 16_384;

fn decode_artifact_signature(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_UPDATE_SIGNATURE_BYTES {
        return Err(
            Error::Network("Update signature is too large".into()).into()
        );
    }
    let signature = std::str::from_utf8(bytes)
        .map_err(|_| {
            Error::Network("Invalid update signature encoding".into())
        })?
        .trim();
    if signature.len() < 32 {
        return Err(Error::Network("Invalid update signature".into()).into());
    }
    Ok(signature.to_owned())
}

async fn artifact_signature(asset: &UpdateArtifact) -> Result<String> {
    if let Some(signature) = &asset.signature {
        return decode_artifact_signature(signature.as_bytes());
    }
    let mut response = theseus::http_client_builder()
        .user_agent(launcher_user_agent())
        .timeout(UPDATE_DOWNLOAD_TIMEOUT)
        .build()?
        .get(format!("{}.sig", asset.url))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(Error::Network("This release has no signed update package. Please update the launcher manually.".into()).into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > MAX_UPDATE_SIGNATURE_BYTES.saturating_sub(bytes.len())
        {
            return Err(
                Error::Network("Update signature is too large".into()).into()
            );
        }
        bytes.extend_from_slice(&chunk);
    }
    decode_artifact_signature(&bytes)
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

    let install_method = *webview.state::<UpdateMethod>();

    // Linux packages carry their own signature; AppImages and other platforms
    // keep the updater manifest signature for their own archive.
    if let Some(installer) = install_method.package_installer() {
        let package =
            fetch_package_update(&update.version, installer.variant).await?;
        update.download_url = package.url;
        update.signature = package.signature;
    } else if install_method != UpdateMethod::Manual {
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
            install_method,
        }),
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date: None,
        body: update.body.clone(),
        published_at,
        force_update,
        raw_json: update.raw_json,
        install_method,
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
    if checked.install_method == UpdateMethod::Manual {
        return Err(catalog_error(
            "This copy of Axolotl Launcher cannot update itself. Download the new version from the website or update it with your package manager.".into(),
        ));
    }
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
    let asset = if let Some(asset) = &checked.portable {
        asset.clone()
    } else {
        fetch_update_artifact(&update.version, &update.download_url).await?
    };
    let signature = if checked.portable.is_some() {
        artifact_signature(&asset).await?
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

    {
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
        PendingUpdate::Installer(
            Arc::new(update.clone()),
            update_data,
            checked.install_method,
        )
    };
    #[cfg(not(windows))]
    let pending = PendingUpdate::Installer(
        Arc::new(update.clone()),
        update_data,
        checked.install_method,
    );
    pending_data.0.lock().unwrap().replace(pending);

    Ok(())
}

#[tauri::command]
pub fn remove_enqueued_update<R: Runtime>(webview: Webview<R>) {
    let pending_data = webview.state::<PendingUpdateData>().inner();
    pending_data.0.lock().unwrap().take();
}

/// How this install applies a launcher update.
///
/// It follows the bundle the running binary was installed from, not the
/// distribution it runs on: an AppImage on Ubuntu replaces itself like on any
/// other distribution, and only a .deb that dpkg tracks goes through apt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMethod {
    /// The updater plugin applies the update (Windows, macOS, AppImage).
    Updater,
    /// apt installs the new .deb over the package that owns this binary.
    Apt,
    /// rpm upgrades the package that owns this binary.
    Rpm,
    /// This copy cannot update itself: a package without the tools to
    /// reinstall it (no pkexec, or a .deb converted for pacman), an unpacked
    /// AppImage, or a build that is not an official bundle (source builds,
    /// Flatpak, distribution packages). The launcher only reports updates.
    Manual,
}

/// Package name of the launcher in both the .deb and the .rpm: the Tauri
/// bundler kebab-cases the product name ("Axolotl Launcher").
const PACKAGE_NAME: &str = "axolotl-launcher";

/// The package manager command that installs an update package.
pub struct PackageInstaller {
    /// Catalog installer variant, which is also the file extension.
    variant: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

/// What the Linux host offers for updating a packaged install.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct LinuxHost {
    /// dpkg lists this executable as a file of [`PACKAGE_NAME`].
    dpkg_owns_exe: bool,
    /// The rpm database lists this executable as a file of a package.
    rpm_owns_exe: bool,
    apt: bool,
    pkexec: bool,
}

impl UpdateMethod {
    /// How a package install applies an update; `None` when the updater
    /// plugin does.
    pub fn package_installer(self) -> Option<PackageInstaller> {
        match self {
            Self::Updater | Self::Manual => None,
            Self::Apt => Some(PackageInstaller {
                variant: "deb",
                program: "apt",
                args: &["install", "-y"],
            }),
            Self::Rpm => Some(PackageInstaller {
                variant: "rpm",
                program: "rpm",
                args: &["--upgrade"],
            }),
        }
    }

    /// Detects the update method once at startup; the install source does
    /// not change while the launcher runs.
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            let bundle = tauri::utils::platform::bundle_type();
            let in_appimage = std::env::var_os("APPIMAGE")
                .is_some_and(|path| !path.is_empty());
            let method =
                linux_update_method(bundle.as_ref(), in_appimage, || {
                    probe_linux_host(bundle.as_ref())
                });
            tracing::info!(
                ?bundle,
                ?method,
                "Selected the launcher update method"
            );
            method
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::Updater
        }
    }
}

/// Picks the update method for the Linux bundle this binary was installed
/// from. `in_appimage` is whether the AppImage runtime named the file to
/// replace (`$APPIMAGE`). The host is only probed for package installs.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_update_method(
    bundle: Option<&tauri::utils::config::BundleType>,
    in_appimage: bool,
    host: impl FnOnce() -> LinuxHost,
) -> UpdateMethod {
    use tauri::utils::config::BundleType;
    match bundle {
        Some(BundleType::AppImage) if in_appimage => UpdateMethod::Updater,
        Some(BundleType::Deb) => {
            let host = host();
            if host.dpkg_owns_exe && host.apt && host.pkexec {
                UpdateMethod::Apt
            } else {
                UpdateMethod::Manual
            }
        }
        Some(BundleType::Rpm) => {
            let host = host();
            if host.rpm_owns_exe && host.pkexec {
                UpdateMethod::Rpm
            } else {
                UpdateMethod::Manual
            }
        }
        _ => UpdateMethod::Manual,
    }
}

/// Whether a dpkg file list (`/var/lib/dpkg/info/<package>.list`) names the
/// executable.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn dpkg_list_contains(list: &str, exe: &std::path::Path) -> bool {
    list.lines().any(|line| std::path::Path::new(line) == exe)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const RPM_PROBE_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(2);
const RPM_PROBE_REAP_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(250);

/// The package database is optional startup metadata. A stalled query must not
/// prevent window creation; the worker owns and reaps only its own probe child.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
async fn bounded_package_probe(
    mut command: tokio::process::Command,
    timeout: std::time::Duration,
) -> bool {
    tokio::spawn(async move {
        let child = command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn();
        let Ok(mut child) = child else {
            return false;
        };
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => status.success(),
            result => {
                // `kill` also waits, so use start_kill followed by a separate
                // bounded reap. Tokio retains reaping responsibility on drop.
                let _ = child.start_kill();
                let _ =
                    tokio::time::timeout(RPM_PROBE_REAP_TIMEOUT, child.wait())
                        .await;
                tracing::warn!(
                    ?result,
                    "Package ownership probe failed; using manual updates"
                );
                false
            }
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn probe_linux_host(
    bundle: Option<&tauri::utils::config::BundleType>,
) -> LinuxHost {
    use tauri::utils::config::BundleType;
    let host_has = |binary: &str| {
        ["/usr/bin", "/bin"]
            .iter()
            .any(|dir| std::path::Path::new(dir).join(binary).exists())
    };
    let exe = std::env::current_exe().ok();
    let dpkg_owns_exe = matches!(bundle, Some(BundleType::Deb))
        && exe.as_deref().is_some_and(|exe| {
            std::fs::read_to_string(format!(
                "/var/lib/dpkg/info/{PACKAGE_NAME}.list"
            ))
            .is_ok_and(|list| dpkg_list_contains(&list, exe))
        });
    let rpm_owns_exe = matches!(bundle, Some(BundleType::Rpm))
        && exe.as_deref().is_some_and(|exe| {
            let mut command = tokio::process::Command::new("rpm");
            command.args(["--query", "--quiet", "--file"]).arg(exe);
            tauri::async_runtime::block_on(bounded_package_probe(
                command,
                RPM_PROBE_TIMEOUT,
            ))
        });
    LinuxHost {
        dpkg_owns_exe,
        rpm_owns_exe,
        apt: host_has("apt"),
        pkexec: host_has("pkexec"),
    }
}

/// Whether this install updates through apt. Kept for the frontend command
/// surface; the decision itself is [`UpdateMethod::detect`].
#[tauri::command]
pub fn is_apt_linux(method: tauri::State<'_, UpdateMethod>) -> bool {
    *method == UpdateMethod::Apt
}

/// Installs the downloaded package over the install its package manager
/// tracks, prompting for root once via `pkexec`. The package is installed
/// from the absolute path of a temporary file, which is removed afterwards.
pub async fn install_linux_package(
    installer: PackageInstaller,
    data: Vec<u8>,
) -> Result<()> {
    let PackageInstaller {
        variant,
        program,
        args,
    } = installer;
    let output = with_staged_package(variant, data, move |install_path| {
        std::process::Command::new("pkexec")
            .arg(program)
            .args(args)
            .arg(install_path)
            .output()
    })
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(catalog_error(format!(
            "{program} {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }

    Ok(())
}

/// Owns the private package until the consumer completes. Keeping creation and
/// installation in one worker also preserves it when the async caller is cancelled.
async fn with_staged_package<T: Send + 'static>(
    variant: &'static str,
    data: Vec<u8>,
    install: impl FnOnce(&std::path::Path) -> std::io::Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(move || {
        let directory = tempfile::Builder::new()
            .prefix("axolotl-package-")
            .tempdir()?;
        let path = directory.path().join(format!("update.{variant}"));
        write_package_exclusively(&path, &data)?;
        install(&path)
    })
    .await
    .map_err(|join| {
        catalog_error(format!("Failed to run package updater: {join}"))
    })?
    .map_err(|io| {
        catalog_error(format!(
            "Failed to stage or install update package: {io}"
        ))
    })
}

fn write_package_exclusively(
    path: &std::path::Path,
    data: &[u8],
) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxHost, UPDATE_SERVER_API, UpdateMethod, VersionEntry,
        dpkg_list_contains, linux_update_method, select_package_artifact,
        select_portable_artifact, select_update_artifact, update_artifact_url,
        update_endpoint, verify_update_signature, with_staged_package,
        write_package_exclusively,
    };
    use base64::{Engine, engine::general_purpose::STANDARD};
    use tauri::utils::config::BundleType;

    fn probe_exit_command(code: u8) -> tokio::process::Command {
        #[cfg(windows)]
        {
            let mut command = tokio::process::Command::new("cmd");
            command.args(["/D", "/C", &format!("exit {code}")]);
            command
        }
        #[cfg(not(windows))]
        {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", &format!("exit {code}")]);
            command
        }
    }

    #[tokio::test]
    async fn package_probe_accepts_only_a_successful_owned_child() {
        let timeout = std::time::Duration::from_secs(1);
        assert!(
            super::bounded_package_probe(probe_exit_command(0), timeout).await
        );
        assert!(
            !super::bounded_package_probe(probe_exit_command(7), timeout).await
        );
        assert!(
            !super::bounded_package_probe(
                tokio::process::Command::new(
                    "axolotl-nonexistent-rpm-probe-611fedca"
                ),
                timeout,
            )
            .await
        );
    }

    #[test]
    #[ignore = "subprocess fixture for the bounded startup probe"]
    fn package_probe_stalled_child_fixture() {
        let directory = std::env::var_os("AXOLOTL_TEST_RPM_PROBE_DIRECTORY")
            .expect("only the probe test starts this fixture");
        let directory = std::path::PathBuf::from(directory);
        std::fs::write(directory.join("started"), b"started").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        std::fs::write(directory.join("completed"), b"must not complete")
            .unwrap();
    }

    #[tokio::test]
    async fn stalled_package_probe_is_killed_and_falls_back_to_manual() {
        let directory = tempfile::tempdir().unwrap();
        let mut command =
            tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "updater_impl::tests::package_probe_stalled_child_fixture",
            ])
            .env("AXOLOTL_TEST_RPM_PROBE_DIRECTORY", directory.path());
        let started = std::time::Instant::now();
        let owned = super::bounded_package_probe(
            command,
            std::time::Duration::from_millis(500),
        )
        .await;
        assert!(!owned);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(directory.path().join("started").exists());
        assert_eq!(
            linux_update_method(
                Some(&BundleType::Rpm),
                false,
                host(false, owned, false, true)
            ),
            UpdateMethod::Manual
        );
        // The fixture writes after two seconds if it was left alive. Waiting
        // here proves timeout cleanup terminated the actual child, not just its waiter.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        assert!(!directory.path().join("completed").exists());
    }

    #[tokio::test]
    async fn staged_packages_are_private_unique_and_removed_after_consumption()
    {
        let first = with_staged_package("rpm", b"first".to_vec(), |path| {
            assert_eq!(std::fs::read(path)?, b"first");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    path.metadata()?.permissions().mode() & 0o777,
                    0o600
                );
                assert_eq!(
                    path.parent().unwrap().metadata()?.permissions().mode()
                        & 0o777,
                    0o700
                );
            }
            Ok(path.to_path_buf())
        })
        .await
        .unwrap();
        assert!(!first.parent().unwrap().exists());
        let second = with_staged_package("rpm", b"second".to_vec(), |path| {
            assert_eq!(std::fs::read(path)?, b"second");
            Ok(path.to_path_buf())
        })
        .await
        .unwrap();
        assert_ne!(first, second);
        assert!(!second.parent().unwrap().exists());
    }

    #[tokio::test]
    async fn staged_packages_clean_up_after_consumer_failure() {
        let path = std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured = path.clone();
        let failure = with_staged_package(
            "deb",
            vec![1],
            move |path| -> std::io::Result<()> {
                *captured.lock().unwrap() = Some(path.to_path_buf());
                std::process::Command::new(
                    "axolotl-missing-package-installer-66b39c7d",
                )
                .output()
                .map(|_| ())
            },
        )
        .await;
        assert!(failure.is_err());
        assert!(
            !path
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .parent()
                .unwrap()
                .exists()
        );
    }

    #[tokio::test]
    async fn a_child_process_consumes_the_staged_bytes_and_releases_the_directory()
     {
        let (path, output) =
            with_staged_package("rpm", b"verified package".to_vec(), |path| {
                #[cfg(windows)]
                let output = std::process::Command::new("cmd")
                    .args(["/D", "/C", "type"])
                    .arg(path)
                    .output()?;
                #[cfg(not(windows))]
                let output =
                    std::process::Command::new("cat").arg(path).output()?;
                Ok((path.to_path_buf(), output))
            })
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"verified package");
        assert!(!path.parent().unwrap().exists());
    }

    #[tokio::test]
    async fn a_nonzero_child_exit_still_removes_the_staged_package() {
        let (path, output) = with_staged_package("deb", vec![1], |path| {
            #[cfg(windows)]
            let output = std::process::Command::new("cmd")
                .args(["/D", "/C", "exit", "7"])
                .output()?;
            #[cfg(not(windows))]
            let output = std::process::Command::new("sh")
                .args(["-c", "exit 7"])
                .output()?;
            Ok((path.to_path_buf(), output))
        })
        .await
        .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn staged_package_refuses_existing_files_without_overwriting_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("update.rpm");
        std::fs::write(&path, b"unrelated").unwrap();
        assert_eq!(
            write_package_exclusively(&path, b"signed package")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(path).unwrap(), b"unrelated");
    }

    #[cfg(unix)]
    #[test]
    fn staged_package_refuses_symlinks_without_touching_the_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("unrelated");
        let path = directory.path().join("update.deb");
        std::fs::write(&target, b"unrelated").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert_eq!(
            write_package_exclusively(&path, b"signed package")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(target).unwrap(), b"unrelated");
    }

    #[tokio::test]
    async fn cancelling_the_wait_keeps_the_package_until_the_consumer_finishes()
    {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(with_staged_package(
            "deb",
            b"verified".to_vec(),
            move |path| {
                started_tx.send(path.to_path_buf()).unwrap();
                release_rx.recv().unwrap();
                assert_eq!(std::fs::read(path)?, b"verified");
                finished_tx.send(()).unwrap();
                Ok(())
            },
        ));
        let path = started_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(path.exists());
        release_tx.send(()).unwrap();
        finished_rx.await.unwrap();
        for _ in 0..100 {
            if !path.parent().unwrap().exists() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("package directory remained after consumer completed");
    }

    /// A host whose package databases own the executable as given.
    fn host(
        dpkg_owns_exe: bool,
        rpm_owns_exe: bool,
        apt: bool,
        pkexec: bool,
    ) -> impl FnOnce() -> LinuxHost {
        move || LinuxHost {
            dpkg_owns_exe,
            rpm_owns_exe,
            apt,
            pkexec,
        }
    }

    #[test]
    fn only_a_dpkg_tracked_deb_updates_through_apt() {
        let deb = Some(&BundleType::Deb);
        assert_eq!(
            linux_update_method(deb, false, host(true, false, true, true)),
            UpdateMethod::Apt
        );
        // A .deb repackaged for another package manager (debtap, alien), or
        // a host without apt or pkexec, never runs apt.
        for probe in [
            host(false, true, true, true),
            host(true, false, false, true),
            host(true, false, true, false),
        ] {
            assert_eq!(
                linux_update_method(deb, false, probe),
                UpdateMethod::Manual
            );
        }
    }

    #[test]
    fn an_rpm_tracked_install_updates_through_rpm() {
        let rpm = Some(&BundleType::Rpm);
        assert_eq!(
            linux_update_method(rpm, false, host(false, true, false, true)),
            UpdateMethod::Rpm
        );
        for probe in [
            host(true, false, true, true),
            host(false, true, true, false),
        ] {
            assert_eq!(
                linux_update_method(rpm, false, probe),
                UpdateMethod::Manual
            );
        }
    }

    #[test]
    fn appimages_replace_themselves_on_any_distribution() {
        let method = linux_update_method(
            Some(&BundleType::AppImage),
            true,
            || -> LinuxHost { panic!("probed the host for an AppImage") },
        );
        assert_eq!(method, UpdateMethod::Updater);
        assert!(method.package_installer().is_none());
    }

    #[test]
    fn unknown_or_unpacked_installs_only_report_updates() {
        // An AppImage run from its extracted squashfs-root has no file to
        // replace; a binary without a bundle type was built from source or
        // repackaged (Flatpak, distribution packages).
        for (bundle, in_appimage) in [
            (Some(&BundleType::AppImage), false),
            (None, false),
            (None, true),
        ] {
            let method =
                linux_update_method(bundle, in_appimage, || -> LinuxHost {
                    panic!("probed the host for {bundle:?}")
                });
            assert_eq!(method, UpdateMethod::Manual);
            assert!(method.package_installer().is_none());
        }
    }

    #[test]
    fn install_methods_serialize_for_the_frontend() {
        assert_eq!(
            serde_json::to_value(UpdateMethod::Manual).unwrap(),
            serde_json::json!("manual")
        );
        assert_eq!(
            serde_json::to_value(UpdateMethod::Updater).unwrap(),
            serde_json::json!("updater")
        );
    }

    #[test]
    fn package_methods_install_their_own_package_format() {
        let apt = UpdateMethod::Apt.package_installer().unwrap();
        assert_eq!((apt.variant, apt.program), ("deb", "apt"));
        let rpm = UpdateMethod::Rpm.package_installer().unwrap();
        assert_eq!((rpm.variant, rpm.program), ("rpm", "rpm"));
    }

    /// Primary artifact shape emitted by the catalog generator. The live API
    /// can additionally return sidecar entries, which selection does not require.
    fn linux_release() -> VersionEntry {
        let artifact =
            |kind: &str, variant: Option<&str>, arch: &str, path: &str| {
                serde_json::json!({
                    "kind": kind, "variant": variant,
                    "platform": if kind == "signature" { "" } else { "linux" },
                    "architecture": if kind == "signature" { "" } else { arch },
                    "relative_path": format!("dist/1.10.5/{path}"),
                    "sha256": "c".repeat(64), "size": 1,
                })
            };
        serde_json::from_value(serde_json::json!({
            "version": "1.10.5",
            "artifacts": [
                artifact("installer", Some("rpm"), "x86_64", "Axolotl.Launcher-1.10.5-1.x86_64.rpm"),
                artifact("installer", Some("appimage"), "x86_64", "Axolotl.Launcher_1.10.5_amd64.AppImage"),
                artifact("updater", Some("tauri"), "x86_64", "Axolotl.Launcher_1.10.5_amd64.AppImage.tar.gz"),
                artifact("installer", Some("deb"), "x86_64", "Axolotl.Launcher_1.10.5_amd64.deb"),
                artifact("installer", Some("deb"), "aarch64", "Axolotl.Launcher_1.10.5_arm64.deb"),
                artifact("signature", None, "", "Axolotl.Launcher-1.10.5-1.x86_64.rpm.sig"),
                artifact("signature", None, "", "Axolotl.Launcher_1.10.5_amd64.AppImage.tar.gz.sig"),
                artifact("signature", None, "", "Axolotl.Launcher_1.10.5_amd64.deb.sig"),
            ]
        }))
        .unwrap()
    }

    #[test]
    fn packages_are_selected_without_sidecars_in_the_primary_artifact_list() {
        let mut release = linux_release();
        release.artifacts.retain(|entry| entry.kind != "signature");
        for (variant, filename) in [
            ("deb", "Axolotl.Launcher_1.10.5_amd64.deb"),
            ("rpm", "Axolotl.Launcher-1.10.5-1.x86_64.rpm"),
        ] {
            let package =
                select_package_artifact(&release, variant, "x86_64").unwrap();
            assert_eq!(package.url.path(), format!("/dist/1.10.5/{filename}"));
            assert!(package.signature.is_none());
            assert_eq!(package.size, 1);
            assert_eq!(package.sha256, "c".repeat(64));
        }
        assert!(select_package_artifact(&release, "rpm", "aarch64").is_err());
        assert!(select_package_artifact(&release, "deb", "aarch64").is_ok());
        release.artifacts[0].size = 0;
        assert!(select_package_artifact(&release, "rpm", "x86_64").is_err());
    }

    #[tokio::test]
    async fn inline_package_signatures_are_used_without_sidecar_entries() {
        let mut release = linux_release();
        release.artifacts.retain(|entry| entry.kind != "signature");
        let signature = "signed-inline-package".repeat(3);
        release.artifacts[0].signature = Some(signature.clone());
        let package =
            select_package_artifact(&release, "rpm", "x86_64").unwrap();
        assert_eq!(
            super::artifact_signature(&package).await.unwrap(),
            signature
        );
    }

    #[test]
    fn signature_validation_bounds_inline_and_downloaded_content() {
        for content in [vec![b'a'; 16_385], vec![0xff; 40], b"short".to_vec()] {
            assert!(super::decode_artifact_signature(&content).is_err());
        }
        assert_eq!(
            super::decode_artifact_signature(&vec![b'a'; 16_384])
                .unwrap()
                .len(),
            16_384
        );
    }

    #[tokio::test]
    async fn package_sidecars_use_the_exact_url_and_bound_the_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (status, bytes, succeeds) in [
            ("200 OK", vec![b'a'; 64], true),
            ("200 OK", vec![b'a'; 16_385], false),
            ("404 Not Found", vec![b'a'; 64], false),
        ] {
            let listener =
                tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let count = stream.read(&mut request).await.unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..count])
                        .starts_with("GET /update.rpm.sig HTTP/1.1")
                );
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                stream.write_all(header.as_bytes()).await.unwrap();
                stream.write_all(&bytes).await.unwrap();
            });
            let package = super::UpdateArtifact {
                url: format!("http://{address}/update.rpm").parse().unwrap(),
                sha256: "c".repeat(64),
                size: 1,
                signature: None,
            };
            assert_eq!(
                super::artifact_signature(&package).await.is_ok(),
                succeeds
            );
            server.await.unwrap();
        }
    }

    #[test]
    fn dpkg_ownership_needs_the_executable_in_the_file_list() {
        let list = "/.
/usr
/usr/bin
/usr/bin/Axolotl Launcher
                    /usr/share/applications/Axolotl Launcher.desktop
";
        assert!(dpkg_list_contains(
            list,
            std::path::Path::new("/usr/bin/Axolotl Launcher")
        ));
        assert!(!dpkg_list_contains(
            list,
            std::path::Path::new("/home/me/Applications/Axolotl Launcher")
        ));
        assert!(!dpkg_list_contains("", std::path::Path::new("/usr/bin/x")));
    }

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
        let selected = select_update_artifact(&release, &url).unwrap();
        assert_eq!(selected.url, url);
        assert_eq!(selected.size, 20);
        assert_eq!(selected.sha256, "b".repeat(64));
        let missing = update_artifact_url("dist/1.10.0/missing.zip").unwrap();
        assert!(select_update_artifact(&release, &missing).is_err());
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
