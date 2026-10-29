//! Dependency completion for directly linked installations (HMCL/PCL parity).
//!
//! HMCL and PCL auto-download missing libraries, assets, and logging configs
//! into their `.minecraft` before launching. A directly associated instance
//! shares that folder with those launchers, so Axolotl completes the same
//! files in the same places before building the launch command:
//!
//! - `libraries/<maven path>` — classpath jars and native classifier jars;
//! - `assets/indexes/<id>.json` plus missing `assets/objects/<h2>/<hash>`;
//! - `assets/log_configs/<id>`.
//!
//! The write boundary is deliberate and narrow: only these standard shared
//! resources are ever created. Version JSONs (`versions/**/*.json`),
//! launcher-private configuration (`PCL/`, `.hmcl/`), and game data (mods,
//! config, saves, resourcepacks, options.txt, ...) are never written; native
//! extraction continues to happen inside Axolotl's own cache.
//!
//! Every download streams to a sibling `.part` file, verifies its declared
//! SHA1, and is renamed into place atomically (see `download_to_path`), so a
//! failed or interrupted fetch never leaves a half-written jar behind. A
//! file whose declared SHA1 no longer matches is treated as corrupt and
//! re-downloaded, mirroring HMCL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use daedalus::minecraft::{
    AssetIndex, AssetsIndex, DownloadType, LoggingConfiguration, LoggingSide,
    VersionInfo,
};
use futures::prelude::*;

use super::direct_link::{DirectLinkedLaunch, is_native_only_library};
use super::download::{
    LIBRARIES_MAVEN, MinecraftDownloadProgress, legacy_library_download_urls,
    legacy_library_sha1, minecraft_library_mirrors,
};
use super::local_version::LinkedLibrary;
use crate::instance::QuickPlayType;
use crate::state::State;
use crate::util::fetch::{
    self, ContentValidation, DownloadRequest, Integrity, ResourceClass,
};
use crate::util::{download as download_util, io};

/// Concurrency used when no explicit limit is configured for this session.
const FALLBACK_CONCURRENCY: usize = 64;

/// A single file that must exist in the linked installation before launch.
#[derive(Debug, Clone)]
pub(crate) struct LinkedFilePlan {
    /// Human-readable identity used in error messages (library coordinate,
    /// asset hash, or log config id).
    pub label: String,
    /// Ordered download candidates; the first entry is the primary URL.
    pub urls: Vec<String>,
    /// Absolute destination inside the linked installation.
    pub destination: PathBuf,
    /// Declared SHA1, when the version JSON provides one.
    pub sha1: Option<String>,
    /// Declared size hint in bytes, when provided.
    pub size: Option<u64>,
    pub validation: ContentValidation,
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Rejects relative artifact paths that would escape the target directory
/// (absolute paths, backslashes, or `..`/empty segments).
pub(crate) fn safe_maven_relative_path(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.starts_with('/')
        && !relative.contains('\\')
        && !relative.contains(':')
        && relative.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".."
        })
}

/// BMCLAPI's Maven proxy mirrors `libraries.minecraft.net` 1:1 by artifact
/// path. Appending it and the official Mojang Maven after every primary URL
/// mirrors HMCL/PCL behaviour, which walk several sources because Mojang's
/// Maven has lost many legacy artifacts over the years (e.g. `net.java.jinput`
/// native platform jars returning HTTP 404).
const BMCLAPI_MAVEN: &str = "https://bmclapi2.bangbang93.com/maven";

/// Maven Central retains legacy Minecraft artifacts that both Mojang's Maven
/// and BMCLAPI have dropped over the years (e.g. `net.java.jinput` platform
/// jars); it is the final fallback of the candidate chain.
const MAVEN_CENTRAL_MAVEN: &str = "https://repo1.maven.org/maven2";

/// Appends the BMCLAPI, Mojang Maven and Maven Central locations for
/// `relative_path` to the candidate chain, skipping URLs already present so
/// declared sources keep their priority while duplicates never cause a
/// second attempt.
fn push_maven_fallbacks(urls: &mut Vec<String>, relative_path: &str) {
    for base in [BMCLAPI_MAVEN, LIBRARIES_MAVEN, MAVEN_CENTRAL_MAVEN] {
        let candidate = format!("{base}/{relative_path}");
        if !urls.contains(&candidate) {
            urls.push(candidate);
        }
    }
}

fn declared_download_urls(
    library_name: &str,
    repository: Option<&str>,
    declared_url: Option<&str>,
    relative_path: &str,
) -> crate::Result<Vec<String>> {
    let mut urls = match declared_url {
        Some(url) => minecraft_library_mirrors(url),
        None => Vec::new(),
    };
    if urls.is_empty() {
        // Old format: resolve through the declaring repository, always ending
        // with Mojang's Maven as the default location.
        let mut urls = legacy_library_download_urls(repository, relative_path)
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(format!(
                    "No safe download location is known for required \
                         linked library {library_name}"
                ))
            })?;
        // legacy_library_download_urls already ends with the Mojang Maven;
        // dedup keeps that position while BMCLAPI slots in before it.
        push_maven_fallbacks(&mut urls, relative_path);
        return Ok(urls);
    }
    push_maven_fallbacks(&mut urls, relative_path);
    Ok(urls)
}

/// Plan for the classpath artifact of a linked library. Returns `None` when no
/// plain jar must be fetched: natives-only declarations (a `natives` mapping
/// without `downloads.artifact`, e.g. `net.java.jinput:jinput-platform:2.0.5`)
/// have no plain jar on any repository and only their classifier jar is
/// planned, by [`linked_native_plan`]; local-hint libraries without a declared
/// URL are expected to ship with the installation (the caller still reports
/// genuinely missing artifacts while building the classpath, exactly as
/// before).
pub(crate) fn linked_classpath_plan(
    direct: &DirectLinkedLaunch,
    library: &LinkedLibrary,
) -> crate::Result<Option<LinkedFilePlan>> {
    let lib = &library.library;
    // Natives-only libraries ship no plain jar anywhere, so planning one
    // would 404 on every mirror and abort the launch; the vanilla launcher
    // never downloads it either.
    if is_native_only_library(lib) {
        return Ok(None);
    }
    let destination = direct.library_path(library)?;
    let relative = library.classpath_relative_path()?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    if !safe_maven_relative_path(&relative) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Refusing unsafe library path {relative:?} for {}",
            lib.name
        ))
        .into());
    }

    let declared = lib
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref());
    let sha1 = declared
        .and_then(|artifact| non_empty(&artifact.sha1))
        .or_else(|| legacy_library_sha1(lib).map(str::to_string));
    let size = declared
        .filter(|artifact| !artifact.url.trim().is_empty() && artifact.size > 0)
        .map(|artifact| artifact.size as u64);
    let declared_url = declared.and_then(|artifact| {
        let url = artifact.url.trim();
        (!url.is_empty()).then(|| url.to_string())
    });
    // Local-hint libraries are expected to ship with the installation; only
    // complete them over the network when a URL is explicitly declared.
    if library.hint.as_deref() == Some("local") && declared_url.is_none() {
        return Ok(None);
    }
    let urls = declared_download_urls(
        &lib.name,
        lib.url.as_deref(),
        declared_url.as_deref(),
        &relative,
    )?;

    Ok(Some(LinkedFilePlan {
        label: lib.name.clone(),
        urls,
        destination,
        sha1,
        size,
        validation: ContentValidation::None,
    }))
}

