use crate::api::Result;
use theseus::prelude::*;
use theseus::servers::ServerInfo;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("servers")
        .invoke_handler(tauri::generate_handler![
            servers_list,
            servers_get,
            servers_create,
            servers_update_settings,
            servers_set_icon,
            servers_set_linked_world,
            servers_is_local_address,
            servers_wait_until_ready,
            servers_delete,
            servers_read_file,
            servers_write_file,
            servers_download_file,
            servers_install_forge,
            servers_install_neoforge,
            servers_install_modpack,
            servers_start,
            servers_send_console_input,
            servers_resize_console,
            servers_stop,
            servers_kill,
            servers_kill_port_process,
            servers_port_process,
            servers_get_log_buffer,
            servers_get_log_delta,
            servers_get_console_delta,
            servers_clear_log,
        ])
        .build()
}

#[tauri::command]
pub async fn servers_list() -> Result<Vec<ServerInfo>> {
    Ok(servers::list().await?)
}

#[tauri::command]
pub async fn servers_get(server_id: &str) -> Result<ServerInfo> {
    Ok(servers::get(server_id).await?)
}

#[tauri::command]
pub async fn servers_create(
    name: &str,
    server_type: &str,
    game_version: &str,
    loader_version: Option<String>,
    java_path: Option<String>,
    memory_mb: Option<u32>,
) -> Result<servers::ServerManifest> {
    Ok(servers::create(
        name,
        server_type,
        game_version,
        loader_version,
        java_path,
        memory_mb,
    )
    .await?)
}

#[tauri::command]
pub async fn servers_update_settings(
    server_id: &str,
    name: Option<String>,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
    pre_launch_hook: Option<String>,
    home_pinned: Option<bool>,
    stop_command: Option<String>,
) -> Result<servers::ServerManifest> {
    Ok(servers::update_settings(
        server_id,
        name,
        java_path,
        memory_mb,
        jvm_args,
        pre_launch_hook,
        home_pinned,
        stop_command,
    )
    .await?)
}

#[tauri::command]
pub async fn servers_set_icon(
    server_id: &str,
    icon_path: Option<String>,
) -> Result<servers::ServerManifest> {
    Ok(servers::set_icon(server_id, icon_path).await?)
}

/// Links a server to the saved multiplayer entry its address points at, or
/// clears the link when `linked` is `None`.
#[tauri::command]
pub async fn servers_set_linked_world(
    server_id: &str,
    linked: Option<servers::LinkedWorld>,
) -> Result<servers::ServerManifest> {
    Ok(servers::set_linked_world(server_id, linked).await?)
}

/// Whether a server address points at this machine (localhost, loopback, or a
/// local interface address).
#[tauri::command]
pub async fn servers_is_local_address(address: &str) -> Result<bool> {
    Ok(servers::is_local_address(address))
}

/// Waits until a running server accepts connections on its port.
#[tauri::command]
pub async fn servers_wait_until_ready(
    server_id: &str,
    timeout_ms: Option<u64>,
) -> Result<bool> {
    Ok(servers::wait_until_ready(
        server_id,
        timeout_ms.unwrap_or(servers::DEFAULT_READY_TIMEOUT_MS),
    )
    .await?)
}

#[tauri::command]
pub async fn servers_delete(server_id: &str) -> Result<()> {
    Ok(servers::delete(server_id).await?)
}

#[tauri::command]
pub async fn servers_read_file(server_id: &str, file: &str) -> Result<String> {
    Ok(servers::read_file(server_id, file).await?)
}

#[tauri::command]
pub async fn servers_write_file(
    server_id: &str,
    file: &str,
    contents: &str,
) -> Result<()> {
    Ok(servers::write_file(server_id, file, contents).await?)
}

#[tauri::command]
pub async fn servers_download_file(
    server_id: &str,
    url: &str,
    filename: &str,
    expected_sha1: Option<String>,
) -> Result<()> {
    Ok(servers::download_file(server_id, url, filename, expected_sha1).await?)
}

#[tauri::command]
pub async fn servers_install_forge(
    server_id: &str,
    mc_version: &str,
    build: &str,
    java_path: Option<String>,
) -> Result<()> {
    Ok(servers::install_forge(server_id, mc_version, build, java_path).await?)
}

#[tauri::command]
pub async fn servers_install_neoforge(
    server_id: &str,
    mc_version: &str,
    build: &str,
    java_path: Option<String>,
) -> Result<()> {
    Ok(
        servers::install_neoforge(server_id, mc_version, build, java_path)
            .await?,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn servers_install_modpack(
    server_id: &str,
    mrpack_url: &str,
    mrpack_sha1: Option<String>,
    jar_url: Option<&str>,
    jar_filename: Option<&str>,
    jar_sha1: Option<String>,
    modpack_project_id: Option<String>,
    modpack_version_id: Option<String>,
    modpack_title: Option<String>,
    modpack_icon_url: Option<String>,
) -> Result<()> {
    Ok(servers::install_modpack(
        server_id,
        mrpack_url,
        mrpack_sha1,
        jar_url,
        jar_filename,
        jar_sha1,
        modpack_project_id,
        modpack_version_id,
        modpack_title,
        modpack_icon_url,
    )
    .await?)
}

#[tauri::command]
pub async fn servers_start(
    server_id: &str,
    java_path: Option<String>,
    memory_mb: Option<u32>,
    jvm_args: Option<Vec<String>>,
) -> Result<()> {
    Ok(servers::start(server_id, java_path, memory_mb, jvm_args).await?)
}

#[tauri::command]
pub async fn servers_send_console_input(
    server_id: &str,
    data: &str,
) -> Result<()> {
    Ok(servers::send_console_input(server_id, data).await?)
}

#[tauri::command]
pub async fn servers_resize_console(
    server_id: &str,
    cols: u16,
    rows: u16,
) -> Result<()> {
    Ok(servers::resize_console(server_id, cols, rows).await?)
}

#[tauri::command]
pub async fn servers_stop(server_id: &str) -> Result<()> {
    Ok(servers::stop(server_id).await?)
}

#[tauri::command]
pub async fn servers_kill(server_id: &str) -> Result<()> {
    Ok(servers::kill(server_id).await?)
}

#[tauri::command]
pub async fn servers_kill_port_process(port: u16) -> Result<()> {
    Ok(servers::kill_port_process(port).await?)
}

#[tauri::command]
pub async fn servers_port_process(
    port: u16,
) -> Result<Option<servers::PortProcessInfo>> {
    Ok(servers::port_process(port).await?)
}

#[tauri::command]
pub async fn servers_get_log_buffer(server_id: &str) -> Result<Vec<String>> {
    Ok(servers::get_log_buffer(server_id).await?)
}

#[tauri::command]
pub async fn servers_get_log_delta(
    server_id: &str,
    cursor: Option<servers::LogCursor>,
) -> Result<servers::LogDelta> {
    Ok(servers::get_log_delta(server_id, cursor))
}

#[tauri::command]
pub async fn servers_get_console_delta(
    server_id: &str,
    cursor: Option<servers::LogCursor>,
) -> Result<servers::ConsoleDelta> {
    Ok(servers::get_console_delta(server_id, cursor))
}

#[tauri::command]
pub async fn servers_clear_log(server_id: &str) -> Result<()> {
    Ok(servers::clear_log(server_id).await?)
}
