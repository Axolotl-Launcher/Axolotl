//! The multiplayer server list the launcher keeps for every instance that
//! syncs servers.
//!
//! A server carries the payload it is stored as; what the payload says is the
//! caller's business. The list has an order, and every read answers it.

use sqlx::SqliteConnection;

use super::{Atom, Collection, Handle};

/// The canonical server list.
pub(crate) struct SyncedServers;

/// A server is identified by its own id.
pub(crate) type SyncedServerKey = String;

/// What a server carries beyond its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncedServerRecord {
    /// Where the server sits in the list.
    pub position: i64,
    pub nbt: Vec<u8>,
}

impl Collection for SyncedServers {
    type Key = SyncedServerKey;
    type Scope = SyncedServerKey;
    type Record = SyncedServerRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, Vec<u8>)> = sqlx::query_as(
            "
            SELECT position, nbt
            FROM synced_servers
            WHERE id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(position, nbt)| Self::Record { position, nbt }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM synced_servers WHERE id = ?")
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
        let rows: Vec<(String, i64, Vec<u8>)> = sqlx::query_as(
            "
            SELECT id, position, nbt
            FROM synced_servers
            ORDER BY position
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, position, nbt)| (id, Self::Record { position, nbt }))
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM synced_servers")
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

impl SyncedServers {
    /// Where `id` sits, or the position a server appended to the list takes.
    pub(crate) async fn position_for(
        handle: &Handle,
        id: &str,
    ) -> crate::Result<i64> {
        let mut connection = handle.connection().await?;
        let position: i64 = sqlx::query_scalar(
            "
            SELECT COALESCE(
                (SELECT position FROM synced_servers WHERE id = ?),
                (SELECT COALESCE(MAX(position), -1) + 1 FROM synced_servers)
            )
            ",
        )
        .bind(id)
        .fetch_one(&mut *connection)
        .await?;

        Ok(position)
    }

    /// Moves every server well past the list's length, so the list can be
    /// written back in one order without two rows claiming the same position.
    pub(crate) async fn shift_positions_in(
        atom: &mut Atom,
        offset: i64,
    ) -> crate::Result<()> {
        sqlx::query("UPDATE synced_servers SET position = position + ?")
            .bind(offset)
            .execute(atom.connection())
            .await?;

        Ok(())
    }

    /// How many servers the list holds and how many payload bytes they carry,
    /// read before the list itself so an oversized one is refused unread.
    pub(crate) async fn stored_size(
        handle: &Handle,
    ) -> crate::Result<(usize, usize)> {
        let mut connection = handle.connection().await?;
        stored_size_rows(&mut connection).await
    }

    /// [`Self::stored_size`] inside `atom`.
    pub(crate) async fn stored_size_in(
        atom: &mut Atom,
    ) -> crate::Result<(usize, usize)> {
        stored_size_rows(atom.connection()).await
    }
}

async fn stored_size_rows(
    conn: &mut SqliteConnection,
) -> crate::Result<(usize, usize)> {
    let row: (i64, i64) = sqlx::query_as(
        "
        SELECT COUNT(*), COALESCE(SUM(length(nbt)), 0)
        FROM synced_servers
        ",
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok((
        row.0.try_into().unwrap_or(usize::MAX),
        row.1.try_into().unwrap_or(usize::MAX),
    ))
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &SyncedServerKey,
    record: &SyncedServerRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_servers (id, position, nbt)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            position = excluded.position,
            nbt = excluded.nbt
        ",
    )
    .bind(key)
    .bind(record.position)
    .bind(&record.nbt)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
