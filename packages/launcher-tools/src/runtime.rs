use crate::{
    Backend, BackendContext, Command, LauncherInfo, ToolError, ToolOutput,
    registry,
};
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct InputError {
    pub message: String,
}

impl InputError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InputError {}

type Authorization =
    dyn Fn(bool) -> Result<CancellationToken, ToolError> + Send + Sync;

/// Trusted host-created ownership and authorization, never deserialized from tool arguments.
/// A scope is not a connection: local callers need no handshake or control-session lifecycle.
#[derive(Clone)]
pub struct ExecutionScope {
    id: String,
    authorization: Arc<Authorization>,
}

impl ExecutionScope {
    pub fn new(
        id: String,
        authorization: impl Fn(bool) -> Result<CancellationToken, ToolError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            id,
            authorization: Arc::new(authorization),
        }
    }

    /// Creates an isolated local operation owner with host-selected permissions.
    pub fn local(full_access: bool, cancel: CancellationToken) -> Self {
        Self::new(format!("local:{}", uuid::Uuid::new_v4()), move |full| {
            if full && !full_access {
                return Err(ToolError::new(
                    "full_access_required",
                    "This tool requires full access mode",
                ));
            }
            Ok(cancel.clone())
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn authorize(&self, full_access: bool) -> Result<(), ToolError> {
        self.token(full_access).map(|_| ())
    }

    pub(crate) fn token(
        &self,
        full_access: bool,
    ) -> Result<CancellationToken, ToolError> {
        let token = (self.authorization)(full_access)?;
        if token.is_cancelled() {
            return Err(ToolError::new(
                "session_revoked",
                "Control session ended",
            ));
        }
        Ok(token)
    }
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub scope: ExecutionScope,
    pub request_cancel: CancellationToken,
}

impl ExecutionContext {
    pub fn new(
        scope: ExecutionScope,
        request_cancel: CancellationToken,
    ) -> Self {
        Self {
            scope,
            request_cancel,
        }
    }
}

/// One instance is shared by all entry points so writes, plans and operations remain coordinated.
pub struct ToolRuntime {
    pub(crate) backend: Arc<dyn Backend>,
    pub(crate) mutations: tokio::sync::Mutex<()>,
    pub(crate) operations: Mutex<crate::operations::Operations>,
    info: LauncherInfo,
}

impl ToolRuntime {
    pub fn new(info: LauncherInfo, backend: Arc<dyn Backend>) -> Self {
        Self {
            info,
            backend,
            mutations: tokio::sync::Mutex::new(()),
            operations: Mutex::new(Default::default()),
        }
    }

    pub fn info(&self) -> &LauncherInfo {
        &self.info
    }

    pub async fn call(
        self: &Arc<Self>,
        name: &str,
        arguments: Map<String, Value>,
        context: ExecutionContext,
    ) -> Result<ToolOutput, InputError> {
        let entry = registry::get(name)
            .ok_or_else(|| InputError::new("Unknown launcher tool"))?;
        registry::validate(entry, &arguments)?;
        let result = match name {
            "get_launcher_info" => {
                Ok(serde_json::to_value(&self.info).expect("launcher metadata"))
            }
            "get_launcher_capabilities" => {
                Ok(json!({"tools": registry::catalog()}))
            }
            "operations_get" => self.operation_get(
                &context,
                arguments["operation_id"].as_str().unwrap(),
            ),
            "operations_list" => self.operation_list(&context),
            _ => {
                let command = serde_json::from_value::<Command>(
                    json!({"name": name, "arguments": arguments}),
                )
                .map_err(|error| InputError::new(error.to_string()))?;
                if entry.execution.deferred {
                    self.enqueue(&context, entry, command)
                } else {
                    self.execute(context, entry, command).await
                }
            }
        };
        Ok(ToolOutput::from_result(name, result))
    }

    async fn execute(
        self: &Arc<Self>,
        context: ExecutionContext,
        entry: &registry::ToolDefinition,
        command: Command,
    ) -> Result<Value, ToolError> {
        let full = entry.access == registry::Access::FullAccess;
        let cancel = context.scope.token(full)?;
        let backend_context = BackendContext::new(context.scope.clone());
        if entry.read_only {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(ToolError::new("session_revoked", "Control session ended")),
                _ = context.request_cancel.cancelled() => Err(ToolError::new("cancelled", "Request cancelled")),
                result = self.backend.execute(command, backend_context) => result,
            }
        } else {
            let runtime = self.clone();
            let interrupts = entry.execution.interrupts_mutation;
            // Detached admission drains writes even when the calling transport drops its future.
            tokio::spawn(async move {
                let _permit = if interrupts { None } else { Some(tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(ToolError::new("session_revoked", "Control session ended before execution")),
                    _ = context.request_cancel.cancelled() => return Err(ToolError::new("cancelled", "Request cancelled before execution")),
                    permit = runtime.mutations.lock() => permit,
                }) };
                context.scope.authorize(full)?;
                if context.request_cancel.is_cancelled() {
                    return Err(ToolError::new("cancelled", "Request cancelled before execution"));
                }
                runtime.backend.execute(command, backend_context).await
            }).await.map_err(|_| ToolError::new("execution_failed", "Launcher operation task failed; inspect current state before retrying"))?
        }
    }
}
