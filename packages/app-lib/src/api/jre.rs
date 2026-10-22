//! Authentication flow interface
use crate::event::emit::{
    emit_java_discovery_update, emit_loading, init_loading,
};
use crate::install::{
    InstallErrorContext, InstallJavaStep, InstallPhaseDetails, InstallPhaseId,
    InstallProgress, InstallProgressReporter,
};
use crate::state::{DiscoveredJava, JavaVersion, java_file_signature};
use crate::util::fetch::{
    ContentValidation, DownloadRequest, FetchProgressFn, Integrity,
    ResourceClass, download_to_path, fetch_json,
};
use futures::{TryStreamExt, stream};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{MemoryRefreshKind, RefreshKind};

use crate::util::io;
use crate::util::jre::extract_java_version;
use crate::{
    LoadingBarType, State,
    util::jre::{self},
};
use xz2::read::XzDecoder;

pub async fn get_java_versions() -> crate::Result<Vec<JavaVersion>> {
    let state = State::get().await?;
    register_managed_java_versions(&state).await?;

    JavaVersion::get_all(&state.pool).await
}

async fn register_managed_java_versions(state: &State) -> crate::Result<()> {
    let root = state.directories.java_versions_dir();
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let install_root = entry.path();
        if !entry.file_type().await?.is_dir()
            || jre::is_java_install_staging_path(&install_root)
        {
            continue;
        }

        let mut candidates = vec![
            install_root.join("bin").join(jre::JAVA_BIN),
            install_root.join("Contents/Home/bin").join(jre::JAVA_BIN),
            install_root
                .join("jre.bundle/Contents/Home/bin")
                .join(jre::JAVA_BIN),
        ];
        let legacy_pointer = install_root.join("bin");
        if legacy_pointer.is_file()
            && let Ok(relative) =
                tokio::fs::read_to_string(&legacy_pointer).await
        {
            let relative = relative.trim();
            if !relative.is_empty() {
                candidates.push(install_root.join(relative));
            }
        }

        for candidate in candidates {
            let Ok(java) = jre::check_java_at_filepath(&candidate).await else {
                continue;
            };
            let set_default = entry
                .file_name()
                .to_string_lossy()
                .rsplit_once("-jdk-")
                .and_then(|(_, version)| version.parse::<u32>().ok())
                == Some(java.parsed_version)
                && JavaVersion::get(java.parsed_version, &state.pool)
                    .await?
                    .is_none();
            let mut transaction = state.pool.begin().await?;
            java.upsert(&mut *transaction).await?;
            if set_default {
                JavaVersion::set_default(
                    java.parsed_version,
                    &java.path,
                    &mut *transaction,
                )
                .await?;
            }
            if let Some(discovered) = DiscoveredJava::from_java(java) {
                discovered.upsert(&mut *transaction).await?;
            }
            transaction.commit().await?;
            break;
        }
    }

    Ok(())
}

pub async fn get_java_default_versions() -> crate::Result<Vec<JavaVersion>> {
    let state = State::get().await?;
    register_managed_java_versions(&state).await?;

    JavaVersion::get_all_defaults(&state.pool).await
}

#[cfg(feature = "tauri")]
pub fn respond_to_java_download_confirmation(
    request_id: uuid::Uuid,
    approved: bool,
) -> bool {
    crate::event::emit::respond_to_java_download_confirmation(
        request_id, approved,
    )
}

/// Lists available Java major versions from the specified distribution API.
pub async fn list_java_distribution_versions(
    distribution: String,
) -> crate::Result<Vec<u32>> {
    let state = State::get().await?;
    match distribution.as_str() {
        "zulu" => {
            let platform = std::env::consts::OS;
            let arch = std::env::consts::ARCH;
            let url = format!(
                "https://api.azul.com/metadata/v1/zulu/packages/?os={platform}&arch={arch}&archive_type=tar.gz&java_package_type=jre&page_size=1000"
            );
            let packages: Vec<AzulPackageSummary> = fetch_json(
                Method::GET,
                &url,
                None,
                None,
                None,
                &state.api_semaphore,
                &state.pool,
            )
            .await?;
            let mut versions: Vec<u32> = packages
                .iter()
                .filter_map(|p| p.java_version.first().copied())
                .filter(|v| *v >= 8)
                .collect();
            versions.sort_unstable();
            versions.dedup();
            Ok(versions)
        }
        _ => Err(crate::ErrorKind::InputError(format!(
            "Unknown distribution: {distribution}"
        ))
        .into()),
    }
}

pub async fn set_java_version(java_version: JavaVersion) -> crate::Result<()> {
    let state = State::get().await?;
    java_version.upsert(&state.pool).await?;
    Ok(())
}

pub async fn set_java_default_version(
    major_version: u32,
    path: String,
) -> crate::Result<JavaVersion> {
    let java_version = check_jre(PathBuf::from(path)).await?;
    if java_version.parsed_version != major_version {
        return Err(crate::ErrorKind::InputError(format!(
            "Java {} cannot be used as the Java {major_version} default",
            java_version.parsed_version
        ))
        .into());
    }

    let state = State::get().await?;
    let mut transaction = state.pool.begin().await?;
    java_version.upsert(&mut *transaction).await?;
    JavaVersion::set_default(
        major_version,
        &java_version.path,
        &mut *transaction,
    )
    .await?;
    transaction.commit().await?;

    Ok(java_version)
}

pub async fn remove_java_default_version(
    major_version: u32,
) -> crate::Result<()> {
    let state = State::get().await?;
    JavaVersion::remove_default(major_version, &state.pool).await
}

/// Removes a configured Java version by its executable path.
pub async fn remove_java_version(path: String) -> crate::Result<()> {
    let state = State::get().await?;
    let mut transaction = state.pool.begin().await?;
    JavaVersion::remove_default_for_path(&path, &mut *transaction).await?;
    JavaVersion::delete(&path, &mut *transaction).await?;
    transaction.commit().await?;
    Ok(())
}

const JAVA_RESCAN_DEBOUNCE: Duration = Duration::from_secs(60);

static JAVA_SCAN_STATE: LazyLock<tokio::sync::Mutex<Option<Instant>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(None));

// Searches for jres on the system given a java version (ex: 8, 17, 21)
pub async fn find_filtered_jres(
    java_version: Option<u32>,
    full_scan: bool,
    force_fresh: bool,
    exhaustive: bool,
) -> crate::Result<Vec<JavaVersion>> {
    get_available_jres(full_scan, force_fresh, exhaustive).await?;

    let state = State::get().await?;
    let jres = JavaVersion::get_all(&state.pool).await?;

    Ok(if let Some(java_version) = java_version {
        jres.into_iter()
            .filter(|jre| jre.parsed_version == java_version)
            .collect()
    } else {
        jres
    })
}

