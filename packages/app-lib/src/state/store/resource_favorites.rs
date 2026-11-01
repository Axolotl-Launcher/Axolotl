//! The resources the user saved, one row per provider project.

use sqlx::SqliteConnection;

use super::{Collection, Handle};

/// The saved resources.
pub(crate) struct ResourceFavorites;

/// A favorite is identified by the project it was saved from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceFavoriteKey {
    pub source: String,
    pub project_id: String,
}

/// What a favorite carries beyond its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceFavoriteRecord {
    pub content_type: String,
    pub saved_at: i64,
}

/// A saved favorite as it was written: the project it names, its content
/// type, when it was first saved, and the resource its project maps to.
pub(crate) type SavedRow = (String, String, String, i64, Option<String>);

/// One project of a resource a favorite names, with the alias it was last
/// recorded under.
pub(crate) type FavoritedProjectRow = (String, String, String, Option<String>);

impl Collection for ResourceFavorites {
    type Key = ResourceFavoriteKey;
    type Scope = ResourceFavoriteKey;
    type Record = ResourceFavoriteRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "
            SELECT content_type, saved_at
            FROM resource_favorites
            WHERE source = ? AND project_id = ?
            ",
        )
        .bind(&key.source)
        .bind(&key.project_id)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(content_type, saved_at)| Self::Record {
            content_type,
            saved_at,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "
                DELETE FROM resource_favorites
                WHERE source = ? AND project_id = ?
                ",
            )
            .bind(&key.source)
            .bind(&key.project_id)
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
            "
            SELECT source, project_id, content_type, saved_at
            FROM resource_favorites
            ORDER BY saved_at DESC, source, project_id
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(source, project_id, content_type, saved_at)| {
                (
                    ResourceFavoriteKey { source, project_id },
                    Self::Record {
                        content_type,
                        saved_at,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_favorites")
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

impl ResourceFavorites {
    /// Saves `key` now, or replaces the content type of the favorite already
    /// saved and keeps the time it was first saved.
    pub(crate) async fn save(
        handle: &Handle,
        key: &ResourceFavoriteKey,
        content_type: &str,
        saved_at: i64,
    ) -> crate::Result<()> {
        let mut connection = handle.connection().await?;
        sqlx::query(
            "
            INSERT INTO resource_favorites
                (source, project_id, content_type, saved_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(source, project_id) DO UPDATE SET
                content_type = excluded.content_type
            ",
        )
        .bind(&key.source)
        .bind(&key.project_id)
        .bind(content_type)
        .bind(saved_at)
        .execute(&mut *connection)
        .await?;

        Ok(())
    }

    /// Every saved favorite, newest first.
    pub(crate) async fn saved(handle: &Handle) -> crate::Result<Vec<SavedRow>> {
        let rows: Vec<SavedRow> = sqlx::query_as(
            "
            SELECT favorite.source, favorite.project_id,
                favorite.content_type, favorite.saved_at,
                project.resource_id
            FROM resource_favorites AS favorite
            LEFT JOIN resource_source_projects AS project
                ON project.source = favorite.source
                AND project.project_id = favorite.project_id
            ORDER BY favorite.saved_at DESC, favorite.source,
                favorite.project_id
            ",
        )
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }

    /// The projects of every resource a favorite names, with the alias the
    /// project was last recorded under.
    pub(crate) async fn favorited_projects(
        handle: &Handle,
    ) -> crate::Result<Vec<FavoritedProjectRow>> {
        let rows: Vec<FavoritedProjectRow> = sqlx::query_as(
            "
            SELECT project.resource_id, project.source, project.project_id,
                (SELECT alias FROM resource_aliases alias
                 WHERE alias.source = project.source
                   AND alias.project_id = project.project_id
                 ORDER BY rowid DESC LIMIT 1) AS alias
            FROM resource_source_projects project
            WHERE project.resource_id IN (
                SELECT project.resource_id
                FROM resource_source_projects project
                JOIN resource_favorites favorite
                    ON favorite.source = project.source
                    AND favorite.project_id = project.project_id)
            ORDER BY project.source, project.project_id
            ",
        )
        .fetch_all(handle.pool())
        .await?;

        Ok(rows)
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &ResourceFavoriteKey,
    record: &ResourceFavoriteRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_favorites
            (source, project_id, content_type, saved_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(source, project_id) DO UPDATE SET
            content_type = excluded.content_type,
            saved_at = excluded.saved_at
        ",
    )
    .bind(&key.source)
    .bind(&key.project_id)
    .bind(&record.content_type)
    .bind(record.saved_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
