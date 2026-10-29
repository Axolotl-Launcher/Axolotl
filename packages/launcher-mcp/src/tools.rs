use crate::session::Shared;
use rmcp::{
    ErrorData, RoleServer,
    model::{CallToolRequestParams, CallToolResult, Tool, ToolAnnotations},
    service::RequestContext,
};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn definitions() -> Vec<Tool> {
    [
        ("get_launcher_info", "Read the launcher name, actual application version, and platform. Does not request control or require start_session.", true, json!({"type":"object","properties":{},"additionalProperties":false})),
        ("start_session", "Request permission to control the launcher for the user's task. May show a confirmation dialog; do not call during startup or tool discovery. Returns a session_id. Call end_session when finished. Does not provide screenshot, input, or other control tools yet.", false, json!({"type":"object","properties":{},"additionalProperties":false})),
        ("end_session", "Release the control session and remove the AI indicator. Keeps the MCP transport available for discovery or a future start_session.", false, json!({"type":"object","properties":{"session_id":{"type":"string"}},"required":["session_id"],"additionalProperties":false})),
    ].into_iter().map(|(name, description, read_only, schema)| {
        Tool::new(name, description, schema.as_object().unwrap().clone())
            .with_annotations(ToolAnnotations::new().read_only(read_only).destructive(false).idempotent(true).open_world(false))
    }).collect()
}

pub(crate) async fn call(
    shared: &Arc<Shared>,
    transport: &str,
    request: CallToolRequestParams,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let arguments = request.arguments.unwrap_or_default();
    let result = match request.name.as_ref() {
        "get_launcher_info" if arguments.is_empty() => {
            Ok(serde_json::to_value(&shared.info).expect("launcher metadata"))
        }
        "start_session" if arguments.is_empty() => {
            shared.start_control(transport, context).await
        }
        "end_session" if arguments.len() == 1 => {
            let control = arguments
                .get("session_id")
                .and_then(|id| id.as_str())
                .filter(|id| !id.is_empty())
                .ok_or_else(|| {
                    ErrorData::invalid_params("session_id is required", None)
                })?;
            shared
                .release_control(transport, control)
                .map(|()| json!({"ended":true}))
        }
        "get_launcher_info" | "start_session" | "end_session" => {
            return Err(ErrorData::invalid_params(
                "Invalid tool arguments",
                None,
            ));
        }
        _ => {
            return Err(ErrorData::invalid_params(
                "Unknown launcher tool",
                None,
            ));
        }
    };
    Ok(match result {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => CallToolResult::structured_error(json!({"error":error})),
    })
}
