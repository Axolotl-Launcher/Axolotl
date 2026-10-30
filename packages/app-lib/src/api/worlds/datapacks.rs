use base64::Engine;
use chrono::{DateTime, Utc};
use either::Either;
use futures::{StreamExt, stream};
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::SystemTime;
use tokio::task::JoinSet;
use url::Url;

use crate::instance::get_full_path;
use crate::util::io;
use crate::{ErrorKind, Result};

use super::{
    World, WorldDetails, get_world_session_lock, read_world_datapack_state,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatapackKind {
    Folder,
    Zip,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldDatapack {
    pub file_name: String,
    pub display_name: String,
    pub kind: DatapackKind,
    #[serde(flatten)]
    pub metadata: PackMetadata,
    #[serde(
        skip_serializing_if = "Option::is_none",
        with = "either::serde_untagged_optional"
    )]
    pub icon: Option<Either<PathBuf, Url>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldWithDatapacks {
    #[serde(flatten)]
    pub world: World,
    pub datapacks: Vec<WorldDatapack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_format: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_formats: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_format: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_format: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<serde_json::Value>,
}

struct CachedZipDatapackMeta {
    len: u64,
    modified: Option<SystemTime>,
    metadata: PackMetadata,
    icon: Option<Vec<u8>>,
}

static ZIP_DATAPACK_META_CACHE: LazyLock<
    Mutex<HashMap<PathBuf, CachedZipDatapackMeta>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

struct WorldMutationGuard {
    _session: tokio::fs::File,
    _operation: tokio::sync::OwnedMutexGuard<()>,
}

static WORLD_MUTATIONS: LazyLock<
    Mutex<HashMap<PathBuf, Weak<tokio::sync::Mutex<()>>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

async fn lock_world_mutation(world: &Path) -> Result<WorldMutationGuard> {
    let path = tokio::fs::canonicalize(world).await?;
    let lock = {
        let mut locks = WORLD_MUTATIONS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&path).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            locks.insert(path.clone(), Arc::downgrade(&lock));
            lock
        }
    };
    let operation = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        lock.lock_owned(),
    )
    .await
    .map_err(|_| {
        ErrorKind::InputError(
            "World is busy; retry after the current operation finishes".into(),
        )
        .as_error()
    })?;
    let session = get_world_session_lock(&path).await?;
    Ok(WorldMutationGuard {
        _session: session,
        _operation: operation,
    })
}

/// Lists every singleplayer save in the instance together with the datapacks
/// found in each save's `datapacks` folder.
pub async fn list_world_datapacks(
    instance_id: &str,
) -> Result<Vec<WorldWithDatapacks>> {
    let instance_dir = get_full_path(instance_id).await?;
    list_world_datapacks_at(&instance_dir).await
}

async fn list_world_datapacks_at(
    instance_dir: &Path,
) -> Result<Vec<WorldWithDatapacks>> {
    let saves_dir = instance_dir.join("saves");
    if !tokio::fs::try_exists(&saves_dir).await? {
        return Ok(Vec::new());
    }
    let mut directory = io::read_dir(&saves_dir).await?;
    let mut paths = Vec::new();
    while let Some(entry) = directory.next_entry().await? {
        let path = entry.path();
        if !tokio::fs::metadata(&path).await?.is_dir() {
            continue;
        }
        if tokio::fs::try_exists(path.join("level.dat")).await?
            || tokio::fs::try_exists(path.join("datapacks")).await?
        {
            paths.push(path);
        }
    }
    Ok(stream::iter(paths)
        .map(|path| async move {
            let (world, metadata_error) =
                match super::read_singleplayer_world(path.clone()).await {
                    Ok(world) => (world, None),
                    Err(error) => {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        (
                            World {
                                name: name.clone(),
                                last_played: None,
                                icon: None,
                                display_status: super::DisplayStatus::Normal,
                                details: WorldDetails::Singleplayer {
                                    path: name,
                                    game_mode:
                                        super::SingleplayerGameMode::Survival,
                                    hardcore: false,
                                    locked: true,
                                },
                            },
                            Some(error.to_string()),
                        )
                    }
                };
            match read_world_datapacks(path, world.clone()).await {
                Ok(mut result) => {
                    result.state_error = result.state_error.or(metadata_error);
                    if result.state_error.is_some() {
                        for datapack in &mut result.datapacks {
                            datapack.enabled = None;
                        }
                    }
                    result
                }
                Err(error) => WorldWithDatapacks {
                    world,
                    datapacks: vec![],
                    read_error: Some(error.to_string()),
                    state_error: metadata_error,
                },
            }
        })
        .buffer_unordered(8)
        .collect()
        .await)
}