/// Returns all known Java installations, served from the discovery cache
/// when possible. When the cache is hit, a debounced rescan runs in the
/// background and a `java_discovery_update` event fires if it changes
/// anything; the cache being empty forces a full scan instead.
///
/// When `full_scan` is true the cache is always bypassed and a fresh
/// full scan (including BFS directory traversal) is performed.
///
/// When `force_fresh` is true the cache is bypassed for a fresh quick scan,
/// ensuring newly installed Java runtimes appear without requiring a deep
/// scan or background rescan.
pub async fn get_available_jres(
    full_scan: bool,
    force_fresh: bool,
    exhaustive: bool,
) -> crate::Result<Vec<JavaVersion>> {
    let state = State::get().await?;

    if full_scan || exhaustive {
        let mut last_scan = JAVA_SCAN_STATE.lock().await;
        let jres = refresh_discovered_javas(&state, true, exhaustive).await?;
        *last_scan = Some(Instant::now());
        return Ok(jres);
    }

    if !force_fresh {
        let cached = validate_cached_javas(&state).await?;
        if !cached.is_empty() {
            schedule_background_java_rescan();
            return Ok(cached);
        }
    }

    let mut last_scan = JAVA_SCAN_STATE.lock().await;
    if !force_fresh {
        // Re-check after taking the lock: a concurrent caller may have just
        // finished the initial scan while this one was waiting
        let cached = validate_cached_javas(&state).await?;
        if !cached.is_empty() {
            return Ok(cached);
        }
    }
    let jres = refresh_discovered_javas(&state, false, false).await?;
    *last_scan = Some(Instant::now());
    Ok(jres)
}

// Serves cache entries whose executable still matches the stored file
// signature; entries that changed on disk are re-verified individually and
// entries that disappeared are dropped
async fn validate_cached_javas(
    state: &State,
) -> crate::Result<Vec<JavaVersion>> {
    let entries = DiscoveredJava::get_all(&state.pool).await?;
    let mut valid = Vec::new();

    for entry in entries {
        let path = PathBuf::from(&entry.java.path);
        match java_file_signature(&path) {
            Some((size, mtime))
                if size == entry.file_size && mtime == entry.file_mtime_ms =>
            {
                valid.push(entry.java);
            }
            Some(_) => {
                if let Ok(java) = jre::check_java_at_filepath(&path).await
                    && let Some(refreshed) =
                        DiscoveredJava::from_java(java.clone())
                {
                    refreshed.upsert(&state.pool).await?;
                    valid.push(java);
                } else {
                    DiscoveredJava::remove(&entry.java.path, &state.pool)
                        .await?;
                }
            }
            None => {
                DiscoveredJava::remove(&entry.java.path, &state.pool).await?;
            }
        }
    }

    Ok(valid)
}

// Runs a system scan and replaces the discovery cache with its results.
// When `full_scan` is true a BFS directory traversal is performed;
// otherwise only well-known paths and registry entries are checked.
async fn refresh_discovered_javas(
    state: &State,
    full_scan: bool,
    exhaustive: bool,
) -> crate::Result<Vec<JavaVersion>> {
    let jres = jre::get_all_jre(full_scan, exhaustive).await?;

    let previous: HashSet<(String, String)> =
        DiscoveredJava::get_all(&state.pool)
            .await?
            .into_iter()
            .map(|entry| (entry.java.path, entry.java.version))
            .collect();

    let entries: Vec<DiscoveredJava> = jres
        .iter()
        .filter_map(|java| DiscoveredJava::from_java(java.clone()))
        .collect();
    DiscoveredJava::replace_all(&state.pool, &entries).await?;

    // Also upsert into java_versions so they appear in the settings list
    for java in &jres {
        if let Err(e) = java.upsert(&state.pool).await {
            tracing::warn!(
                "Failed to upsert discovered Java {} into java_versions: {e}",
                java.parsed_version
            );
        }
    }

    let current: HashSet<(String, String)> = entries
        .iter()
        .map(|entry| (entry.java.path.clone(), entry.java.version.clone()))
        .collect();
    if current != previous {
        let _ = emit_java_discovery_update(current.len()).await;
    }

    Ok(jres)
}

// Schedules a debounced background rescan of system Javas
fn schedule_background_java_rescan() {
    tokio::spawn(async {
        let mut last_scan = JAVA_SCAN_STATE.lock().await;
        if let Some(at) = *last_scan
            && at.elapsed() < JAVA_RESCAN_DEBOUNCE
        {
            return;
        }

        let Ok(state) = State::get().await else {
            return;
        };
        match refresh_discovered_javas(&state, true, false).await {
            Ok(_) => *last_scan = Some(Instant::now()),
            Err(e) => {
                tracing::warn!("Background Java rescan failed: {e}");
            }
        }
    });
}

/// Looks up a previously discovered Java matching `major_version` and
/// re-verifies it before returning, so instance launches can reuse an
/// existing installation instead of downloading a new runtime.
pub async fn find_cached_java(
    major_version: u32,
) -> crate::Result<Option<JavaVersion>> {
    let state = State::get().await?;

    for entry in DiscoveredJava::get_all(&state.pool).await? {
        if entry.java.parsed_version != major_version {
            continue;
        }
        let path = PathBuf::from(&entry.java.path);
        if let Ok(java) = jre::check_java_at_filepath(&path).await
            && java.parsed_version == major_version
        {
            return Ok(Some(java));
        }
    }

    Ok(None)
}

/// Resolves an installed Java for `major_version` without downloading: serves
/// it from the discovery cache when present, otherwise runs a system scan and
/// checks again. Returns `None` only when no matching Java is installed, so
/// launch/install callers fall back to downloading a runtime as a last resort.
pub async fn find_java_for_version(
    major_version: u32,
) -> crate::Result<Option<JavaVersion>> {
    let state = State::get().await?;
    if let Some(configured) =
        JavaVersion::get(major_version, &state.pool).await?
    {
        let path = PathBuf::from(&configured.path);
        if let Ok(java) = jre::check_java_at_filepath(&path).await
            && java.parsed_version == major_version
        {
            java.upsert(&state.pool).await?;
            return Ok(Some(java));
        }

        tracing::warn!(
            major_version,
            path = %configured.path,
            "Configured Java default is no longer valid"
        );
        JavaVersion::remove_default(major_version, &state.pool).await?;
    }

    if let Some(java) = find_cached_java(major_version).await? {
        return Ok(Some(java));
    }

    let scanned = get_available_jres(false, false, false).await?;
    Ok(scanned
        .into_iter()
        .find(|java| java.parsed_version == major_version))
}

/// Resolves an installed Java that can run `major_version` without
/// downloading. Prefers an exact match, then falls back to the closest newer
/// runtime so a user-installed Java can still be used when the metadata's
/// recommended major version is not present.
pub async fn find_compatible_java_for_version(
    major_version: u32,
) -> crate::Result<Option<JavaVersion>> {
    if let Some(java) = find_java_for_version(major_version).await? {
        return Ok(Some(java));
    }

    let state = State::get().await?;
    let mut jres = get_available_jres(false, false, false).await?;
    jres.extend(JavaVersion::get_all(&state.pool).await?);
    Ok(closest_compatible_java(jres.iter(), major_version))
}

fn closest_compatible_java<'a>(
    jres: impl IntoIterator<Item = &'a JavaVersion>,
    major_version: u32,
) -> Option<JavaVersion> {
    jres.into_iter()
        .filter(|java| java.parsed_version >= major_version)
        .min_by_key(|java| (java.parsed_version, java.path.as_str()))
        .cloned()
}

