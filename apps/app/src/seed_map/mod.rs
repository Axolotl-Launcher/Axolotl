use std::{
    io::{self, Cursor},
    path::Path,
};

use quartz_nbt::{NbtCompound, NbtTag};
use serde::{Deserialize, Serialize};

pub mod details;
pub mod ores;
mod template_resources;
pub mod underground;

#[repr(C)]
#[derive(Clone, Copy)]
struct BridgeFeature {
    x: i32,
    z: i32,
    kind: u32,
    approximate: u8,
    end_ship: i8,
}

mod bridge;
use bridge::*;

pub const FEATURE_VILLAGE: u32 = 1 << 0;
pub const FEATURE_OUTPOST: u32 = 1 << 1;
pub const FEATURE_SHIPWRECK: u32 = 1 << 2;
pub const FEATURE_MONUMENT: u32 = 1 << 3;
pub const FEATURE_MANSION: u32 = 1 << 4;
pub const FEATURE_ANCIENT_CITY: u32 = 1 << 5;
pub const FEATURE_TRAIL_RUINS: u32 = 1 << 6;
pub const FEATURE_TRIAL_CHAMBERS: u32 = 1 << 7;
pub const FEATURE_RUINED_PORTAL: u32 = 1 << 8;
pub const FEATURE_STRONGHOLD: u32 = 1 << 9;
pub const FEATURE_SLIME_CHUNK: u32 = 1 << 10;
pub const FEATURE_DESERT_PYRAMID: u32 = 1 << 11;
pub const FEATURE_JUNGLE_TEMPLE: u32 = 1 << 12;
pub const FEATURE_SWAMP_HUT: u32 = 1 << 13;
pub const FEATURE_IGLOO: u32 = 1 << 14;
pub const FEATURE_OCEAN_RUIN: u32 = 1 << 15;
pub const FEATURE_BURIED_TREASURE: u32 = 1 << 16;
pub const FEATURE_MINESHAFT: u32 = 1 << 17;
pub const FEATURE_DESERT_WELL: u32 = 1 << 18;
pub const FEATURE_GEODE: u32 = 1 << 19;
pub const FEATURE_FORTRESS: u32 = 1 << 20;
pub const FEATURE_BASTION: u32 = 1 << 21;
pub const FEATURE_END_CITY: u32 = 1 << 22;
pub const FEATURE_END_GATEWAY: u32 = 1 << 23;
pub const FEATURE_ABANDONED_CAMP: u32 = 1 << 24;
pub const FEATURE_NETHER_FOSSIL: u32 = 1 << 25;
pub const ALL_FEATURES: u32 = FEATURE_VILLAGE
    | FEATURE_OUTPOST
    | FEATURE_SHIPWRECK
    | FEATURE_MONUMENT
    | FEATURE_MANSION
    | FEATURE_ANCIENT_CITY
    | FEATURE_TRAIL_RUINS
    | FEATURE_TRIAL_CHAMBERS
    | FEATURE_RUINED_PORTAL
    | FEATURE_STRONGHOLD
    | FEATURE_SLIME_CHUNK
    | FEATURE_DESERT_PYRAMID
    | FEATURE_JUNGLE_TEMPLE
    | FEATURE_SWAMP_HUT
    | FEATURE_IGLOO
    | FEATURE_OCEAN_RUIN
    | FEATURE_BURIED_TREASURE
    | FEATURE_MINESHAFT
    | FEATURE_DESERT_WELL
    | FEATURE_GEODE
    | FEATURE_FORTRESS
    | FEATURE_BASTION
    | FEATURE_END_CITY
    | FEATURE_END_GATEWAY
    | FEATURE_ABANDONED_CAMP
    | FEATURE_NETHER_FOSSIL;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Edition {
    Java,
    #[serde(rename = "java-large-biomes")]
    JavaLargeBiomes,
}

