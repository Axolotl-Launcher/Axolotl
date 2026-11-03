//! The translations the launcher has already fetched.
//!
//! A translation is kept until it is older than the cache allows; the entry
//! point that clears the cache removes every row.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The cached translations, one row per translated request.
pub(crate) struct TranslationCache;

/// A translation is identified by the digest of what was translated.
pub(crate) type TranslationCacheKey = String;

/// What a cached translation carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranslationCacheRecord {
    pub translation: String,
    pub created_at: i64,
}

impl Collection for TranslationCache {
    type Key = TranslationCacheKey;
    type Scope = Self::Key;
    type Record = TranslationCacheRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "
            SELECT translation, created_at
            FROM translation_cache
            WHERE key = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(translation, created_at)| Self::Record {
            translation,
            created_at,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM translation_cache WHERE key = ?")
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
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "
            SELECT key, translation, created_at
            FROM translation_cache
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(key, translation, created_at)| {
                (
                    key,
                    Self::Record {
                        translation,
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
        sqlx::query("DELETE FROM translation_cache")
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

impl TranslationCache {
    /// Removes the translations created before `cutoff`.
    pub(crate) async fn delete_created_before(
        handle: &Handle,
        cutoff: i64,
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM translation_cache WHERE created_at < ?")
            .bind(cutoff)
            .execute(handle.pool())
            .await?;
        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &TranslationCacheKey,
    record: &TranslationCacheRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO translation_cache (key, translation, created_at)
        VALUES (?, ?, ?)
        ON CONFLICT (key) DO UPDATE SET
            translation = excluded.translation,
            created_at = excluded.created_at
        ",
    )
    .bind(key)
    .bind(&record.translation)
    .bind(record.created_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
