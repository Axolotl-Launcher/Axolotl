use super::*;
use axum::http::StatusCode;
use futures::StreamExt;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

#[path = "backup_tests.rs"]
mod backup_tests;
#[path = "domain_tests.rs"]
mod domain_tests;
#[path = "maintenance_tests.rs"]
mod maintenance_tests;
#[path = "operation_tests.rs"]
mod operation_tests;
#[path = "runtime_tests.rs"]
mod runtime_tests;
#[path = "workflow_tests.rs"]
mod workflow_tests;

#[derive(Clone)]
struct Client {
    http: reqwest::Client,
    endpoint: String,
    wire: String,
    auth: bool,
    ids: Arc<AtomicU32>,
}

impl Client {
    fn post(&self) -> reqwest::RequestBuilder {
        let request = self
            .http
            .post(&self.endpoint)
            .header("Mcp-Session-Id", &self.wire)
            .header("Accept", "application/json, text/event-stream");
        if self.auth {
            request.bearer_auth("test-token")
        } else {
            request
        }
    }

    fn message(&self, method: &str, params: Value) -> Value {
        json!({"jsonrpc":"2.0","id":self.ids.fetch_add(1, Ordering::SeqCst),"method":method,"params":params})
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        decode(
            self.post()
                .json(&self.message(method, params))
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    async fn tool(&self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({"name":name,"arguments":arguments}))
            .await
    }
}

async fn server(full: bool, auth: bool) -> Server {
    server_with_backend(full, auth, Arc::new(TestBackend::default())).await
}

#[derive(Default)]
struct TestBackend {
    scopes: std::sync::Mutex<Vec<String>>,
    calls: std::sync::Mutex<Vec<Command>>,
    block: std::sync::atomic::AtomicBool,
    fail: std::sync::atomic::AtomicBool,
    finished: AtomicU32,
    resume: tokio::sync::Notify,
}

impl Backend for TestBackend {
    fn execute(
        &self,
        command: Command,
        context: BackendContext,
    ) -> futures::future::BoxFuture<'_, Result<Value, ToolError>> {
        Box::pin(async move {
            self.scopes.lock().unwrap().push(context.session_id);
            self.calls.lock().unwrap().push(command);
            if self.block.load(Ordering::SeqCst) {
                self.resume.notified().await;
            }
            self.finished.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(ToolError::new("not_found", "Instance not found"));
            }
            Ok(json!({"executed":true}))
        })
    }
}

async fn server_with_backend(
    full: bool,
    auth: bool,
    backend: Arc<dyn Backend>,
) -> Server {
    Server::start(
        0,
        if auth {
            "test-token".into()
        } else {
            String::new()
        },
        full,
        auth,
        LauncherInfo::new("9.8.7-test"),
        backend,
        |_| {},
    )
    .await
    .unwrap()
}

async fn connect(server: &Server, auth: bool, name: &str) -> Client {
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let endpoint = server.snapshot().endpoint.unwrap();
    let request = http
        .post(&endpoint)
        .header("Accept", "application/json, text/event-stream");
    let request = if auth {
        request.bearer_auth("test-token")
    } else {
        request
    };
    let response = request.json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":name,"version":"1"}
    }})).send().await.unwrap();
    assert!(response.status().is_success());
    let wire = response.headers()["mcp-session-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let body = decode(response).await;
    assert!(body["result"]["capabilities"].get("tools").is_some());
    assert_eq!(body["result"]["serverInfo"]["version"], "9.8.7-test");
    let client = Client {
        http,
        endpoint,
        wire,
        auth,
        ids: Arc::new(AtomicU32::new(2)),
    };
    let response = client
        .post()
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    wait_for(|| {
        server
            .shared
            .inner
            .lock()
            .unwrap()
            .slots
            .values()
            .any(|slot| {
                slot.wire_id.as_deref() == Some(&client.wire)
                    && slot.initialized
            })
    })
    .await;
    client
}

async fn decode(response: reqwest::Response) -> Value {
    let text = response.text().await.unwrap();
    if let Ok(value) = serde_json::from_str(&text) {
        return value;
    }
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .find_map(|data| serde_json::from_str(data.trim()).ok())
        .unwrap_or_else(|| panic!("No JSON-RPC payload: {text}"))
}

