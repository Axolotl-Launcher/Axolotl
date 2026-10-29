use crate::api::Result;
use async_zip::base::read::seek::ZipFileReader;
use image::ImageEncoder;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;
use tauri::{Emitter, Runtime};
use tauri_plugin_dialog::DialogExt;
use theseus::instance::get_full_path;

const STUDIO_FILES_CHANGED_EVENT: &str = "studio-files-changed";

#[derive(PartialEq)]
struct LocalIconCacheKey {
    source: PathBuf,
    size: u64,
    modified: SystemTime,
    max_dimension: u32,
}

static LOCAL_ICON_CACHE: LazyLock<
    Mutex<VecDeque<(LocalIconCacheKey, String)>>,
> = LazyLock::new(|| Mutex::new(VecDeque::new()));

// Limit native decoding as well as the webview queue: old in-flight requests
// can outlive their cards, and each decoded original may occupy hundreds of MB.
static SCREENSHOT_DECODER: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(1);
static SCREENSHOT_THUMBNAILS: LazyLock<
    Mutex<VecDeque<(LocalIconCacheKey, Vec<u8>)>>,
> = LazyLock::new(|| Mutex::new(VecDeque::new()));
const SCREENSHOT_THUMBNAIL_CACHE_BYTES: usize = 32 * 1024 * 1024;

fn safe_extract_relative_path(name: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(name);
    if name.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, std::path::Component::Normal(_))
        })
    {
        return None;
    }
    Some(path.to_path_buf())
}

pub(crate) fn ensure_browsable<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    root: &std::path::Path,
) {
    use tauri_plugin_fs::FsExt;
    if let Err(error) = app.fs_scope().allow_directory(root, true) {
        tracing::warn!(%error, path = %root.display(), "Failed to grant webview access to external instance");
    }
}

#[derive(Default)]
pub struct StudioWatchers {
    watchers: Mutex<HashMap<String, StudioWatcher>>,
}

struct StudioWatcher {
    registration_id: String,
    root: PathBuf,
    watcher: RecommendedWatcher,
}

impl Drop for StudioWatcher {
    fn drop(&mut self) {
        if let Err(error) = self.watcher.unwatch(&self.root) {
            tracing::warn!(%error, "Failed to stop Studio file watcher");
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioFilesChangedEvent {
    instance_id: String,
    registration_id: String,
    paths: Vec<String>,
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("files")
        .invoke_handler(tauri::generate_handler![
            file_extract_zip,
            file_save_as,
            file_read_dragged_file,
            screenshot_thumbnail,
            instance_icon_thumbnail,
            studio_read_text,
            studio_read_binary,
            studio_write_binary,
            studio_trash,
            studio_watch_register,
            studio_watch_unregister,
        ])
        .build()
}

#[tauri::command]
pub async fn studio_trash(instance_id: &str, file_path: &str) -> Result<()> {
    let base = get_full_path(instance_id).await?;
    let source = tokio::fs::canonicalize(base.join(file_path)).await?;
    let canonical_base = tokio::fs::canonicalize(&base).await?;
    if !source.starts_with(&canonical_base) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "file_path escapes the instance directory".to_string(),
        ))
        .into());
    }
    tokio::task::spawn_blocking(move || trash::delete(source))
        .await
        .map_err(|error| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Failed to send file to trash: {error}"
            )))
        })?
        .map_err(|error| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Failed to send file to trash: {error}"
            )))
        })?;
    Ok(())
}

#[tauri::command]
pub async fn studio_read_text(
    instance_id: &str,
    file_path: &str,
) -> Result<String> {
    let base = get_full_path(instance_id).await?;
    let source = tokio::fs::canonicalize(base.join(file_path)).await?;
    let canonical_base = tokio::fs::canonicalize(&base).await?;
    if !source.starts_with(&canonical_base) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "file_path escapes the instance directory".to_string(),
        ))
        .into());
    }

    let bytes = tokio::fs::read(source).await?;
    if bytes.contains(&0) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "File is not a text file".to_string(),
        ))
        .into());
    }
    String::from_utf8(bytes).map_err(|_| {
        theseus::Error::from(theseus::ErrorKind::OtherError(
            "File is not a UTF-8 text file".to_string(),
        ))
        .into()
    })
}

