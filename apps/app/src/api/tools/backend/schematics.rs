use super::{
    LauncherBackend, business_error,
    files::{nbt, paths},
    page, serialize,
};
use crate::api::schematic_preview::{
    DetachedSchematic, PreviewBlockEdit, PreviewBlockState, PreviewTransform,
};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
#[derive(Clone)]
pub(super) struct Document {
    data: DetachedSchematic,
    revision: u32,
}
fn error(e: impl std::fmt::Display) -> ToolError {
    tracing::warn!(%e,"MCP schematic failed");
    ToolError::new(
        "schematic_failed",
        "Schematic is unsupported, expired, out of bounds or exceeds MCP limits",
    )
}
fn review(
    backend: &LauncherBackend,
    session: &str,
    document: Document,
) -> Result<Value, ToolError> {
    let summary = summary(document.data.manifest());
    let revision = document.revision;
    let id = backend.schematics.insert(session, revision, document)?;
    Ok(json!({"schematic_id":id,"revision":revision,"summary":summary}))
}
fn summary(m: &crate::api::schematic_preview::PreviewManifest) -> Value {
    json!({"format":m.format,"format_version":m.format_version,"data_version":m.data_version,"size":m.size,"min":m.min,"max":m.max,"block_count":m.block_count,"entity_count":m.entity_count,"block_entity_count":m.block_entity_count,"palette_count":m.palette.len(),"region_count":m.regions.len(),"warning_count":m.warnings.len()})
}
pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    if let Command::SchematicListFiles {
        instance_id,
        offset,
        limit,
    } = command
    {
        let root = paths::root("instance", &instance_id, false).await?;
        let files=tokio::task::spawn_blocking(move||{
            let dir=paths::directory(&paths::open_root(&root)?,"schematics")?;let mut items=Vec::new();let mut truncated=false;
            for (scanned,entry) in dir.entries().map_err(paths::error)?.take(10001).enumerate(){if scanned>=10000{truncated=true;break;}let entry=entry.map_err(paths::error)?;let name=entry.file_name().to_string_lossy().into_owned();let m=dir.symlink_metadata(&name).map_err(paths::error)?;
                if m.is_file()&&!m.file_type().is_symlink()&&[".schem",".litematic"].iter().any(|suffix|name.to_ascii_lowercase().ends_with(suffix)){items.push(json!({"path":format!("schematics/{name}"),"file_name":name,"bytes":m.len()}));}
            }items.sort_by(|a,b|a["path"].as_str().cmp(&b["path"].as_str()));Ok::<_,ToolError>((items,truncated))
        }).await.map_err(error)??;
        let mut result =
            page(files.0, offset.unwrap_or(0), limit.unwrap_or(50))?;
        result["scan_truncated"] = json!(files.1);
        return Ok(result);
    }
    if let Command::SchematicOpen {
        instance_id,
        path,
        expected_sha256,
    } = command
    {
        let root = paths::root("instance", &instance_id, false).await?;
        let document = tokio::task::spawn_blocking(move || {
            let bytes =
                paths::read(&paths::open_root(&root)?, &path, 2 * 1024 * 1024)?;
            paths::check(&bytes, &expected_sha256)?;
            nbt::validate_document(&bytes)?;
            let extension = std::path::Path::new(&path)
                .extension()
                .and_then(|s| s.to_str())
                .filter(|e| matches!(*e, "schem" | "litematic"))
                .ok_or_else(|| error("extension"))?;
            let temp = tempfile::Builder::new()
                .suffix(&format!(".{extension}"))
                .tempfile()
                .map_err(error)?;
            std::fs::write(temp.path(), bytes).map_err(error)?;
            Ok::<_, ToolError>(Document {
                data: DetachedSchematic::open(temp.path()).map_err(error)?,
                revision: 1,
            })
        })
        .await
        .map_err(error)??;
        return review(backend, &context.session_id, document);
    }
    let (id, revision) = match &command {
        Command::SchematicGet {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicGetBlock {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicReadRegion {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicApplyEdits {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicTransform {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicExport {
            schematic_id,
            revision,
            ..
        }
        | Command::SchematicClose {
            schematic_id,
            revision,
        } => (*schematic_id, *revision as u32),
        _ => return Err(error("command")),
    };
    if matches!(command, Command::SchematicClose { .. }) {
        backend.schematics.take(&context.session_id, id, revision)?;
        return Ok(json!({"closed":true}));
    }
    let mut document =
        backend.schematics.get(&context.session_id, id, revision)?;
    match command {
        Command::SchematicGet {
            section,
            offset,
            limit,
            ..
        } => {
            let m = document.data.manifest();
            match section.as_str(){"summary"=>Ok(summary(m)),"palette"=>page(m.palette.clone(),offset.unwrap_or(0),limit.unwrap_or(50)),"materials"=>page(m.materials.clone(),offset.unwrap_or(0),limit.unwrap_or(50)),"regions"=>page(m.regions.iter().map(|r|json!({"region_id":r.id,"name":r.name,"origin":r.origin,"size":r.size,"min":r.min,"max":r.max,"block_count":r.block_count})).collect(),offset.unwrap_or(0),limit.unwrap_or(50)),_=>Err(error("section"))}
        }
        Command::SchematicGetBlock {
            region_id, x, y, z, ..
        } => serialize(
            document.data.block(&region_id, [x, y, z]).map_err(error)?,
        ),
        Command::SchematicReadRegion {
            region_id,
            x,
            y,
            z,
            width,
            height,
            depth,
            ..
        } => {
            if width * height * depth > 1024 {
                return Err(error("volume"));
            }
            tokio::task::spawn_blocking(move||{let mut items=Vec::new();for dy in 0..height{for dz in 0..depth{for dx in 0..width{let p=[x+dx as i32,y+dy as i32,z+dz as i32];items.push(json!({"position":p,"state":document.data.block(&region_id,p).map_err(error)?}));}}}Ok(json!({"blocks":items}))}).await.map_err(error)?
        }
        Command::SchematicApplyEdits {
            edits_json,
            target_state_json,
            ..
        } => {
            let edits: Vec<PreviewBlockEdit> =
                serde_json::from_str(&edits_json).map_err(error)?;
            if edits.is_empty() || edits.len() > 256 {
                return Err(error("edit count"));
            }
            let target: Option<PreviewBlockState> = target_state_json
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(error)?;
            if target.as_ref().is_some_and(|s| {
                s.name.len() > 256
                    || s.properties.len() > 32
                    || s.properties
                        .iter()
                        .any(|(k, v)| k.len() > 128 || v.len() > 256)
            }) {
                return Err(error("block state"));
            }
            document = tokio::task::spawn_blocking(move || {
                document.data.edit(edits, target).map_err(error)?;
                Ok::<_, ToolError>(document)
            })
            .await
            .map_err(error)??;
            backend.schematics.take(&context.session_id, id, revision)?;
            document.revision += 1;
            review(backend, &context.session_id, document)
        }
        Command::SchematicTransform { transform, .. } => {
            let t: PreviewTransform =
                serde_json::from_value(json!(transform)).map_err(error)?;
            document = tokio::task::spawn_blocking(move || {
                document.data.transform(t).map_err(error)?;
                Ok::<_, ToolError>(document)
            })
            .await
            .map_err(error)??;
            backend.schematics.take(&context.session_id, id, revision)?;
            document.revision += 1;
            review(backend, &context.session_id, document)
        }
        Command::SchematicExport {
            format,
            target_kind,
            target_id,
            path,
            ..
        } => {
            let root = paths::root(&target_kind, &target_id, true).await?;
            let _guard = if target_kind == "instance" {
                Some(
                    theseus::instance::lock_content_file_edit(&target_id)
                        .await
                        .map_err(business_error)?,
                )
            } else {
                None
            };
            tokio::task::spawn_blocking(move||{let bytes=document.data.export(&format).map_err(error)?;if bytes.len()>16*1024*1024{return Err(error("export size"));}paths::write(&paths::open_root(&root)?,&path,&bytes,None,true)?;Ok(json!({"path":path,"bytes":bytes.len(),"sha256":paths::hash(&bytes)}))}).await.map_err(error)?
        }
        _ => Err(error("command")),
    }
}
