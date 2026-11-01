//! Theseus settings management interface

use std::collections::BTreeSet;

use serde_json::{Map, Value};

pub use crate::state::documents::settings::SettingLayer;
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
    // The keys the patch names are the change it carries, and the only ones its
    // commit states a settings document from: a domain beside them keeps what
    // the user left there, which restating the row would undo.
    let changed: BTreeSet<String> = patch
        .as_object()
        .map(|changes| changes.keys().cloned().collect())
        .unwrap_or_default();
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
    unit.settings_changed(changed);
    settings.save_in(&mut unit).await?;
    let saved = Settings::load_in(&mut unit).await?;
    // The domains whose writes have moved to their documents are stated by this
    // commit, once the row has landed, and only the ones carrying a key the
    // patch named.
    unit.commit().await?;
    Ok(saved)
}

/// Merges field-level changes while serializing settings writes.
pub async fn patch(patch: serde_json::Value) -> crate::Result<Settings> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let settings = patch_with(&state.data, patch).await?;
    // A persisted opt-out must take effect even if rebuilding HTTP fails.
    crate::util::download::telemetry::set_enabled(
        settings.advanced_download_info_enabled(),
    );
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

/// The settings as one JSON object, keyed the way a settings document keys
/// them, with the values normalised the way a read answers them.
///
/// `version` is left out: it is the row's alone, because a file able to answer
/// it could skip the settings migrations. `personalized_ads` never serializes —
/// it is a column of the row — and it is removed here as well so the promise
/// holds even if that changes.
pub fn effective_json(settings: &Settings) -> Value {
    let Ok(Value::Object(mut values)) =
        serde_json::to_value(settings.clone().normalize())
    else {
        // The settings are serialized for every read already, so this is not a
        // state a caller can bring about; an empty object beats a wrong one.
        tracing::warn!(
            "Could not read the settings as JSON, so the export is empty"
        );
        return Value::Object(Map::new());
    };
    values.remove("version");
    values.remove("personalized_ads");
    Value::Object(values)
}

/// The keys that differ from the build's defaults, per domain, in the shape a
/// deployment's own document takes: what can be written back so a launch reads
/// these settings.
///
/// Only the domains `DOMAINS` lists are walked, so the two keys no document
/// carries are out by construction rather than by a rule here: `version` would
/// let a file choose the migrations, and `personalized_ads` is the row's.
/// A key is kept when it differs from the default *as a setting*, which is what
/// `same_setting` decides: a document saying `0` where the range starts at `1`
/// reads as `1` and is therefore the default, and writing it back would pin a
/// value the user never chose against a later change of that default.
pub fn patch_json(settings: &Settings) -> Value {
    let defaults = match crate::state::documents::settings::default_values() {
        Ok(defaults) => defaults,
        Err(error) => {
            tracing::warn!(
                %error,
                "Could not read the settings defaults, so the export is empty"
            );
            return Value::Object(Map::new());
        }
    };
    let Ok(Value::Object(values)) =
        serde_json::to_value(settings.clone().normalize())
    else {
        tracing::warn!(
            "Could not read the settings as JSON, so the export is empty"
        );
        return Value::Object(Map::new());
    };
    let mut domains = Map::new();
    for &(name, keys, _) in crate::state::documents::settings::DOMAINS {
        let mut domain = Map::new();
        for &key in keys {
            let Some(value) = values.get(key) else {
                continue;
            };
            let differs = match defaults.get(key) {
                Some(default) => {
                    !crate::state::documents::settings::same_setting(
                        key, default, value, &defaults,
                    )
                }
                // A key the build has no default for is one the defaults cannot
                // vouch for, so its value is written rather than dropped.
                None => true,
            };
            if differs {
                domain.insert(key.to_string(), value.clone());
            }
        }
        // An empty domain would ask the deployment to state a document that
        // says nothing, so it is left out instead.
        if !domain.is_empty() {
            domains.insert(name.to_string(), Value::Object(domain));
        }
    }
    Value::Object(domains)
}

/// Every key of the settings with what each layer says about it.
pub async fn layers() -> crate::Result<Vec<SettingLayer>> {
    let state = State::get().await?;
    crate::state::documents::settings::layers(&state.data).await
}

/// Drops a key from the local settings document, so the deployment's answer
/// takes over again.
///
/// `expected` is the local value the caller saw: a value that no longer matches
/// is one the user changed since, which is theirs to keep. Everything else the
/// runtime derives from the settings is refreshed here, the same way `patch`
/// does after a write, because the effective value changed without a row write.
pub async fn clear_override(
    key: &str,
    expected: &Value,
) -> crate::Result<bool> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let cleared = crate::state::documents::settings::clear_local_key(
        &state.data,
        key,
        expected,
    )
    .await?;
    if !cleared {
        return Ok(false);
    }
    let settings = Settings::get(&state.data).await?;
    crate::util::download::telemetry::set_enabled(
        settings.advanced_download_info_enabled(),
    );
    state.update_http_client_for_settings(&settings).await?;
    state.update_download_settings(&settings);
    Ok(true)
}

/// Rewrites every local settings document in the shape this build knows, and
/// reports how many domains were written.
///
/// This is the explicit way out of the read protection a downgrade leaves: a
/// document a newer build wrote is read as it stands and never written back, so
/// without this the user cannot bring one back to a shape this build may lower.
/// What a rewrite cannot keep is in the documents' own log.
///
/// The effective values change without a row write, so everything the runtime
/// derives from them is refreshed here the way `patch` refreshes it.
pub async fn sanitise() -> crate::Result<usize> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let written =
        crate::state::documents::settings::sanitise(&state.data).await?;
    let settings = Settings::get(&state.data).await?;
    crate::util::download::telemetry::set_enabled(
        settings.advanced_download_info_enabled(),
    );
    state.update_http_client_for_settings(&settings).await?;
    state.update_download_settings(&settings);
    Ok(written)
}

/// Merges an exported settings document into the local documents, and reports
/// how many domains were written.
///
/// The description of what a merge does is the document writer's; what is here
/// is the same refresh `sanitise` makes, for the same reason.
pub async fn import(input: Value) -> crate::Result<usize> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let written =
        crate::state::documents::settings::import(&state.data, input).await?;
    let settings = Settings::get(&state.data).await?;
    crate::util::download::telemetry::set_enabled(
        settings.advanced_download_info_enabled(),
    );
    state.update_http_client_for_settings(&settings).await?;
    state.update_download_settings(&settings);
    Ok(written)
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
