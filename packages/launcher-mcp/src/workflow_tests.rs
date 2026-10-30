use super::*;

fn samples() -> Vec<(&'static str, Value)> {
    let id = uuid::Uuid::new_v4();
    vec![
        (
            "resource_check_updates",
            json!({"instance_id":"instance","entry_ids":["entry"]}),
        ),
        (
            "resource_prepare_update",
            json!({"instance_id":"instance","entry_ids":["entry"]}),
        ),
        (
            "resource_prepare_switch",
            json!({"instance_id":"instance","entry_id":"entry","source":"modrinth","project_id":"project","release_id":"release"}),
        ),
        (
            "resource_execute_changes",
            json!({"plan_id":id,"revision":1}),
        ),
        (
            "modpack_prepare_install",
            json!({"source":"curseforge","project_id":"123","release_id":"456","name":"Pack"}),
        ),
        (
            "modpack_prepare_local",
            json!({"path":"C:/packs/pack.mrpack","name":"Pack"}),
        ),
        ("modpack_execute_install", json!({"plan_id":id})),
        ("instance_scan_imports", json!({"launcher":"prism"})),
        ("instance_prepare_import", json!({"source_id":id})),
        ("instance_execute_import", json!({"plan_id":id})),
        (
            "instance_plan_upgrade",
            json!({"instance_id":"instance","game_version":"1.21.1","loader":"fabric","shader_runtime":"none"}),
        ),
        (
            "instance_get_upgrade_plan",
            json!({"plan_id":id,"revision":1,"section":"items"}),
        ),
        (
            "instance_revise_upgrade",
            json!({"plan_id":id,"revision":1,"content_id":"content","action":"disable"}),
        ),
        (
            "instance_select_upgrade_solution",
            json!({"plan_id":id,"revision":1,"solution":"minimal_change"}),
        ),
        (
            "instance_execute_upgrade",
            json!({"plan_id":id,"revision":1,"create_full_backup":true}),
        ),
        ("ui_get_context", json!({})),
        ("ui_screenshot", json!({})),
        ("ui_navigate", json!({"path":"/settings/mcp"})),
        ("ui_click", json!({"view_id":id,"element_id":"element-1"})),
        (
            "ui_type",
            json!({"view_id":id,"element_id":"element-2","text":"新实例\n备注\t内容"}),
        ),
        ("ui_key", json!({"view_id":id,"key":"Enter"})),
        (
            "ui_scroll",
            json!({"view_id":id,"element_id":"viewport","direction":"down","pixels":500}),
        ),
    ]
}

#[test]
fn workflow_schemas_match_typed_dispatch_and_enforce_bounds() {
    let entries = catalog::entries();
    let typing = entries
        .iter()
        .find(|entry| entry.tool.name == "ui_type")
        .unwrap();
    for text in ["", "  ", "\n\t"] {
        catalog::validate(typing, json!({"session_id":"session","view_id":uuid::Uuid::new_v4(),"element_id":"element-1","text":text}).as_object().unwrap()).unwrap();
    }
    for (name, mut args) in samples() {
        let entry = entries
            .iter()
            .find(|entry| entry.tool.name == name)
            .unwrap();
        args["session_id"] = json!("session");
        catalog::validate(entry, args.as_object().unwrap()).unwrap();
        args.as_object_mut().unwrap().remove("session_id");
        let _command = serde_json::from_value::<Command>(
            json!({"name":name,"arguments":args}),
        )
        .unwrap();
        assert_eq!(entry.deferred, !name.starts_with("ui_"), "{name}");
    }
    for (name, mut args) in [
        (
            "resource_prepare_update",
            json!({"instance_id":"i","entry_ids":[]}),
        ),
        (
            "resource_prepare_update",
            json!({"instance_id":"i","entry_ids":vec!["entry";101]}),
        ),
        (
            "modpack_prepare_install",
            json!({"source":"mcarchive","project_id":"p","release_id":"r","name":"Pack"}),
        ),
        (
            "instance_execute_upgrade",
            json!({"plan_id":uuid::Uuid::new_v4(),"revision":1}),
        ),
        (
            "ui_scroll",
            json!({"view_id":uuid::Uuid::new_v4(),"element_id":"viewport","direction":"down","pixels":2001}),
        ),
        (
            "ui_type",
            json!({"view_id":uuid::Uuid::new_v4(),"element_id":"element-1","text":"bad\u{0}"}),
        ),
        (
            "ui_click",
            json!({"view_id":uuid::Uuid::new_v4(),"element_id":"element-1","script":"alert(1)"}),
        ),
    ] {
        args["session_id"] = json!("session");
        assert!(
            catalog::validate(
                entries
                    .iter()
                    .find(|entry| entry.tool.name == name)
                    .unwrap(),
                args.as_object().unwrap()
            )
            .is_err(),
            "{name}"
        );
    }
}

#[tokio::test]
async fn workflow_permissions_and_ui_control_revalidation() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(false, false, backend.clone()).await;
    let client = connect(&server, false, "Workflow owner").await;
    let task = client.clone();
    let start =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| server.snapshot().connection.is_some()).await;
    server.approve(&server.snapshot().connection.unwrap().id);
    let control = control_id(&start.await.unwrap());
    assert!(server.control_is_active(&control, false));
    assert!(!server.control_is_active(&control, true));
    for (name, mut args) in samples() {
        let entry = catalog::entries()
            .into_iter()
            .find(|entry| entry.tool.name == name)
            .unwrap();
        if entry.access != catalog::Access::FullAccess {
            continue;
        }
        args["session_id"] = json!(control);
        assert_eq!(
            tool_error(&client.tool(name, args).await),
            "full_access_required",
            "{name}"
        );
    }
    assert!(backend.calls.lock().unwrap().is_empty());
    client
        .tool("ui_get_context", json!({"session_id":control}))
        .await;
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    client
        .tool("end_session", json!({"session_id":control}))
        .await;
    assert!(!server.control_is_active(&control, false));
    assert!(!server.control_is_active("forged", false));
    server.stop().await;
}

#[tokio::test]
async fn workflow_operations_and_capabilities_agree() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Workflow dispatch").await;
    let control = control_id(&client.tool("start_session", json!({})).await);
    assert!(server.control_is_active(&control, true));
    let capabilities =
        client.tool("get_launcher_capabilities", json!({})).await;
    for (name, mut args) in samples() {
        let meta = capabilities["result"]["structuredContent"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert_eq!(meta["returns_operation_id"], !name.starts_with("ui_"));
        if name.starts_with("ui_") {
            continue;
        }
        args["session_id"] = json!(control);
        let response = client.tool(name, args).await;
        let operation = response["result"]["structuredContent"]["operation_id"]
            .as_str()
            .expect("Deferred operation ID");
        operation_tests::poll(&client, &control, operation, "succeeded").await;
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 15);
    server.stop().await;
}
