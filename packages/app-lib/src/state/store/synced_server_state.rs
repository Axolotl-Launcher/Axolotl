//! How far the canonical multiplayer server list has been revised.

use sqlx::SqliteConnection;

use super::{Atom, Collection};

/// The state, kept once for the whole launcher.
pub(crate) struct SyncedServerState;

/// There is one state per launcher.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub(crate) struct Singleton;

/// What the state carries.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub(crate) struct SyncedServerStateRecord {
    pub revision: i64,
}

impl Collection for SyncedServerState {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = SyncedServerStateRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let revision: Option<i64> = sqlx::query_scalar(
            "
            SELECT revision
            FROM synced_server_state
            WHERE singleton = 1
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(revision.map(|revision| Self::Record { revision }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM synced_server_state")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        sqlx::query(
            "
            INSERT INTO synced_server_state (singleton, revision)
            VALUES (1, ?)
            ON CONFLICT(singleton) DO UPDATE SET
                revision = excluded.revision
            ",
        )
        .bind(record.revision)
        .execute(&mut *conn)
        .await?;

        Ok(())
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

impl SyncedServerState {
    /// Records that a list has been committed, without claiming a revision.
    pub(crate) async fn ensure_in(atom: &mut Atom) -> crate::Result<()> {
        if Self::atom_get(atom, &Singleton).await?.is_none() {
            Self::atom_set(
                atom,
                &Singleton,
                Some(&SyncedServerStateRecord { revision: 0 }),
            )
            .await?;
        }
        Ok(())
    }

    /// Records one more revision, inside the unit that changed the list.
    pub(crate) async fn bump_in(atom: &mut Atom) -> crate::Result<()> {
        let revision = Self::atom_get(atom, &Singleton)
            .await?
            .map(|record| record.revision)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Server sync revision exhausted".into(),
                )
            })?;
        Self::atom_set(
            atom,
            &Singleton,
            Some(&SyncedServerStateRecord { revision }),
        )
        .await
    }
}

impl crate::state::sync_state::collection::Collection for SyncedServerState {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = SyncedServerStateRecord;
    const NAME: &'static str = "synced_server_state";
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }
}
crate::state::sync_state::collection::verbs!(
    SyncedServerState,
    Singleton,
    Singleton,
    SyncedServerStateRecord
);
