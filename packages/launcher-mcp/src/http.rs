use crate::session::Shared;
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::session::SessionManager;
use std::{sync::Arc, time::Duration};
pub(crate) async fn gate(
    State(shared): State<Arc<Shared>>,
    request: Request,
    next: Next,
) -> Response {
    let require_auth = shared
        .inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .snapshot
        .require_auth;
    if require_auth
        && request
            .headers()
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            != Some(format!("Bearer {}", shared.token).as_str())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if shared.shutdown.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let wire_id = request
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|header| header.to_str().ok())
        .map(Arc::<str>::from);
    if let Some(wire_id) = wire_id {
        let id = {
            let inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .slots
                .values()
                .find(|slot| slot.wire_id.as_ref() == Some(&wire_id))
                .map(|slot| slot.connection.id.clone())
        };
        let Some(id) = id else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let deleting = request.method() == axum::http::Method::DELETE;
        let response = next.run(request).await;
        if deleting && response.status().is_success() {
            shared.clear(&id);
        }
        return response;
    }
    if request.method() != axum::http::Method::POST {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(_admission) = shared.admission.try_acquire() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(body, 16 * 1024),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
    };
    let body: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(body) => body,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if body["method"] != "initialize" {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if shared
        .inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .slots
        .len()
        >= 16
    {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if let Some(wire) = response
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|header| header.to_str().ok())
    {
        let live = shared.update(|inner| {
            if let Some(slot) =
                inner.slots.values_mut().find(|slot| slot.wire_id.is_none())
            {
                slot.wire_id = Some(Arc::from(wire));
                true
            } else {
                false
            }
        });
        if !live {
            let _ = shared.sessions.close_session(&Arc::from(wire)).await;
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    response
}
