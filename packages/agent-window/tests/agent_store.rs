use agent_window::persistence::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

async fn setup() -> (tempfile::TempDir, AgentStore, Attempt, ExecutionRequest) {
    let dir = tempfile::tempdir().unwrap();
    let store = AgentStore::open(dir.path()).await.unwrap();
    store
        .set_thread("thread", &json!({"id": "thread", "draft": "keep me"}))
        .await
        .unwrap();
    let attempt = Attempt {
        id: "attempt".into(),
        thread_id: "thread".into(),
        turn_id: "turn".into(),
        state: AttemptState::Running,
        computer_use: true,
    };
    store.create_attempt(&attempt).await.unwrap();
    let call = ExecutionRequest {
        attempt_id: attempt.id.clone(),
        call_id: "call".into(),
        tool: "instances_rename".into(),
        arguments: json!({"id":"instance", "name":"new"}),
        is_write: true,
        recovery_control: false,
        approval: Some(ApprovalPermit {
            id: "approval".into(),
            revision: 1,
        }),
    };
    (dir, store, attempt, call)
}

async fn prepare(store: &AgentStore, call: &ExecutionRequest, expires_at: i64) {
    store
        .prepare_approval(&Approval {
            id: "approval".into(),
            attempt_id: call.attempt_id.clone(),
            call_id: call.call_id.clone(),
            tool: call.tool.clone(),
            arguments: call.arguments.clone(),
            revision: 1,
            expires_at,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn write_admission_requires_exact_current_approval_and_executes_once() {
    let (_dir, store, _attempt, call) = setup().await;
    assert!(store.begin_execution(&call).await.is_err());
    prepare(&store, &call, now() + 300).await;
    assert!(store.begin_execution(&call).await.is_err());
    assert!(store.decide("approval", 1, true).await.unwrap());
    assert!(!store.decide("approval", 1, true).await.unwrap());
    let mut tampered = call.clone();
    tampered.arguments["name"] = json!("other");
    assert!(store.begin_execution(&tampered).await.is_err());
    tampered = call.clone();
    tampered.tool = "instances_delete".into();
    assert!(store.begin_execution(&tampered).await.is_err());
    tampered = call.clone();
    tampered.approval.as_mut().unwrap().revision = 2;
    assert!(store.begin_execution(&tampered).await.is_err());
    let (first, second) = tokio::join!(
        store.begin_execution(&call),
        store.begin_execution(&call)
    );
    assert!(matches!(first.unwrap(), Admission::Execute));
    assert!(matches!(second.unwrap(), Admission::Existing(_)));
    assert!(!store.decide("approval", 1, true).await.unwrap());
    assert!(store.decide("approval", 1, false).await.is_err());
    assert!(store.finish_execution("attempt", "call", &json!({"ok":true,"access_token":"private", "nested":[{"api-key":"private"}]}), false).await.unwrap());
    assert!(
        !store
            .finish_execution("attempt", "call", &json!({"ok":false}), true)
            .await
            .unwrap()
    );
    let execution = store.execution("attempt", "call").await.unwrap().unwrap();
    assert_eq!(execution.state, "completed");
    assert_eq!(
        execution.result.unwrap(),
        json!({"ok":true,"access_token":"[redacted]", "nested":[{"api-key":"[redacted]"}]})
    );
    tampered.arguments["name"] = json!("different");
    assert!(store.begin_execution(&tampered).await.is_err());
}

#[tokio::test]
async fn rejection_expiry_and_terminal_runs_cannot_start_writes() {
    let (_dir, store, attempt, call) = setup().await;
    prepare(&store, &call, now() + 300).await;
    store.decide("approval", 1, false).await.unwrap();
    assert!(!store.decide("approval", 1, false).await.unwrap());
    assert!(store.decide("approval", 1, true).await.is_err());
    assert!(store.begin_execution(&call).await.is_err());
    assert!(
        store
            .transition(
                &attempt.id,
                AttemptState::Running,
                AttemptState::Cancelled
            )
            .await
            .unwrap()
    );
    assert!(
        !store
            .transition(
                &attempt.id,
                AttemptState::Running,
                AttemptState::Cancelled
            )
            .await
            .unwrap()
    );
    assert!(store.begin_execution(&call).await.is_err());
    assert!(
        store
            .append_event(&attempt.id, "late", &json!({"type":"done"}))
            .await
            .is_err()
    );
    assert!(
        store
            .transition(
                &attempt.id,
                AttemptState::Cancelled,
                AttemptState::Running
            )
            .await
            .is_err()
    );
    assert!(
        store
            .put_checkpoint(&attempt.id, "", "late", &json!({}))
            .await
            .is_err()
    );

    let (_dir, store, _, call) = setup().await;
    let expired = Approval {
        id: "expired".into(),
        attempt_id: call.attempt_id,
        call_id: call.call_id,
        tool: call.tool,
        arguments: call.arguments,
        revision: 1,
        expires_at: now() - 1,
    };
    assert!(store.prepare_approval(&expired).await.is_err());
    let long = Approval {
        expires_at: now() + 601,
        ..expired
    };
    assert!(store.prepare_approval(&long).await.is_err());
}

#[tokio::test]
async fn events_are_ordered_deduplicated_and_replay_by_cursor() {
    let (_dir, store, attempt, _) = setup().await;
    let event = store
        .append_event(&attempt.id, "event-1", &json!({"text":"a"}))
        .await
        .unwrap();
    let same = store
        .append_event(&attempt.id, "event-1", &json!({"text":"a"}))
        .await
        .unwrap();
    assert_eq!(event.sequence, same.sequence);
    assert!(
        store
            .append_event(&attempt.id, "event-1", &json!({"text":"changed"}))
            .await
            .is_err()
    );
    let second = store
        .append_event(&attempt.id, "event-2", &json!({"text":"b"}))
        .await
        .unwrap();
    assert!(second.sequence > event.sequence);
    let replay = store
        .events(&attempt.thread_id, event.sequence, 100)
        .await
        .unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].event_id, "event-2");
    assert!(store.events("another", 0, 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn restart_preserves_checkpoint_but_never_replays_uncertain_writes() {
    let (dir, store, attempt, call) = setup().await;
    prepare(&store, &call, now() + 300).await;
    store.decide("approval", 1, true).await.unwrap();
    store.begin_execution(&call).await.unwrap();
    let checkpoint = json!({"messages":[{"role":"assistant","tool_calls":[{"id":"call","args":{"id":"instance"}}],"opaque":{"thought_signature":"retain"}}]});
    store
        .put_checkpoint(&attempt.id, "", "cp", &checkpoint)
        .await
        .unwrap();
    store.close().await;
    assert!(dir.path().join("agent-window/agent.db").is_file());
    assert!(!dir.path().join("app.db").exists());
    let reopened = AgentStore::open(dir.path()).await.unwrap();
    let recovered = reopened.attempt(&attempt.id).await.unwrap().unwrap();
    assert_eq!(recovered.state, AttemptState::Interrupted);
    assert!(!recovered.computer_use);
    assert_eq!(
        reopened
            .execution(&attempt.id, "call")
            .await
            .unwrap()
            .unwrap()
            .state,
        "unknown"
    );
    assert!(reopened.begin_execution(&call).await.is_err());
    assert_eq!(
        reopened
            .checkpoints(&attempt.thread_id, "", 10)
            .await
            .unwrap()[0]
            .payload,
        checkpoint
    );
    assert!(reopened.threads().await.unwrap()[0]["draft"] == "keep me");
}

#[tokio::test]
async fn import_is_atomic_repeatable_and_native_records_win() {
    let (_dir, store, _, _) = setup().await;
    let threads = vec![
        ("thread".into(), json!({"draft":"old"})),
        (
            "imported".into(),
            json!({"id":"imported","preview":true,"draft":"retain"}),
        ),
    ];
    assert!(store.import_threads("v2", &threads).await.unwrap());
    assert!(!store.import_threads("v2", &threads).await.unwrap());
    let all = store.threads().await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|t| t["draft"] == "keep me"));
    assert!(all.iter().any(|t| t["preview"] == true));
    let invalid = vec![("rollback".into(), json!({})), ("".into(), json!({}))];
    assert!(store.import_threads("failed", &invalid).await.is_err());
    assert_eq!(store.threads().await.unwrap().len(), 2);
    assert!(
        store
            .import_threads("failed", &[("valid".into(), json!({}))])
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn task_ownership_and_active_delete_are_enforced() {
    let (_dir, store, attempt, mut call) = setup().await;
    assert!(!store.create_attempt(&attempt).await.unwrap());
    assert!(
        store
            .create_attempt(&Attempt {
                id: "another".into(),
                ..attempt.clone()
            })
            .await
            .is_err()
    );
    store
        .set_thread("other", &json!({"id":"other"}))
        .await
        .unwrap();
    assert!(
        store
            .set_message("other", Some(&attempt.id), "message", &json!({}))
            .await
            .is_err()
    );
    store
        .set_message(
            "thread",
            Some(&attempt.id),
            "message",
            &json!({"content":"test"}),
        )
        .await
        .unwrap();
    assert!(
        store
            .set_message("other", None, "message", &json!({}))
            .await
            .is_err()
    );
    store
        .set_attachment("thread", "attachment", &json!({"text":"real"}))
        .await
        .unwrap();
    assert!(
        store
            .set_attachment("other", "attachment", &json!({}))
            .await
            .is_err()
    );
    assert!(store.delete_thread("thread").await.is_err());
    assert!(
        store
            .transition(
                &attempt.id,
                AttemptState::Running,
                AttemptState::Awaiting
            )
            .await
            .unwrap()
    );
    call.is_write = false;
    assert!(store.begin_execution(&call).await.is_err());
    store
        .transition(
            &attempt.id,
            AttemptState::Awaiting,
            AttemptState::Cancelled,
        )
        .await
        .unwrap();
    store.delete_thread("thread").await.unwrap();
    store.delete_thread("thread").await.unwrap();
    assert!(store.attempt(&attempt.id).await.unwrap().is_none());
    assert!(
        store
            .execution(&attempt.id, "call")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn credentials_are_rejected_in_arguments_and_redacted_in_text_mirrors() {
    let (_dir, store, _, mut call) = setup().await;
    call.arguments = json!({"credential":"do not persist"});
    let approval = Approval {
        id: "secret".into(),
        attempt_id: call.attempt_id.clone(),
        call_id: call.call_id.clone(),
        tool: call.tool.clone(),
        arguments: call.arguments.clone(),
        revision: 1,
        expires_at: now() + 300,
    };
    assert!(store.prepare_approval(&approval).await.is_err());
    call.is_write = false;
    assert!(store.begin_execution(&call).await.is_err());
    let result = json!({"structuredContent":{"api_key":"private"},"content":[{"text":"{\"api_key\":\"private\",\"name\":\"visible\"}"}]});
    let safe = redact(result);
    assert_eq!(safe["structuredContent"]["api_key"], "[redacted]");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            safe["content"][0]["text"].as_str().unwrap()
        )
        .unwrap()["api_key"],
        "[redacted]"
    );
    let text = json!({"text":"{ \"version\": 1 }"});
    assert_eq!(redact(text.clone()), text);
}

#[tokio::test]
async fn migrations_reopen_existing_records_and_fail_on_unknown_checksum() {
    use sqlx::{Connection, Row};
    let (dir, store, attempt, _) = setup().await;
    store
        .transition(&attempt.id, AttemptState::Running, AttemptState::Completed)
        .await
        .unwrap();
    store.close().await;
    let path = dir.path().join("agent-window/agent.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .foreign_keys(true);
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut connection)
            .await
            .unwrap()
            .is_empty()
    );
    let row = sqlx::query("SELECT version, success FROM _sqlx_migrations")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("version"), 1);
    assert!(row.get::<bool, _>("success"));
    sqlx::query("UPDATE _sqlx_migrations SET checksum = x'00'")
        .execute(&mut connection)
        .await
        .unwrap();
    drop(connection);
    assert!(matches!(
        AgentStore::open(dir.path()).await,
        Err(StoreError::Migration(_))
    ));
}
