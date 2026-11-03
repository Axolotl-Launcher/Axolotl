//! The telemetry events waiting to be uploaded.
//!
//! A dedupe key names one event across the queue: writing an event whose dedupe
//! key is already queued replaces the event queued under it, so a day's
//! heartbeat stays one event.
//!
//! The payload is carried as the text it is stored as: what it says is the
//! caller's business.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;

use super::{Collection, Handle};

/// The queued events.
pub(crate) struct TelemetryOutbox;

/// An event is identified by its own id.
pub(crate) type TelemetryOutboxKey = String;

/// What kind of event a queued event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TelemetryEventType {
    Heartbeat,
    Error,
}

impl TelemetryEventType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Heartbeat => "heartbeat",
            Self::Error => "error",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "heartbeat" => Some(Self::Heartbeat),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Which queued events a read wants: everything of one kind, or the part of it
/// that may be sent now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TelemetryOutboxScope {
    Kind(TelemetryEventType),
    Due { kind: TelemetryEventType, at: i64 },
}

/// What a queued event carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TelemetryOutboxRecord {
    pub event_type: TelemetryEventType,
    pub payload: String,
    pub created_at: i64,
    pub next_attempt_at: i64,
    pub attempts: i64,
    pub size_bytes: i64,
    pub dedupe_key: String,
    pub occurrence_count: i64,
}

impl Collection for TelemetryOutbox {
    type Key = TelemetryOutboxKey;
    type Scope = TelemetryOutboxScope;
    type Record = TelemetryOutboxRecord;

    /// An event's kind and schedule live in the record rather than the key, so
    /// the collection cannot tell from the key whether an event is in a scope:
    /// the replacement checks each record against the scope instead.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row = sqlx::query(
            "
            SELECT event_id, event_type, json(payload) AS payload, created_at,
                next_attempt_at, attempts, size_bytes, dedupe_key,
                occurrence_count
            FROM telemetry_outbox
            WHERE event_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        match row {
            Some(row) => Ok(to_record(row)?.map(|(_, record)| record)),
            None => Ok(None),
        }
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM telemetry_outbox WHERE event_id = ?")
                .bind(key)
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows = sqlx::query(
            "
            SELECT event_id, event_type, json(payload) AS payload, created_at,
                next_attempt_at, attempts, size_bytes, dedupe_key,
                occurrence_count
            FROM telemetry_outbox
            ORDER BY created_at, event_id
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        to_records(rows)
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM telemetry_outbox")
            .execute(&mut *conn)
            .await?;

        for (key, record) in records {
            write_row(conn, key, record).await?;
        }

        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows = match scope {
            TelemetryOutboxScope::Kind(kind) => {
                sqlx::query(
                    "
                    SELECT event_id, event_type, json(payload) AS payload,
                        created_at, next_attempt_at, attempts, size_bytes,
                        dedupe_key, occurrence_count
                    FROM telemetry_outbox
                    WHERE event_type = ?
                    ORDER BY created_at, event_id
                    ",
                )
                .bind(kind.as_str())
                .fetch_all(&mut *conn)
                .await?
            }
            TelemetryOutboxScope::Due { kind, at } => {
                sqlx::query(
                    "
                    SELECT event_id, event_type, json(payload) AS payload,
                        created_at, next_attempt_at, attempts, size_bytes,
                        dedupe_key, occurrence_count
                    FROM telemetry_outbox
                    WHERE event_type = ? AND next_attempt_at <= ?
                    ORDER BY created_at, event_id
                    ",
                )
                .bind(kind.as_str())
                .bind(*at)
                .fetch_all(&mut *conn)
                .await?
            }
        };

        to_records(rows)
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, scope, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }

        Ok(())
    }
}

impl TelemetryOutbox {
    /// Trimming keeps the newest events within an age, a count and a byte
    /// budget, which no record-by-record replacement can say.
    pub(crate) async fn trim(
        handle: &Handle,
        oldest: i64,
        max_events: i64,
        max_bytes: i64,
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM telemetry_outbox WHERE created_at < ?")
            .bind(oldest)
            .execute(handle.pool())
            .await?;
        sqlx::query(
            "DELETE FROM telemetry_outbox WHERE event_id IN (SELECT event_id FROM telemetry_outbox ORDER BY created_at DESC LIMIT -1 OFFSET ?)",
        )
        .bind(max_events)
        .execute(handle.pool())
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
        .bind(max_bytes)
        .execute(handle.pool())
        .await?;
        Ok(())
    }
}

fn to_records(
    rows: Vec<SqliteRow>,
) -> crate::Result<Vec<(TelemetryOutboxKey, TelemetryOutboxRecord)>> {
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(record) = to_record(row)? {
            records.push(record);
        }
    }

    Ok(records)
}

