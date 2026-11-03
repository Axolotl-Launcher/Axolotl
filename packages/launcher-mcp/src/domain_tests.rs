use super::*;
fn is_new(name: &str) -> bool {
    [
        "core_",
        "multiplayer_",
        "screenshot_",
        "skin_",
        "cape_",
        "file_",
        "archive_",
        "nbt_",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn sample(rule: &Value) -> Value {
    if let Some(v) = rule["enum"].as_array() {
        return v[0].clone();
    }
    match rule["type"].as_str().unwrap() {
        "string" => match rule["format"].as_str() {
            Some("uuid") => json!(uuid::Uuid::new_v4()),
            Some("image-filename") => json!("game.png"),
            Some("sha256") => json!("a".repeat(64)),
            _ => json!("test"),
        },
        "boolean" => json!(true),
        "integer" => rule.get("minimum").cloned().unwrap_or(json!(1)),
        "array" => json!([sample(&rule["items"])]),
        _ => panic!("unsupported schema"),
    }
}
fn arguments(entry: &catalog::Entry) -> Value {
    let mut args = json!({});
    let schema = &entry.tool.input_schema;
    for field in schema["required"].as_array().unwrap() {
        let key = field.as_str().unwrap();
        args[key] = sample(&schema["properties"][key]);
    }
    args
}

#[tokio::test]
async fn multiplayer_stop_interrupts_deferred_host_before_mutation_lock_releases()
 {
    let backend = Arc::new(TestBackend::default());
    backend.block.store(true, Ordering::SeqCst);
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Stop pending host").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let started=client.tool("multiplayer_host",json!({"session_id":session,"provider":"terracotta","player_name":"Player"})).await;
    let host = started["result"]["structuredContent"]["operation_id"]
        .as_str()
        .unwrap();
    operation_tests::poll(&client, &session, host, "running").await;
    backend.block.store(false, Ordering::SeqCst);
    let stopped = client
        .tool("multiplayer_stop", json!({"session_id":session}))
        .await;
    let stop = stopped["result"]["structuredContent"]["operation_id"]
        .as_str()
        .unwrap();
    operation_tests::poll(&client, &session, stop, "succeeded").await;
    assert_eq!(backend.calls.lock().unwrap().len(), 2);
    backend.resume.notify_one();
    operation_tests::poll(&client, &session, host, "succeeded").await;
    server.stop().await;
}
#[test]
fn domain_catalogs_match_typed_dispatch_and_permission_boundaries() {
    let entries = catalog::entries();
    let mut count = 0;
    for entry in entries.iter().filter(|e| is_new(&e.tool.name)) {
        count += 1;
        let mut args = arguments(entry);
        catalog::validate(entry, args.as_object().unwrap()).unwrap();
        args.as_object_mut().unwrap().remove("session_id");
        let _: Command = serde_json::from_value(
            json!({"name":entry.tool.name,"arguments":args}),
        )
        .unwrap();
        if [
            "core_remove_component",
            "file_write",
            "file_trash",
            "nbt_patch",
            "screenshot_delete",
            "cape_equip",
            "multiplayer_host",
        ]
        .contains(&entry.tool.name.as_ref())
        {
            assert!(entry.access == catalog::Access::FullAccess);
        }
    }
    assert_eq!(count, 61);
    let list = entries.iter().find(|e| e.tool.name == "file_list").unwrap();
    let mut args = arguments(list);
    args["directory"] = json!("");
    catalog::validate(list, args.as_object().unwrap()).unwrap();
    for path in ["../outside", "C:/file", "file:stream", "a\\b", "a//b"] {
        args["directory"] = json!(path);
        assert!(catalog::validate(list, args.as_object().unwrap()).is_err());
    }
    let png = entries
        .iter()
        .find(|e| e.tool.name == "screenshot_get")
        .unwrap();
    let mut args = arguments(png);
    args["file_name"] = json!("not-a-log.png");
    catalog::validate(png, args.as_object().unwrap()).unwrap();
    args["file_name"] = json!("../secret.png");
    assert!(catalog::validate(png, args.as_object().unwrap()).is_err());
    let write = entries
        .iter()
        .find(|e| e.tool.name == "file_write")
        .unwrap();
    let mut args = arguments(write);
    args["content"] = json!("");
    catalog::validate(write, args.as_object().unwrap()).unwrap();
    args["content"] = json!("line one\r\nline two\ttext");
    catalog::validate(write, args.as_object().unwrap()).unwrap();
    args["expected_sha256"] = json!("short");
    assert!(catalog::validate(write, args.as_object().unwrap()).is_err());
}

#[tokio::test]
async fn ordinary_session_cannot_call_domain_destructive_or_content_read_tools()
{
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(false, false, backend.clone()).await;
    let client = connect(&server, false, "Domain permission").await;
    let task = client.clone();
    let pending =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| server.snapshot().connection.is_some()).await;
    server.approve(&server.snapshot().connection.unwrap().id);
    let session = control_id(&pending.await.unwrap());
    for entry in catalog::entries().into_iter().filter(|e| {
        is_new(&e.tool.name) && e.access == catalog::Access::FullAccess
    }) {
        let mut args = arguments(&entry);
        args["session_id"] = json!(session);
        assert_eq!(
            tool_error(&client.tool(&entry.tool.name, args).await),
            "full_access_required",
            "{}",
            entry.tool.name
        );
    }
    assert!(backend.calls.lock().unwrap().is_empty());
    server.stop().await;
}

#[tokio::test]
async fn domain_dispatch_deferred_metadata_and_large_payloads_are_consistent() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Domain dispatch").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let response = client.tool("get_launcher_capabilities", json!({})).await;
    for entry in catalog::entries()
        .into_iter()
        .filter(|e| is_new(&e.tool.name))
    {
        let mut args = arguments(&entry);
        args["session_id"] = json!(session);
        let mut typed = args.clone();
        typed.as_object_mut().unwrap().remove("session_id");
        let _command: Command = serde_json::from_value(
            json!({"name":entry.tool.name,"arguments":typed}),
        )
        .unwrap();
        let tool = response["result"]["structuredContent"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == entry.tool.name.as_ref())
            .unwrap();
        assert_eq!(tool["returns_operation_id"], entry.deferred);
        if entry.tool.name == "screenshot_get" {
            continue;
        }
        if entry.tool.name == "file_write" {
            args["content"] = json!("x".repeat(70000));
        }
        let result = client.tool(&entry.tool.name, args).await;
        if entry.deferred {
            let id = result["result"]["structuredContent"]["operation_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{}: {result}", entry.tool.name));
            operation_tests::poll(&client, &session, id, "succeeded").await;
        } else {
            assert_eq!(
                result["result"]["isError"], false,
                "{}: {result}",
                entry.tool.name
            );
        }
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 60);
    server.stop().await;
}