/// Accepts exactly one normal path component, rejecting empty names, `.`,
/// `..`, multi-segment paths and absolute paths.
fn is_single_normal_component(path: &Path) -> bool {
    matches!(
        path.components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    )
}

/// Deletes a datapack (folder or zip) inside a save's `datapacks` folder.
pub async fn delete_world_datapack(
    instance_id: &str,
    world_path: &str,
    file_name: &str,
) -> Result<()> {
    let instance_dir = get_full_path(instance_id).await?;
    delete_world_datapack_at(&instance_dir, world_path, file_name).await
}

async fn delete_world_datapack_at(
    instance_dir: &Path,
    world_path: &str,
    file_name: &str,
) -> Result<()> {
    let world_path = Path::new(world_path);
    if !is_single_normal_component(world_path) {
        return Err(
            ErrorKind::InputError("Invalid world path".into()).as_error()
        );
    }
    let file_name = Path::new(file_name);
    if !is_single_normal_component(file_name) {
        return Err(ErrorKind::InputError("Invalid datapack file name".into())
            .as_error());
    }

    let world_dir = instance_dir.join("saves").join(world_path);
    let _guard = lock_world_mutation(&world_dir).await?;
    let datapacks_dir = world_dir.join("datapacks");
    let target = datapacks_dir.join(file_name);
    if target.parent() != Some(datapacks_dir.as_path()) {
        return Err(ErrorKind::InputError("Invalid datapack file name".into())
            .as_error());
    }

    let meta = io::metadata(&target).await?;
    if meta.is_dir() {
        io::remove_dir_all(&target).await?;
    } else {
        io::remove_file(&target).await?;
    }
    Ok(())
}

/// Enables or disables a datapack by updating the `DataPacks` tag in the
/// world's `level.dat`.
pub async fn set_world_datapack_enabled(
    instance_id: &str,
    world_path: &str,
    file_name: &str,
    enabled: bool,
) -> Result<()> {
    let instance_dir = get_full_path(instance_id).await?;
    set_world_datapack_enabled_at(&instance_dir, world_path, file_name, enabled)
        .await
}

async fn set_world_datapack_enabled_at(
    instance_dir: &Path,
    world_path: &str,
    file_name: &str,
    enabled: bool,
) -> Result<()> {
    let world_path = Path::new(world_path);
    if !is_single_normal_component(world_path) {
        return Err(
            ErrorKind::InputError("Invalid world path".into()).as_error()
        );
    }
    let file_name = Path::new(file_name);
    if !is_single_normal_component(file_name) {
        return Err(ErrorKind::InputError("Invalid datapack file name".into())
            .as_error());
    }

    let world_dir = instance_dir.join("saves").join(world_path);
    let _guard = lock_world_mutation(&world_dir).await?;
    let datapacks_dir = world_dir.join("datapacks");
    let target = datapacks_dir.join(file_name);
    if target.parent() != Some(datapacks_dir.as_path()) {
        return Err(ErrorKind::InputError("Invalid datapack file name".into())
            .as_error());
    }
    if !target.exists() {
        return Err(
            ErrorKind::InputError("Datapack does not exist".into()).as_error()
        );
    }

    let file_id = format!("file/{}", file_name.to_string_lossy());

    let level_dat_path = world_dir.join("level.dat");
    let raw = io::read(&level_dat_path).await?;
    let updated = tokio::task::spawn_blocking(move || {
        let (mut root, _) = quartz_nbt::io::read_nbt(
            &mut Cursor::new(raw),
            quartz_nbt::io::Flavor::GzCompressed,
        )?;
        let data = root.get_mut::<_, &mut NbtCompound>("Data")?;

        if data.get::<_, &NbtCompound>("DataPacks").is_err() {
            data.insert("DataPacks", NbtTag::Compound(NbtCompound::new()));
        }
        let data_packs = data.get_mut::<_, &mut NbtCompound>("DataPacks")?;

        for key in ["Enabled", "Disabled"] {
            if let Ok(list) = data_packs.get_mut::<_, &mut NbtList>(key) {
                list.inner_mut().retain(|tag| {
                    !matches!(tag, NbtTag::String(value) if value == &file_id)
                });
            }
        }

        let key = if enabled { "Enabled" } else { "Disabled" };
        if let Ok(list) = data_packs.get_mut::<_, &mut NbtList>(key) {
            list.push(NbtTag::String(file_id));
        } else {
            let mut list = NbtList::new();
            list.push(NbtTag::String(file_id));
            data_packs.insert(key, list);
        }

        let mut level_data = vec![];
        quartz_nbt::io::write_nbt(
            &mut level_data,
            None,
            &root,
            quartz_nbt::io::Flavor::GzCompressed,
        )?;
        Ok::<_, crate::Error>(level_data)
    })
    .await
    .map_err(|error| {
        ErrorKind::InputError(format!(
            "Datapack state write task failed: {error}"
        ))
        .as_error()
    })??;

    io::write(level_dat_path, updated).await?;
    Ok(())
}

