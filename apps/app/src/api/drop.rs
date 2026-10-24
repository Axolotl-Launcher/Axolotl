use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;
use tauri::Emitter;
use theseus::drop_classifier::{
    DroppedCandidate, DroppedItemType, ModrinthLookupResult, lookup_mod_hash,
};
use theseus::pack::import::{ImportLauncherType, get_importable_instances};
use theseus::{LockingProcess, get_locking_processes};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const DROP_CLASSIFY_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const DROP_EXTRACT_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const DROP_SCAN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DROP_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DROP_METADATA_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const DROP_METADATA_MAX_BYTES: u64 = 512 * 1024 * 1024;

async fn run_cancellable_blocking<T, F>(
    operation: &'static str,
    timeout_duration: Duration,
    operation_fn: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
{
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let mut worker =
        tokio::task::spawn_blocking(move || operation_fn(worker_cancellation));

    match tokio::time::timeout(timeout_duration, &mut worker).await {
        Ok(result) => result
            .map_err(|error| format!("{operation} task panicked: {error}"))?,
        Err(_) => {
            cancellation.cancel();
            let _ = worker.await;
            Err(format!(
                "{operation} timed out after {} minutes",
                timeout_duration.as_secs() / 60
            ))
        }
    }
}

fn remove_dir_all(path: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!("Failed to inspect '{}': {error}", path.display())
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        std::fs::remove_file(path).map_err(|error| {
            format!("Failed to remove '{}': {error}", path.display())
        })?;
        return Ok(());
    }

    let entries = std::fs::read_dir(path).map_err(|error| {
        format!("Failed to read '{}': {error}", path.display())
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!("Failed to enumerate '{}': {error}", path.display())
        })?;
        remove_dir_all(&entry.path())?;
    }
    std::fs::remove_dir(path).map_err(|error| {
        format!("Failed to remove '{}': {error}", path.display())
    })
}

fn remove_dir_all_cancellable(
    path: &std::path::Path,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    if cancellation.is_cancelled() {
        return Err("Operation cancelled".to_string());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!("Failed to inspect '{}': {error}", path.display())
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        std::fs::remove_file(path).map_err(|error| {
            format!("Failed to remove '{}': {error}", path.display())
        })?;
        return Ok(());
    }

    let entries = std::fs::read_dir(path).map_err(|error| {
        format!("Failed to read '{}': {error}", path.display())
    })?;
    for entry in entries {
        if cancellation.is_cancelled() {
            return Err("Operation cancelled".to_string());
        }
        let entry = entry.map_err(|error| {
            format!("Failed to enumerate '{}': {error}", path.display())
        })?;
        remove_dir_all_cancellable(&entry.path(), cancellation)?;
    }
    if cancellation.is_cancelled() {
        return Err("Operation cancelled".to_string());
    }
    std::fs::remove_dir(path).map_err(|error| {
        format!("Failed to remove '{}': {error}", path.display())
    })
}

fn read_file_cancellable(
    path: &std::path::Path,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!("Failed to read '{}': {error}", path.display())
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            format!("Failed to inspect '{}': {error}", path.display())
        })?
        .len();
    if length > DROP_METADATA_MAX_BYTES {
        return Err(format!(
            "Metadata file is too large (maximum {} MiB)",
            DROP_METADATA_MAX_BYTES / (1024 * 1024)
        ));
    }

    let mut bytes = Vec::with_capacity(length as usize);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err("Operation cancelled".to_string());
        }
        let read = file.read(&mut buffer).map_err(|error| {
            format!("Failed to read '{}': {error}", path.display())
        })?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

/// A scanned importable instance: name plus the resolved filesystem path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedInstance {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub compatible_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_path: Option<String>,
}

/// One candidate inside a multi-candidate classification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateResult {
    pub item_type: String,
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner_base: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

