//! Tool catalog and argument handling for the observability MCP.
//!
//! All tools are read-only. Arguments are validated here, before a source call,
//! so a malformed request never reaches the capture.

use crate::source::Source;
use rmcp::{
    ErrorData, RoleServer,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Tool,
        ToolAnnotations,
    },
    service::RequestContext,
};
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// Default number of records a listing tool returns.
pub const DEFAULT_LIMIT: usize = 50;
/// Hard ceiling for listing tools, whatever the client asks for.
pub const MAX_LIMIT: usize = 500;

/// Every tool this server exposes.
pub fn tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "observability_status",
            "Report whether local telemetry capture is running, how much it holds, and which sessions are active. Read-only.",
            json_schema(json!({"type":"object","properties":{},"additionalProperties":false})),
        )
        .with_annotations(read_only()),
        Tool::new(
            "observability_events",
            "List the most recent structured telemetry events, newest first. Read-only.",
            json_schema(json!({
                "type":"object",
                "properties":{
                    "limit":{"type":"integer","minimum":1,"maximum":MAX_LIMIT},
                    "severity":{"type":"string","enum":["trace","debug","info","warn","error"]},
                    "name_prefix":{"type":"string","minLength":1,"maxLength":64}
                },
                "additionalProperties":false
            })),
        )
        .with_annotations(read_only()),
        Tool::new(
            "observability_metrics",
            "List aggregated counters and histograms recorded since capture started. Read-only.",
            json_schema(json!({
                "type":"object",
                "properties":{
                    "name_prefix":{"type":"string","minLength":1,"maxLength":64}
                },
                "additionalProperties":false
            })),
        )
        .with_annotations(read_only()),
        Tool::new(
            "observability_spans",
            "List recently closed domain spans with their durations, newest first. Read-only.",
            json_schema(json!({
                "type":"object",
                "properties":{
                    "limit":{"type":"integer","minimum":1,"maximum":MAX_LIMIT},
                    "min_duration_ms":{"type":"number","minimum":0}
                },
                "additionalProperties":false
            })),
        )
        .with_annotations(read_only()),
        Tool::new(
            "observability_flush",
            "Flush every telemetry consumer that buffers. Local capture keeps data in memory, so this usually only reports status. Read-only.",
            json_schema(json!({"type":"object","properties":{},"additionalProperties":false})),
        )
        .with_annotations(read_only()),
    ]
}

fn json_schema(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn read_only() -> ToolAnnotations {
    ToolAnnotations::new().read_only(true).open_world(false)
}

/// Dispatch one validated tool call.
pub fn call(
    source: &Arc<dyn Source>,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    let arguments = request.arguments.unwrap_or_default();
    match request.name.as_ref() {
        "observability_status" => {
            let stats = source.stats();
            Ok(success(json!({
                "recording": stats.recording,
                "event_capacity": stats.event_capacity,
                "event_count": stats.event_count,
                "dropped_events": stats.dropped_events,
                "span_count": stats.span_count,
                "installed_at_unix_ms": stats.installed_at_unix_ms,
            })))
        }
        "observability_events" => {
            let limit = limit(&arguments)?;
            let severity = optional_string(&arguments, "severity")?;
            if severity.as_deref().is_some_and(|severity| {
                !["trace", "debug", "info", "warn", "error"].contains(&severity)
            }) {
                return Err(ErrorData::invalid_params(
                    "severity must be trace, debug, info, warn or error",
                    None,
                ));
            }
            let prefix = optional_string(&arguments, "name_prefix")?;
            Ok(success(source.events(
                limit,
                severity.as_deref(),
                prefix.as_deref(),
            )))
        }
        "observability_metrics" => {
            let prefix = optional_string(&arguments, "name_prefix")?;
            let matches = |name: &str| {
                prefix
                    .as_deref()
                    .is_none_or(|prefix| name.starts_with(prefix))
            };
            let counters: Vec<_> = source
                .counters()
                .into_iter()
                .filter(|counter| matches(&counter.name))
                .collect();
            let histograms: Vec<_> = source
                .histograms()
                .into_iter()
                .filter(|histogram| matches(&histogram.name))
                .collect();
            Ok(success(
                json!({"counters": counters, "histograms": histograms}),
            ))
        }
        "observability_spans" => {
            let limit = limit(&arguments)?;
            let min_duration_ms =
                optional_f64(&arguments, "min_duration_ms")?.unwrap_or(0.0);
            if !min_duration_ms.is_finite() || min_duration_ms < 0.0 {
                return Err(ErrorData::invalid_params(
                    "min_duration_ms must be a finite number",
                    None,
                ));
            }
            Ok(success(source.spans(limit, min_duration_ms)))
        }
        "observability_flush" => {
            Ok(success(json!({"flushed": source.flush()})))
        }
        other => Err(ErrorData::invalid_params(
            format!("Unknown observability tool: {other}"),
            None,
        )),
    }
}

/// Rust-typed handler used by the protocol layer.
///
/// The flush reaches the synchronous provider flush, which can block for its
/// whole budget, so it runs on a blocking worker instead of pinning a runtime
/// thread that unrelated Tauri commands also use.
pub async fn handle(
    source: &Arc<dyn Source>,
    request: CallToolRequestParams,
    _context: RequestContext<RoleServer>,
) -> Result<CallToolResponse, ErrorData> {
    if request.name.as_ref() != "observability_flush" {
        return call(source, request).map(Into::into);
    }
    let source = Arc::clone(source);
    let flushed = tokio::task::spawn_blocking(move || source.flush())
        .await
        .map_err(|error| {
            ErrorData::internal_error(
                format!("flush task failed: {error}"),
                None,
            )
        })?;
    Ok(success(json!({"flushed": flushed})).into())
}

fn limit(arguments: &Map<String, Value>) -> Result<usize, ErrorData> {
    let Some(value) = arguments.get("limit") else {
        return Ok(DEFAULT_LIMIT);
    };
    let Some(limit) = value.as_u64() else {
        return Err(ErrorData::invalid_params(
            "limit must be a positive integer",
            None,
        ));
    };
    if limit == 0 {
        return Err(ErrorData::invalid_params(
            "limit must be at least 1",
            None,
        ));
    }
    Ok(usize::try_from(limit).unwrap_or(MAX_LIMIT).min(MAX_LIMIT))
}

fn optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ErrorData> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            if value.trim().is_empty() {
                return Err(ErrorData::invalid_params(
                    format!("{key} must not be empty"),
                    None,
                ));
            }
            Ok(Some(value.clone()))
        }
        Some(_) => Err(ErrorData::invalid_params(
            format!("{key} must be a string"),
            None,
        )),
    }
}

fn optional_f64(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<f64>, ErrorData> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => Ok(number.as_f64()),
        Some(_) => Err(ErrorData::invalid_params(
            format!("{key} must be a number"),
            None,
        )),
    }
}

fn success<T: serde::Serialize>(value: T) -> CallToolResult {
    let text = serde_json::to_string(&value)
        .unwrap_or_else(|_| "{\"error\":\"unserializable result\"}".into());
    CallToolResult::success(vec![rmcp::model::ContentBlock::text(text)])
}