pub async fn auto_install_java(
    java_version: u32,
) -> crate::Result<Option<PathBuf>> {
    auto_install_java_inner(java_version, true, None, false).await
}

pub async fn auto_install_java_with_loading(
    java_version: u32,
    show_loading: bool,
) -> crate::Result<Option<PathBuf>> {
    auto_install_java_inner(java_version, show_loading, None, true).await
}

pub async fn auto_install_java_with_reporter(
    java_version: u32,
    reporter: InstallProgressReporter,
) -> crate::Result<Option<PathBuf>> {
    auto_install_java_inner(java_version, false, Some(reporter), true).await
}

const JAVA_INSTALL_STEPS: u64 = 4;
const JAVA_DOWNLOAD_PROGRESS_MIN_BYTES: u64 = 256 * 1024;
const MOJANG_RUNTIME_INDEX_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

static JAVA_INSTALL_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

type MojangRuntimeIndex =
    HashMap<String, HashMap<String, Vec<MojangRuntimeRelease>>>;

#[derive(Clone, Deserialize)]
struct RuntimeDownload {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct MojangRuntimeRelease {
    manifest: RuntimeDownload,
}

#[derive(Deserialize)]
struct MojangRuntimeManifest {
    files: HashMap<String, MojangRuntimeFile>,
}

#[derive(Clone, Deserialize)]
struct MojangRuntimeDownloads {
    raw: RuntimeDownload,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum MojangRuntimeFile {
    Directory,
    File {
        downloads: MojangRuntimeDownloads,
        #[serde(default)]
        executable: bool,
    },
    Link {
        target: String,
    },
}

#[derive(Deserialize)]
struct AzulPackageSummary {
    package_uuid: String,
    java_version: Vec<u32>,
}

#[derive(Deserialize)]
struct AzulPackage {
    download_url: String,
    name: PathBuf,
    sha256_hash: String,
    size: u64,
}

// ── JetBrains JDK feed types ────────────────────────────────────────────────

const JDK_FEED_URL: &str =
    "https://download.jetbrains.com/jdk/feed/v1/jdks.json.xz";

#[derive(Deserialize, Clone)]
struct JdkFeed {
    jdks: Vec<JdkFeedEntry>,
}

#[derive(Deserialize, Clone)]
struct JdkFeedEntry {
    vendor: String,
    product: Option<String>,
    #[serde(rename = "jdk_version_major")]
    jdk_version_major: u32,
    #[serde(rename = "jdk_version")]
    jdk_version: String,
    packages: Vec<JdkFeedPackage>,
}

#[derive(Deserialize, Clone)]
struct JdkFeedPackage {
    os: String,
    arch: String,
    url: String,
    sha256: String,
    archive_size: u64,
    install_folder_name: String,
}

#[derive(Clone, Default)]
struct JavaDownloadMetrics {
    source: Arc<Mutex<Option<String>>>,
    fallback_count: Arc<AtomicU64>,
}

impl JavaDownloadMetrics {
    fn record(&self, result: &crate::util::fetch::DownloadResult) {
        if result.attempts > 0
            && let Ok(mut source) = self.source.lock()
        {
            *source = Some(result.source.as_str().to_string());
        }
        self.fallback_count
            .fetch_add(result.fallback_count as u64, Ordering::Relaxed);
    }

    async fn finish(
        &self,
        reporter: Option<&InstallProgressReporter>,
    ) -> crate::Result<()> {
        let source = self.source.lock().ok().and_then(|source| source.clone());
        if let (Some(reporter), Some(source)) = (reporter, source) {
            reporter
                .record_download_metrics(
                    source,
                    self.fallback_count.load(Ordering::Relaxed),
                )
                .await?;
        }
        Ok(())
    }
}

async fn update_java_install_progress(
    reporter: Option<&InstallProgressReporter>,
    java_version: u32,
    step: InstallJavaStep,
    progress: Option<InstallProgress>,
) -> crate::Result<()> {
    if let Some(reporter) = reporter {
        reporter
            .update(
                InstallPhaseId::PreparingJava,
                progress,
                InstallPhaseDetails::Java {
                    major_version: java_version,
                    step,
                },
            )
            .await?;
    }

    Ok(())
}

fn java_step_progress(current: u64) -> InstallProgress {
    InstallProgress {
        current,
        total: JAVA_INSTALL_STEPS,
        secondary: None,
    }
}

async fn confirm_java_download(
    java_version: u32,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<bool> {
    if let Some(reporter) = reporter
        && reporter.is_java_download_postponed(java_version).await
    {
        return Ok(false);
    }

    let approved =
        crate::event::emit::request_java_download_confirmation(java_version)
            .await?;
    if !approved && let Some(reporter) = reporter {
        reporter.postpone_java_download(java_version).await;
    }

    Ok(approved)
}

async fn auto_install_java_inner(
    java_version: u32,
    show_loading: bool,
    reporter: Option<InstallProgressReporter>,
    confirm_download: bool,
) -> crate::Result<Option<PathBuf>> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let state = State::get().await?;
    let _install_guard = JAVA_INSTALL_LOCK.lock().await;
    if confirm_download
        && !confirm_java_download(java_version, reporter.as_ref()).await?
    {
        return Ok(None);
    }

    let loading_bar = if show_loading {
        Some(
            init_loading(
                LoadingBarType::JavaDownload {
                    version: java_version,
                },
                100.0,
                "Downloading java version",
            )
            .await?,
        )
    } else {
        None
    };

    if let Some(loading_bar) = &loading_bar {
        emit_loading(loading_bar, 0.0, Some("Fetching java version"))?;
    }
    update_java_install_progress(
        reporter.as_ref(),
        java_version,
        InstallJavaStep::FetchingMetadata,
        Some(java_step_progress(1)),
    )
    .await?;

    if let Some(path) = install_mojang_runtime(
        &state,
        java_version,
        loading_bar.as_ref(),
        reporter.as_ref(),
    )
    .await?
    {
        return Ok(Some(path));
    }

    install_azul_runtime(
        &state,
        java_version,
        loading_bar.as_ref(),
        reporter.as_ref(),
    )
    .await
    .map(Some)
}

fn mojang_runtime_component(java_version: u32) -> Option<&'static str> {
    match java_version {
        8 => Some("jre-legacy"),
        16 => Some("java-runtime-alpha"),
        17 => Some("java-runtime-gamma"),
        21 => Some("java-runtime-delta"),
        25 => Some("java-runtime-epsilon"),
        _ => None,
    }
}

fn mojang_runtime_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-x64"),
        ("windows", "x86") => Some("windows-x86"),
        ("windows", "aarch64") => Some("windows-arm64"),
        ("macos", "x86_64") => Some("mac-os"),
        ("macos", "aarch64") => Some("mac-os-arm64"),
        ("linux", "x86_64") => Some("linux"),
        ("linux", "x86") => Some("linux-i386"),
        _ => None,
    }
}

fn runtime_executable_relative(platform: &str) -> PathBuf {
    if platform.starts_with("mac-os") {
        PathBuf::from("jre.bundle/Contents/Home/bin/java")
    } else {
        PathBuf::from("bin").join(jre::JAVA_BIN)
    }
}

