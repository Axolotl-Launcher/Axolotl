//! The responses fetched for the launcher, kept between runs.
//!
//! A row is the payload fetched for one type and id, the alias it answers to as
//! well, and when it stops being used. A payload is carried as the JSON text it
//! is stored as: what the text means is the caller's business, and a payload
//! the cache would not have written is not read back.
//!
//! A set of keys is not a row of this collection, so the statements that read
//! or write a whole set in one go stay beside the callers that need them.

use sqlx::SqliteConnection;

use super::Collection;

/// The cached responses, one row per fetched document.
pub(crate) struct Cache;

/// A row is identified by the kind of response and the id within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheKey {
    pub data_type: String,
    pub id: String,
}

/// What a row carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheRecord {
    pub alias: Option<String>,
    pub data: Option<String>,
    pub expires: i64,
}

/// The largest payload that is read back. A row above it is not one the cache
/// wrote, and loading it would cost the memory this bound protects.
const MAX_PAYLOAD_BYTES: i64 = 1024 * 1024;

impl Collection for Cache {
    type Key = CacheKey;
    type Scope = String;
    type Record = CacheRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        key.data_type == *scope
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(Option<String>, Option<String>, i64)> =
            sqlx::query_as(
                "
                SELECT alias, data, expires
                FROM cache
                WHERE id = ? AND data_type = ?
                    AND (data IS NULL
                         OR length(CAST(data AS BLOB)) <= ?)
                ",
            )
            .bind(&key.id)
            .bind(&key.data_type)
            .bind(MAX_PAYLOAD_BYTES)
            .fetch_optional(&mut *conn)
            .await?;

        Ok(row.map(|(alias, data, expires)| Self::Record {
            alias,
            data,
            expires,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM cache WHERE id = ? AND data_type = ?")
                .bind(&key.id)
                .bind(&key.data_type)
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, Option<String>, Option<String>, i64)> =
            sqlx::query_as(
                "
                SELECT id, data_type, alias, data, expires
                FROM cache
                WHERE data IS NULL
                    OR length(CAST(data AS BLOB)) <= ?
                ",
            )
            .bind(MAX_PAYLOAD_BYTES)
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(|(id, data_type, alias, data, expires)| {
                (
                    CacheKey { data_type, id },
                    CacheRecord {
                        alias,
                        data,
                        expires,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM cache").execute(&mut *conn).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, Option<String>, Option<String>, i64)> =
            sqlx::query_as(
                "
                SELECT id, alias, data, expires
                FROM cache
                WHERE data_type = ?
                    AND (data IS NULL
                         OR length(CAST(data AS BLOB)) <= ?)
                ",
            )
            .bind(scope)
            .bind(MAX_PAYLOAD_BYTES)
            .fetch_all(&mut *conn)
            .await?;

        Ok(rows
            .into_iter()
            .map(|(id, alias, data, expires)| {
                (
                    CacheKey {
                        data_type: scope.clone(),
                        id,
                    },
                    CacheRecord {
                        alias,
                        data,
                        expires,
                    },
                )
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM cache WHERE data_type = ?")
            .bind(scope)
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

/// Writes one row, after taking the alias from whichever row of the same type
/// held it: an alias names one row of a type, so the row that held it is
/// replaced rather than reported as a conflict.
async fn write_row(
    conn: &mut SqliteConnection,
    key: &CacheKey,
    record: &CacheRecord,
) -> crate::Result<()> {
    if let Some(alias) = &record.alias {
        sqlx::query(
            "
            DELETE FROM cache
            WHERE data_type = ? AND alias = ? AND id <> ?
            ",
        )
        .bind(&key.data_type)
        .bind(alias)
        .bind(&key.id)
        .execute(&mut *conn)
        .await?;
    }

    sqlx::query(
        "
        INSERT INTO cache (id, data_type, alias, data, expires)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (id, data_type) DO UPDATE SET
            alias = excluded.alias,
            data = excluded.data,
            expires = excluded.expires
        ",
    )
    .bind(&key.id)
    .bind(&key.data_type)
    .bind(&record.alias)
    .bind(&record.data)
    .bind(record.expires)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