async fn read_world_datapacks(
    world_path: PathBuf,
    world: World,
) -> Result<WorldWithDatapacks> {
    let datapacks_dir = world_path.join("datapacks");
    let mut datapacks = Vec::new();
    let mut state_error = None;
    if tokio::fs::try_exists(&datapacks_dir).await? {
        // Only read the world's level.dat for the enabled/disabled state when it
        // actually has a datapacks folder, and tolerate a missing/corrupt file.
        let (enabled, disabled) =
            match read_world_datapack_state(&world_path).await {
                Ok(state) => state,
                Err(error) => {
                    state_error = Some(error.to_string());
                    (Vec::new(), Vec::new())
                }
            };

        let mut entries = io::read_dir(&datapacks_dir).await?;
        let mut tasks = JoinSet::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if entry.file_type().await?.is_dir() {
                tasks.spawn(read_folder_datapack(
                    path,
                    enabled.clone(),
                    disabled.clone(),
                ));
            } else if is_zip_path(&path) {
                tasks.spawn(read_zip_datapack(
                    path,
                    enabled.clone(),
                    disabled.clone(),
                ));
            }
        }
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Ok(datapack)) => datapacks.push(datapack),
                Ok(Err(error)) => return Err(error),
                Err(error) => {
                    return Err(ErrorKind::InputError(format!(
                        "Datapack read task failed: {error}"
                    ))
                    .as_error());
                }
            }
        }
    }

    Ok(WorldWithDatapacks {
        world,
        datapacks,
        read_error: None,
        state_error,
    })
}

async fn read_folder_datapack(
    dir: PathBuf,
    enabled: Vec<String>,
    disabled: Vec<String>,
) -> Result<WorldDatapack> {
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let metadata = read_folder_pack_meta(&dir).await;

    let icon = if dir.join("pack.png").exists() {
        Some(Either::Left(dir.join("pack.png")))
    } else {
        None
    };

    let size = folder_size(&dir).await.unwrap_or(0);
    let modified = io::metadata(&dir)
        .await
        .ok()
        .and_then(|meta| meta.modified().ok().map(DateTime::<Utc>::from));

    Ok(WorldDatapack {
        file_name: name.clone(),
        display_name: name.clone(),
        kind: DatapackKind::Folder,
        metadata,
        icon,
        enabled: match_datapack_state(&name, &enabled, &disabled),
        size,
        modified,
    })
}

async fn read_zip_datapack(
    path: PathBuf,
    enabled: Vec<String>,
    disabled: Vec<String>,
) -> Result<WorldDatapack> {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let zip_path = path.clone();
    let (metadata, icon_bytes) =
        tokio::task::spawn_blocking(move || read_zip_pack_meta(&zip_path))
            .await
            .map_err(|error| {
                ErrorKind::InputError(format!(
                    "Datapack zip read task failed: {error}"
                ))
                .as_error()
            })??;

    let icon = icon_bytes
        .map(|bytes| {
            Url::parse(&format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ))
        })
        .transpose()
        .ok()
        .flatten();

    let size = io::metadata(&path)
        .await
        .ok()
        .map(|meta| meta.len())
        .unwrap_or(0);
    let modified = io::metadata(&path)
        .await
        .ok()
        .and_then(|meta| meta.modified().ok().map(DateTime::<Utc>::from));

    Ok(WorldDatapack {
        file_name: file_name.clone(),
        display_name: stem.clone(),
        kind: DatapackKind::Zip,
        metadata,
        icon: icon.map(Either::Right),
        enabled: match_datapack_state(&file_name, &enabled, &disabled),
        size,
        modified,
    })
}