fn safe_runtime_path(root: &Path, relative: &str) -> crate::Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(crate::ErrorKind::InputError(
            "Java runtime manifest contains an invalid path".to_string(),
        )
        .into());
    }

    Ok(root.join(relative))
}

fn resolved_runtime_link_target(
    link_path: &str,
    target: &str,
) -> crate::Result<PathBuf> {
    let mut resolved = PathBuf::new();
    if let Some(parent) = Path::new(link_path).parent() {
        resolved.push(parent);
    }

    for component in Path::new(target).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => resolved.push(component),
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(crate::ErrorKind::InputError(
                        "Java runtime manifest link escapes its install directory"
                            .to_string(),
                    )
                    .into());
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(crate::ErrorKind::InputError(
                    "Java runtime manifest contains an invalid link target"
                        .to_string(),
                )
                .into());
            }
        }
    }

    Ok(resolved)
}

async fn remove_path_if_present(path: &Path) -> crate::Result<()> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        io::remove_dir_all(path).await?;
    } else {
        io::remove_file(path).await?;
    }
    Ok(())
}

async fn create_runtime_link(
    root: &Path,
    link_path: &str,
    target: &str,
) -> crate::Result<()> {
    let path = safe_runtime_path(root, link_path)?;
    let resolved_target = resolved_runtime_link_target(link_path, target)?;
    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }
    remove_path_if_present(&path).await?;

    #[cfg(unix)]
    {
        let _ = resolved_target;
        let path = path.clone();
        let target = target.to_string();
        tokio::task::spawn_blocking(move || {
            std::os::unix::fs::symlink(target, path)
        })
        .await??;
    }

    #[cfg(windows)]
    {
        tokio::fs::copy(root.join(resolved_target), &path).await?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        tokio::fs::copy(root.join(resolved_target), &path).await?;
    }

    Ok(())
}

async fn set_runtime_executable(path: &Path) -> crate::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = tokio::fs::metadata(path).await?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        tokio::fs::set_permissions(path, permissions).await?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}

async fn validate_installed_java(
    path: PathBuf,
    java_version: u32,
    reporter: Option<&InstallProgressReporter>,
    loading_bar: Option<&crate::event::LoadingBarId>,
) -> crate::Result<PathBuf> {
    update_java_install_progress(
        reporter,
        java_version,
        InstallJavaStep::Validating,
        Some(java_step_progress(4)),
    )
    .await?;
    if !test_jre(path.clone(), java_version).await? {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Downloaded Java runtime did not report major version {java_version}"
        ))
        .into());
    }
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 10.0, Some("Done installing java"))?;
    }
    Ok(path)
}

async fn install_mojang_runtime(
    state: &State,
    java_version: u32,
    loading_bar: Option<&crate::event::LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<Option<PathBuf>> {
    let Some(component) = mojang_runtime_component(java_version) else {
        return Ok(None);
    };
    let Some(platform) = mojang_runtime_platform() else {
        return Ok(None);
    };

    if let Some(reporter) = reporter {
        reporter
            .set_context(
                InstallErrorContext::new("fetch Mojang Java runtime index")
                    .urls(vec![MOJANG_RUNTIME_INDEX_URL.to_string()])
                    .java_version(java_version)
                    .os(std::env::consts::OS)
                    .arch(std::env::consts::ARCH)
                    .build(),
            )
            .await?;
    }
    let index = fetch_json::<MojangRuntimeIndex>(
        Method::GET,
        MOJANG_RUNTIME_INDEX_URL,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    let Some(release) = index
        .get(platform)
        .and_then(|components| components.get(component))
        .and_then(|releases| releases.first())
    else {
        return Ok(None);
    };

    let manifest_path = state
        .directories
        .caches_dir()
        .join("java")
        .join("manifests")
        .join(format!("{}.json", release.manifest.sha1));
    let manifest_integrity = Integrity::sha1(&release.manifest.sha1)
        .with_size(release.manifest.size)
        .with_content_validation(ContentValidation::Json);
    if let Some(reporter) = reporter {
        reporter
            .set_context(
                InstallErrorContext::new("fetch Mojang Java runtime manifest")
                    .urls(vec![release.manifest.url.clone()])
                    .expected_hash(release.manifest.sha1.clone())
                    .expected_size(release.manifest.size)
                    .target_path(manifest_path.display().to_string())
                    .java_version(java_version)
                    .os(std::env::consts::OS)
                    .arch(std::env::consts::ARCH)
                    .build(),
            )
            .await?;
    }
    let mut request =
        DownloadRequest::new(&release.manifest.url, ResourceClass::Java)
            .with_integrity(manifest_integrity);
    if let Some(reporter) = reporter {
        request = request.with_install_tracking(
            reporter.clone(),
            manifest_path.display().to_string(),
            manifest_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "runtime-manifest.json".to_string()),
        );
    }
    let download_result = download_to_path(
        request,
        &manifest_path,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await?;
    if let Some(reporter) = reporter
        && download_result.attempts > 0
    {
        reporter
            .record_download_metrics(
                download_result.source.as_str(),
                download_result.fallback_count as u64,
            )
            .await?;
    }
    let manifest: MojangRuntimeManifest =
        serde_json::from_slice(&io::read(&manifest_path).await?)?;

    let install_name = format!(
        "mojang-{component}-{platform}-{}",
        &release.manifest.sha1[..12.min(release.manifest.sha1.len())]
    );
    let java_root = state.directories.java_versions_dir();
    let final_root = java_root.join(&install_name);
    let staging_root = java_root.join(format!(".{install_name}.installing"));
    let executable_relative = runtime_executable_relative(platform);
    let final_executable = final_root.join(&executable_relative);
    if final_executable.is_file() {
        return Ok(Some(
            validate_installed_java(
                final_executable,
                java_version,
                reporter,
                loading_bar,
            )
            .await?,
        ));
    }

    io::create_dir_all(&staging_root).await?;
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut links = Vec::new();
    for (relative_path, entry) in manifest.files {
        match entry {
            MojangRuntimeFile::Directory => directories.push(relative_path),
            MojangRuntimeFile::File {
                downloads,
                executable,
            } => files.push((relative_path, downloads.raw, executable)),
            MojangRuntimeFile::Link { target } => {
                links.push((relative_path, target));
            }
        }
    }
    directories.sort_by_key(|path| Path::new(path).components().count());
    for directory in directories {
        io::create_dir_all(safe_runtime_path(&staging_root, &directory)?)
            .await?;
    }

    let total_bytes = files
        .iter()
        .map(|(_, download, _)| download.size)
        .sum::<u64>();
    update_java_install_progress(
        reporter,
        java_version,
        InstallJavaStep::Downloading,
        Some(InstallProgress {
            current: 0,
            total: total_bytes.max(1),
            secondary: None,
        }),
    )
    .await?;
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 10.0, Some("Downloading java version"))?;
    }

    let downloaded_bytes = Arc::new(AtomicU64::new(0));
    let last_reported = Arc::new(AtomicU64::new(0));
    let download_metrics = JavaDownloadMetrics::default();
    stream::iter(files.into_iter().map(Ok::<_, crate::Error>))
        .try_for_each_concurrent(
            None,
            |(relative_path, download, executable)| {
                let downloaded_bytes = downloaded_bytes.clone();
                let last_reported = last_reported.clone();
                let staging_root = staging_root.clone();
                let reporter = reporter.cloned();
                let download_metrics = download_metrics.clone();
                async move {
                    let path =
                        safe_runtime_path(&staging_root, &relative_path)?;
                    if let Some(reporter) = reporter.as_ref() {
                        reporter
                            .set_transient_context(
                                InstallErrorContext::new(
                                    "download Mojang Java runtime file",
                                )
                                .urls(vec![download.url.clone()])
                                .file_path(relative_path.clone())
                                .target_path(path.display().to_string())
                                .expected_hash(download.sha1.clone())
                                .expected_size(download.size)
                                .java_version(java_version)
                                .os(std::env::consts::OS)
                                .arch(std::env::consts::ARCH)
                                .build(),
                            )
                            .await?;
                    }

                    let mut file_progress = 0_u64;
                    let mut progress = |current: u64,
                                        _total: u64|
                     -> Pin<
                        Box<dyn Future<Output = crate::Result<()>> + Send>,
                    > {
                        let delta = current.saturating_sub(file_progress);
                        file_progress = current;
                        let current = downloaded_bytes
                            .fetch_add(delta, Ordering::Relaxed)
                            .saturating_add(delta)
                            .min(total_bytes);
                        let previous = last_reported.load(Ordering::Relaxed);
                        let min_delta = (total_bytes / 200)
                            .max(JAVA_DOWNLOAD_PROGRESS_MIN_BYTES);
                        if current < total_bytes
                            && current.saturating_sub(previous) < min_delta
                        {
                            return Box::pin(async { Ok(()) });
                        }
                        last_reported.store(current, Ordering::Relaxed);
                        let reporter = reporter.clone();
                        Box::pin(async move {
                            update_java_install_progress(
                                reporter.as_ref(),
                                java_version,
                                InstallJavaStep::Downloading,
                                Some(InstallProgress {
                                    current,
                                    total: total_bytes.max(1),
                                    secondary: None,
                                }),
                            )
                            .await
                        })
                    };
                    let mut request = DownloadRequest::new(
                        &download.url,
                        ResourceClass::Java,
                    )
                    .with_integrity(
                        Integrity::sha1(&download.sha1)
                            .with_size(download.size),
                    );
                    if let Some(reporter) = &reporter {
                        request = request.with_install_tracking(
                            reporter.clone(),
                            path.display().to_string(),
                            relative_path.clone(),
                        );
                    }
                    let result = download_to_path(
                        request,
                        &path,
                        &state.download_semaphore,
                        &state.pool,
                        Some(&mut progress as &mut FetchProgressFn<'_>),
                    )
                    .await?;
                    download_metrics.record(&result);
                    if executable {
                        set_runtime_executable(&path).await?;
                    }
                    Ok(())
                }
            },
        )
        .await?;
    download_metrics.finish(reporter).await?;

    update_java_install_progress(
        reporter,
        java_version,
        InstallJavaStep::Extracting,
        Some(java_step_progress(3)),
    )
    .await?;
    for (link_path, target) in links {
        create_runtime_link(&staging_root, &link_path, &target).await?;
    }
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 80.0, Some("Installing java runtime"))?;
    }

    if final_root.exists() {
        io::remove_dir_all(&final_root).await?;
    }
    tokio::fs::rename(&staging_root, &final_root).await?;
    let executable = final_root.join(executable_relative);
    Ok(Some(
        validate_installed_java(
            executable,
            java_version,
            reporter,
            loading_bar,
        )
        .await?,
    ))
}