/// Serializable classification result mapped from `DroppedItemType`.
///
/// All `PathBuf` fields are converted to `String` via `to_string_lossy()`.
/// The JSON representation uses an `item_type` tag (via `#[serde(tag = "item_type")]`)
/// so the frontend can discriminate variants with a string switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "item_type")]
pub enum ClassificationResult {
    #[serde(rename = "server")]
    Server {
        file_path: String,
        preview: theseus::servers::ServerImportPreview,
    },
    #[serde(rename = "launcher")]
    Launcher {
        launcher_type: String,
        base_path: String,
        #[serde(
            rename = "innerBase",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        inner_base: Option<String>,
    },
    #[serde(rename = "hmcl_launcher")]
    HmclLauncher {
        launcher_dir: String,
        data_dir: String,
    },
    #[serde(rename = "mod")]
    Mod { file_path: String },
    #[serde(rename = "litematic")]
    Litematic { file_path: String },
    #[serde(rename = "resource_pack")]
    ResourcePack {
        file_path: String,
        candidates: Vec<String>,
        #[serde(
            rename = "innerBase",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        inner_base: Option<String>,
    },
    #[serde(rename = "shader_pack")]
    ShaderPack {
        file_path: String,
        #[serde(
            rename = "innerBase",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        inner_base: Option<String>,
    },
    #[serde(rename = "world_save")]
    WorldSave {
        file_path: String,
        #[serde(
            rename = "innerBase",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        inner_base: Option<String>,
    },
    #[serde(rename = "modpack")]
    Modpack { file_path: String },
    #[serde(rename = "multiple")]
    Multiple {
        file_path: String,
        candidates: Vec<CandidateResult>,
    },
    #[serde(rename = "shortcut_resolved")]
    ShortcutResolved {
        original: String,
        resolved_to: Box<ClassificationResult>,
    },
    #[serde(rename = "unknown")]
    Unknown { reason: String },
}

impl From<DroppedCandidate> for CandidateResult {
    fn from(candidate: DroppedCandidate) -> Self {
        CandidateResult {
            item_type: candidate.item_type,
            file_path: candidate.file_path.to_string_lossy().to_string(),
            inner_base: candidate.inner_base,
            candidates: candidate.candidates,
        }
    }
}

impl From<DroppedItemType> for ClassificationResult {
    fn from(item: DroppedItemType) -> Self {
        match item {
            DroppedItemType::Server { file_path, preview } => Self::Server {
                file_path: file_path.to_string_lossy().into_owned(),
                preview,
            },
            DroppedItemType::Launcher {
                launcher_type,
                base_path,
                inner_base,
            } => ClassificationResult::Launcher {
                launcher_type: launcher_type.to_string(),
                base_path: base_path.to_string_lossy().to_string(),
                inner_base,
            },
            DroppedItemType::HmclLauncher {
                launcher_dir,
                data_dir,
            } => ClassificationResult::HmclLauncher {
                launcher_dir: launcher_dir.to_string_lossy().to_string(),
                data_dir: data_dir.to_string_lossy().to_string(),
            },
            DroppedItemType::Mod { file_path } => ClassificationResult::Mod {
                file_path: file_path.to_string_lossy().to_string(),
            },
            DroppedItemType::Litematic { file_path } => {
                ClassificationResult::Litematic {
                    file_path: file_path.to_string_lossy().to_string(),
                }
            }
            DroppedItemType::ResourcePack {
                file_path,
                candidates,
                inner_base,
            } => ClassificationResult::ResourcePack {
                file_path: file_path.to_string_lossy().to_string(),
                candidates,
                inner_base,
            },
            DroppedItemType::ShaderPack {
                file_path,
                inner_base,
            } => ClassificationResult::ShaderPack {
                file_path: file_path.to_string_lossy().to_string(),
                inner_base,
            },
            DroppedItemType::WorldSave {
                file_path,
                inner_base,
            } => ClassificationResult::WorldSave {
                file_path: file_path.to_string_lossy().to_string(),
                inner_base,
            },
            DroppedItemType::ShortcutResolved {
                original,
                resolved_to,
            } => ClassificationResult::ShortcutResolved {
                original: original.to_string_lossy().to_string(),
                resolved_to: Box::new(ClassificationResult::from(*resolved_to)),
            },
            DroppedItemType::Modpack { file_path } => {
                ClassificationResult::Modpack {
                    file_path: file_path.to_string_lossy().to_string(),
                }
            }
            DroppedItemType::Multiple {
                file_path,
                candidates,
            } => ClassificationResult::Multiple {
                file_path: file_path.to_string_lossy().to_string(),
                candidates: candidates.into_iter().map(Into::into).collect(),
            },
            DroppedItemType::Unknown { reason } => {
                ClassificationResult::Unknown { reason }
            }
        }
    }
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("drop")
        .invoke_handler(tauri::generate_handler![
            drop_classify,
            drop_classify_extract,
            drop_extract_zip_to_temp,
            drop_scan_launcher_instances,
            drop_resolve_gamedirs,
            drop_remove_temp_dir,
            drop_detect_file_lock,
            drop_extract_mod_metadata,
            drop_lookup_mod_hash,
        ])
        .build()
}

