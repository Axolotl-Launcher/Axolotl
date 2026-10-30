//! Downloader for Minecraft data

use crate::data::ModLoader;
use crate::install::{
    InstallErrorContext, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter,
};
use crate::instance::QuickPlayType;
use crate::launcher::parse_rules;
use crate::util::download::constants::DERIVED_VERSION_CACHE_FORMAT;
use crate::util::download::constants::FABRIC_MAVEN;
use crate::util::download::constants::FORGE_MAVEN;
use crate::util::download::constants::LAUNCHER_META_MAVEN;
pub(crate) use crate::util::download::constants::LIBRARIES_MAVEN;
use crate::util::download::constants::MAVEN_CENTRAL;
use crate::util::download::constants::MINECRAFT_DOWNLOAD_PROGRESS_MIN_BYTES;
use crate::util::download::constants::NEOFORGE_MAVEN;
use crate::util::download::constants::QUILT_MAVEN;
use crate::util::download::constants::SPONGE_MAVEN;
use crate::{
    event::{
        LoadingBarId,
        emit::{emit_loading, loading_try_for_each_concurrent},
    },
    state::State,
    util::download::h2_download::{
        ASSET_BATCH_CONCURRENCY, H2BatchAsset, download_asset_batch_via_h2,
    },
    util::{fetch::*, io},
};
use daedalus::minecraft::{LibraryDownload, LoggingConfiguration, LoggingSide};
use daedalus::{
    self as d,
    minecraft::{
        Asset, AssetsIndex, Library, Version as GameVersion,
        VersionInfo as GameVersionInfo,
    },
    modded::LoaderVersion,
};
use futures::prelude::*;
use reqwest::Method;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalRuntimeSource {
    pub root: PathBuf,
}

impl LocalRuntimeSource {
    pub fn discover(instance_or_dotminecraft: &Path) -> Option<Self> {
        if is_local_runtime_root(instance_or_dotminecraft) {
            return Some(Self {
                root: instance_or_dotminecraft.to_path_buf(),
            });
        }

        let dotminecraft = instance_or_dotminecraft.join(".minecraft");
        if is_local_runtime_root(&dotminecraft) {
            return Some(Self { root: dotminecraft });
        }

        instance_or_dotminecraft
            .ancestors()
            .find(|path| path.join("versions").is_dir())
            .map(|root| Self {
                root: root.to_path_buf(),
            })
    }
}

fn is_local_runtime_root(path: &Path) -> bool {
    path.join("assets").is_dir() && path.join("libraries").is_dir()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactAvailability {
    Cached,
    LocalReusable,
    NetworkRequired,
}

pub async fn classify_local_artifact(
    local: Option<&LocalRuntimeSource>,
    destination: &Path,
    relative_path: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
) -> crate::Result<ArtifactAvailability> {
    if destination.exists() {
        let cached = std::fs::metadata(destination).is_ok_and(|metadata| {
            metadata.is_file()
                && expected_size.is_none_or(|size| metadata.len() == size)
        });
        if cached {
            return Ok(ArtifactAvailability::Cached);
        }
    }

    let Some(local) = local else {
        return Ok(ArtifactAvailability::NetworkRequired);
    };
    let Some(expected_sha1) = expected_sha1 else {
        return Ok(ArtifactAvailability::NetworkRequired);
    };

    let candidate = local.root.join(relative_path);
    if !candidate.is_file() {
        return Ok(ArtifactAvailability::NetworkRequired);
    }

    let metadata = io::metadata(&candidate).await?;
    if let Some(expected_size) = expected_size
        && metadata.len() != expected_size
    {
        return Ok(ArtifactAvailability::NetworkRequired);
    }

    let (_, actual_sha1) = sha1_file_async(&candidate).await?;
    if actual_sha1.eq_ignore_ascii_case(expected_sha1) {
        Ok(ArtifactAvailability::LocalReusable)
    } else {
        Ok(ArtifactAvailability::NetworkRequired)
    }
}

pub async fn copy_verified_local_artifact(
    st: &State,
    local: &LocalRuntimeSource,
    relative_path: &Path,
    destination: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    progress: Option<&MinecraftDownloadProgress>,
    context: InstallErrorContext,
) -> crate::Result<bool> {
    let source = local.root.join(relative_path);
    if let Some(progress) = progress {
        progress.set_context(context.clone()).await?;
    }
    let copied = match super::local_artifact::copy_verified(
        &source,
        destination,
        expected_sha1,
        expected_size,
        &st.io_semaphore,
    )
    .await
    {
        Ok(copied) => copied,
        Err(error) => {
            if let Some(progress) = progress {
                progress.persist_failure_context(context).await;
            }
            return Err(error);
        }
    };
    if !copied {
        return Ok(false);
    }

    let size = match expected_size {
        Some(size) => size,
        None => io::metadata(&source).await?.len(),
    };
    if let Some(progress) = progress {
        progress.add_bytes(size).await?;
    }

    Ok(true)
}

#[derive(Clone, Debug)]
pub struct MinecraftDownloadProgress {
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    current: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    last_reported: Arc<AtomicU64>,
    source: Arc<Mutex<Option<String>>>,
    fallback_count: Arc<AtomicU64>,
}

impl MinecraftDownloadProgress {
    async fn new(
        reporter: InstallProgressReporter,
        details: InstallPhaseDetails,
        total: u64,
    ) -> crate::Result<Self> {
        let progress = Self {
            reporter,
            details,
            current: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(total)),
            last_reported: Arc::new(AtomicU64::new(0)),
            source: Arc::new(Mutex::new(None)),
            fallback_count: Arc::new(AtomicU64::new(0)),
        };

        if total > 0 {
            progress.emit_progress(0, total).await?;
        }

        Ok(progress)
    }

    pub(crate) async fn add_total(&self, total: u64) -> crate::Result<()> {
        if total == 0 {
            return Ok(());
        }

        let total = self.total.fetch_add(total, Ordering::Relaxed) + total;
        self.emit_if_needed(self.current.load(Ordering::Relaxed), total, true)
            .await
    }

    pub(crate) async fn add_bytes(&self, bytes: u64) -> crate::Result<()> {
        if bytes == 0 {
            return Ok(());
        }

        let current = self.current.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let total = self.total.load(Ordering::Relaxed);
        self.emit_if_needed(current, total, false).await
    }

    /// Rolls back bytes that were counted for an abandoned download attempt,
    /// so retried files do not inflate the progress bar to 100% early.
    async fn sub_bytes(&self, bytes: u64) -> crate::Result<()> {
        if bytes == 0 {
            return Ok(());
        }

        let mut current = self.current.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_sub(bytes);
            match self.current.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    current = next;
                    break;
                }
                Err(actual) => current = actual,
            }
        }
        let total = self.total.load(Ordering::Relaxed);
        self.emit_if_needed(current, total, true).await
    }

    async fn emit_if_needed(
        &self,
        current: u64,
        total: u64,
        force: bool,
    ) -> crate::Result<()> {
        if total == 0 {
            return Ok(());
        }

        let min_delta =
            (total / 200).max(MINECRAFT_DOWNLOAD_PROGRESS_MIN_BYTES);
        let last_reported = self.last_reported.load(Ordering::Relaxed);
        if !force
            && current < total
            && current.saturating_sub(last_reported) < min_delta
        {
            return Ok(());
        }

        self.last_reported.store(current, Ordering::Relaxed);
        self.emit_progress(current, total).await
    }

    async fn emit_progress(
        &self,
        current: u64,
        total: u64,
    ) -> crate::Result<()> {
        // A zero total represents a fully cached/no-op Minecraft install.
        // Persist it as a completed unit so parallel modpack progress does
        // not remain indeterminate after the task returned successfully.
        let (current, total) =
            if total == 0 { (1, 1) } else { (current, total) };
        self.reporter
            .update(
                InstallPhaseId::DownloadingMinecraft,
                Some(InstallProgress {
                    current: current.min(total),
                    total,
                    secondary: None,
                }),
                self.details.clone(),
            )
            .await
    }

    async fn set_context(
        &self,
        context: InstallErrorContext,
    ) -> crate::Result<()> {
        self.reporter.set_transient_context(context).await
    }

    async fn persist_failure_context(&self, context: InstallErrorContext) {
        self.reporter.persist_failure_context(context).await;
    }

    fn record_download_result(&self, result: &DownloadResult) {
        if result.attempts > 0
            && let Ok(mut source) = self.source.lock()
        {
            *source = Some(result.source.as_str().to_string());
        }
        self.fallback_count
            .fetch_add(result.fallback_count as u64, Ordering::Relaxed);
    }

    pub(crate) async fn finish(&self) -> crate::Result<()> {
        // The initial missing-byte estimate can be stale when another install,
        // a local runtime source, or a cache satisfies the file before this
        // task reaches it. A successful Minecraft install is terminal for this
        // progress track, so publish its total as complete instead of leaving
        // a misleading `0/N` download bar behind.
        let total = self.total.load(Ordering::Relaxed);
        self.emit_progress(total, total).await?;
        let source = self.source.lock().ok().and_then(|source| source.clone());
        let fallback_count = self.fallback_count.load(Ordering::Relaxed);
        if let Some(source) = source {
            self.reporter
                .record_download_metrics(source, fallback_count)
                .await?;
        }
        Ok(())
    }
}

async fn download_minecraft_file(
    st: &State,
    url: &str,
    sha1: Option<&str>,
    expected_size: Option<u64>,
    destination: &std::path::Path,
    resource: ResourceClass,
    content_validation: ContentValidation,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
    context: InstallErrorContext,
) -> crate::Result<DownloadResult> {
    download_minecraft_file_with_semaphore(
        st,
        url,
        sha1,
        expected_size,
        destination,
        resource,
        content_validation,
        force,
        progress,
        context,
        &st.download_semaphore,
    )
    .await
}

async fn download_minecraft_file_with_semaphore(
    st: &State,
    url: &str,
    sha1: Option<&str>,
    expected_size: Option<u64>,
    destination: &std::path::Path,
    resource: ResourceClass,
    content_validation: ContentValidation,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
    context: InstallErrorContext,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    let urls = minecraft_library_mirrors(url);
    download_minecraft_file_with_candidates(
        st,
        &urls,
        sha1,
        expected_size,
        destination,
        resource,
        content_validation,
        force,
        progress,
        context,
        semaphore,
    )
    .await
}

async fn download_minecraft_file_with_candidates(
    _st: &State,
    urls: &[String],
    sha1: Option<&str>,
    expected_size: Option<u64>,
    destination: &std::path::Path,
    resource: ResourceClass,
    content_validation: ContentValidation,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
    context: InstallErrorContext,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    let Some(url) = urls.first() else {
        return Err(crate::ErrorKind::LauncherError(
            "No trusted download URL is available".to_string(),
        )
        .into());
    };
    let mut context = context;
    context.urls.extend(urls.iter().cloned());
    context.expected_hash = sha1.map(str::to_string);
    context.expected_size = expected_size;
    if let Some(progress) = &progress {
        progress.set_context(context.clone()).await?;
    }
    if force && destination.exists() {
        io::remove_file(destination).await?;
    }

    let integrity = Integrity {
        size: expected_size,
        sha1: sha1.map(str::to_string),
        content: content_validation,
        ..Integrity::default()
    };
    let mut request = DownloadRequest::new(url, resource)
        .with_candidate_urls(urls.iter().skip(1).cloned())
        .with_integrity(integrity);
    if let Some(progress) = &progress {
        request = request.with_install_tracking(
            progress.reporter.clone(),
            destination.display().to_string(),
            destination
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| url.to_string()),
        );
    }
    let Some(progress) = progress else {
        return download_to_path(request, destination, semaphore, None).await;
    };

    let last_downloaded = Arc::new(AtomicU64::new(0));
    let mut progress_fn = {
        let progress = progress.clone();
        let last_downloaded = last_downloaded.clone();
        move |downloaded: u64,
              _total: u64|
              -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>> {
            let previous =
                last_downloaded.swap(downloaded, Ordering::Relaxed);
            let progress = progress.clone();
            Box::pin(async move {
                if downloaded >= previous {
                    progress.add_bytes(downloaded - previous).await
                } else {
                    // The downloaded count went backwards, meaning a failed
                    // attempt restarted; un-count the abandoned bytes.
                    progress.sub_bytes(previous - downloaded).await
                }
            })
        }
    };

    let result = match download_to_path(
        request,
        destination,
        semaphore,
        Some(&mut progress_fn as &mut FetchProgressFn<'_>),
    )
    .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            progress.persist_failure_context(context).await;
            return Err(error);
        }
    };
    progress.record_download_result(&result);

    if let Some(expected_size) = expected_size {
        let downloaded = last_downloaded.load(Ordering::Relaxed);
        progress
            .add_bytes(expected_size.saturating_sub(downloaded))
            .await?;
    }

    Ok(result)
}

pub(crate) fn minecraft_library_mirrors(url: &str) -> Vec<String> {
    use crate::util::download::constants::MACHINA_LWJGL_RELEASE;

    use crate::util::download::constants::MOJANG_LWJGL_PATH;

    let mut mirrors = vec![url.to_string()];
    if let Some(file_name) = url.strip_prefix(MACHINA_LWJGL_RELEASE) {
        mirrors.push(format!("{MOJANG_LWJGL_PATH}{file_name}"));
    }
    mirrors
}