fn validate_archive_file_name(name: &Path) -> crate::Result<()> {
    if name.as_os_str().is_empty()
        || name.is_absolute()
        || name.components().count() != 1
        || !matches!(name.components().next(), Some(Component::Normal(_)))
    {
        return Err(crate::ErrorKind::InputError(
            "Java package metadata contains an invalid file name".to_string(),
        )
        .into());
    }
    Ok(())
}

fn extract_azul_archive(
    archive_path: &Path,
    staging_root: &Path,
) -> crate::Result<PathBuf> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "Failed to read Java archive: {error}"
        ))
    })?;
    let root = archive
        .file_names()
        .find_map(|name| {
            name.split('/').find(|component| !component.is_empty())
        })
        .map(PathBuf::from)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Java archive does not contain an install directory"
                    .to_string(),
            )
        })?;
    validate_archive_file_name(&root)?;
    archive.extract(staging_root).map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "Failed to extract Java archive: {error}"
        ))
    })?;
    Ok(root)
}

async fn install_azul_runtime(
    state: &State,
    java_version: u32,
    loading_bar: Option<&crate::event::LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<PathBuf> {
    let metadata_url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages?arch={}&java_version={}&os={}&archive_type=zip&javafx_bundled=false&java_package_type=jre&page_size=1",
        std::env::consts::ARCH,
        java_version,
        std::env::consts::OS
    );
    if let Some(reporter) = reporter {
        reporter
            .set_context(
                InstallErrorContext::new("fetch Azul Java package metadata")
                    .urls(vec![metadata_url.clone()])
                    .java_version(java_version)
                    .os(std::env::consts::OS)
                    .arch(std::env::consts::ARCH)
                    .build(),
            )
            .await?;
    }
    let packages = fetch_json::<Vec<AzulPackageSummary>>(
        Method::GET,
        &metadata_url,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    let summary = packages.first().ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "No Java Version found for Java version {}, OS {}, and Architecture {}",
            java_version,
            std::env::consts::OS,
            std::env::consts::ARCH,
        ))
    })?;
    let details_url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/{}",
        summary.package_uuid
    );
    let download = fetch_json::<AzulPackage>(
        Method::GET,
        &details_url,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    validate_archive_file_name(&download.name)?;

    let archive_path = state
        .directories
        .caches_dir()
        .join("java")
        .join("azul")
        .join(&summary.package_uuid)
        .join(&download.name);
    if let Some(reporter) = reporter {
        reporter
            .set_context(
                InstallErrorContext::new("download Azul Java archive")
                    .urls(vec![download.download_url.clone()])
                    .file_path(download.name.display().to_string())
                    .target_path(archive_path.display().to_string())
                    .expected_hash(download.sha256_hash.clone())
                    .expected_size(download.size)
                    .java_version(java_version)
                    .os(std::env::consts::OS)
                    .arch(std::env::consts::ARCH)
                    .build(),
            )
            .await?;
    }
    update_java_install_progress(
        reporter,
        java_version,
        InstallJavaStep::Downloading,
        Some(InstallProgress {
            current: 0,
            total: download.size,
            secondary: None,
        }),
    )
    .await?;
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 10.0, Some("Downloading java version"))?;
    }
    let mut last_reported_bytes = 0_u64;
    let download_reporter = reporter.cloned();
    let mut progress =
        |current: u64,
         total: u64|
         -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>> {
            let min_delta = (total / 200).max(JAVA_DOWNLOAD_PROGRESS_MIN_BYTES);
            if current < total
                && current.saturating_sub(last_reported_bytes) < min_delta
            {
                return Box::pin(async { Ok(()) });
            }
            last_reported_bytes = current;
            let reporter = download_reporter.clone();
            Box::pin(async move {
                update_java_install_progress(
                    reporter.as_ref(),
                    java_version,
                    InstallJavaStep::Downloading,
                    Some(InstallProgress {
                        current,
                        total,
                        secondary: None,
                    }),
                )
                .await
            })
        };
    let mut request =
        DownloadRequest::new(&download.download_url, ResourceClass::Java)
            .with_integrity(Integrity {
                size: None,
                sha256: Some(download.sha256_hash.clone()),
                content: ContentValidation::Jar,
                ..Integrity::default()
            });
    if let Some(reporter) = reporter {
        request = request.with_install_tracking(
            reporter.clone(),
            archive_path.display().to_string(),
            archive_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "java-runtime".to_string()),
        );
    }
    let download_result = download_to_path(
        request,
        &archive_path,
        &state.download_semaphore,
        &state.pool,
        Some(&mut progress as &mut FetchProgressFn<'_>),
    )
    .await?;
    if let Some(reporter) = reporter
        && download_result.attempts > 0
    {
        reporter
            .record_download_metrics(
                download_result.source.as_str(),
                download_result.fallback_count as u64,
            )
            .await?;
    }
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 80.0, Some("Extracting java"))?;
    }
    update_java_install_progress(
        reporter,
        java_version,
        InstallJavaStep::Extracting,
        Some(java_step_progress(3)),
    )
    .await?;

    let java_root = state.directories.java_versions_dir();
    let staging_root =
        java_root.join(format!(".azul-{}.installing", summary.package_uuid));
    remove_path_if_present(&staging_root).await?;
    io::create_dir_all(&staging_root).await?;
    let archive_path_for_extract = archive_path.clone();
    let staging_for_extract = staging_root.clone();
    let extracted_root = tokio::task::spawn_blocking(move || {
        extract_azul_archive(&archive_path_for_extract, &staging_for_extract)
    })
    .await??;
    let extracted_path = staging_root.join(&extracted_root);
    let final_root = java_root.join(&extracted_root);
    remove_path_if_present(&final_root).await?;
    tokio::fs::rename(&extracted_path, &final_root).await?;
    remove_path_if_present(&staging_root).await?;

    let executable = if cfg!(target_os = "macos") {
        final_root.join("Contents/Home/bin/java")
    } else {
        final_root.join("bin").join(jre::JAVA_BIN)
    };
    validate_installed_java(executable, java_version, reporter, loading_bar)
        .await
}

