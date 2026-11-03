//! The creative hotbar layout the launcher keeps for every instance that
//! syncs hotbars.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The hotbar state, kept once for the whole launcher.
pub(crate) struct SyncedHotbarState;

/// There is one state per launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singleton;

/// What the state carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HotbarStateRecord {
    pub schema_version: i64,
    pub revision: i64,
    pub nbt: Vec<u8>,
}

impl Collection for SyncedHotbarState {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = HotbarStateRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, i64, Vec<u8>)> = sqlx::query_as(
            "
            SELECT schema_version, revision, nbt
            FROM synced_hotbar_state
            WHERE singleton = 1
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(schema_version, revision, nbt)| Self::Record {
            schema_version,
            revision,
            nbt,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM synced_hotbar_state")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, &Singleton)
            .await?
            .map(|record| vec![(Singleton, record)])
            .unwrap_or_default())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, &Singleton, records.first().map(|(_, record)| record))
            .await
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let _ = scope;
        Self::read_all(conn).await
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let _ = scope;
        Self::replace_all(conn, records).await
    }
}

impl SyncedHotbarState {
    /// Whether a layout has ever been written.
    pub(crate) async fn exists(handle: &Handle) -> crate::Result<bool> {
        Ok(Self::get(handle, &Singleton).await?.is_some())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    record: &HotbarStateRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_hotbar_state
            (singleton, schema_version, revision, nbt)
        VALUES (1, ?, ?, ?)
        ON CONFLICT(singleton) DO UPDATE SET
            schema_version = excluded.schema_version,
            revision = excluded.revision,
            nbt = excluded.nbt
        ",
    )
    .bind(record.schema_version)
    .bind(record.revision)
    .bind(&record.nbt)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
