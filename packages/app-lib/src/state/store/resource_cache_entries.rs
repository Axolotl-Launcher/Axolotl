//! A provider answer kept until it expires or the cache is cleared.

use sqlx::SqliteConnection;

use super::Collection;

/// The cached answers, one per query.
pub(crate) struct ResourceCacheEntries;

/// An answer is identified by the namespace it was asked in, the source it came
/// from, and the query itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceCacheEntryKey {
    pub namespace: String,
    pub source: String,
    pub query_key: String,
}

/// What a cached answer carries.
///
/// The answer is kept as the text it is stored as: what the text says is the
/// caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceCacheEntryRecord {
    pub value_version: i64,
    pub payload: String,
    pub retrieved_at: i64,
    pub expires_at: i64,
}

impl Collection for ResourceCacheEntries {
    type Key = ResourceCacheEntryKey;
    type Scope = Self::Key;
    type Record = ResourceCacheEntryRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, String, i64, i64)> = sqlx::query_as(
            "
            SELECT value_version, payload, retrieved_at, expires_at
            FROM resource_cache_entries
            WHERE namespace = ? AND source = ? AND query_key = ?
            ",
        )
        .bind(&key.namespace)
        .bind(&key.source)
        .bind(&key.query_key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(value_version, payload, retrieved_at, expires_at)| {
                Self::Record {
                    value_version,
                    payload,
                    retrieved_at,
                    expires_at,
                }
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "
                DELETE FROM resource_cache_entries
                WHERE namespace = ? AND source = ? AND query_key = ?
                ",
            )
            .bind(&key.namespace)
            .bind(&key.source)
            .bind(&key.query_key)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String, i64, String, i64, i64)> =
            sqlx::query_as(
                "
                SELECT namespace, source, query_key, value_version, payload,
                    retrieved_at, expires_at
                FROM resource_cache_entries
                ",
            )
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    namespace,
                    source,
                    query_key,
                    value_version,
                    payload,
                    retrieved_at,
                    expires_at,
                )| {
                    (
                        ResourceCacheEntryKey {
                            namespace,
                            source,
                            query_key,
                        },
                        ResourceCacheEntryRecord {
                            value_version,
                            payload,
                            retrieved_at,
                            expires_at,
                        },
                    )
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_cache_entries")
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

async fn write_row(
    conn: &mut SqliteConnection,
    key: &ResourceCacheEntryKey,
    record: &ResourceCacheEntryRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_cache_entries
            (namespace, source, query_key, value_version, payload,
             retrieved_at, expires_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (namespace, source, query_key) DO UPDATE SET
            value_version = excluded.value_version,
            payload = excluded.payload,
            retrieved_at = excluded.retrieved_at,
            expires_at = excluded.expires_at
        ",
    )
    .bind(&key.namespace)
    .bind(&key.source)
    .bind(&key.query_key)
    .bind(record.value_version)
    .bind(&record.payload)
    .bind(record.retrieved_at)
    .bind(record.expires_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
