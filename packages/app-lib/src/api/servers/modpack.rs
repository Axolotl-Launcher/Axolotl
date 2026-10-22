//! Modpack server installation: materializing an `.mrpack` (files, overrides,
//! and the server launcher) into a managed server directory.
//!
//! The flow mirrors what a modpack client install does, but lands in the
//! dedicated servers folder: the archive is downloaded and unpacked, every file
//! listed in `modrinth.index.json` is fetched to its target path, client and
//! server overrides are applied, and the loader's server launcher jar is placed
//! next to them so the regular `servers.start` flow can boot it.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::stream::{self, StreamExt};
use serde::Deserialize;

use crate::State;
use crate::api::pack::archive_util::{
    extract_archive_subdir, read_archive_entry_to_string,
};
use crate::api::pack::detect::decode_zip_entry_name;
use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::event::emit::loading_try_for_each_concurrent;
use crate::util::fetch::{
    DownloadRequest, FetchProgressFn, Integrity, ResourceClass,
    download_to_path,
};
use crate::util::io::IOError;
use crate::{ErrorKind, Result};

use super::files::{download_to_dir, safe_relative_join};
use super::manifest::{
    InstallState, ModpackInfo, read_manifest, server_path, write_manifest,
};

const MRPACK_MANIFEST_ENTRY: &str = "modrinth.index.json";
const MRPACK_FILENAME: &str = "pack.mrpack";
const OVERRIDES_DIR: &str = "overrides";
const SERVER_OVERRIDES_DIR: &str = "server/overrides";
/// Top-level override folders that only a Minecraft client reads. Modpacks
/// ship them for players; a dedicated server never loads them, so they are
/// dropped right after overrides are extracted.
const CLIENT_ONLY_OVERRIDE_DIRS: &[&str] = &["shaderpacks"];
/// Index file path prefixes that only a Minecraft client ever reads,
/// following ATLauncher's classification of Modrinth pack files by location.
/// Datapacks are deliberately absent because servers load those.
const CLIENT_ONLY_INDEX_PREFIXES: &[&str] =
    &["shaderpacks/", "resourcepacks/", "texturepacks/"];
/// How many modpack files download at once. Bounded by the global download
/// semaphore, which also leaves headroom for concurrent instance installs.
const MODPACK_DOWNLOAD_CONCURRENCY: usize = 8;

/// Shared aggregate progress across concurrent file downloads, rendered as one
/// smooth bar: completed bytes plus the in-flight file's partial bytes, kept
/// monotonic so the UI never regresses.
#[derive(Clone)]
struct AggregateProgress {
    bytes_done: Arc<AtomicU64>,
    reported: Arc<AtomicU64>,
    total: u64,
}

impl AggregateProgress {
    fn new(total: u64) -> Self {
        Self {
            bytes_done: Arc::new(AtomicU64::new(0)),
            reported: Arc::new(AtomicU64::new(0)),
            total,
        }
    }
}

/// `modrinth.index.json` document; only the fields the server installer needs
/// are modeled. `env` marks files that are client-only, server-only, or both.
#[derive(Deserialize)]
struct MrpackIndex {
    #[serde(default)]
    files: Vec<MrpackFile>,
}

#[derive(Deserialize, Clone)]
struct MrpackFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<MrpackEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default, rename = "fileSize")]
    file_size: Option<u64>,
}

impl MrpackFile {
    fn integrity(&self) -> Result<Integrity> {
        let sha1 = self.hashes.get("sha1").cloned();
        let sha512 = self.hashes.get("sha512").cloned();
        if sha1.is_none() && sha512.is_none() {
            return Err(ErrorKind::InputError(format!(
                "Modpack file {} is missing a SHA-1 or SHA-512 checksum",
                self.path
            ))
            .as_error());
        }
        for (hash, length) in [(&sha1, 40), (&sha512, 128)] {
            if let Some(hash) = hash
                && (hash.len() != length
                    || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            {
                return Err(ErrorKind::InputError(format!(
                    "Modpack file {} has an invalid checksum",
                    self.path
                ))
                .as_error());
            }
        }
        Ok(Integrity {
            sha1,
            sha512,
            size: self.file_size,
            ..Integrity::default()
        })
    }
}

/// Validate the entire index before starting concurrent downloads, including
/// entries excluded from server installation but fetched for metadata.
fn validate_index(index: &MrpackIndex, dir: &Path) -> Result<()> {
    for file in &index.files {
        safe_relative_join(dir, &file.path)?;
        file.integrity()?;
    }
    Ok(())
}

#[derive(Deserialize, Clone)]
struct MrpackEnv {
    #[serde(default)]
    server: Option<String>,
}

/// Fabric metadata embedded in a mod jar. Only identity and dependency
/// information are modeled; this is the same signal Fabric Loader uses to
/// resolve mods at runtime.
#[derive(Clone, Deserialize)]
struct ModMetadata {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    environment: Option<String>,
    #[serde(default)]
    provides: Vec<String>,
    // Modeled as raw JSON: most mods use an object keyed by mod id, but
    // non-standard shapes (plain lists) exist and must not fail parsing.
    #[serde(default)]
    depends: Option<serde_json::Value>,
}

impl ModMetadata {
    /// Upper bound on how many bytes of `fabric.mod.json` are read, so a
    /// bloated or malicious entry cannot balloon memory.
    const MAX_READ_BYTES: u64 = 1024 * 1024;