async fn wait_for(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

fn control_id(response: &Value) -> String {
    assert_eq!(response["result"]["isError"], false, "{response}");
    response["result"]["structuredContent"]["session_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn discovery_and_metadata_are_available_without_permission_or_token() {
    let server = server(false, false).await;
    let client = connect(&server, false, "Codex discovery").await;
    assert!(server.snapshot().connection.is_none());
    for (header, value) in
        [("Origin", "https://evil.example"), ("Host", "evil.example")]
    {
        let response = client.http.post(&client.endpoint).header(header, value)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let listed = client.rpc("tools/list", json!({})).await;
    let names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        crate::catalog::definitions()
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>()
    );
    let capabilities =
        client.tool("get_launcher_capabilities", json!({})).await;
    assert_eq!(
        capabilities["result"]["structuredContent"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        names.len()
    );
    let info = client.tool("get_launcher_info", json!({})).await;
    assert_eq!(info["result"]["structuredContent"]["version"], "9.8.7-test");
    assert_eq!(
        info["result"]["structuredContent"]["name"],
        "Axolotl Launcher"
    );
    assert!(server.snapshot().connection.is_none());
    let unknown = client.tool("unknown", json!({})).await;
    assert!(unknown.get("error").is_some());
    server.stop().await;
}

#[tokio::test]
async fn start_session_requires_approval_and_end_session_is_scoped_and_repeatable()
 {
    let server = server(false, false).await;
    let first = connect(&server, false, "First client").await;
    let second = connect(&server, false, "Second client").await;
    let task = first.clone();
    let pending =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| {
        server
            .snapshot()
            .connection
            .as_ref()
            .is_some_and(|c| c.awaiting_approval)
    })
    .await;
    assert!(!pending.is_finished());
    let connection = server.snapshot().connection.unwrap();
    assert_eq!(connection.client, "First client");
    assert!(!server.approve("stale-id"));
    let busy = second.tool("start_session", json!({})).await;
    assert_eq!(busy["result"]["isError"], true);
    server.approve(&connection.id);
    let response = pending.await.unwrap();
    let control = control_id(&response);
    assert_eq!(
        response["result"]["structuredContent"]["full_access"],
        false
    );
    assert!(server.snapshot().connection.unwrap().connected);
    assert_eq!(
        control_id(&first.tool("start_session", json!({})).await),
        control
    );
    let foreign = second
        .tool("end_session", json!({"session_id":control}))
        .await;
    assert_eq!(foreign["result"]["isError"], true);
    assert!(server.snapshot().connection.is_some());
    for _ in 0..2 {
        let ended = first
            .tool("end_session", json!({"session_id":control}))
            .await;
        assert_eq!(ended["result"]["structuredContent"]["ended"], true);
    }
    assert!(server.snapshot().connection.is_none());
    let task = first.clone();
    let pending =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| {
        server
            .snapshot()
            .connection
            .as_ref()
            .is_some_and(|c| c.awaiting_approval)
    })
    .await;
    let stale = first
        .tool("end_session", json!({"session_id":control}))
        .await;
    assert_eq!(stale["result"]["isError"], true);
    server.deny(&connection.id);
    assert_eq!(pending.await.unwrap()["result"]["isError"], true);
    assert!(server.snapshot().connection.is_none());
    assert!(
        first
            .rpc("tools/list", json!({}))
            .await
            .get("result")
            .is_some()
    );
    server.stop().await;
}

#[tokio::test]
async fn full_access_starts_only_on_tool_use_and_forced_disconnect_revokes_transport()
 {
    let server = server(true, true).await;
    let client = connect(&server, true, "Full client").await;
    assert!(server.snapshot().connection.is_none());
    let response = client.tool("start_session", json!({})).await;
    control_id(&response);
    assert_eq!(response["result"]["structuredContent"]["full_access"], true);
    let connection = server.snapshot().connection.unwrap();
    assert!(connection.connected);
    server.disconnect(&connection.id).await;
    assert!(server.snapshot().connection.is_none());
    assert_eq!(
        client
            .post()
            .json(&client.message("ping", json!({})))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    server.stop().await;
}

#[tokio::test]
async fn bearer_auth_and_origin_validation_remain_independent_of_control_permission()
 {
    let server = server(true, true).await;
    let endpoint = server.snapshot().endpoint.unwrap();
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    assert_eq!(
        http.post(&endpoint).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(&endpoint)
            .bearer_auth("wrong")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let client = connect(&server, true, "Authenticated").await;
    client.tool("start_session", json!({})).await;
    let response = client
        .http
        .delete(&endpoint)
        .bearer_auth("test-token")
        .header("Mcp-Session-Id", &client.wire)
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(server.snapshot().connection.is_some());
    let response = client
        .http
        .delete(&endpoint)
        .bearer_auth("test-token")
        .header("Mcp-Session-Id", &client.wire)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert!(server.snapshot().connection.is_none());
    server.stop().await;
}

#[tokio::test]
async fn lost_control_client_is_reaped() {
    let server = server(true, false).await;
    let client = connect(&server, false, "Lost client").await;
    client.tool("start_session", json!({})).await;
    tokio::time::timeout(Duration::from_secs(30), async {
        while server.snapshot().connection.is_some() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert!(server.shared.inner.lock().unwrap().slots.is_empty());
    server.stop().await;
}

#[tokio::test]
async fn healthy_idle_client_remains_connected_without_requesting_control() {
    let server = server(false, false).await;
    let client = connect(&server, false, "Idle client").await;
    let response = client
        .http
        .get(&client.endpoint)
        .header("Mcp-Session-Id", &client.wire)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let (sent, received) = tokio::sync::oneshot::channel();
    let responder = tokio::spawn(async move {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut sent = Some(sent);
        while let Some(Ok(chunk)) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find('\n') {
                let line = buffer[..end].to_string();
                buffer.drain(..=end);
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let Ok(message) = serde_json::from_str::<Value>(data.trim())
                else {
                    continue;
                };
                if message["method"] == "ping" {
                    let response = client.post().json(&json!({"jsonrpc":"2.0","id":message["id"],"result":{}})).send().await.unwrap();
                    assert_eq!(response.status(), StatusCode::ACCEPTED);
                    if let Some(sent) = sent.take() {
                        let _ = sent.send(());
                    }
                }
            }
        }
    });
    tokio::time::timeout(Duration::from_secs(20), received)
        .await
        .unwrap()
        .unwrap();
    assert!(server.snapshot().connection.is_none());
    assert_eq!(server.shared.inner.lock().unwrap().slots.len(), 1);
    responder.abort();
    server.stop().await;
}

#[tokio::test]
async fn shutdown_cancels_pending_control_requests() {
    let server = server(false, false).await;
    let client = connect(&server, false, "Pending client").await;
    let message = client
        .message("tools/call", json!({"name":"start_session","arguments":{}}));
    let pending =
        tokio::spawn(async move { client.post().json(&message).send().await });
    wait_for(|| server.snapshot().connection.is_some()).await;
    server.stop().await;
    assert!(
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn cancelling_a_start_request_clears_the_prompt_without_closing_transport()
 {
    let server = server(false, false).await;
    let client = connect(&server, false, "Cancelled client").await;
    let message = client
        .message("tools/call", json!({"name":"start_session","arguments":{}}));
    let request_id = message["id"].clone();
    let task = client.clone();
    let pending = tokio::spawn(async move {
        if let Ok(response) = task.post().json(&message).send().await {
            let _ = response.bytes().await;
        }
    });
    wait_for(|| server.snapshot().connection.is_some()).await;
    let cancelled = client.post().json(&json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":request_id,"reason":"Task stopped"}})).send().await.unwrap();
    assert_eq!(cancelled.status(), StatusCode::ACCEPTED);
    wait_for(|| server.snapshot().connection.is_none()).await;
    tokio::time::timeout(Duration::from_secs(3), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(
        client
            .tool("get_launcher_info", json!({}))
            .await
            .get("result")
            .is_some()
    );
    server.stop().await;
}

fn tool_error(response: &Value) -> &str {
    assert_eq!(response["result"]["isError"], true, "{response}");
    response["result"]["structuredContent"]["error"]["code"]
        .as_str()
        .unwrap()
}

#[tokio::test]
async fn business_tools_require_approved_transport_scoped_sessions() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(false, false, backend.clone()).await;
    let client = connect(&server, false, "Owner").await;
    let other = connect(&server, false, "Other").await;
    assert_eq!(
        tool_error(
            &client
                .tool("instance_list", json!({"session_id":"invented"}))
                .await
        ),
        "session_required"
    );
    assert!(server.snapshot().connection.is_none());
    let task = client.clone();
    let pending =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| server.snapshot().connection.is_some()).await;
    let (transport, control) = {
        let inner = server.shared.inner.lock().unwrap();
        let transport = inner.active.as_ref().unwrap().clone();
        let control = inner.slots[&transport].control_id.clone().unwrap();
        (transport, control)
    };
    assert_eq!(
        tool_error(
            &client
                .tool("instance_list", json!({"session_id":control}))
                .await
        ),
        "session_required"
    );
    assert!(backend.calls.lock().unwrap().is_empty());
    server.approve(&transport);
    assert_eq!(control_id(&pending.await.unwrap()), control);
    assert_eq!(
        tool_error(
            &other
                .tool("instance_list", json!({"session_id":control}))
                .await
        ),
        "session_required"
    );
    assert_eq!(
        client
            .tool("instance_list", json!({"session_id":control}))
            .await["result"]["isError"],
        false
    );
    assert!(matches!(
        &backend.calls.lock().unwrap()[0],
        Command::InstanceList {
            offset: 0,
            limit: 50,
            search: None
        }
    ));
    assert_eq!(
        tool_error(
            &client
                .tool(
                    "instance_group_delete",
                    json!({"session_id":control,"group_id":"group"})
                )
                .await
        ),
        "full_access_required"
    );
    assert_eq!(tool_error(&client.tool("content_remove", json!({"session_id":control,"instance_id":"instance","entry_id":"entry"})).await), "full_access_required");
    for (name, args) in [
        (
            "resource_favorite_remove",
            json!({"session_id":control,"source":"modrinth","project_id":"project"}),
        ),
        (
            "resource_history_rollback",
            json!({"session_id":control,"instance_id":"instance","entry_id":"entry","history_id":"history"}),
        ),
        (
            "resource_execute_history_cleanup",
            json!({"session_id":control,"plan_id":uuid::Uuid::new_v4(),"revision":1}),
        ),
        (
            "instance_delete",
            json!({"session_id":control,"preview_id":uuid::Uuid::new_v4()}),
        ),
        (
            "server_install_modpack",
            json!({"session_id":control,"server_id":"server-123","project_id":"pack","release_id":"release"}),
        ),
        (
            "server_delete",
            json!({"session_id":control,"server_id":"server-123"}),
        ),
        (
            "server_console_send",
            json!({"session_id":control,"server_id":"server-123","command":"say test"}),
        ),
        (
            "account_remove",
            json!({"session_id":control,"account_id":format!("account:{}","a".repeat(64))}),
        ),
        (
            "download_job_delete",
            json!({"session_id":control,"job_id":uuid::Uuid::new_v4()}),
        ),
        ("download_history_clear", json!({"session_id":control})),
        (
            "install_job_repair_and_retry",
            json!({"session_id":control,"job_id":uuid::Uuid::new_v4()}),
        ),
        (
            "java_remove_default",
            json!({"session_id":control,"major_version":21}),
        ),
        (
            "java_remove_registration",
            json!({"session_id":control,"registered_path":"C:\\Java\\bin\\javaw.exe"}),
        ),
        (
            "world_delete",
            json!({"session_id":control,"instance_id":"instance","world_id":"save"}),
        ),
        (
            "server_entry_remove",
            json!({"session_id":control,"instance_id":"instance","index":0,"expected_name":"Server","expected_address":"localhost","expected_pack_status":"prompt"}),
        ),
        (
            "backup_restore",
            json!({"session_id":control,"preview_id":uuid::Uuid::new_v4()}),
        ),
        (
            "backup_delete_snapshot",
            json!({"session_id":control,"instance_id":"instance","snapshot_id":"snapshot"}),
        ),
    ] {
        assert_eq!(
            tool_error(&client.tool(name, args).await),
            "full_access_required"
        );
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    assert_eq!(client.tool("instance_rename", json!({"session_id":control,"instance_id":"instance","name":"Name"})).await["result"]["isError"], false);
    assert_eq!(backend.calls.lock().unwrap().len(), 2);
    client
        .tool("end_session", json!({"session_id":control}))
        .await;
    assert_eq!(tool_error(&client.tool("instance_rename", json!({"session_id":control,"instance_id":"instance","name":"Name"})).await), "session_required");
    // A later session on the same transport cannot reuse the old capability.
    let task = client.clone();
    let pending =
        tokio::spawn(
            async move { task.tool("start_session", json!({})).await },
        );
    wait_for(|| server.snapshot().connection.is_some()).await;
    server.approve(&transport);
    let next = control_id(&pending.await.unwrap());
    assert_ne!(next, control);
    assert_eq!(
        tool_error(
            &client
                .tool("instance_list", json!({"session_id":control}))
                .await
        ),
        "session_required"
    );
    server.disconnect(&transport).await;
    assert!(server.shared.authorize(&transport, &next, false).is_err());
    server.stop().await;
}

#[tokio::test]
async fn full_access_allows_deletion_and_all_business_commands_are_dispatched()
{
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Full control").await;
    let control = control_id(&client.tool("start_session", json!({})).await);
    let cases = [
        (
            "instance_list",
            json!({"offset":2,"limit":10,"search":"测试"}),
        ),
        ("instance_get", json!({"instance_id":"instance"})),
        (
            "instance_create",
            json!({"name":"Test","game_version":"1.21.1","loader":"fabric","loader_version":"0.16.0"}),
        ),
        (
            "instance_rename",
            json!({"instance_id":"instance","name":"New"}),
        ),
        (
            "instance_set_pinned",
            json!({"instance_id":"instance","pinned":true}),
        ),
        ("instance_group_list", json!({})),
        ("instance_group_create", json!({"name":"Group"})),
        (
            "instance_group_rename",
            json!({"group_id":"group","name":"Group 2"}),
        ),
        ("instance_group_delete", json!({"group_id":"group"})),
        (
            "instance_set_groups",
            json!({"instance_id":"instance","group_ids":["group"]}),
        ),
        ("process_list", json!({"instance_id":"instance"})),
        ("java_list", json!({})),
        ("system_get_memory", json!({})),
        ("install_job_list", json!({"include_finished":true})),
        ("install_job_get", json!({"job_id":uuid::Uuid::new_v4()})),
        ("process_get", json!({"process_id":uuid::Uuid::new_v4()})),
        (
            "process_wait",
            json!({"process_id":uuid::Uuid::new_v4(),"timeout_seconds":2}),
        ),
        ("process_kill", json!({"process_id":uuid::Uuid::new_v4()})),
        ("logs_list", json!({"instance_id":"instance"})),
        ("resource_list_sources", json!({})),
        (
            "resource_search",
            json!({"source":"modrinth","query":"Sodium","resource_type":"mod","instance_id":"instance","limit":10}),
        ),
        (
            "resource_get",
            json!({"source":"curseforge","project_id":"123"}),
        ),
        (
            "resource_list_releases",
            json!({"source":"modrinth","project_id":"project","instance_id":"instance"}),
        ),
        (
            "resource_get_release",
            json!({"source":"mcarchive","project_id":"project","release_id":"release"}),
        ),
        ("content_list", json!({"instance_id":"instance"})),
        ("resource_favorites_list", json!({"offset":0,"limit":10})),
        (
            "resource_history_list",
            json!({"instance_id":"instance","entry_id":"entry","limit":10}),
        ),
        (
            "instance_check_installation",
            json!({"instance_id":"instance"}),
        ),
        ("instance_delete_preview", json!({"instance_id":"instance"})),
        ("server_list", json!({"limit":10})),
        ("server_get", json!({"server_id":"server-123"})),
        (
            "server_update_settings",
            json!({"server_id":"server-123","name":"New","memory_mib":4096,"pinned":false}),
        ),
        (
            "server_wait_ready",
            json!({"server_id":"server-123","timeout_seconds":2}),
        ),
        ("server_stop", json!({"server_id":"server-123"})),
        ("server_kill", json!({"server_id":"server-123"})),
        (
            "server_logs_read",
            json!({"server_id":"server-123","max_bytes":1024}),
        ),
        (
            "server_logs_search",
            json!({"server_id":"server-123","query":"error"}),
        ),
        ("account_list", json!({"offline_only":true,"limit":10})),
        (
            "account_get",
            json!({"account_id":format!("account:{}","a".repeat(64))}),
        ),
        ("account_get_default", json!({"offline_mode":true})),
        ("account_get_eligibility", json!({})),
        (
            "download_job_list",
            json!({"status":"failed","provider":"java","query":"测试","limit":10}),
        ),
        ("download_job_get", json!({"job_id":uuid::Uuid::new_v4()})),
        (
            "download_job_get_items",
            json!({"job_id":uuid::Uuid::new_v4(),"offset":1,"limit":10}),
        ),
        (
            "install_job_missing_files",
            json!({"job_id":uuid::Uuid::new_v4(),"limit":10}),
        ),
        ("java_list_vendors", json!({})),
        (
            "java_list_versions",
            json!({"vendor":"Vendor Name","limit":10}),
        ),
        ("java_remove_default", json!({"major_version":21})),
        (
            "java_remove_registration",
            json!({"registered_path":"C:\\Java\\bin\\javaw.exe"}),
        ),
        ("world_list", json!({"instance_id":"instance"})),
        (
            "world_get",
            json!({"instance_id":"instance","world_id":"My World"}),
        ),
        (
            "world_rename",
            json!({"instance_id":"instance","world_id":"save","name":"World"}),
        ),
        (
            "world_set_display_status",
            json!({"instance_id":"instance","world_id":"save","status":"favorite"}),
        ),
        (
            "world_update_settings",
            json!({"instance_id":"instance","world_id":"save","game_mode":"creative","difficulty":"hard","allow_commands":false}),
        ),
        ("server_entry_list", json!({"instance_id":"instance"})),
        (
            "server_entry_add",
            json!({"instance_id":"instance","name":"Server","address":"[::1]:25565","pack_status":"prompt"}),
        ),
        (
            "server_entry_update",
            json!({"instance_id":"instance","index":0,"expected_name":"Server","expected_address":"localhost","expected_pack_status":"prompt","name":"New","address":"mc.example.com","pack_status":"disabled"}),
        ),
        (
            "server_entry_remove",
            json!({"instance_id":"instance","index":0,"expected_name":"Server","expected_address":"localhost","expected_pack_status":"prompt"}),
        ),
        (
            "server_ping",
            json!({"instance_id":"instance","address":"localhost:25565"}),
        ),
        (
            "instance_get_protocol_version",
            json!({"instance_id":"instance"}),
        ),
        (
            "metadata_list_game_versions",
            json!({"release_type":"release","search":"1.21","offset":0,"limit":10}),
        ),
        (
            "metadata_list_loader_versions",
            json!({"loader":"neoforge","game_version":"1.21.1","stable_only":true}),
        ),
        ("launch_config_get", json!({"instance_id":"instance"})),
        (
            "launch_set_memory",
            json!({"instance_id":"instance","automatic":false,"maximum_mib":4096,"optimize_before_launch":false}),
        ),
        (
            "launch_set_window_options",
            json!({"instance_id":"instance","fullscreen":false,"maximized":false,"width":1280,"height":720}),
        ),
        (
            "launch_set_java",
            json!({"instance_id":"instance","registered_path":"C:\\Java\\bin\\javaw.exe"}),
        ),
        (
            "launch_reset_overrides",
            json!({"instance_id":"instance","fields":["memory","window","java"]}),
        ),
        (
            "launch_get_memory_status",
            json!({"instance_id":"instance"}),
        ),
        ("backup_repository_status", json!({})),
        ("backup_get_config", json!({"instance_id":"instance"})),
        (
            "backup_list_snapshots",
            json!({"instance_id":"instance","limit":10}),
        ),
        (
            "backup_list_operations",
            json!({"instance_id":"instance","active_only":true}),
        ),
        (
            "backup_get_operation",
            json!({"instance_id":"instance","backup_operation_id":uuid::Uuid::new_v4()}),
        ),
        (
            "backup_cancel",
            json!({"instance_id":"instance","backup_operation_id":uuid::Uuid::new_v4()}),
        ),
        (
            "backup_get_restore_preview",
            json!({"preview_id":uuid::Uuid::new_v4()}),
        ),
        (
            "content_set_enabled",
            json!({"instance_id":"instance","entry_id":"entry","enabled":false}),
        ),
        (
            "logs_read",
            json!({"instance_id":"instance","log_type":"info","filename":"latest.log"}),
        ),
        (
            "logs_search",
            json!({"instance_id":"instance","log_type":"crash","filename":"crash.txt","query":"error"}),
        ),
    ];
    for (name, mut arguments) in cases {
        arguments["session_id"] = json!(control);
        let result = client.tool(name, arguments).await;
        assert_eq!(
            result["result"]["structuredContent"]["executed"], true,
            "{name}: {result}"
        );
    }
    assert_eq!(backend.calls.lock().unwrap().len(), 78);
    assert!(
        backend
            .scopes
            .lock()
            .unwrap()
            .iter()
            .all(|scope| scope == &control)
    );
    for name in [
        "instance_create",
        "instance_group_create",
        "instance_group_delete",
    ] {
        let tool = crate::catalog::definitions()
            .into_iter()
            .find(|tool| tool.name == name)
            .unwrap();
        assert_eq!(
            serde_json::to_value(tool).unwrap()["annotations"]["idempotentHint"],
            false
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn invalid_arguments_are_rejected_before_backend_execution() {
    let backend = Arc::new(TestBackend::default());
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Validation").await;
    let control = control_id(&client.tool("start_session", json!({})).await);
    let cases = [
        (
            "resource_favorite_add",
            json!({"source":"modrinth","project_id":"project","resource_type":"mod_pack"}),
        ),
        (
            "resource_favorite_add",
            json!({"source":"unknown","project_id":"project","resource_type":"mod"}),
        ),
        (
            "resource_prepare_history_cleanup",
            json!({"instance_id":"instance","backup_ids":[]}),
        ),
        (
            "resource_prepare_history_cleanup",
            json!({"instance_id":"instance","backup_ids":vec!["backup";101]}),
        ),
        (
            "resource_prepare_history_cleanup",
            json!({"instance_id":"instance","backup_ids":["backup"],"path":"C:/arbitrary"}),
        ),
        (
            "resource_execute_history_cleanup",
            json!({"plan_id":"native-plan","revision":1}),
        ),
        (
            "resource_history_rollback",
            json!({"instance_id":"instance","entry_id":"entry","history_id":"history","backup_path":"C:/arbitrary"}),
        ),
        (
            "instance_delete",
            json!({"preview_id":uuid::Uuid::new_v4(),"instance_id":"replacement"}),
        ),
        ("instance_delete", json!({"preview_id":"not-a-preview"})),
        (
            "instance_delete_preview",
            json!({"instance_id":"instance","path":"C:/arbitrary"}),
        ),
        ("instance_reinstall", json!({"instance_id":"instance"})),
        (
            "instance_reinstall",
            json!({"instance_id":"instance","force":"true"}),
        ),
        (
            "instance_duplicate",
            json!({"instance_id":"instance","destination":"C:/arbitrary"}),
        ),
        (
            "server_install_forge",
            json!({"server_id":"server-123","build":"../../outside"}),
        ),
        (
            "server_install_neoforge",
            json!({"server_id":"server-123","build":"21.1.1?other"}),
        ),
        (
            "server_install_forge",
            json!({"server_id":"server-123","build":"47.1.1","url":"https://arbitrary/installer"}),
        ),
        (
            "server_install_modpack",
            json!({"server_id":"server-123","project_id":"pack","release_id":"release","mrpack_url":"https://arbitrary/pack"}),
        ),
        (
            "server_install_modpack",
            json!({"server_id":"../outside","project_id":"pack","release_id":"release"}),
        ),
        ("server_get", json!({"server_id":"../outside"})),
        ("server_get", json!({"server_id":"server:stream"})),
        (
            "server_wait_ready",
            json!({"server_id":"server-123","timeout_seconds":21}),
        ),
        (
            "server_create",
            json!({"name":"Server","server_type":"shell","game_version":"1.21"}),
        ),
        (
            "server_update_settings",
            json!({"server_id":"server-123","jvm_args":["arbitrary"]}),
        ),
        (
            "server_console_send",
            json!({"server_id":"server-123","command":"stop\nsay injected"}),
        ),
        (
            "server_console_send",
            json!({"server_id":"server-123","command":"\u{1b}[A"}),
        ),
        (
            "server_logs_read",
            json!({"server_id":"server-123","max_bytes":32769}),
        ),
        ("account_get", json!({"account_id":"native-account-id"})),
        (
            "account_set_default",
            json!({"account_id":format!("account:{}","z".repeat(64))}),
        ),
        ("account_add_offline", json!({"username":"bad name"})),
        ("account_add_offline", json!({"username":"a".repeat(17)})),
        (
            "account_add_offline",
            json!({"username":"valid","uuid":"invalid-uuid"}),
        ),
        (
            "account_add_offline",
            json!({"username":"valid","bypass_eligibility":true}),
        ),
        (
            "account_get",
            json!({"account_id":format!("account:{}","a".repeat(64)),"include_tokens":true}),
        ),
        ("download_job_list", json!({"status":"unknown"})),
        ("download_job_list", json!({"provider":"curseforge"})),
        (
            "download_job_get_items",
            json!({"job_id":uuid::Uuid::new_v4(),"limit":101}),
        ),
        ("download_job_retry", json!({"job_id":"invalid"})),
        (
            "install_job_scan_missing_files",
            json!({"job_id":uuid::Uuid::new_v4(),"directory":"C:/private"}),
        ),
        (
            "download_job_delete",
            json!({"job_id":uuid::Uuid::new_v4(),"delete_files":true}),
        ),
        ("java_install", json!({"vendor":"Vendor","major_version":0})),
        (
            "java_install",
            json!({"vendor":"Vendor","major_version":21,"url":"https://arbitrary/download"}),
        ),
        (
            "java_test",
            json!({"registered_path":"path","major_version":256}),
        ),
        (
            "java_set_default",
            json!({"registered_path":"","major_version":21}),
        ),
        (
            "java_remove_registration",
            json!({"registered_path":"path","delete_files":true}),
        ),
        (
            "world_get",
            json!({"instance_id":"id","world_id":"../outside"}),
        ),
        (
            "world_get",
            json!({"instance_id":"id","world_id":"save:stream"}),
        ),
        (
            "world_update_settings",
            json!({"instance_id":"id","world_id":"save","game_mode":"invalid"}),
        ),
        (
            "world_update_settings",
            json!({"instance_id":"id","world_id":"save","seed":"arbitrary"}),
        ),
        (
            "world_set_display_status",
            json!({"instance_id":"id","world_id":"save","status":"deleted"}),
        ),
        ("server_ping", json!({"address":"https://example.com"})),
        (
            "server_entry_add",
            json!({"instance_id":"id","name":"Server","address":"localhost:0","pack_status":"prompt"}),
        ),
        ("server_entry_remove", json!({"instance_id":"id","index":0})),
        (
            "server_entry_update",
            json!({"instance_id":"id","index":-1,"expected_name":"Server","expected_address":"localhost","expected_pack_status":"prompt","name":"Server","address":"localhost","pack_status":"prompt"}),
        ),
        ("metadata_list_game_versions", json!({"release_type":"any"})),
        (
            "metadata_list_loader_versions",
            json!({"loader":"unknown","game_version":"1.21"}),
        ),
        (
            "metadata_list_loader_versions",
            json!({"loader":"fabric","game_version":"../file"}),
        ),
        (
            "launch_set_memory",
            json!({"instance_id":"id","automatic":false,"maximum_mib":0,"optimize_before_launch":false}),
        ),
        (
            "launch_set_memory",
            json!({"instance_id":"id","automatic":true,"maximum_mib":1.5,"optimize_before_launch":false}),
        ),
        (
            "launch_set_window_options",
            json!({"instance_id":"id","fullscreen":false,"maximized":true,"width":16385,"height":720}),
        ),
        (
            "launch_reset_overrides",
            json!({"instance_id":"id","fields":[]}),
        ),
        (
            "launch_reset_overrides",
            json!({"instance_id":"id","fields":["hooks"]}),
        ),
        (
            "launch_reset_overrides",
            json!({"instance_id":"id","fields":["memory","java","window","memory"]}),
        ),
        (
            "launch_set_java",
            json!({"instance_id":"id","registered_path":""}),
        ),
        (
            "launch_set_memory",
            json!({"instance_id":"id","automatic":false,"maximum_mib":4096,"optimize_before_launch":false,"hooks":{"wrapper":"command"}}),
        ),
        (
            "backup_restore",
            json!({"preview_id":uuid::Uuid::new_v4(),"plan_token":"native-secret"}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":["../outside"],"directories":[]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":["C:\\secret"],"directories":[]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":[],"directories":["a//b"]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":[],"directories":["."]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":["file:stream"],"directories":[]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":["a\\b"],"directories":[]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":vec!["valid";101],"directories":[]}),
        ),
        (
            "backup_set_exclusions",
            json!({"instance_id":"id","files":[null],"directories":[]}),
        ),
        (
            "backup_cancel",
            json!({"instance_id":"id","backup_operation_id":"not-uuid"}),
        ),
        (
            "resource_revise_plan",
            json!({"plan_id":uuid::Uuid::new_v4(),"revision":1,"accept_compatibility_nodes":vec!["node";101]}),
        ),
        (
            "resource_revise_plan",
            json!({"plan_id":uuid::Uuid::new_v4(),"revision":1,"ignore_dependencies":"true"}),
        ),
        (
            "resource_revise_plan",
            json!({"plan_id":uuid::Uuid::new_v4(),"revision":1,"native_plan_id":"foreign"}),
        ),
        (
            "content_remove",
            json!({"instance_id":"instance","entry_id":"entry","path":"../world"}),
        ),
        (
            "resource_search",
            json!({"source":"unknown","query":"x","resource_type":"mod"}),
        ),
        (
            "resource_search",
            json!({"source":"modrinth","query":"x","resource_type":"mod","limit":51}),
        ),
        (
            "resource_search",
            json!({"source":"modrinth","query":"x","resource_type":"mod","cursor":"a".repeat(8193)}),
        ),
        (
            "resource_prepare_install",
            json!({"instance_id":"id","source":"modrinth","project_id":"project","release_id":"release","resource_type":"world"}),
        ),
        (
            "resource_prepare_install",
            json!({"instance_id":"id","source":"modrinth","project_id":"project","release_id":"release","resource_type":"mod","ignore_dependencies":true}),
        ),
        (
            "resource_execute_plan",
            json!({"plan_id":uuid::Uuid::new_v4(),"revision":-1}),
        ),
        (
            "resource_execute_plan",
            json!({"plan_id":"native-plan","revision":1}),
        ),
        (
            "content_set_enabled",
            json!({"instance_id":"id","entry_id":"entry","enabled":true,"path":"../secret"}),
        ),
        ("process_get", json!({"process_id":123})),
        (
            "process_wait",
            json!({"process_id":uuid::Uuid::new_v4(),"timeout_seconds":21}),
        ),
        (
            "instance_launch",
            json!({"instance_id":"instance","extra_launch_args":["-javaagent:arbitrary"]}),
        ),
        (
            "logs_read",
            json!({"instance_id":"id","log_type":"info","filename":"../secret.txt"}),
        ),
        (
            "logs_read",
            json!({"instance_id":"id","log_type":"info","filename":"..\\secret.txt"}),
        ),
        (
            "logs_read",
            json!({"instance_id":"id","log_type":"info","filename":"latest.log:secret"}),
        ),
        (
            "logs_read",
            json!({"instance_id":"id","log_type":"info","filename":"latest.log","max_bytes":32769}),
        ),
        (
            "logs_search",
            json!({"instance_id":"id","log_type":"info","filename":"latest.log","query":""}),
        ),
        (
            "logs_search",
            json!({"instance_id":"id","log_type":"other","filename":"latest.log","query":"text"}),
        ),
        ("instance_list", json!({"limit":0})),
        ("instance_list", json!({"limit":101})),
        ("instance_list", json!({"offset":-1})),
        ("instance_list", json!({"limit":1.5})),
        ("instance_list", json!({"unknown":true})),
        ("instance_get", json!({"instance_id":""})),
        ("instance_get", json!({"instance_id":"   "})),
        ("instance_get", json!({"instance_id":null})),
        ("instance_rename", json!({"instance_id":"id","name":"\n"})),
        (
            "instance_rename",
            json!({"instance_id":"id","name":"a".repeat(257)}),
        ),
        (
            "instance_set_pinned",
            json!({"instance_id":"id","pinned":"true"}),
        ),
        (
            "instance_set_groups",
            json!({"instance_id":"id","group_ids":vec!["g";101]}),
        ),
        (
            "instance_set_groups",
            json!({"instance_id":"id","group_ids":[null]}),
        ),
        ("install_job_get", json!({"job_id":"invalid"})),
        (
            "instance_create",
            json!({"name":"Test","game_version":"1.21","loader":"invalid"}),
        ),
        (
            "instance_create",
            json!({"name":"Test","game_version":"../../outside","loader":"vanilla"}),
        ),
        (
            "instance_create",
            json!({"name":"Test","game_version":"1.21","loader":"fabric","loader_version":"..\\outside"}),
        ),
        (
            "instance_create",
            json!({"name":"Test","game_version":"1.21","loader":"vanilla","hooks":{"pre_launch":"command"}}),
        ),
    ];
    for (name, mut arguments) in cases {
        arguments["session_id"] = json!(control);
        let result = client.tool(name, arguments).await;
        assert!(result.get("error").is_some(), "{name}: {result}");
    }
    assert!(
        client
            .tool("instance_list", json!({}))
            .await
            .get("error")
            .is_some()
    );
    assert!(backend.calls.lock().unwrap().is_empty());
    server.stop().await;
}

#[test]
fn historical_game_ids_are_accepted_without_allowing_paths() {
    let tool = crate::catalog::entries()
        .into_iter()
        .find(|entry| entry.tool.name == "metadata_list_loader_versions")
        .unwrap();
    for id in [
        "1.21.1",
        "24w14potato",
        "3D Shareware v1.34",
        "inf-20100618",
        "b1.9-pre1",
    ] {
        let args =
            json!({"session_id":"session","loader":"fabric","game_version":id});
        assert!(
            crate::catalog::validate(&tool, args.as_object().unwrap()).is_ok(),
            "{id}"
        );
    }
    for id in [
        "../file",
        "C:\\file",
        "1.21/../../outside",
        " version",
        "version.",
        "version ",
    ] {
        let args =
            json!({"session_id":"session","loader":"fabric","game_version":id});
        assert!(
            crate::catalog::validate(&tool, args.as_object().unwrap()).is_err(),
            "{id}"
        );
    }
}

#[tokio::test]
async fn ending_control_drains_admitted_mutation_and_cancels_queued_mutation() {
    let backend = Arc::new(TestBackend::default());
    backend.block.store(true, Ordering::SeqCst);
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Cancellation").await;
    let control = control_id(&client.tool("start_session", json!({})).await);
    let task = client.clone();
    let args = json!({"session_id":control,"instance_id":"id","name":"First"});
    let first =
        tokio::spawn(async move { task.tool("instance_rename", args).await });
    wait_for(|| backend.calls.lock().unwrap().len() == 1).await;
    let task = client.clone();
    let args = json!({"session_id":control,"instance_id":"id","name":"Second"});
    let queued =
        tokio::spawn(async move { task.tool("instance_rename", args).await });
    client
        .tool("end_session", json!({"session_id":control}))
        .await;
    let result = tokio::time::timeout(Duration::from_secs(3), queued)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        tool_error(&result),
        "session_required" | "session_revoked"
    ));
    backend.resume.notify_one();
    assert_eq!(first.await.unwrap()["result"]["isError"], false);
    assert_eq!(backend.finished.load(Ordering::SeqCst), 1);
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
    assert!(server.snapshot().connection.is_none());
    server.stop().await;
}

#[tokio::test]
async fn forced_disconnect_drops_read_but_finishes_admitted_write() {
    for (name, write) in [("instance_get", false), ("instance_rename", true)] {
        let backend = Arc::new(TestBackend::default());
        backend.block.store(true, Ordering::SeqCst);
        let server = server_with_backend(true, false, backend.clone()).await;
        let client = connect(&server, false, "Disconnect").await;
        let control =
            control_id(&client.tool("start_session", json!({})).await);
        let mut args = json!({"session_id":control,"instance_id":"id"});
        if write {
            args["name"] = json!("Renamed");
        }
        let pending = tokio::spawn(async move {
            let _ = client
                .post()
                .json(&client.message(
                    "tools/call",
                    json!({"name":name,"arguments":args}),
                ))
                .send()
                .await;
        });
        wait_for(|| !backend.calls.lock().unwrap().is_empty()).await;
        server
            .disconnect(&server.snapshot().connection.unwrap().id)
            .await;
        backend.resume.notify_one();
        if write {
            wait_for(|| backend.finished.load(Ordering::SeqCst) == 1).await;
        } else {
            assert_eq!(backend.finished.load(Ordering::SeqCst), 0);
        }
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap();
        server.stop().await;
    }
}

#[tokio::test]
async fn backend_failures_are_tool_errors_and_do_not_end_control() {
    let backend = Arc::new(TestBackend::default());
    backend.fail.store(true, Ordering::SeqCst);
    let server = server_with_backend(true, false, backend.clone()).await;
    let client = connect(&server, false, "Business errors").await;
    let control = control_id(&client.tool("start_session", json!({})).await);
    for (name, args) in [
        (
            "instance_get",
            json!({"session_id":control,"instance_id":"missing"}),
        ),
        (
            "instance_rename",
            json!({"session_id":control,"instance_id":"missing","name":"Name"}),
        ),
    ] {
        let response = client.tool(name, args).await;
        assert_eq!(tool_error(&response), "not_found");
        assert_eq!(
            response["result"]["structuredContent"]["error"]["message"],
            "Instance not found"
        );
    }
    assert!(server.snapshot().connection.unwrap().connected);
    backend.fail.store(false, Ordering::SeqCst);
    assert_eq!(
        client
            .tool(
                "instance_get",
                json!({"session_id":control,"instance_id":"present"})
            )
            .await["result"]["isError"],
        false
    );
    server.stop().await;
}
