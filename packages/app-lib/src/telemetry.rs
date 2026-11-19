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
use crate::state::telemetry_state::TelemetryState;

const ENDPOINT: &str = "https://telemetry.axlmc.org/v1/batch";
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
    let telemetry = TelemetryState::new(&state.persistence);
    telemetry.clear().await?;
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
    reconcile(state).await?;
    if is_enabled(state).await? {
        upload_next_batch(state, client).await?;
    }
    Ok(())
}

/// Identity, heartbeat day and queue updates each publish one file transaction.
/// No install database permit is involved in optional telemetry maintenance.
pub(crate) async fn reconcile(state: &State) -> crate::Result<()> {
    let telemetry = TelemetryState::new(&state.persistence);
    if !is_enabled(state).await? {
        return telemetry.clear().await;
    }
    ensure_identity(state).await?;
    telemetry.clean().await?;
    enqueue_heartbeat(state).await
}

async fn is_enabled(state: &State) -> crate::Result<bool> {
    let settings = Settings::get(&state.data).await?;
    Ok(settings.telemetry && settings.telemetry_consent_version > 0)
}

async fn ensure_identity(state: &State) -> crate::Result<String> {
    TelemetryState::new(&state.persistence)
        .ensure_identity()
        .await
}

async fn enqueue_heartbeat(state: &State) -> crate::Result<()> {
    let day = Utc::now().format("%Y-%m-%d").to_string();
    let id = Uuid::new_v4().to_string();
    let occurred = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let payload = heartbeat_payload(&id, &occurred, &day);
    TelemetryState::new(&state.persistence)
        .heartbeat(day, id, payload)
        .await
}

fn heartbeat_payload(event_id: &str, occurred_at: &str, day: &str) -> Value {
    json!({
        "type": "heartbeat",
        "event_id": event_id,
        "occurred_at": occurred_at,
        "day": day,
    })
}

async fn upload_next_batch(
    state: &State,
    client: &reqwest::Client,
) -> crate::Result<()> {
    let telemetry = TelemetryState::new(&state.persistence);
    let queued = telemetry.due().await?;
    if queued.is_empty() {
        return Ok(());
    }

    let mut events = Vec::new();
    let mut event_ids = Vec::new();
    let mut sent = Vec::new();
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
        event_ids.push(event_id.clone());
        sent.push((event_id, record));
    }

    let installation_id = ensure_identity(state).await?;
    if !is_enabled(state).await? {
        return Ok(());
    }
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
            telemetry.acknowledge(sent, false).await?;
        }
        Ok(response)
            if response.status().is_client_error()
                && response.status().as_u16() != 429 =>
        {
            telemetry.acknowledge(sent, false).await?;
        }
        _ => telemetry.acknowledge(sent, true).await?,
    }
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

#[cfg(test)]
mod reconcile_tests {
    use super::*;

    async fn fixture(enabled: bool) -> (tempfile::TempDir, Arc<State>) {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "telemetry-test".into(),
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query(
            "UPDATE settings SET telemetry = ?, telemetry_consent_version = ?",
        )
        .bind(enabled)
        .bind(i64::from(enabled))
        .execute(&pool)
        .await
        .unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        (temp, state)
    }

    async fn heartbeats(state: &State) -> i64 {
        TelemetryState::new(&state.persistence)
            .due()
            .await
            .unwrap()
            .len() as i64
    }

    #[tokio::test]
    async fn an_interrupted_cycle_converges_and_repeats_harmlessly() {
        let (_temp, state) = fixture(true).await;
        // A session that stopped after creating its identity, before today's
        // heartbeat was queued.
        ensure_identity(&state).await.unwrap();
        assert_eq!(heartbeats(&state).await, 0);

        reconcile(&state).await.unwrap();
        assert_eq!(heartbeats(&state).await, 1, "the heartbeat is queued");
        reconcile(&state).await.unwrap();
        assert_eq!(heartbeats(&state).await, 1, "and only once");
        state.pool.close().await;
    }

    #[tokio::test]
    async fn disabled_telemetry_leaves_an_empty_outbox() {
        let (_temp, state) = fixture(true).await;
        reconcile(&state).await.unwrap();
        assert_eq!(heartbeats(&state).await, 1);
        sqlx::query("UPDATE settings SET telemetry = 0")
            .execute(&state.pool)
            .await
            .unwrap();
        reconcile(&state).await.unwrap();
        let left = TelemetryState::new(&state.persistence)
            .due()
            .await
            .unwrap()
            .len();
        assert_eq!(left, 0);
        state.pool.close().await;
    }
}
