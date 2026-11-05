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

async fn reviewed_calls(
    store: &AgentStore,
    id: &str,
    count: usize,
    selected: &[usize],
    check_only: bool,
) -> Vec<ExecutionRequest> {
    store
        .enqueue_attempt(&attempt(id), &json!({"check_only":check_only}))
        .await
        .unwrap();
    store
        .transition(id, AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 300;
    let actions: Vec<_> = (0..count).map(|index| Approval {
        id: format!("{id}-permit-{index}"), attempt_id: id.into(), call_id: format!("{id}-call-{index}"), tool: "instances_rename".into(), arguments: json!({"instance_id":"instance","name":index.to_string()}), revision: 1, expires_at,
    }).collect();
    let calls = actions
        .iter()
        .map(|action| ExecutionRequest {
            attempt_id: id.into(),
            call_id: action.call_id.clone(),
            tool: action.tool.clone(),
            arguments: action.arguments.clone(),
            is_write: true,
            recovery_control: false,
            approval: Some(ApprovalPermit {
                id: action.id.clone(),
                revision: 1,
            }),
        })
        .collect();
    let batch = ApprovalBatch {
        id: format!("{id}-batch"),
        attempt_id: id.into(),
        revision: 1,
        expires_at,
        actions,
    };
    store.prepare_batch(&batch).await.unwrap();
    store
        .decide_batch(
            &batch.id,
            1,
            &selected
                .iter()
                .map(|index| format!("{id}-call-{index}"))
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();
    calls
}

#[tokio::test]
async fn unknown_prior_writes_block_new_mutations_until_exact_native_receipt_settles()
 {
    let (directory, store) = setup().await;
    let old = reviewed_calls(&store, "old", 1, &[0], false)
        .await
        .remove(0);
    store.begin_execution(&old).await.unwrap();
    store.execution_progress("old", &old.call_id, &json!({"is_error":false,"structured_content":{"operation_id":"operation","status":"queued"}})).await.unwrap();
    store
        .transition("old", AttemptState::Running, AttemptState::Cancelled)
        .await
        .unwrap();
    store.close().await;
    let store = AgentStore::open(directory.path()).await.unwrap();
    let next = reviewed_calls(&store, "next", 1, &[0], false)
        .await
        .remove(0);
    assert!(store.begin_execution(&next).await.is_err());
    assert_eq!(store.approvals("next").await.unwrap()[0].state, "approved");
    let read = ExecutionRequest {
        call_id: "read".into(),
        tool: "instances_list".into(),
        arguments: json!({}),
        is_write: false,
        approval: None,
        ..next.clone()
    };
    assert!(matches!(
        store.begin_execution(&read).await.unwrap(),
        Admission::Execute
    ));
    let pending = store.unsettled_writes("task").await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["operation_id"], "operation");
    assert!(
        store
            .unsettled_writes("other-thread")
            .await
            .unwrap()
            .is_empty()
    );
    let observation = json!({"is_error":false,"structured_content":{"operation_id":"operation","tool":"instances_rename","status":"succeeded","result":{"name":"0"}}});
    for (field, value) in [
        ("operation_id", json!("foreign")),
        ("tool", json!("instances_delete")),
    ] {
        let mut wrong = observation.clone();
        wrong["structured_content"][field] = value;
        assert!(
            store
                .reconcile_operation("old", &old.call_id, &wrong)
                .await
                .is_err()
        );
    }
    assert!(
        store
            .reconcile_operation(
                "old",
                &old.call_id,
                &json!({"message":"I checked it, done"})
            )
            .await
            .is_err()
    );
    let mut running = observation.clone();
    running["structured_content"]["status"] = json!("running");
    assert!(
        !store
            .reconcile_operation("old", &old.call_id, &running)
            .await
            .unwrap()
    );
    assert!(store.begin_execution(&next).await.is_err());
    assert!(
        store
            .reconcile_operation("old", &old.call_id, &observation)
            .await
            .unwrap()
    );
    assert!(
        !store
            .reconcile_operation("old", &old.call_id, &observation)
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .execution("old", &old.call_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        "completed"
    );
    assert!(store.unsettled_writes("task").await.unwrap().is_empty());
    assert!(matches!(
        store.begin_execution(&next).await.unwrap(),
        Admission::Execute
    ));
    assert!(matches!(
        store.begin_execution(&next).await.unwrap(),
        Admission::Existing(_)
    ));
    assert!(store.begin_execution(&old).await.is_err());
    assert_eq!(store.applied_result("old").await.unwrap()["applied"], 1);
}

#[tokio::test]
async fn missing_operation_identity_and_failed_observation_never_prove_success()
{
    let (_directory, store) = setup().await;
    let call = reviewed_calls(&store, "old", 1, &[0], false)
        .await
        .remove(0);
    store.begin_execution(&call).await.unwrap();
    store
        .transition("old", AttemptState::Running, AttemptState::Failed)
        .await
        .unwrap();
    let fabricated = json!({"is_error":false,"structured_content":{"operation_id":"invented","tool":"instances_rename","status":"succeeded"}});
    assert!(
        store
            .reconcile_operation("old", &call.call_id, &fabricated)
            .await
            .is_err()
    );
    let next = reviewed_calls(&store, "next", 1, &[0], false)
        .await
        .remove(0);
    assert!(store.begin_execution(&next).await.is_err());
    let result = store.applied_result("old").await.unwrap();
    assert_eq!(result["unknown"], 1);
    assert_eq!(result["applied"], 0);
    assert_eq!(result["complete"], false);
}

#[tokio::test]
async fn applied_summary_counts_only_selected_successful_receipts() {
    let (_directory, store) = setup().await;
    let calls = reviewed_calls(&store, "run", 5, &[0, 1, 2, 3], false).await;
    store.begin_execution(&calls[0]).await.unwrap();
    store.finish_execution("run", &calls[0].call_id, &json!({"is_error":false,"structured_content":{"status":"succeeded"}}), false).await.unwrap();
    store.begin_execution(&calls[1]).await.unwrap();
    store
        .finish_execution(
            "run",
            &calls[1].call_id,
            &json!({"is_error":true}),
            true,
        )
        .await
        .unwrap();
    store.begin_execution(&calls[2]).await.unwrap();
    assert!(store.begin_execution(&calls[4]).await.is_err());
    let result = store.applied_result("run").await.unwrap();
    assert_eq!(result["selected"], 4);
    assert_eq!(result["applied"], 1);
    assert_eq!(result["failed"], 1);
    assert_eq!(result["unknown"], 1);
    assert_eq!(result["not_executed"], 1);
    assert_eq!(result["complete"], false);
    store
        .transition("run", AttemptState::Running, AttemptState::Cancelled)
        .await
        .unwrap();
    assert_eq!(store.applied_result("run").await.unwrap(), result);
}

#[tokio::test]
async fn recovered_native_admission_keeps_exact_arguments_and_stays_unknown_until_host_settles()
 {
    let (directory, store) = setup().await;
    let call = reviewed_calls(&store, "native", 1, &[0], false)
        .await
        .remove(0);
    store.begin_execution(&call).await.unwrap();
    let admission = json!({"is_error":false,"structured_content":{"operation_id":"operation","tool":call.tool,"status":"succeeded","result":{"job_id":"native-job","status":"queued"}}});
    store
        .execution_progress("native", &call.call_id, &admission)
        .await
        .unwrap();
    store
        .transition("native", AttemptState::Running, AttemptState::Interrupted)
        .await
        .unwrap();
    store.close().await;
    let store = AgentStore::open(directory.path()).await.unwrap();
    let pending = store.unsettled_writes("task").await.unwrap();
    assert_eq!(pending[0]["arguments"], call.arguments);
    assert_eq!(pending[0]["result"], admission);
    let next = reviewed_calls(&store, "follow", 1, &[0], false)
        .await
        .remove(0);
    let mut running = admission.clone();
    running["structured_content"]["native_observation"] = json!({"is_error":false,"structured_content":{"job_id":"native-job","status":"running"}});
    assert!(
        store
            .reconcile_execution_receipt(
                "native",
                &call.call_id,
                &admission,
                &running,
                None
            )
            .await
            .unwrap()
    );
    assert_eq!(store.applied_result("native").await.unwrap()["applied"], 0);
    assert_eq!(store.applied_result("native").await.unwrap()["unknown"], 1);
    assert!(store.begin_execution(&next).await.is_err());
    assert!(
        store
            .reconcile_execution_receipt(
                "native",
                &call.call_id,
                &admission,
                &running,
                Some(false)
            )
            .await
            .is_err()
    );
    let mut completed = running.clone();
    completed["structured_content"]["native_observation"]["structured_content"]
        ["status"] = json!("succeeded");
    assert!(
        store
            .reconcile_execution_receipt(
                "native",
                &call.call_id,
                &running,
                &completed,
                Some(false)
            )
            .await
            .unwrap()
    );
    assert_eq!(store.applied_result("native").await.unwrap()["applied"], 1);
    assert!(matches!(
        store.begin_execution(&next).await.unwrap(),
        Admission::Execute
    ));
    assert!(
        !store
            .reconcile_execution_receipt(
                "native",
                &call.call_id,
                &running,
                &completed,
                Some(false)
            )
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn rejected_cancellation_receipts_are_failed_never_applied() {
    let (_directory, store) = setup().await;
    let call = reviewed_calls(&store, "cancel", 1, &[0], false)
        .await
        .remove(0);
    store.begin_execution(&call).await.unwrap();
    let admission =
        json!({"is_error":false,"structured_content":{"accepted":false}});
    store
        .execution_progress("cancel", &call.call_id, &admission)
        .await
        .unwrap();
    store
        .transition("cancel", AttemptState::Running, AttemptState::Cancelled)
        .await
        .unwrap();
    let mut rejected = admission.clone();
    rejected["is_error"] = json!(true);
    assert!(
        store
            .reconcile_execution_receipt(
                "cancel",
                &call.call_id,
                &admission,
                &admission,
                Some(true)
            )
            .await
            .is_err()
    );
    assert!(
        store
            .reconcile_execution_receipt(
                "cancel",
                &call.call_id,
                &admission,
                &rejected,
                Some(true)
            )
            .await
            .unwrap()
    );
    let summary = store.applied_result("cancel").await.unwrap();
    assert_eq!(summary["applied"], 0);
    assert_eq!(summary["failed"], 1);
    assert_eq!(summary["complete"], false);
}

#[tokio::test]
async fn same_attempt_uncertainty_blocks_another_approved_write_without_replay()
{
    let (_directory, store) = setup().await;
    let calls = reviewed_calls(&store, "same", 2, &[0, 1], false).await;
    store.begin_execution(&calls[0]).await.unwrap();
    store.execution_progress("same", &calls[0].call_id, &json!({"is_error":false,"structured_content":{"job_id":"native-job","status":"running"}})).await.unwrap();
    assert!(store.has_unsettled_writes("task").await.unwrap());
    assert!(store.begin_execution(&calls[1]).await.is_err());
    assert_eq!(store.approvals("same").await.unwrap()[1].state, "approved");
    assert!(matches!(
        store.begin_execution(&calls[0]).await.unwrap(),
        Admission::Existing(_)
    ));
    let read = ExecutionRequest {
        call_id: "same-read".into(),
        tool: "instance_get".into(),
        is_write: false,
        approval: None,
        ..calls[1].clone()
    };
    assert!(matches!(
        store.begin_execution(&read).await.unwrap(),
        Admission::Execute
    ));
    store
        .finish_execution(
            "same",
            &calls[0].call_id,
            &json!({"is_error":false}),
            false,
        )
        .await
        .unwrap();
    assert!(!store.has_unsettled_writes("task").await.unwrap());
    assert!(matches!(
        store.begin_execution(&calls[1]).await.unwrap(),
        Admission::Execute
    ));
}

async fn reviewed_cancellation(
    store: &AgentStore,
    id: &str,
    check_only: bool,
) -> ExecutionRequest {
    store
        .enqueue_attempt(&attempt(id), &json!({"check_only":check_only}))
        .await
        .unwrap();
    store
        .transition(id, AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 300;
    let action = Approval {
        id: format!("{id}-permit"),
        attempt_id: id.into(),
        call_id: format!("{id}-cancel"),
        tool: "install_job_cancel".into(),
        arguments: json!({"job_id":"job"}),
        revision: 1,
        expires_at,
    };
    let call = ExecutionRequest {
        attempt_id: id.into(),
        call_id: action.call_id.clone(),
        tool: action.tool.clone(),
        arguments: action.arguments.clone(),
        is_write: true,
        recovery_control: true,
        approval: Some(ApprovalPermit {
            id: action.id.clone(),
            revision: 1,
        }),
    };
    store
        .prepare_batch(&ApprovalBatch {
            id: format!("{id}-batch"),
            attempt_id: id.into(),
            revision: 1,
            expires_at,
            actions: vec![action],
        })
        .await
        .unwrap();
    store
        .decide_batch(
            &format!("{id}-batch"),
            1,
            std::slice::from_ref(&call.call_id),
        )
        .await
        .unwrap();
    call
}

#[tokio::test]
async fn explicit_cancellation_bypasses_uncertainty_only_with_exact_approval_and_not_check_only()
 {
    let (_directory, store) = setup().await;
    let old = reviewed_calls(&store, "old-control", 1, &[0], false)
        .await
        .remove(0);
    store.begin_execution(&old).await.unwrap();
    store
        .transition(
            "old-control",
            AttemptState::Running,
            AttemptState::Cancelled,
        )
        .await
        .unwrap();
    let control = reviewed_cancellation(&store, "control", false).await;
    let mut invalid = control.clone();
    invalid.recovery_control = false;
    assert!(store.begin_execution(&invalid).await.is_err());
    invalid = control.clone();
    invalid.approval = None;
    assert!(store.begin_execution(&invalid).await.is_err());
    invalid = control.clone();
    invalid.arguments["job_id"] = json!("foreign");
    assert!(store.begin_execution(&invalid).await.is_err());
    assert_eq!(
        store.approvals("control").await.unwrap()[0].state,
        "approved"
    );
    assert!(matches!(
        store.begin_execution(&control).await.unwrap(),
        Admission::Execute
    ));
    assert!(matches!(
        store.begin_execution(&control).await.unwrap(),
        Admission::Existing(_)
    ));
    let readonly =
        reviewed_cancellation(&store, "readonly-control", true).await;
    assert!(store.begin_execution(&readonly).await.is_err());
    assert_eq!(
        store.approvals("readonly-control").await.unwrap()[0].state,
        "approved"
    );
}

#[test]
fn recovery_control_classification_cannot_be_deserialized_from_worker_json() {
    let request: ExecutionRequest = serde_json::from_value(json!({
        "attempt_id":"attempt","call_id":"call","tool":"install_job_cancel",
        "arguments":{"job_id":"job"},"is_write":true,"recovery_control":true,"approval":null
    })).unwrap();
    assert!(!request.recovery_control);
    assert!(
        serde_json::to_value(&request)
            .unwrap()
            .get("recovery_control")
            .is_none()
    );
}

#[tokio::test]
async fn persisted_readonly_mode_cannot_be_bypassed_even_with_approval() {
    let (_directory, store) = setup().await;
    let call = reviewed_calls(&store, "verify", 1, &[0], true)
        .await
        .remove(0);
    assert!(store.begin_execution(&call).await.is_err());
    assert!(
        store
            .execution("verify", &call.call_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.approvals("verify").await.unwrap()[0].state,
        "approved"
    );
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
        recovery_control: false,
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
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows[0].get::<Vec<u8>, _>("checksum"),
        migration.checksum.as_ref()
    );
    assert_eq!(rows[1].get::<i64, _>("version"), 2);
}
