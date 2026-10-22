use tauri::Runtime;

use crate::{api::Result, seed_map};

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("seed-map")
        .invoke_handler(tauri::generate_handler![
            seed_map_profiles,
            seed_map_render_tile,
            seed_map_find_features,
            seed_map_spawn,
            seed_map_biome_at,
            seed_map_scan_ores,
            seed_map_read_level_dat,
            seed_map_structure_details,
            seed_map_container_loot,
            seed_map_underground_tile,
            seed_map_underground_volume,
            seed_map_cancel_underground,
        ])
        .build()
}

#[tauri::command]
pub fn seed_map_profiles() -> Vec<seed_map::VersionProfile> {
    seed_map::version_profiles()
}

#[tauri::command]
pub async fn seed_map_render_tile(
    request: seed_map::TileRequest,
) -> Result<tauri::ipc::Response> {
    let pixels = tauri::async_runtime::spawn_blocking(move || {
        seed_map::render_tile(request)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(tauri::ipc::Response::new(pixels))
}

#[tauri::command]
pub async fn seed_map_find_features(
    query: seed_map::FeatureQuery,
) -> Result<Vec<seed_map::MapFeature>> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::find_features(query)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_spawn(
    seed: String,
    edition: seed_map::Edition,
    version: String,
) -> Result<seed_map::SpawnPoint> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::spawn(seed, edition, version)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_biome_at(
    seed: String,
    edition: seed_map::Edition,
    version: String,
    dimension: seed_map::Dimension,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::biome_at(seed, edition, version, dimension, x, y, z)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_scan_ores(
    request: seed_map::ores::OreScanRequest,
) -> Result<Vec<seed_map::ores::OreChunkResult>> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::ores::scan_ores(request)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_read_level_dat(
    path: String,
) -> Result<seed_map::LevelDatInfo> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::read_level_dat(path)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_structure_details(
    query: seed_map::details::StructureQuery,
) -> Result<serde_json::Value> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::details::structure_details(query, None)
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_container_loot(
    query: seed_map::details::StructureQuery,
    container: u32,
) -> Result<serde_json::Value> {
    Ok(tauri::async_runtime::spawn_blocking(move || {
        seed_map::details::structure_details(query, Some(container))
    })
    .await
    .map_err(std::io::Error::other)??)
}

#[tauri::command]
pub async fn seed_map_underground_tile(
    request: seed_map::details::UndergroundRequest,
) -> Result<tauri::ipc::Response> {
    let pixels = tauri::async_runtime::spawn_blocking(move || {
        seed_map::details::underground_tile(request)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(tauri::ipc::Response::new(pixels))
}

#[tauri::command]
pub async fn seed_map_underground_volume(
    request: seed_map::underground::VolumeRequest,
) -> Result<tauri::ipc::Response> {
    seed_map::underground::begin(request.id);
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        seed_map::underground::generate_volume(request)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn seed_map_cancel_underground(id: u64) {
    seed_map::underground::cancel(id);
}