async fn read_folder_pack_meta(dir: &Path) -> PackMetadata {
    let Ok(bytes) = io::read(dir.join("pack.mcmeta")).await else {
        return PackMetadata::default();
    };
    parse_pack_mcmeta(&bytes)
}

fn parse_pack_mcmeta(bytes: &[u8]) -> PackMetadata {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return PackMetadata::default();
    };
    let Some(pack) = value.get("pack") else {
        return PackMetadata::default();
    };
    let pack_format = pack
        .get("pack_format")
        .and_then(serde_json::Value::as_i64)
        .and_then(|format| i32::try_from(format).ok())
        .filter(|format| *format >= 0);
    let supported_formats = pack.get("supported_formats").cloned();
    let legacy_range = supported_formats
        .as_ref()
        .and_then(|formats| {
            if let Some(range) = formats.as_object() {
                Some((
                    parse_format(range.get("min_inclusive")?)?,
                    parse_format(range.get("max_inclusive")?)?,
                ))
            } else if let Some(range) = formats.as_array() {
                Some((
                    parse_format(range.first()?)?,
                    parse_format(range.last()?)?,
                ))
            } else {
                let format = parse_format(formats)?;
                Some((format, format))
            }
        })
        .or_else(|| {
            pack_format.map(|format| ([format as u32, 0], [format as u32, 0]))
        });
    PackMetadata {
        pack_format,
        supported_formats,
        min_format: pack
            .get("min_format")
            .and_then(parse_format)
            .or(legacy_range.map(|range| range.0)),
        max_format: pack
            .get("max_format")
            .and_then(parse_max_format)
            .or(legacy_range.map(|range| range.1)),
        description: pack.get("description").cloned(),
    }
}

fn parse_format(value: &serde_json::Value) -> Option<[u32; 2]> {
    let number =
        |value: &serde_json::Value| u32::try_from(value.as_u64()?).ok();
    if let Some(values) = value.as_array() {
        match values.as_slice() {
            [major] => Some([number(major)?, 0]),
            [major, minor] => Some([number(major)?, number(minor)?]),
            _ => None,
        }
    } else {
        Some([number(value)?, 0])
    }
}

fn parse_max_format(value: &serde_json::Value) -> Option<[u32; 2]> {
    let mut format = parse_format(value)?;
    if !value.is_array() || value.as_array()?.len() == 1 {
        format[1] = i32::MAX as u32;
    }
    Some(format)
}

fn read_zip_pack_meta(
    path: &Path,
) -> std::io::Result<(PackMetadata, Option<Vec<u8>>)> {
    let metadata = std::fs::metadata(path)?;
    let signature = (metadata.len(), metadata.modified().ok());
    let cache_key = path.to_path_buf();

    {
        let cache = ZIP_DATAPACK_META_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&cache_key)
            && cached.len == signature.0
            && cached.modified == signature.1
        {
            return Ok((cached.metadata.clone(), cached.icon.clone()));
        }
    }

    let parsed = read_zip_pack_meta_uncached(path)?;

    let mut cache = ZIP_DATAPACK_META_CACHE.lock().unwrap();
    if cache.len() >= 1024 {
        cache.clear();
    }
    cache.insert(
        cache_key,
        CachedZipDatapackMeta {
            len: signature.0,
            modified: signature.1,
            metadata: parsed.0.clone(),
            icon: parsed.1.clone(),
        },
    );

    Ok(parsed)
}

fn read_zip_pack_meta_uncached(
    path: &Path,
) -> std::io::Result<(PackMetadata, Option<Vec<u8>>)> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let metadata = read_zip_entry(&mut archive, "pack.mcmeta")?
        .as_deref()
        .map(parse_pack_mcmeta)
        .unwrap_or_default();
    let icon = read_zip_entry(&mut archive, "pack.png")?;

    Ok((metadata, icon))
}

fn read_zip_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> std::io::Result<Option<Vec<u8>>> {
    if let Ok(mut entry) = archive.by_name(name) {
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        return Ok(Some(bytes));
    }
    let matching = archive
        .file_names()
        .filter_map(|name| name.ok())
        .map(|name| name.into_owned())
        .find(|file_name| file_name.ends_with(&format!("/{name}")));
    if let Some(matching) = matching {
        let mut entry = archive.by_name(&matching)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        return Ok(Some(bytes));
    }
    Ok(None)
}

