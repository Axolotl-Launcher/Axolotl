//! The probed addresses of the Google Translate server.
//!
//! A row is one address and the latency it answered with. The cache is
//! replaced as a whole after a scan, and the newest `created_at` says when that
//! was.

use sqlx::SqliteConnection;

use super::Collection;

/// The probed addresses.
pub(crate) struct GoogleTranslateIpCache;

/// An address is identified by itself.
pub(crate) type GoogleTranslateIpCacheKey = String;

/// What a probed address carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoogleTranslateIpCacheRecord {
    pub latency_ms: i64,
    pub created_at: i64,
}

impl Collection for GoogleTranslateIpCache {
    type Key = GoogleTranslateIpCacheKey;
    type Scope = Self::Key;
    type Record = GoogleTranslateIpCacheRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, i64)> = sqlx::query_as(
            "
            SELECT latency_ms, created_at
            FROM google_translate_ip_cache
            WHERE ip = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(latency_ms, created_at)| Self::Record {
            latency_ms,
            created_at,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM google_translate_ip_cache WHERE ip = ?")
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
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "
            SELECT ip, latency_ms, created_at
            FROM google_translate_ip_cache
            ORDER BY latency_ms ASC, ip ASC
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(ip, latency_ms, created_at)| {
                (
                    ip,
                    Self::Record {
                        latency_ms,
                        created_at,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM google_translate_ip_cache")
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
    key: &GoogleTranslateIpCacheKey,
    record: &GoogleTranslateIpCacheRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO google_translate_ip_cache (ip, latency_ms, created_at)
        VALUES (?, ?, ?)
        ON CONFLICT (ip) DO UPDATE SET
            latency_ms = excluded.latency_ms,
            created_at = excluded.created_at
        ",
    )
    .bind(key)
    .bind(record.latency_ms)
    .bind(record.created_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
