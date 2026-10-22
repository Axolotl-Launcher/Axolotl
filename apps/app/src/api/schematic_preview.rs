use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use quartz_nbt::io::Flavor;
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use serde::{Deserialize, Serialize};
use tauri::Runtime;
use tauri::ipc::Response;
use uuid::Uuid;

const CHUNK_SIZE: i32 = 16;
const CHUNK_VOLUME: usize = 16 * 16 * 16;
const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;
const MAX_DECOMPRESSED_SIZE: u64 = 512 * 1024 * 1024;
const MAX_VOLUME: u64 = 128_000_000;
const MAX_PALETTE_ENTRIES: usize = 1_048_576;
const MAX_EDIT_BLOCKS: usize = 250_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SchematicSource {
    External {
        path: String,
    },
    Instance {
        instance_id: String,
        relative_path: String,
    },
    InstanceFile {
        instance_id: String,
        relative_path: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewBlockState {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

impl PreviewBlockState {
    fn air() -> Self {
        Self {
            name: "minecraft:air".to_string(),
            properties: BTreeMap::new(),
        }
    }

    fn key(&self) -> String {
        if self.properties.is_empty() {
            return self.name.clone();
        }
        let properties = self
            .properties
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{}[{properties}]", self.name)
    }

    fn is_air(&self) -> bool {
        matches!(
            self.name.as_str(),
            "minecraft:air"
                | "minecraft:cave_air"
                | "minecraft:void_air"
                | "minecraft:light"
                | "minecraft:barrier"
                | "minecraft:structure_void"
        )
    }
}

fn replacement_state(
    source: &PreviewBlockState,
    target: &PreviewBlockState,
) -> PreviewBlockState {
    let mut state = target.clone();
    for (key, value) in &source.properties {
        if state.properties.contains_key(key) {
            state.properties.insert(key.clone(), value.clone());
        }
    }
    state
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewChunkDescriptor {
    pub position: [i32; 3],
    pub non_air_blocks: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRegion {
    pub id: String,
    pub name: String,
    pub origin: [i32; 3],
    pub size: [u32; 3],
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub block_count: u64,
    pub chunks: Vec<PreviewChunkDescriptor>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMaterial {
    pub name: String,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewManifest {
    pub session_id: String,
    pub file_name: String,
    pub source_path: String,
    pub source_instance_id: Option<String>,
    pub format: String,
    pub format_version: i32,
    pub data_version: Option<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<i64>,
    pub modified_at: Option<i64>,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub size: [u32; 3],
    pub block_count: u64,
    pub entity_count: u64,
    pub block_entity_count: u64,
    pub palette: Vec<PreviewBlockState>,
    pub materials: Vec<PreviewMaterial>,
    pub regions: Vec<PreviewRegion>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSchematicFile {
    pub relative_path: String,
    pub file_name: String,
    pub format: String,
    pub size: u64,
    pub modified_at: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewBlockEdit {
    pub region_id: String,
    pub position: [i32; 3],
    pub palette_index: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewChangedChunk {
    pub region_id: String,
    pub position: [i32; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEditResult {
    pub manifest: PreviewManifest,
    pub changed_chunks: Vec<PreviewChangedChunk>,
    pub applied_palette_indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewTransform {
    RotateClockwise,
    RotateCounterClockwise,
    MirrorX,
    MirrorZ,
}

#[derive(Clone)]
struct PreviewChunk {
    // Transactional session clones share unchanged chunks; edits detach only touched data.
    blocks: Arc<Vec<u32>>,
    non_air_blocks: u32,
}

#[derive(Clone)]
struct SessionRegion {
    manifest: PreviewRegion,
    chunks: HashMap<[i32; 3], PreviewChunk>,
}

#[derive(Clone)]
struct PreviewSession {
    manifest: PreviewManifest,
    regions: Vec<SessionRegion>,
}

static SESSIONS: OnceLock<Mutex<VecDeque<Arc<PreviewSession>>>> =
    OnceLock::new();
static REQUEST_CANCELLATIONS: OnceLock<
    Mutex<HashMap<String, Arc<AtomicBool>>>,
> = OnceLock::new();
fn sessions() -> &'static Mutex<VecDeque<Arc<PreviewSession>>> {
    SESSIONS.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn request_cancellations() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    REQUEST_CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("schematic-preview")
        .invoke_handler(tauri::generate_handler![
            schematic_preview_open,
            schematic_preview_list_instance_files,
            schematic_preview_read_chunk,
            schematic_preview_block_info,
            schematic_preview_apply_edits,
            schematic_preview_transform,
            schematic_preview_export_sponge,
            schematic_preview_export_litematic,
            schematic_preview_cancel,
            schematic_preview_close,
        ])
        .build()
}

#[tauri::command]
pub async fn schematic_preview_open(
    source: SchematicSource,
    request_id: String,
) -> Result<PreviewManifest, String> {
    let cancellation = Arc::new(AtomicBool::new(false));
    request_cancellations()
        .lock()
        .map_err(|_| "Request cancellation storage is unavailable")?
        .insert(request_id.clone(), cancellation.clone());
    let result: Result<PreviewSession, String> = async {
        let (path, source_instance_id) = resolve_source(&source).await?;
        ensure_not_cancelled(Some(&cancellation))?;
        let task_cancellation = cancellation.clone();
        tauri::async_runtime::spawn_blocking(move || {
            parse_schematic(&path, source_instance_id, Some(&task_cancellation))
        })
        .await
        .map_err(|error| error.to_string())?
    }
    .await;
    if let Ok(mut requests) = request_cancellations().lock()
        && requests
            .get(&request_id)
            .is_some_and(|active| Arc::ptr_eq(active, &cancellation))
    {
        requests.remove(&request_id);
    }
    let session = result?;
    let manifest = session.manifest.clone();
    let mut active = sessions()
        .lock()
        .map_err(|_| "Session storage is unavailable")?;
    active.retain(|item| item.manifest.session_id != manifest.session_id);
    active.push_front(Arc::new(session));
    active.truncate(2);
    Ok(manifest)
}

#[tauri::command]
pub async fn schematic_preview_list_instance_files(
    instance_id: String,
) -> Result<Vec<InstanceSchematicFile>, String> {
    let instance_path = theseus::instance::get_full_path(&instance_id)
        .await
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        list_instance_schematics(&instance_path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn schematic_preview_read_chunk(
    session_id: String,
    region_id: String,
    position: [i32; 3],
) -> Result<Response, String> {
    let session = find_session(&session_id)?;
    let region = session
        .regions
        .iter()
        .find(|region| region.manifest.id == region_id)
        .ok_or_else(|| "Unknown schematic region".to_string())?;
    let mut output = Vec::with_capacity(8 + CHUNK_VOLUME * 4);
    output.extend_from_slice(b"SPC1");
    output.extend_from_slice(&(CHUNK_VOLUME as u32).to_le_bytes());
    if let Some(chunk) = region.chunks.get(&position) {
        for value in chunk.blocks.iter() {
            output.extend_from_slice(&value.to_le_bytes());
        }
    } else {
        output.resize(8 + CHUNK_VOLUME * 4, 0);
    }
    Ok(Response::new(output))
}

#[tauri::command]
pub async fn schematic_preview_block_info(
    session_id: String,
    region_id: String,
    position: [i32; 3],
) -> Result<Option<PreviewBlockState>, String> {
    let session = find_session(&session_id)?;
    let region = session
        .regions
        .iter()
        .find(|region| region.manifest.id == region_id)
        .ok_or_else(|| "Unknown schematic region".to_string())?;
    let chunk_position = position.map(|value| value.div_euclid(CHUNK_SIZE));
    let local = position.map(|value| value.rem_euclid(CHUNK_SIZE) as usize);
    let index = local[1] * 256 + local[2] * 16 + local[0];
    let palette_index = region
        .chunks
        .get(&chunk_position)
        .map(|chunk| chunk.blocks[index] as usize)
        .unwrap_or(0);
    Ok(session.manifest.palette.get(palette_index).cloned())
}

#[tauri::command]
pub fn schematic_preview_apply_edits(
    session_id: String,
    edits: Vec<PreviewBlockEdit>,
    target_state: Option<PreviewBlockState>,
) -> Result<PreviewEditResult, String> {
    if edits.len() > MAX_EDIT_BLOCKS {
        return Err(format!(
            "A single edit cannot change more than {MAX_EDIT_BLOCKS} blocks"
        ));
    }
    let mut active = sessions()
        .lock()
        .map_err(|_| "Session storage is unavailable".to_string())?;
    let session_index = active
        .iter()
        .position(|session| session.manifest.session_id == session_id)
        .ok_or_else(|| {
            "The schematic preview session has expired".to_string()
        })?;
    let mut session = (*active[session_index]).clone();
    let target_state = target_state.map(|mut state| {
        state.name = normalize_block_name(&state.name);
        state
    });
    let mut changed = BTreeSet::<(String, [i32; 3])>::new();
    let mut applied_palette_indices = Vec::with_capacity(edits.len());
    for edit in edits {
        let source_palette_index = {
            let region = session
                .regions
                .iter()
                .find(|region| region.manifest.id == edit.region_id)
                .ok_or_else(|| "Unknown schematic region".to_string())?;
            let chunk_position =
                edit.position.map(|value| value.div_euclid(CHUNK_SIZE));
            let local = edit
                .position
                .map(|value| value.rem_euclid(CHUNK_SIZE) as usize);
            let block_index = local[1] * 256 + local[2] * 16 + local[0];
            region
                .chunks
                .get(&chunk_position)
                .map(|chunk| chunk.blocks[block_index])
                .unwrap_or(0)
        };
        let requested_palette_index = if let Some(target) = &target_state {
            let source = session
                .manifest
                .palette
                .get(source_palette_index as usize)
                .ok_or_else(|| {
                    format!(
                        "Unknown schematic palette entry {source_palette_index}"
                    )
                })?;
            let state = replacement_state(source, target);
            let key = state.key();
            if let Some(index) = session
                .manifest
                .palette
                .iter()
                .position(|candidate| candidate.key() == key)
            {
                index as u32
            } else {
                validate_palette_size(session.manifest.palette.len() + 1)?;
                let index = session.manifest.palette.len() as u32;
                session.manifest.palette.push(state);
                index
            }
        } else {
            edit.palette_index
        };
        let palette_index = session
            .manifest
            .palette
            .get(requested_palette_index as usize)
            .ok_or_else(|| {
                format!(
                    "Unknown schematic palette entry {requested_palette_index}"
                )
            })
            .map(|state| {
                if state.is_air() {
                    0
                } else {
                    requested_palette_index
                }
            })?;
        applied_palette_indices.push(palette_index);
        let region = session
            .regions
            .iter_mut()
            .find(|region| region.manifest.id == edit.region_id)
            .ok_or_else(|| "Unknown schematic region".to_string())?;
        if edit.position.iter().enumerate().any(|(axis, value)| {
            *value < region.manifest.min[axis]
                || *value > region.manifest.max[axis]
        }) {
            return Err(
                "Block edit is outside its schematic region".to_string()
            );
        }
        let chunk_position =
            edit.position.map(|value| value.div_euclid(CHUNK_SIZE));
        if region.set_block(edit.position, palette_index) {
            changed.insert((edit.region_id, chunk_position));
        }
    }
    session.refresh_manifest();
    let result = PreviewEditResult {
        manifest: session.manifest.clone(),
        changed_chunks: changed
            .into_iter()
            .map(|(region_id, position)| PreviewChangedChunk {
                region_id,
                position,
            })
            .collect(),
        applied_palette_indices,
    };
    active[session_index] = Arc::new(session);
    Ok(result)
}

#[tauri::command]
pub fn schematic_preview_export_sponge(
    session_id: String,
) -> Result<Response, String> {
    let session = find_session(&session_id)?;
    Ok(Response::new(export_sponge_v3(&session)?))
}

#[tauri::command]
pub fn schematic_preview_export_litematic(
    session_id: String,
) -> Result<Response, String> {
    let session = find_session(&session_id)?;
    Ok(Response::new(export_litematic(&session)?))
}

#[tauri::command]
pub fn schematic_preview_transform(
    session_id: String,
    transform: PreviewTransform,
) -> Result<PreviewManifest, String> {
    let mut active = sessions()
        .lock()
        .map_err(|_| "Session storage is unavailable".to_string())?;
    let session_index = active
        .iter()
        .position(|session| session.manifest.session_id == session_id)
        .ok_or_else(|| {
            "The schematic preview session has expired".to_string()
        })?;
    let mut session = (*active[session_index]).clone();
    session.apply_transform(transform)?;
    let manifest = session.manifest.clone();
    active[session_index] = Arc::new(session);
    Ok(manifest)
}

#[tauri::command]
pub fn schematic_preview_cancel(request_id: String) -> Result<(), String> {
    if let Some(cancellation) = request_cancellations()
        .lock()
        .map_err(|_| "Request cancellation storage is unavailable")?
        .get(&request_id)
    {
        cancellation.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn schematic_preview_close(session_id: String) -> Result<(), String> {
    sessions()
        .lock()
        .map_err(|_| "Session storage is unavailable")?
        .retain(|session| session.manifest.session_id != session_id);
    Ok(())
}

async fn resolve_source(
    source: &SchematicSource,
) -> Result<(PathBuf, Option<String>), String> {
    match source {
        SchematicSource::External { path } => {
            let path = PathBuf::from(path);
            if !path.is_file() {
                return Err("The schematic file does not exist".to_string());
            }
            Ok((path, None))
        }
        SchematicSource::Instance {
            instance_id,
            relative_path,
        } => {
            if Path::new(relative_path)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err("Invalid instance schematic path".to_string());
            }
            let instance_path = theseus::instance::get_full_path(instance_id)
                .await
                .map_err(|error| error.to_string())?;
            let schematics = instance_path.join("schematics");
            let candidate = schematics.join(relative_path);
            let canonical =
                std::fs::canonicalize(&candidate).map_err(|_| {
                    "The instance schematic file does not exist".to_string()
                })?;
            let canonical_root =
                std::fs::canonicalize(&schematics).map_err(|_| {
                    "The instance has no schematics folder".to_string()
                })?;
            if !canonical.starts_with(canonical_root) || !canonical.is_file() {
                return Err("Invalid instance schematic path".to_string());
            }
            Ok((canonical, Some(instance_id.clone())))
        }
        SchematicSource::InstanceFile {
            instance_id,
            relative_path,
        } => {
            if Path::new(relative_path)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err("Invalid instance file path".to_string());
            }
            let instance_path = theseus::instance::get_full_path(instance_id)
                .await
                .map_err(|error| error.to_string())?;
            let candidate = instance_path.join(relative_path);
            let canonical = std::fs::canonicalize(&candidate)
                .map_err(|_| "The instance file does not exist".to_string())?;
            let canonical_root = std::fs::canonicalize(&instance_path)
                .map_err(|_| "The instance does not exist".to_string())?;
            if !canonical.starts_with(canonical_root) || !canonical.is_file() {
                return Err("Invalid instance file path".to_string());
            }
            Ok((canonical, Some(instance_id.clone())))
        }
    }
}

fn parse_schematic(
    path: &Path,
    source_instance_id: Option<String>,
    cancellation: Option<&AtomicBool>,
) -> Result<PreviewSession, String> {
    ensure_not_cancelled(cancellation)?;
    let metadata =
        std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(
            "The schematic is larger than the 256 MiB limit".to_string()
        );
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    ensure_not_cancelled(cancellation)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "schematic" {
        return Err(
            "Legacy MCEdit .schematic files are not supported".to_string()
        );
    }
    let root = read_root(&bytes)?;
    ensure_not_cancelled(cancellation)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("schematic")
        .to_string();
    if root.get::<_, &NbtCompound>("Regions").is_ok() {
        parse_litematic(
            root,
            path.to_string_lossy().to_string(),
            file_name,
            source_instance_id,
            cancellation,
        )
    } else {
        parse_sponge(
            root,
            path.to_string_lossy().to_string(),
            file_name,
            source_instance_id,
            cancellation,
        )
    }
}

fn ensure_not_cancelled(
    cancellation: Option<&AtomicBool>,
) -> Result<(), String> {
    if cancellation.is_some_and(|value| value.load(Ordering::Relaxed)) {
        return Err("Schematic preview request was cancelled".to_string());
    }
    Ok(())
}

fn read_root(bytes: &[u8]) -> Result<NbtCompound, String> {
    let mut decoded = Vec::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut reader = GzDecoder::new(bytes).take(MAX_DECOMPRESSED_SIZE + 1);
        reader.read_to_end(&mut decoded).map_err(|error| {
            format!("Unable to decompress schematic: {error}")
        })?;
        if decoded.len() as u64 > MAX_DECOMPRESSED_SIZE {
            return Err("The decompressed schematic exceeds the 512 MiB limit"
                .to_string());
        }
    } else {
        decoded.extend_from_slice(bytes);
    }
    quartz_nbt::io::read_nbt(&mut Cursor::new(decoded), Flavor::Uncompressed)
        .map(|(root, _)| root)
        .map_err(|error| format!("Unable to read schematic NBT: {error}"))
}

fn parse_litematic(
    root: NbtCompound,
    source_path: String,
    file_name: String,
    source_instance_id: Option<String>,
    cancellation: Option<&AtomicBool>,
) -> Result<PreviewSession, String> {
    let format_version = root.get::<_, i32>("Version").unwrap_or(0);
    if !(4..=7).contains(&format_version) {
        return Err(format!(
            "Unsupported Litematic format version {format_version}"
        ));
    }
    let data_version = root.get::<_, i32>("MinecraftDataVersion").ok();
    let metadata = root.get::<_, &NbtCompound>("Metadata").ok();
    let name = metadata
        .and_then(|value| value.get::<_, &str>("Name").ok())
        .map(str::to_string);
    let description = metadata
        .and_then(|value| value.get::<_, &str>("Description").ok())
        .map(str::to_string);
    let author = metadata
        .and_then(|value| value.get::<_, &str>("Author").ok())
        .map(str::to_string);
    let created_at =
        metadata.and_then(|value| value.get::<_, i64>("TimeCreated").ok());
    let modified_at =
        metadata.and_then(|value| value.get::<_, i64>("TimeModified").ok());
    let regions = root
        .get::<_, &NbtCompound>("Regions")
        .map_err(|_| "Litematic has no Regions compound".to_string())?;
    validate_litematic_regions(regions)?;
    let mut builder = SessionBuilder::new();
    let mut entity_count = 0;
    let mut block_entity_count = 0;
    for (index, (region_name, tag)) in regions.inner().iter().enumerate() {
        ensure_not_cancelled(cancellation)?;
        let NbtTag::Compound(region) = tag else {
            continue;
        };
        let position =
            compound_vec3(region.get::<_, &NbtCompound>("Position").map_err(
                |_| format!("Region {region_name} has no Position"),
            )?)?;
        let signed_size = compound_vec3(
            region
                .get::<_, &NbtCompound>("Size")
                .map_err(|_| format!("Region {region_name} has no Size"))?,
        )?;
        let size = signed_size.map(i32::unsigned_abs);
        let (min, _) = signed_region_bounds(position, signed_size)?;
        let palette_list = region
            .get::<_, &NbtList>("BlockStatePalette")
            .map_err(|_| {
                format!("Region {region_name} has no block palette")
            })?;
        let local_palette = parse_litematic_palette(palette_list)?;
        validate_palette_size(local_palette.len())?;
        let local_to_global = local_palette
            .iter()
            .map(|state| {
                if state.is_air() {
                    0
                } else {
                    builder.palette_index(state.clone())
                }
            })
            .collect::<Vec<_>>();
        let packed = region
            .get::<_, &[i64]>("BlockStates")
            .map_err(|_| format!("Region {region_name} has no BlockStates"))?;
        let volume = volume(size) as usize;
        let bits = if local_palette.len() <= 1 {
            2
        } else {
            ((usize::BITS - (local_palette.len() - 1).leading_zeros()) as usize)
                .max(2)
        };
        let required_longs = (volume * bits).div_ceil(64);
        if packed.len() < required_longs {
            return Err(format!(
                "Region {region_name} has truncated block data"
            ));
        }
        let mut session_region = builder.new_region(
            format!("region-{index}"),
            region_name.to_string(),
            position,
            size,
            min,
        )?;
        // BlockStates always starts at the minimum corner, even when Size is negative.
        for block_index in 0..volume {
            if block_index % CHUNK_VOLUME == 0 {
                ensure_not_cancelled(cancellation)?;
            }
            let palette_index =
                unpack_litematic_value(packed, block_index, bits) as usize;
            let global_index = *local_to_global.get(palette_index).ok_or_else(|| {
                format!("Region {region_name} references palette entry {palette_index}")
            })?;
            if !builder.palette[global_index as usize].is_air() {
                let x = block_index % size[0] as usize;
                let z = (block_index / size[0] as usize) % size[2] as usize;
                let y = block_index / (size[0] as usize * size[2] as usize);
                session_region.put_block(
                    litematic_block_position(min, [x, y, z]),
                    global_index,
                );
                builder.record_material(global_index);
            }
        }
        entity_count += nbt_list_len(region, "Entities") as u64;
        block_entity_count += nbt_list_len(region, "TileEntities") as u64;
        builder.finish_region(session_region);
    }
    builder.finish(ManifestMetadata {
        source_path,
        source_instance_id,
        file_name,
        format: "litematic".to_string(),
        format_version,
        data_version,
        name,
        description,
        author,
        created_at,
        modified_at,
        entity_count,
        block_entity_count,
    })
}

fn parse_sponge(
    root: NbtCompound,
    source_path: String,
    file_name: String,
    source_instance_id: Option<String>,
    cancellation: Option<&AtomicBool>,
) -> Result<PreviewSession, String> {
    let schematic = root.get::<_, &NbtCompound>("Schematic").unwrap_or(&root);
    let format_version = schematic
        .get::<_, i32>("Version")
        .map_err(|_| "This is not a supported Sponge schematic".to_string())?;
    if !matches!(format_version, 2 | 3) {
        return Err(format!(
            "Unsupported Sponge schematic version {format_version}"
        ));
    }
    let dimensions = [
        schematic
            .get::<_, i16>("Width")
            .map_err(|_| "Sponge schematic has no Width".to_string())?
            as i32,
        schematic
            .get::<_, i16>("Height")
            .map_err(|_| "Sponge schematic has no Height".to_string())?
            as i32,
        schematic
            .get::<_, i16>("Length")
            .map_err(|_| "Sponge schematic has no Length".to_string())?
            as i32,
    ];
    if dimensions.iter().any(|value| *value <= 0) {
        return Err("Sponge schematic dimensions must be positive".to_string());
    }
    let size = dimensions.map(|value| value as u32);
    validate_volume(size)?;
    let offset = schematic
        .get::<_, &[i32]>("Offset")
        .ok()
        .filter(|value| value.len() >= 3)
        .map(|value| [value[0], value[1], value[2]])
        .unwrap_or([0, 0, 0]);
    let block_container = if format_version == 3 {
        schematic.get::<_, &NbtCompound>("Blocks").map_err(|_| {
            "Sponge v3 schematic has no Blocks compound".to_string()
        })?
    } else {
        schematic
    };
    let palette_compound = block_container
        .get::<_, &NbtCompound>("Palette")
        .map_err(|_| "Sponge schematic has no Palette".to_string())?;
    let maximum_palette_index = palette_compound
        .inner()
        .values()
        .filter_map(|value| match value {
            NbtTag::Int(value) if *value >= 0 => Some(*value as usize),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    validate_palette_size(maximum_palette_index.saturating_add(1))?;
    let mut local_palette =
        vec![PreviewBlockState::air(); maximum_palette_index + 1];
    for (state, value) in palette_compound.inner() {
        if let NbtTag::Int(index) = value
            && *index >= 0
        {
            local_palette[*index as usize] = parse_state_string(state);
        }
    }
    let mut builder = SessionBuilder::new();
    let local_to_global = local_palette
        .iter()
        .map(|state| {
            if state.is_air() {
                0
            } else {
                builder.palette_index(state.clone())
            }
        })
        .collect::<Vec<_>>();
    let block_data = block_container
        .get::<_, &Vec<i8>>("BlockData")
        .or(block_container.get::<_, &Vec<i8>>("Data"))
        .map_err(|_| "Sponge schematic has no block data".to_string())?;
    let block_data_bytes = block_data
        .iter()
        .map(|value| *value as u8)
        .collect::<Vec<_>>();
    let mut bytes = block_data_bytes.as_slice();
    let mut region = builder.new_region(
        "region-0".to_string(),
        "Main".to_string(),
        offset,
        size,
        offset,
    )?;
    let total = volume(size) as usize;
    for index in 0..total {
        if index % CHUNK_VOLUME == 0 {
            ensure_not_cancelled(cancellation)?;
        }
        let palette_index = read_varint(&mut bytes)? as usize;
        let global_index =
            *local_to_global.get(palette_index).ok_or_else(|| {
                format!(
                    "Sponge schematic references palette entry {palette_index}"
                )
            })?;
        if !builder.palette[global_index as usize].is_air() {
            let x = index % size[0] as usize;
            let z = (index / size[0] as usize) % size[2] as usize;
            let y = index / (size[0] as usize * size[2] as usize);
            region.put_block(
                [
                    offset[0] + x as i32,
                    offset[1] + y as i32,
                    offset[2] + z as i32,
                ],
                global_index,
            );
            builder.record_material(global_index);
        }
    }
    builder.finish_region(region);
    let metadata = schematic.get::<_, &NbtCompound>("Metadata").ok();
    builder.finish(ManifestMetadata {
        source_path,
        source_instance_id,
        file_name,
        format: format!("schem_v{format_version}"),
        format_version,
        data_version: schematic.get::<_, i32>("DataVersion").ok(),
        name: metadata
            .and_then(|value| value.get::<_, &str>("Name").ok())
            .map(str::to_string),
        description: None,
        author: None,
        created_at: None,
        modified_at: None,
        entity_count: nbt_list_len(schematic, "Entities") as u64,
        block_entity_count: nbt_list_len(block_container, "BlockEntities")
            as u64,
    })
}

struct SessionBuilder {
    palette: Vec<PreviewBlockState>,
    palette_lookup: HashMap<String, u32>,
    material_counts: HashMap<u32, u64>,
    regions: Vec<SessionRegion>,
}

struct ManifestMetadata {
    source_path: String,
    source_instance_id: Option<String>,
    file_name: String,
    format: String,
    format_version: i32,
    data_version: Option<i32>,
    name: Option<String>,
    description: Option<String>,
    author: Option<String>,
    created_at: Option<i64>,
    modified_at: Option<i64>,
    entity_count: u64,
    block_entity_count: u64,
}

impl SessionBuilder {
    fn new() -> Self {
        let air = PreviewBlockState::air();
        Self {
            palette: vec![air.clone()],
            palette_lookup: HashMap::from([(air.key(), 0)]),
            material_counts: HashMap::new(),
            regions: Vec::new(),
        }
    }

    fn palette_index(&mut self, state: PreviewBlockState) -> u32 {
        let key = state.key();
        if let Some(index) = self.palette_lookup.get(&key) {
            return *index;
        }
        let index = self.palette.len() as u32;
        self.palette.push(state);
        self.palette_lookup.insert(key, index);
        index
    }

    fn record_material(&mut self, palette_index: u32) {
        *self.material_counts.entry(palette_index).or_default() += 1;
    }

    fn new_region(
        &self,
        id: String,
        name: String,
        origin: [i32; 3],
        size: [u32; 3],
        min: [i32; 3],
    ) -> Result<SessionRegion, String> {
        let max = checked_region_max(min, size)?;
        Ok(SessionRegion {
            manifest: PreviewRegion {
                id,
                name,
                origin,
                size,
                min,
                max,
                block_count: 0,
                chunks: Vec::new(),
            },
            chunks: HashMap::new(),
        })
    }

    fn finish_region(&mut self, mut region: SessionRegion) {
        region.manifest.block_count = region
            .chunks
            .values()
            .map(|chunk| chunk.non_air_blocks as u64)
            .sum();
        let mut chunks = region
            .chunks
            .iter()
            .map(|(position, chunk)| PreviewChunkDescriptor {
                position: *position,
                non_air_blocks: chunk.non_air_blocks,
            })
            .collect::<Vec<_>>();
        chunks.sort_by_key(|chunk| {
            (chunk.position[1], chunk.position[2], chunk.position[0])
        });
        region.manifest.chunks = chunks;
        self.regions.push(region);
    }

    fn finish(
        self,
        metadata: ManifestMetadata,
    ) -> Result<PreviewSession, String> {
        if self.regions.is_empty() {
            return Err("The schematic contains no regions".to_string());
        }
        let min = [
            self.regions
                .iter()
                .map(|region| region.manifest.min[0])
                .min()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.min[1])
                .min()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.min[2])
                .min()
                .unwrap_or(0),
        ];
        let max = [
            self.regions
                .iter()
                .map(|region| region.manifest.max[0])
                .max()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.max[1])
                .max()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.max[2])
                .max()
                .unwrap_or(0),
        ];
        let mut size = [0u32; 3];
        for axis in 0..3 {
            size[axis] = (max[axis] as i64 - min[axis] as i64 + 1)
                .try_into()
                .map_err(|_| {
                "Schematic coordinate span exceeds supported bounds".to_string()
            })?;
        }
        let block_count = self
            .regions
            .iter()
            .map(|region| region.manifest.block_count)
            .sum();
        let mut materials_by_name = HashMap::<String, u64>::new();
        for (index, count) in &self.material_counts {
            if let Some(state) = self.palette.get(*index as usize) {
                *materials_by_name.entry(state.name.clone()).or_default() +=
                    count;
            }
        }
        let mut materials = materials_by_name
            .into_iter()
            .map(|(name, count)| PreviewMaterial { name, count })
            .collect::<Vec<_>>();
        materials.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.name.cmp(&right.name))
        });
        let session_id = Uuid::new_v4().to_string();
        let manifest = PreviewManifest {
            session_id,
            file_name: metadata.file_name,
            source_path: metadata.source_path,
            source_instance_id: metadata.source_instance_id,
            format: metadata.format,
            format_version: metadata.format_version,
            data_version: metadata.data_version,
            name: metadata.name,
            description: metadata.description,
            author: metadata.author,
            created_at: metadata.created_at,
            modified_at: metadata.modified_at,
            min,
            max,
            size,
            block_count,
            entity_count: metadata.entity_count,
            block_entity_count: metadata.block_entity_count,
            palette: self.palette,
            materials,
            regions: self
                .regions
                .iter()
                .map(|region| region.manifest.clone())
                .collect(),
            warnings: Vec::new(),
        };
        Ok(PreviewSession {
            manifest,
            regions: self.regions,
        })
    }
}

impl SessionRegion {
    fn put_block(&mut self, position: [i32; 3], palette_index: u32) {
        let chunk_position = position.map(|value| value.div_euclid(CHUNK_SIZE));
        let local = position.map(|value| value.rem_euclid(CHUNK_SIZE) as usize);
        let index = local[1] * 256 + local[2] * 16 + local[0];
        let chunk =
            self.chunks
                .entry(chunk_position)
                .or_insert_with(|| PreviewChunk {
                    blocks: Arc::new(vec![0; CHUNK_VOLUME]),
                    non_air_blocks: 0,
                });
        if chunk.blocks[index] == 0 {
            chunk.non_air_blocks += 1;
        }
        Arc::make_mut(&mut chunk.blocks)[index] = palette_index;
    }

    fn block_at(&self, position: [i32; 3]) -> u32 {
        let chunk_position = position.map(|value| value.div_euclid(CHUNK_SIZE));
        let local = position.map(|value| value.rem_euclid(CHUNK_SIZE) as usize);
        let index = local[1] * 256 + local[2] * 16 + local[0];
        self.chunks
            .get(&chunk_position)
            .map(|chunk| chunk.blocks[index])
            .unwrap_or(0)
    }

    fn set_block(&mut self, position: [i32; 3], palette_index: u32) -> bool {
        let chunk_position = position.map(|value| value.div_euclid(CHUNK_SIZE));
        let local = position.map(|value| value.rem_euclid(CHUNK_SIZE) as usize);
        let index = local[1] * 256 + local[2] * 16 + local[0];
        if palette_index == 0 {
            let Some(chunk) = self.chunks.get_mut(&chunk_position) else {
                return false;
            };
            if chunk.blocks[index] == 0 {
                return false;
            }
            Arc::make_mut(&mut chunk.blocks)[index] = 0;
            chunk.non_air_blocks = chunk.non_air_blocks.saturating_sub(1);
            if chunk.non_air_blocks == 0 {
                self.chunks.remove(&chunk_position);
            }
            return true;
        }
        let chunk =
            self.chunks
                .entry(chunk_position)
                .or_insert_with(|| PreviewChunk {
                    blocks: Arc::new(vec![0; CHUNK_VOLUME]),
                    non_air_blocks: 0,
                });
        if chunk.blocks[index] == palette_index {
            return false;
        }
        if chunk.blocks[index] == 0 {
            chunk.non_air_blocks += 1;
        }
        Arc::make_mut(&mut chunk.blocks)[index] = palette_index;
        true
    }
}

impl PreviewSession {
    fn refresh_manifest(&mut self) {
        let mut material_counts = HashMap::<String, u64>::new();
        let mut block_count = 0u64;
        for region in &mut self.regions {
            region.manifest.block_count = region
                .chunks
                .values()
                .map(|chunk| chunk.non_air_blocks as u64)
                .sum();
            block_count += region.manifest.block_count;
            region.manifest.chunks = region
                .chunks
                .iter()
                .filter(|(_, chunk)| chunk.non_air_blocks > 0)
                .map(|(position, chunk)| PreviewChunkDescriptor {
                    position: *position,
                    non_air_blocks: chunk.non_air_blocks,
                })
                .collect();
            region.manifest.chunks.sort_by_key(|chunk| {
                (chunk.position[1], chunk.position[2], chunk.position[0])
            });
            for chunk in region.chunks.values() {
                for palette_index in chunk.blocks.iter().copied() {
                    let Some(state) =
                        self.manifest.palette.get(palette_index as usize)
                    else {
                        continue;
                    };
                    if !state.is_air() {
                        *material_counts
                            .entry(state.name.clone())
                            .or_default() += 1;
                    }
                }
            }
        }
        self.manifest.block_count = block_count;
        self.manifest.regions = self
            .regions
            .iter()
            .map(|region| region.manifest.clone())
            .collect();
        self.manifest.materials = material_counts
            .into_iter()
            .map(|(name, count)| PreviewMaterial { name, count })
            .collect();
        self.manifest.materials.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.name.cmp(&right.name))
        });
    }

    fn apply_transform(
        &mut self,
        transform: PreviewTransform,
    ) -> Result<(), String> {
        let transform_min = self.manifest.min;
        let transform_max = self.manifest.max;
        for region in &mut self.regions {
            let previous = region.clone();
            let mut transformed = SessionRegion {
                manifest: previous.manifest.clone(),
                chunks: HashMap::new(),
            };
            for (chunk_position, chunk) in &previous.chunks {
                for (index, palette_index) in
                    chunk.blocks.iter().copied().enumerate()
                {
                    if palette_index == 0 {
                        continue;
                    }
                    let position = [
                        chunk_position[0] * CHUNK_SIZE + (index % 16) as i32,
                        chunk_position[1] * CHUNK_SIZE + (index / 256) as i32,
                        chunk_position[2] * CHUNK_SIZE
                            + ((index / 16) % 16) as i32,
                    ];
                    transformed.put_block(
                        transform_position(
                            position,
                            transform_min,
                            transform_max,
                            transform,
                        )?,
                        palette_index,
                    );
                }
            }
            let corners =
                region_corners(previous.manifest.min, previous.manifest.max)
                    .into_iter()
                    .map(|position| {
                        transform_position(
                            position,
                            transform_min,
                            transform_max,
                            transform,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            transformed.manifest.min = [
                corners.iter().map(|point| point[0]).min().unwrap_or(0),
                corners.iter().map(|point| point[1]).min().unwrap_or(0),
                corners.iter().map(|point| point[2]).min().unwrap_or(0),
            ];
            transformed.manifest.max = [
                corners.iter().map(|point| point[0]).max().unwrap_or(0),
                corners.iter().map(|point| point[1]).max().unwrap_or(0),
                corners.iter().map(|point| point[2]).max().unwrap_or(0),
            ];
            transformed.manifest.origin = transform_position(
                previous.manifest.origin,
                transform_min,
                transform_max,
                transform,
            )?;
            transformed.manifest.size = [
                (transformed.manifest.max[0] - transformed.manifest.min[0] + 1)
                    as u32,
                (transformed.manifest.max[1] - transformed.manifest.min[1] + 1)
                    as u32,
                (transformed.manifest.max[2] - transformed.manifest.min[2] + 1)
                    as u32,
            ];
            *region = transformed;
        }
        self.manifest.min = [
            self.regions
                .iter()
                .map(|region| region.manifest.min[0])
                .min()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.min[1])
                .min()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.min[2])
                .min()
                .unwrap_or(0),
        ];
        self.manifest.max = [
            self.regions
                .iter()
                .map(|region| region.manifest.max[0])
                .max()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.max[1])
                .max()
                .unwrap_or(0),
            self.regions
                .iter()
                .map(|region| region.manifest.max[2])
                .max()
                .unwrap_or(0),
        ];
        self.manifest.size = [
            (self.manifest.max[0] - self.manifest.min[0] + 1) as u32,
            (self.manifest.max[1] - self.manifest.min[1] + 1) as u32,
            (self.manifest.max[2] - self.manifest.min[2] + 1) as u32,
        ];
        self.refresh_manifest();
        Ok(())
    }
}

fn region_corners(min: [i32; 3], max: [i32; 3]) -> [[i32; 3]; 8] {
    [
        [min[0], min[1], min[2]],
        [min[0], min[1], max[2]],
        [min[0], max[1], min[2]],
        [min[0], max[1], max[2]],
        [max[0], min[1], min[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], min[2]],
        [max[0], max[1], max[2]],
    ]
}

fn transform_position(
    position: [i32; 3],
    min: [i32; 3],
    max: [i32; 3],
    transform: PreviewTransform,
) -> Result<[i32; 3], String> {
    let x = position[0] as i64;
    let z = position[2] as i64;
    let min_x = min[0] as i64;
    let min_z = min[2] as i64;
    let max_x = max[0] as i64;
    let max_z = max[2] as i64;
    let (new_x, new_z) = match transform {
        PreviewTransform::RotateClockwise => {
            (min_x + z - min_z, min_z + max_x - x)
        }
        PreviewTransform::RotateCounterClockwise => {
            (min_x + max_z - z, min_z + x - min_x)
        }
        PreviewTransform::MirrorX => (min_x + max_x - x, z),
        PreviewTransform::MirrorZ => (x, min_z + max_z - z),
    };
    Ok([
        new_x
            .try_into()
            .map_err(|_| "Transformed X coordinate is out of range")?,
        position[1],
        new_z
            .try_into()
            .map_err(|_| "Transformed Z coordinate is out of range")?,
    ])
}

fn write_varint(mut value: u32, output: &mut Vec<i8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte as i8);
        if value == 0 {
            return;
        }
    }
}

fn litematic_vec3(values: [i32; 3]) -> NbtCompound {
    let mut compound = NbtCompound::new();
    compound.insert("x", values[0]);
    compound.insert("y", values[1]);
    compound.insert("z", values[2]);
    compound
}

fn litematic_palette_entry(state: &PreviewBlockState) -> NbtCompound {
    let mut entry = NbtCompound::new();
    entry.insert("Name", state.name.as_str());
    if !state.properties.is_empty() {
        let mut properties = NbtCompound::new();
        for (key, value) in &state.properties {
            properties.insert(key.as_str(), value.as_str());
        }
        entry.insert("Properties", properties);
    }
    entry
}

fn pack_litematic_values(values: &[u32], bits: usize) -> Vec<i64> {
    let mut packed = vec![0u64; (values.len() * bits).div_ceil(64)];
    let mask = (1u64 << bits) - 1;
    for (index, value) in values.iter().copied().enumerate() {
        let bit_index = index * bits;
        let start = bit_index / 64;
        let offset = bit_index % 64;
        let value = (value as u64) & mask;
        packed[start] |= value << offset;
        if offset + bits > 64 {
            packed[start + 1] |= value >> (64 - offset);
        }
    }
    packed.into_iter().map(|value| value as i64).collect()
}

fn export_litematic(session: &PreviewSession) -> Result<Vec<u8>, String> {
    let mut regions = NbtCompound::new();
    let mut total_volume = 0u64;
    for region in &session.regions {
        let region_volume = volume(region.manifest.size);
        total_volume = total_volume.saturating_add(region_volume);
        let mut local_palette = vec![0u32];
        let mut local_lookup = HashMap::from([(0u32, 0u32)]);
        let mut block_states = Vec::with_capacity(region_volume as usize);
        for y in region.manifest.min[1]..=region.manifest.max[1] {
            for z in region.manifest.min[2]..=region.manifest.max[2] {
                for x in region.manifest.min[0]..=region.manifest.max[0] {
                    let global_index = region.block_at([x, y, z]);
                    if session
                        .manifest
                        .palette
                        .get(global_index as usize)
                        .is_none()
                    {
                        return Err(format!(
                            "Region {} references unknown palette entry {global_index}",
                            region.manifest.name
                        ));
                    }
                    let local_index =
                        if let Some(index) = local_lookup.get(&global_index) {
                            *index
                        } else {
                            let index = local_palette.len() as u32;
                            local_palette.push(global_index);
                            local_lookup.insert(global_index, index);
                            index
                        };
                    block_states.push(local_index);
                }
            }
        }
        let mut palette = NbtList::new();
        for global_index in local_palette {
            palette.push(litematic_palette_entry(
                &session.manifest.palette[global_index as usize],
            ));
        }
        let bits = if palette.len() <= 1 {
            2
        } else {
            ((usize::BITS - (palette.len() - 1).leading_zeros()) as usize)
                .max(2)
        };
        let mut exported_region = NbtCompound::new();
        exported_region.insert("Position", litematic_vec3(region.manifest.min));
        exported_region.insert(
            "Size",
            litematic_vec3(region.manifest.size.map(|value| value as i32)),
        );
        exported_region.insert("BlockStatePalette", palette);
        exported_region.insert(
            "BlockStates",
            NbtTag::LongArray(pack_litematic_values(&block_states, bits)),
        );
        exported_region.insert("Entities", NbtList::new());
        exported_region.insert("TileEntities", NbtList::new());
        exported_region.insert("PendingBlockTicks", NbtList::new());
        exported_region.insert("PendingFluidTicks", NbtList::new());
        regions.insert(region.manifest.name.as_str(), exported_region);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default();
    let mut metadata = NbtCompound::new();
    metadata.insert(
        "Name",
        session
            .manifest
            .name
            .as_deref()
            .unwrap_or(&session.manifest.file_name),
    );
    metadata.insert(
        "Author",
        session
            .manifest
            .author
            .as_deref()
            .unwrap_or("Axolotl Launcher"),
    );
    metadata.insert(
        "Description",
        session.manifest.description.as_deref().unwrap_or_default(),
    );
    metadata.insert("RegionCount", session.regions.len() as i32);
    metadata.insert("TimeCreated", session.manifest.created_at.unwrap_or(now));
    metadata.insert("TimeModified", now);
    metadata.insert("TotalBlocks", session.manifest.block_count as i64);
    metadata.insert("TotalVolume", total_volume as i64);
    metadata.insert(
        "EnclosingSize",
        litematic_vec3(session.manifest.size.map(|value| value as i32)),
    );

    let mut root = NbtCompound::new();
    root.insert("Version", 6i32);
    root.insert("SubVersion", 1i32);
    root.insert(
        "MinecraftDataVersion",
        session.manifest.data_version.unwrap_or_default(),
    );
    root.insert("Metadata", metadata);
    root.insert("Regions", regions);
    let mut output = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut output,
        Some("Litematic"),
        &root,
        Flavor::GzCompressed,
    )
    .map_err(|error| format!("Unable to export Litematic: {error}"))?;
    Ok(output)
}

fn export_sponge_v3(session: &PreviewSession) -> Result<Vec<u8>, String> {
    let dimensions = session.manifest.size;
    if dimensions
        .iter()
        .any(|value| *value == 0 || *value > i16::MAX as u32)
    {
        return Err(
            "Sponge export supports dimensions between 1 and 32767 blocks"
                .to_string(),
        );
    }
    let mut palette = NbtCompound::new();
    for (index, state) in session.manifest.palette.iter().enumerate() {
        palette.insert(state.key(), index as i32);
    }
    let mut block_data = Vec::new();
    for y in session.manifest.min[1]..=session.manifest.max[1] {
        for z in session.manifest.min[2]..=session.manifest.max[2] {
            for x in session.manifest.min[0]..=session.manifest.max[0] {
                let position = [x, y, z];
                let mut palette_index = 0;
                for region in &session.regions {
                    if position.iter().enumerate().all(|(axis, value)| {
                        *value >= region.manifest.min[axis]
                            && *value <= region.manifest.max[axis]
                    }) {
                        let candidate = region.block_at(position);
                        if candidate != 0 {
                            palette_index = candidate;
                        }
                    }
                }
                write_varint(palette_index, &mut block_data);
            }
        }
    }
    let mut blocks = NbtCompound::new();
    blocks.insert("Palette", palette);
    blocks.insert("PaletteMax", session.manifest.palette.len() as i32);
    blocks.insert("BlockData", NbtTag::ByteArray(block_data));
    let mut metadata = NbtCompound::new();
    metadata.insert(
        "Name",
        session
            .manifest
            .name
            .as_deref()
            .unwrap_or(&session.manifest.file_name),
    );
    if let Some(author) = &session.manifest.author {
        metadata.insert("Author", author.as_str());
    }
    let mut schematic = NbtCompound::new();
    schematic.insert("Version", 3i32);
    schematic.insert(
        "DataVersion",
        session.manifest.data_version.unwrap_or_default(),
    );
    schematic.insert("Width", dimensions[0] as i16);
    schematic.insert("Height", dimensions[1] as i16);
    schematic.insert("Length", dimensions[2] as i16);
    schematic.insert("Offset", NbtTag::IntArray(session.manifest.min.to_vec()));
    schematic.insert("Metadata", metadata);
    schematic.insert("Blocks", blocks);
    let mut root = NbtCompound::new();
    root.insert("Schematic", schematic);
    let mut output = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut output,
        Some("Schematic"),
        &root,
        Flavor::GzCompressed,
    )
    .map_err(|error| format!("Unable to export Sponge schematic: {error}"))?;
    Ok(output)
}

fn find_session(session_id: &str) -> Result<Arc<PreviewSession>, String> {
    sessions()
        .lock()
        .map_err(|_| "Session storage is unavailable".to_string())?
        .iter()
        .find(|session| session.manifest.session_id == session_id)
        .cloned()
        .ok_or_else(|| "The schematic preview session has expired".to_string())
}

fn compound_vec3(compound: &NbtCompound) -> Result<[i32; 3], String> {
    Ok([
        compound
            .get::<_, i32>("x")
            .map_err(|_| "Missing x coordinate".to_string())?,
        compound
            .get::<_, i32>("y")
            .map_err(|_| "Missing y coordinate".to_string())?,
        compound
            .get::<_, i32>("z")
            .map_err(|_| "Missing z coordinate".to_string())?,
    ])
}

fn validate_litematic_regions(regions: &NbtCompound) -> Result<(), String> {
    let mut declared_volume = 0u64;
    for (region_name, tag) in regions.inner() {
        let NbtTag::Compound(region) = tag else {
            continue;
        };
        let position =
            compound_vec3(region.get::<_, &NbtCompound>("Position").map_err(
                |_| format!("Region {region_name} has no Position"),
            )?)?;
        let signed_size = compound_vec3(
            region
                .get::<_, &NbtCompound>("Size")
                .map_err(|_| format!("Region {region_name} has no Size"))?,
        )?;
        let size = signed_size.map(i32::unsigned_abs);
        validate_volume(size)?;
        signed_region_bounds(position, signed_size)?;
        declared_volume = declared_volume
            .checked_add(volume(size))
            .ok_or_else(|| "Schematic volume overflowed".to_string())?;
        if declared_volume > MAX_VOLUME {
            return Err(format!(
                "Schematic volume {declared_volume} exceeds the {MAX_VOLUME} block limit"
            ));
        }
    }
    Ok(())
}

fn normalize_block_name(value: &str) -> String {
    let name = value.trim().to_ascii_lowercase();
    if name.contains(':') {
        name
    } else {
        format!("minecraft:{name}")
    }
}

fn parse_litematic_palette(
    palette: &NbtList,
) -> Result<Vec<PreviewBlockState>, String> {
    let mut states = Vec::new();
    for tag in palette {
        let NbtTag::Compound(compound) = tag else {
            return Err(
                "Litematic palette contains a non-compound entry".to_string()
            );
        };
        let name = compound
            .get::<_, &str>("Name")
            .map_err(|_| "Litematic palette entry has no Name".to_string())?
            .to_string();
        let name = normalize_block_name(&name);
        let properties = compound
            .get::<_, &NbtCompound>("Properties")
            .ok()
            .map(|properties| {
                properties
                    .inner()
                    .iter()
                    .filter_map(|(key, value)| match value {
                        NbtTag::String(value) => {
                            Some((key.clone(), value.clone()))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        states.push(PreviewBlockState { name, properties });
    }
    if states.is_empty() {
        return Err("Litematic palette is empty".to_string());
    }
    Ok(states)
}

fn parse_state_string(value: &str) -> PreviewBlockState {
    let (name, properties) = value
        .split_once('[')
        .map(|(name, properties)| {
            let properties = properties
                .trim_end_matches(']')
                .split(',')
                .filter_map(|property| property.split_once('='))
                .map(|(key, value)| {
                    (key.trim().to_string(), value.trim().to_string())
                })
                .collect();
            (name, properties)
        })
        .unwrap_or((value, BTreeMap::new()));
    PreviewBlockState {
        name: normalize_block_name(name),
        properties,
    }
}

fn unpack_litematic_value(values: &[i64], index: usize, bits: usize) -> u64 {
    let bit_index = index * bits;
    let start = bit_index / 64;
    let offset = bit_index % 64;
    let mask = (1u64 << bits) - 1;
    if offset + bits <= 64 {
        ((values[start] as u64) >> offset) & mask
    } else {
        let low = (values[start] as u64) >> offset;
        let high = values[start + 1] as u64;
        (low | (high << (64 - offset))) & mask
    }
}

fn litematic_block_position(min: [i32; 3], local: [usize; 3]) -> [i32; 3] {
    [
        min[0] + local[0] as i32,
        min[1] + local[1] as i32,
        min[2] + local[2] as i32,
    ]
}

fn read_varint(bytes: &mut &[u8]) -> Result<u32, String> {
    let mut result = 0u32;
    for shift in (0..35).step_by(7) {
        let Some((&byte, remaining)) = bytes.split_first() else {
            return Err("Sponge block data is truncated".to_string());
        };
        *bytes = remaining;
        result |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err("Sponge block data contains an invalid VarInt".to_string())
}

fn nbt_list_len(compound: &NbtCompound, key: &str) -> usize {
    compound
        .get::<_, &NbtList>(key)
        .map(NbtList::len)
        .unwrap_or(0)
}

fn volume(size: [u32; 3]) -> u64 {
    size[0] as u64 * size[1] as u64 * size[2] as u64
}

fn signed_region_bounds(
    origin: [i32; 3],
    signed_size: [i32; 3],
) -> Result<([i32; 3], [i32; 3]), String> {
    let mut min = [0; 3];
    let mut max = [0; 3];
    for axis in 0..3 {
        let end = origin[axis]
            .checked_add(signed_size[axis])
            .and_then(|value| value.checked_sub(signed_size[axis].signum()))
            .ok_or_else(|| {
                "Schematic region coordinates exceed supported bounds"
                    .to_string()
            })?;
        min[axis] = origin[axis].min(end);
        max[axis] = origin[axis].max(end);
    }
    Ok((min, max))
}

fn checked_region_max(
    min: [i32; 3],
    size: [u32; 3],
) -> Result<[i32; 3], String> {
    let mut max = [0; 3];
    for axis in 0..3 {
        let offset = size[axis]
            .checked_sub(1)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| {
                "Schematic region dimensions exceed supported bounds"
                    .to_string()
            })?;
        max[axis] = min[axis].checked_add(offset).ok_or_else(|| {
            "Schematic region coordinates exceed supported bounds".to_string()
        })?;
    }
    Ok(max)
}

fn validate_palette_size(size: usize) -> Result<(), String> {
    if size > MAX_PALETTE_ENTRIES {
        return Err(format!(
            "Schematic palette size {size} exceeds the {MAX_PALETTE_ENTRIES} entry limit"
        ));
    }
    Ok(())
}

fn validate_volume(size: [u32; 3]) -> Result<(), String> {
    if size.contains(&0) {
        return Err(
            "Schematic regions cannot have a zero dimension".to_string()
        );
    }
    let volume = volume(size);
    if volume > MAX_VOLUME {
        return Err(format!(
            "Schematic region volume {volume} exceeds the {MAX_VOLUME} block limit"
        ));
    }
    Ok(())
}

fn list_instance_schematics(
    instance_path: &Path,
) -> Result<Vec<InstanceSchematicFile>, String> {
    let root = instance_path.join("schematics");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in
            std::fs::read_dir(&directory).map_err(|error| error.to_string())?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type =
                entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(extension.as_str(), "litematic" | "schem") {
                continue;
            }
            let metadata =
                entry.metadata().map_err(|error| error.to_string())?;
            result.push(InstanceSchematicFile {
                relative_path: instance_relative_path(&root, &path)?,
                file_name: entry.file_name().to_string_lossy().to_string(),
                format: extension,
                size: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_secs()),
            });
        }
    }
    result.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(result)
}

// IPC file paths use `/` regardless of the host separator. Local filesystem
// paths remain `Path` values until this serialization boundary.
fn instance_relative_path(root: &Path, path: &Path) -> Result<String, String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|error| error.to_string())?
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_file_sources_accept_frontend_field_names() {
        let source: SchematicSource =
            serde_json::from_value(serde_json::json!({
                "kind": "instance_file",
                "instanceId": "demo",
                "relativePath": "config/worldedit/schematics/house.schem"
            }))
            .unwrap();
        let SchematicSource::InstanceFile {
            instance_id,
            relative_path,
        } = source
        else {
            panic!("expected an instance file source");
        };
        assert_eq!(instance_id, "demo");
        assert_eq!(relative_path, "config/worldedit/schematics/house.schem");
    }

    #[test]
    fn instance_relative_paths_use_ipc_separators() {
        let root = PathBuf::from("instance").join("schematics");
        let path = root.join("nested").join("house.schem");

        assert_eq!(
            instance_relative_path(&root, &path).unwrap(),
            "nested/house.schem"
        );
    }

    fn vec3(x: i32, y: i32, z: i32) -> NbtCompound {
        let mut value = NbtCompound::new();
        value.insert("x", x);
        value.insert("y", y);
        value.insert("z", z);
        value
    }

    fn palette_entry(name: &str) -> NbtCompound {
        let mut value = NbtCompound::new();
        value.insert("Name", name);
        value
    }

    fn palette_at(
        session: &PreviewSession,
        region_index: usize,
        position: [i32; 3],
    ) -> &PreviewBlockState {
        let chunk_position = position.map(|value| value.div_euclid(16));
        let local = position.map(|value| value.rem_euclid(16) as usize);
        let block_index = local[1] * 256 + local[2] * 16 + local[0];
        let palette_index = session.regions[region_index].chunks
            [&chunk_position]
            .blocks[block_index] as usize;
        &session.manifest.palette[palette_index]
    }

    #[test]
    fn state_strings_keep_sorted_properties() {
        let state = parse_state_string(
            "minecraft:oak_stairs[waterlogged=false,facing=north]",
        );
        assert_eq!(state.name, "minecraft:oak_stairs");
        assert_eq!(
            state.key(),
            "minecraft:oak_stairs[facing=north,waterlogged=false]"
        );
        assert!(parse_state_string(" Minecraft:Air ").is_air());
        assert!(parse_state_string("CAVE_AIR").is_air());
        assert!(parse_state_string("minecraft:light").is_air());
        assert!(parse_state_string("minecraft:barrier").is_air());
        assert!(parse_state_string("minecraft:structure_void").is_air());
    }

    #[test]
    fn replacement_states_preserve_properties_supported_by_the_target() {
        let source = parse_state_string(
            "minecraft:oak_slab[type=top,waterlogged=true,custom=source]",
        );
        let target = parse_state_string(
            "minecraft:stone_slab[type=bottom,waterlogged=false]",
        );

        assert_eq!(
            replacement_state(&source, &target).key(),
            "minecraft:stone_slab[type=top,waterlogged=true]"
        );
    }

    #[test]
    fn varints_decode_palette_values() {
        let bytes = [0xac, 0x02];
        let mut slice = bytes.as_slice();
        assert_eq!(read_varint(&mut slice).unwrap(), 300);
        assert!(slice.is_empty());
    }

    #[test]
    fn litematic_values_can_cross_long_boundaries() {
        let packed = [0u64, 0b10101u64];
        let packed = packed.map(|value| value as i64);
        assert_eq!(unpack_litematic_value(&packed, 12, 5), 16);
    }

    #[test]
    fn litematic_block_positions_start_at_the_region_minimum() {
        assert_eq!(
            litematic_block_position([6, 20, -4], [2, 1, 1]),
            [8, 21, -3]
        );
    }

    #[test]
    fn region_chunks_handle_negative_coordinates() {
        let builder = SessionBuilder::new();
        let mut region = builder
            .new_region(
                "region-0".to_string(),
                "Negative".to_string(),
                [-1, -1, -1],
                [1, 1, 1],
                [-1, -1, -1],
            )
            .unwrap();
        region.put_block([-1, -1, -1], 1);
        assert_eq!(region.chunks[&[-1, -1, -1]].blocks[4095], 1);
    }

    #[test]
    fn cloned_schematic_regions_copy_only_edited_chunk_data() {
        let builder = SessionBuilder::new();
        let mut original = builder
            .new_region(
                "region".to_string(),
                "Region".to_string(),
                [0, 0, 0],
                [32, 1, 1],
                [0, 0, 0],
            )
            .unwrap();
        original.put_block([0, 0, 0], 1);
        original.put_block([16, 0, 0], 1);
        let mut edited = original.clone();
        for position in [[0, 0, 0], [1, 0, 0]] {
            assert!(Arc::ptr_eq(
                &original.chunks[&position].blocks,
                &edited.chunks[&position].blocks
            ));
        }
        assert!(edited.set_block([0, 0, 0], 2));
        assert_eq!(original.block_at([0, 0, 0]), 1);
        assert_eq!(edited.block_at([0, 0, 0]), 2);
        assert!(!Arc::ptr_eq(
            &original.chunks[&[0, 0, 0]].blocks,
            &edited.chunks[&[0, 0, 0]].blocks
        ));
        assert!(Arc::ptr_eq(
            &original.chunks[&[1, 0, 0]].blocks,
            &edited.chunks[&[1, 0, 0]].blocks
        ));
        assert!(edited.set_block([16, 0, 0], 0));
        assert_eq!(original.block_at([16, 0, 0]), 1);
        assert_eq!(edited.block_at([16, 0, 0]), 0);
    }

    #[test]
    fn alternate_air_states_collapse_to_the_global_air_palette_entry() {
        let mut litematic_palette = NbtList::new();
        litematic_palette.push(palette_entry("minecraft:cave_air"));
        litematic_palette.push(palette_entry("minecraft:stone"));
        let mut litematic_region = NbtCompound::new();
        litematic_region.insert("Position", vec3(0, 0, 0));
        litematic_region.insert("Size", vec3(2, 1, 1));
        litematic_region.insert("BlockStatePalette", litematic_palette);
        litematic_region.insert("BlockStates", NbtTag::LongArray(vec![4]));
        let mut litematic_regions = NbtCompound::new();
        litematic_regions.insert("Region", litematic_region);
        let mut litematic_root = NbtCompound::new();
        litematic_root.insert("Version", 6i32);
        litematic_root.insert("MinecraftDataVersion", 3700i32);
        litematic_root.insert("Regions", litematic_regions);
        let litematic = parse_litematic(
            litematic_root,
            "air.litematic".to_string(),
            "air.litematic".to_string(),
            None,
            None,
        )
        .unwrap();

        let mut sponge_palette = NbtCompound::new();
        sponge_palette.insert("minecraft:void_air", 0i32);
        sponge_palette.insert("minecraft:stone", 1i32);
        let mut sponge_blocks = NbtCompound::new();
        sponge_blocks.insert("Palette", sponge_palette);
        sponge_blocks.insert("BlockData", NbtTag::ByteArray(vec![0, 1]));
        let mut sponge_schematic = NbtCompound::new();
        sponge_schematic.insert("Version", 3i32);
        sponge_schematic.insert("DataVersion", 3700i32);
        sponge_schematic.insert("Width", 2i16);
        sponge_schematic.insert("Height", 1i16);
        sponge_schematic.insert("Length", 1i16);
        sponge_schematic.insert("Blocks", sponge_blocks);
        let mut sponge_root = NbtCompound::new();
        sponge_root.insert("Schematic", sponge_schematic);
        let sponge = parse_sponge(
            sponge_root,
            "air.schem".to_string(),
            "air.schem".to_string(),
            None,
            None,
        )
        .unwrap();

        for session in [&litematic, &sponge] {
            assert_eq!(session.manifest.block_count, 1);
            assert_eq!(session.manifest.palette.len(), 2);
            assert_eq!(session.manifest.palette[0].name, "minecraft:air");
            assert_eq!(session.manifest.palette[1].name, "minecraft:stone");
        }
    }

    #[test]
    fn litematic_fixture_normalizes_negative_axes_from_the_minimum_corner() {
        let mut palette = NbtList::new();
        palette.push(palette_entry("minecraft:air"));
        palette.push(palette_entry("minecraft:stone"));
        palette.push(palette_entry("minecraft:dirt"));

        let mut negative = NbtCompound::new();
        negative.insert("Position", vec3(10, 5, 3));
        negative.insert("Size", vec3(-3, -2, -2));
        negative.insert("BlockStatePalette", palette.clone());
        negative.insert(
            "BlockStates",
            NbtTag::LongArray(pack_litematic_values(
                &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
                2,
            )),
        );

        let mut positive = NbtCompound::new();
        positive.insert("Position", vec3(-2, 4, 2));
        positive.insert("Size", vec3(1, 1, 1));
        positive.insert("BlockStatePalette", palette);
        positive.insert("BlockStates", NbtTag::LongArray(vec![1]));

        let mut regions = NbtCompound::new();
        regions.insert("Negative", negative);
        regions.insert("Positive", positive);
        let mut root = NbtCompound::new();
        root.insert("Version", 6i32);
        root.insert("MinecraftDataVersion", 3700i32);
        root.insert("Regions", regions);

        let session = parse_litematic(
            root,
            "fixture.litematic".to_string(),
            "fixture.litematic".to_string(),
            None,
            None,
        )
        .unwrap();
        let negative_index = session
            .regions
            .iter()
            .position(|region| region.manifest.name == "Negative")
            .unwrap();
        assert_eq!(
            palette_at(&session, negative_index, [8, 4, 2]).name,
            "minecraft:stone"
        );
        assert_eq!(
            palette_at(&session, negative_index, [10, 5, 3]).name,
            "minecraft:dirt"
        );
        assert_eq!(session.manifest.regions.len(), 2);
    }

    fn sponge_fixture(version: i32) -> NbtCompound {
        let mut palette = NbtCompound::new();
        palette.insert("minecraft:air", 0i32);
        palette.insert("minecraft:stone", 1i32);
        let mut blocks = NbtCompound::new();
        blocks.insert("Palette", palette);
        blocks.insert("BlockData", NbtTag::ByteArray(vec![1, 0]));
        let mut schematic = NbtCompound::new();
        schematic.insert("Version", version);
        schematic.insert("DataVersion", 3700i32);
        schematic.insert("Width", 2i16);
        schematic.insert("Height", 1i16);
        schematic.insert("Length", 1i16);
        schematic.insert("Offset", NbtTag::IntArray(vec![-4, 8, 2]));
        if version == 3 {
            schematic.insert("Blocks", blocks);
        } else {
            for (key, value) in blocks.into_inner() {
                schematic.insert(key, value);
            }
        }
        let mut root = NbtCompound::new();
        if version == 3 {
            root.insert("Schematic", schematic);
        } else {
            root = schematic;
        }
        root
    }

    #[test]
    fn sponge_v2_and_v3_fixtures_parse_offsets_and_varints() {
        for version in [2, 3] {
            let session = parse_sponge(
                sponge_fixture(version),
                "fixture.schem".to_string(),
                "fixture.schem".to_string(),
                None,
                None,
            )
            .unwrap();
            assert_eq!(session.manifest.format, format!("schem_v{version}"));
            assert_eq!(session.manifest.min, [-4, 8, 2]);
            assert_eq!(session.manifest.block_count, 1);
            assert_eq!(
                palette_at(&session, 0, [-4, 8, 2]).name,
                "minecraft:stone"
            );
        }
    }

    #[test]
    fn edits_refresh_materials_and_export_to_supported_formats() {
        let mut session = parse_sponge(
            sponge_fixture(3),
            "fixture.schem".to_string(),
            "fixture.schem".to_string(),
            None,
            None,
        )
        .unwrap();
        assert!(session.regions[0].set_block([-4, 8, 2], 0));
        assert!(session.regions[0].set_block([-3, 8, 2], 1));
        session.refresh_manifest();
        assert_eq!(session.manifest.block_count, 1);
        assert_eq!(session.manifest.materials[0].name, "minecraft:stone");

        let bytes = export_sponge_v3(&session).unwrap();
        let root = read_root(&bytes).unwrap();
        let exported = parse_sponge(
            root,
            "exported.schem".to_string(),
            "exported.schem".to_string(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(exported.manifest.format, "schem_v3");
        assert_eq!(
            palette_at(&exported, 0, [-3, 8, 2]).name,
            "minecraft:stone"
        );

        let bytes = export_litematic(&session).unwrap();
        let root = read_root(&bytes).unwrap();
        let exported = parse_litematic(
            root,
            "exported.litematic".to_string(),
            "exported.litematic".to_string(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(exported.manifest.format, "litematic");
        assert_eq!(exported.manifest.size, [2, 1, 1]);
        assert_eq!(
            palette_at(&exported, 0, [-3, 8, 2]).name,
            "minecraft:stone"
        );
    }

    #[test]
    fn current_and_snapshot_schematics_keep_their_original_data_version_on_export()
     {
        for data_version in [3700, 5005, 5023] {
            let mut fixture = sponge_fixture(3);
            let schematic =
                fixture.get_mut::<_, &mut NbtCompound>("Schematic").unwrap();
            schematic.insert("DataVersion", data_version);
            let palette = schematic
                .get_mut::<_, &mut NbtCompound>("Blocks")
                .unwrap()
                .get_mut::<_, &mut NbtCompound>("Palette")
                .unwrap();
            palette.inner_mut().remove("minecraft:stone");
            palette.insert("minecraft:cinnabar", 1i32);
            let mut session = parse_sponge(
                fixture,
                "test.schem".into(),
                "test.schem".into(),
                None,
                None,
            )
            .unwrap();
            session.regions[0].set_block([-3, 8, 2], 1);
            session.refresh_manifest();
            for (bytes, litematic) in [
                (export_sponge_v3(&session).unwrap(), false),
                (export_litematic(&session).unwrap(), true),
            ] {
                let root = read_root(&bytes).unwrap();
                let exported = if litematic {
                    parse_litematic(
                        root,
                        "test.litematic".into(),
                        "test.litematic".into(),
                        None,
                        None,
                    )
                } else {
                    parse_sponge(
                        root,
                        "test.schem".into(),
                        "test.schem".into(),
                        None,
                        None,
                    )
                }
                .unwrap();
                assert_eq!(exported.manifest.data_version, Some(data_version));
                assert_eq!(
                    palette_at(&exported, 0, [-3, 8, 2]).name,
                    "minecraft:cinnabar"
                );
            }
        }
    }

    #[test]
    fn rotations_are_reversible_and_update_dimensions() {
        let mut session = parse_sponge(
            sponge_fixture(3),
            "fixture.schem".to_string(),
            "fixture.schem".to_string(),
            None,
            None,
        )
        .unwrap();
        session
            .apply_transform(PreviewTransform::RotateClockwise)
            .unwrap();
        assert_eq!(session.manifest.size, [1, 1, 2]);
        assert_eq!(palette_at(&session, 0, [-4, 8, 3]).name, "minecraft:stone");
        session
            .apply_transform(PreviewTransform::RotateCounterClockwise)
            .unwrap();
        assert_eq!(session.manifest.size, [2, 1, 1]);
        assert_eq!(palette_at(&session, 0, [-4, 8, 2]).name, "minecraft:stone");
    }

    #[test]
    fn malformed_gzip_and_invalid_sponge_palette_indices_fail() {
        assert!(read_root(&[0x1f, 0x8b, 0, 1, 2]).is_err());
        let mut fixture = sponge_fixture(2);
        fixture.insert("BlockData", NbtTag::ByteArray(vec![2, 0]));
        assert!(
            parse_sponge(
                fixture,
                "invalid.schem".to_string(),
                "invalid.schem".to_string(),
                None,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn cancelled_parse_requests_stop_before_work() {
        let cancellation = AtomicBool::new(true);
        assert_eq!(
            ensure_not_cancelled(Some(&cancellation)).unwrap_err(),
            "Schematic preview request was cancelled"
        );
    }

    #[test]
    fn extreme_palette_and_coordinate_declarations_are_rejected() {
        assert!(validate_palette_size(MAX_PALETTE_ENTRIES + 1).is_err());
        assert!(signed_region_bounds([i32::MAX, 0, 0], [2, 1, 1]).is_err());
        assert!(checked_region_max([i32::MAX, 0, 0], [2, 1, 1]).is_err());
        let mut regions = NbtCompound::new();
        for name in ["First", "Second"] {
            let mut region = NbtCompound::new();
            region.insert("Position", vec3(0, 0, 0));
            region.insert("Size", vec3(4_000, 4_000, 5));
            regions.insert(name, region);
        }
        assert!(validate_litematic_regions(&regions).is_err());
    }
}
