//! How far the shared game settings have been revised.

use sqlx::SqliteConnection;

use super::Collection;

/// The state, kept once for the whole launcher.
pub(crate) struct SyncedGameOptionState;

/// There is one state per launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singleton;

/// What the state carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GameOptionStateRecord {
    pub revision: i64,
    pub catalog_revision: i64,
}

impl Collection for SyncedGameOptionState {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = GameOptionStateRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, i64)> = sqlx::query_as(
            "
            SELECT revision, catalog_revision
            FROM synced_game_option_state
            WHERE singleton = 1
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(revision, catalog_revision)| Self::Record {
            revision,
            catalog_revision,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM synced_game_option_state")
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
        sqlx::query("DELETE FROM synced_game_option_state")
            .execute(&mut *conn)
            .await?;
        for (_, record) in records {
            write_row(conn, record).await?;
        }
        Ok(())
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
    record: &GameOptionStateRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO synced_game_option_state
            (singleton, revision, catalog_revision)
        VALUES (1, ?, ?)
        ON CONFLICT (singleton) DO UPDATE SET
            revision = excluded.revision,
            catalog_revision = excluded.catalog_revision
        ",
    )
    .bind(record.revision)
    .bind(record.catalog_revision)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
