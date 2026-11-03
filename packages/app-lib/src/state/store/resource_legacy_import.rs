//! How far the import of the legacy cache got.

use sqlx::SqliteConnection;

use super::Collection;

/// The import checkpoint, a single row.
pub(crate) struct ResourceLegacyImport;

/// The checkpoint is one row, so its key says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singleton;

/// What the checkpoint carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceLegacyImportRecord {
    pub last_rowid: i64,
    pub completed: bool,
}

impl Collection for ResourceLegacyImport {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = ResourceLegacyImportRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, bool)> = sqlx::query_as(
            "SELECT last_rowid, completed FROM resource_legacy_import WHERE id = 1",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(last_rowid, completed)| Self::Record {
            last_rowid,
            completed,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        match record {
            Some(record) => {
                sqlx::query(
                    "
                    INSERT INTO resource_legacy_import (
                        id, last_rowid, completed
                    )
                    VALUES (1, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        last_rowid = excluded.last_rowid,
                        completed = excluded.completed
                    ",
                )
                .bind(record.last_rowid)
                .bind(record.completed)
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM resource_legacy_import")
                    .execute(&mut *conn)
                    .await?;
            }
        }

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
        _scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Self::read_all(conn).await
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        _scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::replace_all(conn, records).await
    }
}
