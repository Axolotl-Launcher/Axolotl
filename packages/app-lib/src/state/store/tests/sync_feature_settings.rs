//! What the sync feature settings collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::instance_sync_preferences::InstanceSyncPreferences;
use crate::state::store::sync_feature_settings::{
    FeatureSettingsKey, FeatureSettingsRecord, SyncFeatureSettings,
};

fn key(feature: &str) -> FeatureSettingsKey {
    FeatureSettingsKey {
        feature: feature.to_string(),
    }
}

fn enabled() -> FeatureSettingsRecord {
    FeatureSettingsRecord {
        globally_enabled: true,
        new_instance_default: true,
    }
}

#[tokio::test]
async fn a_feature_round_trips_and_clears() {
    let handle = support::handle().await;
    SyncFeatureSettings::set(&handle, &key("screenshots"), Some(&enabled()))
        .await
        .unwrap();

    assert_eq!(
        SyncFeatureSettings::get(&handle, &key("screenshots"))
            .await
            .unwrap(),
        Some(enabled())
    );

    SyncFeatureSettings::set(&handle, &key("screenshots"), None)
        .await
        .unwrap();
    assert_eq!(
        SyncFeatureSettings::get(&handle, &key("screenshots"))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_migrated_feature_reads_as_the_migration_left_it() {
    let handle = support::handle().await;

    assert_eq!(
        SyncFeatureSettings::get(&handle, &key("game_options"))
            .await
            .unwrap(),
        Some(FeatureSettingsRecord {
            globally_enabled: false,
            new_instance_default: false,
        })
    );
    assert!(
        !SyncFeatureSettings::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn turning_a_feature_on_reaches_every_instance() {
    let pool = support::pool().await;
    support::add_instance(&pool, "one").await;
    support::add_instance(&pool, "two").await;
    support::enable_feature(&pool, "one", "multiplayer_servers").await;
    let handle = Handle::new(pool);

    SyncFeatureSettings::set_globally_enabled(
        &handle,
        "multiplayer_servers",
        true,
    )
    .await
    .unwrap();

    assert_eq!(
        SyncFeatureSettings::get(&handle, &key("multiplayer_servers"))
            .await
            .unwrap(),
        Some(FeatureSettingsRecord {
            globally_enabled: true,
            new_instance_default: true,
        })
    );
    let mut enabled = InstanceSyncPreferences::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .filter(|(key, record)| {
            key.feature == "multiplayer_servers" && record.enabled
        })
        .map(|(key, _)| key.instance_id)
        .collect::<Vec<_>>();
    enabled.sort();
    assert_eq!(enabled, vec!["one".to_string(), "two".to_string()]);

    SyncFeatureSettings::set_globally_enabled(
        &handle,
        "multiplayer_servers",
        false,
    )
    .await
    .unwrap();
    assert!(
        InstanceSyncPreferences::get_all(&handle)
            .await
            .unwrap()
            .iter()
            .all(|(_, record)| !record.enabled)
    );
}