fn match_datapack_state(
    name: &str,
    enabled: &[String],
    disabled: &[String],
) -> Option<bool> {
    let file_id = format!("file/{name}");
    if enabled.iter().any(|value| value == &file_id) {
        return Some(true);
    }
    if disabled.iter().any(|value| value == &file_id) {
        return Some(false);
    }
    None
}

const MAX_SIZE_SCAN_ENTRIES: usize = 50_000;

async fn folder_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    let mut scanned = 0usize;
    while let Some(current) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current).await?;
        while let Some(entry) = entries.next_entry().await? {
            scanned += 1;
            if scanned > MAX_SIZE_SCAN_ENTRIES {
                return Ok(total);
            }
            if entry.file_type().await?.is_dir() {
                stack.push(entry.path());
            } else {
                total +=
                    entry.metadata().await.map(|meta| meta.len()).unwrap_or(0);
            }
        }
    }
    Ok(total)
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn reads_modern_and_legacy_format_ranges() {
        for (pack, min, max) in [
            (
                json!({"min_format": [121, 0], "max_format": [121, 0]}),
                [121, 0],
                [121, 0],
            ),
            (json!({"pack_format": 48}), [48, 0], [48, 0]),
            (
                json!({"min_format": [107, 1], "max_format": 107}),
                [107, 1],
                [107, i32::MAX as u32],
            ),
            (
                json!({"min_format": 121, "max_format": [121]}),
                [121, 0],
                [121, i32::MAX as u32],
            ),
            (json!({"supported_formats": 48}), [48, 0], [48, 0]),
            (json!({"supported_formats": [48, 61]}), [48, 0], [61, 0]),
            (
                json!({"supported_formats": {"min_inclusive": 48, "max_inclusive": 61}}),
                [48, 0],
                [61, 0],
            ),
            (
                json!({"min_format": 107, "max_format": [121, 1]}),
                [107, 0],
                [121, 1],
            ),
        ] {
            let metadata = parse_pack_mcmeta(
                &serde_json::to_vec(&json!({"pack": pack})).unwrap(),
            );
            assert_eq!(metadata.min_format, Some(min));
            assert_eq!(metadata.max_format, Some(max));
        }
        for invalid in [
            json!(-1),
            json!([121, -1]),
            json!([121, 0, 1]),
            json!(4294967296_u64),
        ] {
            assert_eq!(parse_format(&invalid), None);
        }
    }

    #[tokio::test]
    async fn folder_zip_and_refreshed_cache_preserve_minor_versions() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        for minor in [0, 12] {
            let bytes = serde_json::to_vec(&json!({"pack": {
                "min_format": [121, minor], "max_format": [121, minor], "description": "test"
            }})).unwrap();
            std::fs::write(dir.path().join("pack.mcmeta"), &bytes).unwrap();
            let mut zip =
                zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zip.start_file(
                "pack.mcmeta",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&bytes).unwrap();
            zip.finish().unwrap();
            let folder = read_folder_pack_meta(dir.path()).await;
            let (cached, _) = read_zip_pack_meta(&zip_path).unwrap();
            assert_eq!(folder.min_format, Some([121, minor]));
            assert_eq!(cached.min_format, folder.min_format);
            assert_eq!(
                read_zip_pack_meta(&zip_path).unwrap().0.max_format,
                folder.max_format
            );
        }
    }

    fn world_fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let world = directory.path().join("saves/world");
        std::fs::create_dir_all(world.join("datapacks/first")).unwrap();
        std::fs::create_dir_all(world.join("datapacks/second")).unwrap();
        let mut root = NbtCompound::new();
        root.insert("Data", NbtCompound::new());
        let mut raw = Vec::new();
        quartz_nbt::io::write_nbt(
            &mut raw,
            None,
            &root,
            quartz_nbt::io::Flavor::GzCompressed,
        )
        .unwrap();
        std::fs::write(world.join("level.dat"), raw).unwrap();
        directory
    }

    #[tokio::test]
    async fn concurrent_datapack_toggles_preserve_both_changes() {
        let directory = world_fixture();
        let (first, second) = tokio::join!(
            set_world_datapack_enabled_at(
                directory.path(),
                "world",
                "first",
                true
            ),
            set_world_datapack_enabled_at(
                directory.path(),
                "world",
                "second",
                true
            ),
        );
        first.unwrap();
        second.unwrap();
        let world = directory.path().join("saves/world");
        let (enabled, disabled) =
            read_world_datapack_state(&world).await.unwrap();
        assert!(enabled.contains(&"file/first".into()));
        assert!(enabled.contains(&"file/second".into()));
        assert!(disabled.is_empty());
        let guard = lock_world_mutation(&world).await.unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(30),
                lock_world_mutation(&world.join("."))
            )
            .await
            .is_err()
        );
        drop(guard);
        lock_world_mutation(&world).await.unwrap();
    }

    #[tokio::test]
    async fn minecraft_session_lock_rejects_datapack_writes_without_changes() {
        let directory = world_fixture();
        let world = directory.path().join("saves/world");
        std::fs::write(world.join("session.lock"), b"game-lock").unwrap();
        let game = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(world.join("session.lock"))
            .await
            .unwrap();
        assert!(fs4::tokio::AsyncFileExt::try_lock_exclusive(&game).unwrap());
        let original = std::fs::read(world.join("level.dat")).unwrap();
        assert!(
            set_world_datapack_enabled_at(
                directory.path(),
                "world",
                "first",
                false
            )
            .await
            .is_err()
        );
        assert!(
            delete_world_datapack_at(directory.path(), "world", "first")
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(world.join("level.dat")).unwrap(), original);
        assert!(world.join("datapacks/first").exists());
        drop(game);
        assert_eq!(
            std::fs::read(world.join("session.lock")).unwrap(),
            b"game-lock"
        );
        set_world_datapack_enabled_at(
            directory.path(),
            "world",
            "first",
            false,
        )
        .await
        .unwrap();
        delete_world_datapack_at(directory.path(), "world", "first")
            .await
            .unwrap();
        assert!(!world.join("datapacks/first").exists());
    }

    #[tokio::test]
    async fn zip_datapack_state_uses_full_file_name() {
        let directory = world_fixture();
        let world = directory.path().join("saves/world");
        let zip_path = world.join("datapacks/first.zip");
        let mut zip =
            zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        zip.start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"pack":{"pack_format":48,"description":"test"}}"#)
            .unwrap();
        zip.finish().unwrap();
        set_world_datapack_enabled_at(
            directory.path(),
            "world",
            "first",
            false,
        )
        .await
        .unwrap();
        set_world_datapack_enabled_at(
            directory.path(),
            "world",
            "first.zip",
            true,
        )
        .await
        .unwrap();
        let (enabled, disabled) =
            read_world_datapack_state(&world).await.unwrap();
        assert_eq!(enabled, vec!["file/first.zip"]);
        assert_eq!(disabled, vec!["file/first"]);
        let zip = read_zip_datapack(zip_path, enabled, disabled)
            .await
            .unwrap();
        assert_eq!(zip.enabled, Some(true));
    }

    #[tokio::test]
    async fn world_list_reports_partial_and_state_read_failures() {
        let directory = world_fixture();
        std::fs::write(directory.path().join("saves/.DS_Store"), b"metadata")
            .unwrap();
        let world = directory.path().join("saves/world");
        std::fs::create_dir_all(
            directory.path().join("saves/broken/datapacks"),
        )
        .unwrap();
        std::fs::write(
            directory.path().join("saves/broken/level.dat"),
            b"corrupt",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("saves/broken/datapacks/broken.zip"),
            b"corrupt",
        )
        .unwrap();
        let result = list_world_datapacks_at(directory.path()).await.unwrap();
        assert_eq!(result.len(), 2);
        let good = result.iter().find(|entry| matches!(&entry.world.details, WorldDetails::Singleplayer { path, .. } if path == "world")).unwrap();
        assert_eq!(good.datapacks.len(), 2);
        assert!(good.read_error.is_none());
        let broken = result
            .iter()
            .find(|entry| entry.read_error.is_some())
            .unwrap();
        assert!(broken.state_error.is_some());
        std::fs::write(world.join("level.dat"), b"corrupt").unwrap();
        let result = list_world_datapacks_at(directory.path()).await.unwrap();
        let unknown = result.iter().find(|entry| matches!(&entry.world.details, WorldDetails::Singleplayer { path, .. } if path == "world")).unwrap();
        assert_eq!(unknown.datapacks.len(), 2);
        assert!(unknown.state_error.is_some());
        assert!(
            unknown
                .datapacks
                .iter()
                .all(|entry| entry.enabled.is_none())
        );
    }
}
