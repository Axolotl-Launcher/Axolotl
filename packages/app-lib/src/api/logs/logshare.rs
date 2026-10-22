use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::crash_analysis::collect_crash_logs;
use crate::State;

const LOGSHARE_BASE_URL: &str = "https://api.logshare.cn";
const LOGSHARE_AI_READ_TIMEOUT_SECS: u64 = 300;
const LOGSHARE_SOURCE_PREFIX: &str = "axolotl";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogShareSettings {
    pub share_provider: String,
    pub ai_source: String,
    pub auto_upload: bool,
    pub multi_file: bool,
    pub no_storage: bool,
    pub show_progress: bool,
}

impl Default for LogShareSettings {
    fn default() -> Self {
        Self {
            share_provider: "logshare".to_string(),
            ai_source: "logshare".to_string(),
            auto_upload: true,
            multi_file: true,
            no_storage: false,
            show_progress: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SharedLog {
    pub id: String,
    pub url: String,
    pub raw: String,
    pub token: String,
    pub provider: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub truncated: bool,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogShareUploadResponse {
    pub success: bool,
    pub id: String,
    pub url: String,
    pub raw: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogShareDeleteResponse {
    pub success: bool,
    pub deleted: Vec<String>,
    pub failed: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
struct UploadFile {
    name: String,
    content: String,
}

#[derive(Serialize, Debug, Clone)]
struct UploadBody {
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<UploadFile>,
    source: String,
}

pub async fn get_log_share_settings() -> crate::Result<LogShareSettings> {
    let state = State::get().await?;
    let row = sqlx::query_as::<_, (String, String, i64, i64, i64, i64)>(
        "SELECT s.share_provider, a.ai_source, s.auto_upload, s.multi_file, s.no_storage, s.show_progress FROM log_share_settings s JOIN crash_analysis_ai_settings a ON a.id = s.id WHERE s.id = 0",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(LogShareSettings {
        share_provider: row.0,
        ai_source: row.1,
        auto_upload: row.2 != 0,
        multi_file: row.3 != 0,
        no_storage: row.4 != 0,
        show_progress: row.5 != 0,
    })
}

pub async fn update_log_share_settings(
    settings: LogShareSettings,
) -> crate::Result<()> {
    let settings = normalize_settings(settings);
    let state = State::get().await?;
    sqlx::query(
        "UPDATE log_share_settings SET share_provider = ?, auto_upload = ?, multi_file = ?, no_storage = ?, show_progress = ? WHERE id = 0",
    )
    .bind(&settings.share_provider)
    .bind(settings.auto_upload)
    .bind(settings.multi_file)
    .bind(settings.no_storage)
    .bind(settings.show_progress)
    .execute(&state.pool)
    .await?;
    Ok(())
}

fn normalize_settings(mut settings: LogShareSettings) -> LogShareSettings {
    if settings.share_provider != "logshare" {
        settings.share_provider = "mclogs".to_string();
    }
    if settings.ai_source != "logshare" {
        settings.ai_source = "custom".to_string();
    }
    settings
}

pub async fn list_shared_logs() -> crate::Result<Vec<SharedLog>> {
    let state = State::get().await?;
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, url, raw, token, provider, instance_id, instance_name, truncated, created_at FROM shared_logs ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| SharedLog {
            id: row.0,
            url: row.1,
            raw: row.2,
            token: row.3,
            provider: row.4,
            instance_id: row.5,
            instance_name: row.6,
            truncated: row.7 != 0,
            created_at: row.8,
        })
        .collect())
}

pub async fn record_shared_log(log: SharedLog) -> crate::Result<()> {
    let state = State::get().await?;
    let created_at = log.created_at.max(now_seconds());
    sqlx::query(
        "INSERT INTO shared_logs (id, url, raw, token, provider, instance_id, instance_name, truncated, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET url = excluded.url, raw = excluded.raw, token = excluded.token, provider = excluded.provider, instance_id = excluded.instance_id, instance_name = excluded.instance_name, truncated = excluded.truncated",
    )
    .bind(&log.id)
    .bind(&log.url)
    .bind(&log.raw)
    .bind(&log.token)
    .bind(&log.provider)
    .bind(&log.instance_id)
    .bind(&log.instance_name)
    .bind(log.truncated)
    .bind(created_at)
    .execute(&state.pool)
    .await?;
    Ok(())
}

async fn remove_shared_log_record(id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    sqlx::query("DELETE FROM shared_logs WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn delete_shared_log(id: String, token: String) -> crate::Result<()> {
    let remote_result = delete_log(&id, &token).await;
    if let Err(error) = remote_result {
        tracing::warn!(
            "Remote LogShare deletion failed for {id}; removing the local record anyway: {error}"
        );
    }
    remove_shared_log_record(&id).await
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn source_identifier() -> String {
    format!("{LOGSHARE_SOURCE_PREFIX}/{}", env!("CARGO_PKG_VERSION"))
}

async fn prepare_upload(instance_id: &str) -> crate::Result<UploadBody> {
    let settings = get_log_share_settings().await?;
    let logs = collect_crash_logs(instance_id).await?;

    let combined = logs.combined_log.as_str().to_string();
    let files = if settings.multi_file {
        logs.sources
            .iter()
            .map(|source| UploadFile {
                name: source.name.clone(),
                content: source.content.as_str().to_string(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(UploadBody {
        content: combined,
        files,
        source: source_identifier(),
    })
}

async fn request_client() -> crate::Result<reqwest::Client> {
    let state = State::get().await?;
    let proxy = state.proxy_config().await?;
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(LOGSHARE_AI_READ_TIMEOUT_SECS))
        .user_agent(crate::launcher_user_agent());
    proxy.apply(builder)?.build().map_err(Into::into)
}

pub async fn upload_crash(
    instance_id: &str,
) -> crate::Result<LogShareUploadResponse> {
    let prepared = prepare_upload(instance_id).await?;
    upload_prepared(&prepared).await
}

async fn upload_prepared(
    prepared: &UploadBody,
) -> crate::Result<LogShareUploadResponse> {
    let client = request_client().await?;
    let body_value = serde_json::to_value(prepared)?;
    let request = client
        .post(format!("{LOGSHARE_BASE_URL}/v1/log"))
        .header("Accept", "application/json")
        .header("Accept-Encoding", "br, gzip");

    let response = request.json(&body_value).send().await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "LogShare upload failed with HTTP {status}: {}",
            text.chars().take(500).collect::<String>()
        ))
        .into());
    }
    let value: Value = serde_json::from_str(&text).map_err(|_| {
        crate::ErrorKind::OtherError(
            "LogShare returned invalid JSON".to_string(),
        )
    })?;
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(crate::ErrorKind::OtherError(format!(
            "LogShare upload failed: {}",
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        ))
        .into());
    }
    Ok(LogShareUploadResponse {
        success: true,
        id: value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        url: value
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        raw: value
            .get("raw")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        token: value
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

pub async fn get_insights(id: &str) -> crate::Result<Value> {
    let client = request_client().await?;
    let response = client
        .get(format!("{LOGSHARE_BASE_URL}/v1/insights/{id}"))
        .header("Accept", "application/json")
        .header("Accept-Encoding", "br, gzip")
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "LogShare analysis failed with HTTP {status}: {}",
            text.chars().take(500).collect::<String>()
        ))
        .into());
    }
    serde_json::from_str(&text).map_err(|_| {
        crate::ErrorKind::OtherError(
            "LogShare returned invalid insights JSON".to_string(),
        )
        .into()
    })
}

pub async fn analyse_crash_direct(instance_id: &str) -> crate::Result<Value> {
    let prepared = prepare_upload(instance_id).await?;
    let client = request_client().await?;
    let response = client
        .post(format!("{LOGSHARE_BASE_URL}/v1/analyse"))
        .header("Accept", "application/json")
        .header("Accept-Encoding", "br, gzip")
        .json(&prepared)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "LogShare analysis failed with HTTP {status}: {}",
            text.chars().take(500).collect::<String>()
        ))
        .into());
    }
    serde_json::from_str(&text).map_err(|_| {
        crate::ErrorKind::OtherError(
            "LogShare returned invalid analysis JSON".to_string(),
        )
        .into()
    })
}

