//! What the instance sync preferences collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::instance_sync_checkpoints::{
    CheckpointKey, CheckpointRecord, CheckpointStatus, InstanceSyncCheckpoints,
};
use crate::state::store::instance_sync_preferences::{
    InstancePreferenceKey, InstancePreferenceRecord, InstanceSyncPreferences,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn key(instance_id: &str, feature: &str) -> InstancePreferenceKey {
    InstancePreferenceKey {
        instance_id: instance_id.to_string(),
        feature: feature.to_string(),
    }
}

fn enabled(enabled: bool) -> InstancePreferenceRecord {
    InstancePreferenceRecord { enabled }
}

#[tokio::test]
async fn a_preference_round_trips_and_clears() {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    let handle = handle_of(&pool);
    let preference = key("instance", "screenshots");

    InstanceSyncPreferences::set(&handle, &preference, Some(&enabled(false)))
        .await
        .unwrap();
    assert_eq!(
        InstanceSyncPreferences::get(&handle, &preference)
            .await
            .unwrap(),
        Some(enabled(false))
    );

    InstanceSyncPreferences::set(&handle, &preference, None)
        .await
        .unwrap();
    assert_eq!(
        InstanceSyncPreferences::get(&handle, &preference)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_scope_reads_only_that_instances_preferences() {
    let pool = support::pool().await;
    support::add_instance(&pool, "first").await;
    support::add_instance(&pool, "second").await;
    let handle = handle_of(&pool);
    for (instance_id, feature) in [
        ("first", "screenshots"),
        ("second", "screenshots"),
        ("first", "game_options"),
    ] {
        InstanceSyncPreferences::set(
            &handle,
            &key(instance_id, feature),
            Some(&enabled(true)),
        )
        .await
        .unwrap();
    }

    let scoped =
        InstanceSyncPreferences::get_scoped(&handle, &"first".to_string())
            .await
            .unwrap();

    assert_eq!(scoped.len(), 2);
    assert!(scoped.iter().all(|(key, _)| key.instance_id == "first"));
}

#[tokio::test]
async fn a_replacement_keeps_the_checkpoint_of_a_preference_it_keeps() {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    support::enable_feature(&pool, "instance", "game_options").await;
    let handle = handle_of(&pool);
    let preference = key("instance", "game_options");
    let checkpoint = CheckpointKey {
        instance_id: "instance".to_string(),
        feature: "game_options".to_string(),
        variant: "default".to_string(),
    };
    InstanceSyncCheckpoints::set(
        &handle,
        &checkpoint,
        Some(&CheckpointRecord {
            expected_sha1: "abc".to_string(),
            merge_base: None,
            source_revision: 1,
            status: CheckpointStatus::Pending,
            link_mode: None,
        }),
    )
    .await
    .unwrap();

    InstanceSyncPreferences::set_all(
        &handle,
        &[(preference.clone(), enabled(true))],
    )
    .await
    .unwrap();

    assert!(
        InstanceSyncCheckpoints::get(&handle, &checkpoint)
            .await
            .unwrap()
            .is_some()
    );

    InstanceSyncPreferences::set_all(&handle, &[])
        .await
        .unwrap();

    assert!(
        InstanceSyncCheckpoints::get(&handle, &checkpoint)
            .await
            .unwrap()
            .is_none()
    );
}
