use crate::api::Result;
use serde::Serialize;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("terracotta")
        .invoke_handler(tauri::generate_handler![
            terracotta_get_state,
            terracotta_get_meta,
            terracotta_start,
            terracotta_stop,
            terracotta_host,
            terracotta_join,
            terracotta_reset,
            terracotta_get_platform_key,
            terracotta_check_for_update,
            terracotta_download,
            terracotta_update,
            terracotta_get_player_name,
            terracotta_get_diagnostic_report,
        ])
        .build()
}

#[tauri::command]
pub async fn terracotta_get_state()
-> Result<theseus::terracotta::TerracottaState> {
    Ok(theseus::terracotta::get_state().await)
}

#[derive(Serialize)]
pub struct TerracottaMetaResponse {
    pub version: String,
    pub compile_timestamp: String,
    pub easytier_version: String,
    pub yggdrasil_port: u16,
    pub target_tuple: String,
    pub target_os: String,
}

#[tauri::command]
pub async fn terracotta_get_meta() -> Result<TerracottaMetaResponse> {
    let meta = theseus::terracotta::get_meta()
        .await
        .map_err(theseus::Error::from)?;
    Ok(TerracottaMetaResponse {
        version: meta.version,
        compile_timestamp: meta.compile_timestamp,
        easytier_version: meta.easytier_version,
        yggdrasil_port: meta.yggdrasil_port,
        target_tuple: meta.target_tuple,
        target_os: meta.target_os,
    })
}

#[tauri::command]
pub async fn terracotta_start(
    binary_path: Option<String>,
    auto_download: Option<bool>,
) -> Result<()> {
    theseus::multiplayer::prepare_terracotta_with_options(
        binary_path,
        auto_download.unwrap_or(true),
    )
    .await
    .map_err(|error| {
        tracing::error!(target: "theseus::terracotta", action = "start", error = %error);
        theseus::Error::from(error)
    })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_stop() -> Result<()> {
    theseus::multiplayer::stop_terracotta_compat()
        .await
        .map_err(|error| {
            tracing::error!(target: "theseus::terracotta", action = "stop", error = %error);
            theseus::Error::from(error)
        })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_host(
    room_code: Option<String>,
    player_name: String,
) -> Result<()> {
    theseus::multiplayer::host(
        theseus::multiplayer::MultiplayerHostRequest::Terracotta {
            room_code,
            player_name,
        },
    )
    .await
    .map_err(|error| {
        tracing::error!(target: "theseus::terracotta", action = "host", error = %error);
        theseus::Error::from(error)
    })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_join(
    room_code: String,
    player_name: String,
) -> Result<()> {
    theseus::multiplayer::join(theseus::multiplayer::MultiplayerJoinRequest {
        provider: theseus::multiplayer::MultiplayerProvider::Terracotta,
        room_code,
        player_name,
    })
    .await
    .map_err(|error| {
        tracing::error!(target: "theseus::terracotta", action = "join", error = %error);
        theseus::Error::from(error)
    })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_reset() -> Result<()> {
    theseus::multiplayer::reset_terracotta_compat()
        .await
        .map_err(|error| {
            tracing::error!(target: "theseus::terracotta", action = "reset", error = %error);
            theseus::Error::from(error)
        })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_get_platform_key() -> Result<String> {
    Ok(theseus::terracotta::terracotta_platform_key().to_string())
}

#[tauri::command]
pub async fn terracotta_check_for_update()
-> Result<theseus::terracotta::TerracottaUpdate> {
    Ok(theseus::terracotta::check_for_update()
        .await
        .map_err(theseus::Error::from)?)
}

#[tauri::command]
pub async fn terracotta_download(version: Option<String>) -> Result<()> {
    theseus::terracotta::download_terracotta(version)
        .await
        .map_err(|error| {
            tracing::error!(target: "theseus::terracotta", action = "download", error = %error);
            theseus::Error::from(error)
        })?;
    Ok(())
}

#[tauri::command]
pub async fn terracotta_update() -> Result<theseus::terracotta::TerracottaUpdate>
{
    Ok(theseus::terracotta::update_terracotta()
        .await
        .map_err(|error| {
            tracing::error!(target: "theseus::terracotta", action = "update", error = %error);
            theseus::Error::from(error)
        })?)
}

#[tauri::command]
pub async fn terracotta_get_player_name() -> Result<String> {
    let name = theseus::terracotta::get_player_name().await;
    Ok(name)
}

#[tauri::command]
pub async fn terracotta_get_diagnostic_report() -> Result<String> {
    Ok(theseus::terracotta::get_diagnostic_report()
        .await
        .map_err(|error| {
            tracing::error!(target: "theseus::terracotta", action = "diagnostic_report", error = %error);
            theseus::Error::from(error)
        })?)
}
