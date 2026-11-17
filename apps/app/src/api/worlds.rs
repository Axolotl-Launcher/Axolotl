use crate::api::Result;
use crate::api::instance::InstanceRunResult;
use either::Either;
use enumset::EnumSet;
use tauri::{AppHandle, Manager, Runtime};
use theseus::instance::{self, GcLaunchIntent, QuickPlayType, get_full_path};
use theseus::server_address::ServerAddress;
use theseus::worlds;
use theseus::worlds::{
    DisplayStatus, ProtocolVersion, ServerPackStatus, ServerStatus, World,
    WorldLevelData, WorldSettingsPatch, WorldType, WorldWithInstance,
};

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("worlds")
        .invoke_handler(tauri::generate_handler![
            get_recent_worlds,
            get_favorite_worlds,
            get_instance_worlds,
            get_singleplayer_world,
            set_world_display_status,
            get_world_level_data,
            update_world_settings,
            rename_world,
            reset_world_icon,
            backup_world,
            delete_world,
            add_server_to_instance,
            edit_server_in_instance,
            remove_server_from_instance,
            get_instance_protocol_version,
            get_server_status,
            start_join_singleplayer_world,
            start_join_server,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_recent_worlds<R: Runtime>(
    app_handle: AppHandle<R>,
    limit: usize,
    display_statuses: Option<EnumSet<DisplayStatus>>,
) -> Result<Vec<WorldWithInstance>> {
    let mut result = worlds::get_recent_worlds(
        limit,
        display_statuses.unwrap_or(EnumSet::all()),
    )
    .await?;
    for world in &mut result {
        adapt_world_icon(&app_handle, &mut world.world);
    }
    Ok(result)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_favorite_worlds<R: Runtime>(
    app_handle: AppHandle<R>,
) -> Result<Vec<WorldWithInstance>> {
    let mut result = worlds::get_favorite_worlds().await?;
    for world in &mut result {
        adapt_world_icon(&app_handle, &mut world.world);
    }
    Ok(result)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_instance_worlds<R: Runtime>(
    app_handle: AppHandle<R>,
    instance_id: &str,
) -> Result<Vec<World>> {
    let mut result = worlds::get_instance_worlds(instance_id).await?;
    for world in &mut result {
        adapt_world_icon(&app_handle, world);
    }
    Ok(result)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_singleplayer_world<R: Runtime>(
    app_handle: AppHandle<R>,
    instance: &str,
    world: &str,
) -> Result<World> {
    let mut world = worlds::get_singleplayer_world(instance, world).await?;
    adapt_world_icon(&app_handle, &mut world);
    Ok(world)
}

fn adapt_world_icon<R: Runtime>(app_handle: &AppHandle<R>, world: &mut World) {
    adapt_icon_field(app_handle, &mut world.icon, &world.name);
}

fn adapt_icon_field<R: Runtime>(
    app_handle: &AppHandle<R>,
    icon: &mut Option<Either<std::path::PathBuf, url::Url>>,
    world_name: &str,
) {
    if let Some(Either::Left(icon_path)) = icon {
        let icon_path = icon_path.clone();
        if let Ok(new_url) = super::utils::tauri_convert_file_src(&icon_path) {
            *icon = Some(Either::Right(new_url));
            if let Err(e) =
                app_handle.asset_protocol_scope().allow_file(&icon_path)
            {
                tracing::warn!(
                    "Failed to allow file access for icon {}: {}",
                    icon_path.display(),
                    e
                );
            }
        } else {
            tracing::warn!(
                "Encountered invalid icon path for world {}: {}",
                world_name,
                icon_path.display()
            );
            *icon = None;
        }
    }
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_world_level_data<R: Runtime>(
    app_handle: AppHandle<R>,
    instance: &str,
    world: &str,
) -> Result<WorldLevelData> {
    let mut data = worlds::get_world_level_data(instance, world).await?;
    let name = data.name.clone();
    adapt_icon_field(&app_handle, &mut data.icon, &name);
    Ok(data)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn update_world_settings(
    instance: &str,
    world: &str,
    patch: WorldSettingsPatch,
) -> Result<()> {
    let instance = get_full_path(instance).await?;
    worlds::update_world_settings(&instance, world, patch).await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn set_world_display_status(
    instance: &str,
    world_type: WorldType,
    world_id: &str,
    display_status: DisplayStatus,
) -> Result<()> {
    Ok(worlds::set_world_display_status(
        instance,
        world_type,
        world_id,
        display_status,
    )
    .await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn rename_world(
    instance: &str,
    world: &str,
    new_name: &str,
) -> Result<()> {
    let instance = get_full_path(instance).await?;
    worlds::rename_world(&instance, world, new_name).await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn reset_world_icon(instance: &str, world: &str) -> Result<()> {
    let instance = get_full_path(instance).await?;
    worlds::reset_world_icon(&instance, world).await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn backup_world(instance: &str, world: &str) -> Result<u64> {
    let instance = get_full_path(instance).await?;
    Ok(worlds::backup_world(&instance, world).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn delete_world(instance: &str, world: &str) -> Result<()> {
    let instance = get_full_path(instance).await?;
    worlds::delete_world(&instance, world).await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn add_server_to_instance(
    instance_id: &str,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
    project_id: Option<String>,
    content_kind: Option<String>,
) -> Result<usize> {
    Ok(worlds::add_server_to_instance(
        instance_id,
        name,
        address,
        pack_status,
        project_id,
        content_kind,
    )
    .await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn edit_server_in_instance(
    instance_id: &str,
    index: usize,
    name: String,
    address: String,
    pack_status: ServerPackStatus,
) -> Result<()> {
    worlds::edit_server_in_instance(
        instance_id,
        index,
        name,
        address,
        pack_status,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn remove_server_from_instance(
    instance_id: &str,
    index: usize,
) -> Result<()> {
    worlds::remove_server_from_instance(instance_id, index).await?;
    Ok(())
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_instance_protocol_version(
    instance_id: &str,
) -> Result<Option<ProtocolVersion>> {
    Ok(worlds::get_instance_protocol_version(instance_id).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn get_server_status(
    address: &str,
    protocol_version: Option<ProtocolVersion>,
) -> Result<ServerStatus> {
    Ok(worlds::get_server_status(address, protocol_version).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn start_join_singleplayer_world(
    instance_id: &str,
    world: String,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
    gc_intent: Option<GcLaunchIntent>,
) -> Result<InstanceRunResult> {
    let (process, gc_notice) = instance::run_with_extra_launch_args_with_gc(
        instance_id,
        QuickPlayType::Singleplayer(world),
        offline_mode,
        extra_launch_args,
        gc_intent,
    )
    .await?;

    Ok(InstanceRunResult { process, gc_notice })
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn start_join_server(
    instance_id: &str,
    address: &str,
    offline_mode: bool,
    extra_launch_args: Option<Vec<String>>,
    gc_intent: Option<GcLaunchIntent>,
) -> Result<InstanceRunResult> {
    let (process, gc_notice) = instance::run_with_extra_launch_args_with_gc(
        instance_id,
        QuickPlayType::Server(ServerAddress::Unresolved(address.to_owned())),
        offline_mode,
        extra_launch_args,
        gc_intent,
    )
    .await?;

    Ok(InstanceRunResult { process, gc_notice })
}
