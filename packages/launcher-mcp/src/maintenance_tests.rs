use super::*;
fn is_new(name: &str) -> bool {
    [
        "schematic_",
        "seed_",
        "settings_",
        "privacy_",
        "proxy_",
        "network_",
        "storage_",
        "database_",
        "launcher_",
        "shortcut_",
        "system_open_",
        "news_",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn sample(rule: &Value) -> Value {
    if let Some(choices) = rule["enum"].as_array() {
        return choices[0].clone();
    }
    match rule["type"].as_str().unwrap() {
        "string" => match rule["format"].as_str() {
            Some("uuid") => json!(uuid::Uuid::new_v4()),
            Some("sha256") => json!("a".repeat(64)),
            _ => json!("test"),
        },
        "boolean" => json!(true),
        "integer" => rule.get("minimum").cloned().unwrap_or(json!(1)),
        "array" => json!(
            (0..rule["minItems"].as_u64().unwrap_or(1))
                .map(|_| sample(&rule["items"]))
                .collect::<Vec<_>>()
        ),
        _ => panic!("schema"),
    }
}
fn args(entry: &catalog::Entry) -> Value {
    let mut value = json!({});
    for field in entry.tool.input_schema["required"].as_array().unwrap() {
        let key = field.as_str().unwrap();
        value[key] = sample(&entry.tool.input_schema["properties"][key]);
    }
    value
}
#[test]
fn maintenance_schemas_keep_signed_coordinates_bounded_and_sensitive_actions_full_access()
 {
    let entries = catalog::entries();
    let mut count = 0;
    for e in entries.iter().filter(|e| is_new(&e.tool.name)) {
        count += 1;
        let mut arguments = args(e);
        catalog::validate(e, arguments.as_object().unwrap()).unwrap();
        arguments.as_object_mut().unwrap().remove("session_id");
        let _: Command = serde_json::from_value(
            json!({"name":e.tool.name,"arguments":arguments}),
        )
        .unwrap();
    }
    assert_eq!(count, 53);
    let biome = entries
        .iter()
        .find(|e| e.tool.name == "seed_get_biome")
        .unwrap();
    let mut arguments = args(biome);
    arguments["x"] = json!(-128);
    arguments["y"] = json!(-64);
    arguments["z"] = json!(-32);
    catalog::validate(biome, arguments.as_object().unwrap()).unwrap();
    arguments["x"] = json!(-30000001);
    assert!(catalog::validate(biome, arguments.as_object().unwrap()).is_err());
    let get = entries
        .iter()
        .find(|e| e.tool.name == "instance_list")
        .unwrap();
    assert!(
        catalog::validate(
            get,
            json!({"session_id":"s","offset":-1}).as_object().unwrap()
        )
        .is_err()
    );
    for name in [
        "database_copy_channel",
        "database_backup",
        "storage_clear_cache",
        "settings_patch",
        "proxy_set",
        "launcher_download_update",
        "launcher_restart",
        "launcher_exit",
        "system_open_url",
        "shortcut_create_instance",
    ] {
        assert!(
            entries.iter().find(|e| e.tool.name == name).unwrap().access
                == catalog::Access::FullAccess
        );
    }
}
#[tokio::test]
async fn ordinary_sessions_cannot_modify_settings_databases_updater_or_system()
{
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(false, false, backend.clone()).await;
    let client = connect(&server, false, "Maintenance permission").await;
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
        let mut arguments = args(&entry);
        arguments["session_id"] = json!(session);
        assert_eq!(
            tool_error(&client.tool(&entry.tool.name, arguments).await),
            "full_access_required",
            "{}",
            entry.tool.name
        );
    }
    assert!(backend.calls.lock().unwrap().is_empty());
    server.stop().await;
}
#[tokio::test]
async fn maintenance_tools_dispatch_and_async_capabilities_agree_without_real_mutations()
 {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Maintenance dispatch").await;
    let session = control_id(&client.tool("start_session", json!({})).await);
    let capabilities =
        client.tool("get_launcher_capabilities", json!({})).await;
    for entry in catalog::entries()
        .into_iter()
        .filter(|e| is_new(&e.tool.name))
    {
        let mut arguments = args(&entry);
        arguments["session_id"] = json!(session);
        let mut typed = arguments.clone();
        typed.as_object_mut().unwrap().remove("session_id");
        let _command: Command = serde_json::from_value(
            json!({"name":entry.tool.name,"arguments":typed}),
        )
        .unwrap();
        let info = capabilities["result"]["structuredContent"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == entry.tool.name.as_ref())
            .unwrap();
        assert_eq!(
            info["returns_operation_id"], entry.deferred,
            "{}",
            entry.tool.name
        );
        let result = client.tool(&entry.tool.name, arguments).await;
        if entry.deferred {
            let id = result["result"]["structuredContent"]["operation_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{} {result}", entry.tool.name));
            operation_tests::poll(&client, &session, id, "succeeded").await;
        } else {
            assert_eq!(
                result["result"]["isError"], false,
                "{} {result}",
                entry.tool.name
            );
        }
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 53);
    server.stop().await;
}
