use agent_window::persistence::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

async fn setup() -> (tempfile::TempDir, AgentStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = AgentStore::open(directory.path()).await.unwrap();
    store
        .set_thread("task", &json!({"id":"task"}))
        .await
        .unwrap();
    (directory, store)
}

fn attempt(id: &str) -> Attempt {
    Attempt {
        id: id.into(),
        thread_id: "task".into(),
        turn_id: format!("turn-{id}"),
        state: AttemptState::Queued,
        computer_use: false,
    }
}

#[tokio::test]
async fn queued_followups_are_durable_but_only_one_task_attempt_can_run() {
    let (_directory, store) = setup().await;
    assert!(
        store
            .enqueue_attempt(&attempt("one"), &json!({"text":"one"}))
            .await
            .unwrap()
    );
    assert!(
        !store
            .enqueue_attempt(&attempt("one"), &json!({"text":"one"}))
            .await
            .unwrap()
    );
    assert!(
        store
            .enqueue_attempt(&attempt("one"), &json!({"text":"tampered"}))
            .await
            .is_err()
    );
    store
        .enqueue_attempt(&attempt("two"), &json!({"text":"two"}))
        .await
        .unwrap();
    store
        .transition("one", AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    assert!(
        store
            .transition("two", AttemptState::Queued, AttemptState::Running)
            .await
            .is_err()
    );
    store
        .transition("one", AttemptState::Running, AttemptState::Completed)
        .await
        .unwrap();
    assert!(
        store
            .transition("two", AttemptState::Queued, AttemptState::Running)
            .await
            .unwrap()
    );
    assert_eq!(store.attempts("task").await.unwrap().len(), 2);
}

#[tokio::test]
async fn batch_decisions_bind_all_actions_and_repeated_approval_is_inert() {
    let (directory, store) = setup().await;
    store
        .enqueue_attempt(&attempt("run"), &json!({}))
        .await
        .unwrap();
    store
        .transition("run", AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 300;
    let actions = (0..2)
        .map(|i| Approval {
            id: format!("permit-{i}"),
            attempt_id: "run".into(),
            call_id: format!("call-{i}"),
            tool: "instances_rename".into(),
            arguments: json!({"name":i.to_string()}),
            revision: 1,
            expires_at: expires,
        })
        .collect();
    let batch = ApprovalBatch {
        id: "batch".into(),
        attempt_id: "run".into(),
        revision: 1,
        expires_at: expires,
        actions,
    };
    store.prepare_batch(&batch).await.unwrap();
    assert_eq!(
        store.attempt("run").await.unwrap().unwrap().state,
        AttemptState::Awaiting
    );
    assert!(
        store
            .decide_batch("batch", 2, &["call-0".into()])
            .await
            .is_err()
    );
    assert!(
        store
            .decide_batch("batch", 1, &["injected".into()])
            .await
            .is_err()
    );
    assert!(
        store
            .decide_batch("batch", 1, &["call-0".into()])
            .await
            .unwrap()
    );
    assert!(
        !store
            .decide_batch("batch", 1, &["call-0".into()])
            .await
            .unwrap()
    );
    assert!(
        store
            .decide_batch("batch", 1, &["call-1".into()])
            .await
            .is_err()
    );
    let selected = ExecutionRequest {
        attempt_id: "run".into(),
        call_id: "call-0".into(),
        tool: "instances_rename".into(),
        arguments: json!({"name":"0"}),
        is_write: true,
        approval: Some(ApprovalPermit {
            id: "permit-0".into(),
            revision: 1,
        }),
    };
    assert!(matches!(
        store.begin_execution(&selected).await.unwrap(),
        Admission::Execute
    ));
    store
        .execution_progress(
            "run",
            "call-0",
            &json!({"structured_content":{"operation_id":"durable-operation"}}),
        )
        .await
        .unwrap();
    let evidence = store.execution_evidence("run").await.unwrap();
    assert_eq!(evidence[0]["operation_id"], "durable-operation");
    assert_eq!(evidence[0]["arguments"], json!({"name":"0"}));
    let rejected = ExecutionRequest {
        call_id: "call-1".into(),
        arguments: json!({"name":"1"}),
        approval: Some(ApprovalPermit {
            id: "permit-1".into(),
            revision: 1,
        }),
        ..selected
    };
    assert!(store.begin_execution(&rejected).await.is_err());
    store
        .transition("run", AttemptState::Running, AttemptState::Cancelled)
        .await
        .unwrap();
    assert!(store.delete_thread("task").await.is_err());
    store.close().await;
    let reopened = AgentStore::open(directory.path()).await.unwrap();
    assert!(
        !reopened
            .decide_batch("batch", 1, &["call-0".into()])
            .await
            .unwrap()
    );
    assert!(reopened.begin_execution(&rejected).await.is_err());
}

#[tokio::test]
async fn graph_writes_preserve_first_results_and_update_errors_without_cross_attempt_access()
 {
    let (directory, store) = setup().await;
    store
        .enqueue_attempt(&attempt("run"), &json!({}))
        .await
        .unwrap();
    store
        .transition("run", AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    let record = GraphCheckpoint {
        id: "checkpoint".into(),
        checkpoint: json!({"type":"json","data":"YQ=="}),
        metadata: json!({"source":"loop"}),
        parent_id: None,
        writes: Vec::new(),
    };
    let writes = vec![
        GraphWrite {
            task_id: "tool-task".into(),
            index: 0,
            channel: "messages".into(),
            value: json!({"original":true}),
        },
        GraphWrite {
            task_id: "tool-task".into(),
            index: -1,
            channel: "__error__".into(),
            value: json!("old"),
        },
    ];
    store
        .put_graph_writes("run", "", "checkpoint", &writes)
        .await
        .unwrap();
    store
        .put_graph_checkpoint("run", "", &record)
        .await
        .unwrap();
    let changed = vec![
        GraphWrite {
            value: json!({"overwrite":true}),
            ..writes[0].clone()
        },
        GraphWrite {
            value: json!("latest"),
            ..writes[1].clone()
        },
    ];
    store
        .put_graph_writes("run", "", "checkpoint", &changed)
        .await
        .unwrap();
    store.close().await;
    let reopened = AgentStore::open(directory.path()).await.unwrap();
    let records = reopened
        .graph_checkpoints("run", "", None, None, 100)
        .await
        .unwrap();
    assert_eq!(records[0].writes[0].value, json!({"original":true}));
    assert_eq!(records[0].writes[1].value, json!("latest"));
    assert!(
        reopened
            .put_graph_writes("run", "", "checkpoint", &changed)
            .await
            .is_err()
    );
    reopened
        .enqueue_attempt(&attempt("other"), &json!({}))
        .await
        .unwrap();
    assert!(
        reopened
            .graph_checkpoints("other", "", None, None, 100)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn upgrades_original_schema_preserving_records_and_checksum() {
    use sqlx::{
        Connection, Row,
        migrate::{Migrate, Migration, MigrationType},
    };
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("agent-window")).unwrap();
    let path = directory.path().join("agent-window/agent.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    connection.ensure_migrations_table().await.unwrap();
    let migration = Migration::new(
        1,
        "agent store".into(),
        MigrationType::Simple,
        include_str!("../migrations/0001_agent_store.sql").into(),
        false,
    );
    connection.apply(&migration).await.unwrap();
    sqlx::query("INSERT INTO threads(id, payload) VALUES ('old', '{\"id\":\"old\",\"draft\":\"preserved\"}')").execute(&mut connection).await.unwrap();
    drop(connection);
    let store = AgentStore::open(directory.path()).await.unwrap();
    assert_eq!(store.threads().await.unwrap()[0]["draft"], "preserved");
    store.set_workspace(&json!({"active":"old"})).await.unwrap();
    assert_eq!(store.workspace().await.unwrap().unwrap()["active"], "old");
    store.close().await;
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
    let rows = sqlx::query(
        "SELECT version, checksum FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&mut connection)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].get::<Vec<u8>, _>("checksum"),
        migration.checksum.as_ref()
    );
    assert_eq!(rows[1].get::<i64, _>("version"), 2);
}
