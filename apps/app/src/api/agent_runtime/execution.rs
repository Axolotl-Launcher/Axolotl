use super::{state::*, types::ToolRequest};
use agent_window::{
    AgentWindow,
    persistence::{Admission, ApprovalPermit, ExecutionRequest},
};
use launcher_tools::{
    Access, ExecutionContext, ExecutionScope, ToolDefinition, ToolError,
    ToolOutput, registry,
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

pub fn allowed(
    app: &AppHandle,
    run: &Run,
    entry: &ToolDefinition,
) -> Result<(), String> {
    let permissions = app.state::<AgentWindow>().permissions_now();
    if run.request.check_only && !entry.read_only {
        return Err("Verification runs cannot perform writes".into());
    }
    if (entry.access == Access::FullAccess || entry.destructive)
        && !permissions.full_access
    {
        return Err("Full access mode is required".into());
    }
    if entry.name.starts_with("ui_")
        && !(run.request.computer_use
            && permissions.full_access
            && permissions.computer_use)
    {
        return Err("Computer Use requires explicit task permission and both permission switches".into());
    }
    Ok(())
}

fn context(
    app: &AppHandle,
    run: &Arc<Run>,
    entry: &ToolDefinition,
    after_cancel: bool,
) -> ExecutionContext {
    let app = app.clone();
    let run = run.clone();
    let entry = entry.clone();
    let cancel = if after_cancel {
        CancellationToken::new()
    } else {
        run.cancel.clone()
    };
    let request_cancel = cancel.clone();
    let scope = ExecutionScope::new(run.scope_id.clone(), move |full| {
        let permissions = app.state::<AgentWindow>().permissions_now();
        allowed(&app, &run, &entry)
            .map_err(|message| ToolError::new("permission_revoked", message))?;
        if full && !permissions.full_access {
            return Err(ToolError::new(
                "full_access_required",
                "Full access was revoked",
            ));
        }
        Ok(cancel.clone())
    });
    ExecutionContext::new(scope, request_cancel)
}

pub async fn tool(
    app: &AppHandle,
    runtime: &AgentRuntime,
    run: Arc<Run>,
    request: ToolRequest,
) -> Result<Value, String> {
    let entry = registry::get(&request.name).ok_or("Unknown launcher tool")?;
    registry::validate(entry, &request.arguments).map_err(|e| e.to_string())?;
    allowed(app, &run, entry)?;
    let store = runtime.store().await?;
    let approval = if entry.read_only {
        None
    } else {
        store
            .approvals(&run.request.attempt_id)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|record| record.approval.call_id == request.call_id)
            .map(|record| ApprovalPermit {
                id: record.approval.id,
                revision: record.approval.revision,
            })
    };
    let admission = store
        .begin_execution(&ExecutionRequest {
            attempt_id: run.request.attempt_id.clone(),
            call_id: request.call_id.clone(),
            tool: request.name.clone(),
            arguments: Value::Object(request.arguments.clone()),
            is_write: !entry.read_only,
            approval,
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Admission::Existing(receipt) = admission {
        return receipt.result.ok_or("This tool call has an uncertain result; verify it instead of replaying it".into());
    }
    // Admission is durable before entering the business executor. Failure to
    // record the final receipt leaves an uncertain write, never a replay permit.
    if let Err(error) =
        budget(&run.tools, 256).and_then(|()| allowed(app, &run, entry))
    {
        store.finish_execution(&run.request.attempt_id, &request.call_id, &json!({"is_error":true,"structured_content":{"error":{"code":"not_started","message":error}}}), true).await.map_err(|e|e.to_string())?;
        return Err(error);
    }
    let tools = app.state::<crate::api::tools::LauncherTools>().0.clone();
    let mut attempts = 0;
    let mut output = loop {
        let output = tools
            .call(
                &request.name,
                request.arguments.clone(),
                context(app, &run, entry, false),
            )
            .await
            .map_err(|e| e.to_string())?;
        let transient = output.structured_content["error"]["code"]
            .as_str()
            .is_some_and(|code| {
                matches!(
                    code,
                    "network_error"
                        | "temporarily_unavailable"
                        | "rate_limited"
                )
            });
        if !entry.read_only
            || !entry.idempotent
            || !transient
            || attempts >= 2
            || run.cancel.is_cancelled()
        {
            break output;
        }
        attempts += 1;
        tokio::time::sleep(Duration::from_millis(150 * attempts)).await;
        allowed(app, &run, entry)?;
    };
    if let Some(operation) = output.structured_content["operation_id"]
        .as_str()
        .map(str::to_owned)
        && matches!(
            output.structured_content["status"].as_str(),
            Some("queued" | "running")
        )
    {
        store
            .execution_progress(
                &run.request.attempt_id,
                &request.call_id,
                &serde_json::to_value(&output).map_err(|e| e.to_string())?,
            )
            .await
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        loop {
            if started.elapsed() > Duration::from_secs(300) {
                return Err(format!(
                    "Operation {operation} has an unconfirmed result; verify it before retrying"
                ));
            }
            allowed(app, &run, entry)?;
            let observed = tools
                .call(
                    "operations_get",
                    json!({"operation_id": operation})
                        .as_object()
                        .unwrap()
                        .clone(),
                    context(app, &run, entry, true),
                )
                .await
                .map_err(|e| e.to_string())?;
            if observed.is_error {
                return Err(
                    "Operation result is unavailable; do not replay the write"
                        .into(),
                );
            }
            match observed.structured_content["status"].as_str() {
                Some("succeeded" | "failed") => {
                    output = ToolOutput {
                        is_error: observed.structured_content["status"]
                            == "failed",
                        ..observed
                    };
                    break;
                }
                _ => tokio::time::sleep(Duration::from_millis(400)).await,
            }
        }
    }
    let safe = agent_window::persistence::redact(
        serde_json::to_value(&output).map_err(|e| e.to_string())?,
    );
    store
        .finish_execution(
            &run.request.attempt_id,
            &request.call_id,
            &safe,
            output.is_error,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe)
}
