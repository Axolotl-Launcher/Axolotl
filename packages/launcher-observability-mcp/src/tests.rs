use super::*;
use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct TestSource {
    events: Vec<EventRecord>,
    counters: Vec<CounterSeries>,
    histograms: Vec<HistogramSeries>,
    spans: Vec<SpanRecord>,
}

impl Source for TestSource {
    fn stats(&self) -> CaptureStats {
        CaptureStats {
            recording: true,
            event_capacity: 512,
            event_count: self.events.len(),
            dropped_events: 0,
            span_count: self.spans.len(),
            installed_at_unix_ms: 1,
        }
    }

    fn events(
        &self,
        limit: usize,
        severity: Option<&str>,
        name_prefix: Option<&str>,
    ) -> Vec<EventRecord> {
        self.events
            .iter()
            .filter(|event| {
                severity.is_none_or(|severity| event.severity == severity)
            })
            .filter(|event| {
                name_prefix.is_none_or(|prefix| event.name.starts_with(prefix))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    fn counters(&self) -> Vec<CounterSeries> {
        self.counters.clone()
    }

    fn histograms(&self) -> Vec<HistogramSeries> {
        self.histograms.clone()
    }

    fn spans(&self, limit: usize, min_duration_ms: f64) -> Vec<SpanRecord> {
        self.spans
            .iter()
            .filter(|span| span.duration_ms >= min_duration_ms)
            .take(limit)
            .cloned()
            .collect()
    }

    fn flush(&self) -> bool {
        true
    }
}

fn source() -> Arc<dyn Source> {
    Arc::new(TestSource {
        events: vec![
            EventRecord {
                name: "install.job".into(),
                severity: "info".into(),
                occurred_at_unix_ms: 2,
                duration_ms: Some(5),
                attributes: Map::default(),
            },
            EventRecord {
                name: "app.started".into(),
                severity: "info".into(),
                occurred_at_unix_ms: 1,
                duration_ms: None,
                attributes: Map::default(),
            },
        ],
        counters: vec![
            CounterSeries {
                name: "axolotl.errors".into(),
                value: 3,
                attributes: Map::default(),
            },
            CounterSeries {
                name: "other.counter".into(),
                value: 1,
                attributes: Map::default(),
            },
        ],
        histograms: vec![HistogramSeries {
            name: "axolotl.ipc.duration".into(),
            count: 2,
            min: 1.0,
            max: 3.0,
            last: 3.0,
            sum: 4.0,
            attributes: Map::default(),
        }],
        spans: vec![
            SpanRecord {
                name: "instance_list".into(),
                target: "theseus::api::instance".into(),
                started_at_unix_ms: 1,
                duration_ms: 30.0,
            },
            SpanRecord {
                name: "cache_read".into(),
                target: "theseus::state".into(),
                started_at_unix_ms: 1,
                duration_ms: 1.0,
            },
        ],
    })
}

struct Client {
    http: reqwest::Client,
    endpoint: String,
    wire: String,
    token: Option<String>,
    ids: AtomicU32,
}

impl Client {
    fn post(&self) -> reqwest::RequestBuilder {
        let request = self
            .http
            .post(&self.endpoint)
            .header("Mcp-Session-Id", &self.wire)
            .header("Accept", "application/json, text/event-stream");
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn tool(&self, name: &str, arguments: Value) -> Value {
        let response = self
            .post()
            .json(&json!({
                "jsonrpc": "2.0",
                "id": self.ids.fetch_add(1, Ordering::SeqCst),
                "method": "tools/call",
                "params": {"name": name, "arguments": arguments},
            }))
            .send()
            .await
            .unwrap();
        decode(response).await
    }
}

async fn connect(
    server: &Server,
    token: Option<&str>,
) -> (Client, reqwest::Response) {
    let http = reqwest::Client::builder().build().expect("HTTP client");
    let endpoint = server.snapshot().endpoint.expect("endpoint");
    let request = http
        .post(&endpoint)
        .header("Accept", "application/json, text/event-stream");
    let request = match token {
        Some(token) => request.bearer_auth(token),
        None => request,
    };
    let response = request
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "observability-test", "version": "1"}
            }
        }))
        .send()
        .await
        .unwrap();
    let wire = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let client = Client {
        http,
        endpoint,
        wire,
        token: token.map(str::to_owned),
        ids: AtomicU32::new(2),
    };
    (client, response)
}