pub(crate) fn legacy_library_download_urls(
    repository: Option<&str>,
    artifact_path: &str,
) -> Option<Vec<String>> {
    if artifact_path.starts_with('/')
        || artifact_path.contains('\\')
        || artifact_path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return None;
    }

    let repository = repository.unwrap_or(LIBRARIES_MAVEN);
    let mut repository_url = Url::parse(repository).ok()?;
    if repository_url.scheme() != "https"
        || repository_url.host_str().is_none()
        || !repository_url.username().is_empty()
        || repository_url.password().is_some()
        || repository_url.query().is_some()
        || repository_url.fragment().is_some()
    {
        return None;
    }
    let normalized_path =
        format!("{}/", repository_url.path().trim_end_matches('/'));
    repository_url.set_path(&normalized_path);
    let declared_url = repository_url.join(artifact_path).ok()?.to_string();
    let canonical_urls = legacy_library_canonical_urls(artifact_path);
    let mut candidates = Vec::new();

    if repository.trim_end_matches('/') == LAUNCHER_META_MAVEN {
        candidates.extend(canonical_urls);
        candidates.push(declared_url);
    } else {
        candidates.push(declared_url);
        candidates.extend(canonical_urls);
    }
    candidates.push(format!("{LIBRARIES_MAVEN}/{artifact_path}"));

    let mut urls = Vec::new();
    for candidate in candidates {
        if !urls.contains(&candidate) {
            urls.push(candidate);
        }
    }

    Some(urls)
}

fn legacy_library_canonical_urls(artifact_path: &str) -> Vec<String> {
    let in_repository =
        |repository: &str| format!("{repository}/{artifact_path}");

    if artifact_path.starts_with("org/scala-lang/scala-parser-combinators_")
        || artifact_path.starts_with("org/scala-lang/scala-swing_")
        || artifact_path.starts_with("org/scala-lang/scala-xml_")
    {
        vec![in_repository(FORGE_MAVEN)]
    } else if artifact_path.starts_with("net/fabricmc/") {
        vec![in_repository(FABRIC_MAVEN)]
    } else if artifact_path.starts_with("org/quiltmc/") {
        vec![in_repository(QUILT_MAVEN)]
    } else if artifact_path.starts_with("net/minecraftforge/")
        || artifact_path.starts_with("cpw/mods/")
    {
        vec![in_repository(FORGE_MAVEN)]
    } else if artifact_path.starts_with("net/neoforged/") {
        vec![in_repository(NEOFORGE_MAVEN)]
    } else if artifact_path.starts_with("org/spongepowered/") {
        vec![in_repository(SPONGE_MAVEN)]
    } else if artifact_path.starts_with("net/minecraft/launchwrapper/") {
        vec![in_repository(LIBRARIES_MAVEN)]
    } else if artifact_path.starts_with("org/ow2/")
        || artifact_path.starts_with("org/scala-lang/")
        || artifact_path.starts_with("org/jline/")
        || artifact_path.starts_with("jline/")
        || artifact_path.starts_with("net/java/dev/jna/")
        || artifact_path.starts_with("com/typesafe/")
    {
        vec![in_repository(MAVEN_CENTRAL)]
    } else if artifact_path.starts_with("com/modrinth/daedalus/") {
        vec![in_repository(LAUNCHER_META_MAVEN)]
    } else {
        Vec::new()
    }
}

fn legacy_library_content_validation(artifact_path: &str) -> ContentValidation {
    if artifact_path.ends_with(".jar") {
        ContentValidation::Jar
    } else {
        ContentValidation::None
    }
}

pub(crate) fn legacy_library_sha1(library: &Library) -> Option<&str> {
    library
        .checksums
        .as_deref()
        .and_then(|checksums| {
            checksums.iter().find(|checksum| {
                checksum.len() == 40
                    && checksum
                        .bytes()
                        .all(|character| character.is_ascii_hexdigit())
            })
        })
        .map(String::as_str)
}

pub(crate) fn local_asset_index_path(asset_index_id: &str) -> PathBuf {
    Path::new("assets")
        .join("indexes")
        .join(format!("{asset_index_id}.json"))
}

pub(crate) fn local_asset_object_path(hash: &str) -> PathBuf {
    Path::new("assets")
        .join("objects")
        .join(&hash[..2])
        .join(hash)
}

pub(crate) fn local_library_path(library_name: &str) -> crate::Result<PathBuf> {
    Ok(Path::new("libraries").join(d::get_path_from_artifact(library_name)?))
}

pub(crate) fn local_native_library_path(
    library: &Library,
    native: &LibraryDownload,
    classifier: &str,
) -> crate::Result<PathBuf> {
    let artifact_path = match native.path.as_deref() {
        Some(path) => path.to_string(),
        None => native_library_artifact_path(library, classifier)?,
    };

    Ok(Path::new("libraries").join(artifact_path))
}

pub(crate) fn is_native_library(library: &Library) -> bool {
    library.natives.is_some()
        || library_classifier(&library.name)
            .is_some_and(|classifier| classifier.starts_with("natives-"))
}

/// Whether this library carries a Java artifact (regular JAR) that must be
/// downloaded. Processor dependencies are represented by libraries with
/// `include_in_classpath` set to false, so that flag cannot be used to decide
/// whether the artifact is needed. A library can also have both a Java
/// artifact and native classifiers after manifest merging (LWJGL is the
/// canonical example); the two are independent and must not be treated as
/// mutually exclusive.
pub(crate) fn needs_java_artifact(library: &Library) -> bool {
    // Four-part native coordinates (group:artifact:version:natives-*) store
    // their native archive metadata in downloads.artifact, which is not a
    // Java JAR. Exclude them so they only produce NativeArtifact tasks.
    if library_classifier(&library.name)
        .is_some_and(|classifier| classifier.starts_with("natives-"))
    {
        return false;
    }
    // Legacy pure-native libraries carry a natives map but no downloads.artifact;
    // their main JAR is not downloaded by the original installer either.
    if library.natives.is_some()
        && library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.artifact.as_ref())
            .is_none_or(|artifact| artifact.url.trim().is_empty())
        && library
            .url
            .as_deref()
            .is_none_or(|url| url.trim().is_empty())
    {
        return false;
    }
    library
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref())
        .is_some_and(|artifact| !artifact.url.trim().is_empty())
        || library
            .url
            .as_deref()
            .is_some_and(|url| !url.trim().is_empty())
        || library.include_in_classpath
}

fn java_artifact_applies(
    library: &Library,
    java_arch: &str,
    minecraft_updated: bool,
) -> bool {
    library.downloadable
        && needs_java_artifact(library)
        && library.rules.as_ref().is_none_or(|rules| {
            parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        })
}

fn library_classifier(library_name: &str) -> Option<&str> {
    let mut coordinates = library_name.split(':');
    coordinates.next()?;
    coordinates.next()?;
    coordinates.next()?;
    coordinates
        .next()
        .and_then(|classifier| classifier.split('@').next())
}

pub(crate) fn native_library_artifact_path(
    library: &Library,
    classifier: &str,
) -> crate::Result<String> {
    // A four-part coordinate is only the native artifact path when its
    // classifier is a native classifier. Other classifiers (sources,
    // javadoc, etc.) can occur in loader metadata and must retain the
    // legacy "append classifier" behaviour when this helper is reused.
    if library_classifier(&library.name)
        .is_some_and(|value| value.starts_with("natives-"))
    {
        Ok(d::get_path_from_artifact(&library.name)?)
    } else {
        classified_library_artifact_path(&library.name, classifier)
    }
}

pub(crate) fn classified_library_artifact_path(
    library_name: &str,
    classifier: &str,
) -> crate::Result<String> {
    let artifact_path = d::get_path_from_artifact(library_name)?;
    Ok(
        if let Some((prefix, extension)) = artifact_path.rsplit_once('.') {
            format!("{prefix}-{classifier}.{extension}")
        } else {
            format!("{artifact_path}-{classifier}")
        },
    )
}

pub(crate) fn library_native_classifier(
    library: &Library,
    java_arch: &str,
) -> Option<String> {
    let os = d::minecraft::Os::native_arch(java_arch);
    let base_os = os.get_os();
    library
        .natives
        .as_ref()
        .and_then(|natives| natives.get(&os).or_else(|| natives.get(&base_os)))
        .map(|classifier| {
            classifier.replace("${arch}", crate::util::platform::ARCH_WIDTH)
        })
        .or_else(|| library_classifier(&library.name).map(str::to_owned))
}

pub(crate) fn local_client_path(game_version: &str) -> PathBuf {
    Path::new("versions")
        .join(game_version)
        .join(format!("{game_version}.jar"))
}

pub(crate) fn local_log_config_path(log_config_id: &str) -> PathBuf {
    Path::new("assets").join("log_configs").join(log_config_id)
}

async fn try_reuse_local_artifact(
    st: &State,
    local: Option<&LocalRuntimeSource>,
    relative_path: &Path,
    destination: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    progress: Option<&MinecraftDownloadProgress>,
    context: InstallErrorContext,
) -> crate::Result<bool> {
    let Some(local) = local else {
        return Ok(false);
    };

    copy_verified_local_artifact(
        st,
        local,
        relative_path,
        destination,
        expected_sha1,
        expected_size,
        progress,
        context,
    )
    .await
}

async fn download_or_reuse_local<F, Fut>(
    st: &State,
    local: Option<&LocalRuntimeSource>,
    relative_path: &Path,
    destination: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    progress: Option<&MinecraftDownloadProgress>,
    context: InstallErrorContext,
    force: bool,
    download: F,
) -> crate::Result<bool>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = crate::Result<DownloadResult>>,
{
    if !force
        && try_reuse_local_artifact(
            st,
            local,
            relative_path,
            destination,
            expected_sha1,
            expected_size,
            progress,
            context,
        )
        .await?
    {
        return Ok(true);
    }
    download().await?;
    Ok(false)
}

fn should_download(path_exists: bool, force: bool) -> bool {
    !path_exists || force
}

fn missing_client_bytes(
    st: &State,
    version: &GameVersionInfo,
    force: bool,
) -> crate::Result<u64> {
    let client_download = version
        .downloads
        .get(&d::minecraft::DownloadType::Client)
        .ok_or(
            crate::ErrorKind::LauncherError(format!(
                "No client downloads exist for version {}",
                version.id
            ))
            .as_error(),
        )?;
    let path = st
        .directories
        .version_dir(&version.id)
        .join(format!("{}.jar", version.id));

    Ok(if should_download(path.exists(), force) {
        client_download.size as u64
    } else {
        0
    })
}

fn missing_assets_index_bytes(
    st: &State,
    version: &GameVersionInfo,
    force: bool,
) -> u64 {
    let path = st
        .directories
        .assets_index_dir()
        .join(format!("{}.json", &version.asset_index.id));

    if should_download(path.exists(), force) {
        version.asset_index.size as u64
    } else {
        0
    }
}

fn asset_file_is_usable(path: &Path, expected_size: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| {
        metadata.is_file() && metadata.len() == expected_size
    })
}

async fn verified_asset_is_usable(
    path: &Path,
    sha1: &str,
    size: u64,
) -> crate::Result<bool> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) if !metadata.is_file() || metadata.len() != size => {
            return Ok(false);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    }
    match verify_file(path, &Integrity::sha1(sha1).with_size(size)).await {
        Ok(_) => Ok(true),
        Err(error) if is_integrity_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn missing_log_config_bytes(
    st: &State,
    version: &GameVersionInfo,
    force: bool,
) -> u64 {
    let log_download = version
        .logging
        .as_ref()
        .and_then(|x| x.get(&LoggingSide::Client));
    let Some(LoggingConfiguration::Log4j2Xml {
        file: log_download, ..
    }) = log_download
    else {
        return 0;
    };

    let path = st.directories.log_configs_dir().join(&log_download.id);
    if should_download(path.exists(), force) {
        log_download.size as u64
    } else {
        0
    }
}

fn missing_asset_bytes(
    st: &State,
    with_legacy: bool,
    index: &AssetsIndex,
    force: bool,
) -> u64 {
    index
        .objects
        .iter()
        .filter_map(|(name, asset)| {
            let hash = &asset.hash;
            let object_path = st.directories.object_dir(hash);
            let legacy_path = st.directories.legacy_assets_dir().join(
                name.replace('/', &String::from(std::path::MAIN_SEPARATOR)),
            );
            let should_fetch_object =
                force || !asset_file_is_usable(&object_path, asset.size as u64);
            let should_fetch_legacy = with_legacy
                && (force
                    || !asset_file_is_usable(&legacy_path, asset.size as u64));

            (should_fetch_object || should_fetch_legacy)
                .then_some(asset.size as u64)
        })
        .sum()
}

fn missing_library_bytes(
    st: &State,
    libraries: &[Library],
    java_arch: &str,
    force: bool,
    minecraft_updated: bool,
) -> crate::Result<u64> {
    let mut total = 0;

    for library in libraries {
        if let Some(rules) = &library.rules
            && !parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }

        if !library.downloadable {
            continue;
        }

        // Native library size for this platform, if any.
        if is_native_library(library)
            && let Some(classifier) =
                library_native_classifier(library, java_arch)
            && let Some(native) = library
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.classifiers.as_ref())
                .and_then(|classifiers| classifiers.get(&classifier))
        {
            total += native.size as u64;
        }

        // Java artifact size. Mixed libraries carry both.
        if needs_java_artifact(library) {
            let artifact_path = d::get_path_from_artifact(&library.name)?;
            let path = st.directories.libraries_dir().join(&artifact_path);

            if (!path.exists() || force)
                && let Some(artifact) = library
                    .downloads
                    .as_ref()
                    .and_then(|downloads| downloads.artifact.as_ref())
                && !artifact.url.is_empty()
            {
                total += artifact.size as u64;
            }
        }
    }

    Ok(total)
}

fn missing_initial_minecraft_bytes(
    st: &State,
    version: &GameVersionInfo,
    java_arch: &str,
    force: bool,
    minecraft_updated: bool,
) -> crate::Result<u64> {
    Ok(missing_client_bytes(st, version, force)?
        + missing_assets_index_bytes(st, version, force)
        + missing_log_config_bytes(st, version, force)
        + missing_library_bytes(
            st,
            version.libraries.as_slice(),
            java_arch,
            force,
            minecraft_updated,
        )?)
}

