//! What the shared logs collection promises: the history the settings page
//! shows, newest first, and the moment each log was first shared.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::shared_logs::{SharedLogRecord, SharedLogs};

fn log(created_at: i64, token: &str) -> SharedLogRecord {
    SharedLogRecord {
        url: "https://logshare.cn/log".to_string(),
        raw: "https://api.logshare.cn/raw".to_string(),
        token: token.to_string(),
        provider: "logshare".to_string(),
        instance_id: Some("instance".to_string()),
        instance_name: Some("Instance".to_string()),
        truncated: false,
        created_at,
    }
}

#[tokio::test]
async fn a_shared_log_round_trips_and_clears() {
    let handle = support::handle().await;
    let id = "log".to_string();

    SharedLogs::set(&handle, &id, Some(&log(10, "first")))
        .await
        .unwrap();
    assert_eq!(
        SharedLogs::get(&handle, &id).await.unwrap(),
        Some(log(10, "first"))
    );

    SharedLogs::set(&handle, &id, None).await.unwrap();
    assert_eq!(SharedLogs::get(&handle, &id).await.unwrap(), None);
}

#[tokio::test]
async fn the_history_answers_newest_first() {
    let handle = support::handle().await;
    for (id, created_at) in [("older", 10), ("newer", 20), ("oldest", 5)] {
        SharedLogs::set(&handle, &id.to_string(), Some(&log(created_at, id)))
            .await
            .unwrap();
    }

    let ids: Vec<String> = SharedLogs::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert_eq!(ids, ["newer", "older", "oldest"]);
}

#[tokio::test]
async fn recording_a_log_again_keeps_the_moment_it_was_first_shared() {
    let handle = support::handle().await;
    let id = "log".to_string();

    SharedLogs::set(&handle, &id, Some(&log(10, "first")))
        .await
        .unwrap();
    SharedLogs::set(&handle, &id, Some(&log(20, "second")))
        .await
        .unwrap();

    let stored = SharedLogs::get(&handle, &id).await.unwrap().unwrap();

    assert_eq!(stored.token, "second");
    assert_eq!(stored.created_at, 10);
}
