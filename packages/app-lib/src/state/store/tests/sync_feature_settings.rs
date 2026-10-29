//! What the sync feature settings collection promises.

use super::support;
use crate::state::store::Collection as _;
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
