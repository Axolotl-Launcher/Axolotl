use super::{
    LauncherBackend, business_error, files::paths, page, reviewed, serialize,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use std::{io::Cursor, path::PathBuf};
use theseus::instance::{self as api, ScreenshotKey};
fn key(instance_id: String, file_name: String) -> ScreenshotKey {
    ScreenshotKey {
        instance_id,
        file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screenshot_resize_keeps_small_images_and_bounds_large_ones() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(64, 32)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let result = png(bytes.get_ref(), 1024).unwrap();
        assert_eq!(image::load_from_memory(&result).unwrap().width(), 64);
        let result = png(bytes.get_ref(), 16).unwrap();
        let decoded = image::load_from_memory(&result).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 8));
        assert!(png(b"not image", 128).is_err());
    }
}
async fn path(key: &ScreenshotKey) -> Result<PathBuf, ToolError> {
    let root = paths::root("instance", &key.instance_id, false).await?;
    paths::valid(&key.file_name, false)?;
    if key.file_name.contains('/')
        || !key.file_name.to_ascii_lowercase().ends_with(".png")
    {
        return Err(paths::error("name"));
    }
    let screenshot = api::get_screenshot_path(key)
        .await
        .map_err(business_error)?;
    let root = tokio::fs::canonicalize(&root).await.map_err(paths::error)?;
    if !screenshot.starts_with(&root) {
        return Err(paths::error("screenshot path"));
    }
    Ok(screenshot)
}
async fn bytes(key: &ScreenshotKey) -> Result<Vec<u8>, ToolError> {
    let root = paths::root("instance", &key.instance_id, false).await?;
    path(key).await?;
    let relative = format!("screenshots/{}", key.file_name);
    tokio::task::spawn_blocking(move || {
        paths::read(&paths::open_root(&root)?, &relative, 32 * 1024 * 1024)
    })
    .await
    .map_err(paths::error)?
}
pub(super) fn png(bytes: &[u8], dimension: u32) -> Result<Vec<u8>, ToolError> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(paths::error)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let original = reader.decode().map_err(paths::error)?;
    let image = if original.width() > dimension || original.height() > dimension
    {
        original.thumbnail(dimension, dimension)
    } else {
        original
    };
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(paths::error)?;
    let bytes = output.into_inner();
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(paths::error("output image"));
    }
    Ok(bytes)
}
pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::ScreenshotList {
            instance_id,
            group_id,
            offset,
            limit,
        } => {
            let (mut items, skipped) = if let Some(id) = instance_id {
                (
                    api::list_screenshots(&id).await.map_err(business_error)?,
                    vec![],
                )
            } else {
                let scan = api::list_all_screenshots_with_warnings()
                    .await
                    .map_err(business_error)?;
                (scan.screenshots, scan.skipped_instances)
            };
            if let Some(group) = group_id {
                items.retain(|s| s.group_id.as_ref() == Some(&group));
            }
            let mut result =
                page(items, offset.unwrap_or(0), limit.unwrap_or(50))?;
            result["skipped_instances"] = json!(skipped);
            Ok(result)
        }
        Command::ScreenshotGet {
            instance_id,
            file_name,
            max_dimension,
        } => {
            let source =
                bytes(&key(instance_id.clone(), file_name.clone())).await?;
            let sha = paths::hash(&source);
            let dimension = max_dimension.unwrap_or(1024) as u32;
            let data =
                tokio::task::spawn_blocking(move || png(&source, dimension))
                    .await
                    .map_err(paths::error)??;
            Ok(
                json!({"image":{"data":STANDARD.encode(data),"mime_type":"image/png"},"context":{"instance_id":instance_id,"file_name":file_name,"source_sha256":sha}}),
            )
        }
        Command::ScreenshotSaveEdit {
            instance_id,
            file_name,
            png_base64,
            mode,
            expected_sha256,
        } => {
            reviewed::instance(&instance_id).await?;
            let key = key(instance_id, file_name);
            paths::check(&bytes(&key).await?, &expected_sha256)?;
            let raw = STANDARD.decode(png_base64).map_err(paths::error)?;
            if raw.len() > 2 * 1024 * 1024
                || !raw.starts_with(b"\x89PNG\r\n\x1a\n")
            {
                return Err(paths::error("image"));
            }
            let raw = tokio::task::spawn_blocking(move || png(&raw, 16384))
                .await
                .map_err(paths::error)??;
            serialize(
                api::save_edited_screenshot(
                    key,
                    raw,
                    if mode == "create_copy" {
                        api::ScreenshotEditSaveMode::CreateCopy
                    } else {
                        api::ScreenshotEditSaveMode::ReplaceEdit
                    },
                )
                .await
                .map_err(business_error)?,
            )
        }
        c @ (Command::ScreenshotDelete { .. }
        | Command::ScreenshotMove { .. }
        | Command::ScreenshotExport { .. }) => {
            let (instance, file_names) = match &c {
                Command::ScreenshotDelete {
                    instance_id,
                    file_names,
                }
                | Command::ScreenshotMove {
                    instance_id,
                    file_names,
                    ..
                }
                | Command::ScreenshotExport {
                    instance_id,
                    file_names,
                    ..
                } => (instance_id, file_names),
                _ => unreachable!(),
            };
            reviewed::instance(instance).await?;
            let mut seen = std::collections::HashSet::new();
            let mut keys = Vec::new();
            for name in file_names {
                if !seen.insert(name) {
                    return Err(paths::error("duplicate"));
                }
                let key = key(instance.clone(), name.clone());
                path(&key).await?;
                keys.push(key);
            }
            match c {
                Command::ScreenshotDelete { .. } => {
                    api::delete_screenshots(&keys)
                        .await
                        .map_err(business_error)?;
                    Ok(json!({"deleted_count":keys.len()}))
                }
                Command::ScreenshotMove {
                    target_instance_id, ..
                } => {
                    reviewed::instance(&target_instance_id).await?;
                    let root =
                        paths::root("instance", &target_instance_id, true)
                            .await?;
                    let screenshots = root.join("screenshots");
                    match tokio::fs::symlink_metadata(&screenshots).await {
                        Ok(metadata)
                            if !metadata.is_dir()
                                || reviewed::linked(&metadata) =>
                        {
                            return Err(paths::error(
                                "linked screenshots directory",
                            ));
                        }
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err(paths::error(e));
                        }
                        _ => {}
                    }
                    serialize(
                        api::move_screenshots(&keys, &target_instance_id)
                            .await
                            .map_err(business_error)?,
                    )
                }
                Command::ScreenshotExport {
                    target_kind,
                    target_id,
                    path,
                    ..
                } => {
                    let root =
                        paths::root(&target_kind, &target_id, true).await?;
                    let mut total = 0_u64;
                    for key in &keys {
                        total = total.saturating_add(
                            tokio::fs::metadata(self::path(key).await?)
                                .await
                                .map_err(paths::error)?
                                .len(),
                        );
                        if total > 64 * 1024 * 1024 {
                            return Err(paths::error("export size limit"));
                        }
                    }
                    let temp = tempfile::Builder::new()
                        .suffix(".zip")
                        .tempfile()
                        .map_err(paths::error)?;
                    api::export_screenshots(&keys, temp.path().to_owned())
                        .await
                        .map_err(business_error)?;
                    if tokio::fs::metadata(temp.path())
                        .await
                        .map_err(paths::error)?
                        .len()
                        > 65 * 1024 * 1024
                    {
                        return Err(paths::error("export size limit"));
                    }
                    let data = tokio::fs::read(temp.path())
                        .await
                        .map_err(paths::error)?;
                    tokio::task::spawn_blocking(move||{paths::write(&paths::open_root(&root)?,&path,&data,None,true)?;Ok(json!({"path":path,"bytes":data.len(),"sha256":paths::hash(&data)}))}).await.map_err(paths::error)?
                }
                _ => unreachable!(),
            }
        }
        Command::ScreenshotGroupList {} => serialize(
            api::list_screenshot_groups()
                .await
                .map_err(business_error)?,
        ),
        Command::ScreenshotGroupCreate { name } => serialize(
            api::create_screenshot_group(name, vec![])
                .await
                .map_err(business_error)?,
        ),
        Command::ScreenshotGroupRename { group_id, name } => serialize(
            api::rename_screenshot_group(group_id, name)
                .await
                .map_err(business_error)?,
        ),
        Command::ScreenshotGroupDelete { group_id } => {
            api::delete_screenshot_group(group_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"deleted":true,"images_deleted":false}))
        }
        Command::ScreenshotSetGroup {
            screenshot_ids,
            group_id,
        } => {
            let updates = screenshot_ids
                .into_iter()
                .map(|screenshot_id| api::ScreenshotGroupMembershipUpdate {
                    screenshot_id,
                    group_id: group_id.clone(),
                })
                .collect();
            api::set_screenshot_group_memberships(updates)
                .await
                .map_err(business_error)?;
            Ok(json!({"saved":true}))
        }
        _ => Err(paths::error("command")),
    }
}
