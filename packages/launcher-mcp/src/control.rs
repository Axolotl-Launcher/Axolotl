use std::{sync::Arc, time::Duration};

use crate::session::Shared;

use rmcp::{RoleServer, service::RequestContext};
use serde_json::{Value, json};
use tokio::sync::oneshot;

fn session_result(id: &str, full_access: bool) -> Value {
    json!({ "session_id": id, "full_access": full_access, "permissions": { "deletion": full_access, "direct_interface_control": full_access } })
}

struct PendingControl {
    shared: Arc<Shared>,
    transport: String,
    control: String,
    committed: bool,
}

impl Drop for PendingControl {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.shared.release_control(&self.transport, &self.control);
        }
    }
}

impl Shared {
    pub(crate) fn authorize(
        &self,
        transport: &str,
        control: &str,
        full_access: bool,
    ) -> Result<tokio_util::sync::CancellationToken, crate::ToolError> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let slot = inner.slots.get(transport).filter(|slot| {
            inner.active.as_deref() == Some(transport)
                && slot.initialized
                && slot.connection.connected
                && !slot.connection.awaiting_approval
                && !slot.cancel.is_cancelled()
                && slot.control_id.as_deref() == Some(control)
        }).ok_or_else(|| crate::ToolError::new("session_required", "Call start_session and use this connection's approved session_id"))?;
        if full_access && !inner.snapshot.full_access {
            return Err(crate::ToolError::new(
                "full_access_required",
                "This tool requires full access mode",
            ));
        }
        slot.control_cancel
            .clone()
            .filter(|cancel| !cancel.is_cancelled())
            .ok_or_else(|| {
                crate::ToolError::new(
                    "session_revoked",
                    "Control session ended",
                )
            })
    }

    pub(crate) fn release_control(
        &self,
        transport: &str,
        control: &str,
    ) -> Result<(), String> {
        self.update(|inner| {
            let slot = inner
                .slots
                .get_mut(transport)
                .ok_or("MCP connection closed")?;
            if let Some(current) = &slot.control_id {
                if current != control {
                    return Err("Control session is stale".into());
                }
            } else if slot.last_control_id.as_deref() == Some(control) {
                return Ok(());
            } else {
                return Err(
                    "Control session does not belong to this connection".into(),
                );
            }
            slot.connection.connected = false;
            slot.connection.awaiting_approval = false;
            slot.control_id = None;
            slot.last_control_id = Some(control.to_owned());
            slot.approval = None;
            if let Some(cancel) = slot.control_cancel.take() {
                cancel.cancel();
            }
            if inner.active.as_deref() == Some(transport) {
                inner.active = None;
            }
            Ok(())
        })
    }

    pub(crate) async fn start_control(
        self: &Arc<Self>,
        transport: &str,
        context: RequestContext<RoleServer>,
    ) -> Result<Value, String> {
        let control = uuid::Uuid::new_v4().to_string();
        let (approval, answer) = oneshot::channel();
        let (existing, automatic, cancel) = self.update(|inner| {
            let slot = inner
                .slots
                .get_mut(transport)
                .ok_or("MCP connection closed")?;
            if !slot.initialized {
                return Err("MCP handshake is incomplete".to_string());
            }
            if let Some(active) = &inner.active {
                if active == transport && slot.connection.connected {
                    return Ok((
                        slot.control_id.clone(),
                        true,
                        slot.cancel.clone(),
                    ));
                }
                return Err(
                    "Another control session or approval request is active"
                        .into(),
                );
            }
            let automatic = inner.snapshot.full_access;
            slot.control_id = Some(control.clone());
            slot.connection.awaiting_approval = !automatic;
            let cancel = slot.cancel.child_token();
            slot.control_cancel = Some(cancel.clone());
            slot.approval = Some(approval);
            inner.active = Some(transport.to_owned());
            Ok((None, automatic, cancel))
        })?;
        if let Some(existing) = existing {
            return Ok(session_result(
                &existing,
                self.inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .snapshot
                    .full_access,
            ));
        }
        let mut pending = PendingControl {
            shared: self.clone(),
            transport: transport.into(),
            control: control.clone(),
            committed: false,
        };
        if !automatic {
            let allowed = tokio::select! {
                biased;
                _ = cancel.cancelled() => false,
                _ = context.ct.cancelled() => false,
                result = tokio::time::timeout(Duration::from_secs(45), answer) => matches!(result, Ok(Ok(true))),
            };
            if !allowed {
                return Err(
                    "Control request declined, cancelled, or expired".into()
                );
            }
        }
        if context.ct.is_cancelled() || cancel.is_cancelled() {
            return Err("Control request cancelled".into());
        }
        let full_access = self.update(|inner| {
            let slot = inner
                .slots
                .get_mut(transport)
                .filter(|slot| slot.control_id.as_deref() == Some(&control))
                .ok_or("Control request revoked")?;
            slot.connection.connected = true;
            slot.connection.awaiting_approval = false;
            slot.approval = None;
            Ok::<_, String>(inner.snapshot.full_access)
        })?;
        pending.committed = true;
        Ok(session_result(&control, full_access))
    }
}