    fn owned_ids(&self) -> impl Iterator<Item = &str> {
        self.id
            .iter()
            .map(String::as_str)
            .chain(self.provides.iter().map(String::as_str))
    }

    fn hard_dependencies(&self) -> Vec<&str> {
        match self.depends.as_ref().and_then(|value| value.as_object()) {
            Some(dependencies) => dependencies
                .iter()
                .filter_map(|(id, spec)| {
                    let optional = spec
                        .as_object()
                        .and_then(|object| object.get("optional"))
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    (!optional).then_some(id.as_str())
                })
                .collect(),
            None => Vec::new(),
        }
    }
}

/// Installs a modpack into an existing managed server.
///
/// `mrpack_url` / `mrpack_sha1` identify the `.mrpack` archive to download;
/// `jar_url` / `jar_filename` / `jar_sha1` describe the loader's server
/// launcher jar that boots the modpack. The optional `modpack_*` fields are
/// recorded on the manifest so the UI can badge and link the server back to
/// its source project.
///
/// The manifest tracks the install while it runs: it is flagged `incomplete`
/// up front (so an interrupted download is resumable from the servers list),
/// cleared on success alongside a best-effort modpack icon fetch, and set to
/// `failed` with the error message when the install errors out.
#[allow(clippy::too_many_arguments)]
pub async fn install_modpack(
    server_id: &str,
    mrpack_url: &str,
    mrpack_sha1: Option<String>,
    jar_url: &str,
    jar_filename: &str,
    jar_sha1: Option<String>,
    modpack_project_id: Option<String>,
    modpack_version_id: Option<String>,
    modpack_title: Option<String>,
    modpack_icon_url: Option<String>,
) -> Result<()> {
    let dir = server_path(server_id).await?;
    let mut manifest = read_manifest(&dir).await?;
    manifest.install_state = Some(InstallState::Incomplete);
    manifest.install_error = None;
    // Record the source pack before any bytes move so an interrupted install
    // still carries everything the resume flow needs.
    if let (Some(project_id), Some(version_id), Some(title)) =
        (&modpack_project_id, &modpack_version_id, &modpack_title)
    {
        manifest.modpack = Some(ModpackInfo {
            project_id: project_id.clone(),
            version_id: version_id.clone(),
            title: title.clone(),
            icon_url: modpack_icon_url.clone(),
        });
    }
    write_manifest(&dir, &manifest).await?;
    drop(manifest);

    // Set the icon path early so the server shows the modpack icon immediately
    if let Some(icon_url) = modpack_icon_url.as_deref()
        && let Ok(icon_path) = download_icon(&dir, icon_url).await
    {
        let mut manifest = read_manifest(&dir).await?;
        manifest.icon_path = Some(icon_path.to_string_lossy().into_owned());
        write_manifest(&dir, &manifest).await?;
    }

    let result = run_modpack_install(
        server_id,
        &dir,
        mrpack_url,
        mrpack_sha1.as_deref(),
        jar_url,
        jar_filename,
        jar_sha1.as_deref(),
    )
    .await;

    let mut manifest = read_manifest(&dir).await?;
    match result {
        Ok(()) => {
            manifest.jar_name = Some(jar_filename.to_string());
            // Icon was already downloaded at the start; only download if still missing
            if manifest.icon_path.is_none()
                && let Some(icon_url) = modpack_icon_url.as_deref()
            {
                match download_icon(&dir, icon_url).await {
                    Ok(icon_path) => {
                        manifest.icon_path =
                            Some(icon_path.to_string_lossy().into_owned());
                    }
                    Err(error) => {
                        log(
                            server_id,
                            &format!(
                                "Failed to download modpack icon: {error}"
                            ),
                        )
                        .await
                        .ok();
                    }
                }
            }
            manifest.install_state = None;
            manifest.install_error = None;
            write_manifest(&dir, &manifest).await?;
            Ok(())
        }
        Err(error) => {
            manifest.install_state = Some(InstallState::Failed);
            manifest.install_error = Some(error.to_string());
            write_manifest(&dir, &manifest).await?;
            Err(error)
        }
    }
}

/// Downloads and unpacks the modpack contents. Manifest bookkeeping lives in
/// [`install_modpack`] so this function stays focused on file transfer.
#[allow(clippy::too_many_arguments)]
async fn run_modpack_install(
    server_id: &str,
    dir: &Path,
    mrpack_url: &str,
    mrpack_sha1: Option<&str>,
    jar_url: &str,
    jar_filename: &str,
    jar_sha1: Option<&str>,
) -> Result<()> {
    let state = State::get().await?;

    log(server_id, "Downloading modpack archive").await?;
    download_with_engine(
        server_id,
        &state,
        mrpack_url,
        &dir.join(MRPACK_FILENAME),
        mrpack_sha1.map(Integrity::sha1).unwrap_or_default(),
        ResourceClass::Modpack,
        AggregateProgress::new(0),
    )
    .await?;
    let archive_path = dir.join(MRPACK_FILENAME);

    let manifest_entry = find_manifest_entry(&archive_path).await?;
    let base_folder = base_folder(&manifest_entry);
    let index = parse_index(&archive_path, &manifest_entry).await?;
    validate_index(&index, dir)?;

    let (installable_files, excluded_files): (
        Vec<&MrpackFile>,
        Vec<&MrpackFile>,
    ) = index
        .files
        .iter()
        .partition(|file| is_server_installable(file));
    let total_bytes: u64 = installable_files
        .iter()
        .filter_map(|file| file.file_size)
        .sum();

    let skipped = index.files.len() - installable_files.len();
    if skipped > 0 {
        log(
            server_id,
            &format!("Skipping {skipped} client-only modpack file(s)"),
        )
        .await?;
    }

    log(
        server_id,
        &format!("Downloading {} modpack file(s)", installable_files.len()),
    )
    .await?;

    let files: Vec<(String, String, Integrity, u64)> = installable_files
        .iter()
        .filter_map(|file| {
            file.downloads.first().map(|url| {
                Ok((
                    file.path.clone(),
                    url.clone(),
                    file.integrity()?,
                    file.file_size.unwrap_or(0),
                ))
            })
        })
        .collect::<Result<_>>()?;

    let progress = AggregateProgress::new(total_bytes);
    let state_ref = state.clone();
    let dir_ref = dir.to_path_buf();
    let server_id_ref = server_id.to_string();
    let num_files = files.len();
    loading_try_for_each_concurrent(
        stream::iter(files).map(Ok::<_, crate::Error>),
        Some(MODPACK_DOWNLOAD_CONCURRENCY),
        None,
        0.0,
        num_files,
        None,
        move |(path, url, integrity, size)| {
            let server_id = server_id_ref.clone();
            let dir = dir_ref.clone();
            let state = state_ref.clone();
            let progress = progress.clone();
            async move {
                log(&server_id, &format!("Downloading {path}")).await?;
                let destination = safe_relative_join(&dir, &path)?;
                if let Some(parent) = destination.parent() {
                    crate::util::io::create_dir_all(parent).await?;
                }
                download_with_engine(
                    &server_id,
                    &state,
                    &url,
                    &destination,
                    integrity,
                    ResourceClass::Modpack,
                    progress.clone(),
                )
                .await?;
                progress.bytes_done.fetch_add(size, Ordering::Relaxed);
                Ok(())
            }
        },
    )
    .await?;

    log(server_id, "Applying modpack overrides").await?;
    extract_archive_subdir(
        archive_path.clone(),
        format!("{base_folder}{OVERRIDES_DIR}/"),
        dir.to_path_buf(),
    )
    .await?;
    extract_archive_subdir(
        archive_path,
        format!("{base_folder}{SERVER_OVERRIDES_DIR}/"),
        dir.to_path_buf(),
    )
    .await?;
    remove_client_only_dirs(server_id, dir).await?;
    let unavailable_ids =
        fetch_excluded_mod_ids(server_id, &state, &excluded_files).await?;

    // Build a set of mod IDs that are explicitly marked as server-installable in the modpack index
    let explicitly_server_installable: HashSet<String> = installable_files
        .iter()
        .filter_map(|file| {
            let path = file.path.replace('\\', "/");
            if path.starts_with("mods/")
                && path.to_ascii_lowercase().ends_with(".jar")
            {
                Some(path.rsplit('/').next().unwrap_or(&path).to_string())
            } else {
                None
            }
        })
        .collect();

    prune_uninstallable_mods(
        server_id,
        dir,
        &unavailable_ids,
        &explicitly_server_installable,
    )
    .await?;

    log(
        server_id,
        &format!("Downloading server launcher ({jar_filename})"),
    )
    .await?;

    // Create eula.txt with eula=false if it doesn't exist
    let eula_path = dir.join("eula.txt");
    if !eula_path.exists() {
        tokio::fs::write(&eula_path, "eula=false\n")
            .await
            .map_err(|e| IOError::with_path(e, &eula_path))?;
        log(server_id, "Created eula.txt with eula=false")
            .await
            .ok();
    }
    download_to_dir(
        server_id,
        dir,
        jar_url,
        jar_filename,
        jar_sha1.map(str::to_string),
    )
    .await?;

    Ok(())
}

/// Fetches the modpack icon into the server directory and returns its path.
async fn download_icon(dir: &Path, icon_url: &str) -> Result<PathBuf> {
    let normalized =
        crate::util::download::retired_sources::normalize(icon_url)?;
    let icon_url = normalized.as_ref();
    let extension = icon_url
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('.').next())
        .map(str::to_ascii_lowercase)
        .filter(|ext| {
            matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif"
            )
        })
        .unwrap_or_else(|| "png".to_string());
    let destination = safe_relative_join(dir, &format!("icon.{extension}"))?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(crate::launcher_user_agent())
        .build()
        .map_err(|e| ErrorKind::NetworkError(e.to_string()))?;
    let response = crate::util::download::native_request::send_cdn_request(
        client.get(icon_url),
    )
    .await?
    .error_for_status()?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ErrorKind::NetworkError(e.to_string()))?;
    tokio::fs::write(&destination, &bytes)
        .await
        .map_err(|e| IOError::with_path(e, &destination))?;
    Ok(destination)
}

