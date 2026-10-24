use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::State;

const ENDPOINT: &str = "https://telemetry.axlmc.org/v1/batch";
const MAX_OUTBOX_EVENTS: i64 = 100;
const MAX_OUTBOX_BYTES: i64 = 2 * 1024 * 1024;
const MAX_EVENT_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;
const MAX_BATCH_EVENTS: i64 = 10;
const MAX_BATCH_BYTES: usize = 60 * 1024;

static STARTED: AtomicBool = AtomicBool::new(false);
static WAKE_TX: OnceLock<tokio::sync::mpsc::Sender<()>> = OnceLock::new();

pub(crate) fn start(state: Arc<State>) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    let (wake_tx, mut wake_rx) = tokio::sync::mpsc::channel(1);
    let _ = WAKE_TX.set(wake_tx);
    let task = tokio::spawn({
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
    state.background_tasks.lock().push(task);
}

pub async fn set_enabled(state: &State, enabled: bool) -> crate::Result<()> {
    let _database_permit = state.acquire_install_db_permit().await?;
    sqlx::query("DELETE FROM telemetry_outbox")
        .execute(&state.pool)
        .await?;

    if enabled {
        ensure_identity(&state.pool).await?;
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
        sqlx::query("DELETE FROM telemetry_outbox")
            .execute(&state.pool)
            .await?;
        return Ok(());
    }

    {
        let _database_permit = state.acquire_install_db_permit().await?;
        ensure_identity(&state.pool).await?;
        // Error events were supported by older clients. Drop any that remain
        // in the local queue before selecting uploadable events.
        sqlx::query(
            "DELETE FROM telemetry_outbox WHERE event_type <> 'heartbeat'",
        )
        .execute(&state.pool)
        .await?;
        enqueue_heartbeat(state).await?;
        cleanup_outbox(state).await?;
    }
    upload_next_batch(state, client).await?;
    Ok(())
}

async fn is_enabled(state: &State) -> crate::Result<bool> {
    let row = sqlx::query(
		"SELECT telemetry, telemetry_consent_version FROM settings WHERE id = 0",
	)
	.fetch_one(&state.pool)
	.await?;
    Ok(row.get::<i64, _>("telemetry") == 1
        && row.get::<i64, _>("telemetry_consent_version") > 0)
}

async fn ensure_identity(pool: &sqlx::SqlitePool) -> crate::Result<String> {
    if let Some(row) = sqlx::query(
        "SELECT installation_id FROM telemetry_identity WHERE id = 0",
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(row.get("installation_id"));
    }

    let installation_id = Uuid::new_v4().to_string();
    sqlx::query(
		"INSERT OR IGNORE INTO telemetry_identity (id, installation_id) VALUES (0, ?)",
	)
	.bind(&installation_id)
	.execute(pool)
	.await?;
    let row = sqlx::query(
        "SELECT installation_id FROM telemetry_identity WHERE id = 0",
    )
    .fetch_one(pool)
    .await?;
    Ok(row.get("installation_id"))
}

async fn enqueue_heartbeat(state: &State) -> crate::Result<()> {
    let day = Utc::now().format("%Y-%m-%d").to_string();
    let row = sqlx::query(
        "SELECT last_heartbeat_day FROM telemetry_identity WHERE id = 0",
    )
    .fetch_one(&state.pool)
    .await?;
    if row
        .get::<Option<String>, _>("last_heartbeat_day")
        .as_deref()
        == Some(&day)
    {
        return Ok(());
    }

    let event_id = Uuid::new_v4().to_string();
    let occurred_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let payload = heartbeat_payload(&event_id, &occurred_at, &day);
    insert_outbox_event(
        state,
        &event_id,
        &format!("heartbeat:{day}"),
        &payload,
    )
    .await?;
    sqlx::query(
        "UPDATE telemetry_identity SET last_heartbeat_day = ? WHERE id = 0",
    )
    .bind(day)
    .execute(&state.pool)
    .await?;
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
    state: &State,
    event_id: &str,
    dedupe_key: &str,
    payload: &Value,
) -> crate::Result<()> {
    let payload = serde_json::to_string(payload)?;
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
		INSERT INTO telemetry_outbox (
			event_id, event_type, payload, created_at, next_attempt_at,
			size_bytes, dedupe_key
		) VALUES (?, 'heartbeat', jsonb(?), ?, ?, ?, ?)
		ON CONFLICT(dedupe_key) DO NOTHING
		"#,
    )
    .bind(event_id)
    .bind(&payload)
    .bind(now)
    .bind(now)
    .bind(payload.len() as i64)
    .bind(dedupe_key)
    .execute(&state.pool)
    .await?;
    cleanup_outbox(state).await
}

