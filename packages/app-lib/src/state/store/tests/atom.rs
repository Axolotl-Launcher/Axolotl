//! What an atomic unit promises: it lands whole, or it does not land.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::atom;
use crate::state::store::join_log::{
    InstanceJoins, JoinLog, JoinLogEntry, JoinLogKey,
};
use chrono::{TimeZone, Utc};

fn key(host: &str) -> JoinLogKey {
    JoinLogKey {
        instance_id: "instance".to_string(),
        host: host.to_string(),
        port: 25565,
    }
}

fn joins(instance_id: &str) -> InstanceJoins {
    InstanceJoins {
        instance_id: instance_id.to_string(),
    }
}

fn entry(at: i64) -> JoinLogEntry {
    JoinLogEntry {
        join_time: Utc.timestamp_opt(at, 0).unwrap(),
    }
}

#[tokio::test]
async fn an_uncommitted_unit_writes_nothing_into_the_handle() {
    let handle = support::handle_with_instances(&["instance"]).await;
    JoinLog::set(&handle, &key("kept"), Some(&entry(1)))
        .await
        .unwrap();

    {
        let mut unit = atom(&handle).await.unwrap();
        JoinLog::atom_set(&mut unit, &key("dropped"), Some(&entry(2)))
            .await
            .unwrap();
        assert_eq!(
            JoinLog::atom_get(&mut unit, &key("dropped")).await.unwrap(),
            Some(entry(2))
        );
    }

    assert_eq!(
        JoinLog::get(&handle, &key("kept")).await.unwrap(),
        Some(entry(1))
    );
    assert!(
        JoinLog::get(&handle, &key("dropped"))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn an_atomic_replace_enforces_the_same_checks_as_a_plain_one() {
    let handle = support::handle_with_instances(&["instance"]).await;
    JoinLog::set(&handle, &key("first"), Some(&entry(1)))
        .await
        .unwrap();

    let mut unit = atom(&handle).await.unwrap();
    let duplicates = JoinLog::atom_set_all(
        &mut unit,
        &[(key("twice"), entry(2)), (key("twice"), entry(2))],
    )
    .await;
    assert!(matches!(
        duplicates.unwrap_err().raw.as_ref(),
        crate::ErrorKind::InputError(_)
    ));

    let foreign = JoinLog::atom_set_scoped(
        &mut unit,
        &joins("another instance"),
        &[(key("first"), entry(1))],
    )
    .await;
    assert!(matches!(
        foreign.unwrap_err().raw.as_ref(),
        crate::ErrorKind::InputError(_)
    ));

    drop(unit);
    assert_eq!(JoinLog::get_all(&handle).await.unwrap().len(), 1);
}
