//! Agent-owned persistence. This database never opens or migrates the launcher database.
//!
//! Admission, approvals and event ordering are transactional so a detached window or a
//! retried IPC cannot turn one user action into two writes. Recovery never resumes work.

mod records;
mod runtime;
mod transcript;
mod workspace;

pub use records::*;
pub use runtime::*;
use serde_json::Value;
use sqlx::{Row, SqliteConnection, SqlitePool, sqlite::SqliteConnectOptions};
use std::{path::Path, time::Duration};
pub use workspace::*;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Approval expired; prepare a fresh plan")]
    ApprovalExpired,
    #[error("Agent database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Agent database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Agent data directory error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid Agent record: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Conflict(&'static str),
}

#[derive(Clone)]
pub struct AgentStore {
    pool: SqlitePool,
}

impl AgentStore {
    /// `data_dir` must be the resolved, channel-specific launcher directory, including
    /// development/portable isolation. Opening performs recovery once per process.
    pub async fn open(data_dir: &Path) -> Result<Self> {
        let directory = data_dir.join("agent-window");
        tokio::fs::create_dir_all(&directory).await?;
        let options = SqliteConnectOptions::new()
            .filename(directory.join("agent.db"))
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        MIGRATIONS.run(&pool).await?;
        let store = Self { pool };
        let mut tx = store.pool.begin().await?;
        sqlx::query(
            "UPDATE executions SET state = 'unknown' WHERE state = 'started'",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE approvals SET state = 'expired' WHERE state IN ('pending', 'approved')")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE approval_batches SET state = 'expired' WHERE state = 'pending'")
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE attempts SET state = 'interrupted', computer_use = 0 WHERE state IN ('queued', 'running', 'awaiting')")
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(store)
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn set_thread(&self, id: &str, payload: &Value) -> Result<()> {
        valid_id(id)?;
        sqlx::query("INSERT INTO threads(id, payload) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = unixepoch()")
            .bind(id).bind(encode(payload)?).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn threads(&self) -> Result<Vec<Value>> {
        let rows = sqlx::query(
            "SELECT payload FROM threads ORDER BY updated_at DESC, id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(row.get("payload"))?))
            .collect()
    }

    /// Deletion is idempotent, but an active task must be stopped before its records
    /// can be removed. FK cascades remove its checkpoints and approval permits.
    pub async fn delete_thread(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        delete_thread_in(&mut tx, id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Import commits the receipt and all rows together. Existing native tasks win
    /// over older browser records; callers retain localStorage until this succeeds.
    pub async fn import_threads(
        &self,
        import_id: &str,
        threads: &[(String, Value)],
    ) -> Result<bool> {
        valid_id(import_id)?;
        let mut tx = self.pool.begin().await?;
        let inserted =
            sqlx::query("INSERT OR IGNORE INTO imports(id) VALUES (?)")
                .bind(import_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        if inserted == 0 {
            return Ok(false);
        }
        for (id, payload) in threads {
            valid_id(id)?;
            sqlx::query(
                "INSERT OR IGNORE INTO threads(id, payload) VALUES (?, ?)",
            )
            .bind(id)
            .bind(encode(payload)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn create_attempt(&self, attempt: &Attempt) -> Result<bool> {
        valid_id(&attempt.id)?;
        valid_id(&attempt.thread_id)?;
        valid_id(&attempt.turn_id)?;
        if !matches!(
            attempt.state,
            AttemptState::Queued | AttemptState::Running
        ) {
            return Err(StoreError::Conflict(
                "New attempts must be queued or running",
            ));
        }
        let mut tx = self.pool.begin().await?;
        if let Some(existing) = get_attempt(&mut tx, &attempt.id).await? {
            if existing.thread_id == attempt.thread_id
                && existing.turn_id == attempt.turn_id
            {
                return Ok(false);
            }
            return Err(StoreError::Conflict(
                "Attempt identifier already belongs to another request",
            ));
        }
        sqlx::query("INSERT INTO attempts(id, thread_id, turn_id, state, computer_use) VALUES (?, ?, ?, ?, ?)")
            .bind(&attempt.id).bind(&attempt.thread_id).bind(&attempt.turn_id)
            .bind(attempt.state.as_str()).bind(attempt.computer_use).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn attempt(&self, id: &str) -> Result<Option<Attempt>> {
        get_attempt(&mut *self.pool.acquire().await?, id).await
    }

    /// Compare-and-set rejects stale worker messages. Terminal states cannot become
    /// running again; manual continuation creates a new attempt identifier.
    pub async fn transition(
        &self,
        id: &str,
        from: AttemptState,
        to: AttemptState,
    ) -> Result<bool> {
        if !from.can_transition(to) {
            return Err(StoreError::Conflict("Invalid attempt transition"));
        }
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE attempts SET state = ? WHERE id = ? AND state = ?",
        )
        .bind(to.as_str())
        .bind(id)
        .bind(from.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if changed && to.is_terminal() {
            sqlx::query("UPDATE approvals SET state = 'expired' WHERE attempt_id = ? AND state IN ('pending', 'approved')")
                .bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE approval_batches SET state = 'expired' WHERE attempt_id = ? AND state = 'pending'")
                .bind(id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn append_event(
        &self,
        attempt_id: &str,
        event_id: &str,
        payload: &Value,
    ) -> Result<Event> {
        valid_id(event_id)?;
        let mut tx = self.pool.begin().await?;
        let encoded = encode(payload)?;
        if let Some(row) = sqlx::query("SELECT sequence, payload FROM events WHERE attempt_id = ? AND event_id = ?")
            .bind(attempt_id).bind(event_id).fetch_optional(&mut *tx).await? {
            if row.get::<String, _>("payload") != encoded { return Err(StoreError::Conflict("Event identifier reused with different content")); }
            return Ok(Event { sequence: row.get("sequence"), attempt_id: attempt_id.into(), event_id: event_id.into(), payload: payload.clone() });
        }
        require_live(&mut tx, attempt_id).await?;
        let sequence = sqlx::query("INSERT INTO events(attempt_id, event_id, payload) VALUES (?, ?, ?)")
            .bind(attempt_id).bind(event_id).bind(encoded).execute(&mut *tx).await?.last_insert_rowid();
        tx.commit().await?;
        Ok(Event {
            sequence,
            attempt_id: attempt_id.into(),
            event_id: event_id.into(),
            payload: payload.clone(),
        })
    }

    pub async fn events(
        &self,
        thread_id: &str,
        after: i64,
        limit: u32,
    ) -> Result<Vec<Event>> {
        let rows = sqlx::query("SELECT e.sequence, e.attempt_id, e.event_id, e.payload FROM events e JOIN attempts a ON a.id = e.attempt_id WHERE a.thread_id = ? AND e.sequence > ? ORDER BY e.sequence LIMIT ?")
            .bind(thread_id).bind(after.max(0)).bind(limit.clamp(1, 1000)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(Event {
                    sequence: row.get("sequence"),
                    attempt_id: row.get("attempt_id"),
                    event_id: row.get("event_id"),
                    payload: serde_json::from_str(row.get("payload"))?,
                })
            })
            .collect()
    }

    pub async fn prepare_approval(&self, approval: &Approval) -> Result<()> {
        valid_id(&approval.id)?;
        valid_id(&approval.call_id)?;
        if approval.revision < 1 {
            return Err(StoreError::Conflict("Invalid approval revision"));
        }
        let mut tx = self.pool.begin().await?;
        require_live(&mut tx, &approval.attempt_id).await?;
        let now: i64 = sqlx::query_scalar("SELECT unixepoch()")
            .fetch_one(&mut *tx)
            .await?;
        if approval.expires_at <= now || approval.expires_at > now + 600 {
            return Err(StoreError::Conflict(
                "Approval must expire within ten minutes",
            ));
        }
        let arguments = encode_arguments(&approval.arguments)?;
        sqlx::query("INSERT INTO approvals(id, attempt_id, call_id, tool, arguments, revision, expires_at, state) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')")
            .bind(&approval.id).bind(&approval.attempt_id).bind(&approval.call_id).bind(&approval.tool)
            .bind(arguments).bind(approval.revision).bind(approval.expires_at).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Only the trusted UI command calls this; the worker capability must not expose it.
    pub async fn decide(
        &self,
        id: &str,
        revision: i64,
        approved: bool,
    ) -> Result<bool> {
        let state = if approved { "approved" } else { "rejected" };
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query("UPDATE approvals SET state = ? WHERE id = ? AND revision = ? AND state = 'pending' AND expires_at > unixepoch() AND EXISTS (SELECT 1 FROM attempts WHERE attempts.id = approvals.attempt_id AND attempts.state IN ('running', 'awaiting'))")
            .bind(state).bind(id).bind(revision).execute(&mut *tx).await?.rows_affected() == 1;
        if !changed {
            let existing: Option<String> = sqlx::query_scalar("SELECT state FROM approvals WHERE id = ? AND revision = ? AND (expires_at > unixepoch() OR state IN ('consumed', 'rejected'))")
                .bind(id).bind(revision).fetch_optional(&mut *tx).await?;
            if existing.as_deref() != Some(state)
                && !(approved && existing.as_deref() == Some("consumed"))
            {
                return Err(StoreError::Conflict(
                    "Approval expired, changed, or already consumed",
                ));
            }
        }
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn approvals(
        &self,
        attempt_id: &str,
    ) -> Result<Vec<ApprovalRecord>> {
        let rows = sqlx::query("SELECT id, call_id, tool, arguments, revision, expires_at, state FROM approvals WHERE attempt_id = ? ORDER BY rowid")
            .bind(attempt_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ApprovalRecord {
                    approval: Approval {
                        id: row.get("id"),
                        attempt_id: attempt_id.into(),
                        call_id: row.get("call_id"),
                        tool: row.get("tool"),
                        arguments: serde_json::from_str(row.get("arguments"))?,
                        revision: row.get("revision"),
                        expires_at: row.get("expires_at"),
                    },
                    state: row.get("state"),
                })
            })
            .collect()
    }

    /// Atomically consumes the exact approval before a business write. A duplicate
    /// call returns its existing receipt, never permission to execute again.
    pub async fn begin_execution(
        &self,
        call: &ExecutionRequest,
    ) -> Result<Admission> {
        valid_id(&call.call_id)?;
        let mut tx = self.pool.begin().await?;
        require_running(&mut tx, &call.attempt_id).await?;
        let attempt = require_live(&mut tx, &call.attempt_id).await?;
        let arguments = encode_arguments(&call.arguments)?;
        if let Some(existing) =
            get_execution(&mut tx, &call.attempt_id, &call.call_id).await?
        {
            if existing.tool != call.tool
                || existing.arguments != call.arguments
                || existing.is_write != call.is_write
            {
                return Err(StoreError::Conflict(
                    "Tool call identifier reused with different arguments",
                ));
            }
            return Ok(Admission::Existing(existing));
        }
        if call.is_write {
            let input: Option<String> = sqlx::query_scalar(
                "SELECT payload FROM attempt_inputs WHERE attempt_id = ?",
            )
            .bind(&call.attempt_id)
            .fetch_optional(&mut *tx)
            .await?;
            if input
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()?
                .is_some_and(|input| input["check_only"] == true)
            {
                return Err(StoreError::Conflict(
                    "Verification runs cannot perform writes",
                ));
            }
            let unsettled: i64 = sqlx::query_scalar("SELECT count(*) FROM executions e JOIN attempts a ON a.id = e.attempt_id WHERE a.thread_id = ? AND e.is_write = 1 AND e.state IN ('started', 'unknown')")
                .bind(&attempt.thread_id).fetch_one(&mut *tx).await?;
            if unsettled != 0 && !call.recovery_control {
                return Err(StoreError::Conflict(
                    "Writes have unconfirmed results; verify readonly before new mutations or use explicit cancellation controls",
                ));
            }
            let permit = call
                .approval
                .as_ref()
                .ok_or(StoreError::Conflict("Write requires approval"))?;
            let consumed = sqlx::query("UPDATE approvals SET state = 'consumed' WHERE id = ? AND revision = ? AND attempt_id = ? AND call_id = ? AND tool = ? AND arguments = ? AND state = 'approved' AND expires_at > unixepoch()")
                .bind(&permit.id).bind(permit.revision).bind(&call.attempt_id).bind(&call.call_id).bind(&call.tool).bind(&arguments)
                .execute(&mut *tx).await?.rows_affected();
            if consumed != 1 {
                return Err(StoreError::Conflict(
                    "Exact unexpired approval required",
                ));
            }
        }
        sqlx::query("INSERT INTO executions(attempt_id, call_id, tool, arguments, is_write, state) VALUES (?, ?, ?, ?, ?, 'started')")
            .bind(&call.attempt_id).bind(&call.call_id).bind(&call.tool).bind(arguments).bind(call.is_write).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Admission::Execute)
    }

    pub async fn finish_execution(
        &self,
        attempt_id: &str,
        call_id: &str,
        result: &Value,
        failed: bool,
    ) -> Result<bool> {
        let safe = redact(result.clone());
        Ok(sqlx::query("UPDATE executions SET state = ?, result = ? WHERE attempt_id = ? AND call_id = ? AND state = 'started'")
            .bind(if failed { "failed" } else { "completed" }).bind(encode(&safe)?).bind(attempt_id).bind(call_id)
            .execute(&self.pool).await?.rows_affected() == 1)
    }

    pub async fn execution(
        &self,
        attempt_id: &str,
        call_id: &str,
    ) -> Result<Option<Execution>> {
        get_execution(&mut *self.pool.acquire().await?, attempt_id, call_id)
            .await
    }

    pub async fn put_checkpoint(
        &self,
        attempt_id: &str,
        namespace: &str,
        checkpoint_id: &str,
        payload: &Value,
    ) -> Result<()> {
        valid_id(checkpoint_id)?;
        let mut tx = self.pool.begin().await?;
        let attempt = require_live(&mut tx, attempt_id).await?;
        sqlx::query("INSERT INTO checkpoints(thread_id, namespace, checkpoint_id, payload) VALUES (?, ?, ?, ?) ON CONFLICT(thread_id, namespace, checkpoint_id) DO UPDATE SET payload = excluded.payload")
            .bind(attempt.thread_id).bind(namespace).bind(checkpoint_id).bind(encode(payload)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn checkpoints(
        &self,
        thread_id: &str,
        namespace: &str,
        limit: u32,
    ) -> Result<Vec<Checkpoint>> {
        let rows = sqlx::query("SELECT checkpoint_id, payload FROM checkpoints WHERE thread_id = ? AND namespace = ? ORDER BY sequence DESC LIMIT ?")
            .bind(thread_id).bind(namespace).bind(limit.clamp(1, 1000)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(Checkpoint {
                    id: row.get("checkpoint_id"),
                    payload: serde_json::from_str(row.get("payload"))?,
                })
            })
            .collect()
    }

    pub async fn set_message(
        &self,
        thread_id: &str,
        attempt_id: Option<&str>,
        id: &str,
        payload: &Value,
    ) -> Result<()> {
        valid_id(id)?;
        let mut tx = self.pool.begin().await?;
        if let Some(attempt_id) = attempt_id {
            let attempt = require_live(&mut tx, attempt_id).await?;
            if attempt.thread_id != thread_id {
                return Err(StoreError::Conflict(
                    "Message belongs to another task",
                ));
            }
        }
        let changed = sqlx::query("INSERT INTO messages(id, thread_id, attempt_id, payload) VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload WHERE messages.thread_id = excluded.thread_id AND messages.attempt_id IS excluded.attempt_id")
            .bind(id).bind(thread_id).bind(attempt_id).bind(encode(payload)?).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(StoreError::Conflict(
                "Message belongs to another task",
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_attachment(
        &self,
        thread_id: &str,
        id: &str,
        payload: &Value,
    ) -> Result<()> {
        valid_id(id)?;
        let changed = sqlx::query("INSERT INTO attachments(id, thread_id, payload) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload WHERE attachments.thread_id = excluded.thread_id")
            .bind(id).bind(thread_id).bind(encode(payload)?).execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            return Err(StoreError::Conflict(
                "Attachment belongs to another task",
            ));
        }
        Ok(())
    }

    pub async fn messages(&self, thread_id: &str) -> Result<Vec<Value>> {
        let rows = sqlx::query(
            "SELECT payload FROM messages WHERE thread_id = ? ORDER BY rowid",
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(row.get("payload"))?))
            .collect()
    }

    pub async fn attachments(&self, thread_id: &str) -> Result<Vec<Value>> {
        let rows = sqlx::query("SELECT payload FROM attachments WHERE thread_id = ? ORDER BY rowid")
            .bind(thread_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(row.get("payload"))?))
            .collect()
    }
}

async fn get_attempt(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<Attempt>> {
    sqlx::query("SELECT id, thread_id, turn_id, state, computer_use FROM attempts WHERE id = ?")
        .bind(id).fetch_optional(connection).await?.map(|row| Ok(Attempt {
            id: row.get("id"), thread_id: row.get("thread_id"), turn_id: row.get("turn_id"),
            state: AttemptState::parse(row.get("state"))?, computer_use: row.get("computer_use"),
        })).transpose()
}

async fn require_live(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Attempt> {
    let attempt = get_attempt(connection, id)
        .await?
        .ok_or(StoreError::Conflict("Unknown run attempt"))?;
    if !matches!(
        attempt.state,
        AttemptState::Running | AttemptState::Awaiting
    ) {
        return Err(StoreError::Conflict("Run attempt is no longer active"));
    }
    Ok(attempt)
}

async fn require_running(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<()> {
    if require_live(connection, id).await?.state != AttemptState::Running {
        return Err(StoreError::Conflict("Run is awaiting approval"));
    }
    Ok(())
}

async fn get_execution(
    connection: &mut SqliteConnection,
    attempt_id: &str,
    call_id: &str,
) -> Result<Option<Execution>> {
    sqlx::query("SELECT tool, arguments, is_write, state, result FROM executions WHERE attempt_id = ? AND call_id = ?")
        .bind(attempt_id).bind(call_id).fetch_optional(connection).await?.map(|row| Ok(Execution {
            tool: row.get("tool"), arguments: serde_json::from_str(row.get("arguments"))?, is_write: row.get("is_write"),
            state: row.get("state"), result: row.get::<Option<String>, _>("result").map(|v| serde_json::from_str(&v)).transpose()?,
        })).transpose()
}

fn valid_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
        return Err(StoreError::Conflict("Invalid record identifier"));
    }
    Ok(())
}

fn encode(value: &Value) -> Result<String> {
    let text = serde_json::to_string(value)?;
    if text.len() > 16 * 1024 * 1024 {
        return Err(StoreError::Conflict(
            "Agent record exceeds storage budget",
        ));
    }
    Ok(text)
}

fn encode_arguments(value: &Value) -> Result<String> {
    if redact(value.clone()) != *value {
        return Err(StoreError::Conflict(
            "Credential arguments cannot be persisted; configure credentials in launcher settings",
        ));
    }
    encode(value)
}

/// Redacts structured credential fields; callers must additionally avoid requesting
/// credential-export tools. Arbitrary user-authored prose is not rewritten.
pub fn redact(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let normalized =
                        key.to_ascii_lowercase().replace(['-', '_'], "");
                    let sensitive = matches!(
                        normalized.as_str(),
                        "apikey"
                            | "accesstoken"
                            | "refreshtoken"
                            | "authorization"
                            | "password"
                            | "secret"
                            | "clientsecret"
                            | "bearertoken"
                            | "credential"
                            | "credentials"
                    );
                    (
                        key,
                        if sensitive {
                            Value::String("[redacted]".into())
                        } else {
                            redact(value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(redact).collect())
        }
        // ToolOutput carries both structured content and its serialized text mirror.
        Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
            match serde_json::from_str::<Value>(&text) {
                Ok(value) => {
                    let safe = redact(value.clone());
                    if safe == value {
                        Value::String(text)
                    } else {
                        Value::String(safe.to_string())
                    }
                }
                Err(_) => Value::String(text),
            }
        }
        value => value,
    }
}

async fn delete_thread_in(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<()> {
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM attempts WHERE thread_id = ? AND state IN ('queued', 'running', 'awaiting')")
            .bind(id).fetch_one(&mut *connection).await?;
    if active != 0 {
        return Err(StoreError::Conflict("Stop the task before deleting it"));
    }
    let writing: i64 = sqlx::query_scalar("SELECT count(*) FROM executions e JOIN attempts a ON a.id = e.attempt_id WHERE a.thread_id = ? AND e.state = 'started'")
            .bind(id).fetch_one(&mut *connection).await?;
    if writing != 0 {
        return Err(StoreError::Conflict(
            "Wait for started operations to settle before deleting their records",
        ));
    }
    sqlx::query("DELETE FROM threads WHERE id = ?")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    Ok(())
}