#[tracing::instrument(skip_all, fields(version = version.id.as_str()))]
#[allow(clippy::too_many_arguments)]
pub async fn download_minecraft(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    game_version: &str,
    version: &GameVersionInfo,
    loading_bar: Option<&LoadingBarId>,
    java_arch: &str,
    force: bool,
    minecraft_updated: bool,
    reporter: Option<InstallProgressReporter>,
    phase_details: InstallPhaseDetails,
) -> crate::Result<Option<MinecraftDownloadProgress>> {
    tracing::info!("Downloading Minecraft version {}", version.id);
    let progress = if let Some(reporter) = reporter {
        Some(
            MinecraftDownloadProgress::new(
                reporter,
                phase_details,
                missing_initial_minecraft_bytes(
                    st,
                    version,
                    java_arch,
                    force,
                    minecraft_updated,
                )?,
            )
            .await?,
        )
    } else {
        None
    };

    let amount = if version.processors.as_ref().is_some_and(|x| !x.is_empty()) {
        25.0
    } else {
        40.0
    };

    tokio::try_join! {
        async {
            let assets_index = download_assets_index(
                st,
                local_source,
                version,
                loading_bar,
                force,
                progress.clone(),
            )
            .await?;
            if let Some(progress) = &progress {
                progress
                    .add_total(missing_asset_bytes(
                        st,
                        version.assets == "legacy",
                        &assets_index,
                        force,
                    ))
                    .await?;
            }
            download_assets(
                st,
                local_source,
                version.assets == "legacy",
                &assets_index,
                loading_bar,
                amount,
                force,
                progress.clone(),
            )
            .await
        },
        async {
            tokio::try_join! {
                download_client(st, local_source, game_version, version, loading_bar, force, progress.clone()),
                download_log_config(st, local_source, version, loading_bar, force, progress.clone()),
                download_libraries(st, local_source, version.libraries.as_slice(), &version.id, loading_bar, amount, java_arch, force, minecraft_updated, progress.clone())
            }?;
            Ok::<_, crate::Error>(())
        }
    }?;
    tracing::info!("Done downloading Minecraft!");
    Ok(progress)
}

#[tracing::instrument(skip_all, fields(version = version.id.as_str(), loader = ?loader))]

pub async fn download_version_info(
    st: &State,
    version: &GameVersion,
    mod_loader: ModLoader,
    loader: Option<&LoaderVersion>,
    force: Option<bool>,
    loading_bar: Option<&LoadingBarId>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<GameVersionInfo> {
    let version_id = loader
        .map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));
    tracing::debug!("Loading version info for Minecraft {version_id}");
    let path = st
        .directories
        .version_dir(&version_id)
        .join(format!("{version_id}.json"));

    let cache_is_current =
        loader.is_none() || derived_version_cache_is_current(&path).await;
    let res = if path.exists() && !force.unwrap_or(false) && cache_is_current {
        let mut info: GameVersionInfo = io::read(&path)
            .err_into::<crate::Error>()
            .await
            .and_then(|ref it| Ok(serde_json::from_slice(it)?))?;
        let normalized_timestamps =
            normalize_version_timestamps(version, &mut info);
        let normalized =
            normalize_version_info(mod_loader, &version.id, &mut info, "cache");
        let restored_legacy_arguments =
            restore_legacy_minecraft_arguments(st, version, loader, &mut info)
                .await?;
        if normalized_timestamps || normalized || restored_legacy_arguments {
            write_version_info(&path, serde_json::to_vec(&info)?).await?;
        }
        info
    } else {
        tracing::info!(
            "Downloading version info for version {} from {}",
            &version.id,
            version.url
        );
        if let Some(reporter) = reporter {
            reporter
                .set_context(
                    InstallErrorContext::new(
                        "download Minecraft version metadata",
                    )
                    .minecraft_version(version.id.clone())
                    .urls(vec![version.url.clone()])
                    .target_path(path.display().to_string())
                    .build(),
                )
                .await?;
        }
        let mut info = match fetch_json(
            Method::GET,
            &version.url,
            Some(&version.sha1),
            None,
            None,
            &st.api_semaphore,
        )
        .await
        {
            Ok(info) => info,
            Err(primary_error) => {
                tracing::warn!(
                    minecraft_version = %version.id,
                    url = %version.url,
                    error = %primary_error,
                    "Version metadata failed; looking up the Mojang fallback"
                );
                let manifest: d::minecraft::VersionManifest = match fetch_json(
                    Method::GET,
                    d::minecraft::VERSION_MANIFEST_URL,
                    None,
                    None,
                    None,
                    &st.api_semaphore,
                )
                .await
                {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        tracing::warn!(
                            minecraft_version = %version.id,
                            error = %error,
                            "Mojang manifest fallback failed"
                        );
                        return Err(primary_error);
                    }
                };
                let Some(fallback_version) = manifest
                    .versions
                    .into_iter()
                    .find(|candidate| candidate.id == version.id)
                else {
                    return Err(primary_error);
                };
                if fallback_version.url == version.url {
                    return Err(primary_error);
                }
                if let Some(reporter) = reporter {
                    reporter
                        .set_context(
                            InstallErrorContext::new(
                                "download Minecraft version metadata",
                            )
                            .minecraft_version(version.id.clone())
                            .urls(vec![
                                version.url.clone(),
                                fallback_version.url.clone(),
                            ])
                            .target_path(path.display().to_string())
                            .build(),
                        )
                        .await?;
                }
                fetch_json(
                    Method::GET,
                    &fallback_version.url,
                    Some(&fallback_version.sha1),
                    None,
                    None,
                    &st.api_semaphore,
                )
                .await?
            }
        };

        if let Some(loader) = loader {
            if let Some(reporter) = reporter {
                reporter
                    .set_context(
                        InstallErrorContext::new(
                            "download loader version metadata",
                        )
                        .minecraft_version(version.id.clone())
                        .urls(vec![loader.url.clone()])
                        .target_path(path.display().to_string())
                        .build(),
                    )
                    .await?;
            }
            let partial: d::modded::PartialVersionInfo =
                if mod_loader == ModLoader::OptiFine {
                    crate::launcher::optifine::build_partial_version_info(
                        st,
                        &info,
                        &version.id,
                        &loader.id,
                    )
                    .await?
                } else {
                    crate::api::loader_metadata::resolve_loader_profile(
                        st,
                        &version.id,
                        loader,
                    )
                    .await?
                };
            info = d::modded::merge_partial_version(partial, info);
        }

        normalize_version_timestamps(version, &mut info);
        normalize_version_info(mod_loader, &version.id, &mut info, "network");

        info.id.clone_from(&version_id);

        write_version_info(&path, serde_json::to_vec(&info)?).await?;
        if loader.is_some() {
            write_derived_version_cache_marker(&path).await?;
        }
        info
    };

    crate::api::loader_metadata::ensure_installer_artifacts(st, &res).await?;

    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 5.0, None)?;
    }

    tracing::debug!("Loaded version info for Minecraft {version_id}");
    Ok(res)
}

pub async fn load_local_version_info(
    st: &State,
    version: &GameVersion,
    mod_loader: ModLoader,
    loader: Option<&LoaderVersion>,
) -> crate::Result<GameVersionInfo> {
    let version_id = loader
        .map_or(version.id.clone(), |it| format!("{}-{}", version.id, it.id));
    let path = st
        .directories
        .version_dir(&version_id)
        .join(format!("{version_id}.json"));

    if !path.is_file() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Offline mode can only launch fully downloaded instances; missing {}",
            path.display()
        ))
        .as_error());
    }

    let bytes = io::read(&path).err_into::<crate::Error>().await?;
    let mut info: GameVersionInfo = serde_json::from_slice(&bytes)?;
    let normalized_timestamps =
        normalize_version_timestamps(version, &mut info);
    let normalized =
        normalize_version_info(mod_loader, &version.id, &mut info, "cache");
    let restored_legacy_arguments =
        restore_legacy_minecraft_arguments(st, version, loader, &mut info)
            .await?;
    if normalized_timestamps || normalized || restored_legacy_arguments {
        write_version_info(&path, serde_json::to_vec(&info)?).await?;
    }
    Ok(info)
}

/// Only reusable, fully merged profiles may bypass loader catalog resolution.
pub(super) async fn try_load_launch_version_info(
    st: &State,
    version: &GameVersion,
    mod_loader: ModLoader,
    loader: Option<&LoaderVersion>,
) -> Option<GameVersionInfo> {
    let version_id = loader.map_or_else(
        || version.id.clone(),
        |it| format!("{}-{}", version.id, it.id),
    );
    let path = st
        .directories
        .version_dir(&version_id)
        .join(format!("{version_id}.json"));
    let info =
        read_launch_profile(&path, &version_id, loader.is_some()).await?;
    if mod_loader == ModLoader::Forge
        && let Some(loader) = loader
    {
        let coordinate = format!("{}-{}", version.id, loader.id);
        let legacy_coordinate = format!("{coordinate}-{}", version.id);
        if !info.libraries.iter().any(|library| {
            let mut parts = library.name.split(':');
            parts.next() == Some("net.minecraftforge")
                && matches!(parts.next(), Some("forge" | "fmlloader"))
                && parts.next().is_some_and(|value| {
                    value == coordinate || value == legacy_coordinate
                })
        }) {
            tracing::warn!(%version_id, "Local Forge profile does not match the requested loader");
            return None;
        }
    }
    match load_local_version_info(st, version, mod_loader, loader).await {
        Ok(info) => Some(info),
        Err(error) => {
            tracing::warn!(%version_id, %error, "Local launch profile could not be loaded; resolving metadata online");
            None
        }
    }
}

async fn read_launch_profile(
    path: &Path,
    version_id: &str,
    derived: bool,
) -> Option<GameVersionInfo> {
    if derived && !derived_version_cache_is_current(path).await {
        return None;
    }
    let bytes = tokio::fs::read(path).await.ok()?;
    let info: GameVersionInfo = serde_json::from_slice(&bytes).ok()?;
    if info.id != version_id
        || info.main_class.trim().is_empty()
        || info.asset_index.id.is_empty()
    {
        return None;
    }
    Some(info)
}

async fn write_version_info(path: &Path, data: Vec<u8>) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }
    io::write(path, data).await?;
    Ok(())
}

/// Bumped when derived loader metadata must be rebuilt from the base profile.

fn normalize_version_timestamps(
    version: &GameVersion,
    info: &mut GameVersionInfo,
) -> bool {
    let changed =
        info.release_time != version.release_time || info.time != version.time;
    info.release_time = version.release_time;
    info.time = version.time;
    changed
}

fn derived_version_cache_marker_path(path: &Path) -> PathBuf {
    path.with_extension("json.axolotl-format")
}

async fn derived_version_cache_is_current(path: &Path) -> bool {
    tokio::fs::read_to_string(derived_version_cache_marker_path(path))
        .await
        .is_ok_and(|format| format.trim() == DERIVED_VERSION_CACHE_FORMAT)
}

async fn write_derived_version_cache_marker(path: &Path) -> crate::Result<()> {
    io::write(
        derived_version_cache_marker_path(path),
        DERIVED_VERSION_CACHE_FORMAT,
    )
    .await?;
    Ok(())
}

async fn restore_legacy_minecraft_arguments(
    st: &State,
    version: &GameVersion,
    loader: Option<&LoaderVersion>,
    version_info: &mut GameVersionInfo,
) -> crate::Result<bool> {
    if loader.is_none() || version_info.minecraft_arguments.is_some() {
        return Ok(false);
    }

    let vanilla_path = st
        .directories
        .version_dir(&version.id)
        .join(format!("{}.json", version.id));
    if !vanilla_path.is_file() {
        return Ok(false);
    }

    let bytes = match io::read(&vanilla_path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                minecraft_version = version.id,
                path = %vanilla_path.display(),
                error = %error,
                "Failed to read vanilla version profile while restoring legacy game arguments"
            );
            return Ok(false);
        }
    };
    let vanilla: GameVersionInfo = match serde_json::from_slice(&bytes) {
        Ok(version_info) => version_info,
        Err(error) => {
            tracing::warn!(
                minecraft_version = version.id,
                path = %vanilla_path.display(),
                error = %error,
                "Failed to parse vanilla version profile while restoring legacy game arguments"
            );
            return Ok(false);
        }
    };
    let Some(arguments) = vanilla.minecraft_arguments else {
        return Ok(false);
    };

    version_info.minecraft_arguments = Some(arguments);
    tracing::info!(
        minecraft_version = version.id,
        "Restored legacy Minecraft game arguments from the vanilla version profile"
    );
    Ok(true)
}

fn normalize_version_info(
    loader: ModLoader,
    game_version: &str,
    version_info: &mut GameVersionInfo,
    version_info_source: &str,
) -> bool {
    let removed = d::modded::normalize_loader_libraries(
        loader.as_meta_str(),
        game_version,
        &mut version_info.libraries,
    );

    for removed_library in &removed {
        tracing::info!(
            loader = loader.as_meta_str(),
            game_version,
            removed_library,
            version_info_source,
            "Removed loader-incompatible library from Minecraft version profile"
        );
    }

    let mut changed = !removed.is_empty();
    // Cleanroom's Foundation bootstrap needs Java 21+. New installers declare
    // this themselves; older alphas ship no javaVersion at all and would
    // otherwise inherit vanilla 1.12.2's Java 8 and fail to launch.
    if loader == ModLoader::Cleanroom
        && version_info
            .java_version
            .as_ref()
            .is_none_or(|java| java.major_version < 21)
    {
        version_info.java_version = Some(d::minecraft::JavaVersion {
            component: "java-runtime-delta".to_string(),
            major_version: 21,
        });
        tracing::info!(
            loader = loader.as_meta_str(),
            game_version,
            version_info_source,
            "Raised Cleanroom version profile to Java 21"
        );
        changed = true;
    }
    if version_info
        .libraries
        .iter()
        .any(|library| is_liteloader_library(&library.name))
        && version_info
            .java_version
            .as_ref()
            .is_none_or(|java| java.major_version != 8)
    {
        version_info.java_version = Some(d::minecraft::JavaVersion {
            component: "jre-legacy".to_string(),
            major_version: 8,
        });
        tracing::info!(
            loader = loader.as_meta_str(),
            game_version,
            version_info_source,
            "Pinned LiteLoader version profile to Java 8"
        );
        changed = true;
    }

    changed
}