/// Classify a dropped file or folder path.
///
/// Returns a `ClassificationResult` with an `item_type` tag that the frontend
/// can use to decide what UI to show (confirm dialog, error, etc.).
#[tauri::command]
pub async fn drop_classify<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
    allow_nested_extraction: Option<bool>,
) -> Result<ClassificationResult, String> {
    debug!("Drop event received: {}", path);
    let path = std::path::PathBuf::from(&path);
    let path_label = path.to_string_lossy().to_string();
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "classify",
            "currentItem": path_label,
            "processed": 0,
            "total": null,
        }),
    );
    // The first pass never unpacks nested archives; when one would be needed
    // the classification reports the total nested size so the frontend can
    // confirm the potentially slow unpack with the user before retrying.
    // Batch drops classify several files concurrently, so the classifier must
    // run on a blocking thread instead of occupying the async runtime.
    let allow_nested_extraction = allow_nested_extraction.unwrap_or(false);
    let result = run_cancellable_blocking(
        "Classification",
        DROP_CLASSIFY_TIMEOUT,
        move |cancellation| {
            Ok(theseus::drop_classifier::classify_dropped_item_with_candidates_cancellable(
                &path,
                allow_nested_extraction,
                &cancellation,
            ))
        },
    )
    .await?;
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "done",
            "currentItem": path_label,
            "processed": 1,
            "total": 1,
        }),
    );
    let classification = ClassificationResult::from(result);
    info!("Classification result: {:?}", classification);
    Ok(classification)
}

/// Classify ZIP

#[tauri::command]
pub async fn drop_classify_extract<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<ClassificationResult, String> {
    debug!("Drop classify with extraction: {}", path);
    let path = std::path::PathBuf::from(&path);
    let path_label = path.to_string_lossy().to_string();
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "extract",
            "currentItem": &path_label,
            "processed": 0,
            "total": null,
        }),
    );
    let result = run_cancellable_blocking(
        "ZIP analysis",
        DROP_EXTRACT_TIMEOUT,
        move |cancellation| {
            Ok(theseus::drop_classifier::classify_zip_with_extraction_cancellable(
                &path,
                &cancellation,
            ))
        },
    )
    .await?;
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "done",
            "currentItem": &path_label,
            "processed": 1,
            "total": 1,
        }),
    );
    let classification = ClassificationResult::from(result);
    info!(
        "Classification result (with extraction): {:?}",
        classification
    );
    Ok(classification)
}

/// Root directory under the system temp where compressed launcher folders
/// are extracted for scanning and importing. Entries are removed with
/// `drop_remove_temp_dir` once the frontend flow ends.
fn launcher_import_temp_base() -> std::path::PathBuf {
    std::env::temp_dir().join("axolotl-launcher-import")
}

