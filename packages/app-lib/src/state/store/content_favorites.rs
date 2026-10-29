//! The content a person keeps for later.

use sqlx::SqliteConnection;

use super::Collection;
use crate::state::content_types::{
    ContentFavoriteProvider, ContentFavoriteType,
};

/// The favorites, one per provider project.
pub(crate) struct ContentFavorites;

/// A favorite is identified by the provider project it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContentFavoriteKey {
    pub provider: ContentFavoriteProvider,
    pub project_id: String,
}

/// What a favorite carries besides the project it points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContentFavoriteRecord {
    pub content_type: ContentFavoriteType,
    pub saved_at: i64,
}

impl Collection for ContentFavorites {
    type Key = ContentFavoriteKey;
    type Scope = ContentFavoriteKey;
    type Record = ContentFavoriteRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT content_type, saved_at
             FROM content_favorites
             WHERE provider = ? AND project_id = ?",
        )
        .bind(key.provider.as_str())
        .bind(&key.project_id)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.and_then(|(content_type, saved_at)| {
            Some(Self::Record {
                content_type: to_type(&content_type)?,
                saved_at,
            })
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM content_favorites
                 WHERE provider = ? AND project_id = ?",
            )
            .bind(key.provider.as_str())
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
            "SELECT provider, project_id, content_type, saved_at
             FROM content_favorites",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(provider, project_id, content_type, saved_at)| {
                Some((
                    ContentFavoriteKey {
                        provider: to_provider(&provider)?,
                        project_id,
                    },
                    ContentFavoriteRecord {
                        content_type: to_type(&content_type)?,
                        saved_at,
                    },
                ))
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM content_favorites")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            insert_row(conn, key, record).await?;
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
        sqlx::query(
            "DELETE FROM content_favorites WHERE provider = ? AND project_id = ?",
        )
        .bind(scope.provider.as_str())
        .bind(&scope.project_id)
        .execute(&mut *conn)
        .await?;
        for (key, record) in records {
            insert_row(conn, key, record).await?;
        }
        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &ContentFavoriteKey,
    record: &ContentFavoriteRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO content_favorites
            (provider, project_id, content_type, saved_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (provider, project_id) DO UPDATE SET
            content_type = excluded.content_type
        ",
    )
    .bind(key.provider.as_str())
    .bind(&key.project_id)
    .bind(record.content_type.as_str())
    .bind(record.saved_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// A replace writes the moment it was given, so a batch carries its own times.
async fn insert_row(
    conn: &mut SqliteConnection,
    key: &ContentFavoriteKey,
    record: &ContentFavoriteRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO content_favorites
            (provider, project_id, content_type, saved_at)
        VALUES (?, ?, ?, ?)
        ",
    )
    .bind(key.provider.as_str())
    .bind(&key.project_id)
    .bind(record.content_type.as_str())
    .bind(record.saved_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// The provider a stored favorite names, or nothing when this build does not
/// know it: a favorite is only useful while its identity can be read.
fn to_provider(provider: &str) -> Option<ContentFavoriteProvider> {
    ContentFavoriteProvider::parse(provider)
        .map_err(|error| {
            tracing::warn!(
                %provider,
                %error,
                "Skipping a favorite whose provider this build does not know"
            );
            error
        })
        .ok()
}

fn to_type(content_type: &str) -> Option<ContentFavoriteType> {
    ContentFavoriteType::parse(content_type)
        .map_err(|error| {
            tracing::warn!(
                %content_type,
                %error,
                "Skipping a favorite whose content type this build does not know"
            );
            error
        })
        .ok()
}
