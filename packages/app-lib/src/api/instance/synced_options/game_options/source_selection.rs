//! Finds eligible source instances and copies the first shared settings from one.

use super::super as synced_options;
use super::CATALOG_REVISION;
use super::api_types::{GameOptionsSourceCandidate, GameOptionsSourceIssue};
use super::catalog::*;
use super::fullscreen::update_app_fullscreen_setting;
use super::launch_overrides::currently_launcher_owned_keys;
use super::options_file::{
    input_error, options_path, read_document, validate_raw_key_value,
};
use super::read_instance_changes::custom_setting_id;
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::instance_sync_preferences::{
    InstancePreferenceKey, InstancePreferenceRecord, InstanceSyncPreferences,
};
use crate::state::store::sync_feature_settings::{
    FeatureSettingsKey, FeatureSettingsRecord, SyncFeatureSettings,
};
use crate::state::store::synced_game_option_preferences::{
    GameOptionPreferenceRecord, GameOptionPreferenceSource,
    SyncedGameOptionPreferences,
};
use crate::state::store::synced_game_option_state::{
    GameOptionStateRecord, Singleton, SyncedGameOptionState,
};
use crate::state::store::synced_game_option_values::{
    GameOptionValueKind, GameOptionValueOrigin, GameOptionValueRecord,
    SyncedGameOptionValues,
};
use crate::state::{
    CanonicalValue, InstanceMetadata, State, load_game_option_preferences,
    load_game_options_sync_state, load_shared_game_options,
};
use chrono::Utc;

const SYNC_FEATURE: &str = "game_options";