// Validates JRE at a given at a given path
pub async fn check_jre(path: PathBuf) -> crate::Result<JavaVersion> {
    jre::check_java_at_filepath(&path).await
}

// Test JRE at a given path
pub async fn test_jre(
    path: PathBuf,
    major_version: u32,
) -> crate::Result<bool> {
    let jre = match jre::check_java_at_filepath(&path).await {
        Ok(jre) => jre,
        Err(e) => {
            tracing::warn!("Invalid Java at {}: {e}", path.display());
            return Ok(false);
        }
    };
    let version = extract_java_version(&jre.version)?;
    tracing::info!(
        "Expected Java version {major_version}, and found {version} at {}",
        path.display()
    );
    Ok(version == major_version)
}

// ── JetBrains JDK feed download / parse ─────────────────────────────────────

static JDK_FEED_CACHE: LazyLock<tokio::sync::Mutex<Option<JdkFeed>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(None));

/// Fetches the JetBrains JDK feed, caching it after the first successful
/// download. The feed is an XZ-compressed JSON file listing all available
/// JDK builds across vendors, platforms and architectures.
async fn fetch_jdk_feed() -> crate::Result<JdkFeed> {
    // Check cache first
    {
        let cache = JDK_FEED_CACHE.lock().await;
        if let Some(ref feed) = *cache {
            return Ok(JdkFeed {
                jdks: feed.jdks.clone(),
            });
        }
    }

    // Download feed
    let client = crate::util::fetch::configured_client().await?;
    let response = client.get(JDK_FEED_URL).send().await?;
    let bytes = response.bytes().await?;

    // Decompress xz
    let mut decoder = XzDecoder::new(&bytes[..]);
    let mut decompressed = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut decompressed)?;

    let feed: JdkFeed = serde_json::from_slice(&decompressed)?;

    // Cache
    {
        let mut cache = JDK_FEED_CACHE.lock().await;
        *cache = Some(feed.clone());
    }

    Ok(feed)
}

fn map_platform_to_feed_os() -> &'static str {
    match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macOS",
        "windows" => "windows",
        _ => "linux",
    }
}

fn map_arch_to_feed_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => "x86_64",
    }
}

