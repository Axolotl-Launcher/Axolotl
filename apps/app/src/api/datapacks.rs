use crate::api::Result;
use either::Either;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use theseus::worlds::{World, WorldDatapack, WorldWithDatapacks};

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("datapacks")
        .invoke_handler(tauri::generate_handler![
            list_datapacks,
            delete_datapack,
            set_datapack_enabled
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn list_datapacks<R: Runtime>(
    app_handle: AppHandle<R>,
    instance_id: &str,
) -> Result<Vec<WorldWithDatapacks>> {
    let mut result = theseus::worlds::list_world_datapacks(instance_id).await?;
    for world in &mut result {
        adapt_world_icon(&app_handle, &mut world.world);
        for datapack in &mut world.datapacks {
            adapt_datapack_icon(&app_handle, datapack);
        }
    }
    Ok(result)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn delete_datapack(
    instance_id: String,
    world_path: String,
    file_name: String,
) -> Result<()> {
    theseus::worlds::delete_world_datapack(
        &instance_id,
        &world_path,
        &file_name,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn set_datapack_enabled(
    instance_id: String,
    world_path: String,
    file_name: String,
    enabled: bool,
) -> Result<()> {
    theseus::worlds::set_world_datapack_enabled(
        &instance_id,
        &world_path,
        &file_name,
        enabled,
    )
    .await?;
    Ok(())
}

fn adapt_world_icon<R: Runtime>(app_handle: &AppHandle<R>, world: &mut World) {
    adapt_icon_field(app_handle, &mut world.icon, &world.name);
}

fn adapt_datapack_icon<R: Runtime>(
    app_handle: &AppHandle<R>,
    datapack: &mut WorldDatapack,
) {
    adapt_icon_field(app_handle, &mut datapack.icon, &datapack.display_name);
}

fn adapt_icon_field<R: Runtime>(
    app_handle: &AppHandle<R>,
    icon: &mut Option<Either<PathBuf, url::Url>>,
    label: &str,
) {
    if let Some(Either::Left(icon_path)) = icon {
        let icon_path = icon_path.clone();
        if let Ok(new_url) = super::utils::tauri_convert_file_src(&icon_path) {
            *icon = Some(Either::Right(new_url));
            if let Err(error) =
                app_handle.asset_protocol_scope().allow_file(&icon_path)
            {
                tracing::warn!(
                    "Failed to allow file access for icon {}: {}",
                    icon_path.display(),
                    error
                );
            }
        } else {
            tracing::warn!(
                "Encountered invalid icon path for {}: {}",
                label,
                icon_path.display()
            );
            *icon = None;
        }
    }
}