impl Edition {
    fn generator_flags(self) -> i32 {
        match self {
            Self::JavaLargeBiomes => 1,
            Self::Java => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}

impl Dimension {
    fn as_cubiomes(self) -> i32 {
        match self {
            Self::Overworld => 0,
            Self::Nether => -1,
            Self::End => 1,
        }
    }
}

/*
 * Supported Java versions, newest first, using Axolotl version codes
 * (major * 10000 + minor * 100 + patch) that the cubiomes bridge maps to its
 * own engine versions.
 */
const JAVA_VERSIONS: &[(&str, i32)] = &[
    ("26.3", 260300),
    ("26.2", 260200),
    ("26.1.2", 260102),
    ("26.1", 260100),
    ("1.21.11", 12111),
    ("1.21.9", 12109),
    ("1.21.6", 12106),
    ("1.21.5", 12105),
    ("1.21.4", 12104),
    ("1.21.3", 12103),
    ("1.21.1", 12101),
    ("1.20.6", 12006),
    ("1.20", 12000),
    ("1.19.4", 11904),
    ("1.19.2", 11902),
    ("1.18.2", 11802),
    ("1.18", 11800),
    ("1.17.1", 11701),
    ("1.17", 11700),
    ("1.16.5", 11605),
    ("1.16.1", 11601),
    ("1.16", 11600),
    ("1.15.2", 11502),
    ("1.15", 11500),
    ("1.14.4", 11404),
    ("1.14", 11400),
    ("1.13.2", 11302),
    ("1.13", 11300),
    ("1.12.2", 11202),
    ("1.12", 11200),
    ("1.11.2", 11102),
    ("1.11", 11100),
    ("1.10.2", 11002),
    ("1.10", 11000),
    ("1.9.4", 10904),
    ("1.9", 10900),
    ("1.8.9", 10809),
    ("1.8", 10800),
    ("1.7.10", 10710),
    ("1.7", 10700),
    ("1.6.4", 10604),
    ("1.6", 10600),
    ("1.5.2", 10502),
    ("1.5", 10500),
    ("1.4.7", 10407),
    ("1.4", 10400),
    ("1.3.2", 10302),
    ("1.3", 10300),
    ("1.2.5", 10205),
    ("1.2", 10200),
    ("1.1.0", 10100),
    ("1.1", 10100),
    ("1.0.0", 10000),
    ("1.0", 10000),
    ("b1.8", -10800),
    ("b1.7", -10700),
];

const LARGE_BIOMES_MIN_CODE: i32 = 10300;
const NETHER_BIOMES_MIN_CODE: i32 = 11600;
const END_BIOMES_MIN_CODE: i32 = 10900;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileRequest {
    pub seed: String,
    pub edition: Edition,
    pub version: String,
    pub dimension: Dimension,
    pub x: i32,
    pub z: i32,
    pub scale: i32,
    pub width: i32,
    pub height: i32,
    pub elevation: Option<i32>,
    pub terrain: bool,
    pub contours: bool,
    pub highlight_biomes: Option<Vec<u16>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureQuery {
    pub seed: String,
    pub edition: Edition,
    pub version: String,
    pub dimension: Dimension,
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
    pub feature_mask: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapFeature {
    pub kind: String,
    pub x: i32,
    pub z: i32,
    pub approximate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ship: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnPoint {
    pub x: i32,
    pub z: i32,
    pub approximate: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionProfile {
    pub edition: Edition,
    pub version: String,
    pub available: bool,
    pub dimensions: Vec<Dimension>,
    pub ores: bool,
    pub ore_kinds: Vec<ores::OreKind>,
    pub feature_masks: [u32; 3],
    pub underground_masks: [u8; 3],
    pub underground_y_ranges: [[i32; 2]; 3],
    pub biomes: Vec<i32>,
    pub engine_revision: &'static str,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelDatInfo {
    pub seed: String,
    pub version: Option<String>,
    pub data_version: Option<i32>,
}

pub fn version_profiles() -> Vec<VersionProfile> {
    let mut profiles = Vec::new();
    let availability = bridge::ensure_available();
    let available = availability.is_ok();
    let note = availability.err().map(|error| error.to_string());
    for edition in [Edition::Java, Edition::JavaLargeBiomes] {
        for &(version, code) in JAVA_VERSIONS {
            if matches!(edition, Edition::JavaLargeBiomes)
                && code < LARGE_BIOMES_MIN_CODE
            {
                continue;
            }
            let mut dimensions = vec![Dimension::Overworld];
            if code >= NETHER_BIOMES_MIN_CODE {
                dimensions.push(Dimension::Nether);
            }
            if code >= END_BIOMES_MIN_CODE {
                dimensions.push(Dimension::End);
            }
            let mc = if available {
                unsafe { axolotl_seed_map_java_version(code) }
            } else {
                0
            };
            let capability = |kind, value| {
                if available {
                    unsafe { axolotl_seed_map_capability(mc, kind, value) }
                } else {
                    0
                }
            };
            profiles.push(VersionProfile {
                edition,
                version: version.to_owned(),
                available,
                dimensions,
                ores: available
                    && ores::supports_ores(code, Dimension::Overworld),
                ore_kinds: ores::ALL_ORE_KINDS
                    .iter()
                    .copied()
                    .filter(|kind| capability(2, *kind as i32) != 0)
                    .collect(),
                feature_masks: [
                    capability(1, 0) as u32,
                    capability(1, -1) as u32,
                    capability(1, 1) as u32,
                ],
                underground_y_ranges: [
                    if code >= 11800 { [-64, 319] } else { [0, 255] },
                    [0, 127],
                    [0, 127],
                ],
                underground_masks: [
                    capability(3, 0) as u8,
                    capability(3, -1) as u8,
                    capability(3, 1) as u8,
                ],
                biomes: (0..256).filter(|id| capability(0, *id) != 0).collect(),
                engine_revision: "a49d5bc5b709e6e28b5a74daddb3efb382fba7bb",
                note: note.clone(),
            });
        }
    }
    profiles
}

/*
 * Renders a tile and returns tightly packed RGBA pixels; the caller knows the
 * requested dimensions and rebuilds the image without any encode/decode step.
 */
pub fn render_tile(request: TileRequest) -> io::Result<Vec<u8>> {
    let (minecraft_version, _) = resolve_java_version(&request.version)?;
    validate_tile_request(&request)?;

    let highlight_mask = request.highlight_biomes.as_ref().and_then(|biomes| {
        if biomes.is_empty() {
            return None;
        }
        let mut mask = [0_u8; 256];
        for &biome in biomes {
            if usize::from(biome) < mask.len() {
                mask[usize::from(biome)] = 1;
            }
        }
        Some(mask)
    });

    let pixel_count = request.width as usize * request.height as usize;
    let mut rgb = vec![0_u8; pixel_count * 3];
    let result = unsafe {
        axolotl_seed_map_render(
            parse_seed(&request.seed),
            minecraft_version,
            request.edition.generator_flags(),
            request.dimension.as_cubiomes(),
            scaled_coordinate(request.x, request.scale),
            scaled_coordinate(request.z, request.scale),
            request.scale,
            request.width,
            request.height,
            request.elevation.unwrap_or(128).clamp(-64, 320),
            i32::from(request.terrain),
            i32::from(request.contours),
            highlight_mask
                .as_ref()
                .map_or(std::ptr::null(), |mask| mask.as_ptr()),
            rgb.as_mut_ptr(),
            rgb.len(),
        )
    };
    if result != 0 {
        return Err(io::Error::other(format!(
            "cubiomes could not generate this tile ({result})"
        )));
    }

    let mut rgba = vec![255_u8; pixel_count * 4];
    for (source, target) in rgb.chunks_exact(3).zip(rgba.chunks_exact_mut(4)) {
        target[..3].copy_from_slice(source);
    }
    Ok(rgba)
}

pub fn find_features(query: FeatureQuery) -> io::Result<Vec<MapFeature>> {
    let (minecraft_version, _) = resolve_java_version(&query.version)?;
    if query.min_x > query.max_x || query.min_z > query.max_z {
        return Err(io::Error::other("The map bounds are invalid."));
    }
    let mut raw_features = vec![
        BridgeFeature {
            x: 0,
            z: 0,
            kind: 0,
            approximate: 0,
            end_ship: -1,
        };
        4_096
    ];
    let count = unsafe {
        axolotl_seed_map_find_features(
            parse_seed(&query.seed),
            minecraft_version,
            query.edition.generator_flags(),
            query.dimension.as_cubiomes(),
            query.min_x,
            query.min_z,
            query.max_x,
            query.max_z,
            query.feature_mask.unwrap_or(ALL_FEATURES) & ALL_FEATURES,
            raw_features.as_mut_ptr(),
            raw_features.len(),
        )
    };

    Ok(raw_features
        .into_iter()
        .take(count)
        .filter_map(|feature| {
            Some(MapFeature {
                kind: feature_name(feature.kind)?.to_owned(),
                x: feature.x,
                z: feature.z,
                approximate: feature.approximate != 0,
                end_ship: match feature.end_ship {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                },
            })
        })
        .collect())
}

pub fn biome_at(
    seed: String,
    edition: Edition,
    version: String,
    dimension: Dimension,
    x: i32,
    y: i32,
    z: i32,
) -> io::Result<i32> {
    let (minecraft_version, _) = resolve_java_version(&version)?;
    let biome = unsafe {
        axolotl_seed_map_biome_at(
            parse_seed(&seed),
            minecraft_version,
            edition.generator_flags(),
            dimension.as_cubiomes(),
            x,
            y.clamp(-64, 319),
            z,
        )
    };
    Ok(biome)
}

pub fn spawn(
    seed: String,
    edition: Edition,
    version: String,
) -> io::Result<SpawnPoint> {
    let (minecraft_version, _) = resolve_java_version(&version)?;
    let mut x = 0;
    let mut z = 0;
    let result = unsafe {
        axolotl_seed_map_get_spawn(
            parse_seed(&seed),
            minecraft_version,
            edition.generator_flags(),
            &raw mut x,
            &raw mut z,
        )
    };
    if result != 0 {
        return Err(io::Error::other(
            "cubiomes could not estimate the spawn point.",
        ));
    }
    Ok(SpawnPoint {
        x,
        z,
        approximate: true,
    })
}

pub(crate) fn resolve_java_version(version: &str) -> io::Result<(i32, i32)> {
    bridge::ensure_available()?;
    let code = JAVA_VERSIONS
        .iter()
        .find(|(name, _)| *name == version)
        .map(|(_, code)| *code)
        .ok_or_else(|| {
            io::Error::other("This Minecraft version is not supported.")
        })?;
    let engine_version = unsafe { axolotl_seed_map_java_version(code) };
    if engine_version == 0 {
        return Err(io::Error::other(
            "The bundled cubiomes version does not support this Minecraft version.",
        ));
    }
    Ok((engine_version, code))
}

pub fn read_level_dat(path: String) -> io::Result<LevelDatInfo> {
    let path = Path::new(&path);
    let root = read_nbt_file(path)?;
    let data = root.get::<_, &NbtCompound>("Data").unwrap_or(&root);
    let seed = match extract_world_seed(data) {
        Some(seed) => seed,
        None => read_split_world_gen_seed(path)?.ok_or_else(|| {
            io::Error::other(
                "The selected world does not contain a world seed.",
            )
        })?,
    };
    let version = data
        .get::<_, &NbtCompound>("Version")
        .ok()
        .and_then(|version| version.get::<_, &str>("Name").ok())
        .and_then(normalize_level_dat_version)
        .or_else(|| {
            data.get::<_, i32>("DataVersion")
                .ok()
                .and_then(version_from_data_version)
        });
    Ok(LevelDatInfo {
        seed: seed.to_string(),
        version,
        data_version: data.get::<_, i32>("DataVersion").ok(),
    })
}

fn read_nbt_file(path: &Path) -> io::Result<NbtCompound> {
    let raw = std::fs::read(path)?;
    read_nbt_any_compression(&raw)
}

fn read_split_world_gen_seed(level_dat_path: &Path) -> io::Result<Option<i64>> {
    let Some(world_path) = level_dat_path.parent() else {
        return Ok(None);
    };
    let settings_path = world_path
        .join("data")
        .join("minecraft")
        .join("world_gen_settings.dat");
    let settings = match read_nbt_file(&settings_path) {
        Ok(settings) => settings,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    Ok(extract_world_seed(&settings))
}

fn read_nbt_any_compression(raw: &[u8]) -> io::Result<NbtCompound> {
    let flavors = [
        quartz_nbt::io::Flavor::GzCompressed,
        quartz_nbt::io::Flavor::Uncompressed,
        quartz_nbt::io::Flavor::ZlibCompressed,
    ];
    let mut last_error = None;
    for flavor in flavors {
        match quartz_nbt::io::read_nbt(&mut Cursor::new(raw), flavor) {
            Ok((root, _)) => return Ok(root),
            Err(error) => last_error = Some(error),
        }
    }
    Err(io::Error::other(match last_error {
        Some(error) => {
            format!("The selected level.dat could not be parsed: {error}")
        }
        None => "The selected level.dat could not be parsed.".to_owned(),
    }))
}

fn extract_world_seed(data: &NbtCompound) -> Option<i64> {
    if let Ok(settings) = data.get::<_, &NbtCompound>("WorldGenSettings")
        && let Ok(seed) = settings.get::<_, &NbtTag>("seed")
        && let Some(seed) = numeric_seed(seed)
    {
        return Some(seed);
    }
    if let Ok(seed) = data.get::<_, &NbtTag>("RandomSeed")
        && let Some(seed) = numeric_seed(seed)
    {
        return Some(seed);
    }
    find_nested_world_seed(data)
}

fn find_nested_world_seed(compound: &NbtCompound) -> Option<i64> {
    for (name, tag) in compound.inner() {
        if name.eq_ignore_ascii_case("seed")
            && let Some(seed) = numeric_seed(tag)
        {
            return Some(seed);
        }
        let nested_seed = match tag {
            NbtTag::Compound(child) => find_nested_world_seed(child),
            NbtTag::List(list) => list.iter().find_map(find_world_seed_in_tag),
            _ => None,
        };
        if let Some(seed) = nested_seed {
            return Some(seed);
        }
    }
    None
}

fn find_world_seed_in_tag(tag: &NbtTag) -> Option<i64> {
    match tag {
        NbtTag::Compound(compound) => find_nested_world_seed(compound),
        NbtTag::List(list) => list.iter().find_map(find_world_seed_in_tag),
        _ => None,
    }
}

fn numeric_seed(tag: &NbtTag) -> Option<i64> {
    match tag {
        NbtTag::Byte(seed) => Some(i64::from(*seed)),
        NbtTag::Short(seed) => Some(i64::from(*seed)),
        NbtTag::Int(seed) => Some(i64::from(*seed)),
        NbtTag::Long(seed) => Some(*seed),
        _ => None,
    }
}

fn validate_tile_request(request: &TileRequest) -> io::Result<()> {
    if !matches!(request.scale, 1 | 4 | 16 | 64 | 256) {
        return Err(io::Error::other(
            "Map scale must be one of 1, 4, 16, 64, or 256.",
        ));
    }
    if !(1..=256).contains(&request.width)
        || !(1..=256).contains(&request.height)
    {
        return Err(io::Error::other(
            "Map tiles must be between 1 and 256 pixels.",
        ));
    }
    if request
        .highlight_biomes
        .as_ref()
        .is_some_and(|biomes| biomes.len() > 256)
    {
        return Err(io::Error::other(
            "Too many highlighted biomes were requested.",
        ));
    }
    Ok(())
}

fn scaled_coordinate(block_coordinate: i32, scale: i32) -> i32 {
    block_coordinate.div_euclid(scale)
}

pub(crate) fn parse_seed(seed: &str) -> u64 {
    let trimmed = seed.trim();
    if let Ok(seed) = trimmed.parse::<i64>() {
        return seed as u64;
    }
    java_string_hash(trimmed) as i64 as u64
}

fn java_string_hash(value: &str) -> i32 {
    value.encode_utf16().fold(0_i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(unit as i32)
    })
}

fn feature_name(feature: u32) -> Option<&'static str> {
    match feature {
        FEATURE_VILLAGE => Some("village"),
        FEATURE_OUTPOST => Some("outpost"),
        FEATURE_SHIPWRECK => Some("shipwreck"),
        FEATURE_MONUMENT => Some("monument"),
        FEATURE_MANSION => Some("mansion"),
        FEATURE_ANCIENT_CITY => Some("ancient-city"),
        FEATURE_TRAIL_RUINS => Some("trail-ruins"),
        FEATURE_TRIAL_CHAMBERS => Some("trial-chambers"),
        FEATURE_RUINED_PORTAL => Some("ruined-portal"),
        FEATURE_STRONGHOLD => Some("stronghold"),
        FEATURE_SLIME_CHUNK => Some("slime-chunk"),
        FEATURE_DESERT_PYRAMID => Some("desert-pyramid"),
        FEATURE_JUNGLE_TEMPLE => Some("jungle-temple"),
        FEATURE_SWAMP_HUT => Some("swamp-hut"),
        FEATURE_IGLOO => Some("igloo"),
        FEATURE_OCEAN_RUIN => Some("ocean-ruin"),
        FEATURE_BURIED_TREASURE => Some("buried-treasure"),
        FEATURE_MINESHAFT => Some("mineshaft"),
        FEATURE_DESERT_WELL => Some("desert-well"),
        FEATURE_GEODE => Some("geode"),
        FEATURE_FORTRESS => Some("fortress"),
        FEATURE_BASTION => Some("bastion"),
        FEATURE_END_CITY => Some("end-city"),
        FEATURE_END_GATEWAY => Some("end-gateway"),
        FEATURE_ABANDONED_CAMP => Some("abandoned-camp"),
        FEATURE_NETHER_FOSSIL => Some("nether-fossil"),
        _ => None,
    }
}

fn normalize_level_dat_version(version: &str) -> Option<String> {
    let version = version.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

fn version_from_data_version(data_version: i32) -> Option<String> {
    let version = match data_version {
        5_023 => "26.3",
        4_903 => "26.2",
        4_786 => "26.1",
        4_671 => "1.21.11",
        4_554 => "1.21.9",
        4_437 => "1.21.6",
        4_325 => "1.21.5",
        4_326.. => return None,
        4_189.. => "1.21.4",
        4_082.. => "1.21.3",
        3_953.. => "1.21.1",
        3_463.. => "1.20",
        3_337.. => "1.19.4",
        3_105.. => "1.19.2",
        2_860.. => "1.18",
        2_724.. => "1.17",
        2_566.. => "1.16",
        2_225.. => "1.15",
        1_952.. => "1.14",
        1_519.. => "1.13",
        1_139.. => "1.12",
        921.. => "1.11",
        512.. => "1.10",
        169.. => "1.9",
        _ => return None,
    };
    Some(version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_seeds_use_minecrafts_java_hash() {
        assert_eq!(parse_seed("hello"), 99_162_322);
        assert_eq!(parse_seed("-42"), (-42_i64) as u64);
    }

    #[test]
    fn modern_versions_select_distinct_generation_rules() {
        let version = |name| resolve_java_version(name).unwrap().0;
        assert_ne!(version("26.2"), version("26.3"));
        assert_ne!(version("1.21.4"), version("1.21.5"));
        assert_eq!(version("1.20"), version("1.20.6"));
        assert_eq!(
            biome_at(
                "-3829811542736183482".into(),
                Edition::Java,
                "26.3".into(),
                Dimension::Overworld,
                68148,
                77,
                80990
            )
            .unwrap(),
            188
        );
        assert_eq!(
            biome_at(
                "856461443495910397".into(),
                Edition::Java,
                "26.2".into(),
                Dimension::End,
                -371136,
                64,
                -64
            )
            .unwrap(),
            43
        );
        assert_eq!(
            biome_at(
                "856461443495910397".into(),
                Edition::Java,
                "26.3".into(),
                Dimension::End,
                -371136,
                64,
                -64
            )
            .unwrap(),
            42
        );
        assert_eq!(
            normalize_level_dat_version("26.3-snapshot-6"),
            Some("26.3-snapshot-6".into())
        );
        assert_eq!(version_from_data_version(5023), Some("26.3".into()));
        assert_eq!(version_from_data_version(5005), None);
    }

    #[test]
    fn profiles_cover_java_and_large_biomes() {
        let profiles = version_profiles();
        assert!(profiles.iter().any(|profile| {
            matches!(profile.edition, Edition::Java)
                && profile.version == "1.21.3"
                && profile.available
                && profile.ores
        }));
        assert!(profiles.iter().any(|profile| {
            matches!(profile.edition, Edition::JavaLargeBiomes)
                && profile.version == "1.21.3"
        }));
        assert!(!profiles.iter().any(|profile| {
            matches!(profile.edition, Edition::JavaLargeBiomes)
                && profile.version == "1.0"
        }));
        let legacy = profiles
            .iter()
            .find(|profile| {
                matches!(profile.edition, Edition::Java)
                    && profile.version == "1.9"
            })
            .expect("1.9 profile should exist");
        assert!(!legacy.ores);
        assert_eq!(legacy.dimensions.len(), 2);
        let oldest = profiles
            .iter()
            .find(|profile| {
                matches!(profile.edition, Edition::Java)
                    && profile.version == "1.8"
            })
            .expect("1.8 profile should exist");
        assert_eq!(oldest.dimensions.len(), 1);
    }

    #[test]
    fn java_1_21_feature_fixture_matches_cubiomes() {
        let villages = find_features(FeatureQuery {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::Overworld,
            min_x: -4_096,
            min_z: -4_096,
            max_x: 4_096,
            max_z: 4_096,
            feature_mask: Some(FEATURE_VILLAGE),
        })
        .expect("fixture query should succeed");
        assert!(!villages.is_empty());
        assert!(villages.iter().all(|feature| feature.kind == "village"));

        let strongholds = find_features(FeatureQuery {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::Overworld,
            min_x: -4_096,
            min_z: -4_096,
            max_x: 4_096,
            max_z: 4_096,
            feature_mask: Some(FEATURE_STRONGHOLD),
        })
        .expect("fixture query should succeed");
        assert!(!strongholds.is_empty());
    }

    #[test]
    fn nether_fossils_require_terrain_and_use_block_positions() {
        for version in ["1.16.5", "1.18.2", "26.3"] {
            let query = FeatureQuery {
                seed: "10292992".into(),
                edition: Edition::Java,
                version: version.into(),
                dimension: Dimension::Nether,
                min_x: -512,
                min_z: -512,
                max_x: 511,
                max_z: 511,
                feature_mask: Some(FEATURE_NETHER_FOSSIL),
            };
            let fossils = find_features(query.clone()).unwrap();
            assert_eq!(
                fossils.len(),
                158,
                "{version}: terrain-filtered fixture"
            );
            assert!(fossils.iter().all(|f| f.kind == "nether-fossil"));
            assert!(fossils.iter().any(|f| f.x.rem_euclid(16) != 0));
            assert!(fossils.iter().any(|f| f.z.rem_euclid(16) != 0));
            let first = &fossils[0];
            let bounded = find_features(FeatureQuery {
                min_x: first.x,
                min_z: first.z,
                max_x: first.x,
                max_z: first.z,
                ..query
            })
            .unwrap();
            assert_eq!(bounded.len(), 1);
            assert_eq!((bounded[0].x, bounded[0].z), (first.x, first.z));
        }
    }

    #[test]
    fn end_island_layer_is_not_exposed_or_returned() {
        let removed_mask = 1 << 26;
        assert_eq!(ALL_FEATURES & removed_mask, 0);
        assert!(feature_name(removed_mask).is_none());
        for profile in version_profiles() {
            assert_eq!(profile.feature_masks[2] & removed_mask, 0);
        }
        assert!(
            find_features(FeatureQuery {
                seed: "12345".into(),
                edition: Edition::Java,
                version: "26.3".into(),
                dimension: Dimension::End,
                min_x: 1024,
                min_z: 1024,
                max_x: 4096,
                max_z: 4096,
                feature_mask: Some(removed_mask),
            })
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn nether_and_end_features_survive_the_native_bridge() {
        let nether = find_features(FeatureQuery {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::Nether,
            min_x: -4_096,
            min_z: -4_096,
            max_x: 4_096,
            max_z: 4_096,
            feature_mask: Some(FEATURE_FORTRESS | FEATURE_BASTION),
        })
        .expect("nether fixture query should succeed");
        assert!(!nether.is_empty());
        assert!(
            nether.iter().all(|feature| feature.kind == "fortress"
                || feature.kind == "bastion")
        );
        assert!(nether.iter().any(|feature| feature.kind == "fortress"));
        assert!(nether.iter().any(|feature| feature.kind == "bastion"));

        let end = find_features(FeatureQuery {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::End,
            min_x: -4_096,
            min_z: -4_096,
            max_x: 4_096,
            max_z: 4_096,
            feature_mask: Some(FEATURE_END_CITY | FEATURE_END_GATEWAY),
        })
        .expect("end fixture query should succeed");
        assert!(!end.is_empty());
        assert!(end.iter().all(|feature| feature.kind == "end-city"
            || feature.kind == "end-gateway"));
        assert!(end.iter().any(|feature| feature.kind == "end-city"));
        assert!(end.iter().any(|feature| feature.kind == "end-gateway"));
        let end_cities = end
            .iter()
            .filter(|feature| feature.kind == "end-city")
            .collect::<Vec<_>>();
        assert!(end_cities.iter().all(|feature| feature.end_ship.is_some()));
        assert!(
            end_cities
                .iter()
                .any(|feature| feature.end_ship == Some(true))
        );
        assert!(
            end_cities
                .iter()
                .any(|feature| feature.end_ship == Some(false))
        );
    }

    #[test]
    fn java_tiles_are_deterministic_for_a_fixed_request() {
        let request = TileRequest {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::Overworld,
            x: -128,
            z: -128,
            scale: 4,
            width: 32,
            height: 32,
            elevation: Some(62),
            terrain: true,
            contours: true,
            highlight_biomes: Some(vec![185]),
        };
        let first =
            render_tile(request.clone()).expect("fixture tile should render");
        let second = render_tile(request).expect("fixture tile should render");
        assert_eq!(first, second);
        assert_eq!(first.len(), 32 * 32 * 4);
        assert!(first.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn modern_overworld_scale_sixteen_renders_negative_tile_origins() {
        let request = TileRequest {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "26.2".to_owned(),
            dimension: Dimension::Overworld,
            x: -12_288,
            z: 0,
            scale: 16,
            width: 8,
            height: 8,
            elevation: Some(128),
            terrain: false,
            contours: false,
            highlight_biomes: None,
        };
        let rgba =
            render_tile(request).expect("negative scale-16 tile should render");
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[..3] != [0, 0, 0]));
    }

    #[test]
    fn terrain_shading_changes_the_tile() {
        let request = TileRequest {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "1.21.3".to_owned(),
            dimension: Dimension::Overworld,
            x: -128,
            z: -128,
            scale: 4,
            width: 32,
            height: 32,
            elevation: Some(62),
            terrain: false,
            contours: false,
            highlight_biomes: None,
        };
        let flat =
            render_tile(request.clone()).expect("flat tile should render");
        let shaded = render_tile(TileRequest {
            terrain: true,
            ..request
        })
        .expect("shaded tile should render");
        assert_ne!(flat, shaded);
    }

    #[test]
    fn end_tiles_render_deterministic_islands_and_void() {
        let request = TileRequest {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "26.2".to_owned(),
            dimension: Dimension::End,
            x: 100_096,
            z: 0,
            scale: 4,
            width: 64,
            height: 64,
            elevation: Some(62),
            terrain: true,
            contours: false,
            highlight_biomes: None,
        };
        let first =
            render_tile(request.clone()).expect("end tile should render");
        let second = render_tile(request).expect("end tile should render");
        assert_eq!(first, second);
        let void_pixels = first
            .chunks_exact(4)
            .filter(|pixel| {
                pixel[0] == 0x4B && pixel[1] == 0x4B && pixel[2] == 0xAB
            })
            .count();
        assert!(void_pixels > 0, "outer end tiles should contain void");
    }

    #[test]
    fn end_void_remains_purple_at_detailed_scales() {
        for scale in [1, 4] {
            let pixels = render_tile(TileRequest {
                seed: "10292992".to_owned(),
                edition: Edition::Java,
                version: "26.3".to_owned(),
                dimension: Dimension::End,
                x: 768,
                z: 0,
                scale,
                width: 32,
                height: 32,
                elevation: Some(62),
                terrain: true,
                contours: false,
                highlight_biomes: None,
            })
            .expect("end void tile should render");
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|pixel| pixel == [0x4B, 0x4B, 0xAB, 255]),
                "the gap between the central and outer islands should stay purple at scale {scale}"
            );
        }
    }

    #[test]
    fn nether_tiles_use_the_canonical_biome_render_without_fake_relief() {
        let request = TileRequest {
            seed: "10292992".to_owned(),
            edition: Edition::Java,
            version: "26.2".to_owned(),
            dimension: Dimension::Nether,
            x: -128,
            z: -128,
            scale: 4,
            width: 32,
            height: 32,
            elevation: Some(62),
            terrain: false,
            contours: false,
            highlight_biomes: None,
        };
        let flat =
            render_tile(request.clone()).expect("nether tile should render");
        let terrain_requested = render_tile(TileRequest {
            terrain: true,
            ..request
        })
        .expect("nether tile should render");
        assert_eq!(flat, terrain_requested);
    }

    #[test]
    fn biome_lookup_resolves_known_positions() {
        let biome = biome_at(
            "10292992".to_owned(),
            Edition::Java,
            "26.2".to_owned(),
            Dimension::Overworld,
            0,
            62,
            0,
        )
        .expect("biome lookup should succeed");
        assert!(biome >= 0);
        let nether = biome_at(
            "10292992".to_owned(),
            Edition::Java,
            "26.2".to_owned(),
            Dimension::Nether,
            0,
            64,
            0,
        )
        .expect("nether biome lookup should succeed");
        assert!(nether >= 0);
    }

    #[test]
    #[ignore = "writes preview tiles for manual shading inspection"]
    fn write_preview_tiles() {
        let Some(dir) = std::env::var_os("AXOLOTL_SEED_MAP_PREVIEW_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let cases: &[(&str, Dimension, i32, bool, bool, i32, i32)] = &[
            (
                "overworld_s1",
                Dimension::Overworld,
                1,
                true,
                false,
                -128,
                -128,
            ),
            (
                "overworld_s4",
                Dimension::Overworld,
                4,
                true,
                false,
                -512,
                -512,
            ),
            (
                "overworld_s4_contours",
                Dimension::Overworld,
                4,
                true,
                true,
                -512,
                -512,
            ),
            (
                "overworld_s16",
                Dimension::Overworld,
                16,
                true,
                false,
                -2048,
                -2048,
            ),
            (
                "overworld_s64",
                Dimension::Overworld,
                64,
                true,
                false,
                -8192,
                -8192,
            ),
            ("nether_s4", Dimension::Nether, 4, true, false, -512, -512),
            (
                "nether_s4_flat",
                Dimension::Nether,
                4,
                false,
                false,
                -512,
                -512,
            ),
            ("end_s4", Dimension::End, 4, false, false, 99_840, -512),
            ("end_s16", Dimension::End, 16, false, false, -2048, -2048),
        ];
        for &(name, dimension, scale, terrain, contours, x, z) in cases {
            let rgba = render_tile(TileRequest {
                seed: "10292992".to_owned(),
                edition: Edition::Java,
                version: "26.2".to_owned(),
                dimension,
                x,
                z,
                scale,
                width: 256,
                height: 256,
                elevation: Some(62),
                terrain,
                contours,
                highlight_biomes: None,
            })
            .expect("preview tile should render");
            let mut ppm = Vec::with_capacity(256 * 256 * 3 + 32);
            ppm.extend_from_slice(b"P6\n256 256\n255\n");
            for pixel in rgba.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(dir.join(format!("{name}.ppm")), ppm)
                .expect("preview tile should be written");
        }
    }

    #[test]
    fn tile_origins_follow_the_scaled_coordinate_contract() {
        assert_eq!(scaled_coordinate(1_024, 64), 16);
        assert_eq!(scaled_coordinate(-1, 64), -1);
        assert_eq!(scaled_coordinate(-1_025, 64), -17);
    }

    #[test]
    fn level_dat_versions_preserve_actual_game_version() {
        assert_eq!(
            normalize_level_dat_version("1.21.3"),
            Some("1.21.3".to_owned())
        );
        assert_eq!(
            normalize_level_dat_version("1.20.4"),
            Some("1.20.4".to_owned())
        );
        assert_eq!(
            normalize_level_dat_version("1.17.1"),
            Some("1.17.1".to_owned())
        );
        assert_eq!(version_from_data_version(3_700), Some("1.20".to_owned()));
        assert_eq!(version_from_data_version(100), None);
    }

    #[test]
    fn level_dat_reads_embedded_world_generation_settings() {
        let mut settings = NbtCompound::new();
        settings.insert("seed", 1_234_567_i64);
        let mut data = NbtCompound::new();
        data.insert("WorldGenSettings", settings);
        assert_eq!(extract_world_seed(&data), Some(1_234_567));
    }

    #[test]
    fn level_dat_reads_split_world_generation_settings() {
        let temp_dir =
            tempfile::tempdir().expect("temporary world should be created");
        let level_dat_path = temp_dir.path().join("level.dat");
        let mut version = NbtCompound::new();
        version.insert("Name", "26.2");
        let mut level_data = NbtCompound::new();
        level_data.insert("Version", version);
        let mut level_root = NbtCompound::new();
        level_root.insert("Data", level_data);
        let mut level_dat = std::fs::File::create(&level_dat_path)
            .expect("level.dat should be created");
        quartz_nbt::io::write_nbt(
            &mut level_dat,
            None,
            &level_root,
            quartz_nbt::io::Flavor::GzCompressed,
        )
        .expect("level.dat should be written");

        let settings_path = temp_dir
            .path()
            .join("data")
            .join("minecraft")
            .join("world_gen_settings.dat");
        std::fs::create_dir_all(settings_path.parent().unwrap())
            .expect("world generation settings directory should be created");
        let mut settings_data = NbtCompound::new();
        settings_data.insert("seed", -7_654_321_i64);
        let mut settings_root = NbtCompound::new();
        settings_root.insert("data", settings_data);
        let mut settings_dat = std::fs::File::create(settings_path)
            .expect("world generation settings should be created");
        quartz_nbt::io::write_nbt(
            &mut settings_dat,
            None,
            &settings_root,
            quartz_nbt::io::Flavor::GzCompressed,
        )
        .expect("world generation settings should be written");

        let info =
            read_level_dat(level_dat_path.to_string_lossy().into_owned())
                .expect("split world generation settings should be read");
        assert_eq!(info.seed, "-7654321");
        assert_eq!(info.version, Some("26.2".to_owned()));
    }
}
