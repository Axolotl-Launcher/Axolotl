use crate::api::Result;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("multiplayer")
        .invoke_handler(tauri::generate_handler![
            multiplayer_get_state,
            multiplayer_get_nodes,
            multiplayer_get_detected_ports,
            multiplayer_download_hongshi,
            multiplayer_switch_provider,
            multiplayer_prepare_terracotta,
            multiplayer_host,
            multiplayer_join,
            multiplayer_stop,
            multiplayer_reset,
            multiplayer_get_player_name,
            multiplayer_open_hongshi_logs,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_download_hongshi() -> Result<()> {
    Ok(theseus::hongshi::download()
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_get_state()
-> Result<theseus::multiplayer::MultiplayerState> {
    Ok(theseus::multiplayer::get_state().await)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_get_nodes(
    force_refresh: Option<bool>,
) -> Result<Vec<theseus::hongshi::HongshiNode>> {
    Ok(theseus::hongshi::get_nodes(force_refresh.unwrap_or(false))
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_get_detected_ports()
-> Result<Vec<theseus::hongshi::DetectedLanPort>> {
    Ok(theseus::hongshi::get_detected_ports().await)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_switch_provider(
    provider: theseus::multiplayer::MultiplayerProvider,
) -> Result<()> {
    Ok(theseus::multiplayer::switch_provider(provider)
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_prepare_terracotta() -> Result<()> {
    Ok(theseus::multiplayer::prepare_terracotta()
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_host(
    request: theseus::multiplayer::MultiplayerHostRequest,
) -> Result<()> {
    Ok(theseus::multiplayer::host(request)
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_join(
    request: theseus::multiplayer::MultiplayerJoinRequest,
) -> Result<()> {
    Ok(theseus::multiplayer::join(request)
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_stop() -> Result<()> {
    Ok(theseus::multiplayer::stop()
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_reset() -> Result<()> {
    Ok(theseus::multiplayer::reset()
        .await
        .map_err(theseus::Error::from)?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_get_player_name() -> Result<String> {
    Ok(theseus::terracotta::get_player_name().await)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn multiplayer_open_hongshi_logs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    tokio::fs::create_dir_all(theseus::hongshi::logs_dir()).await?;
    crate::api::utils::open_path(app, theseus::hongshi::logs_dir()).await;
    Ok(())
}