/// Downloads a single file through the shared launcher download engine
/// (mirrors, retries, integrity, range-segmented multi-connection transfer for
/// large files, background-friendly concurrency) instead of a bespoke HTTP
/// client. Progress is reported as server events through the shared aggregate.
async fn download_with_engine(
    server_id: &str,
    state: &State,
    url: &str,
    destination: &Path,
    integrity: Integrity,
    resource: ResourceClass,
    progress: AggregateProgress,
) -> Result<()> {
    let request = DownloadRequest::new(url, resource)
        .with_integrity(integrity)
        .with_segmented_download(true);

    let server_id = server_id.to_string();
    let progress = progress.clone();
    let mut progress_fn: Box<FetchProgressFn<'_>> = Box::new(
        move |downloaded: u64,
              file_total: u64|
              -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            let server_id = server_id.clone();
            let progress = progress.clone();
            Box::pin(async move {
                let current = progress
                    .bytes_done
                    .load(Ordering::Relaxed)
                    .saturating_add(downloaded);
                let shown = progress
                    .reported
                    .fetch_max(current, Ordering::Relaxed)
                    .max(current);
                emit_server(
                    &server_id,
                    ServerPayloadType::DownloadProgress {
                        downloaded: shown,
                        total: if progress.total > 0 {
                            Some(progress.total)
                        } else if file_total > 0 {
                            Some(file_total)
                        } else {
                            None
                        },
                    },
                )
                .await
                .ok();
                Ok(())
            })
        },
    );

    download_to_path(
        request,
        destination,
        &state.download_semaphore,
        &state.pool,
        Some(progress_fn.as_mut()),
    )
    .await?;
    Ok(())
}