async fn studio_file_path(
    instance_id: &str,
    file_path: &str,
) -> Result<PathBuf> {
    let base = get_full_path(instance_id).await?;
    let source = tokio::fs::canonicalize(base.join(file_path)).await?;
    let canonical_base = tokio::fs::canonicalize(&base).await?;
    if !source.starts_with(&canonical_base) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "file_path escapes the instance directory".to_string(),
        ))
        .into());
    }
    Ok(source)
}

#[tauri::command]
pub async fn studio_read_binary(
    instance_id: &str,
    file_path: &str,
) -> Result<tauri::ipc::Response> {
    Ok(tauri::ipc::Response::new(
        tokio::fs::read(studio_file_path(instance_id, file_path).await?)
            .await?,
    ))
}

#[tauri::command]
pub async fn studio_write_binary(
    instance_id: &str,
    file_path: &str,
    bytes: Vec<u8>,
) -> Result<()> {
    tokio::fs::write(studio_file_path(instance_id, file_path).await?, bytes)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn studio_watch_register<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, StudioWatchers>,
    instance_id: String,
) -> Result<String> {
    let root = get_full_path(&instance_id).await?;
    if !root.is_dir() {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "Instance directory does not exist".to_string(),
        ))
        .into());
    }

    let registration_id = uuid::Uuid::new_v4().to_string();
    let event_instance_id = instance_id.clone();
    let event_registration_id = registration_id.clone();
    let event_root = root.clone();
    let mut watcher = notify::recommended_watcher(
        move |result: notify::Result<notify::Event>| match result {
            Ok(event) => {
                let mut paths = event
                    .paths
                    .into_iter()
                    .filter_map(|path| {
                        path.strip_prefix(&event_root)
                            .ok()
                            .filter(|relative| !relative.as_os_str().is_empty())
                            .map(|relative| {
                                relative.to_string_lossy().replace('\\', "/")
                            })
                    })
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                if paths.is_empty() {
                    return;
                }

                if let Err(error) = app.emit(
                    STUDIO_FILES_CHANGED_EVENT,
                    StudioFilesChangedEvent {
                        instance_id: event_instance_id.clone(),
                        registration_id: event_registration_id.clone(),
                        paths,
                    },
                ) {
                    tracing::warn!(%error, "Failed to emit Studio file watcher event");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Studio file watcher failed");
            }
        },
    )
    .map_err(|error| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to create Studio file watcher: {error}"
        )))
    })?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Failed to watch instance directory: {error}"
            )))
        })?;

    let mut watchers = state.watchers.lock().map_err(|_| {
        theseus::Error::from(theseus::ErrorKind::OtherError(
            "Studio watcher state is unavailable".to_string(),
        ))
    })?;
    watchers.insert(
        instance_id,
        StudioWatcher {
            registration_id: registration_id.clone(),
            root,
            watcher,
        },
    );

    Ok(registration_id)
}

#[tauri::command]
pub fn studio_watch_unregister(
    state: tauri::State<'_, StudioWatchers>,
    instance_id: String,
    registration_id: String,
) -> Result<()> {
    let mut watchers = state.watchers.lock().map_err(|_| {
        theseus::Error::from(theseus::ErrorKind::OtherError(
            "Studio watcher state is unavailable".to_string(),
        ))
    })?;
    if watchers
        .get(&instance_id)
        .is_some_and(|watcher| watcher.registration_id == registration_id)
    {
        watchers.remove(&instance_id);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ExtractDryRunResult {
    modpack_name: Option<String>,
    conflicting_files: Vec<String>,
}

#[tauri::command]
pub async fn file_read_dragged_file(
    path: String,
) -> Result<tauri::ipc::Response> {
    let metadata = tokio::fs::metadata(&path).await?;
    if !metadata.is_file() {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "Dropped path is not a file".to_string(),
        ))
        .into());
    }

    // Raw binary payload: the frontend receives an ArrayBuffer instead of a
    // JSON number array, which would balloon multi-hundred-MB files to
    // gigabytes of transient memory on both sides of the IPC boundary.
    Ok(tauri::ipc::Response::new(tokio::fs::read(path).await?))
}