fn is_liteloader_library(name: &str) -> bool {
    let mut coordinates = name.split(':');
    coordinates.next() == Some("com.mumfrey")
        && coordinates.next() == Some("liteloader")
        && coordinates.next().is_some()
}

pub fn ensure_local_log_config(
    st: &State,
    version_info: &GameVersionInfo,
) -> crate::Result<()> {
    let log_download = version_info
        .logging
        .as_ref()
        .and_then(|logging| logging.get(&LoggingSide::Client));
    let Some(LoggingConfiguration::Log4j2Xml { file, .. }) = log_download
    else {
        return Ok(());
    };

    let path = st.directories.log_configs_dir().join(&file.id);
    if !path.is_file() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Offline mode can only launch fully downloaded instances; missing {}",
            path.display()
        ))
        .as_error());
    }

    Ok(())
}

/// Reports whether an existing client JAR may be reused. `Ok(None)` means the
/// version metadata carries no hash, so only its presence could be confirmed.
async fn client_jar_is_intact(
    path: &Path,
    expected_sha1: Option<&str>,
) -> crate::Result<Option<bool>> {
    let Some(expected_sha1) = expected_sha1 else {
        return Ok(None);
    };
    if expected_sha1.is_empty() {
        return Ok(None);
    }
    let (_, actual_sha1) = sha1_file_async(path).await?;
    Ok(Some(actual_sha1.eq_ignore_ascii_case(expected_sha1)))
}

/// Moves a client JAR that failed verification next to itself instead of
/// deleting it, so the broken file stays available for diagnosis. The moved
/// file cannot block the replacement download because the downloader commits
/// atomically.
async fn move_corrupt_client(path: &Path) -> Option<PathBuf> {
    static CORRUPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let directory = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".corrupt");
    if let Err(error) = io::create_dir_all(&directory).await {
        tracing::warn!(
            directory = %directory.display(),
            error = %error,
            "Could not create the corrupt-client directory"
        );
        return None;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let sequence = CORRUPT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let quarantine =
        directory.join(format!("{file_name}.{stamp:x}-{sequence}.corrupt"));
    match tokio::fs::rename(path, &quarantine).await {
        Ok(()) => Some(quarantine),
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "Could not move the corrupt client JAR aside"
            );
            None
        }
    }
}

#[tracing::instrument(skip_all)]

pub async fn download_client(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    game_version: &str,
    version_info: &GameVersionInfo,
    loading_bar: Option<&LoadingBarId>,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
) -> crate::Result<()> {
    let version = &version_info.id;
    tracing::debug!("Locating client for version {version}");
    let client_download = version_info
        .downloads
        .get(&d::minecraft::DownloadType::Client)
        .ok_or(
            crate::ErrorKind::LauncherError(format!(
                "No client downloads exist for version {version}"
            ))
            .as_error(),
        )?;
    let path = st
        .directories
        .version_dir(version)
        .join(format!("{version}.jar"));

    // A client JAR that exists is not proof that it is complete: an
    // interrupted transfer can leave a file of the right length with the wrong
    // contents, and the client is what the game starts from. Verify the
    // metadata hash before reusing it.
    if !force && path.exists() {
        let reusable = match client_jar_is_intact(
            &path,
            Some(&client_download.sha1),
        )
        .await
        {
            Ok(Some(true)) => true,
            Ok(Some(false)) => {
                let quarantine = move_corrupt_client(&path).await;
                tracing::warn!(
                    path = %path.display(),
                    quarantined_to = ?quarantine,
                    "Cached client JAR failed its SHA-1 check; downloading a replacement"
                );
                false
            }
            Ok(None) => {
                tracing::warn!(
                    path = %path.display(),
                    "Client JAR has no checksum metadata; reusing the cached file without strong validation"
                );
                true
            }
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "Could not verify the cached client JAR; downloading a replacement"
                );
                false
            }
        };
        if reusable {
            tracing::debug!("Reusing verified client for version {version}");
            if let Some(loading_bar) = loading_bar {
                emit_loading(loading_bar, 9.0, None)?;
            }
            return Ok(());
        }
    }

    if !path.exists() || force {
        let context = InstallErrorContext::new("download Minecraft client")
            .minecraft_version(version.to_string())
            .file_path(format!("{version}.jar"))
            .target_path(path.display().to_string())
            .build();
        let reused = download_or_reuse_local(
            st,
            local_source,
            &local_client_path(game_version),
            &path,
            Some(&client_download.sha1),
            Some(client_download.size as u64),
            progress.as_ref(),
            context.clone(),
            force,
            || {
                download_minecraft_file(
                    st,
                    &client_download.url,
                    Some(&client_download.sha1),
                    Some(client_download.size as u64),
                    &path,
                    ResourceClass::MinecraftLibrary,
                    ContentValidation::Jar,
                    force,
                    progress.clone(),
                    context,
                )
            },
        )
        .await?;
        if reused {
            tracing::trace!("Reused local client version {version}");
        } else {
            tracing::trace!("Fetched client version {version}");
        }
    }
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 9.0, None)?;
    }

    tracing::debug!("Client loaded for version {version}!");
    Ok(())
}

#[tracing::instrument(skip_all)]

pub async fn download_assets_index(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    version: &GameVersionInfo,
    loading_bar: Option<&LoadingBarId>,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
) -> crate::Result<AssetsIndex> {
    tracing::debug!("Loading assets index");
    let path = st
        .directories
        .assets_index_dir()
        .join(format!("{}.json", &version.asset_index.id));

    let cached = if path.exists() && !force {
        match io::read(&path)
            .err_into::<crate::Error>()
            .await
            .and_then(|ref bytes| Ok(serde_json::from_slice(bytes)?))
        {
            Ok(index) => Some(index),
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "Cached assets index is invalid; downloading a replacement"
                );
                io::remove_file(&path).await?;
                None
            }
        }
    } else {
        None
    };
    let res = if let Some(index) = cached {
        index
    } else {
        let context =
            InstallErrorContext::new("download Minecraft assets index")
                .minecraft_version(version.id.clone())
                .file_path(format!("{}.json", version.asset_index.id))
                .target_path(path.display().to_string())
                .build();
        let reused = download_or_reuse_local(
            st,
            local_source,
            &local_asset_index_path(&version.asset_index.id),
            &path,
            Some(&version.asset_index.sha1),
            Some(version.asset_index.size as u64),
            progress.as_ref(),
            context.clone(),
            force,
            || {
                download_minecraft_file(
                    st,
                    &version.asset_index.url,
                    Some(&version.asset_index.sha1),
                    Some(version.asset_index.size as u64),
                    &path,
                    ResourceClass::Metadata,
                    ContentValidation::Json,
                    force,
                    progress.clone(),
                    context,
                )
            },
        )
        .await?;
        if reused {
            tracing::info!("Reused local assets index");
        } else {
            tracing::info!("Fetched assets index");
        }
        serde_json::from_slice(&io::read(&path).await?)?
    };

    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 5.0, None)?;
    }
    tracing::debug!("Assets index successfully loaded!");
    Ok(res)
}

/// Owned per-asset work item for the per-file fallback path of
/// `download_assets`. Owned so the concurrent fallback futures do not borrow
/// from the assets index.
struct FallbackAsset {
    name: String,
    hash: String,
    size: u64,
    url: String,
    resource_path: PathBuf,
    legacy_resource_paths: Vec<PathBuf>,
    logical_items: u32,
}

fn fallback_batch_asset(item: H2BatchAsset) -> FallbackAsset {
    FallbackAsset {
        name: item.sha1.clone(),
        // Resolve the policy from the declared official URL again. A selected
        // batch mirror must not become the authority of a fallback request.
        url: format!(
            "https://resources.download.minecraft.net/{}/{}",
            &item.sha1[..2],
            item.sha1
        ),
        hash: item.sha1,
        size: item.size,
        resource_path: item.destination,
        legacy_resource_paths: item.legacy_destinations,
        logical_items: item.logical_items,
    }
}

fn build_fallback_asset(
    st: &State,
    name: &str,
    asset: &Asset,
) -> FallbackAsset {
    let hash = &asset.hash;
    let url = format!(
        "https://resources.download.minecraft.net/{sub_hash}/{hash}",
        sub_hash = &hash[..2]
    );
    FallbackAsset {
        name: name.to_string(),
        hash: hash.clone(),
        size: asset.size as u64,
        url,
        resource_path: st.directories.object_dir(hash),
        legacy_resource_paths: vec![
            st.directories.legacy_assets_dir().join(
                name.replace('/', &String::from(std::path::MAIN_SEPARATOR)),
            ),
        ],
        logical_items: 1,
    }
}

/// Coalesce every fallback that writes the same physical object. This is
/// especially important for legacy indexes, where several logical names can
/// share one hash: concurrent local-reuse tasks would otherwise race on the
/// same `.part` and destination paths.
fn coalesce_fallback_assets(items: Vec<FallbackAsset>) -> Vec<FallbackAsset> {
    let mut positions: HashMap<(PathBuf, String, u64), usize> = HashMap::new();
    let mut coalesced: Vec<FallbackAsset> = Vec::new();
    for mut item in items {
        let key = (item.resource_path.clone(), item.hash.clone(), item.size);
        if let Some(&position) = positions.get(&key) {
            let existing = &mut coalesced[position];
            existing.logical_items =
                existing.logical_items.saturating_add(item.logical_items);
            for legacy in item.legacy_resource_paths.drain(..) {
                if !existing.legacy_resource_paths.contains(&legacy) {
                    existing.legacy_resource_paths.push(legacy);
                }
            }
        } else {
            positions.insert(key, coalesced.len());
            coalesced.push(item);
        }
    }
    coalesced
}

/// Coalesce index aliases that refer to one physical object. The key includes
/// the destination and full integrity contract so unrelated artifacts can
/// never share a writer. Legacy paths remain separate outputs of that single
/// committed object.
fn coalesce_batch_assets(items: Vec<H2BatchAsset>) -> Vec<H2BatchAsset> {
    let mut positions: HashMap<(PathBuf, String, u64), usize> = HashMap::new();
    let mut coalesced: Vec<H2BatchAsset> = Vec::new();
    for mut item in items {
        let key = (item.destination.clone(), item.sha1.clone(), item.size);
        if let Some(&position) = positions.get(&key) {
            let existing = &mut coalesced[position];
            existing.logical_items =
                existing.logical_items.saturating_add(item.logical_items);
            for legacy in item.legacy_destinations.drain(..) {
                if !existing.legacy_destinations.contains(&legacy) {
                    existing.legacy_destinations.push(legacy);
                }
            }
        } else {
            positions.insert(key, coalesced.len());
            coalesced.push(item);
        }
    }
    coalesced
}

