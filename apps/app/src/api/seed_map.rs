use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
            seed_map_cancel_ore_scan,
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

/// Area ore scans by the request ID their caller chose, so a cancel reaches the
/// native loop, and cancels that arrived before their scan was registered.
#[derive(Default)]
struct OreScans {
    running: HashMap<u64, bool>,
    early: HashMap<u64, Instant>,
}

const EARLY_ORE_SCAN_CANCEL_TTL: Duration = Duration::from_secs(30);

fn ore_scans() -> &'static Mutex<OreScans> {
    static ORE_SCANS: OnceLock<Mutex<OreScans>> = OnceLock::new();
    ORE_SCANS.get_or_init(Mutex::default)
}

struct OreScan(u64);

impl OreScan {
    fn register(id: u64) -> Self {
        if let Ok(mut scans) = ore_scans().lock() {
            let canceled = scans
                .early
                .remove(&id)
                .is_some_and(|at| at.elapsed() < EARLY_ORE_SCAN_CANCEL_TTL);
            scans.running.insert(id, canceled);
        }
        Self(id)
    }
}

impl Drop for OreScan {
    fn drop(&mut self) {
        if let Ok(mut scans) = ore_scans().lock() {
            scans.running.remove(&self.0);
        }
    }
}

/// Polled by the native scan between veins; an unknown request counts as
/// canceled, since every cancellable scan registers before it starts.
extern "C" fn ore_scan_canceled(id: u64) -> i32 {
    ore_scans().lock().map_or(1, |scans| {
        i32::from(scans.running.get(&id).is_none_or(|canceled| *canceled))
    })
}

fn cancel_ore_scan(id: u64) {
    let Ok(mut scans) = ore_scans().lock() else {
        return;
    };
    if let Some(canceled) = scans.running.get_mut(&id) {
        *canceled = true;
        return;
    }
    scans
        .early
        .retain(|_, at| at.elapsed() < EARLY_ORE_SCAN_CANCEL_TTL);
    if scans.early.len() < 64 {
        scans.early.insert(id, Instant::now());
    }
}

#[tauri::command]
pub async fn seed_map_scan_ores(
    request: seed_map::ores::OreScanRequest,
    request_id: Option<u64>,
) -> Result<Vec<seed_map::ores::OreChunkResult>> {
    let scan = request_id.map(OreScan::register);
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _scan = scan;
        match request_id {
            Some(id) => seed_map::ores::scan_ores_cancellable(
                request,
                id,
                ore_scan_canceled,
            ),
            None => seed_map::ores::scan_ores(request),
        }
    })
    .await
    .map_err(std::io::Error::other)??)
}

/// Stops an area ore scan at its next native check. A cancel that arrives
/// before the scan starts is kept briefly, so the scan stops as it begins.
#[tauri::command]
pub fn seed_map_cancel_ore_scan(request_id: u64) {
    cancel_ore_scan(request_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ore_scan_cancel_reaches_running_and_not_yet_started_scans() {
        let running = OreScan::register(9_001);
        assert_eq!(ore_scan_canceled(9_001), 0);
        cancel_ore_scan(9_001);
        assert_eq!(ore_scan_canceled(9_001), 1);
        drop(running);
        assert_eq!(ore_scan_canceled(9_001), 1);

        cancel_ore_scan(9_002);
        let early = OreScan::register(9_002);
        assert_eq!(ore_scan_canceled(9_002), 1);
        drop(early);
        // The early cancel was used up by the scan it belonged to.
        let next = OreScan::register(9_002);
        assert_eq!(ore_scan_canceled(9_002), 0);
        drop(next);
    }
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
