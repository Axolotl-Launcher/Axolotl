use crate::{
    Connection,
    session::{Shared, Slot},
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, Implementation,
        InitializeRequestParams, InitializeResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
        ServerConfig, Tool,
    },
    service::{NotificationContext, RequestContext},
};
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) struct Handler {
    pub(crate) shared: Arc<Shared>,
    pub(crate) id: Mutex<Option<String>>,
}

impl Drop for Handler {
    fn drop(&mut self) {
        if let Some(id) =
            self.id.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            self.shared.clear(&id);
        }
    }
}

impl ServerHandler for Handler {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![
            ProtocolVersion::V_2025_11_25,
            ProtocolVersion::V_2025_06_18,
            ProtocolVersion::V_2025_03_26,
        ])
    }

    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::default();
        info.server_info = Implementation::new(
            "axolotl-launcher",
            self.shared.info.version.clone(),
        );
        info.capabilities =
            ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some("Discovery and get_launcher_info do not grant control. Call start_session only when the user wants to operate the launcher, then call end_session with the returned session_id when finished. Control sessions are scoped to this MCP connection; never reuse a previous session_id. Launcher control tools are not available yet.".into());
        info
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        crate::tools::definitions()
            .into_iter()
            .find(|tool| tool.name == name)
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut result = ListToolsResult::default();
        result.tools = crate::tools::definitions();
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let id = self
            .id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| {
                ErrorData::invalid_request("MCP handshake is incomplete", None)
            })?;
        crate::tools::call(&self.shared, &id, request, context)
            .await
            .map(Into::into)
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        let result = self.negotiate_initialize(&request)?;
        let id = Uuid::new_v4().to_string();
        let client: String = request
            .client_info
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(100)
            .collect();
        if client.trim().is_empty() {
            return Err(ErrorData::invalid_params(
                "Client name is required",
                None,
            ));
        }
        let cancel = self.shared.shutdown.child_token();
        self.shared.update(|inner| {
            inner.slots.insert(
                id.clone(),
                Slot {
                    connection: Connection {
                        id: id.clone(),
                        client,
                        connected: false,
                        awaiting_approval: false,
                    },
                    approval: None,
                    cancel: cancel.clone(),
                    wire_id: None,
                    initialized: false,
                    control_id: None,
                    last_control_id: None,
                    control_cancel: None,
                },
            );
        });
        *self.id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());
        let shared = Arc::downgrade(&self.shared);
        let initialize_id = id.clone();
        tokio::spawn(async move {
            tokio::select! { _ = cancel.cancelled() => return, _ = tokio::time::sleep(Duration::from_secs(60)) => {} }
            if let Some(shared) = shared.upgrade() {
                let pending = shared
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .slots
                    .get(&initialize_id)
                    .is_some_and(|slot| !slot.initialized);
                if pending {
                    shared.disconnect(&initialize_id).await;
                }
            }
        });
        context.peer.set_peer_info(request);
        Ok(result)
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        let Some(id) =
            self.id.lock().unwrap_or_else(|e| e.into_inner()).clone()
        else {
            return;
        };
        let cancel = self.shared.update(|inner| {
            inner
                .slots
                .get_mut(&id)
                .filter(|slot| !slot.initialized)
                .map(|slot| {
                    slot.initialized = true;
                    slot.approval = None;
                    slot.cancel.clone()
                })
        });
        let Some(cancel) = cancel else {
            return;
        };
        let shared = Arc::downgrade(&self.shared);
        tokio::spawn(async move {
            loop {
                tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(HEARTBEAT_INTERVAL) => {} }
                let result = tokio::select! { _ = cancel.cancelled() => break, result = tokio::time::timeout(HEARTBEAT_TIMEOUT, context.peer.send_request(rmcp::model::ServerRequest::PingRequest(Default::default()))) => result };
                if !matches!(result, Ok(Ok(_))) {
                    if let Some(shared) = shared.upgrade() {
                        shared.disconnect(&id).await;
                    }
                    break;
                }
            }
        });
    }
}