/// Uses the selected instance's `options.txt` as the initial shared settings.
pub(in crate::api::instance) async fn initialize_from_source_instance(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<()> {
    let path = options_path(metadata, state)?;
    if !path.exists() {
        return Err(input_error(
            "Launch the source instance once so Minecraft can create options.txt.",
        ));
    }
    let (document, _) = read_document(&path).await?;
    let launcher_keys = currently_launcher_owned_keys(metadata);
    let entries = document.effective_entries();
    let current_values = load_shared_game_options(&state.data).await?;
    let current_preferences = load_game_option_preferences(&state.data).await?;
    let (current_revision, _) =
        load_game_options_sync_state(&state.data, CATALOG_REVISION).await?;
    let now = Utc::now().timestamp();
    let source_version = metadata.applied_content_set.game_version.as_str();
    let source_id = metadata.instance.id.as_str();
    let locale_snapshot =
        if super::locales::has_localizable_mod_settings(&document) {
            super::locales::capture_observation(metadata, state).await
        } else {
            None
        };
    let mut unit = atom(&state.data).await?;
    let catalog_revision = CATALOG_REVISION as i64;

    let feature_key = FeatureSettingsKey {
        feature: SYNC_FEATURE.to_string(),
    };
    let stored_feature =
        SyncFeatureSettings::atom_get(&mut unit, &feature_key).await?;
    SyncFeatureSettings::atom_set(
        &mut unit,
        &feature_key,
        Some(&FeatureSettingsRecord {
            globally_enabled: true,
            new_instance_default: stored_feature
                .is_none_or(|stored| stored.new_instance_default),
        }),
    )
    .await?;
    let instance_key = InstancePreferenceKey {
        instance_id: source_id.to_string(),
        feature: SYNC_FEATURE.to_string(),
    };
    InstanceSyncPreferences::atom_set(
        &mut unit,
        &instance_key,
        Some(&InstancePreferenceRecord { enabled: true }),
    )
    .await?;
    let stored_values = SyncedGameOptionValues::atom_get_all(&mut unit).await?;
    for (option_id, stored) in stored_values {
        SyncedGameOptionValues::atom_set(
            &mut unit,
            &option_id,
            Some(&GameOptionValueRecord {
                seeded: false,
                revision: stored.revision.saturating_add(1),
                updated_at: now,
                ..stored
            }),
        )
        .await?;
    }
    for definition in all_supported_settings() {
        for key in definition.keys {
            let custom_id = custom_setting_id(key);
            let Some(custom) = current_values.get(&custom_id) else {
                continue;
            };
            let revision = custom
                .revision
                .max(
                    current_preferences
                        .get(&custom_id)
                        .map(|preference| preference.revision)
                        .unwrap_or(0),
                )
                .saturating_add(1) as i64;
            if let Some(stored) =
                SyncedGameOptionValues::atom_get(&mut unit, &custom_id).await?
            {
                SyncedGameOptionValues::atom_set(
                    &mut unit,
                    &custom_id,
                    Some(&GameOptionValueRecord {
                        seeded: false,
                        revision,
                        updated_at: now,
                        ..stored
                    }),
                )
                .await?;
            }
            if SyncedGameOptionPreferences::atom_get(&mut unit, &custom_id)
                .await?
                .is_some()
            {
                SyncedGameOptionPreferences::atom_set(
                    &mut unit,
                    &custom_id,
                    Some(&GameOptionPreferenceRecord {
                        enabled: false,
                        source: GameOptionPreferenceSource::CatalogDefault,
                        revision,
                    }),
                )
                .await?;
            }
        }
    }
    let mut seeded_any = false;
    let mut fullscreen_value = None;
    let mut fullscreen_sync_enabled = false;

    for definition in all_supported_settings() {
        if !supported_settings_cover_game_version(source_version) {
            break;
        }
        let found = definition.keys.iter().find_map(|key| {
            if launcher_keys.contains(*key) {
                return None;
            }
            let (index, _) = entries.get(*key).copied()?;
            let entry = document.lines[index].entry.as_ref()?;
            decode_value_for_version(
                definition,
                key,
                &entry.value,
                source_version,
            )
            .map(|value| ((*key).to_string(), value))
        });
        let Some((_physical_key, value)) = found else {
            continue;
        };
        if definition.id == "fullscreen" {
            fullscreen_value = Some(value.clone());
            fullscreen_sync_enabled = current_preferences
                .get(definition.id)
                .map(|preference| preference.enabled)
                .unwrap_or(definition.default_on);
        }
        seeded_any = true;
        let option_revision = current_values
            .get(definition.id)
            .map(|value| value.revision)
            .unwrap_or(0)
            .max(
                current_preferences
                    .get(definition.id)
                    .map(|value| value.revision)
                    .unwrap_or(0),
            )
            .saturating_add(1) as i64;
        let value_json = serde_json::to_string(&value)?;
        let canonical_type = value.type_name();
        let option_id = definition.id.to_string();
        SyncedGameOptionValues::atom_set(
            &mut unit,
            &option_id,
            Some(&GameOptionValueRecord {
                kind: GameOptionValueKind::Vanilla,
                raw_key: None,
                canonical_type: canonical_type.to_string(),
                canonical_value: Some(value_json),
                value_codec: "catalog".to_string(),
                seeded: true,
                revision: option_revision,
                origin: GameOptionValueOrigin::SourceSeed,
                source_game_version: Some(source_version.to_string()),
                source_instance_id: Some(source_id.to_string()),
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
                    revision: option_revision,
                }),
            )
            .await?;
        }
    }

    for (key, (index, _)) in entries {
        if launcher_keys.contains(key)
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
        let value = CanonicalValue::ExternalRaw(entry.value.clone());
        seeded_any = true;
        let option_revision = current_values
            .get(&option_id)
            .map(|value| value.revision)
            .unwrap_or(0)
            .max(
                current_preferences
                    .get(&option_id)
                    .map(|value| value.revision)
                    .unwrap_or(0),
            )
            .saturating_add(1) as i64;
        let value_json = serde_json::to_string(&value)?;
        let stored =
            SyncedGameOptionValues::atom_get(&mut unit, &option_id).await?;
        let record = match stored {
            Some(stored) => GameOptionValueRecord {
                raw_key: Some(key.to_string()),
                canonical_value: Some(value_json),
                seeded: true,
                revision: option_revision,
                origin: GameOptionValueOrigin::SourceSeed,
                source_game_version: Some(source_version.to_string()),
                source_instance_id: Some(source_id.to_string()),
                updated_at: now,
                ..stored
            },
            None => GameOptionValueRecord {
                kind: GameOptionValueKind::External,
                raw_key: Some(key.to_string()),
                canonical_type: "external_raw".to_string(),
                canonical_value: Some(value_json),
                value_codec: "exact_raw".to_string(),
                seeded: true,
                revision: option_revision,
                origin: GameOptionValueOrigin::SourceSeed,
                source_game_version: Some(source_version.to_string()),
                source_instance_id: Some(source_id.to_string()),
                updated_at: now,
            },
        };
        SyncedGameOptionValues::atom_set(&mut unit, &option_id, Some(&record))
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
                    revision: option_revision,
                }),
            )
            .await?;
        }
    }
    if !seeded_any {
        return Err(input_error(
            "The selected options.txt has no settings that can be safely used as a sync baseline.",
        ));
    }

    let next_revision = current_revision.saturating_add(1) as i64;
    SyncedGameOptionState::atom_set(
        &mut unit,
        &Singleton,
        Some(&GameOptionStateRecord {
            revision: next_revision,
            catalog_revision,
        }),
    )
    .await?;
    if let Some(value) = fullscreen_value.as_ref() {
        update_app_fullscreen_setting(
            &mut unit,
            value,
            fullscreen_sync_enabled,
        )
        .await?;
    }
    unit.commit().await?;
    super::locales::record_observations(
        &state.data,
        metadata,
        &document,
        locale_snapshot.as_deref(),
    )
    .await;
    super::locales::queue_game_locale_index();
    Ok(())
}

