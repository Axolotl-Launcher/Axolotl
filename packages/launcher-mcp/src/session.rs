use rmcp::transport::streamable_http_server::session::{
    SessionManager, local::LocalSessionManager,
};
use serde::Serialize;
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{Semaphore, oneshot};
use tokio_util::sync::CancellationToken;
#[derive(Clone, Debug, Serialize)]
pub struct Connection {
    pub id: String,
    pub client: String,
    pub connected: bool,
    pub awaiting_approval: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub running: bool,
    pub full_access: bool,
    pub require_auth: bool,
    pub endpoint: Option<String>,
    pub connection: Option<Connection>,
}

pub(crate) struct Slot {
    pub(crate) connection: Connection,
    pub(crate) approval: Option<oneshot::Sender<bool>>,
    pub(crate) cancel: CancellationToken,
    pub(crate) wire_id: Option<Arc<str>>,
    pub(crate) initialized: bool,
    pub(crate) control_id: Option<String>,
    pub(crate) last_control_id: Option<String>,
    pub(crate) control_cancel: Option<CancellationToken>,
}

pub(crate) struct Inner {
    pub(crate) snapshot: Snapshot,
    pub(crate) slots: HashMap<String, Slot>,
    pub(crate) active: Option<String>,
}

pub(crate) struct Shared {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) changed: Arc<dyn Fn(Snapshot) + Send + Sync>,
    pub(crate) sessions: Arc<LocalSessionManager>,
    pub(crate) token: String,
    pub(crate) info: crate::LauncherInfo,
    pub(crate) shutdown: CancellationToken,
    pub(crate) admission: Semaphore,
}

impl Shared {
    pub(crate) fn update<T>(&self, update: impl FnOnce(&mut Inner) -> T) -> T {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let result = update(&mut inner);
        inner.snapshot.revision += 1;
        inner.snapshot.connection = inner
            .active
            .as_ref()
            .and_then(|id| inner.slots.get(id))
            .map(|slot| slot.connection.clone());
        (self.changed)(inner.snapshot.clone());
        result
    }

    pub(crate) fn clear(&self, id: &str) -> Option<Arc<str>> {
        self.update(|inner| {
            let slot = inner.slots.remove(id)?;
            slot.cancel.cancel();
            if inner.active.as_deref() == Some(id) {
                inner.active = None;
            }
            slot.wire_id
        })
    }

    pub(crate) async fn disconnect(&self, id: &str) {
        if let Some(wire_id) = self.clear(id) {
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                self.sessions.close_session(&wire_id),
            )
            .await;
        }
    }
}
