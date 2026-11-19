//! Installation identity, heartbeat day and bounded queue in one independent
//! document. Each update is one serialized publication; uploads hold no writer.

use super::persistence::{
    Persistence, document::Document, status::Availability,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const MAX_EVENTS: usize = 100;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_AGE: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Event {
    pub(crate) event_type: String,
    pub(crate) payload: String,
    pub(crate) created_at: i64,
    pub(crate) next_attempt_at: i64,
    pub(crate) attempts: i64,
    pub(crate) size_bytes: i64,
    pub(crate) dedupe_key: String,
    pub(crate) occurrence_count: i64,
}

pub(crate) struct TelemetryState {
    document: Document,
}

impl TelemetryState {
    pub(crate) fn new(persistence: &Persistence) -> Self {
        Self {
            document: persistence
                .document(
                    crate::state::layout::TELEMETRY,
                    "axolotl-telemetry",
                    1,
                )
                .expect("constant telemetry document path"),
        }
    }

    async fn read(&self) -> crate::Result<Map<String, Value>> {
        let result = self.document.read().await?;
        if !matches!(
            result.availability,
            Availability::Ready | Availability::Recovered
        ) {
            return Err(crate::ErrorKind::OtherError(
                "Telemetry state is unavailable".into(),
            )
            .into());
        }
        let snapshot = result.snapshot.expect("readable telemetry document");
        let data = snapshot.data().expect("telemetry payload").clone();
        if !data.get("outbox").is_some_and(Value::is_object) {
            return Err(crate::ErrorKind::OtherError(
                "Telemetry queue is unreadable".into(),
            )
            .into());
        }
        Ok(data)
    }

    async fn update<F>(&self, change: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Map<String, Value>) -> crate::Result<()>
            + Send
            + 'static,
    {
        // Missing after import is an unavailable authority, never a request to
        // recreate an old identity or queue. Startup performs the one import.
        self.read().await?;
        self.document
            .update(move |data| {
                queue(data)?;
                change(data)?;
                trim(queue(data)?)
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn ensure_identity(&self) -> crate::Result<String> {
        self.update(|data| {
            if data.get("installation_id").is_none_or(Value::is_null) {
                data.insert(
                    "installation_id".into(),
                    json!(uuid::Uuid::new_v4().to_string()),
                );
            }
            if !data.get("installation_id").is_some_and(Value::is_string) {
                return Err(crate::ErrorKind::InputError(
                    "Telemetry installation identity is invalid".into(),
                )
                .into());
            }
            Ok(())
        })
        .await?;
        self.read()
            .await?
            .get("installation_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                crate::ErrorKind::OtherError(
                    "Telemetry identity is missing".into(),
                )
                .into()
            })
    }

    pub(crate) async fn heartbeat(
        &self,
        day: String,
        event_id: String,
        payload: Value,
    ) -> crate::Result<()> {
        self.update(move |data| {
            if data.get("last_heartbeat_day").and_then(Value::as_str)
                == Some(&day)
            {
                return Ok(());
            }
            let events = queue(data)?;
            let dedupe_key = format!("heartbeat:{day}");
            if !events.values().any(|event| {
                event.get("dedupe_key").and_then(Value::as_str)
                    == Some(&dedupe_key)
            }) {
                let payload = serde_json::to_string(&payload)?;
                let now = Utc::now().timestamp();
                let event = Event {
                    event_type: "heartbeat".into(),
                    size_bytes: payload.len() as i64,
                    payload,
                    created_at: now,
                    next_attempt_at: now,
                    attempts: 0,
                    dedupe_key,
                    occurrence_count: 1,
                };
                events.insert(event_id, serde_json::to_value(event)?);
            }
            data.insert("last_heartbeat_day".into(), json!(day));
            Ok(())
        })
        .await
    }

    pub(crate) async fn clear(&self) -> crate::Result<()> {
        self.update(|data| {
            queue(data)?.clear();
            Ok(())
        })
        .await
    }

    pub(crate) async fn clean(&self) -> crate::Result<()> {
        self.update(|data| {
            queue(data)?.retain(|_, value| {
                value.get("event_type").and_then(Value::as_str) != Some("error")
            });
            Ok(())
        })
        .await
    }

    pub(crate) async fn due(&self) -> crate::Result<Vec<(String, Event)>> {
        let data = self.read().await?;
        let mut events = Vec::new();
        for (id, raw) in data["outbox"].as_object().expect("validated queue") {
            let Ok(event) = serde_json::from_value::<Event>(raw.clone()) else {
                continue;
            };
            if event.event_type == "heartbeat"
                && event.next_attempt_at <= Utc::now().timestamp()
                && event.attempts >= 0
                && uuid::Uuid::parse_str(id).is_ok()
            {
                events.push((id.clone(), event));
            }
        }
        events.sort_by(|left, right| {
            (left.1.created_at, &left.0).cmp(&(right.1.created_at, &right.0))
        });
        Ok(events)
    }

    pub(crate) async fn acknowledge(
        &self,
        sent: Vec<(String, Event)>,
        retry: bool,
    ) -> crate::Result<()> {
        self.update(move |data| {
            let queue = queue(data)?;
            for (id, event) in sent {
                let Some(stored) = queue.get_mut(&id) else {
                    continue;
                };
                // A response must not acknowledge a replacement queued while
                // the upload was in flight. Unknown sibling fields survive.
                if stored.get("payload").and_then(Value::as_str)
                    != Some(&event.payload)
                    || stored.get("dedupe_key").and_then(Value::as_str)
                        != Some(&event.dedupe_key)
                    || stored.get("attempts").and_then(Value::as_i64)
                        != Some(event.attempts)
                {
                    continue;
                }
                if retry {
                    let attempts = event.attempts.saturating_add(1);
                    let delay = match attempts {
                        1 => 60,
                        2 => 5 * 60,
                        3 => 30 * 60,
                        _ => 6 * 60 * 60,
                    };
                    stored["attempts"] = json!(attempts);
                    stored["next_attempt_at"] =
                        json!(Utc::now().timestamp() + delay);
                } else {
                    queue.remove(&id);
                }
            }
            Ok(())
        })
        .await
    }
}

fn queue(
    data: &mut Map<String, Value>,
) -> crate::Result<&mut Map<String, Value>> {
    data.get_mut("outbox")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Telemetry queue is unreadable".into())
                .into()
        })
}

