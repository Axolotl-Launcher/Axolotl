//! What a content entry held before an operation replaced it.

use sqlx::SqliteConnection;

use super::Collection;

/// The recorded operations, one per replaced artifact.
pub(crate) struct ResourceHistory;

/// An entry is identified by its own id.
pub(crate) type ResourceHistoryKey = String;

/// Which entries a read wants: those of one instance, or those of one content
/// entry of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceHistoryScope {
    Instance(String),
    Entry {
        instance_id: String,
        entry_id: String,
    },
}

/// What an entry carries beyond its key.
///
/// The installed resources on both sides are kept as the text they are stored
/// as: what the text says is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceHistoryRecord {
    pub instance_id: String,
    pub entry_id: String,
    pub recorded_at: i64,
    pub payload: String,
    pub backup_relative_path: Option<String>,
    pub backup_size: Option<i64>,
    pub backup_sha1: Option<String>,
    pub backup_available: bool,
}

impl Collection for ResourceHistory {
    type Key = ResourceHistoryKey;
    type Scope = ResourceHistoryScope;
    type Record = ResourceHistoryRecord;

    /// The instance and the content entry an entry belongs to are in the
    /// record, not in its id, so what decides whether an entry is in a scope is
    /// the record; the replacement checks each record against the scope before
    /// it writes anything.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<HistoryRow> = sqlx::query_as(
            "
            SELECT instance_id, entry_id, recorded_at, payload,
                backup_relative_path, backup_size, backup_sha1,
                backup_available
            FROM resource_history
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(to_record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM resource_history WHERE id = ?")
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
        let rows: Vec<HistoryRowWithKey> = sqlx::query_as(
            "
            SELECT id, instance_id, entry_id, recorded_at, payload,
                backup_relative_path, backup_size, backup_sha1,
                backup_available
            FROM resource_history
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(split_row).collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_history")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    /// The entries of the scope, newest first, ties broken by insertion order:
    /// a rolled back artifact is the last entry recorded for it, and the
    /// callers read the newest one before the older ones.
    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<HistoryRowWithKey> = match scope {
            ResourceHistoryScope::Instance(instance_id) => {
                sqlx::query_as(
                    "
                    SELECT id, instance_id, entry_id, recorded_at, payload,
                        backup_relative_path, backup_size, backup_sha1,
                        backup_available
                    FROM resource_history
                    WHERE instance_id = ?
                    ORDER BY recorded_at DESC, rowid DESC
                    ",
                )
                .bind(instance_id)
                .fetch_all(&mut *conn)
                .await?
            }
            ResourceHistoryScope::Entry {
                instance_id,
                entry_id,
            } => {
                sqlx::query_as(
                    "
                    SELECT id, instance_id, entry_id, recorded_at, payload,
                        backup_relative_path, backup_size, backup_sha1,
                        backup_available
                    FROM resource_history
                    WHERE instance_id = ? AND entry_id = ?
                    ORDER BY recorded_at DESC, rowid DESC
                    ",
                )
                .bind(instance_id)
                .bind(entry_id)
                .fetch_all(&mut *conn)
                .await?
            }
        };

        Ok(rows.into_iter().map(split_row).collect())
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

type HistoryRow = (
    String,
    String,
    i64,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    bool,
);

type HistoryRowWithKey = (
    String,
    String,
    String,
    i64,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    bool,
);

fn split_row(
    (
        id,
        instance_id,
        entry_id,
        recorded_at,
        payload,
        backup_relative_path,
        backup_size,
        backup_sha1,
        backup_available,
    ): HistoryRowWithKey,
) -> (String, ResourceHistoryRecord) {
    (
        id,
        ResourceHistoryRecord {
            instance_id,
            entry_id,
            recorded_at,
            payload,
            backup_relative_path,
            backup_size,
            backup_sha1,
            backup_available,
        },
    )
}

fn to_record(row: HistoryRow) -> ResourceHistoryRecord {
    let (
        instance_id,
        entry_id,
        recorded_at,
        payload,
        backup_relative_path,
        backup_size,
        backup_sha1,
        backup_available,
    ) = row;
    ResourceHistoryRecord {
        instance_id,
        entry_id,
        recorded_at,
        payload,
        backup_relative_path,
        backup_size,
        backup_sha1,
        backup_available,
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &ResourceHistoryRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_history
            (id, instance_id, entry_id, recorded_at, payload,
             backup_relative_path, backup_size, backup_sha1,
             backup_available)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            instance_id = excluded.instance_id,
            entry_id = excluded.entry_id,
            recorded_at = excluded.recorded_at,
            payload = excluded.payload,
            backup_relative_path = excluded.backup_relative_path,
            backup_size = excluded.backup_size,
            backup_sha1 = excluded.backup_sha1,
            backup_available = excluded.backup_available
        ",
    )
    .bind(key)
    .bind(&record.instance_id)
    .bind(&record.entry_id)
    .bind(record.recorded_at)
    .bind(&record.payload)
    .bind(&record.backup_relative_path)
    .bind(record.backup_size)
    .bind(&record.backup_sha1)
    .bind(record.backup_available)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the entries of the scope that the replacement leaves out, and only
/// those: the backup of an entry the replacement keeps is still on disk, and
/// the file it restores would lose the record that names it.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: &ResourceHistoryScope,
    records: &[(ResourceHistoryKey, ResourceHistoryRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        ResourceHistoryScope::Instance(instance_id) => {
            sqlx::query_scalar(
                "SELECT id FROM resource_history WHERE instance_id = ?",
            )
            .bind(instance_id)
            .fetch_all(&mut *conn)
            .await?
        }
        ResourceHistoryScope::Entry {
            instance_id,
            entry_id,
        } => {
            sqlx::query_scalar(
                "
                SELECT id
                FROM resource_history
                WHERE instance_id = ? AND entry_id = ?
                ",
            )
            .bind(instance_id)
            .bind(entry_id)
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for id in existing {
        if records.iter().any(|(key, _)| key == &id) {
            continue;
        }
        sqlx::query("DELETE FROM resource_history WHERE id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// A scoped replacement may only touch the entries of that scope, and the
/// entries it writes must be in it: one that names another instance would move
/// between instances, and one that names another content entry would replace
/// the history of a file it did not ask about.
fn ensure_records_are_in_scope(
    scope: &ResourceHistoryScope,
    records: &[(ResourceHistoryKey, ResourceHistoryRecord)],
) -> crate::Result<()> {
    let in_scope = |record: &ResourceHistoryRecord| match scope {
        ResourceHistoryScope::Instance(instance_id) => {
            &record.instance_id == instance_id
        }
        ResourceHistoryScope::Entry {
            instance_id,
            entry_id,
        } => &record.instance_id == instance_id && &record.entry_id == entry_id,
    };
    if records.iter().all(|(_, record)| in_scope(record)) {
        return Ok(());
    }
    Err(crate::ErrorKind::InputError(
        "A history entry is not among the entries of the scope it was written to"
            .to_string(),
    )
    .into())
}

/// A scoped replacement writes by id, so an id already recorded under another
/// instance — or another entry — would be taken from its owner instead of
/// replaced here.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &ResourceHistoryScope,
    records: &[(ResourceHistoryKey, ResourceHistoryRecord)],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = ResourceHistory::read(conn, key).await? else {
            continue;
        };
        let in_scope = match scope {
            ResourceHistoryScope::Instance(instance_id) => {
                &existing.instance_id == instance_id
            }
            ResourceHistoryScope::Entry {
                instance_id,
                entry_id,
            } => {
                &existing.instance_id == instance_id
                    && &existing.entry_id == entry_id
            }
        };
        if !in_scope {
            return Err(crate::ErrorKind::InputError(format!(
                "History entry {key} belongs to another scope"
            ))
            .into());
        }
    }

    Ok(())
}
