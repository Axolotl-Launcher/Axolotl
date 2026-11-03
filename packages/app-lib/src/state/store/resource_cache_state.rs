//! Which generation of the resource cache is current.
//!
//! Clearing the cache bumps the generation, which is what a write started
//! before the clear checks itself against: an answer that was asked for before
//! the cache was cleared must not land in the emptied cache.

use sqlx::SqliteConnection;

use super::Collection;

/// The one generation the cache has.
pub(crate) struct ResourceCacheState;

/// The state is a single row, so its key carries nothing.
pub(crate) type ResourceCacheStateKey = ();

/// What the state carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceCacheStateRecord {
    pub generation: i64,
}

impl Collection for ResourceCacheState {
    type Key = ResourceCacheStateKey;
    type Scope = Self::Key;
    type Record = ResourceCacheStateRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let generation: Option<i64> = sqlx::query_scalar(
            "SELECT generation FROM resource_cache_state WHERE id = 1",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(generation.map(|generation| Self::Record { generation }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM resource_cache_state WHERE id = 1")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, &())
            .await?
            .map(|record| vec![((), record)])
            .unwrap_or_default())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, &(), records.first().map(|(_, record)| record)).await
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(*scope, record)])
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
    record: &ResourceCacheStateRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_cache_state (id, generation)
        VALUES (1, ?)
        ON CONFLICT (id) DO UPDATE SET generation = excluded.generation
        ",
    )
    .bind(record.generation)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
