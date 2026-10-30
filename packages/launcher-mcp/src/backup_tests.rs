use super::*;

#[tokio::test]
async fn backup_mutations_are_deferred_and_scoped() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Backup tools").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let cases = [
        ("backup_enable", json!({"instance_id":"instance"})),
        ("backup_disable", json!({"instance_id":"instance"})),
        (
            "backup_set_exclusions",
            json!({"instance_id":"instance","files":["options.txt"],"directories":["logs","测试目录"]}),
        ),
        ("backup_create", json!({"instance_id":"instance"})),
        (
            "backup_preview_restore",
            json!({"instance_id":"instance","snapshot_id":"snapshot"}),
        ),
        ("backup_restore", json!({"preview_id":uuid::Uuid::new_v4()})),
        (
            "backup_delete_snapshot",
            json!({"instance_id":"instance","snapshot_id":"snapshot"}),
        ),
    ];
    for (name, mut args) in cases {
        args["session_id"] = json!(session);
        let response = client.tool(name, args).await;
        let operation = response["result"]["structuredContent"]["operation_id"]
            .as_str()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let response = client
                    .tool(
                        "operations_get",
                        json!({"session_id":session,"operation_id":operation}),
                    )
                    .await;
                let result = &response["result"]["structuredContent"];
                if result["status"] == "succeeded" {
                    assert_eq!(result["result"]["executed"], true);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 7);
    assert!(
        backend
            .scopes
            .lock()
            .unwrap()
            .iter()
            .all(|scope| scope == &session)
    );
    server.stop().await;
}

#[tokio::test]
async fn backup_reads_do_not_request_control_or_accept_foreign_sessions() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let owner = connect(&server, false, "Owner").await;
    let other = connect(&server, false, "Other").await;
    assert_eq!(
        tool_error(
            &owner
                .tool(
                    "backup_repository_status",
                    json!({"session_id":"invented"})
                )
                .await
        ),
        "session_required"
    );
    assert!(server.snapshot().connection.is_none());
    let session = control_id(&owner.tool("start_session", json!({})).await);
    assert_eq!(tool_error(&other.tool("backup_get_restore_preview", json!({"session_id":session,"preview_id":uuid::Uuid::new_v4()})).await), "session_required");
    assert!(backend.calls.lock().unwrap().is_empty());
    server.stop().await;
}