fn trim(queue: &mut Map<String, Value>) -> crate::Result<()> {
    let oldest = Utc::now().timestamp() - MAX_AGE;
    queue.retain(|_, raw| {
        !known(raw)
            || raw
                .get("created_at")
                .and_then(Value::as_i64)
                .is_none_or(|created| created >= oldest)
    });
    let mut ordered = queue
        .iter()
        .filter(|(_, raw)| known(raw))
        .map(|(id, raw)| {
            (
                raw.get("created_at").and_then(Value::as_i64).unwrap_or(0),
                id.clone(),
            )
        })
        .collect::<Vec<_>>();
    ordered.sort();
    let mut bytes: usize = queue
        .values()
        .map(|raw| {
            raw.get("payload")
                .and_then(Value::as_str)
                .map_or(0, str::len)
        })
        .sum();
    for (_, id) in ordered {
        if queue.len() <= MAX_EVENTS && bytes <= MAX_BYTES {
            break;
        }
        if let Some(removed) = queue.remove(&id) {
            bytes = bytes.saturating_sub(
                removed
                    .get("payload")
                    .and_then(Value::as_str)
                    .map_or(0, str::len),
            );
        }
    }
    if queue.len() > MAX_EVENTS || bytes > MAX_BYTES {
        return Err(crate::ErrorKind::InputError(
            "Opaque telemetry events exceed the supported queue budget".into(),
        )
        .into());
    }
    Ok(())
}

