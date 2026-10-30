//! Imports saved `options.txt` settings into the canonical values.

use super::CATALOG_REVISION;
use super::api_types::canonical_values_equal;
use super::catalog::*;
use super::fullscreen::update_app_fullscreen_setting;
use super::launch_overrides::currently_launcher_owned_keys;
use super::options_file::{GameOptionsDocument, validate_raw_key_value};
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::synced_game_option_preferences::{
    GameOptionPreferenceRecord, GameOptionPreferenceSource,
    SyncedGameOptionPreferences,
};
use crate::state::store::synced_game_option_state::{
    Singleton, SyncedGameOptionState,
};
use crate::state::store::synced_game_option_values::{
    GameOptionValueKind, GameOptionValueOrigin, GameOptionValueRecord,
    SyncedGameOptionValues,
};
use crate::state::{
    CanonicalValue, GameOptionKind, InstanceMetadata, State,
    load_game_option_preferences, load_game_options_sync_state,
    load_shared_game_options,
};
use chrono::Utc;
use std::collections::HashSet;

pub(super) fn custom_setting_id(key: &str) -> String {
    format!("external:{key}")
}

/// Imports every enabled setting present in `options.txt`. The caller skips file
/// hashes produced by app writes.
pub(super) async fn read_instance_changes_into_shared_settings(
    metadata: &InstanceMetadata,
    document: &GameOptionsDocument,
    state: &State,
) -> crate::Result<bool> {
    let values = load_shared_game_options(&state.data).await?;
    let preferences = load_game_option_preferences(&state.data).await?;
    let launcher_keys = currently_launcher_owned_keys(metadata);
    let now = Utc::now().timestamp();
    let mut updates = Vec::new();
    for (option_id, stored) in &values {
        if !preferences
            .get(option_id)
            .is_some_and(|preference| preference.enabled)
        {
            continue;
        }
        if matches!(stored.kind, GameOptionKind::External)
            && stored.raw_key.is_none()
        {
            continue;
        }
        if stored
            .raw_key
            .as_deref()
            .is_some_and(|key| setting_by_file_key(key).is_some())
        {
            continue;
        }
        let definition = setting_by_id(option_id);
        if definition.is_some()
            && !supported_settings_cover_game_version(
                &metadata.applied_content_set.game_version,
            )
        {
            continue;
        }
        let current_key = if let Some(definition) = definition {
            observed_physical_key(
                definition,
                document,
                &metadata.applied_content_set.game_version,
            )
        } else {
            stored.raw_key.clone()
        };
        let Some(current_key) = current_key else {
            continue;
        };
        if launcher_keys.contains(&current_key) {
            continue;
        }
        let Some(current_raw) = document.value(&current_key) else {
            continue;
        };
        let candidate = if let Some(definition) = definition {
            decode_value_for_version(
                definition,
                &current_key,
                current_raw,
                &metadata.applied_content_set.game_version,
            )
        } else if matches!(stored.kind, GameOptionKind::External) {
            if validate_raw_key_value(&current_key, current_raw).is_err() {
                continue;
            }
            Some(CanonicalValue::ExternalRaw(current_raw.to_string()))
        } else {
            None
        };
        let Some(candidate) = candidate else {
            continue;
        };
        if stored
            .value
            .as_ref()
            .is_some_and(|current| canonical_values_equal(current, &candidate))
        {
            continue;
        }
        updates.push((option_id.to_string(), candidate, stored.revision));
    }

    let mut changed = false;
    let mut fullscreen_value = None;
    if !updates.is_empty() {
        let (canonical_revision, _) =
            load_game_options_sync_state(&state.data, CATALOG_REVISION).await?;
        let mut unit = atom(&state.data).await?;
        for (option_id, candidate, revision) in updates {
            let Some(mut stored) =
                SyncedGameOptionValues::atom_get(&mut unit, &option_id).await?
            else {
                continue;
            };
            if stored.revision != revision as i64 {
                continue;
            }
            stored.canonical_type = candidate.type_name().to_string();
            stored.canonical_value = Some(serde_json::to_string(&candidate)?);
            stored.seeded = true;
            stored.revision = revision.saturating_add(1) as i64;
            stored.origin = GameOptionValueOrigin::Instance;
            stored.source_game_version =
                Some(metadata.applied_content_set.game_version.clone());
            stored.source_instance_id = Some(metadata.instance.id.clone());
            stored.updated_at = now;
            SyncedGameOptionValues::atom_set(
                &mut unit,
                &option_id,
                Some(&stored),
            )
            .await?;
            if option_id == "fullscreen" {
                fullscreen_value = Some(candidate.clone());
            }
            changed = true;
        }
        if changed {
            let next_revision = canonical_revision.saturating_add(1) as i64;
            if let Some(mut stored) =
                SyncedGameOptionState::atom_get(&mut unit, &Singleton).await?
            {
                stored.revision = next_revision;
                stored.catalog_revision = CATALOG_REVISION as i64;
                SyncedGameOptionState::atom_set(
                    &mut unit,
                    &Singleton,
                    Some(&stored),
                )
                .await?;
            }
        }
        if let Some(value) = fullscreen_value.as_ref() {
            update_app_fullscreen_setting(&mut unit, value, true).await?;
        }
        unit.commit().await?;
    }
    let discovered =
        discover_custom_settings(metadata, document, &launcher_keys, state)
            .await?;
    Ok(discovered || changed)
}