/// Decodes a screenshot and returns a downscaled thumbnail as raw image bytes,
/// so the webview never decodes high-resolution originals in the screenshots grid.
/// Files that already fit within `max_dimension` are returned unchanged.
#[tauri::command]
pub async fn screenshot_thumbnail(
    instance_id: &str,
    file_path: &str,
    max_dimension: u32,
) -> Result<tauri::ipc::Response> {
    let base = get_full_path(instance_id).await?;
    let canonical_source =
        tokio::fs::canonicalize(base.join(file_path)).await?;
    let canonical_base = tokio::fs::canonicalize(&base).await?;
    if !canonical_source.starts_with(&canonical_base) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "file_path escapes the instance directory".to_string(),
        ))
        .into());
    }

    let max_dimension = max_dimension.clamp(1, 1024);
    let metadata = tokio::fs::metadata(&canonical_source).await?;
    let key = LocalIconCacheKey {
        source: canonical_source.clone(),
        size: metadata.len(),
        modified: metadata.modified()?,
        max_dimension,
    };
    let _permit = SCREENSHOT_DECODER.acquire().await.map_err(|error| {
        theseus::Error::from(theseus::ErrorKind::OtherError(error.to_string()))
    })?;
    if let Ok(mut cache) = SCREENSHOT_THUMBNAILS.lock() {
        if let Some(index) = cache.iter().position(|(cached, _)| cached == &key)
        {
            let entry = cache.remove(index).unwrap();
            let bytes = entry.1.clone();
            cache.push_back(entry);
            return Ok(tauri::ipc::Response::new(bytes));
        }
    }
    let bytes = tokio::fs::read(&canonical_source).await?;
    let thumbnail = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let (width, height) =
            image::ImageReader::new(Cursor::new(bytes.as_slice()))
                .with_guessed_format()
                .map_err(|error| thumbnail_error(error.into()))?
                .into_dimensions()
                .map_err(thumbnail_error)?;
        if width <= max_dimension && height <= max_dimension {
            return Ok(bytes);
        }

        let decoded = image::ImageReader::new(Cursor::new(bytes.as_slice()))
            .with_guessed_format()
            .map_err(|error| thumbnail_error(error.into()))?
            .decode()
            .map_err(thumbnail_error)?;
        let thumbnail = decoded.thumbnail(max_dimension, max_dimension);
        let mut output = Vec::new();
        if thumbnail.color().has_alpha() {
            let rgba = thumbnail.to_rgba8();
            image::codecs::png::PngEncoder::new(&mut output)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(thumbnail_error)?;
        } else {
            let rgb = thumbnail.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 85)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(thumbnail_error)?;
        }
        Ok(output)
    })
    .await
    .map_err(|e| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Screenshot thumbnail task failed: {e}"
        )))
    })??;

    if thumbnail.len() <= SCREENSHOT_THUMBNAIL_CACHE_BYTES {
        if let Ok(mut cache) = SCREENSHOT_THUMBNAILS.lock() {
            cache.retain(|(cached, _)| {
                cached.source != key.source
                    || cached.max_dimension != key.max_dimension
            });
            let mut size =
                cache.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
            while size + thumbnail.len() > SCREENSHOT_THUMBNAIL_CACHE_BYTES {
                if let Some((_, bytes)) = cache.pop_front() {
                    size -= bytes.len();
                } else {
                    break;
                }
            }
            cache.push_back((key, thumbnail.clone()));
        }
    }
    Ok(tauri::ipc::Response::new(thumbnail))
}

