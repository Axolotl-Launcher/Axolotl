//! The logs the user has shared, newest first.
//!
//! A shared log keeps the token its upload answered with, which is what deletes
//! it again; the list is the history the settings page shows.

use sqlx::SqliteConnection;

use super::Collection;

/// The shared logs.
pub(crate) struct SharedLogs;

/// A shared log is identified by its own id.
pub(crate) type SharedLogKey = String;

/// What a shared log carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedLogRecord {
    pub url: String,
    pub raw: String,
    pub token: String,
    pub provider: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    pub truncated: bool,
    pub created_at: i64,
}

impl Collection for SharedLogs {
    type Key = SharedLogKey;
    type Scope = SharedLogKey;
    type Record = SharedLogRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<SharedLogRow> = sqlx::query_as(
            "
            SELECT url, raw, token, provider, instance_id, instance_name,
                truncated, created_at
            FROM shared_logs
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(record_from))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM shared_logs WHERE id = ?")
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
        let rows: Vec<SharedLogRowWithKey> = sqlx::query_as(
            "
            SELECT id, url, raw, token, provider, instance_id, instance_name,
                truncated, created_at
            FROM shared_logs
            ORDER BY created_at DESC
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
        sqlx::query("DELETE FROM shared_logs")
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
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(scope.clone(), record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

type SharedLogRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    bool,
    i64,
);

type SharedLogRowWithKey = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    bool,
    i64,
);

fn record_from(
    (
        url,
        raw,
        token,
        provider,
        instance_id,
        instance_name,
        truncated,
        created_at,
    ): SharedLogRow,
) -> SharedLogRecord {
    SharedLogRecord {
        url,
        raw,
        token,
        provider,
        instance_id,
        instance_name,
        truncated,
        created_at,
    }
}

fn split_row(
    (
        id,
        url,
        raw,
        token,
        provider,
        instance_id,
        instance_name,
        truncated,
        created_at,
    ): SharedLogRowWithKey,
) -> (String, SharedLogRecord) {
    (
        id,
        SharedLogRecord {
            url,
            raw,
            token,
            provider,
            instance_id,
            instance_name,
            truncated,
            created_at,
        },
    )
}

/// Records one shared log; a second record under the same id refreshes what the
/// upload answered but keeps the moment it was first shared.
async fn write_row(
    conn: &mut SqliteConnection,
    key: &SharedLogKey,
    record: &SharedLogRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO shared_logs (
            id, url, raw, token, provider, instance_id, instance_name,
            truncated, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            raw = excluded.raw,
            token = excluded.token,
            provider = excluded.provider,
            instance_id = excluded.instance_id,
            instance_name = excluded.instance_name,
            truncated = excluded.truncated
        ",
    )
    .bind(key)
    .bind(&record.url)
    .bind(&record.raw)
    .bind(&record.token)
    .bind(&record.provider)
    .bind(&record.instance_id)
    .bind(&record.instance_name)
    .bind(record.truncated)
    .bind(record.created_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
