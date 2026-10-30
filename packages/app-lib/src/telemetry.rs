use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::State;
use crate::state::Settings;
use crate::state::store::Collection as _;
use crate::state::store::telemetry_identity::{
    Installation, TelemetryIdentity, TelemetryIdentityRecord,
};
use crate::state::store::telemetry_outbox::{
    TelemetryEventType, TelemetryOutbox, TelemetryOutboxRecord,
    TelemetryOutboxScope,
};
use crate::state::store::{Atom, atom};

const ENDPOINT: &str = "https://telemetry.axlmc.org/v1/batch";
const MAX_OUTBOX_EVENTS: i64 = 100;
const MAX_OUTBOX_BYTES: i64 = 2 * 1024 * 1024;
const MAX_EVENT_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;
const MAX_BATCH_EVENTS: usize = 10;
const MAX_BATCH_BYTES: usize = 60 * 1024;

static STARTED: AtomicBool = AtomicBool::new(false);
static WAKE_TX: OnceLock<tokio::sync::mpsc::Sender<()>> = OnceLock::new();

pub(crate) fn start(state: Arc<State>) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    let (wake_tx, mut wake_rx) = tokio::sync::mpsc::channel(1);
    let _ = WAKE_TX.set(wake_tx);
    state.spawn_background({
        let state = Arc::clone(&state);
        async move {
            loop {
                state.maintenance_gate.wait_until_resumed().await;
                let client = match crate::util::fetch::configured_client().await
                {
                    Ok(client) => client,
                    Err(error) => {
                        tracing::debug!(target: "theseus::telemetry", %error, "Telemetry client configuration failed");
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        continue;
                    }
                };
                if let Err(error) = run_cycle(&state, &client).await {
                    tracing::debug!(target: "theseus::telemetry", %error, "Telemetry cycle failed");
                }

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {},
                    _ = wake_rx.recv() => {},
                }
            }
        }
    });
}

pub async fn set_enabled(state: &State, enabled: bool) -> crate::Result<()> {
    let _database_permit = state.acquire_install_db_permit().await?;
    TelemetryOutbox::set_all(&state.data, &[]).await?;

    if enabled {
        ensure_identity(state).await?;
        enqueue_heartbeat(state).await?;
        wake();
    }
    Ok(())
}

pub fn notify_online() {
    wake();
}

async fn run_cycle(
    state: &State,
    client: &reqwest::Client,
) -> crate::Result<()> {
    if !is_enabled(state).await? {
        let _database_permit = state.acquire_install_db_permit().await?;
        TelemetryOutbox::set_all(&state.data, &[]).await?;
        return Ok(());
    }

    {
        let _database_permit = state.acquire_install_db_permit().await?;
        ensure_identity(state).await?;
        // Older clients queued error events this build never sends.
        TelemetryOutbox::set_scoped(
            &state.data,
            &TelemetryOutboxScope::Kind(TelemetryEventType::Error),
            &[],
        )
        .await?;
        enqueue_heartbeat(state).await?;
        cleanup_outbox(state).await?;
    }
    upload_next_batch(state, client).await?;
    Ok(())
}

async fn is_enabled(state: &State) -> crate::Result<bool> {
    let settings = Settings::get(&state.data).await?;
    Ok(settings.telemetry && settings.telemetry_consent_version > 0)
}

async fn ensure_identity(state: &State) -> crate::Result<String> {
    if let Some(record) =
        TelemetryIdentity::get(&state.data, &Installation).await?
    {
        return Ok(record.installation_id);
    }

    let mut unit = atom(&state.data).await?;
    if let Some(record) =
        TelemetryIdentity::atom_get(&mut unit, &Installation).await?
    {
        return Ok(record.installation_id);
    }

    let record = TelemetryIdentityRecord {
        installation_id: Uuid::new_v4().to_string(),
        last_heartbeat_day: None,
    };
    TelemetryIdentity::atom_set(&mut unit, &Installation, Some(&record))
        .await?;
    unit.commit().await?;
    Ok(record.installation_id)
}

async fn enqueue_heartbeat(state: &State) -> crate::Result<()> {
    let day = Utc::now().format("%Y-%m-%d").to_string();
    let mut unit = atom(&state.data).await?;
    let Some(identity) =
        TelemetryIdentity::atom_get(&mut unit, &Installation).await?
    else {
        return Err(crate::ErrorKind::OtherError(
            "The telemetry installation identity is missing".to_string(),
        )
        .into());
    };
    if identity.last_heartbeat_day.as_deref() == Some(&day) {
        return Ok(());
    }

    let event_id = Uuid::new_v4().to_string();
    let occurred_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let payload = heartbeat_payload(&event_id, &occurred_at, &day);
    let dedupe_key = format!("heartbeat:{day}");
    insert_outbox_event(&mut unit, &event_id, &dedupe_key, &payload).await?;
    let identity = TelemetryIdentityRecord {
        last_heartbeat_day: Some(day),
        ..identity
    };
    TelemetryIdentity::atom_set(&mut unit, &Installation, Some(&identity))
        .await?;
    unit.commit().await?;
    Ok(())
}

fn heartbeat_payload(event_id: &str, occurred_at: &str, day: &str) -> Value {
    json!({
        "type": "heartbeat",
        "event_id": event_id,
        "occurred_at": occurred_at,
        "day": day,
    })
}