#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub async fn download_assets(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    with_legacy: bool,
    index: &AssetsIndex,
    loading_bar: Option<&LoadingBarId>,
    loading_amount: f64,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
) -> crate::Result<()> {
    tracing::debug!("Loading assets");
    let asset_download_semaphore =
        FetchSemaphore(tokio::sync::Semaphore::new(ASSET_BATCH_CONCURRENCY));
    let num_futs = index.objects.len();
    let per_file_fraction = if num_futs > 0 {
        loading_amount / num_futs as f64
    } else {
        0.0
    };

    // Partition assets: batch downloads (object missing), legacy-only copies
    // (object present, legacy missing), and local reuse fallbacks.
    let mut batch_items = Vec::new();
    let mut legacy_copies = Vec::new();
    let mut fallback_assets = Vec::new();
    let mut skipped_count = 0_u64;
    for (name, asset) in &index.objects {
        let hash = &asset.hash;
        let resource_path = st.directories.object_dir(hash);
        let legacy_resource_path = st
            .directories
            .legacy_assets_dir()
            .join(name.replace('/', &String::from(std::path::MAIN_SEPARATOR)));
        let should_fetch_object =
            force || !asset_file_is_usable(&resource_path, asset.size as u64);
        let should_fetch_legacy = with_legacy
            && (force
                || !asset_file_is_usable(
                    &legacy_resource_path,
                    asset.size as u64,
                ));

        if should_fetch_object {
            let local_candidate = !force
                && super::local_artifact::candidate_is_usable(
                    local_source,
                    &local_asset_object_path(hash),
                    Some(asset.size as u64),
                )
                .await?;
            if local_candidate {
                fallback_assets.push(build_fallback_asset(st, name, asset));
            } else {
                let url = format!(
                    "https://resources.download.minecraft.net/{sub_hash}/{hash}",
                    sub_hash = &hash[..2]
                );
                batch_items.push(H2BatchAsset {
                    url,
                    destination: resource_path,
                    legacy_destinations: should_fetch_legacy
                        .then_some(legacy_resource_path)
                        .into_iter()
                        .collect(),
                    sha1: hash.clone(),
                    size: asset.size as u64,
                    logical_items: 1,
                });
            }
        } else if should_fetch_legacy {
            legacy_copies.push((name, asset));
        } else {
            skipped_count += 1;
        }
    }

    let mut batch_items = coalesce_batch_assets(batch_items);

    // Batch-download the object files over a single shared HTTP/2 connection:
    // hundreds of concurrent multiplexed streams, one connection per
    // authority — never one connection per file.
    if !batch_items.is_empty() {
        let source_mode = st.minecraft_file_source();
        let first_url = batch_items[0].url.clone();
        let mut routes = resolve_download_routes_for(
            &first_url,
            ResourceClass::MinecraftAsset,
            source_mode,
        );
        let apply_native_policy = true;
        if apply_native_policy {
            let probe_request =
                DownloadRequest::new(&first_url, ResourceClass::MinecraftAsset)
                    .with_integrity(
                        Integrity::sha1(&batch_items[0].sha1)
                            .with_size(batch_items[0].size),
                    );
            prepare_native_download_routes(
                &probe_request,
                &mut routes,
                &asset_download_semaphore,
            )
            .await;
        }
        let route = routes.into_iter().next();
        if let Some(route) = route {
            // Resolve each item's URL onto the chosen route (official or
            // mirror) so the batch reuses one connection to that authority.
            // Items whose resolved URL targets a different authority cannot
            // share the batch connection and go through the per-file path.
            let route_authority = url_authority(&route.url);
            for item in &mut batch_items {
                let item_routes = resolve_download_routes_for(
                    &item.url,
                    ResourceClass::MinecraftAsset,
                    source_mode,
                );
                item.url = item_routes
                    .iter()
                    .find(|candidate| {
                        apply_native_policy
                            && candidate.source == route.source
                            && candidate.proxy == route.proxy
                    })
                    .or_else(|| item_routes.first())
                    .map(|route| route.url.clone())
                    .unwrap_or_else(|| item.url.clone());
            }
            let (batch_items, rerouted_items): (Vec<_>, Vec<_>) = batch_items
                .into_iter()
                .partition(|item| url_authority(&item.url) == route_authority);
            fallback_assets
                .extend(rerouted_items.into_iter().map(fallback_batch_asset));
            let callback = {
                let progress = progress.clone();
                let loading_bar = loading_bar.cloned();
                move |item: H2BatchAsset| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    let progress = progress.clone();
                    let loading_bar = loading_bar.clone();
                    Box::pin(async move {
                        for _ in 0..item.logical_items {
                            if let Some(progress) = &progress
                                && let Err(error) = progress.add_bytes(item.size).await {
                                    tracing::warn!(
                                        error = %error,
                                        "Failed to record batch asset bytes"
                                    );
                                }
                            if let Some(loading_bar) = &loading_bar {
                                let _ = emit_loading(loading_bar, per_file_fraction, None);
                            }
                        }
                    })
                }
            };
            let unresolved = download_asset_batch_via_h2(
                &route,
                batch_items,
                ASSET_BATCH_CONCURRENCY,
                apply_native_policy,
                callback,
            )
            .await?;
            fallback_assets
                .extend(unresolved.into_iter().map(fallback_batch_asset));
        } else {
            fallback_assets
                .extend(batch_items.into_iter().map(fallback_batch_asset));
        }
    }

    // Legacy copies for assets whose object is already on disk.
    futures::stream::iter(legacy_copies)
        .map(Ok::<_, crate::Error>)
        .try_for_each_concurrent(ASSET_BATCH_CONCURRENCY, |(name, asset)| {
            let progress = progress.clone();
            async move {
                let resource_path = st.directories.object_dir(&asset.hash);
                let legacy_resource_path =
                    st.directories.legacy_assets_dir().join(name.replace(
                        '/',
                        &String::from(std::path::MAIN_SEPARATOR),
                    ));
                crate::util::fetch::copy(
                    &resource_path,
                    &legacy_resource_path,
                    &st.io_semaphore,
                )
                .await?;
                if let Some(progress) = &progress {
                    progress.add_bytes(asset.size as u64).await?;
                }
                if let Some(loading_bar) = loading_bar {
                    emit_loading(loading_bar, per_file_fraction, None)?;
                }
                Ok::<_, crate::Error>(())
            }
        })
        .await?;

    // The batch returns only unresolved physical objects. Completed objects
    // have already reported progress and never enter this path, even on force.
    let fallback_assets = coalesce_fallback_assets(fallback_assets);
    if !fallback_assets.is_empty() {
        futures::stream::iter(fallback_assets)
            .map(Ok::<FallbackAsset, crate::Error>)
            .try_for_each_concurrent(st.download_concurrency().clamp(1, ASSET_BATCH_CONCURRENCY), |item| {
                let progress = progress.clone();
                let semaphore = &asset_download_semaphore;
                async move {
                    let resource_path = &item.resource_path;
                    let hash = &item.hash;
                    let name = &item.name;
                    let should_fetch_object = force
                        || !verified_asset_is_usable(resource_path, hash, item.size).await?;
                    let legacy_resource_paths = item
                        .legacy_resource_paths
                        .iter()
                        .filter(|path| {
                            with_legacy
                                && (force
                                    || !asset_file_is_usable(path, item.size))
                        })
                        .collect::<Vec<_>>();
                    let should_fetch_legacy = !legacy_resource_paths.is_empty();
                    let fetch_progress = should_fetch_object.then(|| progress.clone()).flatten();
                    if should_fetch_object {
                        let context = InstallErrorContext::new(
                            "download Minecraft asset",
                        )
                        .file_path(name.clone())
                        .target_path(resource_path.display().to_string())
                        .build();
                        let reused = download_or_reuse_local(
                            st,
                            local_source,
                            &local_asset_object_path(hash),
                            resource_path,
                            Some(hash),
                            Some(item.size),
                            fetch_progress.as_ref(),
                            context.clone(),
                            force,
                            || {
                                download_minecraft_file_with_semaphore(
                                    st,
                                    &item.url,
                                    Some(hash),
                                    Some(item.size),
                                    resource_path,
                                    ResourceClass::MinecraftAsset,
                                    ContentValidation::None,
                                    force,
                                    fetch_progress.clone(),
                                    context,
                                    semaphore,
                                )
                            },
                        )
                        .await?;
                        // Both local reuse and the downloader verify SHA-1.
                        // Assets additionally require their declared size.
                        if tokio::fs::metadata(resource_path).await?.len() != item.size {
                            return Err(crate::ErrorKind::OtherError("Downloaded Minecraft asset failed size validation".into()).into());
                        }
                        if reused {
                            tracing::trace!("Reused asset with hash {hash}");
                        } else {
                            tracing::trace!("Fetched asset with hash {hash}");
                        }
                    }
                    if should_fetch_legacy {
                        for legacy_resource_path in legacy_resource_paths {
                            crate::util::fetch::copy(
                                resource_path,
                                legacy_resource_path,
                                &st.io_semaphore,
                            )
                            .await?;
                            tracing::trace!(
                                "Copied legacy asset with hash {hash}"
                            );
                        }
                    }

                    if let Some(progress) = &progress {
                        for _ in fallback_progress_units(item.logical_items, should_fetch_object) {
                            progress.add_bytes(item.size).await?;
                        }
                    }

                    if let Some(loading_bar) = loading_bar {
                        for _ in 0..item.logical_items {
                            emit_loading(loading_bar, per_file_fraction, None)?;
                        }
                    }
                    tracing::trace!("Loaded asset with hash {hash}");
                    Ok::<_, crate::Error>(())
                }
            })
            .await?;
    }

    // Account for assets that were already on disk so the loading bar still
    // reaches its total.
    for _ in 0..skipped_count {
        if let Some(loading_bar) = loading_bar {
            emit_loading(loading_bar, per_file_fraction, None)?;
        }
    }

    tracing::debug!("Done loading assets!");
    Ok(())
}

fn fallback_progress_units(
    logical_items: u32,
    fetched: bool,
) -> std::ops::Range<u32> {
    u32::from(fetched)..logical_items
}

#[tracing::instrument(skip_all, fields(version))]
#[allow(clippy::too_many_arguments)]
pub async fn download_libraries(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    libraries: &[Library],
    version: &str,
    loading_bar: Option<&LoadingBarId>,
    loading_amount: f64,
    java_arch: &str,
    force: bool,
    minecraft_updated: bool,
    progress: Option<MinecraftDownloadProgress>,
) -> crate::Result<()> {
    tracing::debug!("Loading libraries");

    tokio::try_join! {
        io::create_dir_all(st.directories.libraries_dir()),
        io::create_dir_all(st.directories.version_natives_dir(version))
    }?;

    // Plan individual file download tasks instead of treating each
    // Library as mutually exclusive "normal or native". A merged library
    // (e.g. LWJGL after manifest merging) may carry both a Java artifact
    // and a native classifier for this platform; both become independent
    // concurrent tasks so neither is starved by the other.
    enum LibraryDownloadTask<'a> {
        JavaArtifact(&'a Library),
        NativeArtifact(&'a Library),
    }

    let mut tasks: Vec<LibraryDownloadTask<'_>> = Vec::new();

    // Java artifact tasks: every library with a downloadable Java JAR.
    // Deduplicate by target path so repeated manifest entries download once.
    let mut seen_java_paths = std::collections::HashSet::new();
    for library in libraries {
        if !java_artifact_applies(library, java_arch, minecraft_updated) {
            continue;
        }
        let target = d::get_path_from_artifact(&library.name)
            .unwrap_or_else(|_| library.name.clone());
        if seen_java_paths.insert(target) {
            tasks.push(LibraryDownloadTask::JavaArtifact(library));
        }
    }

    // Native artifact tasks: reuse the existing native planner, which
    // handles rules, classifier existence and SHA-1 deduplication.
    for library in
        native_libraries_to_download(libraries, java_arch, minecraft_updated)?
    {
        tasks.push(LibraryDownloadTask::NativeArtifact(library));
    }

    let num_files = tasks.len();
    loading_try_for_each_concurrent(
        stream::iter(tasks).map(Ok::<LibraryDownloadTask<'_>, crate::Error>),
        crate::util::download::task_concurrency_limit(st),
        loading_bar,
        loading_amount,
        num_files,
        None,
        |task| {
            let progress = progress.clone();
            async move {
                let library = match &task {
                    LibraryDownloadTask::JavaArtifact(lib) => *lib,
                    LibraryDownloadTask::NativeArtifact(lib) => *lib,
                };

                if let Some(rules) = &library.rules
                    && !parse_rules(
                        rules,
                        java_arch,
                        &QuickPlayType::None,
                        minecraft_updated,
                    )
                {
                    tracing::trace!("Skipped library {}", &library.name);
                    return Ok(());
                }

                if !library.downloadable {
                    tracing::trace!(
                        "Skipped non-downloadable library {}",
                        &library.name
                    );
                    return Ok(());

                }
                match task {
                    LibraryDownloadTask::JavaArtifact(library) => {
                    let artifact_path = d::get_path_from_artifact(&library.name)?;
                    let path = st.directories.libraries_dir().join(&artifact_path);

                    if path.exists() && !force {
                        // already present; skip Java artifact download but continue to natives
                    } else if let Some(d::minecraft::LibraryDownloads {
                        artifact: Some(ref artifact),
                        ..
                    }) = library.downloads
                        && !artifact.url.is_empty()
                    {
                        let local_relative = local_library_path(&library.name)?;
                        let context = InstallErrorContext::new(
                            "download Minecraft library",
                        )
                        .minecraft_version(version.to_string())
                        .file_path(library.name.clone())
                        .target_path(path.display().to_string())
                        .build();
                        let reused = download_or_reuse_local(
                            st,
                            local_source,
                            &local_relative,
                            &path,
                            Some(&artifact.sha1),
                            Some(artifact.size as u64),
                            progress.as_ref(),
                            context.clone(),
                            force,
                            || {
                                download_minecraft_file(
                                    st,
                                    &artifact.url,
                                    Some(&artifact.sha1),
                                    Some(artifact.size as u64),
                                    &path,
                                    ResourceClass::MinecraftLibrary,
                                    ContentValidation::None,
                                    force,
                                    progress.clone(),
                                    context,
                                )
                            },
                        )
                        .await?;
                        if reused {
                            tracing::trace!(
                                "Reused library {} to path {:?}",
                                &library.name,
                                &path
                            );
                        } else {
                            tracing::trace!(
                                "Fetched library {} to path {:?}",
                                &library.name,
                                &path
                            );
                        }
                    } else {
                        let Some(urls) = legacy_library_download_urls(
                            library.url.as_deref(),
                            &artifact_path,
                        ) else {
                            return Err(crate::ErrorKind::LauncherError(format!(
                                "No safe Maven repository is known for required library {}",
                                library.name
                            ))
                            .into());
                        };

                        let local_relative = local_library_path(&library.name)?;
                        let context = InstallErrorContext::new(
                            "download loader library",
                        )
                        .minecraft_version(version.to_string())
                        .file_path(library.name.clone())
                        .target_path(path.display().to_string())
                        .build();
                        let reused = download_or_reuse_local(
                            st,
                            local_source,
                            &local_relative,
                            &path,
                            legacy_library_sha1(library),
                            None,
                            progress.as_ref(),
                            context.clone(),
                            force,
                            || {
                                download_minecraft_file_with_candidates(
                                    st,
                                    &urls,
                                    legacy_library_sha1(library),
                                    None,
                                    &path,
                                    ResourceClass::Loader,
                                    legacy_library_content_validation(&artifact_path),
                                    force,
                                    progress.clone(),
                                    context,
                                    &st.download_semaphore,
                                )
                            },
                        )
                        .await?;
                        if reused {
                            tracing::debug!(
                                "Reused legacy library {} to path {:?}",
                                &library.name,
                                &path
                            );
                        } else {
                            tracing::debug!(
                                "Fetched legacy library {} to path {:?}",
                                &library.name,
                                &path
                            );
                        }
                    }
                    }
                    LibraryDownloadTask::NativeArtifact(library) => {
                    if let Some(classifier) =
                        library_native_classifier(library, java_arch)
                    {
                        let native = library
                            .downloads
                            .as_ref()
                            .and_then(|downloads| downloads.classifiers.as_ref())
                            .and_then(|classifiers| classifiers.get(&classifier));
                        let _native_archive_path = if let Some(native) = native {
                            let path = st
                                .directories
                                .caches_dir()
                                .join("minecraft-natives")
                                .join(format!("{}.jar", native.sha1));
                            let context = InstallErrorContext::new(
                                "download Minecraft native library",
                            )
                            .minecraft_version(version.to_string())
                            .file_path(library.name.clone())
                            .target_path(path.display().to_string())
                            .build();
                            let local_relative =
                                local_native_library_path(library, native, &classifier)?;
                            let reused = download_or_reuse_local(
                                st,
                                local_source,
                                &local_relative,
                                &path,
                                Some(&native.sha1),
                                Some(native.size as u64),
                                progress.as_ref(),
                                context.clone(),
                                force,
                                || {
                                    download_minecraft_file(
                                        st,
                                        &native.url,
                                        Some(&native.sha1),
                                        Some(native.size as u64),
                                        &path,
                                        ResourceClass::MinecraftLibrary,
                                        ContentValidation::Jar,
                                        force,
                                        progress.clone(),
                                        context,
                                    )
                                },
                            )
                            .await?;
                            if reused {
                                tracing::trace!("Reused native {}", &library.name);
                            }
                            path
                        } else if library_classifier(&library.name)
                            .is_some_and(|value| value.starts_with("natives-"))
                            && let Some(artifact) = library
                                .downloads
                                .as_ref()
                                .and_then(|downloads| downloads.artifact.as_ref())
                                .filter(|artifact| !artifact.url.trim().is_empty())
                        {
                            let artifact_path =
                                native_library_artifact_path(library, &classifier)?;
                            let path = st
                                .directories
                                .libraries_dir()
                                .join(&artifact_path);
                            let local_relative =
                                Path::new("libraries").join(&artifact_path);
                            let context = InstallErrorContext::new(
                                "download Minecraft native library",
                            )
                            .minecraft_version(version.to_string())
                            .file_path(format!("{}:{classifier}", library.name))
                            .target_path(path.display().to_string())
                            .build();
                            let reused = download_or_reuse_local(
                                st,
                                local_source,
                                &local_relative,
                                &path,
                                Some(&artifact.sha1),
                                Some(artifact.size as u64),
                                progress.as_ref(),
                                context.clone(),
                                force,
                                || {
                                    download_minecraft_file(
                                        st,
                                        &artifact.url,
                                        Some(&artifact.sha1),
                                        Some(artifact.size as u64),
                                        &path,
                                        ResourceClass::MinecraftLibrary,
                                        ContentValidation::Jar,
                                        force,
                                        progress.clone(),
                                        context,
                                    )
                                },
                            )
                            .await?;
                            if reused {
                                tracing::trace!("Reused native {}", &library.name);
                            } else {
                                tracing::trace!("Fetched native {}", &library.name);
                            }
                            path
                        } else {
                            let artifact_path = native_library_artifact_path(
                                library,
                                &classifier,
                            )?;
                            let path =
                                st.directories.libraries_dir().join(&artifact_path);
                            let Some(urls) = legacy_library_download_urls(
                                library.url.as_deref(),
                                &artifact_path,
                            ) else {
                                return Err(crate::ErrorKind::LauncherError(format!(
                                    "No safe Maven repository is known for required native library {}",
                                    library.name
                                ))
                                .into());
                            };
                            let local_relative =
                                Path::new("libraries").join(&artifact_path);
                            let context = InstallErrorContext::new(
                                "download loader native library",
                            )
                            .minecraft_version(version.to_string())
                            .file_path(format!("{}:{classifier}", library.name))
                            .urls(urls.clone())
                            .target_path(path.display().to_string())
                            .build();
                            let reused = download_or_reuse_local(
                                st,
                                local_source,
                                &local_relative,
                                &path,
                                None,
                                None,
                                progress.as_ref(),
                                context.clone(),
                                force,
                                || {
                                    download_minecraft_file_with_candidates(
                                        st,
                                        &urls,
                                        None,
                                        None,
                                        &path,
                                        ResourceClass::Loader,
                                        ContentValidation::Jar,
                                        force,
                                        progress.clone(),
                                        context,
                                        &st.download_semaphore,
                                    )
                                },
                            )
                            .await?;
                            if reused {
                                tracing::debug!(
                                    "Reused legacy native {} to path {:?}",
                                    &library.name,
                                    &path
                                );
                            } else {
                                tracing::debug!(
                                    "Fetched legacy native {} to path {:?}",
                                    &library.name,
                                    &path
                                );
                            }
                            path
                        };

                        tracing::debug!("Downloaded native {}", &library.name);
                    } else {
                        tracing::trace!(
                            "Skipped native library without a classifier for this platform: {}",
                            &library.name
                        );
                    }
                }
                }

                tracing::debug!("Loaded library {}", library.name);
                Ok(())
            }
        },
    )
    .await?;

    crate::launcher::natives::prepare_native_libraries(
        &st.directories.natives_dir(),
        &st.directories.libraries_dir(),
        &st.directories.caches_dir(),
        libraries,
        version,
        java_arch,
        minecraft_updated,
    )
    .await?;

    tracing::debug!("Done loading libraries!");
    Ok(())
}

fn native_libraries_to_download<'a>(
    libraries: &'a [Library],
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<Vec<&'a Library>> {
    let mut identities = HashSet::new();
    let mut result = Vec::new();
    for library in libraries {
        if !is_native_library(library) || !library.downloadable {
            continue;
        }
        if let Some(rules) = &library.rules
            && !parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }
        let Some(classifier) = library_native_classifier(library, java_arch)
        else {
            continue;
        };
        let identity = library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.as_ref())
            .and_then(|classifiers| classifiers.get(&classifier))
            .filter(|download| !download.sha1.is_empty())
            .map_or_else(
                || native_library_artifact_path(library, &classifier),
                |download| Ok(download.sha1.clone()),
            )?;
        if identities.insert(identity.clone()) {
            result.push(library);
        } else {
            tracing::debug!(
                library = %library.name,
                classifier,
                identity,
                "Skipped duplicate native archive download"
            );
        }
    }
    Ok(result)
}

