use base64::Engine;
use chrono::{DateTime, Utc};
use either::Either;
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;
use tokio::task::JoinSet;
use url::Url;

use crate::instance::get_full_path;
use crate::util::io;
use crate::{ErrorKind, Result};

use super::{
    World, WorldDetails, get_singleplayer_worlds_in_instance,
    read_world_datapack_state,
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

/// Lists every singleplayer save in the instance together with the datapacks
/// found in each save's `datapacks` folder.
pub async fn list_world_datapacks(
    instance_id: &str,
) -> Result<Vec<WorldWithDatapacks>> {
    let instance_dir = get_full_path(instance_id).await?;
    let mut worlds = Vec::new();
    get_singleplayer_worlds_in_instance(&instance_dir, &mut worlds).await?;

    let saves_dir = instance_dir.join("saves");
    let mut tasks = JoinSet::new();
    for world in worlds {
        let world_path = match &world.details {
            WorldDetails::Singleplayer { path, .. } => saves_dir.join(path),
            WorldDetails::Server { .. } => continue,
        };
        tasks.spawn(read_world_datapacks(world_path, world));
    }

    let mut result = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok(item)) => result.push(item),
            Ok(Err(error)) => {
                tracing::warn!("Skipping unreadable world datapacks: {error}");
            }
            Err(error) => {
                tracing::warn!("World datapack read task panicked: {error}");
            }
        }
    }
    Ok(result)
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

    let datapacks_dir = instance_dir
        .join("saves")
        .join(world_path)
        .join("datapacks");
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

    let file_id = if target.is_dir() {
        format!("file/{}", file_name.to_string_lossy())
    } else {
        let stem = file_name.file_stem().unwrap_or_default().to_string_lossy();
        format!("file/{stem}")
    };

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
    if datapacks_dir.exists() {
        // Only read the world's level.dat for the enabled/disabled state when it
        // actually has a datapacks folder, and tolerate a missing/corrupt file.
        let (enabled, disabled) =
            match read_world_datapack_state(&world_path).await {
                Ok(state) => state,
                Err(error) => {
                    tracing::debug!(
                        "Could not read datapack state for world {}: {error}",
                        world.name
                    );
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
                Ok(Err(error)) => {
                    tracing::warn!("Skipping unreadable datapack: {error}");
                }
                Err(error) => {
                    tracing::warn!("Datapack read task panicked: {error}");
                }
            }
        }
    }

    Ok(WorldWithDatapacks { world, datapacks })
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
        file_name,
        display_name: stem.clone(),
        kind: DatapackKind::Zip,
        metadata,
        icon: icon.map(Either::Right),
        enabled: match_datapack_state(&stem, &enabled, &disabled),
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
        .map(str::to_string)
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
}
