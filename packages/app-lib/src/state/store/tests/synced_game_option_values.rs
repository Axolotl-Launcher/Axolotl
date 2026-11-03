//! What the shared game option values collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::synced_game_option_values::{
    GameOptionValueKind, GameOptionValueOrigin, GameOptionValueRecord,
    SyncedGameOptionValues,
};

fn vanilla() -> GameOptionValueRecord {
    GameOptionValueRecord {
        kind: GameOptionValueKind::Vanilla,
        raw_key: None,
        canonical_type: "bool".to_string(),
        canonical_value: Some("true".to_string()),
        value_codec: "catalog".to_string(),
        seeded: true,
        revision: 1,
        origin: GameOptionValueOrigin::Instance,
        source_game_version: Some("1.21".to_string()),
        source_instance_id: Some("instance".to_string()),
        updated_at: 7,
    }
}

#[tokio::test]
async fn a_value_round_trips_and_clears() {
    let handle = Handle::new(support::pool().await);
    let option_id = "renderDistance".to_string();
    SyncedGameOptionValues::set(&handle, &option_id, Some(&vanilla()))
        .await
        .unwrap();

    assert_eq!(
        SyncedGameOptionValues::get(&handle, &option_id)
            .await
            .unwrap(),
        Some(vanilla())
    );

    SyncedGameOptionValues::set(&handle, &option_id, None)
        .await
        .unwrap();
    assert_eq!(
        SyncedGameOptionValues::get(&handle, &option_id)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_value_this_build_cannot_read_is_no_record() {
    let pool = support::pool().await;
    let handle = Handle::new(pool.clone());
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "
        INSERT INTO synced_game_option_values
            (option_id, kind, raw_key, canonical_type, value_codec,
             canonical_value_json, seeded, revision, origin, updated_at)
        VALUES ('future', 'modded', NULL, 'bool', 'catalog', 'true',
            1, 1, 'app_editor', 0)
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
        SyncedGameOptionValues::get(&handle, &"future".to_string())
            .await
            .unwrap(),
        None
    );
    assert!(
        SyncedGameOptionValues::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}
