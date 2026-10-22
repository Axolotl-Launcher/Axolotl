use std::{
    ffi::{CStr, CString},
    io::{self, Read},
    sync::Mutex,
};

use serde::Deserialize;

use super::{
    Dimension, Edition, TileRequest, parse_seed, resolve_java_version,
};

static STRUCTURE_COMPUTATION: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureQuery {
    pub seed: String,
    pub edition: Edition,
    pub version: String,
    pub dimension: Dimension,
    pub x: i32,
    pub z: i32,
    pub feature_mask: u32,
    #[serde(default)]
    pub include_blocks: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UndergroundRequest {
    pub tile: TileRequest,
    pub layers: u32,
}

struct NativeResult(*mut std::ffi::c_char);

impl Drop for NativeResult {
    fn drop(&mut self) {
        unsafe { super::axolotl_seed_map_free_result(self.0) };
    }
}

fn ancient_city_layout(
    query: &StructureQuery,
    mc: i32,
) -> io::Result<serde_json::Value> {
    let compressed = super::template_resources::ancient_city_resources(
        &query.version,
    )
    .ok_or_else(|| {
        io::Error::other(
            "Ancient city detailed layouts are not bundled for this version.",
        )
    })?
    .0;
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .map_err(io::Error::other)?;
    let layout = CString::new(decoded).map_err(|_| {
        io::Error::other("Ancient city layout contains an invalid byte.")
    })?;
    let result = NativeResult(unsafe {
        super::axolotl_seed_map_ancient_city_layout(
            parse_seed(&query.seed),
            mc,
            query.x,
            query.z,
            layout.as_ptr(),
            layout.as_bytes().len(),
        )
    });
    if result.0.is_null() {
        return Err(io::Error::other(
            "The ancient city layout engine could not allocate a result.",
        ));
    }
    serde_json::from_slice(unsafe { CStr::from_ptr(result.0) }.to_bytes())
        .map_err(io::Error::other)
}

fn ancient_city_block_pack(
    query: &StructureQuery,
) -> io::Result<serde_json::Value> {
    let compressed = super::template_resources::ancient_city_resources(
        &query.version,
    )
    .ok_or_else(|| {
        io::Error::other(
            "Ancient city block templates are not bundled for this version.",
        )
    })?
    .1;
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .map_err(io::Error::other)?;
    let pack: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(io::Error::other)?;
    if !pack["templates"].is_object()
        || pack["version"]
            .as_str()
            .and_then(super::template_resources::ancient_city_resources)
            .is_none_or(|resource| resource.1 != compressed)
    {
        return Err(io::Error::other(
            "Ancient city block template version mismatch.",
        ));
    }
    Ok(pack)
}

fn ancient_city_blocks(
    pieces: &serde_json::Value,
    pack: &serde_json::Value,
) -> io::Result<serde_json::Value> {
    let mut blocks =
        std::collections::HashMap::<[i64; 3], serde_json::Value>::new();
    for piece in pieces.as_array().into_iter().flatten() {
        let template_id = piece["templateId"].as_str().ok_or_else(|| {
            io::Error::other("Ancient city template ID is missing.")
        })?;
        let template = pack["templates"].get(template_id).ok_or_else(|| {
            io::Error::other(format!(
                "Ancient city template {template_id} is missing."
            ))
        })?;
        let origin = piece["position"].as_array().ok_or_else(|| {
            io::Error::other("Ancient city template position is invalid.")
        })?;
        let origin = [
            origin[0].as_i64().unwrap_or_default(),
            origin[1].as_i64().unwrap_or_default(),
            origin[2].as_i64().unwrap_or_default(),
        ];
        let rotation = piece["rotation"]
            .as_i64()
            .unwrap_or_default()
            .rem_euclid(360);
        for block in template["blocks"].as_array().into_iter().flatten() {
            if blocks.len() >= 2_000_000 {
                return Err(io::Error::other(
                    "Ancient city block limit exceeded.",
                ));
            }
            let values = block.as_array().ok_or_else(|| {
                io::Error::other("Ancient city block entry is invalid.")
            })?;
            let local = [
                values[0].as_i64().unwrap_or_default(),
                values[1].as_i64().unwrap_or_default(),
                values[2].as_i64().unwrap_or_default(),
            ];
            let index = values[3].as_u64().unwrap_or_default() as usize;
            let state = template["palette"]
                .as_array()
                .and_then(|palette| palette.get(index))
                .ok_or_else(|| {
                    io::Error::other("Ancient city palette index is invalid.")
                })?;
            let name = state["name"].as_str().unwrap_or_default();
            if name.ends_with(":jigsaw")
                || name.ends_with(":structure_void")
                || name.ends_with(":structure_block")
            {
                continue;
            }
            let (x, z) = match rotation {
                0 => (local[0], local[2]),
                90 => (-local[2], local[0]),
                180 => (-local[0], -local[2]),
                270 => (local[2], -local[0]),
                _ => {
                    return Err(io::Error::other(
                        "Ancient city rotation is invalid.",
                    ));
                }
            };
            let position = [origin[0] + x, origin[1] + local[1], origin[2] + z];
            if name == "minecraft:air"
                || name == "minecraft:cave_air"
                || name == "minecraft:void_air"
            {
                if template["overwriteAir"].as_bool().unwrap_or(true) {
                    blocks.remove(&position);
                }
                continue;
            }
            blocks.insert(position, serde_json::json!({
                "position": [origin[0] + x, origin[1] + local[1], origin[2] + z],
                "state": state,
                "rotation": rotation,
                "mirror": false,
                "pieceId": piece["id"],
                "overwriteAir": template["overwriteAir"].as_bool().unwrap_or(true),
            }));
        }
    }
    let mut blocks = blocks.into_values().collect::<Vec<_>>();
    blocks.sort_by_key(|block| {
        let position = block["position"].as_array().unwrap();
        (
            position[0].as_i64().unwrap_or_default(),
            position[1].as_i64().unwrap_or_default(),
            position[2].as_i64().unwrap_or_default(),
        )
    });
    Ok(serde_json::Value::Array(blocks))
}

fn ancient_city_containers(
    pieces: &serde_json::Value,
    pack: &serde_json::Value,
) -> io::Result<serde_json::Value> {
    let mut containers = Vec::new();
    for piece in pieces.as_array().into_iter().flatten() {
        let template_id = piece["templateId"].as_str().ok_or_else(|| {
            io::Error::other("Ancient city template ID is missing.")
        })?;
        let template = pack["templates"].get(template_id).ok_or_else(|| {
            io::Error::other(format!(
                "Ancient city template {template_id} is missing."
            ))
        })?;
        let origin = piece["position"].as_array().ok_or_else(|| {
            io::Error::other("Ancient city template position is invalid.")
        })?;
        let origin = [
            origin[0].as_i64().unwrap_or_default(),
            origin[1].as_i64().unwrap_or_default(),
            origin[2].as_i64().unwrap_or_default(),
        ];
        let rotation = piece["rotation"]
            .as_i64()
            .unwrap_or_default()
            .rem_euclid(360);
        for chest in template["chests"].as_array().into_iter().flatten() {
            if containers.len() >= 8192 {
                return Err(io::Error::other(
                    "Ancient city container limit exceeded.",
                ));
            }
            let local = chest["position"].as_array().ok_or_else(|| {
                io::Error::other("Ancient city chest position is invalid.")
            })?;
            let local = [
                local[0].as_i64().unwrap_or_default(),
                local[1].as_i64().unwrap_or_default(),
                local[2].as_i64().unwrap_or_default(),
            ];
            let (x, z) = match rotation {
                0 => (local[0], local[2]),
                90 => (-local[2], local[0]),
                180 => (-local[0], -local[2]),
                270 => (local[2], -local[0]),
                _ => {
                    return Err(io::Error::other(
                        "Ancient city rotation is invalid.",
                    ));
                }
            };
            let table = chest["lootTable"].as_str();
            if table.is_none()
                && chest["items"].as_array().is_none_or(Vec::is_empty)
            {
                continue;
            }
            containers.push(serde_json::json!({
                "id": containers.len(),
                "piece": piece["id"],
                "x": origin[0] + x,
                "y": origin[1] + local[1],
                "z": origin[2] + z,
                "table": table,
                "estimated": table.is_some(),
                "fixedItems": chest["items"],
            }));
        }
    }
    Ok(serde_json::Value::Array(containers))
}

fn estimate_ancient_city_loot_seed(
    seed: u64,
    container: &serde_json::Value,
) -> u64 {
    let x = container["x"].as_i64().unwrap_or_default() as u64;
    let y = container["y"].as_i64().unwrap_or_default() as u64;
    let z = container["z"].as_i64().unwrap_or_default() as u64;
    let mut value =
        seed ^ x.rotate_left(17) ^ y.rotate_left(31) ^ z.rotate_left(47);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn ancient_city_container_loot(
    query: &StructureQuery,
    mc: i32,
    container_id: u32,
    layout: &serde_json::Value,
    pack: &serde_json::Value,
) -> io::Result<serde_json::Value> {
    let containers = ancient_city_containers(&layout["pieces"], pack)?;
    let container = containers
        .as_array()
        .and_then(|containers| containers.get(container_id as usize))
        .ok_or_else(|| {
            io::Error::other("Ancient city container is unavailable.")
        })?;
    if let Some(items) = container["fixedItems"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        return Ok(serde_json::json!({
            "supported": true,
            "estimated": false,
            "truncated": false,
            "items": items.iter().map(|item| serde_json::json!({
                "id": item["id"],
                "count": item["count"],
                "enchantments": [],
            })).collect::<Vec<_>>(),
            "unresolvedTables": [],
            "limits": [],
        }));
    }
    let table_name = container["table"].as_str().ok_or_else(|| {
        io::Error::other("Ancient city loot table is unavailable.")
    })?;
    let table = pack["lootTables"].get(table_name).ok_or_else(|| {
        io::Error::other(format!(
            "Ancient city loot table {table_name} is not bundled."
        ))
    })?;
    let table_json =
        CString::new(serde_json::to_vec(table).map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
    let result = NativeResult(unsafe {
        super::axolotl_seed_map_generate_loot(
            table_json.as_ptr(),
            table_json.as_bytes().len(),
            estimate_ancient_city_loot_seed(parse_seed(&query.seed), container),
            mc,
        )
    });
    if result.0.is_null() {
        return Err(io::Error::other(
            "The ancient city loot table could not be parsed.",
        ));
    }
    let mut loot: serde_json::Value =
        serde_json::from_slice(unsafe { CStr::from_ptr(result.0) }.to_bytes())
            .map_err(io::Error::other)?;
    if loot["supported"] != true {
        return Err(io::Error::other(
            "The ancient city loot table is unsupported.",
        ));
    }
    Ok(std::mem::take(&mut loot))
}

pub fn structure_details(
    query: StructureQuery,
    container: Option<u32>,
) -> io::Result<serde_json::Value> {
    let (mc, _) = resolve_java_version(&query.version)?;
    if query.x.abs_diff(0) > 30_000_000
        || query.z.abs_diff(0) > 30_000_000
        || query.feature_mask.count_ones() != 1
        || query.feature_mask & super::ALL_FEATURES == 0
        || container.is_some_and(|index| index >= 8192)
    {
        return Err(io::Error::other("The structure query is invalid."));
    }
    let supported = unsafe {
        super::axolotl_seed_map_capability(mc, 1, query.dimension.as_cubiomes())
    } as u32;
    if supported & query.feature_mask == 0 {
        return Err(io::Error::other(
            "This structure is not supported in the selected version and dimension.",
        ));
    }
    let _guard = STRUCTURE_COMPUTATION.lock().map_err(|_| {
        io::Error::other(
            "Structure calculation was interrupted. Retry this query.",
        )
    })?;
    if query.feature_mask == super::FEATURE_ANCIENT_CITY {
        let layout = ancient_city_layout(&query, mc)?;
        let pack = ancient_city_block_pack(&query)?;
        if let Some(container) = container {
            return ancient_city_container_loot(
                &query, mc, container, &layout, &pack,
            );
        }
        let mut result = layout;
        let containers = ancient_city_containers(&result["pieces"], &pack)?;
        let has_containers = !containers.as_array().is_none_or(Vec::is_empty);
        result["containers"] = containers.clone();
        result["capabilities"]["containers"] =
            serde_json::Value::Bool(has_containers);
        if containers.as_array().is_some_and(|containers| {
            containers
                .iter()
                .any(|container| container["estimated"] == true)
        }) {
            if let Some(limits) = result["limits"].as_array_mut() {
                limits.push(serde_json::json!("loot_seed_estimated"));
            }
        }
        if query.include_blocks {
            result["blocks"] = ancient_city_blocks(&result["pieces"], &pack)?;
            result["complete"] = serde_json::Value::Bool(false);
            if let Some(limits) = result["limits"].as_array_mut() {
                limits.push(serde_json::json!(
                    "terrain_dependent_processors_not_applied"
                ));
            }
        }
        if let Some(object) = result.as_object_mut() {
            if let Some(pieces) = object
                .get_mut("pieces")
                .and_then(serde_json::Value::as_array_mut)
            {
                for piece in pieces {
                    let position = piece
                        .get("position")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let min_values = piece
                        .get("min")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let max_values = piece
                        .get("max")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let to_position = |values: Vec<serde_json::Value>| {
                        serde_json::json!({
                            "x": values.first().and_then(serde_json::Value::as_i64).unwrap_or_default(),
                            "y": values.get(1).and_then(serde_json::Value::as_i64).unwrap_or_default(),
                            "z": values.get(2).and_then(serde_json::Value::as_i64).unwrap_or_default(),
                        })
                    };
                    let template_id = piece
                        .get("templateId")
                        .cloned()
                        .unwrap_or(serde_json::Value::String("unknown".into()));
                    piece["name"] = template_id.clone();
                    piece["positionKnown"] = serde_json::Value::Bool(true);
                    piece["position"] = to_position(position);
                    piece["min"] = to_position(min_values);
                    piece["max"] = to_position(max_values);
                }
            }
            object.insert("truncated".into(), serde_json::json!(false));
            object.insert(
                "capabilities".into(),
                serde_json::json!({
                    "variant": true,
                    "containers": has_containers,
                    "projection": true,
                    "blocks": true
                }),
            );
            object.insert(
                "limits".into(),
                object
                    .get("limits")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([])),
            );
        }
        return Ok(result);
    }
    let result = NativeResult(unsafe {
        super::axolotl_seed_map_structure_details(
            parse_seed(&query.seed),
            mc,
            query.edition.generator_flags(),
            query.dimension.as_cubiomes(),
            query.x,
            query.z,
            query.feature_mask,
            container.map_or(-1, |index| index as i32),
        )
    });
    if result.0.is_null() {
        return Err(io::Error::other(
            "The engine could not allocate the structure result.",
        ));
    }
    let bytes = unsafe { CStr::from_ptr(result.0) }.to_bytes();
    serde_json::from_slice(bytes).map_err(io::Error::other)
}

pub fn underground_tile(request: UndergroundRequest) -> io::Result<Vec<u8>> {
    let tile = request.tile;
    let (mc, code) = resolve_java_version(&tile.version)?;
    super::validate_tile_request(&tile)?;
    let min_y =
        if matches!(tile.dimension, Dimension::Overworld) && code >= 11800 {
            -64
        } else {
            0
        };
    let max_y = if matches!(tile.dimension, Dimension::Overworld) {
        if code >= 11800 { 319 } else { 255 }
    } else {
        127
    };
    let y = tile.elevation.unwrap_or(16);
    if code < 11300
        || (request.layers & 1 != 0 && code < 11400)
        || tile.width > 64
        || tile.height > 64
        || tile.width % 16 != 0
        || tile.height % 16 != 0
        || !matches!(tile.scale, 1 | 4)
        || tile.x % 16 != 0
        || tile.z % 16 != 0
        || y < min_y
        || y > max_y
        || request.layers & !7 != 0
    {
        return Err(io::Error::other(
            "The underground tile bounds, height or layers are invalid.",
        ));
    }
    let supported = unsafe {
        super::axolotl_seed_map_capability(mc, 3, tile.dimension.as_cubiomes())
    } as u32;
    if request.layers & !supported != 0 {
        return Err(io::Error::other(
            "These underground layers are not supported in the selected version and dimension.",
        ));
    }
    let mut rgba = vec![0; tile.width as usize * tile.height as usize * 4];
    let status = unsafe {
        super::axolotl_seed_map_underground(
            parse_seed(&tile.seed),
            mc,
            tile.edition.generator_flags(),
            tile.dimension.as_cubiomes(),
            tile.x,
            tile.z,
            tile.scale,
            tile.width,
            tile.height,
            y,
            request.layers,
            rgba.as_mut_ptr(),
            rgba.len() as u64,
        )
    };
    if status != 0 {
        return Err(io::Error::other(format!(
            "Underground generation failed ({status})."
        )));
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(
        seed: &str,
        version: &str,
        x: i32,
        z: i32,
        feature_mask: u32,
    ) -> StructureQuery {
        StructureQuery {
            seed: seed.into(),
            edition: Edition::Java,
            version: version.into(),
            dimension: Dimension::Overworld,
            x,
            z,
            feature_mask,
            include_blocks: false,
        }
    }
    #[test]
    fn abandoned_camp_matches_upstream_vanilla_fixture() {
        let query = query(
            "-3829811542736183482",
            "26.3",
            224,
            -384,
            super::super::FEATURE_ABANDONED_CAMP,
        );
        let result = structure_details(query.clone(), None).unwrap();
        assert_eq!(result["supported"], true);
        assert_eq!(result["capabilities"]["containers"], true);
        assert_eq!(result["pieces"].as_array().unwrap().len(), 2);
        assert!(
            result["pieces"][0]["name"]
                .as_str()
                .unwrap()
                .ends_with("_9")
        );
        assert_eq!(
            result["containers"][0]["table"],
            "barrels/abandoned_camp_barrel"
        );
        let loot = structure_details(query, Some(0)).unwrap();
        assert_eq!(loot["supported"], true);
        assert!(!loot["items"].as_array().unwrap().is_empty());
    }
    #[test]
    fn stronghold_details_identify_the_end_portal_room_by_piece_type() {
        let feature = super::super::find_features(super::super::FeatureQuery {
            seed: "10292992".into(),
            edition: Edition::Java,
            version: "26.3".into(),
            dimension: Dimension::Overworld,
            min_x: -4096,
            min_z: -4096,
            max_x: 4096,
            max_z: 4096,
            feature_mask: Some(super::super::FEATURE_STRONGHOLD),
        })
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let result = structure_details(
            query(
                "10292992",
                "26.3",
                feature.x,
                feature.z,
                super::super::FEATURE_STRONGHOLD,
            ),
            None,
        )
        .unwrap();
        let portal_rooms: Vec<_> = result["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|piece| piece["endPortalRoom"] == true)
            .collect();
        assert_eq!(portal_rooms.len(), 1);
        assert_eq!(portal_rooms[0]["name"], "SHPR");
    }

    #[test]
    fn ancient_city_details_expand_versioned_jigsaw_templates() {
        for version in [
            "1.19.2", "1.19.4", "1.20.6", "1.20", "1.21.1", "1.21.3", "1.21.4",
            "1.21.5", "1.21.6", "1.21.9", "1.21.11", "26.1", "26.1.2", "26.2",
            "26.3",
        ] {
            let feature =
                super::super::find_features(super::super::FeatureQuery {
                    seed: "10292992".into(),
                    edition: Edition::Java,
                    version: version.into(),
                    dimension: Dimension::Overworld,
                    min_x: -8192,
                    min_z: -8192,
                    max_x: 8192,
                    max_z: 8192,
                    feature_mask: Some(super::super::FEATURE_ANCIENT_CITY),
                })
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            let mut structure_query = query(
                "10292992",
                version,
                feature.x,
                feature.z,
                super::super::FEATURE_ANCIENT_CITY,
            );
            let result =
                structure_details(structure_query.clone(), None).unwrap();
            assert_eq!(result["supported"], true, "{version}");
            assert!(
                result["pieces"].as_array().unwrap().len() > 1,
                "{version} should include adjoining city pieces: {result}"
            );
            assert!(
                result["pieces"][0]["templateId"]
                    .as_str()
                    .unwrap()
                    .contains("ancient_city"),
                "{version}"
            );
            assert!(result["pieces"][0]["min"]["x"].is_number(), "{version}");
            assert!(
                (i64::from(feature.x)
                    - result["pieces"][0]["position"]["x"].as_i64().unwrap())
                .abs()
                    <= 64,
                "{version} returned a city outside its world X coordinate"
            );
            assert!(
                (i64::from(feature.z)
                    - result["pieces"][0]["position"]["z"].as_i64().unwrap())
                .abs()
                    <= 64,
                "{version} returned a city outside its world Z coordinate"
            );
            structure_query.include_blocks = true;
            let detailed = structure_details(structure_query, None).unwrap();
            assert!(
                detailed["blocks"]
                    .as_array()
                    .is_some_and(|blocks| !blocks.is_empty()),
                "{version}"
            );
        }
    }
    #[test]
    fn ancient_city_nbt_chests_expose_estimated_loot_and_fixed_items() {
        for version in [
            "1.19.2", "1.19.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4",
            "1.21.5", "1.21.6", "1.21.9", "1.21.11", "26.1", "26.1.2", "26.2",
            "26.3",
        ] {
            let query = query(
                "10292992",
                version,
                0,
                0,
                super::super::FEATURE_ANCIENT_CITY,
            );
            let pack = ancient_city_block_pack(&query).unwrap();
            let barracks = serde_json::json!([{
                "id": 0,
                "templateId": "minecraft:ancient_city/structures/barracks",
                "position": [120, -40, -88],
                "rotation": 90,
            }]);
            let containers = ancient_city_containers(&barracks, &pack).unwrap();
            assert_eq!(containers.as_array().unwrap().len(), 2);
            assert_eq!(containers[0]["x"], 105);
            assert_eq!(containers[0]["y"], -36);
            assert_eq!(containers[0]["z"], -84);
            assert_eq!(containers[0]["table"], "chests/ancient_city");
            assert_eq!(containers[0]["estimated"], true);
            let layout = serde_json::json!({ "pieces": barracks });
            let loot = ancient_city_container_loot(
                &query,
                resolve_java_version(version).unwrap().0,
                0,
                &layout,
                &pack,
            )
            .unwrap();
            assert_eq!(loot["supported"], true);
            assert_eq!(loot["estimated"], true);
            assert!(!loot["items"].as_array().unwrap().is_empty());

            let city_center = serde_json::json!([{
                "id": 0,
                "templateId": "minecraft:ancient_city/city_center/city_center_2",
                "position": [0, 0, 0],
                "rotation": 0,
            }]);
            let layout = serde_json::json!({ "pieces": city_center });
            let fixed = ancient_city_container_loot(
                &query,
                resolve_java_version(version).unwrap().0,
                0,
                &layout,
                &pack,
            )
            .unwrap();
            assert_eq!(fixed["estimated"], false);
            assert_eq!(fixed["items"][0]["id"], "minecraft:golden_apple");
            assert_eq!(fixed["items"][0]["count"], 1);
        }
    }
    #[test]
    fn outpost_loot_matches_upstream_enchantment_fixture() {
        let result = structure_details(
            query(
                "-7594343816690189614",
                "1.21.9",
                1344,
                -9872,
                super::super::FEATURE_OUTPOST,
            ),
            Some(0),
        )
        .unwrap();
        assert!(result["items"].as_array().unwrap().iter().any(
            |item| item["id"] == "minecraft:book"
                && item["enchantments"].as_array().unwrap().iter().any(
                    |enchantment| {
                        enchantment["id"]
                            .as_str()
                            .is_some_and(|id| id.ends_with("fortune"))
                            && enchantment["level"] == 1
                    }
                )
        ));
    }
    #[test]
    fn shipwreck_bounds_use_world_coordinates_and_placeholder_heights() {
        for x in [1024, 2048, -4096, 8192] {
            let result = structure_details(
                query(
                    "10292992",
                    "26.3",
                    x,
                    -1024,
                    super::super::FEATURE_SHIPWRECK,
                ),
                None,
            )
            .unwrap();
            let piece = &result["pieces"][0];
            assert!(
                result["limits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|limit| limit == "height_placeholder")
            );
            for axis in ["x", "z"] {
                let origin = piece["position"][axis].as_i64().unwrap();
                let min = piece["min"][axis].as_i64().unwrap();
                let max = piece["max"][axis].as_i64().unwrap();
                assert!(min <= origin && origin <= max);
                assert!(max - min < 32);
                for container in result["containers"].as_array().unwrap() {
                    let position = container[axis].as_i64().unwrap();
                    assert!(min <= position && position <= max);
                }
            }
            assert!(piece["min"]["y"].as_i64().unwrap() >= 64);
            assert_eq!(piece["rotation"], result["variant"]["rotation"]);
        }
    }
    #[test]
    fn underground_slices_are_bounded_and_deterministic() {
        let request = UndergroundRequest {
            tile: TileRequest {
                seed: "12345".into(),
                edition: Edition::Java,
                version: "26.3".into(),
                dimension: Dimension::Overworld,
                x: -16,
                z: 0,
                scale: 1,
                width: 16,
                height: 16,
                elevation: Some(-32),
                terrain: false,
                contours: false,
                highlight_biomes: None,
            },
            layers: 7,
        };
        let image = underground_tile(request.clone()).unwrap();
        assert_eq!(image.len(), 16 * 16 * 4);
        assert!(image.chunks_exact(4).all(|pixel| pixel[3] == 255));
        assert_eq!(image, underground_tile(request.clone()).unwrap());
        let mut legacy = request.clone();
        legacy.tile.version = "1.13.2".into();
        legacy.tile.elevation = Some(32);
        legacy.layers = 6;
        assert_eq!(underground_tile(legacy).unwrap().len(), 16 * 16 * 4);
        let mut invalid = request;
        invalid.tile.elevation = Some(320);
        assert!(underground_tile(invalid).is_err());
    }
}