/// Finds settings added by mods and adds them to the shared settings list.
pub(super) async fn discover_custom_settings(
    metadata: &InstanceMetadata,
    document: &GameOptionsDocument,
    excluded_keys: &HashSet<String>,
    state: &State,
) -> crate::Result<bool> {
    let existing = load_shared_game_options(&state.data).await?;
    let preferences = load_game_option_preferences(&state.data).await?;
    let (canonical_revision, _) =
        load_game_options_sync_state(&state.data, CATALOG_REVISION).await?;
    let entries = document.effective_entries();
    let now = Utc::now().timestamp();
    let mut discovered = false;
    let locale_snapshot =
        if super::locales::has_localizable_mod_settings(document)
            && document.effective_entries().keys().any(|key| {
                let id = setting_by_file_key(key)
                    .map(|s| s.id.to_owned())
                    .unwrap_or_else(|| custom_setting_id(key));
                !existing.contains_key(&id)
            })
        {
            super::locales::capture_observation(metadata, state).await
        } else {
            None
        };
    let mut unit = atom(&state.data).await?;
    let game_version = metadata.applied_content_set.game_version.as_str();

    for definition in all_supported_settings() {
        for key in definition.keys {
            let custom_id = custom_setting_id(key);
            let Some(custom) = existing.get(&custom_id) else {
                continue;
            };
            if !custom.seeded {
                continue;
            }
            let revision = custom.revision.saturating_add(1) as i64;
            if let Some(mut stored) =
                SyncedGameOptionValues::atom_get(&mut unit, &custom_id).await?
            {
                stored.seeded = false;
                stored.revision = revision;
                stored.updated_at = now;
                SyncedGameOptionValues::atom_set(
                    &mut unit,
                    &custom_id,
                    Some(&stored),
                )
                .await?;
            }
            if let Some(mut stored) =
                SyncedGameOptionPreferences::atom_get(&mut unit, &custom_id)
                    .await?
            {
                stored.enabled = false;
                stored.source = GameOptionPreferenceSource::CatalogDefault;
                stored.revision = revision;
                SyncedGameOptionPreferences::atom_set(
                    &mut unit,
                    &custom_id,
                    Some(&stored),
                )
                .await?;
            }
            discovered = true;
        }
    }

    if supported_settings_cover_game_version(game_version) {
        for definition in all_supported_settings() {
            if existing.contains_key(definition.id) {
                continue;
            }
            let discovered_value = definition.keys.iter().find_map(|key| {
                if excluded_keys.contains(*key) {
                    return None;
                }
                let raw = document.value(key)?;
                decode_value_for_version(definition, key, raw, game_version)
            });
            let Some(value) = discovered_value else {
                continue;
            };
            let option_id = definition.id.to_string();
            SyncedGameOptionValues::atom_set(
                &mut unit,
                &option_id,
                Some(&GameOptionValueRecord {
                    kind: GameOptionValueKind::Vanilla,
                    raw_key: None,
                    canonical_type: value.type_name().to_string(),
                    canonical_value: Some(serde_json::to_string(&value)?),
                    value_codec: "catalog".to_string(),
                    seeded: true,
                    revision: 1,
                    origin: GameOptionValueOrigin::Instance,
                    source_game_version: Some(game_version.to_string()),
                    source_instance_id: Some(metadata.instance.id.clone()),
                    updated_at: now,
                }),
            )
            .await?;
            if SyncedGameOptionPreferences::atom_get(&mut unit, &option_id)
                .await?
                .is_none()
            {
                SyncedGameOptionPreferences::atom_set(
                    &mut unit,
                    &option_id,
                    Some(&GameOptionPreferenceRecord {
                        enabled: definition.default_on,
                        source: GameOptionPreferenceSource::CatalogDefault,
                        revision: 1,
                    }),
                )
                .await?;
            }
            discovered = true;
        }
    }

    for (key, (index, _)) in entries {
        if excluded_keys.contains(key)
            || setting_by_file_key(key).is_some()
            || is_never_sync_key(key)
        {
            continue;
        }
        let Some(entry) = &document.lines[index].entry else {
            continue;
        };
        if validate_raw_key_value(key, &entry.value).is_err() {
            continue;
        }
        let option_id = custom_setting_id(key);
        let should_seed =
            existing.get(&option_id).is_none_or(|stored| !stored.seeded);
        if !should_seed {
            continue;
        }
        let revision = existing
            .get(&option_id)
            .map(|stored| stored.revision)
            .unwrap_or(0)
            .max(
                preferences
                    .get(&option_id)
                    .map(|preference| preference.revision)
                    .unwrap_or(0),
            )
            .saturating_add(1) as i64;
        let value = CanonicalValue::ExternalRaw(entry.value.clone());
        // The conflict update left kind, canonical type and codec alone.
        let stored =
            SyncedGameOptionValues::atom_get(&mut unit, &option_id).await?;
        let (kind, canonical_type, value_codec) = match &stored {
            Some(stored) => (
                stored.kind,
                stored.canonical_type.clone(),
                stored.value_codec.clone(),
            ),
            None => (
                GameOptionValueKind::External,
                "external_raw".to_string(),
                "exact_raw".to_string(),
            ),
        };
        SyncedGameOptionValues::atom_set(
            &mut unit,
            &option_id,
            Some(&GameOptionValueRecord {
                kind,
                raw_key: Some(key.to_string()),
                canonical_type,
                canonical_value: Some(serde_json::to_string(&value)?),
                value_codec,
                seeded: true,
                revision,
                origin: GameOptionValueOrigin::Instance,
                source_game_version: Some(game_version.to_string()),
                source_instance_id: Some(metadata.instance.id.clone()),
                updated_at: now,
            }),
        )
        .await?;
        if SyncedGameOptionPreferences::atom_get(&mut unit, &option_id)
            .await?
            .is_none()
        {
            SyncedGameOptionPreferences::atom_set(
                &mut unit,
                &option_id,
                Some(&GameOptionPreferenceRecord {
                    enabled: false,
                    source: GameOptionPreferenceSource::DiscoveryDefault,
                    revision,
                }),
            )
            .await?;
        }
        discovered = true;
    }

    if discovered {
        let next_revision = canonical_revision.saturating_add(1) as i64;
        if let Some(mut stored) =
            SyncedGameOptionState::atom_get(&mut unit, &Singleton).await?
        {
            stored.revision = next_revision;
            stored.catalog_revision = CATALOG_REVISION as i64;
            SyncedGameOptionState::atom_set(
                &mut unit,
                &Singleton,
                Some(&stored),
            )
            .await?;
        }
    }
    unit.commit().await?;
    super::locales::record_observations(
        &state.data,
        metadata,
        document,
        locale_snapshot.as_deref(),
    )
    .await;
    if discovered {
        super::locales::queue_game_locale_index();
    }
    Ok(discovered)
}