/// Remove `drop-*` directories under `base` whose contents are older than
/// one day. The frontend cleans up after every flow, but a crashed process
/// would otherwise leave stale extractions behind forever.
fn sweep_stale_launcher_import_dirs(base: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(24 * 60 * 60));
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("drop-") {
            continue;
        }
        let stale = match path.metadata().and_then(|m| m.modified()) {
            Ok(modified) => cutoff.is_none_or(|cutoff| modified < cutoff),
            Err(_) => false,
        };
        if stale {
            tracing::debug!(
                "Removing stale launcher import temp dir: {}",
                path.display()
            );
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Extract a ZIP archive into a fresh temporary directory and return its
/// path. The frontend scans and imports instances from the extraction, then
/// calls [`drop_remove_temp_dir`] to clean it up — the archive is unpacked
/// exactly once.
#[tauri::command]
pub async fn drop_extract_zip_to_temp<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    zip_path: String,
) -> Result<String, String> {
    let zip_path = std::path::PathBuf::from(&zip_path);
    info!("Extracting launcher ZIP to temp: {}", zip_path.display());
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "extract",
            "currentItem": zip_path.to_string_lossy(),
            "processed": 0,
            "total": null,
        }),
    );

    let base = launcher_import_temp_base();
    let zip_path_label = zip_path.to_string_lossy().to_string();
    let extracted = run_cancellable_blocking(
        "Launcher ZIP extraction",
        DROP_EXTRACT_TIMEOUT,
        move |cancellation| -> Result<String, String> {
            std::fs::create_dir_all(&base).map_err(|e| {
                format!("Failed to create temp base '{}': {e}", base.display())
            })?;
            sweep_stale_launcher_import_dirs(&base);
            let dir = base.join(format!(
                "drop-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::create_dir(&dir)
                .map_err(|e| format!("Failed to create temp directory: {e}"))?;
            if let Err(error) =
                theseus::drop_classifier::extract_zip_to_dir_cancellable(
                    &zip_path,
                    &dir,
                    &cancellation,
                )
            {
                let _ = remove_dir_all(&dir);
                tracing::warn!(
                    "Launcher ZIP extraction failed for '{}': {error}",
                    zip_path.display()
                );
                return Err(error);
            }
            Ok(dir.to_string_lossy().to_string())
        },
    )
    .await?;

    info!("Extracted launcher ZIP to: {extracted}");
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "done",
            "currentItem": zip_path_label,
            "processed": 1,
            "total": 1,
        }),
    );
    Ok(extracted)
}

/// Remove a temporary directory created by [`drop_extract_zip_to_temp`].
/// Only paths inside the launcher import temp root are accepted.
#[tauri::command]
pub async fn drop_remove_temp_dir(path: String) -> Result<(), String> {
    let base = launcher_import_temp_base();
    let base = std::fs::canonicalize(&base)
        .map_err(|e| format!("Launcher import temp base missing: {e}"))?;
    let target = std::fs::canonicalize(&path)
        .map_err(|e| format!("Temp path missing: {e}"))?;
    if !target.starts_with(&base) {
        return Err(format!(
            "Refusing to remove '{}': not inside the launcher import temp root",
            target.display()
        ));
    }
    if !target.is_dir() {
        return Err(format!(
            "Refusing to remove '{}': not a directory",
            target.display()
        ));
    }
    run_cancellable_blocking(
        "Temp directory cleanup",
        DROP_CLEANUP_TIMEOUT,
        move |cancellation| remove_dir_all_cancellable(&target, &cancellation),
    )
    .await
}