/// Plan for the client jar that the directly linked version starts. The
/// merged document carries the vanilla client's declared download while the
/// launcher convention keeps the jar beside the linked version JSON.
fn linked_client_plan(
    direct: &DirectLinkedLaunch,
    version_info: &VersionInfo,
) -> Option<LinkedFilePlan> {
    let client = version_info.downloads.get(&DownloadType::Client)?;
    let url = non_empty(&client.url)?;

    Some(LinkedFilePlan {
        label: format!("Minecraft client {}", direct.version_id),
        urls: vec![url],
        destination: direct.client_jar(&direct.version_id),
        sha1: non_empty(&client.sha1),
        size: (client.size > 0).then_some(client.size as u64),
        validation: ContentValidation::Jar,
    })
}

/// Plan for the native classifier jar selected for this platform, using the
/// same path resolution as native extraction so the archive is present before
/// extraction runs. Returns `None` when no native applies to this platform.
pub(crate) fn linked_native_plan(
    direct: &DirectLinkedLaunch,
    library: &LinkedLibrary,
    java_arch: &str,
) -> crate::Result<Option<LinkedFilePlan>> {
    let lib = &library.library;
    let Some((classifier, download)) =
        super::direct_link::native_download(library, java_arch)
    else {
        return Ok(None);
    };

    let destination = if library.hint.as_deref() == Some("local") {
        direct.library_path(library)?
    } else {
        let relative = match download.and_then(|download| download.path.clone())
        {
            Some(path) => PathBuf::from(path),
            None => super::direct_link::classified_artifact_path(
                &lib.name,
                &classifier,
            )?
            .into(),
        };
        direct.libraries_dir().join(relative)
    };

    let relative = match download.and_then(|download| download.path.clone()) {
        Some(path) => path,
        None => super::direct_link::classified_artifact_path(
            &lib.name,
            &classifier,
        )?,
    };
    if !safe_maven_relative_path(&relative) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Refusing unsafe native library path {relative:?} for {}",
            lib.name
        ))
        .into());
    }

    let sha1 = download
        .and_then(|download| non_empty(&download.sha1))
        .or_else(|| legacy_library_sha1(lib).map(str::to_string));
    let size = download
        .filter(|download| !download.url.trim().is_empty() && download.size > 0)
        .map(|download| download.size as u64);
    let declared_url = download.and_then(|download| non_empty(&download.url));
    // Same local-hint rule as the classpath artifact: no declared URL means
    // the archive must already be part of the installation.
    if library.hint.as_deref() == Some("local") && declared_url.is_none() {
        return Ok(None);
    }
    let urls = declared_download_urls(
        &lib.name,
        lib.url.as_deref(),
        declared_url.as_deref(),
        &relative,
    )?;

    Ok(Some(LinkedFilePlan {
        label: format!("{}:{classifier}", lib.name),
        urls,
        destination,
        sha1,
        size,
        validation: ContentValidation::Jar,
    }))
}

/// Whether the file exists and satisfies the metadata available for it.
/// SHA1 is authoritative when declared; otherwise a declared size still
/// protects against accepting a partial or truncated file. An unreadable file
/// counts as not current so it gets replaced.
pub(crate) async fn file_is_current(
    path: &std::path::Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
) -> bool {
    if !path.is_file() {
        return false;
    }
    if let Some(expected_size) = expected_size
        && std::fs::metadata(path)
            .map_or(true, |metadata| metadata.len() != expected_size)
    {
        return false;
    }
    match expected_sha1 {
        Some(expected) => match fetch::sha1_file_async(path).await {
            Ok((_, actual)) => actual.eq_ignore_ascii_case(expected),
            Err(_) => false,
        },
        None => true,
    }
}

/// Validates locally supplied runtime artifacts without fetching them from Maven.
pub(crate) async fn validate_local_runtime_library(
    direct: &DirectLinkedLaunch,
    library: &LinkedLibrary,
    java_arch: &str,
) -> crate::Result<()> {
    let mut plans = Vec::new();
    if library.library.include_in_classpath
        && let Some(plan) = linked_classpath_plan(direct, library)?
    {
        plans.push(plan);
    }
    if let Some(plan) = linked_native_plan(direct, library, java_arch)? {
        plans.push(plan);
    }
    for plan in plans {
        if !file_is_current(&plan.destination, plan.sha1.as_deref(), plan.size)
            .await
        {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Missing or invalid required runtime artifact {} at {}",
                plan.label,
                plan.destination.display()
            ))
            .into());
        }
    }
    Ok(())
}

/// Makes sure one planned file exists in the linked installation. Returns
/// `true` when it had to be downloaded.
async fn ensure_file(st: &State, plan: &LinkedFilePlan) -> crate::Result<bool> {
    if file_is_current(&plan.destination, plan.sha1.as_deref(), plan.size).await
    {
        return Ok(false);
    }

    let Some(primary) = plan.urls.first() else {
        return Err(crate::ErrorKind::LauncherError(format!(
            "No download location is known for required linked dependency {}",
            plan.label
        ))
        .into());
    };
    let request =
        DownloadRequest::new(primary, ResourceClass::MinecraftLibrary)
            .with_candidate_urls(plan.urls.iter().skip(1).cloned())
            .with_integrity(Integrity {
                size: plan.size,
                sha1: plan.sha1.clone(),
                content: plan.validation,
                ..Integrity::default()
            });
    fetch::download_to_path(
        request,
        &plan.destination,
        &st.download_semaphore,
        None,
    )
    .await
    .map_err(|error| {
        // The fetch layer already walked every candidate (mirrors switch
        // silently on 404s and network errors, like HMCL/PCL); surface the
        // whole chain so a total failure is diagnosable.
        crate::ErrorKind::LauncherError(format!(
            "Failed to download required linked dependency {} from any of \
             the {} attempted location(s) [{}]: {error}",
            plan.label,
            plan.urls.len(),
            plan.urls.join(", ")
        ))
    })?;
    Ok(true)
}

/// Official Mojang resource download base for asset objects.
const MINECRAFT_RESOURCES_BASE: &str =
    "https://resources.download.minecraft.net";

/// Ensures the assets index exists under the linked `assets/indexes` and then
/// backfills missing or corrupt asset objects under `assets/objects`.
pub(crate) async fn ensure_linked_assets(
    st: &State,
    direct: &DirectLinkedLaunch,
    asset_index: &AssetIndex,
    with_legacy: bool,
) -> crate::Result<()> {
    ensure_linked_assets_from_with_progress(
        st,
        direct,
        asset_index,
        MINECRAFT_RESOURCES_BASE,
        with_legacy,
        None,
    )
    .await
}

pub(crate) async fn ensure_linked_assets_from(
    st: &State,
    direct: &DirectLinkedLaunch,
    asset_index: &AssetIndex,
    resources_base: &str,
    with_legacy: bool,
) -> crate::Result<()> {
    ensure_linked_assets_from_with_progress(
        st,
        direct,
        asset_index,
        resources_base,
        with_legacy,
        None,
    )
    .await
}