/// Lists available JDK vendors from the JetBrains feed that provide builds
/// for the current platform.
pub async fn list_java_feed_vendors() -> crate::Result<Vec<String>> {
    let feed = fetch_jdk_feed().await?;
    let os = map_platform_to_feed_os();
    let arch = map_arch_to_feed_arch();

    let mut vendors: Vec<String> = feed
        .jdks
        .iter()
        .filter(|e| e.packages.iter().any(|p| p.os == os && p.arch == arch))
        .map(|e| e.vendor.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    vendors.sort();
    Ok(vendors)
}

#[derive(Serialize)]
pub struct JdkVersionInfo {
    pub major_version: u32,
    pub full_version: String,
    pub vendor: String,
    pub product: String,
}

/// Lists available JDK versions for a given vendor from the JetBrains feed,
/// filtered to builds that match the current platform.
pub async fn list_java_feed_versions(
    vendor: &str,
) -> crate::Result<Vec<JdkVersionInfo>> {
    let feed = fetch_jdk_feed().await?;
    let os = map_platform_to_feed_os();
    let arch = map_arch_to_feed_arch();

    let mut versions: Vec<JdkVersionInfo> = feed
        .jdks
        .iter()
        .filter(|e| {
            e.vendor == vendor
                && e.packages.iter().any(|p| p.os == os && p.arch == arch)
        })
        .map(|e| JdkVersionInfo {
            major_version: e.jdk_version_major,
            full_version: e.jdk_version.clone(),
            vendor: e.vendor.clone(),
            product: e.product.clone().unwrap_or_default(),
        })
        .collect();

    versions.sort_by(|a, b| b.major_version.cmp(&a.major_version));
    versions.dedup_by(|a, b| a.major_version == b.major_version);
    Ok(versions)
}

/// Extracts a zip or tar.gz archive to the destination directory.
fn extract_archive(archive_path: &Path, dest: &Path) -> crate::Result<()> {
    // Try zip first
    {
        let file = std::fs::File::open(archive_path)?;
        if let Ok(mut archive) = zip::ZipArchive::new(file) {
            archive.extract(dest).map_err(|error| {
                crate::ErrorKind::InputError(format!(
                    "Failed to extract zip archive: {error}"
                ))
            })?;
            return Ok(());
        }
    }

    // Fall back to tar.gz
    let file = std::fs::File::open(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest)?;
    Ok(())
}

/// Downloads and installs a JDK build from the JetBrains feed. The download
/// is verified against the SHA-256 and size advertised in the feed before
/// extraction.
pub async fn download_java_from_feed(
    vendor: &str,
    jdk_version_major: u32,
) -> crate::Result<PathBuf> {
    download_java_from_feed_inner(vendor, jdk_version_major, None).await
}

/// Downloads Java from the JetBrains feed with an install progress reporter,
/// so the caller can observe progress through the install-job system.
pub async fn download_java_from_feed_with_reporter(
    vendor: &str,
    jdk_version_major: u32,
    reporter: InstallProgressReporter,
) -> crate::Result<PathBuf> {
    download_java_from_feed_inner(vendor, jdk_version_major, Some(reporter))
        .await
}

async fn download_java_from_feed_inner(
    vendor: &str,
    jdk_version_major: u32,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<PathBuf> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let state = State::get().await?;
    let _install_guard = JAVA_INSTALL_LOCK.lock().await;

    let loading_bar = if reporter.is_none() {
        Some(
            init_loading(
                LoadingBarType::JavaDownload {
                    version: jdk_version_major,
                },
                100.0,
                "Downloading java version",
            )
            .await?,
        )
    } else {
        None
    };

    if let Some(loading_bar) = &loading_bar {
        emit_loading(loading_bar, 0.0, Some("Fetching metadata"))?;
    }
    update_java_install_progress(
        reporter.as_ref(),
        jdk_version_major,
        InstallJavaStep::FetchingMetadata,
        Some(java_step_progress(1)),
    )
    .await?;

    let feed = fetch_jdk_feed().await?;
    let os = map_platform_to_feed_os();
    let arch = map_arch_to_feed_arch();

    let entry = feed
        .jdks
        .iter()
        .find(|e| {
            e.vendor == vendor
                && e.jdk_version_major == jdk_version_major
                && e.packages
                    .iter()
                    .any(|p| p.os == os && p.arch == arch)
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "No JDK found for {vendor} Java {jdk_version_major} on {os}/{arch}"
            ))
        })?;

    let pkg = entry
        .packages
        .iter()
        .find(|p| p.os == os && p.arch == arch)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No package for current platform".to_string(),
            )
        })?;

    if let Some(reporter) = &reporter {
        reporter
            .set_context(
                InstallErrorContext::new("download Java from feed")
                    .urls(vec![pkg.url.clone()])
                    .java_version(jdk_version_major)
                    .os(std::env::consts::OS)
                    .arch(std::env::consts::ARCH)
                    .build(),
            )
            .await?;
    }

    let metadata_dir = state.directories.metadata_dir();
    let java_dir = metadata_dir.join("java_versions");
    io::create_dir_all(&java_dir).await?;

    let dir_name = format!(
        "{}-jdk-{}",
        vendor.to_lowercase().replace(' ', "-"),
        jdk_version_major
    );
    let staging_root = java_dir.join(format!("{dir_name}.staging"));
    let final_root = java_dir.join(&dir_name);

    remove_path_if_present(&staging_root).await?;
    io::create_dir_all(&staging_root).await?;

    let archive_path = staging_root.join(&pkg.install_folder_name);

    // Download with integrity verification.
    // The JetBrains feed serves both .tar.gz and .zip archives, so we skip
    // ContentValidation::Jar (which only validates zip files) – SHA-256
    // verification already guarantees file integrity.
    let integrity = Integrity {
        size: Some(pkg.archive_size),
        sha256: Some(pkg.sha256.clone()),
        content: ContentValidation::None,
        ..Integrity::default()
    };

    // Track real download progress
    let mut download_pct = 0_f64;
    let reporter_for_progress = reporter.clone();
    let mut progress =
        |current: u64,
         total: u64|
         -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>> {
            if let Some(ref reporter) = reporter_for_progress {
                let reporter = reporter.clone();
                Box::pin(async move {
                    update_java_install_progress(
                        Some(&reporter),
                        jdk_version_major,
                        InstallJavaStep::Downloading,
                        Some(InstallProgress {
                            current,
                            total: total.max(1),
                            secondary: None,
                        }),
                    )
                    .await
                })
            } else {
                let pct = if total > 0 {
                    (current as f64 / total as f64) * 50.0
                } else {
                    0.0
                };
                let delta = pct - download_pct;
                download_pct = pct;
                let lb = loading_bar.clone();
                Box::pin(async move {
                    if let Some(lb) = &lb {
                        emit_loading(lb, delta, Some("Downloading..."))?;
                    }
                    Ok(())
                })
            }
        };

    let mut request = DownloadRequest::new(&pkg.url, ResourceClass::Java)
        .with_integrity(integrity);
    if let Some(reporter) = &reporter {
        request = request.with_install_tracking(
            reporter.clone(),
            archive_path.display().to_string(),
            pkg.install_folder_name.clone(),
        );
    }
    download_to_path(
        request,
        &archive_path,
        &state.download_semaphore,
        &state.pool,
        Some(&mut progress as &mut FetchProgressFn<'_>),
    )
    .await
    .map_err(|e| {
        tracing::warn!(
            "Failed to download Java from vendor {vendor} at URL {}: {e}",
            pkg.url
        );
        crate::Error::from(crate::ErrorKind::OtherError(format!(
            "Failed to download Java from {vendor}: {e}"
        )))
    })?;

    if let Some(reporter) = &reporter {
        // Report extracting phase via reporter
        update_java_install_progress(
            Some(reporter),
            jdk_version_major,
            InstallJavaStep::Extracting,
            Some(java_step_progress(3)),
        )
        .await?;
    }

    // Emit remaining delta to reach exactly 50 % before extraction (legacy path)
    if let Some(loading_bar) = &loading_bar {
        emit_loading(
            loading_bar,
            (50.0 - download_pct).max(0.0),
            Some("Extracting..."),
        )?;
    }

    // Extract archive (zip or tar.gz)
    let staging_for_extract = staging_root.clone();
    let archive_for_extract = archive_path.clone();
    tokio::task::spawn_blocking(move || {
        extract_archive(&archive_for_extract, &staging_for_extract)
    })
    .await??;

    io::remove_file(&archive_path).await?;

    // Find the extracted directory (first entry)
    let extracted_dir = {
        let mut entries = std::fs::read_dir(&staging_root)?;
        let first = entries.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Empty archive")
        })??;
        first.path()
    };

    remove_path_if_present(&final_root).await?;
    tokio::fs::rename(&extracted_dir, &final_root).await?;
    remove_path_if_present(&staging_root).await?;

    let executable = if cfg!(target_os = "macos") {
        final_root.join("Contents/Home/bin/java")
    } else {
        final_root.join("bin").join(jre::JAVA_BIN)
    };

    // Advance from 50 % (post-download) to 90 % to reflect installation work
    if let Some(loading_bar) = &loading_bar {
        emit_loading(loading_bar, 40.0, Some("Verifying installation"))?;
    }

    // Report validating phase via reporter
    update_java_install_progress(
        reporter.as_ref(),
        jdk_version_major,
        InstallJavaStep::Validating,
        Some(java_step_progress(4)),
    )
    .await?;

    let result = check_jre(executable).await?;
    let mut transaction = state.pool.begin().await?;
    result.upsert(&mut *transaction).await?;
    JavaVersion::set_default(
        result.parsed_version,
        &result.path,
        &mut *transaction,
    )
    .await?;
    if let Some(discovered) = DiscoveredJava::from_java(result.clone()) {
        discovered.upsert(&mut *transaction).await?;
    }
    transaction.commit().await?;

    // Complete the remaining 10 % of the bar
    if let Some(loading_bar) = &loading_bar {
        emit_loading(loading_bar, 10.0, None)?;
    }

    Ok(result.path.into())
}

