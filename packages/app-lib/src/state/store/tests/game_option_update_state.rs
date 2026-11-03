//! What the update state collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::game_option_update_state::{
    GameOptionUpdateState, UpdateStateKey, UpdateStateRecord,
};

fn key(instance_id: &str) -> UpdateStateKey {
    UpdateStateKey {
        instance_id: instance_id.to_string(),
    }
}

fn replaced() -> UpdateStateRecord {
    UpdateStateRecord::Replaced {
        sha1: "abc".to_string(),
        document: b"options".to_vec(),
    }
}

#[tokio::test]
async fn an_update_state_round_trips_and_clears() {
    let handle = support::handle_with_instances(&["instance"]).await;

    GameOptionUpdateState::set(&handle, &key("instance"), Some(&replaced()))
        .await
        .unwrap();
    assert_eq!(
        GameOptionUpdateState::get(&handle, &key("instance"))
            .await
            .unwrap(),
        Some(replaced())
    );

    GameOptionUpdateState::set(
        &handle,
        &key("instance"),
        Some(&UpdateStateRecord::Absent),
    )
    .await
    .unwrap();
    assert_eq!(
        GameOptionUpdateState::get(&handle, &key("instance"))
            .await
            .unwrap(),
        Some(UpdateStateRecord::Absent)
    );

    GameOptionUpdateState::set(&handle, &key("instance"), None)
        .await
        .unwrap();
    assert_eq!(
        GameOptionUpdateState::get(&handle, &key("instance"))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn writing_a_whole_collection_drops_what_it_leaves_out() {
    let handle = support::handle_with_instances(&["kept", "dropped"]).await;
    GameOptionUpdateState::set(&handle, &key("kept"), Some(&replaced()))
        .await
        .unwrap();
    GameOptionUpdateState::set(&handle, &key("dropped"), Some(&replaced()))
        .await
        .unwrap();

    GameOptionUpdateState::set_all(&handle, &[(key("kept"), replaced())])
        .await
        .unwrap();

    assert_eq!(
        GameOptionUpdateState::get_all(&handle).await.unwrap(),
        vec![(key("kept"), replaced())]
    );
}

#[tokio::test]
async fn a_state_that_lost_its_document_is_ignored() {
    let handle = support::handle_with_instances(&["instance"]).await;
    sqlx::query(
        "INSERT INTO instance_game_option_update_state
            (instance_id, had_file, sha1, document)
         VALUES ('instance', 1, 'abc', NULL)",
    )
    .execute(handle.pool())
    .await
    .unwrap();

    assert_eq!(
        GameOptionUpdateState::get(&handle, &key("instance"))
            .await
            .unwrap(),
        None
    );
    assert!(
        GameOptionUpdateState::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}