pub async fn delete_log(
    id: &str,
    token: &str,
) -> crate::Result<LogShareDeleteResponse> {
    let client = request_client().await?;
    let response = client
        .delete(format!("{LOGSHARE_BASE_URL}/v1/log/{id}"))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "br, gzip")
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "LogShare deletion failed with HTTP {status}: {}",
            text.chars().take(500).collect::<String>()
        ))
        .into());
    }
    serde_json::from_str(&text).map_err(|_| {
        crate::ErrorKind::OtherError(
            "LogShare returned invalid deletion JSON".to_string(),
        )
        .into()
    })
}

pub(super) async fn analyze_snapshot(
    files: Vec<(String, String)>,
    settings: &LogShareSettings,
    ticket: Option<LogShareUploadResponse>,
    emit: impl Fn(&str, Value) + Send + Sync,
) -> crate::Result<super::logagent::CloudResult> {
    let content = files
        .iter()
        .map(|(name, content)| format!("===== {name} =====\n{content}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let prepared = UploadBody {
        content,
        files: if settings.multi_file {
            files
                .into_iter()
                .map(|(name, content)| UploadFile { name, content })
                .collect()
        } else {
            Vec::new()
        },
        source: source_identifier(),
    };
    let (url, body, ticket) = if settings.no_storage {
        (
            format!("{LOGSHARE_BASE_URL}/v1/ai/analyse"),
            Some(serde_json::to_value(&prepared)?),
            None,
        )
    } else {
        let ticket = match ticket {
            Some(ticket)
                if !ticket.id.is_empty()
                    && ticket.id.chars().all(|ch| {
                        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
                    }) =>
            {
                ticket
            }
            _ => upload_prepared(&prepared).await?,
        };
        (
            format!("{LOGSHARE_BASE_URL}/v1/ai/{}", ticket.id),
            None,
            Some(ticket),
        )
    };
    if let Some(ticket) = &ticket {
        emit("uploaded", serde_json::to_value(ticket)?);
    }
    let content = stream_ai(url, body, settings.show_progress, emit).await?;
    Ok(super::logagent::CloudResult { content, ticket })
}

async fn stream_ai(
    url: String,
    body: Option<Value>,
    show_progress: bool,
    emit: impl Fn(&str, Value) + Send + Sync,
) -> crate::Result<String> {
    let client = request_client().await?;
    let mut request = client
        .request(
            if body.is_some() {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            },
            url.as_str(),
        )
        .header("Accept", "text/event-stream")
        .header("Accept-Encoding", "br, gzip");
    if let Some(payload) = body {
        request = request.json(&payload);
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(crate::ErrorKind::OtherError(
                "LogAgent queue is full, please retry later".to_string(),
            )
            .into());
        }
        let text = response.text().await?;
        return Err(crate::ErrorKind::OtherError(format!(
            "LogAgent analysis failed with HTTP {status}: {}",
            text.chars().take(500).collect::<String>()
        ))
        .into());
    }

    let mut stream = response.bytes_stream();
    let mut parser = SseParser::new();
    let mut output = String::new();
    let mut received_bytes = 0usize;

    loop {
        let maybe = tokio::time::timeout(
            Duration::from_secs(LOGSHARE_AI_READ_TIMEOUT_SECS),
            stream.next(),
        )
        .await
        .map_err(|_| {
            crate::ErrorKind::OtherError(
                "LogAgent analysis timed out".to_string(),
            )
        })?;
        let Some(chunk) = maybe else {
            break;
        };
        let chunk = chunk?;
        received_bytes = received_bytes.saturating_add(chunk.len());
        if received_bytes > 8 * 1024 * 1024 {
            return Err(crate::ErrorKind::OtherError(
                "LogShare analysis exceeded its response budget".to_string(),
            )
            .into());
        }
        for event in parser.feed(&chunk) {
            if let Some(delta) = event.content {
                output.push_str(&delta);
                if show_progress {
                    emit("delta", json!({ "content": delta }));
                }
            }
            if let Some(status_event) = event.status_event
                && (show_progress || status_event.event_type == "queued")
            {
                emit(&status_event.event_type, status_event.data);
            }
            if let Some(message) = event.error {
                return Err(crate::ErrorKind::OtherError(format!(
                    "LogAgent analysis failed: {message}"
                ))
                .into());
            }
            if event.done {
                return Ok(output);
            }
        }
    }

    // A stream that ended without `event: done` is acceptable as long as text
    // was produced; otherwise it is a protocol error worth surfacing.
    if output.is_empty() {
        return Err(crate::ErrorKind::OtherError(
            "LogAgent analysis ended without output".to_string(),
        )
        .into());
    }
    Ok(output)
}