pub(crate) async fn local_instance_icon_path(
    instance_id: &str,
    max_dimension: u32,
) -> Result<Option<String>> {
    const LOCAL_ICON_NAMES: [&str; 4] =
        ["icon.png", "icon.jpg", "icon.jpeg", "icon.webp"];
    const MAX_LOCAL_ICON_BYTES: usize = 2 * 1024 * 1024;
    let base = get_full_path(instance_id).await?;
    let max_dimension = max_dimension.max(1);

    for file_name in LOCAL_ICON_NAMES {
        let source = base.join(file_name);
        let Ok(metadata) = tokio::fs::metadata(&source).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let cache_key =
            metadata.modified().ok().map(|modified| LocalIconCacheKey {
                source: source.clone(),
                size: metadata.len(),
                modified,
                max_dimension,
            });
        let cached = cache_key.as_ref().and_then(|key| {
            let mut cache = LOCAL_ICON_CACHE.lock().ok()?;
            let index =
                cache.iter().position(|(cached_key, _)| cached_key == key)?;
            let entry = cache.remove(index)?;
            let path = entry.1.clone();
            cache.push_back(entry);
            Some(path)
        });
        if let Some(path) = cached {
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                return Ok(Some(path));
            }
        }

        let candidate: Result<Option<String>> = async {
            let bytes = tokio::fs::read(&source).await?;
            let (processed, cache_name) = tokio::task::spawn_blocking(
                move || -> Result<(Vec<u8>, &'static str)> {
                    let (width, height) =
                        image::ImageReader::new(Cursor::new(bytes.as_slice()))
                            .with_guessed_format()
                            .map_err(|error| thumbnail_error(error.into()))?
                            .into_dimensions()
                            .map_err(thumbnail_error)?;

                    if width <= max_dimension
                        && height <= max_dimension
                        && bytes.len() <= MAX_LOCAL_ICON_BYTES
                    {
                        return Ok((bytes, file_name));
                    }

                    let decoded =
                        image::ImageReader::new(Cursor::new(bytes.as_slice()))
                            .with_guessed_format()
                            .map_err(|error| thumbnail_error(error.into()))?
                            .decode()
                            .map_err(thumbnail_error)?;
                    let thumbnail =
                        decoded.thumbnail(max_dimension, max_dimension);
                    let mut output = Vec::new();
                    if thumbnail.color().has_alpha() {
                        let rgba = thumbnail.to_rgba8();
                        image::codecs::png::PngEncoder::new(&mut output)
                            .write_image(
                                rgba.as_raw(),
                                rgba.width(),
                                rgba.height(),
                                image::ExtendedColorType::Rgba8,
                            )
                            .map_err(thumbnail_error)?;
                        Ok((output, "icon.png"))
                    } else {
                        let rgb = thumbnail.to_rgb8();
                        image::codecs::jpeg::JpegEncoder::new_with_quality(
                            &mut output,
                            85,
                        )
                        .write_image(
                            rgb.as_raw(),
                            rgb.width(),
                            rgb.height(),
                            image::ExtendedColorType::Rgb8,
                        )
                        .map_err(thumbnail_error)?;
                        Ok((output, "icon.jpg"))
                    }
                },
            )
            .await
            .map_err(|error| {
                theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                    "Instance icon thumbnail task failed: {error}"
                )))
            })??;

            let cached_path =
                theseus::instance::cache_icon(cache_name, processed).await?;
            Ok(Some(cached_path))
        }
        .await;

        if let Ok(Some(path)) = candidate {
            if let Some(key) = cache_key {
                if let Ok(mut cache) = LOCAL_ICON_CACHE.lock() {
                    cache.retain(|(existing, _)| {
                        existing.source != key.source
                            || existing.max_dimension != key.max_dimension
                    });
                    cache.push_back((key, path.clone()));
                    while cache.len() > 256 {
                        cache.pop_front();
                    }
                }
            }
            return Ok(Some(path));
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn instance_icon_thumbnail(
    instance_id: &str,
    max_dimension: u32,
) -> Result<Option<String>> {
    local_instance_icon_path(instance_id, max_dimension).await
}

fn thumbnail_error(error: image::ImageError) -> theseus::Error {
    theseus::Error::from(theseus::ErrorKind::OtherError(format!(
        "Failed to process screenshot: {error}"
    )))
}

#[tauri::command]
pub async fn file_extract_zip(
    instance_id: &str,
    file_path: &str,
    override_conflicts: bool,
    dry_run: bool,
) -> Result<Option<ExtractDryRunResult>> {
    let base = get_full_path(instance_id).await?;
    let zip_path = base.join(file_path);
    let canonical_zip = tokio::fs::canonicalize(&zip_path).await?;
    let canonical_base = tokio::fs::canonicalize(&base).await?;
    if !canonical_zip.starts_with(&canonical_base) {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "file_path escapes the instance directory".to_string(),
        ))
        .into());
    }
    let extract_dir = zip_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base.clone());

    let file_bytes = tokio::fs::read(&zip_path).await?;
    let reader = Cursor::new(file_bytes);

    let zip_reader = ZipFileReader::with_tokio(reader).await.map_err(|e| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to read zip file: {e}"
        )))
    })?;

    let entries: Vec<(usize, String)> = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            let name = entry.filename().as_str().ok()?.to_string();
            if name.ends_with('/') {
                None
            } else {
                Some((i, name))
            }
        })
        .collect();

    if dry_run {
        let mut conflicting_files = Vec::new();
        let canonical_extract = tokio::fs::canonicalize(&extract_dir).await?;
        for (_, name) in &entries {
            let target = extract_dir.join(name);
            if let Some(parent) = target.parent() {
                let normalized = parent
                    .canonicalize()
                    .unwrap_or_else(|_| extract_dir.join(parent));
                if !normalized.starts_with(&canonical_extract) {
                    continue;
                }
            }
            if target.exists() {
                conflicting_files.push(name.clone());
            }
        }
        return Ok(Some(ExtractDryRunResult {
            modpack_name: None,
            conflicting_files,
        }));
    }

    let canonical_extract_dir = tokio::fs::canonicalize(&extract_dir).await?;
    let mut zip_reader = zip_reader;
    for (index, name) in &entries {
        let Some(relative) = safe_extract_relative_path(name) else {
            continue;
        };
        let target = extract_dir.join(relative);

        if !override_conflicts && target.exists() {
            continue;
        }

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
            let canonical_parent = tokio::fs::canonicalize(parent).await?;
            if !canonical_parent.starts_with(&canonical_extract_dir) {
                continue;
            }
        }

        let mut file_bytes = Vec::new();
        let mut entry_reader =
            zip_reader.reader_with_entry(*index).await.map_err(|e| {
                theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                    "Failed to read zip entry: {e}"
                )))
            })?;
        entry_reader
            .read_to_end_checked(&mut file_bytes)
            .await
            .map_err(|e| {
                theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                    "Failed to extract zip entry: {e}"
                )))
            })?;

        tokio::fs::write(&target, &file_bytes).await?;
    }

    Ok(None)
}

#[tauri::command]
pub async fn file_save_as<R: Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: &str,
    file_path: &str,
) -> Result<()> {
    let base = get_full_path(instance_id).await?;
    let source = base.join(file_path);
    let file_name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&file_name)
        .save_file(|path| {
            let _ = tx.send(path);
        });

    if let Some(dest) = rx.await.unwrap_or(None) {
        let dest_path = std::path::PathBuf::try_from(dest).map_err(|e| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Invalid save path: {e}"
            )))
        })?;
        tokio::fs::copy(&source, &dest_path).await?;
    }

    Ok(())
}