fn is_server_installable(file: &MrpackFile) -> bool {
    if file.env.as_ref().and_then(|env| env.server.as_deref())
        == Some("unsupported")
    {
        return false;
    }
    let path = file.path.replace('\\', "/");
    !CLIENT_ONLY_INDEX_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// Removes client-only folders (see [`CLIENT_ONLY_OVERRIDE_DIRS`]) that came
/// along with the modpack's overrides; the `env` filtering applied to files
/// listed in `modrinth.index.json` does not cover override contents.
async fn remove_client_only_dirs(server_id: &str, dir: &Path) -> Result<()> {
    for folder in CLIENT_ONLY_OVERRIDE_DIRS {
        let path = dir.join(folder);
        if !path.is_dir() {
            continue;
        }
        tokio::fs::remove_dir_all(&path)
            .await
            .map_err(|e| IOError::with_path(e, &path))?;
        log(
            server_id,
            &format!("Removed client-only {folder} folder from overrides"),
        )
        .await
        .ok();
    }
    Ok(())
}

/// Temporarily downloads the client-side mods excluded from the server
/// install, purely to learn their mod IDs from embedded metadata so installed
/// mods hard-depending on them can be excluded too (pack authors sometimes
/// mark a client UI mod as required on both sides while marking its
/// client-side dependency as unsupported). Failures are logged and skipped:
/// an unreadable mod simply keeps its dependents.
async fn fetch_excluded_mod_ids(
    server_id: &str,
    state: &State,
    excluded_files: &[&MrpackFile],
) -> Result<HashSet<String>> {
    let candidates: Vec<(String, String, Integrity)> = excluded_files
        .iter()
        .filter_map(|file| {
            let path = file.path.replace('\\', "/");
            if !path.starts_with("mods/")
                || !path.to_ascii_lowercase().ends_with(".jar")
            {
                return None;
            }
            file.downloads.first().map(|url| {
                Ok((
                    path.rsplit('/').next().unwrap_or(&path).to_string(),
                    url.clone(),
                    file.integrity()?,
                ))
            })
        })
        .collect::<Result<_>>()?;
    if candidates.is_empty() {
        return Ok(HashSet::new());
    }

    log(
        server_id,
        &format!("Inspecting {} client-side mod(s)", candidates.len()),
    )
    .await?;

    let temp_dir = tempfile::tempdir().map_err(|e| {
        ErrorKind::FSError(format!("Failed to create temporary directory: {e}"))
    })?;
    let mut unavailable = HashSet::new();
    for (filename, url, integrity) in candidates {
        let destination = safe_relative_join(temp_dir.path(), &filename)?;
        match download_metadata_jar(state, &url, &destination, integrity).await
        {
            Ok(()) => match read_mod_metadata(&destination) {
                Some(metadata) => {
                    unavailable
                        .extend(metadata.owned_ids().map(str::to_string));
                }
                None => {
                    log(
						server_id,
						&format!("Could not read metadata of client-side mod {filename}"),
					)
					.await
					.ok();
                }
            },
            Err(error) => {
                log(
					server_id,
					&format!("Failed to download client-side mod {filename} for inspection: {error}"),
				)
				.await
				.ok();
            }
        }
    }
    Ok(unavailable)
}

/// Downloads a single jar for metadata inspection without touching the
/// server's progress reporting.
async fn download_metadata_jar(
    state: &State,
    url: &str,
    destination: &Path,
    integrity: Integrity,
) -> Result<()> {
    let request = DownloadRequest::new(url, ResourceClass::Modpack)
        .with_integrity(integrity);
    download_to_path(
        request,
        destination,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await?;
    Ok(())
}

/// Removes mods that cannot run on a dedicated server: those declaring
/// themselves client-only in their Fabric metadata, and transitively any mod
/// whose hard dependencies point to a removed or excluded mod. Left in place,
/// such mods crash the server during dependency resolution.
///
/// Mods that are explicitly marked as server-installable in the modpack index
/// (via `env.server != "unsupported"`) are never pruned, even if their own
/// metadata declares them as client-only. This respects the pack author's intent.
async fn prune_uninstallable_mods(
    server_id: &str,
    dir: &Path,
    unavailable_ids: &HashSet<String>,
    explicitly_server_installable: &HashSet<String>,
) -> Result<()> {
    let mods_dir = dir.join("mods");
    if !mods_dir.is_dir() {
        return Ok(());
    }

    let metas = collect_mod_metadata(mods_dir).await?;
    for (path, reason) in compute_prune_plan(
        &metas,
        unavailable_ids,
        explicitly_server_installable,
    ) {
        log(
            server_id,
            &format!(
                "Removed mod {} ({reason})",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
        )
        .await
        .ok();
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| IOError::with_path(e, &path))?;
    }
    Ok(())
}

/// Walks the mods directory recursively and reads each jar's embedded Fabric
/// metadata. Jars without readable metadata are skipped entirely: only an
/// explicit declaration justifies a removal.
async fn collect_mod_metadata(
    mods_dir: PathBuf,
) -> Result<Vec<(PathBuf, ModMetadata)>> {
    tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        let mut pending = vec![mods_dir];
        while let Some(current) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
                    && let Some(metadata) = read_mod_metadata(&path)
                {
                    found.push((path, metadata));
                }
            }
        }
        found
    })
    .await
    .map_err(|e| {
        ErrorKind::FSError(format!("Failed to scan installed mods: {e}"))
            .as_error()
    })
}

