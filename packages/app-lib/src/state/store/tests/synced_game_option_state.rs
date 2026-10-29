//! What the shared game option state collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::synced_game_option_state::{
    GameOptionStateRecord, Singleton, SyncedGameOptionState,
};

fn state() -> GameOptionStateRecord {
    GameOptionStateRecord {
        revision: 4,
        catalog_revision: 3,
    }
}

#[tokio::test]
async fn the_state_round_trips_and_clears() {
    let handle = support::handle().await;
    assert_eq!(
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap(),
        None
    );

    SyncedGameOptionState::set(&handle, &Singleton, Some(&state()))
        .await
        .unwrap();
    assert_eq!(
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap(),
        Some(state())
    );
    assert_eq!(
        SyncedGameOptionState::get_all(&handle).await.unwrap(),
        vec![(Singleton, state())]
    );

    SyncedGameOptionState::set(&handle, &Singleton, None)
        .await
        .unwrap();
    assert_eq!(
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap(),
        None
    );
    assert!(
        SyncedGameOptionState::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_replacement_writes_exactly_what_it_was_given() {
    let handle = support::handle().await;
    SyncedGameOptionState::set(&handle, &Singleton, Some(&state()))
        .await
        .unwrap();

    let replaced = GameOptionStateRecord {
        revision: 9,
        catalog_revision: 5,
    };
    SyncedGameOptionState::set_all(&handle, &[(Singleton, replaced)])
        .await
        .unwrap();

    assert_eq!(
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap(),
        Some(replaced)
    );

    SyncedGameOptionState::set_all(&handle, &[]).await.unwrap();
    assert_eq!(
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap(),
        None
    );
}
