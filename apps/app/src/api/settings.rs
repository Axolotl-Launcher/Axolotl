use crate::api::Result;
use tauri::{Emitter, Runtime};
use theseus::prelude::*;
use theseus::{ProxyConfig, ProxyTestResult};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("settings")
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_patch,
            privacy_get,
            privacy_set,
            telemetry_set,
            discord_rpc_set,
            cancel_directory_change,
            proxy_get,
            proxy_set,
            proxy_test,
            network_test,
            network_test_cancel
        ])
        .build()
}

pub(crate) fn log_level_for_channel(channel: &str, log_level: &str) -> String {
    if channel == "beta" && !matches!(log_level, "debug" | "trace") {
        "debug".to_string()
    } else {
        log_level.to_string()
    }
}

static NETWORK_TEST_SLOTS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(2);

#[tauri::command]
pub async fn network_test(
    request_id: uuid::Uuid,
    target: theseus::NetworkTestTarget,
    mode: theseus::NetworkTestMode,
) -> Result<Option<theseus::NetworkTestResult>> {
    let cancellation = super::search_cancellation::SearchCancellation::register(
        format!("network-test:{request_id}"),
    );
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Ok(None),
        result = async {
            let _permit = NETWORK_TEST_SLOTS.acquire().await.map_err(|_| {
                std::io::Error::other("Network diagnostics are unavailable")
            })?;
            Ok(Some(theseus::test_network_connection(target, mode).await?))
        } => result,
    }
}

#[tauri::command]
pub fn network_test_cancel(request_id: uuid::Uuid) {
    if let Some(pending) = super::search_cancellation::cancel(&format!(
        "network-test:{request_id}"
    )) {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            pending.expire();
        });
    }
}

// Get full settings
// invoke('plugin:settings|settings_get')
#[tauri::command]
pub async fn settings_get() -> Result<Settings> {
    let res = settings::get().await?;
    Ok(res)
}

/// Merges only the edited fields into current settings.
#[tauri::command]
pub async fn settings_patch(
    app: tauri::AppHandle<impl Runtime>,
    mut patch: serde_json::Value,
) -> Result<Settings> {
    let channel = crate::resolve_update_channel(&app).await?;
    if let Some(log_level) =
        patch.get("log_level").and_then(|value| value.as_str())
    {
        let log_level = log_level_for_channel(&channel, log_level);
        patch["log_level"] = serde_json::Value::String(log_level);
    }
    let settings = settings::patch(patch).await?;
    let log_level = settings.log_level.clone();
    // Apply the log level right away so the new verbosity takes effect without
    // a restart. Invalid values are rejected by the settings table itself.
    if let Err(error) = theseus::set_log_level(&log_level) {
        tracing::warn!("Keeping the previous log level: {error}");
    }
    let _ = app.emit("settings", ());
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::log_level_for_channel;

    #[test]
    fn beta_only_allows_verbose_log_levels() {
        assert_eq!(log_level_for_channel("beta", "info"), "debug");
        assert_eq!(log_level_for_channel("beta", "debug"), "debug");
        assert_eq!(log_level_for_channel("beta", "trace"), "trace");
    }

    #[test]
    fn release_preserves_the_selected_log_level() {
        assert_eq!(log_level_for_channel("release", "error"), "error");
        assert_eq!(log_level_for_channel("release", "info"), "info");
        assert_eq!(log_level_for_channel("release", "trace"), "trace");
    }
}

#[tauri::command]
pub async fn privacy_get() -> Result<PrivacySettings> {
    Ok(settings::get_privacy().await?)
}

#[tauri::command]
pub async fn privacy_set(privacy: PrivacySettings) -> Result<PrivacySettings> {
    Ok(settings::set_privacy(privacy).await?)
}

#[tauri::command]
pub async fn telemetry_set(enabled: bool) -> Result<PrivacySettings> {
    Ok(settings::set_telemetry(enabled).await?)
}

#[tauri::command]
pub async fn discord_rpc_set(enabled: bool) -> Result<PrivacySettings> {
    Ok(settings::set_discord_rpc(enabled).await?)
}

#[tauri::command]
pub async fn cancel_directory_change<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let identifier = &app.config().identifier;
    settings::cancel_directory_change(identifier).await?;
    Ok(())
}

#[tauri::command]
pub async fn proxy_get() -> Result<ProxyConfig> {
    let state = State::get().await?;
    Ok(state.proxy_config().await?)
}

#[tauri::command]
pub async fn proxy_set(
    app: tauri::AppHandle<impl Runtime>,
    config: ProxyConfig,
) -> Result<()> {
    let state = State::get().await?;
    config.validate()?;
    state.update_proxy_config(&config).await?;
    let _ = app.emit("settings", ());
    Ok(())
}

#[tauri::command]
pub async fn proxy_test(config: ProxyConfig) -> Result<ProxyTestResult> {
    Ok(theseus::test_proxy_connection(&config).await?)
}
