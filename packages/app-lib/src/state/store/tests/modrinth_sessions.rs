//! What the Modrinth sessions and the discovered runtimes promise.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::discovered_javas::{
    DiscoveredJavaRecord, DiscoveredJavas,
};
use crate::state::store::modrinth_users::{ModrinthUserRecord, ModrinthUsers};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn session(active: bool, expires: i64) -> ModrinthUserRecord {
    ModrinthUserRecord {
        active,
        session_id: "session".to_string(),
        expires,
    }
}

fn runtime(
    major_version: u32,
    distribution: Option<&str>,
) -> DiscoveredJavaRecord {
    DiscoveredJavaRecord {
        major_version,
        full_version: format!("{major_version}.0.1"),
        architecture: "x86_64".to_string(),
        distribution: distribution.map(|value| value.to_string()),
        file_size: 1024,
        file_mtime_ms: 1,
    }
}

#[tokio::test]
async fn a_session_round_trips_and_clears() {
    let handle = support::handle().await;
    let id = "account".to_string();

    ModrinthUsers::set(&handle, &id, Some(&session(false, 10)))
        .await
        .unwrap();
    assert_eq!(
        ModrinthUsers::get(&handle, &id).await.unwrap(),
        Some(session(false, 10))
    );

    ModrinthUsers::set(&handle, &id, None).await.unwrap();
    assert_eq!(ModrinthUsers::get(&handle, &id).await.unwrap(), None);
}

#[tokio::test]
async fn only_one_session_stays_active() {
    let handle = support::handle().await;

    ModrinthUsers::set(&handle, &"first".to_string(), Some(&session(true, 10)))
        .await
        .unwrap();
    ModrinthUsers::set(
        &handle,
        &"second".to_string(),
        Some(&session(true, 20)),
    )
    .await
    .unwrap();

    assert_eq!(
        ModrinthUsers::get(&handle, &"first".to_string())
            .await
            .unwrap()
            .map(|record| record.active),
        Some(false)
    );
    assert_eq!(
        ModrinthUsers::get(&handle, &"second".to_string())
            .await
            .unwrap()
            .map(|record| record.active),
        Some(true)
    );
}

#[tokio::test]
async fn a_replacement_naming_two_actives_is_refused() {
    let handle = support::handle().await;

    let refused = ModrinthUsers::set_all(
        &handle,
        &[
            ("first".to_string(), session(true, 10)),
            ("second".to_string(), session(true, 20)),
        ],
    )
    .await;

    assert!(refused.is_err());
    assert!(ModrinthUsers::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_runtime_keeps_the_distribution_it_came_from() {
    let (_directory, handle) = support::derived().await;
    let path = "/runtimes/21".to_string();

    DiscoveredJavas::set(&handle, &path, Some(&runtime(21, Some("temurin"))))
        .await
        .unwrap();

    assert_eq!(
        DiscoveredJavas::get(&handle, &path).await.unwrap(),
        Some(runtime(21, Some("temurin")))
    );
}

#[tokio::test]
async fn a_version_group_reads_only_its_runtimes() {
    let (_directory, handle) = support::derived().await;
    for (path, major) in [("/runtimes/17", 17), ("/runtimes/21", 21)] {
        DiscoveredJavas::set(
            &handle,
            &path.to_string(),
            Some(&runtime(major, None)),
        )
        .await
        .unwrap();
    }

    let group = DiscoveredJavas::get_scoped(&handle, &21).await.unwrap();

    assert_eq!(group.len(), 1);
    assert_eq!(group[0].0, "/runtimes/21");
}

#[tokio::test]
async fn a_runtime_of_another_version_cannot_be_written_into_a_group() {
    let (_directory, handle) = support::derived().await;
    let path = "/runtimes/17".to_string();
    DiscoveredJavas::set(&handle, &path, Some(&runtime(17, None)))
        .await
        .unwrap();

    let refused = DiscoveredJavas::set_scoped(
        &handle,
        &21,
        &[(path.clone(), runtime(21, None))],
    )
    .await;

    assert!(refused.is_err());
    assert_eq!(
        DiscoveredJavas::get(&handle, &path)
            .await
            .unwrap()
            .map(|record| record.major_version),
        Some(17)
    );
}

#[tokio::test]
async fn the_active_session_reads_on_its_own() {
    let handle = support::handle().await;
    ModrinthUsers::set(
        &handle,
        &"first".to_string(),
        Some(&session(false, 10)),
    )
    .await
    .unwrap();
    ModrinthUsers::set(
        &handle,
        &"second".to_string(),
        Some(&session(true, 20)),
    )
    .await
    .unwrap();

    let active = ModrinthUsers::get_scoped(&handle, &true).await.unwrap();
    let idle = ModrinthUsers::get_scoped(&handle, &false).await.unwrap();

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].0, "second");
    assert_eq!(idle.len(), 1);
    assert_eq!(idle[0].0, "first");
}

#[tokio::test]
async fn a_session_cannot_be_written_into_a_state_it_is_not_in() {
    let handle = support::handle().await;
    ModrinthUsers::set(&handle, &"idle".to_string(), Some(&session(false, 5)))
        .await
        .unwrap();
    ModrinthUsers::set(
        &handle,
        &"signed-in".to_string(),
        Some(&session(true, 10)),
    )
    .await
    .unwrap();

    let refused = ModrinthUsers::set_scoped(
        &handle,
        &false,
        &[("intruder".to_string(), session(true, 20))],
    )
    .await
    .unwrap_err();

    assert!(matches!(
        refused.raw.as_ref(),
        crate::ErrorKind::InputError(_)
    ));
    assert_eq!(
        ModrinthUsers::get(&handle, &"signed-in".to_string())
            .await
            .unwrap()
            .map(|record| record.active),
        Some(true)
    );
    assert_eq!(
        ModrinthUsers::get(&handle, &"idle".to_string())
            .await
            .unwrap(),
        Some(session(false, 5))
    );
}

#[tokio::test]
async fn a_session_cannot_be_moved_between_states() {
    let handle = support::handle().await;
    ModrinthUsers::set(&handle, &"idle".to_string(), Some(&session(false, 5)))
        .await
        .unwrap();
    ModrinthUsers::set(
        &handle,
        &"signed-in".to_string(),
        Some(&session(true, 10)),
    )
    .await
    .unwrap();

    let refused = ModrinthUsers::set_scoped(
        &handle,
        &true,
        &[("idle".to_string(), session(true, 7))],
    )
    .await
    .unwrap_err();

    assert!(matches!(
        refused.raw.as_ref(),
        crate::ErrorKind::InputError(_)
    ));
    assert_eq!(
        ModrinthUsers::get(&handle, &"signed-in".to_string())
            .await
            .unwrap(),
        Some(session(true, 10))
    );
    assert_eq!(
        ModrinthUsers::get(&handle, &"idle".to_string())
            .await
            .unwrap(),
        Some(session(false, 5))
    );
}
