use super::{
    native_jobs::{self, Completion},
    state::*,
    types::ToolRequest,
};
use agent_window::{
    AgentWindow,
    persistence::{Admission, ApprovalPermit, ExecutionRequest},
};
use launcher_tools::{
    Access, ExecutionContext, ExecutionScope, ToolDefinition, ToolError,
    registry,
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

/// Planning owns cached handles but does not change installed user content.
/// Keep registry read-only/locking semantics separate from native consent.
pub(super) fn requires_approval(entry: &ToolDefinition) -> bool {
    !entry.read_only
        && !matches!(
            entry.name.as_ref(),
            "resource_prepare_install"
                | "resource_prepare_update"
                | "resource_prepare_switch"
                | "resource_revise_plan"
                | "modpack_prepare_install"
                | "modpack_prepare_local"
        )
}

/// Models often spell absent optional values as null (or an empty cursor).
/// Canonicalize before archiving/binding calls; required and unknown fields
/// still go through the registry's normal validation.
pub(super) fn normalize_model_arguments(reply: &mut Value, tools: &Value) {
    for call in reply["tool_calls"].as_array_mut().into_iter().flatten() {
        let Some(entry) = call["name"].as_str().and_then(registry::get) else {
            continue;
        };
        let Some(arguments) = call["args"].as_object_mut() else {
            continue;
        };
        let required = entry.input_schema["required"].as_array();
        let properties = &entry.input_schema["properties"];
        let paged = entry.read_only
            && matches!(
                entry.name.as_ref(),
                "resource_search" | "resource_list_releases"
            );
        let cursor_rule = tools
            .as_array()
            .into_iter()
            .flatten()
            .find(|tool| {
                tool["function"]["name"].as_str() == Some(entry.name.as_ref())
            })
            .and_then(|tool| {
                tool["function"]["parameters"]["properties"].get("cursor")
            });
        arguments.retain(|name, value| {
            // The runtime offers paging only after observing a returned token.
            // An unoffered cursor/first-page marker is omitted before binding
            // this readonly call, never forwarded as a guessed continuation.
            if name == "cursor"
                && paged
                && (cursor_rule.is_none()
                    || cursor_rule
                        .and_then(|rule| rule["enum"].as_array())
                        .is_some_and(|allowed| !allowed.contains(value))
                    || value.as_str().is_some_and(|cursor| {
                        cursor.eq_ignore_ascii_case("initial")
                    }))
            {
                return false;
            }
            let optional = properties.get(name).is_some()
                && !required
                    .is_some_and(|keys| keys.iter().any(|key| key == name));
            !(optional
                && (value.is_null()
                    || (value.as_str() == Some("")
                        && properties[name]["minLength"]
                            .as_u64()
                            .is_some_and(|min| min > 0))))
        });
    }
}

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

/// These native controls may observe/stop ongoing work without granting a new
/// business mutation. The classification is never supplied by the worker/model.
pub(super) fn recovery_control(entry: &ToolDefinition) -> bool {
    entry.execution.interrupts_mutation
        || matches!(
            entry.name.as_ref(),
            "install_job_cancel" | "download_job_cancel" | "backup_cancel"
        )
}

pub(super) fn available_with_unsettled(
    entry: &ToolDefinition,
    unsettled: bool,
) -> bool {
    !unsettled || entry.read_only || recovery_control(entry)
}

async fn write_permit<'a>(
    serial: &'a tokio::sync::Mutex<()>,
    cancel: &CancellationToken,
) -> Result<tokio::sync::MutexGuard<'a, ()>, String> {
    let guard = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err("Task stopped before write admission".into()),
        guard = serial.lock() => guard,
    };
    if cancel.is_cancelled() {
        return Err("Task stopped before write admission".into());
    }
    Ok(guard)
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
    if let Err(error) = super::resource_targets::validate(&request).await {
        return serde_json::to_value(launcher_tools::ToolOutput::from_result(
            &request.name,
            Err(error),
        ))
        .map_err(|e| e.to_string());
    }
    // Hold ordinary writes through native completion. Explicit stop/cancel tools
    // bypass this lock so they can stop the very job the ordinary write awaits.
    let _write = if !entry.read_only && !recovery_control(entry) {
        Some(write_permit(&run.write_serial, &run.cancel).await?)
    } else {
        None
    };
    let store = runtime.store().await?;
    if !entry.read_only {
        reconcile(app, runtime, &run.request.thread_id).await?;
    }
    let approval = if !requires_approval(entry) {
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
    if run.cancel.is_cancelled() {
        return Err("Task stopped before tool admission".into());
    }
    let admission = store
        .begin_execution(&ExecutionRequest {
            attempt_id: run.request.attempt_id.clone(),
            call_id: request.call_id.clone(),
            tool: request.name.clone(),
            arguments: Value::Object(request.arguments.clone()),
            is_write: requires_approval(entry),
            recovery_control: recovery_control(entry),
            approval,
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Admission::Existing(receipt) = admission {
        if !matches!(receipt.state.as_str(), "completed" | "failed") {
            return Err("This tool call has an uncertain result; verify it instead of replaying it".into());
        }
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
    let output = loop {
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
    let mut safe = agent_window::persistence::redact(
        serde_json::to_value(&output).map_err(|e| e.to_string())?,
    );
    let arguments = Value::Object(request.arguments.clone());
    let started = Instant::now();
    let failed = loop {
        // Persist every admission before polling or checking cancellation. Recovery
        // can then observe the exact native job without repeating its producer.
        store
            .execution_progress(
                &run.request.attempt_id,
                &request.call_id,
                &safe,
            )
            .await
            .map_err(|e| e.to_string())?;
        let query = match native_jobs::classify(&request.name, &arguments, &safe) {
            Completion::Finished { failed } => break failed,
            Completion::Unconfirmed | Completion::Attention(_) => return Err("Native result needs verification or user attention; do not replay the write".into()),
            Completion::Pending(query) => query,
        };
        if started.elapsed() > Duration::from_secs(300)
            || run.cancel.is_cancelled()
        {
            return Err(
                "Native work is still unconfirmed; verify it before retrying"
                    .into(),
            );
        }
        allowed(app, &run, entry)?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let observed = tools
            .call(
                query.tool,
                query.arguments.clone(),
                context(app, &run, entry, true),
            )
            .await
            .map_err(|e| e.to_string())?;
        if !native_jobs::observe(
            &mut safe,
            &query,
            serde_json::to_value(observed).map_err(|e| e.to_string())?,
        ) {
            return Err(
                "Operation result is unavailable; do not replay the write"
                    .into(),
            );
        }
        safe = agent_window::persistence::redact(safe);
    };
    safe["is_error"] = json!(failed);
    store
        .finish_execution(
            &run.request.attempt_id,
            &request.call_id,
            &safe,
            failed,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe)
}

/// Recovery can observe retained native operations, but never calls the original
/// mutation. Evicted operations and missing identities stay unresolved.
pub async fn reconcile(
    app: &AppHandle,
    runtime: &AgentRuntime,
    thread_id: &str,
) -> Result<(), String> {
    let store = runtime.store().await?;
    let tools = app.state::<crate::api::tools::LauncherTools>().0.clone();
    for pending in store
        .unsettled_writes(thread_id)
        .await
        .map_err(|e| e.to_string())?
    {
        let attempt = pending["attempt_id"]
            .as_str()
            .ok_or("Missing attempt identity")?;
        let call =
            pending["call_id"].as_str().ok_or("Missing call identity")?;
        let tool = pending["tool"].as_str().ok_or("Missing tool identity")?;
        let Some(previous) =
            pending.get("result").filter(|result| result.is_object())
        else {
            continue;
        };
        let mut receipt = previous.clone();
        let arguments = &pending["arguments"];
        let scope = ExecutionScope::new(format!("agent:{attempt}"), |_| {
            Ok(CancellationToken::new())
        });
        // At most the retained wrapper and one native observation per reconcile.
        // Running jobs stay unresolved without blocking follow-up submission.
        for _ in 0..2 {
            match native_jobs::classify(tool, arguments, &receipt) {
                Completion::Finished { .. } | Completion::Unconfirmed => break,
                Completion::Pending(query) | Completion::Attention(query) => {
                    let observed = tools
                        .call(
                            query.tool,
                            query.arguments.clone(),
                            ExecutionContext::new(
                                scope.clone(),
                                CancellationToken::new(),
                            ),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    if !native_jobs::observe(
                        &mut receipt,
                        &query,
                        serde_json::to_value(observed)
                            .map_err(|e| e.to_string())?,
                    ) {
                        break;
                    }
                }
            }
        }
        let failed = match native_jobs::classify(tool, arguments, &receipt) {
            Completion::Finished { failed } => Some(failed),
            _ => None,
        };
        if let Some(failed) = failed {
            receipt["is_error"] = json!(failed);
        }
        store
            .reconcile_execution_receipt(
                attempt, call, previous, &receipt, failed,
            )
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn only_canonical_controls_survive_unknown_write_catalog_filter() {
        for entry in registry::catalog() {
            let control = entry.execution.interrupts_mutation
                || matches!(
                    entry.name.as_ref(),
                    "install_job_cancel"
                        | "download_job_cancel"
                        | "backup_cancel"
                );
            assert_eq!(recovery_control(entry), control);
            assert_eq!(
                available_with_unsettled(entry, true),
                entry.read_only || control
            );
            assert!(available_with_unsettled(entry, false));
        }
        for name in [
            "install_job_cancel",
            "download_job_cancel",
            "backup_cancel",
            "server_stop",
            "server_kill",
            "multiplayer_stop",
            "multiplayer_reset",
        ] {
            assert!(recovery_control(registry::get(name).unwrap()), "{name}");
        }
        for name in [
            "instance_create",
            "java_install",
            "backup_create",
            "instance_rename",
        ] {
            assert!(
                !available_with_unsettled(registry::get(name).unwrap(), true),
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn waiting_write_does_not_start_after_task_is_cancelled() {
        let serial = Arc::new(tokio::sync::Mutex::new(()));
        let held = serial.lock().await;
        let cancel = CancellationToken::new();
        let started = Arc::new(AtomicBool::new(false));
        let waiter = tokio::spawn({
            let serial = serial.clone();
            let cancel = cancel.clone();
            let started = started.clone();
            async move {
                let _permit = write_permit(&serial, &cancel).await?;
                started.store(true, Ordering::Release);
                Ok::<(), String>(())
            }
        });
        tokio::task::yield_now().await;
        cancel.cancel();
        drop(held);
        assert!(waiter.await.unwrap().is_err());
        assert!(!started.load(Ordering::Acquire));
        assert!(write_permit(&serial, &cancel).await.is_err());
    }
}