fn system_memory() -> sysinfo::System {
    sysinfo::System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    )
}

fn system_memory_bytes() -> u64 {
    system_memory().total_memory()
}

#[cfg(target_os = "windows")]
pub(crate) fn system_available_memory_bytes() -> u64 {
    available_memory_bytes(&system_memory())
}

fn available_memory_bytes(system: &sysinfo::System) -> u64 {
    #[cfg(target_os = "macos")]
    {
        macos_available_memory_bytes()
            .unwrap_or_else(|| system.available_memory())
            .min(system.total_memory())
    }

    #[cfg(not(target_os = "macos"))]
    {
        system.available_memory()
    }
}

#[cfg(target_os = "macos")]
fn macos_available_memory_bytes() -> Option<u64> {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return None;
    }

    let mut statistics = unsafe { std::mem::zeroed::<libc::vm_statistics64>() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    #[allow(deprecated)]
    let host = unsafe { libc::mach_host_self() };
    let result = unsafe {
        libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            &mut statistics as *mut libc::vm_statistics64 as *mut _,
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return None;
    }

    Some(
        u64::from(statistics.free_count)
            .saturating_add(u64::from(statistics.inactive_count))
            .saturating_add(u64::from(statistics.purgeable_count))
            .saturating_mul(page_size as u64),
    )
}

/// Recommended default max heap (MiB) for new instances based on system RAM.
pub fn default_memory_max_mb() -> u32 {
    const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
    let system_gib = system_memory_bytes() / BYTES_PER_GIB;

    if system_gib < 8 {
        1024 * 2
    } else if system_gib >= 24 {
        1024 * 6
    } else {
        1024 * 4
    }
}

// Gets maximum memory in KiB.
pub async fn get_max_memory() -> crate::Result<u64> {
    Ok(system_memory_bytes() / 1024)
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct MemoryStatus {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub allocated_mb: u32,
    pub optimization_supported: bool,
}

pub async fn get_memory_status(
    instance_id: Option<&str>,
    requested_memory_mb: u32,
    automatic: bool,
) -> crate::Result<MemoryStatus> {
    let state = State::get().await?;
    let system = system_memory();
    let total_bytes = system.total_memory();
    let available_bytes = available_memory_bytes(&system);
    let (modded, mod_count) = if let Some(instance_id) = instance_id {
        let context =
            crate::state::instances::commands::get_instance_launch_context(
                instance_id,
                &state.pool,
            )
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::OtherError(format!(
                    "Tried to inspect a nonexistent instance {instance_id}"
                ))
                .as_error()
            })?;
        let modded = matches!(
            context.applied_content_set.loader,
            crate::state::ModLoader::Forge
                | crate::state::ModLoader::Fabric
                | crate::state::ModLoader::Quilt
                | crate::state::ModLoader::NeoForge
                | crate::state::ModLoader::Cleanroom
                | crate::state::ModLoader::LiteLoader
                | crate::state::ModLoader::LegacyFabric
        );
        let path = state.directories.instance_game_dir(&context.instance);
        (modded, count_mods(&path))
    } else {
        (false, 0)
    };
    let allocated_mb = if automatic {
        automatic_memory_max_mb(available_bytes, mod_count, modded)
    } else {
        requested_memory_mb
    };

    Ok(MemoryStatus {
        total_bytes,
        available_bytes,
        allocated_mb,
        optimization_supported: crate::api::memory::optimization_supported(),
    })
}

/// Calculates a launch heap using four progressively conservative stages.
pub fn automatic_memory_max_mb(
    available_memory_bytes: u64,
    mod_count: usize,
    modded: bool,
) -> u32 {
    const BYTES_PER_GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let mut available_gib = ((available_memory_bytes as f64 / BYTES_PER_GIB)
        * 10.0)
        .round_ties_even()
        / 10.0;
    let (minimum, target1, target2, target3) = if modded {
        (
            0.5 + mod_count as f64 / 150.0,
            1.5 + mod_count as f64 / 90.0,
            2.7 + mod_count as f64 / 50.0,
            4.5 + mod_count as f64 / 25.0,
        )
    } else {
        (0.5, 1.5, 2.5, 4.0)
    };

    let mut allocated = 0.0;
    let stages = [
        (target1, 1.0),
        (target2 - target1, 0.7),
        (target3 - target2, 0.4),
        (target3, 0.15),
    ];
    for (delta, ratio) in stages {
        allocated += (available_gib * ratio).min(delta);
        available_gib -= delta / ratio;
        if available_gib < 0.1 {
            break;
        }
    }

    let allocated_gib =
        (allocated.max(minimum) * 10.0).round_ties_even() / 10.0;
    (allocated_gib * 1024.0).floor().max(512.0) as u32
}

/// Calculates automatic memory from the current available RAM and installed mods.
pub fn automatic_memory_max_mb_for_instance(
    instance_path: &std::path::Path,
    modded: bool,
) -> u32 {
    let mod_count = if modded { count_mods(instance_path) } else { 0 };

    automatic_memory_max_mb(
        available_memory_bytes(&system_memory()),
        mod_count,
        modded,
    )
}

fn count_mods(instance_path: &std::path::Path) -> usize {
    std::fs::read_dir(instance_path.join("mods"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "jar" | "zip" | "litemod"
                    )
                })
                .unwrap_or(false)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn closest_compatible_prefers_exact_then_newest_small_enough() {
        let java = |parsed_version: u32, path: &str| JavaVersion {
            parsed_version,
            version: parsed_version.to_string(),
            architecture: "x86_64".to_string(),
            path: path.to_string(),
            distribution: None,
        };
        let jres =
            [java(8, "/java8"), java(21, "/java21"), java(25, "/java25")];

        assert_eq!(
            closest_compatible_java(jres.iter(), 17),
            Some(java(21, "/java21"))
        );
        assert_eq!(
            closest_compatible_java(jres.iter(), 8),
            Some(java(8, "/java8"))
        );
        assert_eq!(closest_compatible_java(jres.iter(), 26), None);
    }

    #[test]
    fn automatic_memory_matches_vanilla_stages() {
        assert_eq!(automatic_memory_max_mb(GIB, 0, false), 1024);
        assert_eq!(automatic_memory_max_mb(4 * GIB, 0, false), 2969);
        assert_eq!(automatic_memory_max_mb(16 * GIB, 0, false), 5529);
    }

    #[test]
    fn automatic_memory_matches_mod_targets() {
        assert_eq!(automatic_memory_max_mb(8 * GIB, 100, true), 5836);
        assert_eq!(automatic_memory_max_mb(0, 300, true), 2560);
    }
}
