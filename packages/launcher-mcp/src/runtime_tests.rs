use super::*;
use tokio_util::sync::CancellationToken;

fn local_context() -> ExecutionContext {
    ExecutionContext::new(
        ExecutionScope::local(true, CancellationToken::new()),
        CancellationToken::new(),
    )
}

async fn shared_server(runtime: Arc<ToolRuntime>) -> Server {
    Server::start_with_runtime(0, String::new(), true, false, runtime, |_| {})
        .await
        .unwrap()
}

#[tokio::test]
async fn local_and_mcp_calls_share_mutation_admission_and_keep_operation_ownership()
 {
    let backend = Arc::new(TestBackend::default());
    backend.block.store(true, Ordering::SeqCst);
    let runtime = Arc::new(ToolRuntime::new(
        LauncherInfo::new("9.8.7-test"),
        backend.clone(),
    ));
    let local = local_context();
    let local_write = tokio::spawn({
        let runtime = runtime.clone();
        let local = local.clone();
        async move {
            runtime
                .call(
                    "instance_rename",
                    json!({"instance_id":"instance","name":"Local"})
                        .as_object()
                        .unwrap()
                        .clone(),
                    local,
                )
                .await
                .unwrap()
        }
    });
    wait_for(|| backend.calls.lock().unwrap().len() == 1).await;
    let server = shared_server(runtime.clone()).await;
    let client = connect(&server, false, "Shared runtime").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let response = client
        .tool(
            "instance_launch",
            json!({"session_id":session,"instance_id":"instance"}),
        )
        .await;
    let operation = response["result"]["structuredContent"]["operation_id"]
        .as_str()
        .unwrap();
    operation_tests::poll(&client, &session, operation, "queued").await;
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    backend.block.store(false, Ordering::SeqCst);
    backend.resume.notify_one();
    assert!(!local_write.await.unwrap().is_error);
    operation_tests::poll(&client, &session, operation, "succeeded").await;
    assert_eq!(
        backend.scopes.lock().unwrap().as_slice(),
        &[local.scope.id(), &session]
    );
    let foreign = runtime
        .call(
            "operations_get",
            json!({"operation_id":operation})
                .as_object()
                .unwrap()
                .clone(),
            local,
        )
        .await
        .unwrap();
    assert_eq!(foreign.structured_content["error"]["code"], "not_found");
    server.stop().await;
}

#[tokio::test]
async fn stopping_and_restarting_mcp_preserves_local_work_and_revokes_old_scope()
 {
    let backend = Arc::new(TestBackend::default());
    backend.block.store(true, Ordering::SeqCst);
    let runtime = Arc::new(ToolRuntime::new(
        LauncherInfo::new("9.8.7-test"),
        backend.clone(),
    ));
    let server = shared_server(runtime.clone()).await;
    let client = connect(&server, false, "Before restart").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let response = client
        .tool(
            "instance_launch",
            json!({"session_id":session,"instance_id":"instance"}),
        )
        .await;
    let operation = response["result"]["structuredContent"]["operation_id"]
        .as_str()
        .unwrap();
    operation_tests::poll(&client, &session, operation, "running").await;
    wait_for(|| backend.calls.lock().unwrap().len() == 1).await;
    let transport = server.shared.inner.lock().unwrap().active.clone().unwrap();
    let old_scope =
        crate::tools::execution_scope(&server.shared, &transport, &session);
    server.stop().await;
    assert!(old_scope.authorize(false).is_err());
    backend.block.store(false, Ordering::SeqCst);
    backend.resume.notify_one();
    wait_for(|| backend.finished.load(Ordering::SeqCst) == 1).await;
    let local = local_context();
    let output = runtime
        .call("instance_list", serde_json::Map::new(), local.clone())
        .await
        .unwrap();
    assert!(!output.is_error);
    let restarted = shared_server(runtime.clone()).await;
    let client = connect(&restarted, false, "After restart").await;
    let next = control_id(&client.tool("start_session", json!({})).await);
    assert_eq!(
        tool_error(
            &client
                .tool(
                    "operations_get",
                    json!({"session_id":next,"operation_id":operation})
                )
                .await
        ),
        "not_found"
    );
    restarted.stop().await;
    assert!(
        !runtime
            .call("instance_list", serde_json::Map::new(), local)
            .await
            .unwrap()
            .is_error
    );
}

#[test]
fn wire_results_preserve_structured_success_errors_and_images() {
    for failed in [false, true] {
        let value = json!({"value":42});
        let output = ToolOutput {
            is_error: failed,
            ..ToolOutput::structured(value.clone())
        };
        let expected = if failed {
            rmcp::model::CallToolResult::structured_error(value)
        } else {
            rmcp::model::CallToolResult::structured(value)
        };
        assert_eq!(
            serde_json::to_value(crate::tools::encode(output)).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
    let image = ToolOutput::from_result(
        "ui_screenshot",
        Ok(
            json!({"image":{"data":"iVBORw0KGgoAAA","mime_type":"image/png"},"context":{"view_id":"view"}}),
        ),
    );
    let wire = serde_json::to_value(crate::tools::encode(image)).unwrap();
    assert_eq!(wire["content"][0]["type"], "image");
    assert_eq!(wire["content"][0]["mimeType"], "image/png");
    assert_eq!(wire["structuredContent"]["context"]["view_id"], "view");
    assert!(!wire["structuredContent"].to_string().contains("iVBOR"));
}
