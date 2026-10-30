//! What the telemetry collections promise: the installation the launcher
//! reports as, and the events waiting to be uploaded.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::telemetry_identity::{
    Installation, TelemetryIdentity, TelemetryIdentityRecord,
};
use crate::state::store::telemetry_outbox::{
    TelemetryEventType, TelemetryOutbox, TelemetryOutboxRecord,
    TelemetryOutboxScope,
};

fn identity(day: Option<&str>) -> TelemetryIdentityRecord {
    TelemetryIdentityRecord {
        installation_id: "installation".to_string(),
        last_heartbeat_day: day.map(|day| day.to_string()),
    }
}

fn key(event_id: &str) -> String {
    event_id.to_string()
}

fn event(created_at: i64) -> TelemetryOutboxRecord {
    let payload = format!("{{\"type\":\"heartbeat\",\"at\":{created_at}}}");
    TelemetryOutboxRecord {
        event_type: TelemetryEventType::Heartbeat,
        size_bytes: payload.len() as i64,
        payload,
        created_at,
        next_attempt_at: created_at,
        attempts: 0,
        dedupe_key: format!("heartbeat:{created_at}"),
        occurrence_count: 1,
    }
}

#[tokio::test]
async fn an_identity_round_trips_and_clears() {
    let handle = support::handle().await;

    TelemetryIdentity::set(&handle, &Installation, Some(&identity(None)))
        .await
        .unwrap();
    assert_eq!(
        TelemetryIdentity::get(&handle, &Installation)
            .await
            .unwrap(),
        Some(identity(None))
    );

    let reported = identity(Some("2026-08-17"));
    TelemetryIdentity::set(&handle, &Installation, Some(&reported))
        .await
        .unwrap();
    assert_eq!(
        TelemetryIdentity::get(&handle, &Installation)
            .await
            .unwrap(),
        Some(reported)
    );

    TelemetryIdentity::set(&handle, &Installation, None)
        .await
        .unwrap();
    assert_eq!(
        TelemetryIdentity::get(&handle, &Installation)
            .await
            .unwrap(),
        None
    );
    assert!(
        TelemetryIdentity::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_installation_holds_one_identity() {
    let handle = support::handle().await;

    TelemetryIdentity::set_all(&handle, &[(Installation, identity(None))])
        .await
        .unwrap();
    assert_eq!(
        TelemetryIdentity::get(&handle, &Installation)
            .await
            .unwrap(),
        Some(identity(None))
    );

    TelemetryIdentity::set_all(&handle, &[]).await.unwrap();
    assert_eq!(
        TelemetryIdentity::get(&handle, &Installation)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn an_event_round_trips_and_clears() {
    let handle = support::handle().await;
    let stored = event(10);

    TelemetryOutbox::set(&handle, &key("event"), Some(&stored))
        .await
        .unwrap();
    assert_eq!(
        TelemetryOutbox::get(&handle, &key("event")).await.unwrap(),
        Some(stored)
    );

    TelemetryOutbox::set(&handle, &key("event"), None)
        .await
        .unwrap();
    assert_eq!(
        TelemetryOutbox::get(&handle, &key("event")).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn a_dedupe_key_holds_one_event() {
    let handle = support::handle().await;
    let mut first = event(10);
    first.dedupe_key = "heartbeat:2026-08-17".to_string();
    let mut second = event(20);
    second.dedupe_key = "heartbeat:2026-08-17".to_string();

    TelemetryOutbox::set(&handle, &key("first"), Some(&first))
        .await
        .unwrap();
    TelemetryOutbox::set(&handle, &key("second"), Some(&second))
        .await
        .unwrap();

    assert_eq!(
        TelemetryOutbox::get_all(&handle).await.unwrap(),
        vec![(key("second"), second)]
    );
}

#[tokio::test]
async fn the_due_scope_reaches_only_the_events_that_may_be_sent() {
    let handle = support::handle().await;
    let due = event(10);
    let mut later = event(20);
    later.next_attempt_at = 100;

    TelemetryOutbox::set(&handle, &key("due"), Some(&due))
        .await
        .unwrap();
    TelemetryOutbox::set(&handle, &key("later"), Some(&later))
        .await
        .unwrap();

    let read = TelemetryOutbox::get_scoped(
        &handle,
        &TelemetryOutboxScope::Due {
            kind: TelemetryEventType::Heartbeat,
            at: 20,
        },
    )
    .await
    .unwrap();

    assert_eq!(read, vec![(key("due"), due)]);
}

#[tokio::test]
async fn writing_no_error_events_clears_only_the_queued_ones() {
    let handle = support::handle().await;
    let heartbeat = event(10);
    let mut error = event(20);
    error.event_type = TelemetryEventType::Error;

    TelemetryOutbox::set(&handle, &key("heartbeat"), Some(&heartbeat))
        .await
        .unwrap();
    TelemetryOutbox::set(&handle, &key("error"), Some(&error))
        .await
        .unwrap();

    TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Kind(TelemetryEventType::Error),
        &[],
    )
    .await
    .unwrap();

    assert_eq!(
        TelemetryOutbox::get_all(&handle).await.unwrap(),
        vec![(key("heartbeat"), heartbeat)]
    );
}

#[tokio::test]
async fn writing_no_events_clears_the_outbox() {
    let handle = support::handle().await;
    TelemetryOutbox::set(&handle, &key("event"), Some(&event(10)))
        .await
        .unwrap();

    TelemetryOutbox::set_all(&handle, &[]).await.unwrap();

    assert!(TelemetryOutbox::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_event_outside_the_scope_is_refused() {
    let handle = support::handle().await;
    let mut error = event(10);
    error.event_type = TelemetryEventType::Error;
    let mut later = event(20);
    later.next_attempt_at = 100;

    let wrong_kind = TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Kind(TelemetryEventType::Heartbeat),
        &[(key("error"), error)],
    )
    .await;
    assert!(wrong_kind.is_err());

    let not_due = TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Due {
            kind: TelemetryEventType::Heartbeat,
            at: 20,
        },
        &[(key("later"), later)],
    )
    .await;
    assert!(not_due.is_err());
    assert!(TelemetryOutbox::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_event_kind_the_table_does_not_trust_is_no_event() {
    let handle = support::handle().await;
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints = ON;
         INSERT INTO telemetry_outbox (
             event_id, event_type, payload, created_at, next_attempt_at,
             attempts, size_bytes, dedupe_key
         ) VALUES (
             'legacy', 'legacy', jsonb('{\"type\":\"legacy\"}'), 1, 1, 0, 19,
             'legacy'
         );
         PRAGMA ignore_check_constraints = OFF;",
    )
    .execute(handle.pool())
    .await
    .unwrap();

    assert_eq!(
        TelemetryOutbox::get(&handle, &key("legacy")).await.unwrap(),
        None
    );
    assert!(TelemetryOutbox::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_row_another_part_of_the_queue_owns_is_refused() {
    let handle = support::handle().await;
    // An error event under one id, and one this build cannot read under another.
    sqlx::raw_sql(
        "PRAGMA ignore_check_constraints = ON;
         INSERT INTO telemetry_outbox (
             event_id, event_type, payload, created_at, next_attempt_at,
             attempts, size_bytes, dedupe_key
         ) VALUES
             ('taken', 'error', jsonb('{\"type\":\"error\"}'), 1, 1, 0, 17,
              'shared'),
             ('legacy', 'legacy', jsonb('{\"type\":\"legacy\"}'), 1, 100, 0,
              19, 'legacy')",
    )
    .execute(handle.pool())
    .await
    .unwrap();

    // Another kind already holds the dedupe key this write would take.
    let mut stolen = event(30);
    stolen.dedupe_key = "shared".to_string();
    let dedupe_taken = TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Kind(TelemetryEventType::Heartbeat),
        &[(key("fresh"), stolen)],
    )
    .await;
    assert!(dedupe_taken.is_err());

    // A row written by a newer build is not the same as no row at all.
    let unreadable = TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Kind(TelemetryEventType::Heartbeat),
        &[(key("legacy"), event(30))],
    )
    .await;
    assert!(unreadable.is_err());

    // A row scheduled after the moment the scope names is not in it either.
    let early = TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Due {
            kind: TelemetryEventType::Error,
            at: 50,
        },
        &[(key("taken"), {
            let mut due = event(10);
            due.event_type = TelemetryEventType::Error;
            due
        })],
    )
    .await;
    assert!(early.is_err());
}

#[tokio::test]
async fn a_due_replacement_keeps_the_events_that_are_not_due_yet() {
    let handle = support::handle().await;
    TelemetryOutbox::set(&handle, &key("due"), Some(&event(10)))
        .await
        .unwrap();
    let mut later = event(20);
    later.next_attempt_at = 100;
    TelemetryOutbox::set(&handle, &key("later"), Some(&later))
        .await
        .unwrap();

    TelemetryOutbox::set_scoped(
        &handle,
        &TelemetryOutboxScope::Due {
            kind: TelemetryEventType::Heartbeat,
            at: 50,
        },
        &[(key("due"), event(10))],
    )
    .await
    .unwrap();

    let mut later = event(20);
    later.next_attempt_at = 100;
    assert_eq!(
        TelemetryOutbox::get_all(&handle).await.unwrap(),
        vec![(key("later"), later)]
    );
}
