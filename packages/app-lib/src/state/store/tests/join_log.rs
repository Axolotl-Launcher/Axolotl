//! What the join log collection promises.

use chrono::{TimeZone, Utc};

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::join_log::{
    InstanceJoins, JoinLog, JoinLogEntry, JoinLogKey,
};

fn key(instance_id: &str, host: &str, port: u16) -> JoinLogKey {
    JoinLogKey {
        instance_id: instance_id.to_string(),
        host: host.to_string(),
        port,
    }
}

fn at(seconds: i64) -> JoinLogEntry {
    JoinLogEntry {
        join_time: Utc.timestamp_opt(seconds, 0).unwrap(),
    }
}

#[tokio::test]
async fn a_scoped_replace_refuses_another_instance() {
    let handle = support::handle_with_instances(&["first"]).await;
    support::add_instance(handle.pool(), "second").await;

    assert!(
        JoinLog::set_scoped(
            &handle,
            &InstanceJoins {
                instance_id: "first".to_string(),
            },
            &[(key("second", "mc.example.com", 25565), at(1))],
        )
        .await
        .is_err()
    );
    assert!(JoinLog::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_scoped_replace_keeps_the_given_times() {
    let handle = support::handle_with_instances(&["instance"]).await;
    let scope = InstanceJoins {
        instance_id: "instance".to_string(),
    };

    JoinLog::set_scoped(
        &handle,
        &scope,
        &[(key("instance", "mc.example.com", 25565), at(5))],
    )
    .await
    .unwrap();

    assert_eq!(
        JoinLog::get_scoped(&handle, &scope).await.unwrap(),
        vec![(key("instance", "mc.example.com", 25565), at(5))]
    );
}

#[tokio::test]
async fn a_join_replaces_the_time_the_address_held() {
    let handle = support::handle_with_instances(&["instance"]).await;
    let address = key("instance", "mc.example.com", 25565);

    JoinLog::set(&handle, &address, Some(&at(1_700_000_000)))
        .await
        .unwrap();
    JoinLog::set(&handle, &address, Some(&at(1_700_000_600)))
        .await
        .unwrap();

    assert_eq!(
        JoinLog::get(&handle, &address).await.unwrap(),
        Some(at(1_700_000_600))
    );
    assert_eq!(
        JoinLog::get_all(&handle).await.unwrap(),
        vec![(address, at(1_700_000_600))]
    );
}

#[tokio::test]
async fn writing_nothing_removes_the_record() {
    let handle = support::handle_with_instances(&["instance"]).await;
    let address = key("instance", "mc.example.com", 25565);

    JoinLog::set(&handle, &address, Some(&at(1_700_000_000)))
        .await
        .unwrap();
    JoinLog::set(&handle, &address, None).await.unwrap();

    assert_eq!(JoinLog::get(&handle, &address).await.unwrap(), None);
    assert!(JoinLog::get_all(&handle).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_whole_collection_round_trips() {
    let handle = support::handle_with_instances(&["first", "second"]).await;
    let records = vec![
        (key("first", "mc.example.com", 25565), at(1_700_000_000)),
        (key("second", "mc.example.org", 25566), at(1_700_000_600)),
    ];

    JoinLog::set_all(&handle, &records).await.unwrap();

    let mut stored = JoinLog::get_all(&handle).await.unwrap();
    stored.sort_by(|left, right| left.0.instance_id.cmp(&right.0.instance_id));
    assert_eq!(stored, records);
}

#[tokio::test]
async fn a_port_outside_the_range_is_ignored() {
    let handle = support::handle_with_instances(&["instance"]).await;
    JoinLog::set(
        &handle,
        &key("instance", "mc.example.com", 25565),
        Some(&at(1_700_000_000)),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO join_log (instance_id, host, port, join_time)
         VALUES ('instance', 'mc.example.org', 70000, 1700000000)",
    )
    .execute(handle.pool())
    .await
    .unwrap();

    let stored = JoinLog::get_all(&handle).await.unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0.host, "mc.example.com");
    assert_eq!(stored[0].0.port, 25565);
}
