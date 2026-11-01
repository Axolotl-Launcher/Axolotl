//! Repository directories left behind by a move, waiting to be removed.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The directories waiting to be removed.
pub(crate) struct PendingBackupRepositoryCleanups;

/// A directory is identified by its path.
pub(crate) type CleanupPath = String;

/// What a cleanup request carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CleanupRecord {
    pub created_at: i64,
}

impl Collection for PendingBackupRepositoryCleanups {
    type Key = CleanupPath;
    type Scope = CleanupPath;
    type Record = CleanupRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let created_at: Option<i64> = sqlx::query_scalar(
            "
            SELECT created_at
            FROM pending_backup_repository_cleanups
            WHERE path = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(created_at.map(|created_at| Self::Record { created_at }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM pending_backup_repository_cleanups WHERE path = ?",
            )
            .bind(key)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        sqlx::query(
            "
            INSERT INTO pending_backup_repository_cleanups (path, created_at)
            VALUES (?, ?)
            ON CONFLICT(path) DO UPDATE SET created_at = excluded.created_at
            ",
        )
        .bind(key)
        .bind(record.created_at)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "
            SELECT path, created_at
            FROM pending_backup_repository_cleanups
            ORDER BY created_at
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(path, created_at)| (path, Self::Record { created_at }))
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM pending_backup_repository_cleanups")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
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

impl PendingBackupRepositoryCleanups {
    /// The directories waiting to be removed, oldest first.
    pub(crate) async fn pending_paths(
        handle: &Handle,
    ) -> crate::Result<Vec<String>> {
        Ok(Self::get_all(handle)
            .await?
            .into_iter()
            .map(|(path, _)| path)
            .collect())
    }
}