/// Reads a jar's embedded `fabric.mod.json`.
fn read_mod_metadata(path: &Path) -> Option<ModMetadata> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name("fabric.mod.json").ok()?;
    let mut contents = Vec::new();
    entry
        .by_ref()
        .take(ModMetadata::MAX_READ_BYTES)
        .read_to_end(&mut contents)
        .ok()?;
    serde_json::from_slice(&contents).ok()
}

/// Plans which installed mods must be removed. Seeds are mods declaring
/// themselves client-only plus every excluded mod's ID that no local mod
/// owns; removal then cascades through hard `depends` edges until stable.
/// Returns paths with a human-readable reason for each removal.
fn compute_prune_plan(
    metas: &[(PathBuf, ModMetadata)],
    unavailable_ids: &HashSet<String>,
    explicitly_server_installable: &HashSet<String>,
) -> Vec<(PathBuf, String)> {
    let locally_owned: HashSet<&str> = metas
        .iter()
        .flat_map(|(_, metadata)| metadata.owned_ids())
        .collect();
    let mut removed: HashSet<String> = unavailable_ids
        .iter()
        .filter(|id| !locally_owned.contains(id.as_str()))
        .cloned()
        .collect();

    let mut planned = vec![false; metas.len()];
    let mut reasons: Vec<Option<String>> = vec![None; metas.len()];
    for (index, (path, metadata)) in metas.iter().enumerate() {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        // Skip pruning if this mod is explicitly marked as server-installable in the modpack index
        let is_explicitly_allowed = metadata
            .id
            .as_ref()
            .map(|id| explicitly_server_installable.contains(id))
            .unwrap_or(false)
            || explicitly_server_installable.contains(&filename.to_string());
        if is_explicitly_allowed {
            continue;
        }
        if metadata.environment.as_deref() == Some("client") {
            planned[index] = true;
            reasons[index] = Some("client-only".to_string());
        }
    }

    loop {
        let mut changed = false;
        for index in 0..metas.len() {
            if planned[index] {
                continue;
            }
            let missing = metas[index]
                .1
                .hard_dependencies()
                .into_iter()
                .find(|dependency| removed.contains(*dependency));
            if let Some(missing) = missing {
                planned[index] = true;
                reasons[index] = Some(format!("requires {missing}"));
                changed = true;
                for id in metas[index].1.owned_ids() {
                    removed.insert(id.to_string());
                }
            }
        }
        if !changed {
            break;
        }
    }

    metas
        .iter()
        .enumerate()
        .filter(|(index, _)| planned[*index])
        .map(|(index, (path, _))| {
            (
                path.clone(),
                reasons[index]
                    .clone()
                    .unwrap_or_else(|| "unusable on servers".to_string()),
            )
        })
        .collect()
}

