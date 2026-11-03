use crate::{
    ToolError,
    catalog::{self, Access},
    session::Shared,
};
use launcher_tools::{
    ContentBlock, ExecutionContext, ExecutionScope, ToolOutput,
};
use rmcp::{
    ErrorData, RoleServer,
    model::{CallToolRequestParams, CallToolResult},
    service::RequestContext,
};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn execution_scope(
    shared: &Arc<Shared>,
    transport: &str,
    control: &str,
) -> ExecutionScope {
    let shared = Arc::downgrade(shared);
    let transport = transport.to_owned();
    let control = control.to_owned();
    ExecutionScope::new(control.clone(), move |full| {
        shared
            .upgrade()
            .ok_or_else(|| {
                ToolError::new("session_revoked", "Control session ended")
            })?
            .authorize(&transport, &control, full)
    })
}

pub(crate) fn encode(output: ToolOutput) -> CallToolResult {
    let mut result = CallToolResult::success(
        output
            .content
            .into_iter()
            .map(|content| match content {
                ContentBlock::Text { text } => {
                    rmcp::model::ContentBlock::text(text)
                }
                ContentBlock::Image { data, mime_type } => {
                    rmcp::model::ContentBlock::image(data, mime_type)
                }
            })
            .collect(),
    );
    result.is_error = Some(output.is_error);
    result.structured_content = Some(output.structured_content);
    result
}

pub(crate) async fn call(
    shared: &Arc<Shared>,
    transport: &str,
    request: CallToolRequestParams,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let mut arguments = request.arguments.unwrap_or_default();
    let entry = catalog::entries()
        .into_iter()
        .find(|entry| entry.tool.name == request.name)
        .ok_or_else(|| {
            ErrorData::invalid_params("Unknown launcher tool", None)
        })?;
    catalog::validate(&entry, &arguments)?;
    let result = match request.name.as_ref() {
        "get_launcher_capabilities" => Ok(json!({
            "tools": catalog::entries().into_iter().map(|entry| json!({
                "name": entry.tool.name,
                "requires_session": entry.access != Access::Public,
                "requires_full_access": entry.access == Access::FullAccess,
                "read_only": entry.read_only,
                "returns_operation_id": entry.deferred,
            })).collect::<Vec<_>>(),
            "restrictions": {
                "ordinary_mode": {"manual_control_approval": true, "deletion": false, "direct_interface_control": false},
                "session_scoped_to_transport": true,
                "max_page_size": 100,
                "in_flight_mutations_finish_on_disconnect": true,
                "created_installation_jobs_continue_after_session_end": true,
                "running_games_continue_after_session_end": true,
                "retained_operation_limit": 64,
                "prepared_plan_ttl_seconds": 600,
                "prepared_plan_limit": 64,
            }
        })),
        "start_session" => shared
            .start_control(transport, context)
            .await
            .map_err(|error| ToolError::new("control_not_granted", error)),
        "end_session" => shared
            .release_control(
                transport,
                arguments["session_id"].as_str().unwrap(),
            )
            .map(|()| json!({"ended":true}))
            .map_err(|error| ToolError::new("invalid_session", error)),
        _ => {
            let control = arguments
                .remove("session_id")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let execution = ExecutionContext::new(
                execution_scope(shared, transport, &control),
                context.ct,
            );
            return shared
                .runtime
                .call(&request.name, arguments, execution)
                .await
                .map(encode)
                .map_err(|error| {
                    ErrorData::invalid_params(error.message, None)
                });
        }
    };
    Ok(encode(ToolOutput::from_result(&request.name, result)))
}
