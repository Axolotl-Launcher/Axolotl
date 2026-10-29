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
    assert_eq!(names, ["get_launcher_info", "start_session", "end_session"]);
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
