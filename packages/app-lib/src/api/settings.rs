//! Theseus settings management interface

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::state::documents::settings::{DANGER_ZONE, DOMAINS};
pub use crate::state::documents::settings::{DangerousImport, SettingLayer};
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

/// Whether `value` is a Minecraft locale code. Called by `patch_with` on a write
/// and by an import on the value it is about to take, so that a file cannot
/// leave settings no write may replace.
pub(crate) fn valid_game_language(value: &str) -> bool {
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

/// Which keys an export writes.
#[derive(Debug, Clone, Copy)]
pub enum ExportKind {
    /// Every domain, carrying the value every one of its known keys reads as.
    Effective,
    /// Only the keys that differ from the build's defaults, and only the
    /// domains that carry one of them.
    Patch,
}

/// The documents an export writes, named by the domain each one belongs to and
/// holding nothing but that domain's keys.
///
/// A document is the shape the launcher's own files take, `schema_version`
/// included, so an archive of them can be unzipped into the settings directory
/// and read as the configuration it describes: the version it states is the one
/// this build writes that domain in, which is what a file has to record for the
/// next read to interpret it. An `Effective` export states every key, since a
/// value equal to the default is what the export is for; a `Patch` export
/// leaves a domain out when it has nothing to write, the way `patch_json`
/// leaves it out of its answer.
pub fn export_documents(
    settings: &Settings,
    kind: ExportKind,
) -> Vec<(String, Value)> {
    match kind {
        ExportKind::Effective => {
            let Value::Object(values) = effective_json(settings) else {
                return Vec::new();
            };
            DOMAINS
                .iter()
                .map(|&(name, keys, schema_version)| {
                    let mut data = Map::new();
                    for &key in keys {
                        if let Some(value) = values.get(key) {
                            data.insert(key.to_string(), value.clone());
                        }
                    }
                    (name.to_string(), document(schema_version, data))
                })
                .collect()
        }
        ExportKind::Patch => {
            let Value::Object(values) = patch_json(settings) else {
                return Vec::new();
            };
            let mut documents = Vec::new();
            for &(name, _, schema_version) in DOMAINS {
                let Some(Value::Object(data)) = values.get(name) else {
                    continue;
                };
                documents.push((
                    name.to_string(),
                    document(schema_version, data.clone()),
                ));
            }
            documents
        }
    }
}

/// One settings document, in the shape the launcher's own files take.
fn document(schema_version: u32, data: Map<String, Value>) -> Value {
    let mut document = Map::new();
    document.insert("schema_version".to_string(), Value::from(schema_version));
    document.insert("data".to_string(), Value::Object(data));
    Value::Object(document)
}

/// What importing a file would find in it, so the user can be told before
/// anything of it is applied.
#[derive(Debug, Default, serde::Serialize)]
pub struct ImportScan {
    /// The domains the file carries a document for.
    pub domains: Vec<String>,
    /// The keys of `DANGER_ZONE` the file would actually set, with the value
    /// each one has now and the one the import would leave in its place.
    pub dangerous: Vec<DangerousImport>,
}

/// What an import wrote.
#[derive(Debug, Default, serde::Serialize)]
pub struct ImportResult {
    /// The local documents it replaced.
    pub documents: usize,
    /// The imported values it dropped because this build cannot read them as
    /// the settings they name.
    pub dropped: usize,
}

/// Reads what `input` would import: the known domains it carries a document
/// for, and the dangerous settings it actually sets, each with the value it
/// reads as now and the one the import would leave in its place.
///
/// A file a user was handed may carry names beside the domains; those are not
/// settings documents, so they are passed over rather than refused. A dangerous
/// key is reported only for the domain whose key list carries it, because one
/// written into the wrong document is read as no setting at all. A key belongs
/// to one domain, so the list needs no ordering of its own.
pub async fn import_scan(input: &Value) -> crate::Result<ImportScan> {
    let state = State::get().await?;
    let settings = Settings::get(&state.data).await?;
    Ok(scan(input, &effective_json(&settings)))
}

/// The scan itself, judged against `current`: the settings as a read answers
/// them, which is what every dangerous key reads as before the import.
fn scan(input: &Value, current: &Value) -> ImportScan {
    let mut scan = ImportScan::default();
    let Some(values) = input.as_object() else {
        return scan;
    };
    for &(name, keys, _) in DOMAINS {
        let Some(data) = values.get(name).and_then(Value::as_object) else {
            continue;
        };
        scan.domains.push(name.to_string());
        for &key in keys {
            if !DANGER_ZONE.contains(&key) {
                continue;
            }
            let Some(after) = data.get(key) else {
                continue;
            };
            scan.dangerous.push(DangerousImport {
                key: key.to_string(),
                before: current.get(key).cloned(),
                after: after.clone(),
            });
        }
    }
    scan
}

/// Everything the runtime derives from the settings, re-applied after a change
/// that did not go through the row.
///
/// A document operation can replace part of its documents and then answer an
/// error, so the effective values may differ from the ones the runtime took at
/// the last write. Each step that fails is reported in the log rather than
/// returned: the caller's own answer is about the write, and replacing it with
/// a failure of the refresh would hide what landed.
async fn refresh(state: &Arc<State>) {
    let settings = match Settings::get(&state.data).await {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(
                %error,
                "Skipping the settings refresh: the settings cannot be read"
            );
            return;
        }
    };
    crate::util::download::telemetry::set_enabled(
        settings.advanced_download_info_enabled(),
    );
    if let Err(error) = state.update_http_client_for_settings(&settings).await {
        tracing::warn!(
            %error,
            "Keeping the network clients built from the previous settings"
        );
    }
    state.update_download_settings(&settings);
    if let Err(error) = crate::set_log_level(&settings.log_level) {
        tracing::warn!(%error, "Keeping the previous log level");
    }
    if let Err(error) =
        crate::telemetry::set_enabled(state, settings.telemetry).await
    {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply telemetry state");
    }
    if let Err(error) = state.discord_rpc.clear_to_default(true).await {
        tracing::debug!(target: "theseus::telemetry", %error, "Failed to apply Discord RPC state");
    }
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
/// does after a write, because the effective value changed without a row write —
/// and it is refreshed whatever the clear answered, since a clear that failed
/// after the document was replaced left the same change behind.
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
    .await;
    refresh(&state).await;
    cleared
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
/// derives from them is refreshed here the way `patch` refreshes it, and the
/// rewrite's own failure is what the caller is told: the domains it did write
/// are in effect either way.
pub async fn sanitise() -> crate::Result<usize> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let written =
        crate::state::documents::settings::sanitise(&state.data).await;
    refresh(&state).await;
    written
}

/// Merges the documents of an exported archive into the local documents, and
/// reports what was written.
///
/// The description of what a merge does is the document writer's, and so is the
/// answer `allow_danger` gives about `DANGER_ZONE`; what is here is the refresh
/// the runtime needs after it. That refresh happens whatever the merge
/// answered, and the merge's own answer is what the caller is told: a merge that
/// wrote part of its domains and then failed still changed them.
pub async fn import(
    input: Value,
    allow_danger: bool,
) -> crate::Result<ImportResult> {
    let _guard = SETTINGS_WRITE.lock().await;
    let state = State::get().await?;
    let imported = crate::state::documents::settings::import(
        &state.data,
        input,
        allow_danger,
    )
    .await;
    refresh(&state).await;
    imported
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
