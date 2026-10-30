//! Theseus settings management interface

use crate::state::store::{Handle, atom};
pub use crate::{
    State,
    state::{
        DownloadSourceMode, Hooks, MemorySettings, PrivacySettings, Settings,
        WindowSize,
    },
};

pub(crate) static SETTINGS_WRITE: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

fn merge_settings(
    current: &mut serde_json::Value,
    patch: serde_json::Value,
) -> crate::Result<()> {
    if let (Some(target), Some(changes)) =
        (current.as_object_mut(), patch.as_object())
    {
        for (key, value) in changes {
            let existing = target.get_mut(key).ok_or_else(|| {
                crate::ErrorKind::InputError(format!("Unknown setting {key}"))
            })?;
            if matches!(key.as_str(), "feature_flags" | "memory" | "hooks")
                && existing.is_object()
                && value.is_object()
            {
                existing
                    .as_object_mut()
                    .unwrap()
                    .extend(value.as_object().unwrap().clone());
            } else {
                *existing = value.clone();
            }
        }
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(
            "Settings patch must be an object".to_string(),
        )
        .into())
    }
}

fn valid_game_language(value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || (value.len() <= 32
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
            }))
}

pub(crate) async fn patch_with(
    handle: &Handle,
    patch: serde_json::Value,
) -> crate::Result<Settings> {
    let mut unit = atom(handle).await?;
    let current = Settings::load_in(&mut unit).await?;
    let mut value = serde_json::to_value(&current)?;
    merge_settings(&mut value, patch)?;
    let mut settings: Settings = serde_json::from_value(value)?;
    if !valid_game_language(&settings.default_game_language) {
        return Err(crate::ErrorKind::InputError(
            "Default game language must be a Minecraft locale code".to_string(),
        )
        .into());
    }
    settings.telemetry = current.telemetry;
    settings.telemetry_consent_version = current.telemetry_consent_version;
    settings.discord_rpc = current.discord_rpc;
    super::terracotta::validate_public_nodes(
        &settings.terracotta_public_nodes,
    )?;
    crate::util::download_dns::DownloadDnsResolver::with_doh(
        settings.doh_enabled,
    )?;
    settings.save_in(&mut unit).await?;
    let saved = Settings::load_in(&mut unit).await?;
    unit.commit().await?;
    Ok(saved)
}

/// Merges field-level changes while serializing settings writes.
pub async fn patch(patch: serde_json::Value) -> crate::Result<Settings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let settings = patch_with(&state.data, patch).await?;
    state.update_http_client_for_settings(&settings).await?;
    state.update_download_settings(&settings);
    Ok(settings)
}

pub(crate) async fn apply_mojang_auth_source_with(
    handle: &Handle,
    expected_source: DownloadSourceMode,
    use_mirror: bool,
) -> crate::Result<bool> {
    let mut unit = atom(handle).await?;
    let mut settings = Settings::load_in(&mut unit).await?;
    if settings.mojang_auth_source != expected_source {
        return Ok(false);
    }
    // Explicit choices cannot be overridden by a delayed automatic probe.
    let use_mirror = match expected_source {
        DownloadSourceMode::MirrorPreferred => true,
        DownloadSourceMode::OfficialOnly => false,
        DownloadSourceMode::Auto | DownloadSourceMode::OfficialPreferred => {
            use_mirror
        }
    };
    let args = crate::util::mojang::auth_proxy_java_args(
        &settings.extra_launch_args,
        use_mirror,
    );
    if args != settings.extra_launch_args {
        settings.extra_launch_args = args;
        settings.save_in(&mut unit).await?;
    }
    unit.commit().await?;
    Ok(true)
}

/// Applies a probe result only while its source mode is still selected, preserving concurrent edits.
pub(crate) async fn apply_mojang_auth_source(
    expected_source: DownloadSourceMode,
    use_mirror: bool,
) -> crate::Result<bool> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let applied =
        apply_mojang_auth_source_with(&state.data, expected_source, use_mirror)
            .await?;
    if applied {
        state.set_mojang_auth_use_mirror(match expected_source {
            DownloadSourceMode::MirrorPreferred => true,
            DownloadSourceMode::OfficialOnly => false,
            DownloadSourceMode::Auto
            | DownloadSourceMode::OfficialPreferred => use_mirror,
        });
    }
    Ok(applied)
}
/// Gets entire settings
#[tracing::instrument]
pub async fn get() -> crate::Result<Settings> {
    let state = State::get().await?;
    let settings = Settings::get(&state.data).await?;
    Ok(settings)
}

#[tracing::instrument]
pub async fn get_privacy() -> crate::Result<PrivacySettings> {
    let state = State::get().await?;
    let settings = Settings::get(&state.data).await?;
    Ok(PrivacySettings {
        telemetry: settings.telemetry,
        discord_rpc: settings.discord_rpc,
        consent_version: settings.telemetry_consent_version,
    })
}

#[tracing::instrument]
pub async fn set_privacy(
    privacy: PrivacySettings,
) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let mut unit = atom(&state.data).await?;
    let mut settings = Settings::load_in(&mut unit).await?;
    settings.telemetry = privacy.telemetry;
    settings.discord_rpc = privacy.discord_rpc;
    settings.telemetry_consent_version = privacy.consent_version;
    settings.save_in(&mut unit).await?;
    unit.commit().await?;

    if let Err(error) =
        crate::telemetry::set_enabled(&state, privacy.telemetry).await
    {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply telemetry state");
    }
    if let Err(error) = state.discord_rpc.clear_to_default(true).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply Discord RPC state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn set_telemetry(enabled: bool) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let mut unit = atom(&state.data).await?;
    let mut settings = Settings::load_in(&mut unit).await?;
    settings.telemetry = enabled;
    settings.save_in(&mut unit).await?;
    unit.commit().await?;
    if let Err(error) = crate::telemetry::set_enabled(&state, enabled).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply telemetry state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn set_discord_rpc(enabled: bool) -> crate::Result<PrivacySettings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let mut unit = atom(&state.data).await?;
    let mut settings = Settings::load_in(&mut unit).await?;
    settings.discord_rpc = enabled;
    settings.save_in(&mut unit).await?;
    unit.commit().await?;
    if let Err(error) = state.discord_rpc.clear_to_default(true).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply Discord RPC state");
    }
    get_privacy().await
}

#[tracing::instrument]
pub async fn cancel_directory_change(
    app_identifier: &str,
) -> crate::Result<()> {
    // This is called to handle state initialization errors due to folder migrations
    // failing, so fetching a DB connection pool from `State::get` is not reliable here
    let pool = crate::state::db::connect(app_identifier).await?;
    let handle = Handle::new(pool);
    let mut settings = Settings::get(&handle).await?;

    if let Some(prev_custom_dir) = settings.prev_custom_dir {
        settings.prev_custom_dir = None;
        settings.custom_dir = Some(prev_custom_dir);
    }

    settings.update(&handle).await?;

    Ok(())
}