async fn cleanup_outbox(state: &State) -> crate::Result<()> {
    let oldest = Utc::now().timestamp() - MAX_EVENT_AGE_SECONDS;
    sqlx::query("DELETE FROM telemetry_outbox WHERE created_at < ?")
        .bind(oldest)
        .execute(&state.pool)
        .await?;
    sqlx::query(
		"DELETE FROM telemetry_outbox WHERE event_id IN (SELECT event_id FROM telemetry_outbox ORDER BY created_at DESC LIMIT -1 OFFSET ?)",
	)
	.bind(MAX_OUTBOX_EVENTS)
	.execute(&state.pool)
	.await?;
    sqlx::query(
		r#"
		DELETE FROM telemetry_outbox
		WHERE event_id IN (
			SELECT event_id FROM (
				SELECT event_id,
					SUM(size_bytes) OVER (ORDER BY created_at DESC, event_id DESC) AS running_bytes
				FROM telemetry_outbox
			)
			WHERE running_bytes > ?
		)
		"#,
	)
	.bind(MAX_OUTBOX_BYTES)
	.execute(&state.pool)
	.await?;
    Ok(())
}

async fn upload_next_batch(
    state: &State,
    client: &reqwest::Client,
) -> crate::Result<()> {
    let now = Utc::now().timestamp();
    let rows = sqlx::query(
		"SELECT event_id, json(payload) AS payload FROM telemetry_outbox WHERE event_type = 'heartbeat' AND next_attempt_at <= ? ORDER BY created_at LIMIT ?",
	)
	.bind(now)
	.bind(MAX_BATCH_EVENTS)
	.fetch_all(&state.pool)
	.await?;
    if rows.is_empty() {
        return Ok(());
    }

    let mut events = Vec::new();
    let mut event_ids = Vec::new();
    let mut approximate_size = 0;
    for row in rows {
        let payload: String = row.get("payload");
        if approximate_size + payload.len() > MAX_BATCH_BYTES
            && !events.is_empty()
        {
            break;
        }
        let event: Value = serde_json::from_str(&payload)?;
        approximate_size += payload.len();
        events.push(event);
        event_ids.push(row.get::<String, _>("event_id"));
    }

    let installation_id = {
        let _database_permit = state.acquire_install_db_permit().await?;
        ensure_identity(&state.pool).await?
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
    for event_id in event_ids {
        sqlx::query("DELETE FROM telemetry_outbox WHERE event_id = ?")
            .bind(event_id)
            .execute(&state.pool)
            .await?;
    }
    Ok(())
}

async fn schedule_retry(
    state: &State,
    event_ids: &[String],
) -> crate::Result<()> {
    let _database_permit = state.acquire_install_db_permit().await?;
    for event_id in event_ids {
        let row = sqlx::query(
            "SELECT attempts FROM telemetry_outbox WHERE event_id = ?",
        )
        .bind(event_id)
        .fetch_optional(&state.pool)
        .await?;
        let Some(row) = row else { continue };
        let attempts = row.get::<i64, _>("attempts") + 1;
        let delay = match attempts {
            1 => 60,
            2 => 5 * 60,
            3 => 30 * 60,
            _ => 6 * 60 * 60,
        };
        sqlx::query(
			"UPDATE telemetry_outbox SET attempts = ?, next_attempt_at = ? WHERE event_id = ?",
		)
		.bind(attempts)
		.bind(Utc::now().timestamp() + delay)
		.bind(event_id)
		.execute(&state.pool)
		.await?;
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