/// Connect and complete the handshake, so tool calls are accepted.
async fn initialize(server: &Server, token: Option<&str>) -> (Client, Value) {
    let (client, response) = connect(server, token).await;
    assert!(response.status().is_success());
    let body = decode(response).await;
    let accepted = client
        .post()
        .json(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    (client, body)
}

async fn decode(response: reqwest::Response) -> Value {
    let text = response.text().await.unwrap();
    if let Ok(value) = serde_json::from_str(&text) {
        return value;
    }
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .find_map(|data| serde_json::from_str(data.trim()).ok())
        .unwrap_or_else(|| panic!("no JSON-RPC payload in {text}"))
}

fn result_text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no tool text in {response}"))
        .to_owned()
}

#[tokio::test]
async fn rejects_requests_without_the_token() {
    let server =
        Server::start(0, "test-token".into(), true, "9.8.7-test", source())
            .await
            .unwrap();
    let (_, response) = connect(&server, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    server.stop().await;
}

#[tokio::test]
async fn lists_tools_and_reports_status() {
    let server = Server::start(0, String::new(), false, "9.8.7-test", source())
        .await
        .unwrap();
    let (client, body) = initialize(&server, None).await;
    assert_eq!(body["result"]["serverInfo"]["version"], "9.8.7-test");
    assert!(body["result"]["capabilities"]["tools"].is_object());

    let status = client.tool("observability_status", json!({})).await;
    let status: Value = serde_json::from_str(&result_text(&status)).unwrap();
    assert_eq!(status["recording"], true);
    assert_eq!(status["event_count"], 2);

    server.stop().await;
}

#[tokio::test]
async fn filters_events_metrics_and_spans() {
    let server = Server::start(0, String::new(), false, "9.8.7-test", source())
        .await
        .unwrap();
    let (client, _) = initialize(&server, None).await;

    let events = client
        .tool(
            "observability_events",
            json!({"limit": 1, "name_prefix": "install."}),
        )
        .await;
    let events: Value = serde_json::from_str(&result_text(&events)).unwrap();
    assert_eq!(events.as_array().unwrap().len(), 1);
    assert_eq!(events[0]["name"], "install.job");

    let metrics = client
        .tool("observability_metrics", json!({"name_prefix": "axolotl."}))
        .await;
    let metrics: Value = serde_json::from_str(&result_text(&metrics)).unwrap();
    assert_eq!(metrics["counters"].as_array().unwrap().len(), 1);
    assert_eq!(metrics["counters"][0]["name"], "axolotl.errors");
    assert_eq!(metrics["histograms"][0]["count"], 2);

    let spans = client
        .tool("observability_spans", json!({"min_duration_ms": 10}))
        .await;
    let spans: Value = serde_json::from_str(&result_text(&spans)).unwrap();
    assert_eq!(spans.as_array().unwrap().len(), 1);
    assert_eq!(spans[0]["name"], "instance_list");

    server.stop().await;
}

#[tokio::test]
async fn rejects_unknown_tools_and_bad_arguments() {
    let server = Server::start(0, String::new(), false, "9.8.7-test", source())
        .await
        .unwrap();
    let (client, _) = initialize(&server, None).await;

    let unknown = client
        .tool("observability_delete_everything", json!({}))
        .await;
    assert!(unknown.get("error").is_some());

    let bad_severity = client
        .tool("observability_events", json!({"severity": "loud"}))
        .await;
    assert!(bad_severity.get("error").is_some());

    let bad_limit = client
        .tool("observability_events", json!({"limit": 0}))
        .await;
    assert!(bad_limit.get("error").is_some());

    server.stop().await;
}

#[tokio::test]
async fn stops_listening_after_stop() {
    let server = Server::start(0, String::new(), false, "9.8.7-test", source())
        .await
        .unwrap();
    let endpoint = server.snapshot().endpoint.unwrap();
    server.stop().await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let response = client.post(&endpoint).send().await;
    assert!(
        response.is_err() || response.unwrap().status().is_server_error(),
        "listener kept accepting requests after stop"
    );
}