async fn ensure_linked_assets_from_with_progress(
    st: &State,
    direct: &DirectLinkedLaunch,
    asset_index: &AssetIndex,
    resources_base: &str,
    with_legacy: bool,
    progress: Option<&MinecraftDownloadProgress>,
) -> crate::Result<()> {
    let Some(index_id) = non_empty(&asset_index.id) else {
        return Ok(());
    };
    let index_path = direct
        .assets_dir()
        .join("indexes")
        .join(format!("{index_id}.json"));

    if !file_is_current(
        &index_path,
        non_empty(&asset_index.sha1).as_deref(),
        (asset_index.size > 0).then_some(asset_index.size as u64),
    )
    .await
    {
        let Some(index_url) = non_empty(&asset_index.url) else {
            if !index_path.exists() {
                tracing::warn!(
                    index = %index_id,
                    path = %index_path.display(),
                    "Linked assets index is missing and declares no download \
                     URL; continuing without completing assets"
                );
            }
            return Ok(());
        };
        // No BMCLAPI fallback for asset indexes: BMCLAPI only proxies asset
        // objects, libraries, and version metadata, and has no stable mapping
        // for `assets/indexes` (the same reason HMCL keeps the declared URL
        // as the single source here).
        let request = DownloadRequest::new(&index_url, ResourceClass::Metadata)
            .with_integrity(Integrity {
                size: (asset_index.size > 0).then_some(asset_index.size as u64),
                sha1: non_empty(&asset_index.sha1),
                content: ContentValidation::Json,
                ..Integrity::default()
            });
        if let Some(progress) = progress {
            progress.add_total(asset_index.size as u64).await?;
        }
        fetch::download_to_path(
            request,
            &index_path,
            &st.download_semaphore,
            None,
        )
        .await
        .map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Failed to download required assets index {index_id} from \
                 {index_url}: {error}"
            ))
        })?;
        if let Some(progress) = progress {
            progress.add_bytes(asset_index.size as u64).await?;
        }
    }

    let bytes = match io::read(&index_path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                index = %index_id,
                %error,
                "Failed to read the linked assets index; continuing without \
                 completing assets"
            );
            return Ok(());
        }
    };
    let index: AssetsIndex = match serde_json::from_slice(&bytes) {
        Ok(index) => index,
        Err(error) => {
            tracing::warn!(
                index = %index_id,
                %error,
                "Failed to parse the linked assets index; continuing without \
                 completing assets"
            );
            return Ok(());
        }
    };

    let objects_dir = direct.assets_dir().join("objects");
    let limit = download_util::task_concurrency_limit(st)
        .unwrap_or(FALLBACK_CONCURRENCY);
    let asset_plans = index
        .objects
        .values()
        .filter_map(|asset| {
            let hash = &asset.hash;
            (hash.len() >= 2).then(|| {
                (
                    hash.clone(),
                    u64::from(asset.size),
                    objects_dir.join(&hash[..2]).join(hash),
                )
            })
        })
        .collect::<Vec<_>>();
    let missing = stream::iter(asset_plans)
        .map(|(hash, size, destination)| async move {
            (!file_is_current(&destination, Some(&hash), Some(size)).await)
                .then_some((hash, size, destination))
        })
        .buffer_unordered(limit)
        .filter_map(|item| async move { item })
        .collect::<Vec<_>>()
        .await;
    if !missing.is_empty() {
        tracing::info!(
            count = missing.len(),
            "Downloading missing Minecraft assets into the linked installation"
        );
        if let Some(progress) = progress {
            progress
                .add_total(missing.iter().map(|(_, size, _)| *size).sum())
                .await?;
        }

        stream::iter(missing)
            .map(Ok::<_, crate::Error>)
            .try_for_each_concurrent(
                limit,
                |(hash, size, destination)| async move {
                    let url = format!("{resources_base}/{}/{}", &hash[..2], hash);
                    let request = DownloadRequest::new(
                        &url,
                        ResourceClass::MinecraftAsset,
                    )
                    .with_integrity(Integrity::sha1(&hash).with_size(size));
                    fetch::download_to_path(
                        request,
                        &destination,
                        &st.download_semaphore,
                        None,
                    )
                    .await
                    .map_err(|error| {
                        crate::ErrorKind::LauncherError(format!(
                            "Failed to download required Minecraft asset {hash} from \
                             {url}: {error}"
                        ))
                    })?;
                    if let Some(progress) = progress {
                        progress.add_bytes(size).await?;
                    }
                    Ok(())
                },
            )
            .await?;
    }

    if with_legacy {
        let legacy_root = direct.assets_dir().join("virtual").join("legacy");
        for (name, asset) in &index.objects {
            if asset.hash.len() < 2 {
                continue;
            }
            let object = objects_dir.join(&asset.hash[..2]).join(&asset.hash);
            let legacy = legacy_root.join(Path::new(name));
            if !legacy.is_file() {
                if let Some(parent) = legacy.parent() {
                    io::create_dir_all(parent).await?;
                }
                fetch::copy(&object, &legacy, &st.io_semaphore).await?;
                if let Some(progress) = progress {
                    progress.add_total(asset.size as u64).await?;
                    progress.add_bytes(asset.size as u64).await?;
                }
            }
        }
    }
    Ok(())
}

/// Ensures the client logging configuration exists under the linked
/// `assets/log_configs`, matching where the JVM argument points.
pub(crate) async fn ensure_linked_log_config(
    st: &State,
    direct: &DirectLinkedLaunch,
    logging: Option<&HashMap<LoggingSide, LoggingConfiguration>>,
) -> crate::Result<()> {
    let Some(LoggingConfiguration::Log4j2Xml { file, .. }) =
        logging.and_then(|logging| logging.get(&LoggingSide::Client))
    else {
        return Ok(());
    };
    let Some(config_id) = non_empty(&file.id) else {
        return Ok(());
    };
    let destination = direct.log_configs_dir().join(&config_id);
    if file_is_current(
        &destination,
        non_empty(&file.sha1).as_deref(),
        (file.size > 0).then_some(file.size as u64),
    )
    .await
    {
        return Ok(());
    }
    let Some(config_url) = non_empty(&file.url) else {
        tracing::warn!(
            config = %config_id,
            path = %destination.display(),
            "Linked log config is missing and declares no download URL; \
             continuing"
        );
        return Ok(());
    };
    // No BMCLAPI fallback for log configs: BMCLAPI has no stable mapping for
    // `assets/log_configs`, so the declared Mojang URL remains the single
    // source, like HMCL.

    let request =
        DownloadRequest::new(&config_url, ResourceClass::MinecraftLibrary)
            .with_integrity(Integrity {
                size: (file.size > 0).then_some(file.size as u64),
                sha1: non_empty(&file.sha1),
                content: ContentValidation::None,
                ..Integrity::default()
            });
    fetch::download_to_path(
        request,
        &destination,
        &st.download_semaphore,
        None,
    )
    .await
    .map_err(|error| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to download required log config {config_id} from \
             {config_url}: {error}"
        ))
    })?;
    Ok(())
}