/// Locates the `modrinth.index.json` entry inside the archive, tolerating packs
/// whose contents are nested under a single base folder.
async fn find_manifest_entry(archive_path: &Path) -> Result<String> {
    let archive_path = archive_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path)
            .map_err(|error| IOError::with_path(error, &archive_path))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| {
            ErrorKind::InputError(format!(
                "Modpack archive is invalid: {error}"
            ))
            .as_error()
        })?;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index).map_err(|error| {
                ErrorKind::InputError(format!(
                    "Failed to read modpack archive entry: {error}"
                ))
                .as_error()
            })?;
            let name =
                decode_zip_entry_name(entry.name_raw()).replace('\\', "/");
            if name == MRPACK_MANIFEST_ENTRY
                || name.ends_with(&format!("/{MRPACK_MANIFEST_ENTRY}"))
            {
                return Ok(name);
            }
        }
        Err(ErrorKind::InputError(
            "Modpack archive is missing modrinth.index.json".to_string(),
        )
        .as_error())
    })
    .await?
}

fn base_folder(manifest_entry: &str) -> String {
    manifest_entry
        .strip_suffix(MRPACK_MANIFEST_ENTRY)
        .unwrap_or_default()
        .to_string()
}

async fn parse_index(
    archive_path: &Path,
    manifest_entry: &str,
) -> Result<MrpackIndex> {
    let contents = read_archive_entry_to_string(
        archive_path.to_path_buf(),
        manifest_entry.to_string(),
    )
    .await?;
    serde_json::from_str(&contents).map_err(|error| {
        ErrorKind::InputError(format!(
            "Failed to parse modrinth.index.json: {error}"
        ))
        .as_error()
    })
}