fn known(raw: &Value) -> bool {
    matches!(
        raw.get("event_type").and_then(Value::as_str),
        Some("heartbeat" | "error")
    ) && serde_json::from_value::<Event>(raw.clone()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture() -> (tempfile::TempDir, Persistence, TelemetryState) {
        let root = tempfile::tempdir().unwrap();
        crate::state::store::legacy_documents::initialize(root.path()).await;
        let persistence = Persistence::new(root.path());
        let state = TelemetryState::new(&persistence);
        (root, persistence, state)
    }

    async fn enqueue(state: &TelemetryState, day: &str) {
        state.ensure_identity().await.unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        state.heartbeat(day.into(), id.clone(), json!({"type":"heartbeat","event_id":id,"occurred_at":"2026-11-15T00:00:00Z","day":day})).await.unwrap();
    }

    #[tokio::test]
    async fn identity_and_heartbeat_survive_reopen_and_reconcile_idempotently()
    {
        let (_root, persistence, state) = fixture().await;
        let id = state.ensure_identity().await.unwrap();
        enqueue(&state, "2026-11-15").await;
        enqueue(&state, "2026-11-15").await;
        let reopened = TelemetryState::new(&persistence);
        assert_eq!(reopened.ensure_identity().await.unwrap(), id);
        assert_eq!(reopened.due().await.unwrap().len(), 1);
        assert_eq!(
            reopened.read().await.unwrap()["last_heartbeat_day"],
            "2026-11-15"
        );
    }

    #[tokio::test]
    async fn retry_preserves_unknown_fields_and_late_acknowledgments_do_not_replace_events()
     {
        let (_root, _persistence, state) = fixture().await;
        enqueue(&state, "2026-11-15").await;
        let sent = state.due().await.unwrap();
        let id = sent[0].0.clone();
        state
            .document
            .update(move |data| {
                data["outbox"][&id]["future"] = json!(true);
                Ok(())
            })
            .await
            .unwrap();
        state.acknowledge(sent.clone(), true).await.unwrap();
        let data = state.read().await.unwrap();
        assert_eq!(data["outbox"][&sent[0].0]["attempts"], 1);
        assert_eq!(data["outbox"][&sent[0].0]["future"], true);
        state.acknowledge(sent, false).await.unwrap();
        assert_eq!(
            state.read().await.unwrap()["outbox"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn disabling_clears_queue_without_changing_identity_and_late_retries_stay_empty()
     {
        let (_root, _persistence, state) = fixture().await;
        enqueue(&state, "2026-11-15").await;
        let id = state.ensure_identity().await.unwrap();
        let sent = state.due().await.unwrap();
        state.clear().await.unwrap();
        state.acknowledge(sent, true).await.unwrap();
        assert!(state.due().await.unwrap().is_empty());
        assert_eq!(state.ensure_identity().await.unwrap(), id);
    }

    #[tokio::test]
    async fn queue_enforces_count_age_and_bytes_while_preserving_opaque_events()
    {
        let (_root, _persistence, state) = fixture().await;
        state
            .document
            .update(|data| {
                let now = Utc::now().timestamp();
                for index in 0..105 {
                    let id = uuid::Uuid::new_v4().to_string();
                    let event = Event {
                        event_type: "heartbeat".into(),
                        payload: "{}".into(),
                        created_at: now + index,
                        next_attempt_at: now,
                        attempts: 0,
                        size_bytes: 2,
                        dedupe_key: format!("day:{index}"),
                        occurrence_count: 1,
                    };
                    data["outbox"][&id] = serde_json::to_value(event)?;
                }
                data["outbox"]["opaque"] =
                    json!({"event_type":"future","payload":"opaque bytes"});
                data["outbox"]["expired"] = serde_json::to_value(Event {
                    event_type: "heartbeat".into(),
                    payload: "{}".into(),
                    created_at: 0,
                    next_attempt_at: 0,
                    attempts: 0,
                    size_bytes: 2,
                    dedupe_key: "expired".into(),
                    occurrence_count: 1,
                })?;
                Ok(())
            })
            .await
            .unwrap();
        state.clean().await.unwrap();
        let data = state.read().await.unwrap();
        assert_eq!(data["outbox"].as_object().unwrap().len(), MAX_EVENTS);
        assert!(data["outbox"].get("expired").is_none());
        assert_eq!(data["outbox"]["opaque"]["payload"], "opaque bytes");
    }

    #[tokio::test]
    async fn missing_taken_over_document_is_not_recreated_by_telemetry_maintenance()
     {
        let (_root, _persistence, state) = fixture().await;
        tokio::fs::remove_file(state.document.path()).await.unwrap();
        assert!(state.ensure_identity().await.is_err());
        assert!(!state.document.path().exists());
    }
}