/// Entry point on the direct-launch path: completes every missing dependency
/// the launch will read from the linked installation. Any failure aborts the
/// launch with a message naming the offending file and URL, like HMCL.
pub(crate) async fn ensure_direct_launch_dependencies(
    st: &State,
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    version_info: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<()> {
    ensure_direct_launch_dependencies_with_progress(
        st,
        direct,
        libraries,
        version_info,
        java_arch,
        minecraft_updated,
        None,
    )
    .await
}

pub(crate) async fn ensure_direct_launch_dependencies_with_progress(
    st: &State,
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    version_info: &VersionInfo,
    java_arch: &str,
    minecraft_updated: bool,
    progress: Option<&MinecraftDownloadProgress>,
) -> crate::Result<()> {
    let mut plans = Vec::new();
    if let Some(plan) = linked_client_plan(direct, version_info) {
        plans.push(plan);
    }
    for library in libraries {
        if let Some(rules) = library.library.rules.as_deref()
            && !super::parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }
        if !library.library.downloadable {
            validate_local_runtime_library(direct, library, java_arch).await?;
            continue;
        }
        if library.library.include_in_classpath
            && let Some(plan) = linked_classpath_plan(direct, library)?
        {
            plans.push(plan);
        }
        if let Some(plan) = linked_native_plan(direct, library, java_arch)? {
            plans.push(plan);
        }
    }

    // Only fetch what is actually missing so a healthy installation performs
    // zero network requests.
    let limit = download_util::task_concurrency_limit(st)
        .unwrap_or(FALLBACK_CONCURRENCY);
    let pending = stream::iter(plans)
        .map(|plan| async move {
            (!file_is_current(
                &plan.destination,
                plan.sha1.as_deref(),
                plan.size,
            )
            .await)
                .then_some(plan)
        })
        .buffer_unordered(limit)
        .filter_map(|plan| async move { plan })
        .collect::<Vec<_>>()
        .await;
    if !pending.is_empty() {
        tracing::info!(
            count = pending.len(),
            "Completing missing dependencies in the linked installation"
        );
        stream::iter(pending)
            .map(Ok::<_, crate::Error>)
            .try_for_each_concurrent(limit, |plan| async move {
                ensure_file(st, &plan).await.map(|_| ())
            })
            .await?;
    }

    ensure_linked_assets_from_with_progress(
        st,
        direct,
        &version_info.asset_index,
        MINECRAFT_RESOURCES_BASE,
        version_info.assets == "legacy",
        progress,
    )
    .await?;
    ensure_linked_log_config(st, direct, version_info.logging.as_ref()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::direct_link::{
        LinkedLauncherDialect, extract_linked_natives,
    };
    use super::*;
    use crate::state::{DirectoryInfo, test_state};
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::{TempDir, tempdir};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    fn direct_for(root: &Path) -> DirectLinkedLaunch {
        DirectLinkedLaunch {
            dot_minecraft: root.to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        }
    }

    fn linked_library(value: serde_json::Value) -> LinkedLibrary {
        serde_json::from_value(value).unwrap()
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        sha1_smol::Sha1::from(bytes).hexdigest()
    }

    fn minimal_version_info() -> VersionInfo {
        serde_json::from_value(json!({
            "id": "demo",
            "assets": "legacy",
            "assetIndex": {"id": "", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
            "downloads": {},
            "libraries": [],
            "mainClass": "net.minecraft.client.main.Main",
            "minimumLauncherVersion": 0,
            "releaseTime": "2013-08-06T14:00:00+02:00",
            "time": "2013-08-06T14:00:00+02:00",
            "type": "release"
        }))
        .unwrap()
    }

    async fn ensure_test_state() -> (TempDir, Arc<State>) {
        let temp = TempDir::new().unwrap();
        let dirs = DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(dirs.instances_dir()).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        (temp, test_state(dirs, pool).await.unwrap())
    }

    type FixtureHits = Arc<std::sync::Mutex<HashMap<String, usize>>>;

    /// Minimal static HTTP fixture server: serves `files` by path, 404s
    /// anything else, and counts requests per path.
    async fn spawn_fixture_server(
        files: HashMap<String, Vec<u8>>,
    ) -> (String, FixtureHits, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hits: FixtureHits = Arc::default();
        let task_hits = hits.clone();
        let handle = tokio::spawn(async move {
            let hits = task_hits;
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let files = files.clone();
                let hits = hits.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let Ok(read) = stream.read(&mut buffer).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let head = String::from_utf8_lossy(&request);
                    let path = head
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_string();
                    *hits.lock().unwrap().entry(path.clone()).or_insert(0) += 1;

                    let Some(body) = files.get(&path) else {
                        let _ = stream
                            .write_all(
                                b"HTTP/1.1 404 Not Found\r\n\
                                  Content-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .await;
                        return;
                    };
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n",
                        body.len()
                    );
                    if stream.write_all(header.as_bytes()).await.is_err() {
                        return;
                    }
                    let _ = stream.write_all(body).await;
                });
            }
        });
        (format!("http://{address}"), hits, handle)
    }

    // -----------------------------------------------------------------------
    // Plan resolution (pure)
    // -----------------------------------------------------------------------

    #[test]
    fn classpath_plan_targets_declared_artifact_path_and_url() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let library = linked_library(json!({
            "name": "com.example:lib:1.0",
            "downloads": {"artifact": {
                "path": "com/example/lib/1.0/lib-1.0.jar",
                "sha1": "abc123", "size": 10,
                "url": "https://libraries.minecraft.net/com/example/lib/1.0/lib-1.0.jar"
            }}
        }));

        let plan = linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.destination,
            root.path()
                .join("libraries/com/example/lib/1.0/lib-1.0.jar")
        );
        // Declared Mojang URL first; the BMCLAPI fallback is deduped against
        // the Mojang Maven tail (dropped as a duplicate) and Maven Central
        // stays as the final archival fallback.
        assert_eq!(
            plan.urls,
            vec![
                "https://libraries.minecraft.net/com/example/lib/1.0/lib-1.0.jar",
                "https://bmclapi2.bangbang93.com/maven/com/example/lib/1.0/lib-1.0.jar",
                "https://repo1.maven.org/maven2/com/example/lib/1.0/lib-1.0.jar",
            ]
        );
        assert_eq!(plan.sha1.as_deref(), Some("abc123"));
    }

    #[test]
    fn declared_artifact_candidate_chain_has_four_sources_in_order() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        // A natives library that DOES declare a usable `downloads.artifact`
        // keeps its plain-jar plan: the artifact exists, so the declared
        // source keeps priority, then BMCLAPI, the Mojang Maven, and Maven
        // Central (the archival home of this legacy artifact) follow in a
        // stable, deduplicated order.
        let library = linked_library(json!({
            "name": "net.java.jinput:jinput-platform:2.0.5",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
            "downloads": {
                "artifact": {
                    "path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar",
                    "sha1": "", "size": 0,
                    "url": "https://example.repo.invalid/maven/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-home.jar"
                },
                "classifiers": {
                    "natives-windows": {"path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-windows.jar", "sha1": "ccc", "size": 7, "url": "https://libraries.minecraft.net/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-windows.jar"}
                }
            }
        }));
        let plan = linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.urls,
            vec![
                "https://example.repo.invalid/maven/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-home.jar",
                "https://bmclapi2.bangbang93.com/maven/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar",
                "https://libraries.minecraft.net/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar",
                "https://repo1.maven.org/maven2/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar",
            ]
        );
    }

    #[test]
    fn legacy_library_plan_appends_bmclapi_before_the_mojang_maven() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        // The 1.12.2 LWJGL platform declaration from the bug report shape:
        // old format, no downloads block.
        let library = linked_library(json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209"
        }));

        let plan = linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.urls,
            vec![
                "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209.jar",
                "https://bmclapi2.bangbang93.com/maven/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209.jar",
                "https://repo1.maven.org/maven2/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209.jar",
            ]
        );
        assert_eq!(
            plan.destination,
            root.path().join(
                "libraries/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209.jar"
            )
        );
    }

    #[test]
    fn legacy_library_with_custom_repository_keeps_mojang_as_fallback() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let library = linked_library(json!({
            "name": "net.minecraftforge:forge:1.20.1-47.4.0",
            "url": "https://maven.minecraftforge.net/"
        }));

        let plan = linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.urls.first().map(String::as_str),
            Some(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.0/forge-1.20.1-47.4.0.jar"
            )
        );
        // The declaring repository keeps priority; BMCLAPI and the official
        // Mojang Maven both remain available as later candidates.
        assert!(plan.urls.iter().any(|url| url.starts_with(
            "https://libraries.minecraft.net/net/minecraftforge/forge/"
        )));
        assert!(plan.urls.iter().any(|url| url.starts_with(
            "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/"
        )));
    }

    #[test]
    fn local_hint_library_without_url_is_not_planned_for_download() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let library = linked_library(json!({
            "name": "local:thing:1",
            "MMC-hint": "local",
            "MMC-filename": "extra/local-thing-1.jar"
        }));

        assert!(linked_classpath_plan(&direct, &library).unwrap().is_none());
    }

    #[test]
    fn unsafe_artifact_paths_are_rejected() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        for path in ["../escape.jar", "C:/escape.jar", "artifact.jar:stream"] {
            let library = linked_library(json!({
                "name": "evil:escape:1",
                "downloads": {"artifact": {
                    "path": path, "sha1": "", "size": 0,
                    "url": "https://libraries.minecraft.net/evil/escape/1/escape-1.jar"
                }}
            }));
            let error = linked_classpath_plan(&direct, &library).unwrap_err();
            assert!(error.to_string().contains("evil:escape:1"));
        }
    }

    #[test]
    fn natives_only_library_has_no_plain_jar_classpath_plan() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        // The real 1.12.2 declaration: `natives` plus `downloads.classifiers`
        // and no `downloads.artifact` — the plain jar exists in no repository.
        let library = linked_library(json!({
            "name": "net.java.jinput:jinput-platform:2.0.5",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
            "downloads": {"classifiers": {
                "natives-linux": {"path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-linux.jar", "sha1": "aaa", "size": 5, "url": "https://libraries.minecraft.net/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-linux.jar"},
                "natives-osx": {"path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-osx.jar", "sha1": "bbb", "size": 6, "url": "https://libraries.minecraft.net/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-osx.jar"},
                "natives-windows": {"path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-windows.jar", "sha1": "ccc", "size": 7, "url": "https://libraries.minecraft.net/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-windows.jar"}
            }}
        }));

        // Planning the plain jar would 404 on every mirror and abort the
        // launch; the vanilla launcher never downloads it either.
        assert!(linked_classpath_plan(&direct, &library).unwrap().is_none());
    }

    #[test]
    fn natives_only_library_still_plans_its_platform_classifier() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let natives_os =
            serde_json::to_value(daedalus::minecraft::Os::native().get_os())
                .unwrap();
        let classifier = format!("natives-{}", natives_os.as_str().unwrap());
        // Each classifier carries the url+sha1 the ensure stage needs;
        // natives-windows is the one selected on Windows, natives-linux here.
        let mut classifiers = serde_json::Map::new();
        for os in ["linux", "osx", "windows"] {
            let classifier = format!("natives-{os}");
            classifiers.insert(
                classifier.clone(),
                json!({
                    "path": format!(
                        "net/java/jinput/jinput-platform/2.0.5/\
                         jinput-platform-2.0.5-{classifier}.jar"
                    ),
                    "sha1": sha1_hex(classifier.as_bytes()),
                    "size": 5,
                    "url": format!(
                        "https://libraries.minecraft.net/net/java/jinput/\
                         jinput-platform/2.0.5/\
                         jinput-platform-2.0.5-{classifier}.jar"
                    ),
                }),
            );
        }
        let library = linked_library(json!({
            "name": "net.java.jinput:jinput-platform:2.0.5",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
            "downloads": {"classifiers": classifiers}
        }));

        let plan =
            linked_native_plan(&direct, &library, std::env::consts::ARCH)
                .unwrap()
                .expect("the platform classifier must still be planned");
        assert_eq!(
            plan.label,
            format!("net.java.jinput:jinput-platform:2.0.5:{classifier}")
        );
        let declared_url = format!(
            "https://libraries.minecraft.net/net/java/jinput/\
             jinput-platform/2.0.5/jinput-platform-2.0.5-{classifier}.jar"
        );
        assert_eq!(
            plan.urls.first().map(String::as_str),
            Some(declared_url.as_str())
        );
        assert_eq!(
            plan.sha1.as_deref(),
            Some(sha1_hex(classifier.as_bytes()).as_str())
        );
        assert_eq!(plan.validation, ContentValidation::Jar);
        assert!(plan.destination.to_string_lossy().ends_with(
            format!("jinput-platform-2.0.5-{classifier}.jar").as_str()
        ));
    }

    #[test]
    fn natives_library_with_a_real_artifact_keeps_its_classpath_plan() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        // The `com.mojang:text2speech` shape: a `natives` mapping AND a
        // published `downloads.artifact`. Only artifacts missing everywhere
        // may be skipped, so the plain jar keeps its plan.
        let library = linked_library(json!({
            "name": "com.mojang:text2speech:1.11.3",
            "natives": {"windows": "natives-windows"},
            "downloads": {
                "artifact": {"path": "com/mojang/text2speech/1.11.3/text2speech-1.11.3.jar", "sha1": "abc", "size": 10, "url": "https://libraries.minecraft.net/com/mojang/text2speech/1.11.3/text2speech-1.11.3.jar"},
                "classifiers": {"natives-windows": {"path": "com/mojang/text2speech/1.11.3/text2speech-1.11.3-natives-windows.jar", "sha1": "ddd", "size": 1, "url": "https://libraries.minecraft.net/com/mojang/text2speech/1.11.3/text2speech-1.11.3-natives-windows.jar"}}
            }
        }));

        let plan = linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.urls.first().map(String::as_str),
            Some(
                "https://libraries.minecraft.net/com/mojang/text2speech/1.11.3/text2speech-1.11.3.jar"
            )
        );
    }

    #[test]
    fn native_plan_selects_this_platform_classifier() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let natives_os =
            serde_json::to_value(daedalus::minecraft::Os::native().get_os())
                .unwrap();
        let classifier = format!("natives-{}", natives_os.as_str().unwrap());
        let library = linked_library(json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
            "downloads": {"classifiers": {
                "natives-linux": {"path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-linux.jar", "sha1": "aaa", "size": 5, "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-linux.jar"},
                "natives-osx": {"path": "x-osx.jar", "sha1": "bbb", "size": 6, "url": "https://libraries.minecraft.net/x-osx.jar"},
                "natives-windows": {"path": "x-win.jar", "sha1": "ccc", "size": 7, "url": "https://libraries.minecraft.net/x-win.jar"}
            }}
        }));

        let plan =
            linked_native_plan(&direct, &library, std::env::consts::ARCH)
                .unwrap()
                .expect("a native applies to this platform");
        assert_eq!(
            plan.label,
            format!(
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209:{classifier}"
            )
        );
        let declared = &library
            .library
            .downloads
            .as_ref()
            .unwrap()
            .classifiers
            .as_ref()
            .unwrap()[&classifier];
        assert_eq!(
            plan.destination,
            direct.libraries_dir().join(declared.path.as_ref().unwrap())
        );
        assert_eq!(plan.sha1.as_deref(), Some(declared.sha1.as_str()));
    }

    #[test]
    fn legacy_native_without_downloads_uses_maven_classifier_path() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let natives_os =
            serde_json::to_value(daedalus::minecraft::Os::native().get_os())
                .unwrap();
        let library = linked_library(json!({
            "name": "x:native:1",
            "natives": {natives_os.as_str().unwrap(): "natives-test"}
        }));

        let plan =
            linked_native_plan(&direct, &library, std::env::consts::ARCH)
                .unwrap()
                .unwrap();
        // The classifier jar walks the same mirror chain; its maven path
        // carries the classifier name and Maven Central completes it.
        assert_eq!(
            plan.urls,
            vec![
                "https://libraries.minecraft.net/x/native/1/native-1-natives-test.jar",
                "https://bmclapi2.bangbang93.com/maven/x/native/1/native-1-natives-test.jar",
                "https://repo1.maven.org/maven2/x/native/1/native-1-natives-test.jar",
            ]
        );
    }

    #[test]
    fn rules_that_fail_are_skipped_by_the_entry_point() {
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let library = linked_library(json!({
            "name": "com.example:blocked:1",
            "rules": [{"action": "allow", "features": {"is_demo_user": true}}],
            "downloads": {"artifact": {"path": "b/1/b-1.jar", "sha1": "", "size": 0, "url": ""}}
        }));
        // The demo-user feature never applies to an ordinary launch (Axolotl
        // never requests it), so the entry point must not produce a plan for
        // it; the empty declared URL would otherwise fail URL resolution for
        // a library that should have been skipped.
        let rules = library.library.rules.as_deref().unwrap();
        assert!(
            !crate::launcher::parse_rules(
                rules,
                std::env::consts::ARCH,
                &QuickPlayType::None,
                true,
            ),
            "the demo-user feature rule must not match an ordinary launch"
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (_temp, state) = ensure_test_state().await;
            ensure_direct_launch_dependencies(
                &state,
                &direct,
                &[library],
                &minimal_version_info(),
                std::env::consts::ARCH,
                true,
            )
            .await
            .unwrap();
        });
    }

    // -----------------------------------------------------------------------
    // Ensure stage (real IO against a local fixture server)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn missing_library_is_downloaded_into_linked_installation() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"demo-classpath-jar-bytes".to_vec();
        let path_key = "/com/example/demo/1/demo-1.jar".to_string();
        let (base, hits, server) = spawn_fixture_server(HashMap::from([(
            path_key.clone(),
            body.clone(),
        )]))
        .await;

        let library = linked_library(json!({
            "name": "com.example:demo:1",
            "downloads": {"artifact": {
                "path": "com/example/demo/1/demo-1.jar",
                "sha1": "", "size": 0,
                "url": format!("{base}{path_key}")
            }}
        }));

        ensure_direct_launch_dependencies(
            &state,
            &direct,
            &[library],
            &minimal_version_info(),
            std::env::consts::ARCH,
            true,
        )
        .await
        .unwrap();

        let destination =
            root.path().join("libraries/com/example/demo/1/demo-1.jar");
        assert_eq!(std::fs::read(&destination).unwrap(), body);
        // At least one fetch; the fetch layer may retry under load.
        assert!(
            hits.lock().unwrap().get(&path_key).copied().unwrap_or(0) >= 1,
            "the missing artifact must be downloaded"
        );
        // The download streamed to a sibling temp file and was renamed into
        // place: no partial file may remain next to the artifact.
        assert!(
            std::fs::read_dir(destination.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".part"))
        );
        server.abort();
    }

    #[tokio::test]
    async fn corrupt_library_with_known_sha1_is_replaced() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"fresh-jar-content".to_vec();
        let path_key = "/corrupt/lib-1.0.jar".to_string();
        let (base, hits, server) = spawn_fixture_server(HashMap::from([(
            path_key.clone(),
            body.clone(),
        )]))
        .await;

        let destination = root.path().join("libraries/corrupt/lib-1.0.jar");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"stale corrupt bytes").unwrap();

        let library = linked_library(json!({
            "name": "com.example:corrupt:1.0",
            "downloads": {"artifact": {
                "path": "corrupt/lib-1.0.jar",
                "sha1": sha1_hex(&body), "size": body.len() as u32,
                "url": format!("{base}{path_key}")
            }}
        }));

        ensure_direct_launch_dependencies(
            &state,
            &direct,
            &[library],
            &minimal_version_info(),
            std::env::consts::ARCH,
            true,
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), body);
        // The corrupt file must be re-fetched at least once; the fetch layer
        // may legitimately retry under load, so only a lower bound holds.
        assert!(
            hits.lock().unwrap().get(&path_key).copied().unwrap_or(0) >= 1,
            "the corrupt artifact must be re-downloaded"
        );
        server.abort();
    }

    #[tokio::test]
    async fn present_library_is_skipped_without_any_request() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"already-here";
        let destination = root.path().join("libraries/present/lib-1.0.jar");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, body).unwrap();

        let (base, hits, server) = spawn_fixture_server(HashMap::from([(
            "/present/lib-1.0.jar".to_string(),
            b"different".to_vec(),
        )]))
        .await;

        let library = linked_library(json!({
            "name": "com.example:present:1.0",
            "downloads": {"artifact": {
                "path": "present/lib-1.0.jar",
                "sha1": sha1_hex(body), "size": body.len() as u32,
                "url": format!("{base}/present/lib-1.0.jar")
            }}
        }));

        ensure_direct_launch_dependencies(
            &state,
            &direct,
            &[library],
            &minimal_version_info(),
            std::env::consts::ARCH,
            true,
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read(&destination).unwrap(),
            body,
            "the existing artifact must not be touched"
        );
        assert!(
            hits.lock().unwrap().values().sum::<usize>() == 0,
            "no download may be attempted for a current file"
        );
        server.abort();
    }

    #[tokio::test]
    async fn generated_library_must_be_supplied_locally_without_maven_requests()
    {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());
        let (base, hits, server) = spawn_fixture_server(HashMap::new()).await;
        let library = linked_library(json!({
            "name": "net.minecraftforge:forge:1.20.1-47.4.0:client",
            "downloadable": false,
            "downloads": {"artifact": {
                "path": "generated/forge-client.jar", "sha1": "", "size": 0,
                "url": format!("{base}/never-download.jar")
            }}
        }));
        let info = minimal_version_info();
        let ensure = || {
            ensure_direct_launch_dependencies(
                &state,
                &direct,
                std::slice::from_ref(&library),
                &info,
                std::env::consts::ARCH,
                true,
            )
        };
        let error = ensure().await.unwrap_err().to_string();
        assert!(error.contains(&library.library.name), "{error}");
        assert!(error.contains("forge-client.jar"), "{error}");
        let destination = direct.library_path(&library).unwrap();
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"generated locally").unwrap();
        ensure().await.unwrap();
        assert!(hits.lock().unwrap().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn failed_download_error_names_the_library_and_all_tried_urls() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        // Two servers that know nothing about any path: every request 404s.
        // The fetch layer collapses same-authority candidates into one route,
        // so each candidate URL needs its own host:port to be attempted.
        let (first_base, first_hits, first_server) =
            spawn_fixture_server(HashMap::new()).await;
        let (second_base, second_hits, second_server) =
            spawn_fixture_server(HashMap::new()).await;
        let first = format!("{first_base}/missing/first.jar");
        let second = format!("{second_base}/missing/second.jar");

        let plan = LinkedFilePlan {
            label: "net.java.jinput:jinput-platform:2.0.5".to_string(),
            urls: vec![first.clone(), second.clone()],
            destination: root.path().join("libraries/missing/first.jar"),
            sha1: None,
            size: None,
            validation: ContentValidation::None,
        };

        let error = ensure_file(&state, &plan).await.unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("net.java.jinput:jinput-platform:2.0.5"),
            "error must name the library: {message}"
        );
        assert!(
            message.contains(&first) && message.contains(&second),
            "error must list every attempted URL: {message}"
        );
        assert!(message.contains("2 attempted"), "{message}");
        // Both candidates must have been tried before giving up.
        assert!(
            first_hits
                .lock()
                .unwrap()
                .get("/missing/first.jar")
                .copied()
                .unwrap_or(0)
                >= 1,
            "the primary URL must be attempted"
        );
        assert!(
            second_hits
                .lock()
                .unwrap()
                .get("/missing/second.jar")
                .copied()
                .unwrap_or(0)
                >= 1,
            "the fallback URL must be attempted after the primary 404s"
        );
        // No artifact file may be left behind after failure (the parent
        // directory itself may exist; that is normal fetch behaviour).
        let artifact = root.path().join("libraries/missing/first.jar");
        assert!(
            !artifact.exists()
                && std::fs::read_dir(artifact.parent().unwrap())
                    .map(|entries| entries.filter_map(Result::ok).all(
                        |entry| !entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".part")
                    ))
                    .unwrap_or(true),
            "no partial artifact may be left behind after failure"
        );
        first_server.abort();
        second_server.abort();
    }

    #[tokio::test]
    async fn failed_replacement_keeps_the_existing_linked_file() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let (base, _hits, server) = spawn_fixture_server(HashMap::new()).await;
        let destination = root.path().join("libraries/existing/file.jar");
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"existing external content").unwrap();

        let plan = LinkedFilePlan {
            label: "existing external content".to_string(),
            urls: vec![format!("{base}/missing/file.jar")],
            destination: destination.clone(),
            sha1: Some(sha1_hex(b"replacement content")),
            size: Some(b"replacement content".len() as u64),
            validation: ContentValidation::None,
        };

        assert!(ensure_file(&state, &plan).await.is_err());
        assert_eq!(
            std::fs::read(destination).unwrap(),
            b"existing external content"
        );
        server.abort();
    }

    #[tokio::test]
    async fn file_without_sha1_uses_its_declared_size() {
        let root = tempdir().unwrap();
        let path = root.path().join("file.bin");
        std::fs::write(&path, b"four").unwrap();

        assert!(file_is_current(&path, None, Some(4)).await);
        assert!(!file_is_current(&path, None, Some(5)).await);
    }

    #[tokio::test]
    async fn download_falls_through_failing_candidate_urls_until_one_succeeds()
    {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"mirror-served-jar-bytes".to_vec();
        // Separate authorities per candidate (see the failure-path test): the
        // primary 404s, the second serves the artifact, the third is never
        // reached because the walk stops at the first success.
        let (missing_base, missing_hits, missing_server) =
            spawn_fixture_server(HashMap::new()).await;
        let (fallback_base, fallback_hits, fallback_server) =
            spawn_fixture_server(HashMap::from([(
                "/fallback/lib-1.0.jar".to_string(),
                body.clone(),
            )]))
            .await;
        let (never_base, never_hits, never_server) =
            spawn_fixture_server(HashMap::new()).await;

        let library = linked_library(json!({
            "name": "com.example:fallback:1.0",
            "downloads": {"artifact": {
                "path": "fallback/lib-1.0.jar",
                "sha1": "", "size": 0,
                "url": format!("{missing_base}/missing/lib-1.0.jar")
            }}
        }));
        let mut plan =
            linked_classpath_plan(&direct, &library).unwrap().unwrap();
        // Replace the production mirror tail with in-process fixture URLs so
        // the candidate walk never leaves localhost.
        plan.urls = vec![
            format!("{missing_base}/missing/lib-1.0.jar"),
            format!("{fallback_base}/fallback/lib-1.0.jar"),
            format!("{never_base}/never/lib-1.0.jar"),
        ];

        let downloaded = ensure_file(&state, &plan).await.unwrap();

        assert!(downloaded, "the missing artifact must be downloaded");
        let destination = root.path().join("libraries/fallback/lib-1.0.jar");
        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert!(
            missing_hits
                .lock()
                .unwrap()
                .get("/missing/lib-1.0.jar")
                .copied()
                .unwrap_or(0)
                >= 1,
            "the failing primary URL must be attempted"
        );
        assert!(
            fallback_hits
                .lock()
                .unwrap()
                .get("/fallback/lib-1.0.jar")
                .copied()
                .unwrap_or(0)
                >= 1,
            "the working candidate must serve the download"
        );
        assert!(
            never_hits.lock().unwrap().is_empty(),
            "the walk must stop at the first successful candidate"
        );
        missing_server.abort();
        fallback_server.abort();
        never_server.abort();
    }

    #[tokio::test]
    async fn missing_asset_index_and_objects_are_completed_in_place() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let missing_bytes = b"brand new asset bytes";
        let present_bytes = b"already downloaded asset";
        let hash_present = sha1_hex(present_bytes);
        let hash_missing = sha1_hex(missing_bytes);
        let index_body = serde_json::to_vec(&json!({
            "objects": {
                "minecraft/sounds/click.ogg": {"hash": hash_missing, "size": missing_bytes.len()},
                "minecraft/textures/present.png": {"hash": hash_present, "size": present_bytes.len()}
            }
        }))
        .unwrap();

        let (base, hits, server) = spawn_fixture_server(HashMap::from([
            ("/indexes/5.json".to_string(), index_body.clone()),
            (
                format!("/{}/{}", &hash_missing[..2], hash_missing),
                missing_bytes.to_vec(),
            ),
        ]))
        .await;

        // Pre-seed one object so only the other must be fetched.
        let present_path = direct
            .assets_dir()
            .join("objects")
            .join(&hash_present[..2])
            .join(&hash_present);
        std::fs::create_dir_all(present_path.parent().unwrap()).unwrap();
        std::fs::write(&present_path, present_bytes).unwrap();

        let asset_index = AssetIndex {
            id: "5".to_string(),
            sha1: sha1_hex(&index_body),
            size: index_body.len() as u32,
            total_size: 43,
            url: format!("{base}/indexes/5.json"),
        };

        ensure_linked_assets_from(&state, &direct, &asset_index, &base, true)
            .await
            .unwrap();

        let index_file = root.path().join("assets/indexes/5.json");
        assert_eq!(std::fs::read(&index_file).unwrap(), index_body);
        assert_eq!(
            std::fs::read(
                root.path()
                    .join("assets/objects")
                    .join(&hash_missing[..2])
                    .join(&hash_missing)
            )
            .unwrap(),
            b"brand new asset bytes"
        );
        assert_eq!(
            std::fs::read(
                root.path()
                    .join("assets/objects")
                    .join(&hash_present[..2])
                    .join(&hash_present)
            )
            .unwrap(),
            b"already downloaded asset"
        );
        assert_eq!(
            std::fs::read(
                root.path()
                    .join("assets")
                    .join("virtual")
                    .join("legacy")
                    .join("minecraft")
                    .join("sounds")
                    .join("click.ogg")
            )
            .unwrap(),
            missing_bytes
        );
        // A second ensure has no missing objects, but must still materialize
        // the legacy virtual tree from the already-present object store.
        let legacy_click = root
            .path()
            .join("assets")
            .join("virtual")
            .join("legacy")
            .join("minecraft")
            .join("sounds")
            .join("click.ogg");
        std::fs::remove_file(&legacy_click).unwrap();
        ensure_linked_assets_from(&state, &direct, &asset_index, &base, true)
            .await
            .unwrap();
        assert_eq!(std::fs::read(legacy_click).unwrap(), missing_bytes);
        let hits = hits.lock().unwrap();
        // The pre-seeded object must never be requested; the missing one must
        // be fetched at least once (the fetch layer may retry under load).
        assert!(
            !hits.contains_key(&format!(
                "/{}/{}",
                &hash_present[..2],
                hash_present
            )),
            "a present asset object may not be re-downloaded"
        );
        assert!(
            hits.get(&format!("/{}/{}", &hash_missing[..2], hash_missing))
                .copied()
                .unwrap_or(0)
                >= 1,
            "the missing asset object must be downloaded"
        );
        server.abort();
    }

    #[tokio::test]
    async fn missing_log_config_is_downloaded_into_log_configs() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"<Configuration/>".to_vec();
        let (base, _hits, server) = spawn_fixture_server(HashMap::from([(
            "/client-1.12.xml".to_string(),
            body.clone(),
        )]))
        .await;

        let logging = serde_json::from_value::<
            HashMap<LoggingSide, LoggingConfiguration>,
        >(json!({
            "client": {
                "argument": "-Dlog4j.configurationFile=${path}",
                "file": {"id": "client-1.12.xml", "sha1": sha1_hex(&body), "size": body.len() as u32, "url": format!("{base}/client-1.12.xml")},
                "type": "log4j2-xml"
            }
        }))
        .unwrap();

        ensure_linked_log_config(&state, &direct, Some(&logging))
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(
                root.path().join("assets/log_configs/client-1.12.xml")
            )
            .unwrap(),
            body
        );
        server.abort();
    }

    #[tokio::test]
    async fn download_walks_404_sources_until_maven_central_succeeds() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let body = b"jinput-platform-2.0.5-bytes".to_vec();
        // Candidate order mirrors the production chain: declared source,
        // BMCLAPI, Mojang Maven, then Maven Central serving the artifact.
        // Each candidate needs its own authority so the fetch layer
        // attempts it separately.
        let (declared_base, declared_hits, declared_server) =
            spawn_fixture_server(HashMap::new()).await;
        let (bmclapi_base, bmclapi_hits, bmclapi_server) =
            spawn_fixture_server(HashMap::new()).await;
        let (mojang_base, mojang_hits, mojang_server) =
            spawn_fixture_server(HashMap::new()).await;
        let central_path =
            "/net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar"
                .to_string();
        let (central_base, central_hits, central_server) =
            spawn_fixture_server(HashMap::from([(
                central_path.clone(),
                body.clone(),
            )]))
            .await;
        let declared_path = "/declared/missing.jar";
        let bmclapi_path = "/bmclapi/missing.jar";
        let mojang_path = "/mojang/missing.jar";

        let library = linked_library(json!({
            "name": "net.java.jinput:jinput-platform:2.0.5",
            "downloads": {"artifact": {
                "path": "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar",
                "sha1": "", "size": 0,
                "url": format!("{declared_base}{declared_path}")
            }}
        }));
        let mut plan =
            linked_classpath_plan(&direct, &library).unwrap().unwrap();
        assert_eq!(
            plan.urls.len(),
            4,
            "candidate chain must be declared + BMCLAPI + Mojang + Maven Central"
        );
        // Replace the production mirror tail with in-process fixture URLs so
        // the candidate walk never leaves localhost.
        plan.urls = vec![
            format!("{declared_base}{declared_path}"),
            format!("{bmclapi_base}{bmclapi_path}"),
            format!("{mojang_base}{mojang_path}"),
            format!("{central_base}{central_path}"),
        ];

        let downloaded = ensure_file(&state, &plan).await.unwrap();

        assert!(downloaded, "the missing artifact must be downloaded");
        let destination = root.path().join(
            "libraries/net/java/jinput/jinput-platform/2.0.5/\
             jinput-platform-2.0.5.jar",
        );
        assert_eq!(std::fs::read(&destination).unwrap(), body);
        // The three failing sources must all have been tried before the
        // Maven Central candidate served the artifact.
        for (hits, path) in [
            (&declared_hits, declared_path),
            (&bmclapi_hits, bmclapi_path),
            (&mojang_hits, mojang_path),
        ] {
            assert!(
                hits.lock().unwrap().get(path).copied().unwrap_or(0) >= 1,
                "the failing {path} candidate must be attempted"
            );
        }
        assert!(
            central_hits
                .lock()
                .unwrap()
                .get(&central_path)
                .copied()
                .unwrap_or(0)
                >= 1,
            "the Maven Central candidate must serve the artifact"
        );
        declared_server.abort();
        bmclapi_server.abort();
        mojang_server.abort();
        central_server.abort();
    }

    #[tokio::test]
    async fn total_failure_error_lists_the_full_four_source_chain() {
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        // Four servers that know nothing about any path: every request 404s.
        // The fetch layer collapses same-authority candidates into one route,
        // so each candidate URL needs its own host:port to be attempted.
        let mut servers = Vec::new();
        let mut urls = Vec::new();
        for index in 0..4 {
            let (base, _hits, server) =
                spawn_fixture_server(HashMap::new()).await;
            urls.push(format!("{base}/missing/source-{index}.jar"));
            servers.push(server);
        }

        let plan = LinkedFilePlan {
            label: "net.java.jinput:jinput-platform:2.0.5".to_string(),
            urls,
            destination: root.path().join("libraries/missing/source-0.jar"),
            sha1: None,
            size: None,
            validation: ContentValidation::None,
        };

        let error = ensure_file(&state, &plan).await.unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("net.java.jinput:jinput-platform:2.0.5"),
            "error must name the library: {message}"
        );
        for url in &plan.urls {
            assert!(
                message.contains(url),
                "error must list every attempted URL: {message}"
            );
        }
        assert!(
            message.contains("4 attempted"),
            "error must count all four sources: {message}"
        );
        for server in servers {
            server.abort();
        }
    }

    #[tokio::test]
    async fn natives_only_library_is_ensured_as_classifier_and_never_classpathed()
     {
        use std::io::Write as _;
        let (_state_temp, state) = ensure_test_state().await;
        let root = tempdir().unwrap();
        let direct = direct_for(root.path());

        let natives_os =
            serde_json::to_value(daedalus::minecraft::Os::native().get_os())
                .unwrap();
        let classifier = format!("natives-{}", natives_os.as_str().unwrap());
        // A tiny native archive served for the platform classifier; the plain
        // jar of this library exists in no repository, so it is never served.
        let mut archive = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut archive);
            zip.start_file(
                "libjinput.so",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"natives").unwrap();
            zip.finish().unwrap();
        }
        let body = archive.into_inner();

        let plain_relative =
            "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5.jar";
        let classifier_path = format!(
            "/net/java/jinput/jinput-platform/2.0.5/\
             jinput-platform-2.0.5-{classifier}.jar"
        );
        let (base, hits, server) = spawn_fixture_server(HashMap::from([(
            classifier_path,
            body.clone(),
        )]))
        .await;

        let mut classifiers = serde_json::Map::new();
        for os in ["linux", "osx", "windows"] {
            let classifier = format!("natives-{os}");
            let relative = format!(
                "net/java/jinput/jinput-platform/2.0.5/\
                 jinput-platform-2.0.5-{classifier}.jar"
            );
            classifiers.insert(
                classifier.clone(),
                json!({
                    "path": relative,
                    "sha1": sha1_hex(&body),
                    "size": body.len(),
                    "url": format!("{base}/{relative}"),
                }),
            );
        }

        let library = linked_library(json!({
            "name": "net.java.jinput:jinput-platform:2.0.5",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"},
            "downloads": {"classifiers": classifiers}
        }));

        ensure_direct_launch_dependencies(
            &state,
            &direct,
            std::slice::from_ref(&library),
            &minimal_version_info(),
            std::env::consts::ARCH,
            true,
        )
        .await
        .unwrap();

        let libraries_dir = root.path().join("libraries");
        let plain_jar = libraries_dir.join(plain_relative);
        assert!(
            !plain_jar.exists(),
            "no plain jar may be fetched for a natives-only library"
        );
        assert!(
            !hits
                .lock()
                .unwrap()
                .contains_key(&format!("/{plain_relative}")),
            "the nonexistent plain jar must never be requested"
        );
        let classifier_jar = libraries_dir
            .join("net/java/jinput/jinput-platform/2.0.5")
            .join(format!("jinput-platform-2.0.5-{classifier}.jar"));
        assert_eq!(
            std::fs::read(&classifier_jar).unwrap(),
            body,
            "the platform classifier jar must be downloaded"
        );

        // Native extraction consumes the ensured classifier jar, like on any
        // normal launch.
        let target = root.path().join("axolotl-cache");
        extract_linked_natives(
            &direct,
            std::slice::from_ref(&library),
            &target,
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert!(target.join("libjinput.so").is_file());

        // The classpath assembles without the natives-only library: its plain
        // jar neither exists nor belongs there (HMCL getClasspath semantics).
        let launcher_jar = root.path().join("main.jar");
        std::fs::write(&launcher_jar, b"main").unwrap();
        let classpath = super::super::args::get_linked_class_paths(
            &direct,
            std::slice::from_ref(&library),
            &[&launcher_jar],
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert!(
            !classpath.contains("jinput-platform"),
            "the natives-only library must not appear on the classpath: \
             {classpath}"
        );
        assert!(
            classpath.contains(
                dunce::canonicalize(&launcher_jar)
                    .unwrap()
                    .to_string_lossy()
                    .as_ref()
            )
        );
        server.abort();
    }
}