async fn log(server_id: &str, line: &str) -> Result<()> {
    emit_server(
        server_id,
        ServerPayloadType::Log {
            line: line.to_string(),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn index_file(path: &str) -> serde_json::Value {
        serde_json::json!({
            "path": path,
            "hashes": { "sha1": "0".repeat(40) },
            "downloads": ["https://example.com/mod.jar"],
            "fileSize": 10
        })
    }

    #[tokio::test]
    async fn malicious_index_paths_fail_before_materializing_files() {
        let temp = tempfile::tempdir().unwrap();
        let server = temp.path().join("server");
        std::fs::create_dir(&server).unwrap();
        let outside = temp.path().join("outside.txt");
        std::fs::write(&outside, "unchanged").unwrap();
        let archive = temp.path().join("pack.mrpack");
        for path in [
            "../outside.txt",
            "mods/../../outside.txt",
            r"..\outside.txt",
            r"mods/..\../outside.txt",
            "/tmp/outside.txt",
            "C:/outside.txt",
            r"C:\outside.txt",
            "C:outside.txt",
            r"\\server\share\outside.txt",
            r"\outside.txt",
            r"\\?\C:\outside.txt",
            "mods//mod.jar",
            "mods/./mod.jar",
            "mods/.. /outside.txt",
            "mods/mod.jar:payload",
            "NUL/outside.txt",
            "mods/CON.jar",
            "",
        ] {
            for environment in ["required", "unsupported"] {
                let mut malicious = index_file(path);
                // Filtered entries must also fail before any downloads.
                malicious["env"] = serde_json::json!({"server": environment});
                let contents = serde_json::json!({
                    "files": [index_file("mods/valid.jar"), malicious]
                })
                .to_string();
                create_jar(&archive, &[(MRPACK_MANIFEST_ENTRY, &contents)]);
                let index =
                    parse_index(&archive, MRPACK_MANIFEST_ENTRY).await.unwrap();
                assert!(validate_index(&index, &server).is_err(), "{path:?}");
                assert_eq!(
                    std::fs::read_to_string(&outside).unwrap(),
                    "unchanged"
                );
                assert_eq!(std::fs::read_dir(&server).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn index_requires_valid_supported_checksums() {
        for hashes in [
            serde_json::json!({}),
            serde_json::json!({"md5": "0".repeat(32)}),
            serde_json::json!({"sha1": ""}),
            serde_json::json!({"sha1": "0".repeat(39)}),
            serde_json::json!({"sha512": "g".repeat(128)}),
            serde_json::json!({"sha1": "invalid", "sha512": "0".repeat(128)}),
        ] {
            let mut file = index_file("mods/mod.jar");
            file["hashes"] = hashes;
            let index: MrpackIndex =
                serde_json::from_value(serde_json::json!({"files": [file]}))
                    .unwrap();
            assert!(validate_index(&index, Path::new("server")).is_err());
        }
        let mut file = index_file("mods/mod.jar");
        file.as_object_mut().unwrap().remove("hashes");
        let index: MrpackIndex =
            serde_json::from_value(serde_json::json!({"files": [file]}))
                .unwrap();
        assert!(validate_index(&index, Path::new("server")).is_err());
    }

    #[tokio::test]
    async fn index_checksums_verify_file_contents() {
        use crate::util::download::integrity::verify_file;
        use sha2::{Digest, Sha512};

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("mod.jar");
        let contents = b"verified mod";
        std::fs::write(&target, contents).unwrap();
        for hashes in [
            serde_json::json!({"sha1": sha1_smol::Sha1::from(contents).digest().to_string()}),
            serde_json::json!({"sha512": format!("{:x}", Sha512::digest(contents))}),
            serde_json::json!({
                "sha1": sha1_smol::Sha1::from(contents).digest().to_string(),
                "sha512": format!("{:x}", Sha512::digest(contents))
            }),
        ] {
            let mut file = index_file("mods/mod.jar");
            file["hashes"] = hashes;
            file["fileSize"] = serde_json::json!(contents.len());
            let index: MrpackIndex =
                serde_json::from_value(serde_json::json!({"files": [file]}))
                    .unwrap();
            validate_index(&index, temp.path()).unwrap();
            let integrity = index.files[0].integrity().unwrap();
            assert_eq!(integrity.size, Some(contents.len() as u64));
            verify_file(&target, &integrity).await.unwrap();
            std::fs::write(&target, b"tampered mod").unwrap();
            assert!(verify_file(&target, &integrity).await.is_err());
            std::fs::write(&target, contents).unwrap();
        }
    }

    #[test]
    fn server_only_files_are_installable() {
        let installable = MrpackFile {
            path: "mods/a.jar".to_string(),
            hashes: HashMap::new(),
            env: None,
            downloads: vec!["https://example.com/a.jar".to_string()],
            file_size: Some(10),
        };
        assert!(is_server_installable(&installable));

        let optional = MrpackFile {
            env: Some(MrpackEnv {
                server: Some("optional".to_string()),
            }),
            ..installable.clone()
        };
        assert!(is_server_installable(&optional));

        let client_only = MrpackFile {
            env: Some(MrpackEnv {
                server: Some("unsupported".to_string()),
            }),
            ..installable
        };
        assert!(!is_server_installable(&client_only));
    }

    #[test]
    fn client_only_paths_are_skipped() {
        let base = MrpackFile {
            path: String::new(),
            hashes: HashMap::new(),
            env: None,
            downloads: vec!["https://example.com/a.zip".to_string()],
            file_size: Some(10),
        };

        assert!(!is_server_installable(&MrpackFile {
            path: "shaderpacks/fancy.zip".to_string(),
            ..base.clone()
        }));
        assert!(!is_server_installable(&MrpackFile {
            path: "resourcepacks/fancy.zip".to_string(),
            ..base.clone()
        }));
        assert!(!is_server_installable(&MrpackFile {
            path: r"texturepacks\fancy.zip".to_string(),
            ..base.clone()
        }));
        // Datapacks stay: dedicated servers load them.
        assert!(is_server_installable(&MrpackFile {
            path: "datapacks/data.zip".to_string(),
            ..base
        }));
    }

    #[test]
    fn base_folder_derivation() {
        assert_eq!(base_folder("modrinth.index.json"), "");
        assert_eq!(base_folder("my-pack/modrinth.index.json"), "my-pack/");
    }

    /// Helper: create a minimal JAR (ZIP) with the given entry contents.
    fn create_jar(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, contents) in entries {
            zip.start_file(*name, zip::write::FileOptions::<()>::default())
                .unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[tokio::test]
    async fn client_declared_mod_jars_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let versioned = mods.join("1.21");
        tokio::fs::create_dir_all(&versioned).await.unwrap();

        create_jar(
            &mods.join("fancymenu.jar"),
            &[(
                "fabric.mod.json",
                r#"{"environment": "client", "id": "fancymenu"}"#,
            )],
        );
        create_jar(
            &versioned.join("nested-client.jar"),
            &[("fabric.mod.json", r#"{"environment": "client"}"#)],
        );
        create_jar(
            &mods.join("server-mod.jar"),
            &[("fabric.mod.json", r#"{"environment": "*"}"#)],
        );
        create_jar(&mods.join("no-metadata.jar"), &[("some.file", "data")]);
        std::fs::write(mods.join("corrupt.jar"), b"not a zip").unwrap();
        std::fs::write(mods.join("readme.txt"), "keep me").unwrap();

        prune_uninstallable_mods(
            "test-server",
            dir.path(),
            &HashSet::new(),
            &HashSet::new(),
        )
        .await
        .unwrap();

        assert!(!mods.join("fancymenu.jar").exists());
        assert!(!versioned.join("nested-client.jar").exists());
        assert!(mods.join("server-mod.jar").exists());
        assert!(mods.join("no-metadata.jar").exists());
        assert!(mods.join("corrupt.jar").exists());
        assert!(mods.join("readme.txt").exists());
    }

    #[tokio::test]
    async fn mods_requiring_excluded_dependencies_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        tokio::fs::create_dir_all(&mods).await.unwrap();

        create_jar(
            &mods.join("fancymenu.jar"),
            &[(
                "fabric.mod.json",
                r#"{"id": "fancymenu", "depends": {"fabric-api": ">=0.9", "melody": ">=1.0"}}"#,
            )],
        );
        create_jar(
            &mods.join("konkrete.jar"),
            &[(
                "fabric.mod.json",
                r#"{"id": "konkrete", "depends": {"fancymenu": "*"}}"#,
            )],
        );
        create_jar(
            &mods.join("optional-dependent.jar"),
            &[(
                "fabric.mod.json",
                r#"{"id": "optional-dependent", "depends": {"melody": {"optional": true}}}"#,
            )],
        );
        create_jar(
            &mods.join("list-depends.jar"),
            &[(
                "fabric.mod.json",
                r#"{"id": "list-depends", "depends": ["nonstandard"]}"#,
            )],
        );
        create_jar(
            &mods.join("plain.jar"),
            &[("fabric.mod.json", r#"{"id": "plain"}"#)],
        );

        // Simulates melody having been excluded from the server install by
        // its env mark: only its mod ID is known (via fetch_excluded_mod_ids),
        // the jar itself never lands on disk.
        let unavailable = HashSet::from(["melody".to_string()]);
        prune_uninstallable_mods(
            "test-server",
            dir.path(),
            &unavailable,
            &HashSet::new(),
        )
        .await
        .unwrap();

        assert!(!mods.join("fancymenu.jar").exists());
        assert!(!mods.join("konkrete.jar").exists());
        assert!(mods.join("optional-dependent.jar").exists());
        assert!(mods.join("list-depends.jar").exists());
        assert!(mods.join("plain.jar").exists());
    }

    fn meta(
        path: &str,
        id: &str,
        depends: serde_json::Value,
    ) -> (PathBuf, ModMetadata) {
        (
            PathBuf::from(path),
            serde_json::from_value(serde_json::json!({
                "id": id,
                "depends": depends,
            }))
            .unwrap(),
        )
    }

    #[test]
    fn prune_plan_cascades_through_hard_dependencies() {
        let metas = vec![
            meta(
                "a.jar",
                "consumer",
                serde_json::json!({"missing-lib": ">=1.0"}),
            ),
            meta("b.jar", "middleman", serde_json::json!({"consumer": "*"})),
            meta("c.jar", "unrelated", serde_json::json!({"fabric-api": "*"})),
        ];
        let plan = compute_prune_plan(
            &metas,
            &HashSet::from(["missing-lib".to_string()]),
            &HashSet::new(),
        );

        let removed: Vec<&str> = plan
            .iter()
            .map(|(path, _)| path.to_str().unwrap())
            .collect();
        assert_eq!(removed, vec!["a.jar", "b.jar"]);
        assert!(plan[0].1.contains("missing-lib"));
    }

    #[test]
    fn prune_plan_respects_local_provides() {
        let metas = [
            meta("shim.jar", "melody-shim", serde_json::json!({})),
            meta("ui.jar", "ui", serde_json::json!({"melody": ">=1.0"})),
        ];
        // A local mod provides the "missing" dependency, so the dependent stays.
        let mut shim = metas[0].1.clone();
        shim.provides = vec!["melody".to_string()];
        let metas = vec![("shim.jar".into(), shim), metas[1].clone()];
        assert!(
            compute_prune_plan(
                &metas,
                &HashSet::from(["melody".to_string()]),
                &HashSet::new()
            )
            .is_empty()
        );

        // Once the provider itself is removed, the dependent goes with it.
        let mut provider = metas[0].1.clone();
        provider.provides = vec!["melody".to_string()];
        provider.depends = Some(serde_json::json!({"removed-core": "*"}));
        let metas = vec![("provider.jar".into(), provider), metas[1].clone()];
        let plan = compute_prune_plan(
            &metas,
            &HashSet::from(["melody".to_string(), "removed-core".to_string()]),
            &HashSet::new(),
        );
        assert_eq!(plan.len(), 2);
    }

    #[tokio::test]
    async fn client_only_override_dirs_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let shaders = dir.path().join("shaderpacks");
        tokio::fs::create_dir_all(shaders.join("Complementary"))
            .await
            .unwrap();
        tokio::fs::write(
            shaders.join("Complementary.zip"),
            b"not really a shader",
        )
        .await
        .unwrap();
        let mods = dir.path().join("mods");
        tokio::fs::create_dir_all(&mods).await.unwrap();

        remove_client_only_dirs("test-server", dir.path())
            .await
            .unwrap();

        assert!(!shaders.exists());
        assert!(mods.exists());
    }

    #[tokio::test]
    async fn missing_client_only_dirs_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        remove_client_only_dirs("test-server", dir.path())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn missing_mods_dir_is_tolerated_when_pruning() {
        let dir = tempfile::tempdir().unwrap();
        prune_uninstallable_mods(
            "test-server",
            dir.path(),
            &HashSet::new(),
            &HashSet::new(),
        )
        .await
        .unwrap();
    }
}