struct ParsedEvent {
    content: Option<String>,
    status_event: Option<ParsedStatusEvent>,
    done: bool,
    error: Option<String>,
}

struct ParsedStatusEvent {
    event_type: String,
    data: Value,
}

struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn feed(&mut self, chunk: &[u8]) -> Vec<ParsedEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        loop {
            let lf_pos = find_bytes(&self.buffer, b"\n\n");
            let crlf_pos = find_bytes(&self.buffer, b"\r\n\r\n");
            let (pos, separator_len) = match (lf_pos, crlf_pos) {
                (Some(lf_pos), Some(crlf_pos)) if crlf_pos < lf_pos => {
                    (crlf_pos, 4)
                }
                (Some(lf_pos), _) => (lf_pos, 2),
                (None, Some(crlf_pos)) => (crlf_pos, 4),
                (None, None) => break,
            };
            let block =
                String::from_utf8_lossy(&self.buffer[..pos]).into_owned();
            self.buffer.drain(..pos + separator_len);
            if let Some(event) = parse_sse_block(&block) {
                events.push(event);
            }
        }
        events
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_sse_block(block: &str) -> Option<ParsedEvent> {
    let mut event_name: Option<String> = None;
    let mut data_lines = Vec::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
        }
    }
    if event_name.is_none() && data_lines.is_empty() {
        return None;
    }

    let data_text = data_lines.join("\n");
    let event_name = event_name.as_deref().unwrap_or("message");

    match event_name {
        "done" => Some(ParsedEvent {
            content: None,
            status_event: None,
            done: true,
            error: None,
        }),
        "error" => Some(ParsedEvent {
            content: None,
            status_event: None,
            done: false,
            error: Some(
                serde_json::from_str::<Value>(&data_text)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("error")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| data_text.clone()),
            ),
        }),
        "status" => {
            let Ok(value) = serde_json::from_str::<Value>(&data_text) else {
                return None;
            };
            let event_type = value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("status")
                .to_string();
            Some(ParsedEvent {
                content: None,
                status_event: Some(ParsedStatusEvent {
                    event_type,
                    data: value,
                }),
                done: false,
                error: None,
            })
        }
        _ => {
            let Ok(value) = serde_json::from_str::<Value>(&data_text) else {
                return None;
            };
            let content = value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(ParsedEvent {
                content,
                status_event: None,
                done: false,
                error: None,
            })
        }
    }
}
