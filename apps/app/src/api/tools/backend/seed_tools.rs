use super::{
    LauncherBackend,
    files::{nbt, paths},
    page, serialize,
};
use crate::seed_map::{self as s, Dimension, Edition, TileRequest, details};
use base64::{Engine, engine::general_purpose::STANDARD};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
#[cfg(test)]
#[path = "seed_tools_tests.rs"]
mod tests;
fn error(e: impl std::fmt::Display) -> ToolError {
    tracing::warn!(%e,"MCP seed computation failed");
    ToolError::new(
        "seed_computation_failed",
        "Unsupported version/dimension, invalid bounds or native computation failed",
    )
}
fn edition(s: &str) -> Result<Edition, ToolError> {
    serde_json::from_value(json!(s)).map_err(error)
}
fn dimension(s: &str) -> Result<Dimension, ToolError> {
    serde_json::from_value(json!(s)).map_err(error)
}
fn image(pixels: Vec<u8>, w: usize, h: usize) -> Result<Value, ToolError> {
    let image = image::RgbaImage::from_raw(w as u32, h as u32, pixels)
        .ok_or_else(|| error("pixels"))?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(error)?;
    Ok(
        json!({"image":{"data":STANDARD.encode(bytes.into_inner()),"mime_type":"image/png"},"context":{"width":w,"height":h,"prediction":true}}),
    )
}
pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    if let Command::SeedReadWorld {
        instance_id,
        world_id,
    } = command
    {
        let root = paths::root("instance", &instance_id, false).await?;
        return tokio::task::spawn_blocking(move || {
            let dir = paths::open_root(&root)?;
            let bytes = paths::read(
                &dir,
                &format!("saves/{world_id}/level.dat"),
                2 * 1024 * 1024,
            )?;
            nbt::validate_document(&bytes)?;
            let temp = tempfile::tempdir().map_err(error)?;
            std::fs::write(temp.path().join("level.dat"), bytes)
                .map_err(error)?;
            let relative = format!(
                "saves/{world_id}/data/minecraft/world_gen_settings.dat"
            );
            if let Ok(bytes) = paths::read(&dir, &relative, 2 * 1024 * 1024) {
                nbt::validate_document(&bytes)?;
                std::fs::create_dir_all(temp.path().join("data/minecraft"))
                    .map_err(error)?;
                std::fs::write(
                    temp.path().join("data/minecraft/world_gen_settings.dat"),
                    bytes,
                )
                .map_err(error)?;
            }
            serialize(
                s::read_level_dat(
                    temp.path()
                        .join("level.dat")
                        .to_string_lossy()
                        .into_owned(),
                )
                .map_err(error)?,
            )
        })
        .await
        .map_err(error)?;
    }
    tokio::task::spawn_blocking(move||match command{
        Command::SeedGetProfiles{offset,limit}=>{let profiles=s::version_profiles().into_iter().map(|p|json!({"edition":p.edition,"version":p.version,"available":p.available,"dimensions":p.dimensions,"ores":p.ores,"ore_kinds":p.ore_kinds,"feature_masks":p.feature_masks,"underground_masks":p.underground_masks,"underground_y_ranges":p.underground_y_ranges,"engine_revision":p.engine_revision,"note":p.note})).collect();page(profiles,offset.unwrap_or(0),limit.unwrap_or(50))},
        Command::SeedGetSpawn{seed,edition:e,version}=>serialize(s::spawn(seed,edition(&e)?,version).map_err(error)?),
        Command::SeedGetBiome{seed,edition:e,version,dimension:d,x,y,z}=>Ok(json!({"biome_id":s::biome_at(seed,edition(&e)?,version,dimension(&d)?,x,y,z).map_err(error)?})),
        Command::SeedFindFeatures{seed,edition:e,version,dimension:d,min_x,min_z,max_x,max_z,feature_mask,offset,limit}=>{
            if min_x>max_x||min_z>max_z||i64::from(max_x)-i64::from(min_x)>8192||i64::from(max_z)-i64::from(min_z)>8192{return Err(error("search bounds"));}
            let result=s::find_features(s::FeatureQuery{seed,edition:edition(&e)?,version,dimension:dimension(&d)?,min_x,min_z,max_x,max_z,feature_mask:feature_mask.map(|v|v as u32)}).map_err(error)?;let capped=result.len()>=4096;let mut result=page(result,offset.unwrap_or(0),limit.unwrap_or(50))?;result["native_result_cap_reached"]=json!(capped);Ok(result)
        },
        c@(Command::SeedRenderTile{..}|Command::SeedRenderUnderground{..})=>{
            let underground=matches!(c,Command::SeedRenderUnderground{..});let(seed,e,version,d,x,z,scale,width,height,elevation,terrain,contours,layers)=match c{
                Command::SeedRenderTile{seed,edition,version,dimension,x,z,scale,width,height,elevation,terrain,contours}=>(seed,edition,version,dimension,x,z,scale,width,height,elevation,terrain,contours,0),
                Command::SeedRenderUnderground{seed,edition,version,dimension,x,z,scale,width,height,elevation,terrain,contours,layers}=>(seed,edition,version,dimension,x,z,scale,width,height,Some(elevation),terrain,contours,layers),_=>unreachable!()};
            let tile=TileRequest{seed,edition:edition(&e)?,version,dimension:dimension(&d)?,x,z,scale:scale.parse().map_err(error)?,width:width as i32,height:height as i32,elevation,terrain:terrain.unwrap_or(false),contours:contours.unwrap_or(false),highlight_biomes:None};
            image(if underground{details::underground_tile(details::UndergroundRequest{tile,layers:layers as u32})}else{s::render_tile(tile)}.map_err(error)?,width,height)
        },
        c@(Command::SeedGetStructureDetails{..}|Command::SeedGetContainerLoot{..})=>{let(seed,e,version,d,x,z,feature_mask,container)=match c{Command::SeedGetStructureDetails{seed,edition,version,dimension,x,z,feature_mask}=>(seed,edition,version,dimension,x,z,feature_mask,None),Command::SeedGetContainerLoot{seed,edition,version,dimension,x,z,feature_mask,container}=>(seed,edition,version,dimension,x,z,feature_mask,Some(container as u32)),_=>unreachable!()};let result=details::structure_details(details::StructureQuery{seed,edition:edition(&e)?,version,dimension:dimension(&d)?,x,z,feature_mask:feature_mask as u32,include_blocks:false},container).map_err(error)?;if serde_json::to_vec(&result).map_err(error)?.len()>512*1024{return Err(error("result size"));}Ok(result)},
        Command::SeedScanOres{seed,edition:e,version,dimension:d,ore_kinds,chunks}=>{if chunks.len()%2!=0{return Err(error("chunk pairs"));}let ores=ore_kinds.into_iter().map(|v|serde_json::from_value(json!(v)).map_err(error)).collect::<Result<_,_>>()?;serialize(s::ores::scan_ores(s::ores::OreScanRequest{seed,edition:edition(&e)?,version,dimension:dimension(&d)?,ores,chunks}).map_err(error)?)},
        _=>Err(error("command")),
    }).await.map_err(error)?
}
