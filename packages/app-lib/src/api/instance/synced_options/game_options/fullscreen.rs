//! Keeps the app-wide fullscreen setting aligned with the shared Minecraft value.

use super::CATALOG_REVISION;
use crate::state::store::Collection as _;
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
use crate::state::store::{Atom, atom};
use crate::state::{
    CanonicalValue, Settings, State, load_game_option_preferences,
    load_game_options_sync_state, load_shared_game_options,
};
use chrono::Utc;

const FULLSCREEN_OPTION_ID: &str = "fullscreen";

pub(crate) async fn shared_fullscreen_value(
    state: &State,
) -> crate::Result<Option<bool>> {
    let preferences = load_game_option_preferences(&state.data).await?;
    if !preferences
        .get(FULLSCREEN_OPTION_ID)
        .is_some_and(|preference| preference.enabled)
    {
        return Ok(None);
    }
    let values = load_shared_game_options(&state.data).await?;
    Ok(values.get(FULLSCREEN_OPTION_ID).and_then(|stored| {
        if !stored.seeded {
            return None;
        }
        match stored.value.as_ref() {
            Some(CanonicalValue::Bool(value)) => Some(*value),
            _ => None,
        }
    }))
}

pub(crate) async fn update_shared_fullscreen_from_app(
    state: &State,
    value: bool,
) -> crate::Result<bool> {
    let values = load_shared_game_options(&state.data).await?;
    let preferences = load_game_option_preferences(&state.data).await?;
    let stored = values.get(FULLSCREEN_OPTION_ID);
    let preference = preferences.get(FULLSCREEN_OPTION_ID);
    let value_matches = stored.is_some_and(|stored| {
        stored.seeded
            && matches!(
                stored.value.as_ref(),
                Some(CanonicalValue::Bool(current)) if *current == value
            )
    });
    let sync_enabled = preference.is_some_and(|preference| preference.enabled);
    if !sync_enabled || value_matches {
        return Ok(false);
    }

    let option_revision = stored
        .map(|stored| stored.revision)
        .unwrap_or(0)
        .max(
            preference
                .map(|preference| preference.revision)
                .unwrap_or(0),
        )
        .saturating_add(1) as i64;
    let (canonical_revision, _) =
        load_game_options_sync_state(&state.data, CATALOG_REVISION).await?;
    let next_canonical_revision = canonical_revision.saturating_add(1) as i64;
    let canonical = CanonicalValue::Bool(value);
    let value_json = serde_json::to_string(&canonical)?;
    let canonical_type = canonical.type_name();
    let now = Utc::now().timestamp();
    let option_id = FULLSCREEN_OPTION_ID.to_string();

    let mut unit = atom(&state.data).await?;
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
            origin: GameOptionValueOrigin::AppEditor,
            source_game_version: None,
            source_instance_id: None,
            updated_at: now,
        }),
    )
    .await?;
    SyncedGameOptionPreferences::atom_set(
        &mut unit,
        &option_id,
        Some(&GameOptionPreferenceRecord {
            enabled: true,
            source: GameOptionPreferenceSource::User,
            revision: option_revision,
        }),
    )
    .await?;
    SyncedGameOptionState::atom_set(
        &mut unit,
        &Singleton,
        Some(&GameOptionStateRecord {
            revision: next_canonical_revision,
            catalog_revision: CATALOG_REVISION as i64,
        }),
    )
    .await?;
    unit.commit().await?;
    Ok(true)
}

pub(super) async fn update_app_fullscreen_setting(
    unit: &mut Atom,
    value: &CanonicalValue,
    sync_enabled: bool,
) -> crate::Result<()> {
    if !sync_enabled {
        return Ok(());
    }
    let CanonicalValue::Bool(value) = value else {
        return Ok(());
    };
    let mut settings = Settings::load_in(unit).await?;
    settings.force_fullscreen = *value;
    settings.save_in(unit).await
}
