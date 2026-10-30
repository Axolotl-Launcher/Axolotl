use crate::{
    BackendContext, Command, ExecutionContext, ToolError, ToolRuntime,
};
use serde_json::{Value, json};
use std::{collections::VecDeque, sync::Arc};

#[derive(Default)]
pub(crate) struct Operations(VecDeque<Operation>);

#[cfg(test)]
impl Operations {
    pub(crate) fn status_for_test(&self, id: &str) -> Option<&'static str> {
        self.0.iter().find(|op| op.id == id).map(|op| op.status)
    }
}

struct Operation {
    id: String,
    control: String,
    tool: String,
    status: &'static str,
    result: Option<Value>,
}

impl Operation {
    fn value(&self) -> Value {
        json!({"operation_id":self.id,"tool":self.tool,"status":self.status,"result":self.result})
    }

    fn summary(&self) -> Value {
        let mut value = self.value();
        if value["result"]["image"].is_object() {
            value["result"]["image"] = serde_json::json!({"available":true,"retrieve_with":"operations_get"});
        }
        value
    }
}

impl ToolRuntime {
    pub(crate) fn operation_get(
        &self,
        context: &ExecutionContext,
        id: &str,
    ) -> Result<Value, ToolError> {
        context.scope.authorize(false)?;
        self.operations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .iter()
            .find(|op| op.id == id && op.control == context.scope.id())
            .map(Operation::value)
            .ok_or_else(|| {
                ToolError::new(
                    "not_found",
                    "Operation not found in this control session",
                )
            })
    }

    pub(crate) fn operation_list(
        &self,
        context: &ExecutionContext,
    ) -> Result<Value, ToolError> {
        context.scope.authorize(false)?;
        Ok(
            json!({"items":self.operations.lock().unwrap_or_else(|e| e.into_inner()).0.iter().rev()
            .filter(|op| op.control == context.scope.id()).map(Operation::summary).collect::<Vec<_>>()}),
        )
    }

    pub(crate) fn enqueue(
        self: &Arc<Self>,
        context: &ExecutionContext,
        entry: &crate::registry::ToolDefinition,
        command: Command,
    ) -> Result<Value, ToolError> {
        if context.request_cancel.is_cancelled() {
            return Err(ToolError::new(
                "cancelled",
                "Request cancelled before acceptance",
            ));
        }
        let full = entry.access == crate::registry::Access::FullAccess;
        let cancel = context.scope.token(full)?;
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut ops =
                self.operations.lock().unwrap_or_else(|e| e.into_inner());
            if ops.0.len() >= 64 {
                let oldest = ops
                    .0
                    .iter()
                    .position(|op| !matches!(op.status, "queued" | "running"))
                    .ok_or_else(|| {
                        ToolError::new(
                            "too_many_operations",
                            "Wait for current operations to finish",
                        )
                    })?;
                ops.0.remove(oldest);
            }
            ops.0.push_back(Operation {
                id: id.clone(),
                control: context.scope.id().into(),
                tool: entry.name.to_string(),
                status: "queued",
                result: None,
            });
        }
        let shared = self.clone();
        let context = context.clone();
        let interrupts = entry.execution.interrupts_mutation;
        let operation_id = id.clone();
        tokio::spawn(async move {
            let result = async {
                let _permit = if interrupts { None } else { Some(tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(ToolError::new("session_revoked", "Control session ended before execution")),
                    permit = shared.mutations.lock() => permit,
                }) };
                context.scope.authorize(full)?;
                shared.operation_update(&operation_id, "running", None);
                // Catch backend panics too: an accepted operation must not remain
                // permanently running if a business implementation panics.
                let backend = shared.backend.clone();
                let context = BackendContext::new(context.scope);
                tokio::spawn(async move { backend.execute(command, context).await }).await
                    .map_err(|_| ToolError::new("execution_failed", "Operation task failed; inspect launcher state before retrying"))?
            }.await;
            match result {
                Ok(value) => shared.operation_update(
                    &operation_id,
                    "succeeded",
                    Some(value),
                ),
                Err(error) => shared.operation_update(
                    &operation_id,
                    if matches!(
                        error.code,
                        "session_revoked" | "session_required" | "cancelled"
                    ) {
                        "cancelled"
                    } else {
                        "failed"
                    },
                    Some(json!({"error":error})),
                ),
            }
        });
        Ok(json!({"operation_id":id,"status":"queued"}))
    }

    fn operation_update(
        &self,
        id: &str,
        status: &'static str,
        result: Option<Value>,
    ) {
        if let Some(op) = self
            .operations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .iter_mut()
            .find(|op| op.id == id)
        {
            op.status = status;
            op.result = result;
        }
    }
}