/// Scan for importable instances in a launcher's data directory.
///
/// `launcher_type` must be one of the `ImportLauncherType` variant names
/// (e.g. `"MultiMC"`, `"PrismLauncher"`, `"HMCL"`).
#[tauri::command]
pub async fn drop_scan_launcher_instances<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    launcher_type: String,
    base_path: String,
) -> Result<Vec<ScannedInstance>, String> {
    info!(
        "Scanning launcher instances — type: {launcher_type}, path: {base_path}"
    );
    let base_path_label = base_path.clone();
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "scan",
            "currentItem": &base_path_label,
            "processed": 0,
            "total": null,
        }),
    );
    let lt: ImportLauncherType =
        serde_json::from_str(&format!("\"{launcher_type}\"")).map_err(|e| {
            format!("Invalid launcher type '{launcher_type}': {e}")
        })?;
    let base = std::path::PathBuf::from(&base_path);
    let instances = tokio::time::timeout(
        DROP_SCAN_TIMEOUT,
        get_importable_instances(lt, base),
    )
    .await
    .map_err(|_| {
        format!(
            "Launcher scan timed out after {} minutes",
            DROP_SCAN_TIMEOUT.as_secs() / 60
        )
    })?
    .map_err(|e| e.to_string())?;
    info!("Scan complete — found {} instance(s)", instances.len());
    for inst in &instances {
        debug!(
            "Scanned instance: name={:?} path={:?} compatible_mode={}",
            inst.name, inst.path, inst.compatible_mode
        );
    }
    let _ = app.emit(
        "drop_classify_progress",
        serde_json::json!({
            "phase": "done",
            "currentItem": &base_path_label,
            "processed": 1,
            "total": 1,
        }),
    );
    Ok(instances
        .into_iter()
        .map(|i| ScannedInstance {
            name: i.name,
            path: i.path,
            compatible_mode: i.compatible_mode,
            version_path: i.version_path,
        })
        .collect())
}

/// Resolve launcher profiles to their external Minecraft game directories.
#[tauri::command]
pub async fn drop_resolve_gamedirs(
    launcher_type: String,
    base_path: String,
) -> Result<Vec<theseus::pack::import::gamedir_resolve::ResolvedGamedir>, String>
{
    let launcher_type: ImportLauncherType =
        serde_json::from_str(&format!("\"{launcher_type}\"")).map_err(
            |error| format!("Invalid launcher type '{launcher_type}': {error}"),
        )?;
    theseus::pack::import::gamedir_resolve::resolve_gamedirs(
        launcher_type,
        std::path::PathBuf::from(base_path),
    )
    .map_err(|error| error.to_string())
}

/// Detect processes holding a file lock on the given path.
///
/// Returns an empty list when detection is unavailable on the current platform
/// or the required tools are not installed.
#[tauri::command]
pub async fn drop_detect_file_lock(
    path: String,
) -> Result<Vec<LockingProcess>, String> {
    let path = std::path::PathBuf::from(&path);
    info!("Detecting file lock for: {}", path.display());
    let processes = get_locking_processes(&path);
    if !processes.is_empty() {
        warn!("File locked by {} process(es)", processes.len());
    }
    Ok(processes)
}

/// Extract mod metadata from a JAR file without installing it.
///
/// Reads the JAR bytes, extracts embedded mod metadata (fabric.mod.json,
/// quilt.mod.json, META-INF/mods.toml, etc.), and returns the parsed
/// `LocalModMetadata` as a JSON string.
#[tauri::command]
pub async fn drop_extract_mod_metadata(path: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(&path);

    let meta = run_cancellable_blocking(
        "Metadata extraction",
        DROP_METADATA_TIMEOUT,
        move |cancellation| {
            let file_bytes = read_file_cancellable(&path, &cancellation)?;
            let bytes = bytes::Bytes::from(file_bytes);
            theseus::mod_metadata::extract_mod_metadata(&bytes)
                .ok_or_else(|| "No mod metadata found in file".to_string())
        },
    )
    .await?;
    serde_json::to_string(&meta)
        .map_err(|e| format!("Failed to serialize metadata: {e}"))
}

/// Look up a mod file by SHA1 hash to find matching Modrinth project and version.
///
/// Computes the SHA1 hash of the given file and queries the Modrinth API
/// to find matching versions. Returns project and version information if found.
#[tauri::command]
pub async fn drop_lookup_mod_hash(
    path: String,
) -> Result<Option<ModrinthLookupResult>, String> {
    let path = std::path::PathBuf::from(&path);
    info!("Looking up mod hash for: {}", path.display());

    lookup_mod_hash(&path)
        .await
        .map_err(|e| format!("Failed to lookup mod hash: {e}"))
}