/// The record a row describes, or nothing when its kind is one this build does
/// not know.
fn to_record(
    row: SqliteRow,
) -> crate::Result<Option<(TelemetryOutboxKey, TelemetryOutboxRecord)>> {
    let stored: String = row.try_get("event_type")?;
    let Some(event_type) = TelemetryEventType::parse(&stored) else {
        tracing::warn!(
            %stored,
            "Ignoring a queued telemetry event of a kind this build does not know"
        );
        return Ok(None);
    };

    Ok(Some((
        row.try_get("event_id")?,
        TelemetryOutboxRecord {
            event_type,
            payload: row.try_get("payload")?,
            created_at: row.try_get("created_at")?,
            next_attempt_at: row.try_get("next_attempt_at")?,
            attempts: row.try_get("attempts")?,
            size_bytes: row.try_get("size_bytes")?,
            dedupe_key: row.try_get("dedupe_key")?,
            occurrence_count: row.try_get("occurrence_count")?,
        },
    )))
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &TelemetryOutboxRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO telemetry_outbox (
            event_id, event_type, payload, created_at, next_attempt_at,
            attempts, size_bytes, dedupe_key, occurrence_count
        )
        VALUES (?, ?, jsonb(?), ?, ?, ?, ?, ?, ?)
        ON CONFLICT DO UPDATE SET
            event_id = excluded.event_id,
            event_type = excluded.event_type,
            payload = excluded.payload,
            created_at = excluded.created_at,
            next_attempt_at = excluded.next_attempt_at,
            attempts = excluded.attempts,
            size_bytes = excluded.size_bytes,
            dedupe_key = excluded.dedupe_key,
            occurrence_count = excluded.occurrence_count
        ",
    )
    .bind(key)
    .bind(record.event_type.as_str())
    .bind(&record.payload)
    .bind(record.created_at)
    .bind(record.next_attempt_at)
    .bind(record.attempts)
    .bind(record.size_bytes)
    .bind(&record.dedupe_key)
    .bind(record.occurrence_count)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the events the scope holds and the replacement leaves out.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: &TelemetryOutboxScope,
    records: &[(TelemetryOutboxKey, TelemetryOutboxRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        TelemetryOutboxScope::Kind(kind) => {
            sqlx::query_scalar(
                "
                SELECT event_id
                FROM telemetry_outbox
                WHERE event_type = ?
                ",
            )
            .bind(kind.as_str())
            .fetch_all(&mut *conn)
            .await?
        }
        TelemetryOutboxScope::Due { kind, at } => {
            sqlx::query_scalar(
                "
                SELECT event_id
                FROM telemetry_outbox
                WHERE event_type = ? AND next_attempt_at <= ?
                ",
            )
            .bind(kind.as_str())
            .bind(*at)
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for event_id in existing {
        if records.iter().any(|(key, _)| key == &event_id) {
            continue;
        }
        sqlx::query("DELETE FROM telemetry_outbox WHERE event_id = ?")
            .bind(&event_id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// A replacement may only write events of the kind its scope names, and only
/// those the scope reaches: an event of another kind would write outside the
/// group being replaced, and one scheduled later would change what the scope
/// holds while it is being replaced.
fn ensure_records_are_in_scope(
    scope: &TelemetryOutboxScope,
    records: &[(TelemetryOutboxKey, TelemetryOutboxRecord)],
) -> crate::Result<()> {
    for (key, record) in records {
        let in_scope = match scope {
            TelemetryOutboxScope::Kind(kind) => record.event_type == *kind,
            TelemetryOutboxScope::Due { kind, at } => {
                record.event_type == *kind && record.next_attempt_at <= *at
            }
        };
        if !in_scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Telemetry event {key} is not among the events of the scope it was written to"
            ))
            .into());
        }
    }

    Ok(())
}

/// A scoped replacement writes by event id, so an event already stored under
/// another kind would move between the kinds instead of being replaced here.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &TelemetryOutboxScope,
    records: &[(TelemetryOutboxKey, TelemetryOutboxRecord)],
) -> crate::Result<()> {
    for (key, record) in records {
        // Every row this write would replace: the one under its own id, and any
        // row already holding its dedupe key, whichever id that row has. Both
        // are read as stored text, so a row this build cannot read is still
        // checked rather than mistaken for an absent one.
        let stored: Vec<(String, String, i64)> = sqlx::query_as(
            "
            SELECT event_id, event_type, next_attempt_at
            FROM telemetry_outbox
            WHERE event_id = ? OR dedupe_key = ?
            ",
        )
        .bind(key)
        .bind(&record.dedupe_key)
        .fetch_all(&mut *conn)
        .await?;

        for (stored_id, stored_kind, next_attempt_at) in stored {
            let Some(event_type) = TelemetryEventType::parse(&stored_kind)
            else {
                tracing::warn!(
                    event_id = %stored_id,
                    stored = %stored_kind,
                    "Refusing to replace a queued telemetry event of a kind this build does not know"
                );
                return Err(crate::ErrorKind::InputError(format!(
                    "Telemetry event {stored_id} was written by another build"
                ))
                .into());
            };
            let in_scope = match scope {
                TelemetryOutboxScope::Kind(kind) => event_type == *kind,
                TelemetryOutboxScope::Due { kind, at } => {
                    event_type == *kind && next_attempt_at <= *at
                }
            };
            if !in_scope {
                return Err(crate::ErrorKind::InputError(format!(
                    "Telemetry event {stored_id} belongs to another part of the queue"
                ))
                .into());
            }
        }
    }

    Ok(())
}
