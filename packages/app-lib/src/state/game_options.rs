use super::{GameOptionKind, StoredOption, StoredPreference};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::sync_feature_settings::{
    FeatureSettingsKey, SyncFeatureSettings,
};
use crate::state::store::synced_game_option_preferences::SyncedGameOptionPreferences;
use crate::state::store::synced_game_option_state::{
    Singleton, SyncedGameOptionState,
};
use crate::state::store::synced_game_option_values::{
    GameOptionValueKind, SyncedGameOptionValues,
};
use std::collections::HashMap;

const GAME_OPTIONS_FEATURE: &str = "game_options";

pub(crate) async fn shared_game_options_exist(
    handle: &Handle,
) -> crate::Result<bool> {
    Ok(SyncedGameOptionState::get(handle, &Singleton)
        .await?
        .is_some())
}

pub(crate) async fn game_options_sync_is_enabled(
    handle: &Handle,
) -> crate::Result<bool> {
    let key = FeatureSettingsKey {
        feature: GAME_OPTIONS_FEATURE.to_string(),
    };
    Ok(SyncFeatureSettings::get(handle, &key)
        .await?
        .is_some_and(|record| record.globally_enabled))
}

pub(crate) async fn load_game_options_sync_state(
    handle: &Handle,
    current_catalog_revision: u32,
) -> crate::Result<(u64, u32)> {
    Ok(SyncedGameOptionState::get(handle, &Singleton)
        .await?
        .map(|record| {
            (
                record.revision.max(0) as u64,
                (record.catalog_revision.max(1) as u32)
                    .max(current_catalog_revision),
            )
        })
        .unwrap_or((0, current_catalog_revision)))
}

pub(crate) async fn load_shared_game_options(
    handle: &Handle,
) -> crate::Result<HashMap<String, StoredOption>> {
    let rows = SyncedGameOptionValues::get_all(handle).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for (option_id, record) in rows {
        let value = record
            .canonical_value
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        out.insert(
            option_id.clone(),
            StoredOption {
                option_id,
                kind: match record.kind {
                    GameOptionValueKind::Vanilla => GameOptionKind::Vanilla,
                    GameOptionValueKind::External => GameOptionKind::External,
                },
                raw_key: record.raw_key,
                value,
                seeded: record.seeded,
                revision: record.revision.max(0) as u64,
            },
        );
    }
    Ok(out)
}

pub(crate) async fn load_game_option_preferences(
    handle: &Handle,
) -> crate::Result<HashMap<String, StoredPreference>> {
    Ok(SyncedGameOptionPreferences::get_all(handle)
        .await?
        .into_iter()
        .map(|(option_id, record)| {
            (
                option_id,
                StoredPreference {
                    enabled: record.enabled,
                    revision: record.revision.max(0) as u64,
                },
            )
        })
        .collect())
}