pub async fn list_sync_sources()
-> crate::Result<Vec<GameOptionsSourceCandidate>> {
    let state = State::get().await?;
    let _guard = state.lock_synced_options().await;
    let instances = crate::state::list_instances(&state.pool).await?;
    let mut sources = Vec::with_capacity(instances.len());
    for metadata in instances {
        let path = options_path(&metadata, &state)?;
        let version_supported = supported_settings_cover_game_version(
            &metadata.applied_content_set.game_version,
        );
        let mut eligible = true;
        let mut disabled_reason = None;
        let mut recognized_setting_count = 0;
        let mut custom_setting_count = 0;
        let installing = synced_options::sync_files_are_protected(&metadata);
        if !installing && !path.exists() {
            eligible = false;
            disabled_reason = Some(GameOptionsSourceIssue::MissingOptionsFile);
        } else if !installing {
            match read_document(&path).await {
                Ok((document, _)) => {
                    let launcher_keys =
                        currently_launcher_owned_keys(&metadata);
                    let entries = document.effective_entries();
                    let source_version =
                        metadata.applied_content_set.game_version.as_str();
                    let recognized = if version_supported {
                        all_supported_settings()
                            .filter(|setting| {
                                setting.keys.iter().any(|key| {
                                    if launcher_keys.contains(*key) {
                                        return false;
                                    }
                                    document.value(key).is_some_and(|raw| {
                                        decode_value_for_version(
                                            setting,
                                            key,
                                            raw,
                                            source_version,
                                        )
                                        .is_some()
                                    })
                                })
                            })
                            .count()
                    } else {
                        0
                    };
                    let custom = entries
                        .into_iter()
                        .filter(|(key, (index, _))| {
                            !launcher_keys.contains(*key)
                                && setting_by_file_key(key).is_none()
                                && !is_never_sync_key(key)
                                && document.lines[*index]
                                    .entry
                                    .as_ref()
                                    .is_some_and(|entry| {
                                        validate_raw_key_value(
                                            key,
                                            &entry.value,
                                        )
                                        .is_ok()
                                    })
                        })
                        .count();
                    if recognized + custom == 0 {
                        eligible = false;
                        disabled_reason = Some(if version_supported {
                            GameOptionsSourceIssue::NoSyncableSettings
                        } else {
                            GameOptionsSourceIssue::UnsupportedVersion
                        });
                    } else {
                        recognized_setting_count = recognized as u32;
                        custom_setting_count = custom as u32;
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        "Could not inspect {} as a game-settings source: {error}",
                        metadata.instance.id
                    );
                    eligible = false;
                    disabled_reason =
                        Some(GameOptionsSourceIssue::UnreadableOptionsFile);
                }
            }
        }
        sources.push(GameOptionsSourceCandidate {
            source_id: metadata.instance.id.clone(),
            instance_id: metadata.instance.id,
            name: metadata.instance.name,
            icon_path: metadata.instance.icon_path,
            game_version: Some(metadata.applied_content_set.game_version),
            eligible,
            disabled_reason,
            recognized_setting_count,
            custom_setting_count,
        });
    }
    sources.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.instance_id.cmp(&right.instance_id))
    });
    Ok(sources)
}
