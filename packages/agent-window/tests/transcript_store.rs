use agent_window::persistence::*;
use serde_json::{Value, json};

async fn setup() -> (tempfile::TempDir, AgentStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = AgentStore::open(dir.path()).await.unwrap();
    store
        .set_thread("thread", &json!({"id":"thread"}))
        .await
        .unwrap();
    store
        .enqueue_attempt(
            &Attempt {
                id: "attempt".into(),
                thread_id: "thread".into(),
                turn_id: "turn".into(),
                state: AttemptState::Queued,
                computer_use: false,
            },
            &json!({"provider_id":"provider","model_id":"model"}),
        )
        .await
        .unwrap();
    store
        .transition("attempt", AttemptState::Queued, AttemptState::Running)
        .await
        .unwrap();
    (dir, store)
}

#[tokio::test]
async fn continuation_keeps_successful_receipts_and_closes_unexecuted_calls_without_replay()
 {
    let (_dir, store) = setup().await;
    let messages = vec![
        json!({"role":"system","content":"old instructions"}),
        json!({"role":"user","content":"task"}),
    ];
    store
        .save_model_input("attempt", "request", &messages)
        .await
        .unwrap();
    store
        .save_model_input("attempt", "request", &messages)
        .await
        .unwrap();
    assert!(
        store
            .save_model_input(
                "attempt",
                "request",
                &[json!({"role":"user","content":"changed"})]
            )
            .await
            .is_err()
    );
    let read = ExecutionRequest {
        attempt_id: "attempt".into(),
        call_id: "observed".into(),
        tool: "read_state".into(),
        arguments: json!({}),
        is_write: false,
        approval: None,
    };
    assert!(matches!(
        store.begin_execution(&read).await.unwrap(),
        Admission::Execute
    ));
    store.finish_execution("attempt", "observed", &json!({"is_error":false,"content":[{"type":"text","text":"confirmed state"},{"type":"image","mime_type":"image/png","data":"aW1hZ2U="}]}), false).await.unwrap();
    let reply = json!({"role":"assistant","content":"","tool_calls":[{"id":"observed","name":"read_state","args":{}},{"id":"pending","name":"write_setting","args":{"name":"new"}}],"opaque":[{"signature":"preserved"}]});
    store
        .transition("attempt", AttemptState::Running, AttemptState::Cancelled)
        .await
        .unwrap();
    store
        .save_model_reply("attempt", "request", &reply)
        .await
        .unwrap();
    store
        .save_model_reply("attempt", "request", &reply)
        .await
        .unwrap();
    let transcript = store
        .conversation_transcript("thread", "provider", "model")
        .await
        .unwrap();
    assert_eq!(transcript.len(), 4);
    assert_eq!(transcript[1]["opaque"], reply["opaque"]);
    assert!(
        transcript[2]["content"]
            .as_str()
            .unwrap()
            .contains("confirmed state")
    );
    assert_eq!(transcript[2]["tool_error"], false);
    assert_eq!(transcript[2]["images"][0]["data"], "aW1hZ2U=");
    assert!(
        transcript[3]["content"]
            .as_str()
            .unwrap()
            .contains("not executed")
    );
    assert_eq!(transcript[3]["tool_error"], true);
    assert!(
        store
            .execution("attempt", "pending")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.attempt("attempt").await.unwrap().unwrap().state,
        AttemptState::Cancelled
    );
}

#[tokio::test]
async fn latest_snapshot_is_not_concatenated_and_model_change_projects_history_without_rewriting_archive()
 {
    let (_dir, store) = setup().await;
    let first = vec![json!({"role":"user","content":"earlier"})];
    store
        .save_model_input("attempt", "first", &first)
        .await
        .unwrap();
    store
        .save_model_reply(
            "attempt",
            "first",
            &json!({"role":"assistant","content":"first reply"}),
        )
        .await
        .unwrap();
    let input = vec![
        first[0].clone(),
        json!({"role":"assistant","content":"first reply"}),
        json!({"role":"user","content":"followup","images":[{"mime_type":"image/png","data":"aW1hZ2U="}]}),
    ];
    let reply = json!({"role":"assistant","content":"latest reply","opaque":[{"signature":"original"}]});
    store
        .save_model_input("attempt", "second", &input)
        .await
        .unwrap();
    store
        .save_model_reply("attempt", "second", &reply)
        .await
        .unwrap();
    store
        .transition("attempt", AttemptState::Running, AttemptState::Failed)
        .await
        .unwrap();
    let original = store
        .conversation_transcript("thread", "provider", "model")
        .await
        .unwrap();
    assert_eq!(original.len(), 4);
    let changed = store
        .conversation_transcript("thread", "different", "model")
        .await
        .unwrap();
    assert_eq!(changed.len(), 4);
    assert!(changed.iter().all(|message| message["role"] == "user"
        && message.get("opaque").is_none()
        && message.get("tool_call_id").is_none()
        && message.get("tool_calls").is_none()));
    assert!(
        changed[3]["content"]
            .as_str()
            .unwrap()
            .contains("Historical assistant")
    );
    assert_eq!(changed[2]["images"], input[2]["images"]);
    assert_eq!(
        store
            .conversation_transcript("thread", "provider", "model")
            .await
            .unwrap(),
        original
    );
    let changed_reply: Value =
        json!({"role":"assistant","content":"overwrite"});
    assert!(
        store
            .save_model_reply("attempt", "second", &changed_reply)
            .await
            .is_err()
    );
}