#[tracing::instrument(skip_all)]
pub async fn download_log_config(
    st: &State,
    local_source: Option<&LocalRuntimeSource>,
    version_info: &GameVersionInfo,
    loading_bar: Option<&LoadingBarId>,
    force: bool,
    progress: Option<MinecraftDownloadProgress>,
) -> crate::Result<bool> {
    let log_download = version_info
        .logging
        .as_ref()
        .and_then(|x| x.get(&LoggingSide::Client));
    let Some(LoggingConfiguration::Log4j2Xml {
        file: log_download, ..
    }) = log_download
    else {
        if let Some(loading_bar) = loading_bar {
            emit_loading(loading_bar, 1.0, None)?;
        }
        return Ok(false);
    };

    let path = st.directories.log_configs_dir().join(&log_download.id);

    if !path.exists() || force {
        let context = InstallErrorContext::new("download Minecraft log config")
            .minecraft_version(version_info.id.clone())
            .file_path(log_download.id.clone())
            .target_path(path.display().to_string())
            .build();
        let reused = download_or_reuse_local(
            st,
            local_source,
            &local_log_config_path(&log_download.id),
            &path,
            Some(&log_download.sha1),
            Some(log_download.size as u64),
            progress.as_ref(),
            context.clone(),
            force,
            || {
                download_minecraft_file(
                    st,
                    &log_download.url,
                    Some(&log_download.sha1),
                    Some(log_download.size as u64),
                    &path,
                    ResourceClass::MinecraftLibrary,
                    ContentValidation::None,
                    force,
                    progress.clone(),
                    context,
                )
            },
        )
        .await?;
        if reused {
            tracing::trace!("Reused log config {}", log_download.id);
        } else {
            tracing::trace!("Fetched log config {}", log_download.id);
        }
    }
    if let Some(loading_bar) = loading_bar {
        emit_loading(loading_bar, 1.0, None)?;
    }

    tracing::debug!("Log config {} loaded", log_download.id);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::natives::prepare_native_libraries as prepare_test_natives;

    async fn client_sha1(bytes: &[u8]) -> String {
        sha1_async(bytes.to_vec().into()).await.expect("hash")
    }

    #[tokio::test]
    async fn an_existing_client_jar_with_the_wrong_contents_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("1.20.1.jar");
        let good = b"a complete client jar".as_slice();
        let truncated = b"a complete client".as_slice();
        tokio::fs::write(&path, truncated).await.unwrap();

        assert_eq!(
            client_jar_is_intact(&path, Some(&client_sha1(good).await))
                .await
                .unwrap(),
            Some(false),
            "a wrong-content cache entry must not be reused"
        );
        assert_eq!(
            client_jar_is_intact(&path, Some(&client_sha1(truncated).await))
                .await
                .unwrap(),
            Some(true),
            "an intact cache entry is reusable"
        );
    }

    #[tokio::test]
    async fn a_client_jar_without_metadata_hash_is_not_strongly_verified() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("1.20.1.jar");
        tokio::fs::write(&path, b"client").await.unwrap();

        assert_eq!(client_jar_is_intact(&path, None).await.unwrap(), None);
        assert_eq!(
            client_jar_is_intact(&path, Some("")).await.unwrap(),
            None,
            "an empty hash must not be reported as a verified file"
        );
    }

    #[tokio::test]
    async fn a_corrupt_client_jar_is_moved_aside_instead_of_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("1.20.1.jar");
        tokio::fs::write(&path, b"broken").await.unwrap();

        let quarantine =
            move_corrupt_client(&path).await.expect("quarantine file");

        assert!(!path.exists(), "the broken file leaves its original path");
        assert_eq!(tokio::fs::read(&quarantine).await.unwrap(), b"broken");
        assert!(
            quarantine.starts_with(directory.path().join(".corrupt")),
            "the file stays beside its version directory"
        );
    }

    fn urls(values: &[&str]) -> Option<Vec<String>> {
        Some(values.iter().map(|value| (*value).to_string()).collect())
    }

    fn version_info_with_java(
        java: Option<d::minecraft::JavaVersion>,
    ) -> GameVersionInfo {
        let mut info: GameVersionInfo = serde_json::from_value(serde_json::json!({
            "assetIndex": {"id": "1.12", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
            "assets": "1.12",
            "downloads": {},
            "id": "1.12.2",
            "libraries": [],
            "mainClass": "net.minecraft.client.main.Main",
            "minimumLauncherVersion": 21,
            "releaseTime": "2026-01-01T00:00:00+00:00",
            "time": "2026-01-01T00:00:00+00:00",
            "type": "release"
        }))
        .unwrap();
        info.java_version = java;
        info
    }

    #[test]
    fn cleanroom_without_java_version_is_raised_to_java_21() {
        let mut info = version_info_with_java(None);
        assert!(normalize_version_info(
            ModLoader::Cleanroom,
            "1.12.2",
            &mut info,
            "network",
        ));
        let java = info.java_version.expect("java version pinned");
        assert_eq!(java.major_version, 21);
        assert_eq!(java.component, "java-runtime-delta");
    }

    #[tokio::test]
    async fn cached_loader_timestamps_are_corrected_online_and_offline() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let version: GameVersion = serde_json::from_value(serde_json::json!({
            "id": "1.12.2", "type": "release", "url": "invalid://no-network",
            "releaseTime": "2017-09-18T08:39:46Z", "time": "2021-12-15T15:04:05Z",
            "sha1": "", "complianceLevel": 0
        })).unwrap();
        let loader: LoaderVersion = serde_json::from_value(serde_json::json!({
            "id": "test-loader", "stable": true, "url": "invalid://no-network"
        }))
        .unwrap();
        let path = state
            .directories
            .version_dir("1.12.2-test-loader")
            .join("1.12.2-test-loader.json");
        for offline in [false, true] {
            let mut info = version_info_with_java(None);
            info.id = "1.12.2-test-loader".to_string();
            write_version_info(&path, serde_json::to_vec(&info).unwrap())
                .await
                .unwrap();
            if offline {
                io::write(derived_version_cache_marker_path(&path), "3")
                    .await
                    .unwrap();
            } else {
                write_derived_version_cache_marker(&path).await.unwrap();
            }
            let mut loaded = if offline {
                load_local_version_info(
                    &state,
                    &version,
                    ModLoader::Forge,
                    Some(&loader),
                )
                .await
            } else {
                download_version_info(
                    &state,
                    &version,
                    ModLoader::Forge,
                    Some(&loader),
                    None,
                    None,
                    None,
                )
                .await
            }
            .unwrap();
            assert_eq!(loaded.release_time, version.release_time);
            assert_eq!(loaded.time, version.time);
            let saved: GameVersionInfo =
                serde_json::from_slice(&io::read(&path).await.unwrap())
                    .unwrap();
            assert_eq!(saved.release_time, version.release_time);
            assert_eq!(saved.time, version.time);
            assert!(!normalize_version_timestamps(&version, &mut loaded));
            assert_eq!(derived_version_cache_is_current(&path).await, !offline);
        }
    }

    #[tokio::test]
    async fn installed_forge_profile_is_validated_without_loader_catalogs() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".into(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let version: GameVersion = serde_json::from_value(serde_json::json!({
            "id":"1.20.1", "type":"release", "url":"invalid://no-network",
            "releaseTime":"2023-06-12T13:25:51Z", "time":"2023-06-12T13:25:51Z",
            "sha1":"", "complianceLevel":1
        }))
        .unwrap();
        let loader: LoaderVersion = serde_json::from_value(serde_json::json!({
            "id":"47.4.28", "stable":false, "url":""
        }))
        .unwrap();
        let path = state
            .directories
            .version_dir("1.20.1-47.4.28")
            .join("1.20.1-47.4.28.json");
        let mut info = version_info_with_java(None);
        info.id = "1.20.1-47.4.28".into();
        info.main_class = "cpw.mods.bootstraplauncher.BootstrapLauncher".into();
        info.libraries = vec![
            serde_json::from_value(serde_json::json!({
                "name":"net.minecraftforge:forge:1.20.1-47.4.28:universal"
            }))
            .unwrap(),
        ];
        assert!(
            try_load_launch_version_info(
                &state,
                &version,
                ModLoader::Forge,
                Some(&loader)
            )
            .await
            .is_none()
        );
        write_version_info(&path, serde_json::to_vec(&info).unwrap())
            .await
            .unwrap();
        assert!(
            try_load_launch_version_info(
                &state,
                &version,
                ModLoader::Forge,
                Some(&loader)
            )
            .await
            .is_none()
        );
        write_derived_version_cache_marker(&path).await.unwrap();
        let loaded = try_load_launch_version_info(
            &state,
            &version,
            ModLoader::Forge,
            Some(&loader),
        )
        .await
        .unwrap();
        assert_eq!(loaded.id, "1.20.1-47.4.28");
        assert_eq!(loaded.main_class, info.main_class);
        assert_eq!(loaded.release_time, version.release_time);
        let (selected, launch_info) =
            crate::launcher::load_managed_launch_profile(
                &state,
                &version,
                ModLoader::Forge,
                Some("47.4.28"),
                false,
            )
            .await
            .unwrap();
        assert_eq!(selected.unwrap().id, "47.4.28");
        assert_eq!(launch_info.id, "1.20.1-47.4.28");
        for bad in ["{", "{}"] {
            write_version_info(&path, bad.as_bytes().to_vec())
                .await
                .unwrap();
            assert!(
                try_load_launch_version_info(
                    &state,
                    &version,
                    ModLoader::Forge,
                    Some(&loader)
                )
                .await
                .is_none()
            );
        }
        for variant in 0..4 {
            let mut invalid: GameVersionInfo =
                serde_json::from_value(serde_json::to_value(&info).unwrap())
                    .unwrap();
            match variant {
                0 => invalid.id = "1.20.1-47.4.22".into(),
                1 => invalid.main_class.clear(),
                2 => invalid.libraries.clear(),
                _ => {
                    invalid.libraries[0].name =
                        "net.minecraftforge:forge:1.20.1-47.4.22:universal"
                            .into()
                }
            }
            write_version_info(&path, serde_json::to_vec(&invalid).unwrap())
                .await
                .unwrap();
            assert!(
                try_load_launch_version_info(
                    &state,
                    &version,
                    ModLoader::Forge,
                    Some(&loader)
                )
                .await
                .is_none(),
                "variant {variant}"
            );
        }
    }

    #[test]
    fn cleanroom_installer_declared_java_is_preserved() {
        let mut info =
            version_info_with_java(Some(d::minecraft::JavaVersion {
                component: "java-runtime-epsilon".to_string(),
                major_version: 25,
            }));
        assert!(!normalize_version_info(
            ModLoader::Cleanroom,
            "1.12.2",
            &mut info,
            "network",
        ));
        assert_eq!(
            info.java_version.as_ref().map(|java| java.major_version),
            Some(25)
        );
    }

    #[test]
    fn vanilla_stays_on_inherited_java_runtime() {
        let mut info = version_info_with_java(None);
        assert!(!normalize_version_info(
            ModLoader::Vanilla,
            "1.12.2",
            &mut info,
            "network",
        ));
        assert!(info.java_version.is_none());
    }

    #[test]
    fn batch_assets_coalesce_duplicate_objects_and_keep_legacy_targets() {
        let destination = PathBuf::from("assets/objects/ab/abcdef");
        let first_legacy = PathBuf::from("resources/first");
        let second_legacy = PathBuf::from("resources/second");
        let make_item = |legacy_destinations| H2BatchAsset {
            url: "https://resources.download.minecraft.net/ab/abcdef".into(),
            destination: destination.clone(),
            legacy_destinations,
            sha1: "abcdef".into(),
            size: 42,
            logical_items: 1,
        };

        let items = coalesce_batch_assets(vec![
            make_item(vec![first_legacy.clone()]),
            make_item(vec![second_legacy.clone(), first_legacy.clone()]),
        ]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].logical_items, 2);
        assert_eq!(
            items[0].legacy_destinations,
            vec![first_legacy, second_legacy]
        );
    }

    #[test]
    fn batch_assets_do_not_coalesce_different_integrity_contracts() {
        let destination = PathBuf::from("assets/objects/ab/object");
        let make_item = |sha1: &str, size| H2BatchAsset {
            url: format!("https://resources.download.minecraft.net/ab/{sha1}"),
            destination: destination.clone(),
            legacy_destinations: Vec::new(),
            sha1: sha1.into(),
            size,
            logical_items: 1,
        };

        assert_eq!(
            coalesce_batch_assets(vec![
                make_item("first", 42),
                make_item("second", 42),
                make_item("first", 43),
            ])
            .len(),
            3
        );
    }

    #[test]
    fn fallback_assets_coalesce_aliases_and_keep_legacy_targets() {
        let destination = PathBuf::from("assets/objects/85/hash");
        let first_legacy = PathBuf::from("resources/sound/liquid/splash2.ogg");
        let second_legacy =
            PathBuf::from("resources/sounds/liquid/splash2.ogg");
        let make_item = |name: &str, legacy_resource_paths: Vec<PathBuf>| {
            FallbackAsset {
                name: name.into(),
                hash: "857abbbfb58186c2f1b5510a4072630950e518f6".into(),
                size: 36_747,
            url: "https://resources.download.minecraft.net/85/857abbbfb58186c2f1b5510a4072630950e518f6".into(),
            resource_path: destination.clone(),
                legacy_resource_paths,
                logical_items: 1,
            }
        };

        let items = coalesce_fallback_assets(vec![
            make_item("sound/liquid/splash2.ogg", vec![first_legacy.clone()]),
            make_item(
                "sounds/liquid/splash2.ogg",
                vec![second_legacy.clone(), first_legacy.clone()],
            ),
        ]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].logical_items, 2);
        assert_eq!(
            items[0].legacy_resource_paths,
            vec![first_legacy, second_legacy]
        );
    }

    #[test]
    fn fallback_counts_each_logical_asset_once_independently_of_legacy_aliases()
    {
        assert_eq!(fallback_progress_units(3, false).count(), 3);
        assert_eq!(fallback_progress_units(3, true).count(), 2);
        assert_eq!(fallback_progress_units(1, true).count(), 0);
        let item = fallback_batch_asset(H2BatchAsset {
            url: "https://mirror.example/ab/object".into(),
            destination: "object".into(),
            legacy_destinations: vec!["legacy".into()],
            sha1: "ab".repeat(20),
            size: 7,
            logical_items: 3,
        });
        assert!(
            item.url
                .starts_with("https://resources.download.minecraft.net/")
        );
        assert_eq!(item.logical_items, 3);
        assert_eq!(item.legacy_resource_paths.len(), 1);
    }

    #[tokio::test]
    async fn fallback_validates_the_hash_of_an_existing_same_size_asset() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("asset");
        std::fs::write(&path, b"good").unwrap();
        let hash = sha1_smol::Sha1::from(b"good").hexdigest();
        assert!(verified_asset_is_usable(&path, &hash, 4).await.unwrap());
        std::fs::write(&path, b"evil").unwrap();
        assert!(!verified_asset_is_usable(&path, &hash, 4).await.unwrap());
    }

    #[test]
    fn fallback_assets_do_not_coalesce_different_integrity_contracts() {
        let destination = PathBuf::from("assets/objects/85/object");
        let make_item = |hash: &str, size| FallbackAsset {
            name: hash.into(),
            hash: hash.into(),
            size,
            url: format!("https://resources.download.minecraft.net/85/{hash}"),
            resource_path: destination.clone(),
            legacy_resource_paths: Vec::new(),
            logical_items: 1,
        };

        assert_eq!(
            coalesce_fallback_assets(vec![
                make_item("first", 42),
                make_item("second", 42),
                make_item("first", 43),
            ])
            .len(),
            3
        );
    }

    #[test]
    fn asset_fast_path_requires_a_regular_file_with_expected_size() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("valid");
        let truncated = directory.path().join("truncated");
        let folder = directory.path().join("folder");
        std::fs::write(&valid, b"asset").unwrap();
        std::fs::write(&truncated, b"as").unwrap();
        std::fs::create_dir(&folder).unwrap();

        assert!(asset_file_is_usable(&valid, 5));
        assert!(!asset_file_is_usable(&valid, 4));
        assert!(!asset_file_is_usable(&truncated, 5));
        assert!(!asset_file_is_usable(&folder, 0));
        assert!(!asset_file_is_usable(&directory.path().join("missing"), 5));
    }

    #[tokio::test]
    async fn cached_artifact_with_wrong_size_is_not_reused() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("asset");
        std::fs::write(&destination, b"bad").unwrap();

        assert_eq!(
            classify_local_artifact(
                None,
                &destination,
                Path::new("assets/objects/00/hash"),
                Some("unused"),
                Some(5),
            )
            .await
            .unwrap(),
            ArtifactAvailability::NetworkRequired,
        );
    }

    #[tokio::test]
    async fn local_asset_availability_distinguishes_reusable_and_missing() {
        let directory = tempfile::tempdir().unwrap();
        let local_root = directory.path().join(".minecraft");
        let relative = Path::new("assets/objects/ab/hash");
        let candidate = local_root.join(relative);
        io::create_dir_all(candidate.parent().unwrap())
            .await
            .unwrap();
        io::write(&candidate, "asset").await.unwrap();
        let source = LocalRuntimeSource { root: local_root };
        let destination = directory.path().join("destination");

        assert_eq!(
            classify_local_artifact(
                Some(&source),
                &destination,
                relative,
                Some("0000000000000000000000000000000000000000"),
                Some(5),
            )
            .await
            .unwrap(),
            ArtifactAvailability::NetworkRequired
        );
        assert_eq!(
            classify_local_artifact(
                Some(&source),
                &destination,
                relative,
                Some("05fac94380a70241f23780e7aef62b190894238f"),
                Some(5),
            )
            .await
            .unwrap(),
            ArtifactAvailability::LocalReusable
        );
    }

    #[test]
    fn liteloader_library_detection_is_coordinate_specific() {
        assert!(is_liteloader_library(
            "com.mumfrey:liteloader:1.12.2-SNAPSHOT"
        ));
        assert!(!is_liteloader_library("example:liteloader:1.12.2-SNAPSHOT"));
        assert!(!is_liteloader_library("com.mumfrey:other:1.0"));
        assert!(!is_liteloader_library("com.mumfrey:liteloader"));
    }

    #[test]
    fn legacy_native_library_uses_platform_classifier_without_downloads_block()
    {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.17",
            "url": "https://maven.legacyfabric.net/",
            "natives": {
                "linux": "natives-linux",
                "osx": "natives-osx",
                "windows": "natives-windows"
            },
            "checksums": ["05fac94380a70241f23780e7aef62b190894238f"]
        }))
        .unwrap();

        assert!(library.natives_os_key_and_classifiers("x86_64").is_none());
        let classifier = library_native_classifier(&library, "x86_64").unwrap();
        assert_eq!(
            legacy_library_sha1(&library),
            Some("05fac94380a70241f23780e7aef62b190894238f")
        );
        assert_eq!(
            legacy_library_content_validation("lwjgl-platform.jar"),
            ContentValidation::Jar
        );
        let artifact_path =
            classified_library_artifact_path(&library.name, &classifier)
                .unwrap();
        assert_eq!(
            artifact_path,
            format!(
                "org/lwjgl/lwjgl/lwjgl-platform/2.9.4+legacyfabric.17/lwjgl-platform-2.9.4+legacyfabric.17-{classifier}.jar"
            )
        );
        assert_eq!(
            legacy_library_download_urls(
                library.url.as_deref(),
                &artifact_path,
            )
            .unwrap()[0],
            format!("https://maven.legacyfabric.net/{artifact_path}")
        );
    }

    #[test]
    fn native_library_with_legacy_repository_keeps_java_artifact_task() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
            "url": "https://libraries.minecraft.net/",
            "natives": {
                "windows": "natives-windows"
            }
        }))
        .unwrap();

        assert!(needs_java_artifact(&library));
    }

    #[test]
    fn processor_dependency_without_classpath_flag_keeps_java_artifact_task() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "net.neoforged.installertools:installertools:2.1.2",
            "downloads": {"artifact": {
                "url": "https://maven.neoforged.net/releases/net/neoforged/installertools/installertools/2.1.2/installertools-2.1.2.jar",
                "sha1": "", "size": 1
            }},
            "include_in_classpath": false,
            "downloadable": true
        }))
        .unwrap();

        assert!(needs_java_artifact(&library));
        assert!(java_artifact_applies(&library, "x86_64", true));
    }

    #[test]
    fn legacy_launchwrapper_without_download_metadata_is_downloaded() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "net.minecraft:launchwrapper:1.12"
        }))
        .unwrap();

        let artifact_path = d::get_path_from_artifact(&library.name).unwrap();
        assert!(needs_java_artifact(&library));
        assert_eq!(
            legacy_library_download_urls(
                library.url.as_deref(),
                &artifact_path
            )
            .unwrap(),
            vec![format!("{LIBRARIES_MAVEN}/{artifact_path}")]
        );
    }

    #[test]
    fn java_artifact_rules_are_applied_before_path_deduplication() {
        let blocked: Library = serde_json::from_value(serde_json::json!({
            "name": "example:library:1.0",
            "rules": [{"action": "allow", "features": {"is_demo_user": true}}],
            "downloads": {"artifact": {
                "url": "https://example.invalid/library.jar",
                "sha1": "", "size": 1
            }}
        }))
        .unwrap();
        let allowed: Library = serde_json::from_value(serde_json::json!({
            "name": "example:library:1.0",
            "downloads": {"artifact": {
                "url": "https://example.invalid/library.jar",
                "sha1": "", "size": 1
            }}
        }))
        .unwrap();

        assert!(!java_artifact_applies(&blocked, "x86_64", true));
        assert!(java_artifact_applies(&allowed, "x86_64", true));
    }

    #[test]
    fn empty_legacy_repository_is_not_a_java_artifact_source() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "example:native:1.0",
            "url": "",
            "natives": {"windows": "natives-windows"}
        }))
        .unwrap();

        assert!(!needs_java_artifact(&library));
    }

    #[test]
    fn forge_classifier_native_library_uses_its_declared_artifact_path() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.2.2:natives-windows"
        }))
        .unwrap();

        assert!(is_native_library(&library));
        assert_eq!(
            library_native_classifier(&library, "x86_64"),
            Some("natives-windows".to_string())
        );
        assert_eq!(
            native_library_artifact_path(&library, "natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-windows.jar"
        );
    }

    #[tokio::test]
    async fn writing_version_info_creates_missing_parent_directory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("meta/versions/1.21.11-21.11.44/1.21.11-21.11.44.json");

        assert!(!path.parent().unwrap().exists());
        write_version_info(&path, br#"{"id":"1.21.11-21.11.44"}"#.to_vec())
            .await
            .unwrap();

        assert_eq!(
            io::read(&path).await.unwrap(),
            br#"{"id":"1.21.11-21.11.44"}"#
        );
    }

    #[tokio::test]
    async fn derived_version_cache_requires_current_format_marker() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("1.20.1-47.2.20.json");
        write_version_info(&path, b"{}".to_vec()).await.unwrap();

        assert!(!derived_version_cache_is_current(&path).await);
        io::write(derived_version_cache_marker_path(&path), "3")
            .await
            .unwrap();
        assert!(!derived_version_cache_is_current(&path).await);

        write_derived_version_cache_marker(&path).await.unwrap();
        assert!(derived_version_cache_is_current(&path).await);
    }

    #[test]
    fn legacy_launcher_meta_maven_uses_canonical_repositories() {
        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven/"),
                "net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
            ),
            urls(&[
                "https://maven.fabricmc.net/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
                "https://launcher-meta.modrinth.com/maven/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
                "https://libraries.minecraft.net/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
            ]),
        );
        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven/"),
                "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
            ),
            urls(&[
                "https://repo.maven.apache.org/maven2/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
                "https://launcher-meta.modrinth.com/maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
                "https://libraries.minecraft.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
            ]),
        );
        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven/"),
                "com/modrinth/daedalus/forge-installer-extracts/1.20.1-47.4.20/forge-installer-extracts-1.20.1-47.4.20-client.lzma",
            ),
            urls(&[
                "https://launcher-meta.modrinth.com/maven/com/modrinth/daedalus/forge-installer-extracts/1.20.1-47.4.20/forge-installer-extracts-1.20.1-47.4.20-client.lzma",
                "https://libraries.minecraft.net/com/modrinth/daedalus/forge-installer-extracts/1.20.1-47.4.20/forge-installer-extracts-1.20.1-47.4.20-client.lzma",
            ]),
        );
        assert_eq!(
            legacy_library_content_validation("library.jar"),
            ContentValidation::Jar,
        );
        assert_eq!(
            legacy_library_content_validation("library.lzma"),
            ContentValidation::None,
        );
    }

    #[test]
    fn legacy_scala_libraries_use_maven_central() {
        let artifact_path = "org/scala-lang/plugins/scala-continuations-library_2.11/1.0.2/scala-continuations-library_2.11-1.0.2.jar";
        let expected = urls(&[
            "https://repo.maven.apache.org/maven2/org/scala-lang/plugins/scala-continuations-library_2.11/1.0.2/scala-continuations-library_2.11-1.0.2.jar",
            "https://launcher-meta.modrinth.com/maven/org/scala-lang/plugins/scala-continuations-library_2.11/1.0.2/scala-continuations-library_2.11-1.0.2.jar",
            "https://libraries.minecraft.net/org/scala-lang/plugins/scala-continuations-library_2.11/1.0.2/scala-continuations-library_2.11-1.0.2.jar",
        ]);

        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven"),
                artifact_path,
            ),
            expected,
        );
        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven/"),
                artifact_path,
            ),
            expected,
        );
    }

    #[test]
    fn historical_forge_scala_modules_keep_archived_coordinates() {
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.scala-lang:scala-xml_2.11:1.0.2",
            "url": "https://launcher-meta.modrinth.com/maven/",
            "checksums": [
                "7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0",
                "62736b01689d56b6d09a0164b7ef9da2b0b9633d"
            ]
        }))
        .unwrap();
        let artifact_path = d::get_path_from_artifact(&library.name).unwrap();

        assert_eq!(
            artifact_path,
            "org/scala-lang/scala-xml_2.11/1.0.2/scala-xml_2.11-1.0.2.jar",
        );
        assert_eq!(
            legacy_library_sha1(&library),
            Some("7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0"),
        );

        for artifact_path in [
            "org/scala-lang/scala-parser-combinators_2.11/1.0.1/scala-parser-combinators_2.11-1.0.1.jar",
            "org/scala-lang/scala-swing_2.11/1.0.1/scala-swing_2.11-1.0.1.jar",
            "org/scala-lang/scala-xml_2.11/1.0.2/scala-xml_2.11-1.0.2.jar",
        ] {
            assert_eq!(
                legacy_library_download_urls(
                    Some("https://launcher-meta.modrinth.com/maven/"),
                    artifact_path,
                ),
                Some(vec![
                    format!("{FORGE_MAVEN}/{artifact_path}"),
                    format!("{LAUNCHER_META_MAVEN}/{artifact_path}"),
                    format!("{LIBRARIES_MAVEN}/{artifact_path}"),
                ]),
            );
        }
    }

    #[test]
    fn legacy_forge_maven_central_groups_use_maven_central() {
        for artifact_path in [
            "org/jline/jline/3.5.1/jline-3.5.1.jar",
            "jline/jline/2.13/jline-2.13.jar",
            "net/java/dev/jna/jna/4.4.0/jna-4.4.0.jar",
            "com/typesafe/akka/akka-actor_2.11/2.3.3/akka-actor_2.11-2.3.3.jar",
            "com/typesafe/config/1.2.1/config-1.2.1.jar",
        ] {
            assert_eq!(
                legacy_library_download_urls(
                    Some("https://launcher-meta.modrinth.com/maven"),
                    artifact_path,
                ),
                Some(vec![
                    format!("{MAVEN_CENTRAL}/{artifact_path}"),
                    format!("{LAUNCHER_META_MAVEN}/{artifact_path}"),
                    format!("{LIBRARIES_MAVEN}/{artifact_path}"),
                ]),
            );
        }
    }

    #[test]
    fn legacy_maven_download_uses_trusted_declared_paths() {
        assert_eq!(
            legacy_library_download_urls(
                Some("https://launcher-meta.modrinth.com/maven/"),
                "example/unknown/1/unknown-1.jar",
            ),
            urls(&[
                "https://launcher-meta.modrinth.com/maven/example/unknown/1/unknown-1.jar",
                "https://libraries.minecraft.net/example/unknown/1/unknown-1.jar",
            ]),
        );
        assert_eq!(
            legacy_library_download_urls(
                Some("https://maven.minecraftforge.net"),
                "example/unknown/1/unknown-1.jar",
            ),
            urls(&[
                "https://maven.minecraftforge.net/example/unknown/1/unknown-1.jar",
                "https://libraries.minecraft.net/example/unknown/1/unknown-1.jar",
            ]),
        );
        assert_eq!(
            legacy_library_download_urls(
                Some("https://example.invalid/maven"),
                "net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
            ),
            urls(&[
                "https://example.invalid/maven/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
                "https://maven.fabricmc.net/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
                "https://libraries.minecraft.net/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar",
            ]),
        );
        for repository in [
            "http://example.invalid/maven",
            "https://user@example.invalid/maven",
            "https://example.invalid/maven?token=secret",
        ] {
            assert_eq!(
                legacy_library_download_urls(
                    Some(repository),
                    "example/unknown/1/unknown-1.jar",
                ),
                None,
            );
        }
        for artifact_path in [
            "/example/unknown.jar",
            "../example/unknown.jar",
            "example\\unknown.jar",
        ] {
            assert_eq!(
                legacy_library_download_urls(
                    Some("https://example.invalid/maven"),
                    artifact_path,
                ),
                None,
            );
        }
    }

    fn write_native_archive(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;

        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    fn modern_native_library(sha1: &str) -> Library {
        serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl-platform:3.2.1",
            "natives": { "windows": "natives-windows" },
            "downloads": {
                "classifiers": {
                    "natives-windows": {
                        "sha1": sha1,
                        "size": 64,
                        "url": "https://example.com/natives.jar"
                    }
                }
            }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn missing_natives_are_restored_from_cached_archives() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");
        let cache = caches_dir.join("minecraft-natives").join("deadbeef.jar");
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        write_native_archive(
            &cache,
            &[("lwjgl.dll", b"native-binary"), ("sub/inner.dll", b"inner")],
        );

        let libraries = [modern_native_library("deadbeef")];
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        let natives_dir = natives_root.join("1.16.5");
        assert_eq!(
            std::fs::read(natives_dir.join("lwjgl.dll")).unwrap(),
            b"native-binary"
        );
        assert_eq!(
            std::fs::read(natives_dir.join("sub/inner.dll")).unwrap(),
            b"inner"
        );

        // Same-length corruption must be repaired on later launches.
        std::fs::write(natives_dir.join("lwjgl.dll"), b"tampered").unwrap();
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(natives_dir.join("lwjgl.dll")).unwrap(),
            b"native-binary"
        );
    }

    #[tokio::test]
    async fn only_missing_or_zero_byte_native_entries_are_restored() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");
        let cache = caches_dir.join("minecraft-natives").join("deadbeef.jar");
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        write_native_archive(
            &cache,
            &[
                ("a.dll", b"from-archive"),
                ("b.dll", b"repaired"),
                ("c.txt", b"added"),
            ],
        );

        let natives_dir = natives_root.join("1.16.5");
        std::fs::create_dir_all(&natives_dir).unwrap();
        std::fs::write(natives_dir.join("a.dll"), b"custom").unwrap();
        std::fs::write(natives_dir.join("b.dll"), b"").unwrap();

        let libraries = [modern_native_library("deadbeef")];
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read(natives_dir.join("a.dll")).unwrap(),
            b"from-archive"
        );
        assert_eq!(
            std::fs::read(natives_dir.join("b.dll")).unwrap(),
            b"repaired"
        );
        assert_eq!(std::fs::read(natives_dir.join("c.txt")).unwrap(), b"added");
    }

    #[tokio::test]
    async fn native_entries_escaping_natives_directory_are_ignored() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");
        let cache = caches_dir.join("minecraft-natives").join("deadbeef.jar");
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        write_native_archive(
            &cache,
            &[("../evil.dll", b"boom"), ("ok.dll", b"fine")],
        );

        let libraries = [modern_native_library("deadbeef")];
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        let natives_dir = natives_root.join("1.16.5");
        assert_eq!(std::fs::read(natives_dir.join("ok.dll")).unwrap(), b"fine");
        assert!(!natives_root.join("evil.dll").exists());
        assert!(!directory.path().join("evil.dll").exists());
    }

    #[tokio::test]
    async fn missing_native_archive_reports_actionable_error() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");

        let libraries = [modern_native_library("nonexistent")];
        let error = prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Repair the instance while online")
        );
    }

    #[tokio::test]
    async fn missing_native_archive_with_extracted_natives_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");

        let natives_dir = natives_root.join("1.16.5");
        std::fs::create_dir_all(&natives_dir).unwrap();
        std::fs::write(natives_dir.join("lwjgl.dll"), b"native-binary")
            .unwrap();

        let libraries = [modern_native_library("nonexistent")];
        let error = prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Repair the instance while online")
        );
    }

    #[tokio::test]
    async fn missing_cached_archive_falls_back_to_classified_library_archive() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");
        let archive = libraries_dir.join(
            "org/lwjgl/lwjgl-platform/3.2.1/lwjgl-platform-3.2.1-natives-windows.jar",
        );
        std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
        write_native_archive(&archive, &[("lwjgl.dll", b"from-libraries")]);

        let libraries = [modern_native_library("deadbeef")];
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &libraries,
            "1.16.5",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read(natives_root.join("1.16.5").join("lwjgl.dll"))
                .unwrap(),
            b"from-libraries"
        );
    }

    #[tokio::test]
    async fn legacy_native_libraries_restore_from_libraries_directory() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");
        let archive = libraries_dir.join(
            "org/lwjgl/lwjgl/lwjgl-platform/2.9.0/lwjgl-platform-2.9.0-natives-windows.jar",
        );
        std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
        write_native_archive(&archive, &[("lwjgl.dll", b"legacy-binary")]);

        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.0",
            "natives": {
                "linux": "natives-linux",
                "osx": "natives-osx",
                "windows": "natives-windows"
            }
        }))
        .unwrap();
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &[library],
            "1.6.4-9.11.1.1345",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read(
                natives_root.join("1.6.4-9.11.1.1345").join("lwjgl.dll")
            )
            .unwrap(),
            b"legacy-binary"
        );
    }

    #[tokio::test]
    async fn non_native_libraries_do_not_trigger_repairs() {
        let directory = tempfile::tempdir().unwrap();
        let natives_root = directory.path().join("natives");
        let libraries_dir = directory.path().join("libraries");
        let caches_dir = directory.path().join("caches");

        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "net.sf.jopt-simple:jopt-simple:4.5"
        }))
        .unwrap();
        prepare_test_natives(
            &natives_root,
            &libraries_dir,
            &caches_dir,
            &[library],
            "1.6.4",
            "x86_64",
            true,
        )
        .await
        .unwrap();

        let natives_dir = natives_root.join("1.6.4");
        assert!(natives_dir.exists());
        assert_eq!(std::fs::read_dir(&natives_dir).unwrap().count(), 0);
    }
}
