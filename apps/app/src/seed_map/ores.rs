//! Versioned ore candidates computed exclusively by xpple/cubiomes.

use super::{Dimension, Edition, parse_seed, resolve_java_version};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum OreKind {
    Diamond,
    Iron,
    IronVein,
    Copper,
    CopperVein,
    Gold,
    Redstone,
    Lapis,
    Coal,
    Netherite,
    Emerald,
    Quartz,
    NetherGold,
    Andesite,
    Diorite,
    Granite,
    Dirt,
    Gravel,
    Infested,
    Deepslate,
    Tuff,
    Clay,
    Blackstone,
    Magma,
    SoulSand,
}

pub const ALL_ORE_KINDS: &[OreKind] = &[
    OreKind::Diamond,
    OreKind::Iron,
    OreKind::IronVein,
    OreKind::Copper,
    OreKind::CopperVein,
    OreKind::Gold,
    OreKind::Redstone,
    OreKind::Lapis,
    OreKind::Coal,
    OreKind::Netherite,
    OreKind::Emerald,
    OreKind::Quartz,
    OreKind::NetherGold,
    OreKind::Andesite,
    OreKind::Diorite,
    OreKind::Granite,
    OreKind::Dirt,
    OreKind::Gravel,
    OreKind::Infested,
    OreKind::Deepslate,
    OreKind::Tuff,
    OreKind::Clay,
    OreKind::Blackstone,
    OreKind::Magma,
    OreKind::SoulSand,
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OreScanRequest {
    pub seed: String,
    pub edition: Edition,
    pub version: String,
    pub dimension: Dimension,
    pub ores: Vec<OreKind>,
    pub chunks: Vec<i32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OreHit {
    pub ore: OreKind,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OreChunkResult {
    pub cx: i32,
    pub cz: i32,
    pub hits: Vec<OreHit>,
    pub truncated: bool,
}

pub fn supports_ores(version_code: i32, dimension: Dimension) -> bool {
    version_code >= 11300 && !matches!(dimension, Dimension::End)
}

pub fn scan_ores(request: OreScanRequest) -> io::Result<Vec<OreChunkResult>> {
    scan_ores_with_budget(request, 4096)
}

pub(super) fn scan_ores_with_budget(
    request: OreScanRequest,
    total_capacity: usize,
) -> io::Result<Vec<OreChunkResult>> {
    scan_ores_with_budget_cancellable(request, total_capacity, None)
}

pub(super) fn scan_ores_with_budget_cancellable(
    request: OreScanRequest,
    total_capacity: usize,
    cancellation: Option<(u64, extern "C" fn(u64) -> i32)>,
) -> io::Result<Vec<OreChunkResult>> {
    let canceled = || cancellation.is_some_and(|(id, check)| check(id) != 0);
    let (mc, code) = resolve_java_version(&request.version)?;
    if !supports_ores(code, request.dimension) {
        return Err(io::Error::other(
            "Ore prediction supports Java 1.13+ Overworld and Nether maps.",
        ));
    }
    if !(1..=409_600).contains(&total_capacity)
        || !request.chunks.len().is_multiple_of(2)
        || request.chunks.len() > 192
        || request.ores.len() > 25
        || request
            .chunks
            .iter()
            .any(|coordinate| coordinate.abs_diff(0) > 1_875_000)
    {
        return Err(io::Error::other("The ore scan chunk list is invalid."));
    }
    let seed = parse_seed(&request.seed);
    let mut kinds = HashSet::new();
    let kinds: Vec<_> = request
        .ores
        .into_iter()
        .filter(|kind| kinds.insert(*kind))
        .collect();
    let mut results = Vec::new();
    for chunk in request.chunks.chunks_exact(2) {
        let mut hits = Vec::new();
        let mut truncated = false;
        let mut seen = HashSet::new();
        let capacity = (total_capacity / kinds.len().max(1)).clamp(1, 16_384);
        let mut xyz = vec![0_i32; capacity * 3];
        for &kind in &kinds {
            if canceled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Ore generation canceled.",
                ));
            }
            let count = unsafe {
                if let Some((id, check)) = cancellation {
                    super::axolotl_seed_map_scan_ores_cancellable(
                        seed,
                        mc,
                        request.edition.generator_flags(),
                        request.dimension.as_cubiomes(),
                        chunk[0],
                        chunk[1],
                        kind as i32,
                        xyz.as_mut_ptr(),
                        xyz.len() / 3,
                        id,
                        check,
                    )
                } else {
                    super::axolotl_seed_map_scan_ores(
                        seed,
                        mc,
                        request.edition.generator_flags(),
                        request.dimension.as_cubiomes(),
                        chunk[0],
                        chunk[1],
                        kind as i32,
                        xyz.as_mut_ptr(),
                        xyz.len() / 3,
                    )
                }
            };
            if canceled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Ore generation canceled.",
                ));
            }
            truncated |= count == xyz.len() / 3;
            for pos in xyz[..count.min(xyz.len() / 3) * 3].chunks_exact(3) {
                if !seen.insert((kind, pos[0], pos[1], pos[2])) {
                    continue;
                }
                hits.push(OreHit {
                    ore: kind,
                    x: pos[0],
                    y: pos[1],
                    z: pos[2],
                    y_min: pos[1],
                    y_max: pos[1],
                    status: "terrain_unverified",
                });
            }
        }
        results.push(OreChunkResult {
            cx: chunk[0],
            cz: chunk[1],
            hits,
            truncated,
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ore_capabilities_follow_upstream_version_range() {
        assert!(!supports_ores(11200, Dimension::Overworld));
        assert!(supports_ores(11300, Dimension::Overworld));
        assert!(supports_ores(260300, Dimension::Nether));
        assert!(!supports_ores(260300, Dimension::End));
    }
}
