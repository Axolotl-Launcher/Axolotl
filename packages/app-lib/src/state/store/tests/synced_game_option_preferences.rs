//! What the shared game option preferences collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::synced_game_option_preferences::{
    GameOptionPreferenceRecord, GameOptionPreferenceSource,
    SyncedGameOptionPreferences,
};

fn enabled() -> GameOptionPreferenceRecord {
    GameOptionPreferenceRecord {
        enabled: true,
        source: GameOptionPreferenceSource::User,
        revision: 2,
    }
}

#[tokio::test]
async fn a_preference_round_trips_and_clears() {
    let handle = support::handle().await;
    let option_id = "renderDistance".to_string();
    SyncedGameOptionPreferences::set(&handle, &option_id, Some(&enabled()))
        .await
        .unwrap();

    assert_eq!(
        SyncedGameOptionPreferences::get(&handle, &option_id)
            .await
            .unwrap(),
        Some(enabled())
    );

    SyncedGameOptionPreferences::set(&handle, &option_id, None)
        .await
        .unwrap();
    assert_eq!(
        SyncedGameOptionPreferences::get(&handle, &option_id)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_whole_collection_round_trips() {
    let handle = support::handle().await;
    let records = vec![
        ("fullscreen".to_string(), enabled()),
        (
            "renderDistance".to_string(),
            GameOptionPreferenceRecord {
                enabled: false,
                source: GameOptionPreferenceSource::CatalogDefault,
                revision: 1,
            },
        ),
    ];

    SyncedGameOptionPreferences::set_all(&handle, &records)
        .await
        .unwrap();

    let mut stored =
        SyncedGameOptionPreferences::get_all(&handle).await.unwrap();
    stored.sort_by(|left, right| left.0.cmp(&right.0));
    let mut expected = records;
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(stored, expected);
}

#[tokio::test]
async fn a_preference_this_build_cannot_read_is_no_record() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "
        INSERT INTO synced_game_option_preferences
            (option_id, enabled, source, revision)
        VALUES ('future', 1, 'modded', 1)
        ",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints = OFF")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        SyncedGameOptionPreferences::get(&handle, &"future".to_string())
            .await
            .unwrap(),
        None
    );
    assert!(
        SyncedGameOptionPreferences::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}