async fn insert_outbox_event(
    unit: &mut Atom,
    event_id: &str,
    dedupe_key: &str,
    payload: &Value,
) -> crate::Result<()> {
    let payload = serde_json::to_string(payload)?;
    let size_bytes = payload.len() as i64;
    let now = Utc::now().timestamp();
    let record = TelemetryOutboxRecord {
        event_type: TelemetryEventType::Heartbeat,
        payload,
        created_at: now,
        next_attempt_at: now,
        attempts: 0,
        size_bytes,
        dedupe_key: dedupe_key.to_string(),
        occurrence_count: 1,
    };
    TelemetryOutbox::atom_set(unit, &event_id.to_string(), Some(&record)).await
}

async fn cleanup_outbox(state: &State) -> crate::Result<()> {
    let oldest = Utc::now().timestamp() - MAX_EVENT_AGE_SECONDS;
    TelemetryOutbox::trim(
        &state.data,
        oldest,
        MAX_OUTBOX_EVENTS,
        MAX_OUTBOX_BYTES,
    )
    .await
}

async fn upload_next_batch(
    state: &State,
    client: &reqwest::Client,
) -> crate::Result<()> {
    let scope = TelemetryOutboxScope::Due {
        kind: TelemetryEventType::Heartbeat,
        at: Utc::now().timestamp(),
    };
    let queued = TelemetryOutbox::get_scoped(&state.data, &scope).await?;
    if queued.is_empty() {
        return Ok(());
    }

    let mut events = Vec::new();
    let mut event_ids = Vec::new();
    let mut approximate_size = 0;
    for (event_id, record) in queued.into_iter().take(MAX_BATCH_EVENTS) {
        if approximate_size + record.payload.len() > MAX_BATCH_BYTES
            && !events.is_empty()
        {
            break;
        }
        let event: Value = serde_json::from_str(&record.payload)?;
        approximate_size += record.payload.len();
        events.push(event);
        event_ids.push(event_id);
    }

    let installation_id = {
        let _database_permit = state.acquire_install_db_permit().await?;
        ensure_identity(state).await?
    };
    let batch_id = stable_batch_id(&event_ids);
    let body = json!({
        "schema_version": 1,
        "batch_id": batch_id,
        "installation_id": installation_id,
        "app": {
            "version": env!("CARGO_PKG_VERSION"),
            "environment": if cfg!(debug_assertions) { "development" } else { "production" },
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
        "events": events,
    });
    let endpoint = std::env::var("THESEUS_TELEMETRY_ENDPOINT")
        .unwrap_or_else(|_| ENDPOINT.to_string());
    let response = client.post(endpoint).json(&body).send().await;
    match response {
        Ok(response) if response.status().is_success() => {
            delete_events(state, &event_ids).await?;
        }
        Ok(response)
            if response.status().is_client_error()
                && response.status().as_u16() != 429 =>
        {
            delete_events(state, &event_ids).await?;
        }
        _ => schedule_retry(state, &event_ids).await?,
    }
    Ok(())
}

async fn delete_events(
    state: &State,
    event_ids: &[String],
) -> crate::Result<()> {
    let _database_permit = state.acquire_install_db_permit().await?;
    let mut unit = atom(&state.data).await?;
    for event_id in event_ids {
        TelemetryOutbox::atom_set(&mut unit, event_id, None).await?;
    }
    unit.commit().await?;
    Ok(())
}

async fn schedule_retry(
    state: &State,
    event_ids: &[String],
) -> crate::Result<()> {
    let _database_permit = state.acquire_install_db_permit().await?;
    let now = Utc::now().timestamp();
    let mut unit = atom(&state.data).await?;
    for event_id in event_ids {
        let Some(stored) =
            TelemetryOutbox::atom_get(&mut unit, event_id).await?
        else {
            continue;
        };
        let attempts = stored.attempts + 1;
        let delay = match attempts {
            1 => 60,
            2 => 5 * 60,
            3 => 30 * 60,
            _ => 6 * 60 * 60,
        };
        let record = TelemetryOutboxRecord {
            next_attempt_at: now + delay,
            attempts,
            ..stored
        };
        TelemetryOutbox::atom_set(&mut unit, event_id, Some(&record)).await?;
    }
    unit.commit().await?;
    Ok(())
}

fn stable_batch_id(event_ids: &[String]) -> String {
    let mut hasher = Sha256::new();
    for event_id in event_ids {
        hasher.update(event_id.as_bytes());
        hasher.update([0]);
    }
    let digest = hex_digest(hasher.finalize().as_slice());
    format!(
        "{}-{}-{}-{}-{}",
        &digest[0..8],
        &digest[8..12],
        &digest[12..16],
        &digest[16..20],
        &digest[20..32]
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn wake() {
    if let Some(sender) = WAKE_TX.get() {
        let _ = sender.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_ids_are_stable() {
        let ids = vec!["one".to_string(), "two".to_string()];
        assert_eq!(stable_batch_id(&ids), stable_batch_id(&ids));
        assert_ne!(stable_batch_id(&ids), stable_batch_id(&ids[..1]));
    }

    #[test]
    fn heartbeat_payload_matches_the_strict_worker_shape() {
        let payload = heartbeat_payload(
            "11111111-1111-4111-8111-111111111111",
            "2026-08-17T00:00:00.000Z",
            "2026-08-17",
        );
        assert_eq!(payload.as_object().unwrap().len(), 4);
        assert!(payload.get("download_stats").is_none());
        assert_eq!(payload["type"], "heartbeat");
    }
}
