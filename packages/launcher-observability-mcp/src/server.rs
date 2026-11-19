//! Loopback HTTP transport for the observability MCP.
//!
//! The shape mirrors `launcher-mcp`: a streamable HTTP service behind one gate
//! middleware. It is deliberately smaller because these tools are read-only and
//! stateless: there is no approval flow, no control session and no admission
//! semaphore, so a client that connects cannot change anything.

use crate::{protocol, source::Source};
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, Implementation,
        InitializeRequestParams, InitializeResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
        ServerConfig, Tool,
    },
    service::RequestContext,
    transport::{
        StreamableHttpServerConfig,
        streamable_http_server::{
            session::local::LocalSessionManager, tower::StreamableHttpService,
        },
    },
};
use serde::Serialize;
use std::{borrow::Cow, sync::Arc, time::Duration};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// What the launcher shows in its settings page.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub running: bool,
    pub require_auth: bool,
    pub endpoint: Option<String>,
}

struct Shared {
    source: Arc<dyn Source>,
    /// Precomputed `Bearer <token>`, so the gate does no allocation.
    expected_authorization: String,
    require_auth: bool,
    version: String,
    shutdown: CancellationToken,
    snapshot: std::sync::Mutex<Snapshot>,
}

/// A running observability MCP listener.
pub struct Server {
    shared: Arc<Shared>,
    task: Option<JoinHandle<()>>,
}

impl Server {
    /// Bind on loopback. A `require_auth` server rejects every request whose
    /// `Authorization: Bearer` header does not match `token`.
    pub async fn start(
        port: u16,
        token: String,
        require_auth: bool,
        version: impl Into<String>,
        source: Arc<dyn Source>,
    ) -> std::io::Result<Self> {
        let listener =
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            source,
            expected_authorization: format!("Bearer {token}"),
            require_auth,
            version: version.into(),
            shutdown: CancellationToken::new(),
            snapshot: std::sync::Mutex::new(Snapshot {
                running: true,
                require_auth,
                endpoint: Some(format!("http://127.0.0.1:{port}/mcp")),
            }),
        });

        let mut sessions = LocalSessionManager::default();
        sessions.session_config.init_timeout = Some(Duration::from_secs(60));
        sessions.session_config.keep_alive = Some(Duration::from_secs(90));
        let factory = shared.clone();
        let transport = StreamableHttpService::new(
            move || {
                Ok(Handler {
                    shared: factory.clone(),
                })
            },
            Arc::new(sessions),
            StreamableHttpServerConfig::default()
                .with_cancellation_token(shared.shutdown.clone())
                .with_json_response(true)
                .with_max_request_body_bytes(1024 * 1024)
                .enforce_origin_validation(),
        );
        let app = Router::new()
            .nest_service("/mcp", transport)
            .layer(middleware::from_fn_with_state(shared.clone(), gate));

        let shutdown = shared.shutdown.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
            {
                tracing::warn!(%error, "Observability MCP listener failed");
            }
        });

        Ok(Self {
            shared,
            task: Some(task),
        })
    }

    /// Current transport state.
    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Stop the listener. Later calls are no-ops.
    pub async fn stop(mut self) {
        self.shared.shutdown.cancel();
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .running = false;
        if let Some(mut task) = self.task.take()
            && tokio::time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
        {
            task.abort();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.shutdown.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn gate(
    State(shared): State<Arc<Shared>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if shared.shutdown.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if shared.require_auth {
        let presented = request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .unwrap_or_default();
        if !token_matches(presented, &shared.expected_authorization) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    next.run(request).await
}

/// Compare the presented header against the expected one in constant time, so
/// a local client cannot learn the token by timing the failure.
fn token_matches(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }
    presented
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

struct Handler {
    shared: Arc<Shared>,
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
            "axolotl-observability",
            self.shared.version.clone(),
        );
        info.capabilities =
            ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "Read-only telemetry recorded by the local launcher. The data is limited to what the launcher already redacts and is only available on this computer. Start a listing with its default limit before raising it.".into(),
        );
        info
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        self.negotiate_initialize(&request)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        protocol::tools().into_iter().find(|tool| tool.name == name)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: protocol::tools(),
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        protocol::handle(&self.shared.source, request, context).await
    }
}
