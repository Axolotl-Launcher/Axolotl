use serde::{Deserialize, Serialize};
use sqlx::Row;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFavoriteProvider {
    Modrinth,
    Curseforge,
    Mcarchive,
}

impl ContentFavoriteProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
            Self::Mcarchive => "mcarchive",
        }
    }

    fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "modrinth" => Ok(Self::Modrinth),
            "curseforge" => Ok(Self::Curseforge),
            "mcarchive" => Ok(Self::Mcarchive),
            _ => Err(crate::ErrorKind::InputError(format!(
                "Unsupported content favorite provider: {value}"
            ))
            .into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFavoriteType {
    Mod,
    Resourcepack,
    Datapack,
    Shader,
}

impl ContentFavoriteType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Resourcepack => "resourcepack",
            Self::Datapack => "datapack",
            Self::Shader => "shader",
        }
    }

    fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "mod" => Ok(Self::Mod),
            "resourcepack" => Ok(Self::Resourcepack),
            "datapack" => Ok(Self::Datapack),
            "shader" => Ok(Self::Shader),
            _ => Err(crate::ErrorKind::InputError(format!(
                "Unsupported content favorite type: {value}"
            ))
            .into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentFavorite {
    #[serde(default)]
    pub resource_id: Option<String>,
    pub provider: ContentFavoriteProvider,
    pub project_id: String,
    pub content_type: ContentFavoriteType,
    pub saved_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentFavoriteInput {
    pub provider: ContentFavoriteProvider,
    pub project_id: String,
    pub content_type: ContentFavoriteType,
}

impl ContentFavoriteInput {
    fn validate(&self) -> crate::Result<()> {
        if self.project_id.trim().is_empty() {
            return Err(crate::ErrorKind::InputError(
                "A content favorite must have a project ID".to_string(),
            )
            .into());
        }
        Ok(())
    }
}

fn from_row(row: sqlx::sqlite::SqliteRow) -> crate::Result<ContentFavorite> {
    let provider: String = row.try_get("provider")?;
    let content_type: String = row.try_get("content_type")?;
    Ok(ContentFavorite {
        resource_id: row.try_get("resource_id")?,
        provider: ContentFavoriteProvider::parse(&provider)?,
        project_id: row.try_get("project_id")?,
        content_type: ContentFavoriteType::parse(&content_type)?,
        saved_at: row.try_get("saved_at")?,
    })
}

pub async fn list(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<Vec<ContentFavorite>> {
    let rows = sqlx::query(
        "
        SELECT provider, project_id, content_type, saved_at,
            (SELECT resource_id FROM resource_source_projects
                WHERE source = content_favorites.provider AND project_id = content_favorites.project_id) AS resource_id
		FROM content_favorites
		ORDER BY saved_at DESC, provider ASC, project_id ASC
		",
    )
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(from_row).collect()
}

pub async fn add(
    favorite: ContentFavoriteInput,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<ContentFavorite> {
    favorite.validate()?;
    let project_id = favorite.project_id.trim();
    let provider = favorite.provider.as_str();
    let content_type = favorite.content_type.as_str();

    sqlx::query(
        "
		INSERT INTO content_favorites (provider, project_id, content_type, saved_at)
		VALUES (?, ?, ?, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
		ON CONFLICT (provider, project_id) DO UPDATE SET
			content_type = excluded.content_type
		",
    )
    .bind(provider)
    .bind(project_id)
    .bind(content_type)
    .execute(exec)
    .await?;

    let row = sqlx::query(
        "
        SELECT provider, project_id, content_type, saved_at,
            (SELECT resource_id FROM resource_source_projects
                WHERE source = content_favorites.provider AND project_id = content_favorites.project_id) AS resource_id
		FROM content_favorites
		WHERE provider = ? AND project_id = ?
		",
    )
    .bind(provider)
    .bind(project_id)
    .fetch_one(exec)
    .await?;

    from_row(row)
}

pub async fn remove(
    provider: ContentFavoriteProvider,
    project_id: &str,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    if project_id.trim().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "A content favorite must have a project ID".to_string(),
        )
        .into());
    }

    sqlx::query(
        "DELETE FROM content_favorites WHERE provider = ? AND project_id = ?",
    )
    .bind(provider.as_str())
    .bind(project_id.trim())
    .execute(exec)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ContentFavoriteInput, ContentFavoriteProvider, ContentFavoriteType,
        add, list, remove,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use std::time::Duration;

    async fn pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::migrate!().run(&pool).await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn content_favorites_migration_preserves_existing_database_rows() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::raw_sql(
			"CREATE TABLE existing_content (id INTEGER PRIMARY KEY, marker TEXT NOT NULL); INSERT INTO existing_content (marker) VALUES ('keep');",
		)
		.execute(&pool)
		.await
		.expect("legacy database data");

        sqlx::raw_sql(include_str!(
            "../../migrations/20260820200000_content-favorites.sql"
        ))
        .execute(&pool)
        .await
        .expect("apply content favorites migration");

        let marker: String = sqlx::query_scalar(
            "SELECT marker FROM existing_content WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("legacy row after migration");
        let table_exists: i64 = sqlx::query_scalar(
			"SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'content_favorites'",
		)
		.fetch_one(&pool)
		.await
		.expect("content favorites table query");

        assert_eq!(marker, "keep");
        assert_eq!(table_exists, 1);
    }

    #[tokio::test]
    async fn content_favorites_are_provider_qualified_and_idempotent() {
        let pool = pool().await;
        let modrinth = add(
            ContentFavoriteInput {
                provider: ContentFavoriteProvider::Modrinth,
                project_id: "same-id".to_string(),
                content_type: ContentFavoriteType::Mod,
            },
            &pool,
        )
        .await
        .expect("add Modrinth favorite");
        let duplicate = add(
            ContentFavoriteInput {
                provider: ContentFavoriteProvider::Modrinth,
                project_id: "same-id".to_string(),
                content_type: ContentFavoriteType::Mod,
            },
            &pool,
        )
        .await
        .expect("add duplicate favorite");
        add(
            ContentFavoriteInput {
                provider: ContentFavoriteProvider::Curseforge,
                project_id: "same-id".to_string(),
                content_type: ContentFavoriteType::Shader,
            },
            &pool,
        )
        .await
        .expect("add CurseForge favorite");

        let favorites = list(&pool).await.expect("list favorites");
        assert_eq!(favorites.len(), 2);
        assert_eq!(duplicate.saved_at, modrinth.saved_at);
        assert!(favorites.iter().any(|favorite| {
            favorite.provider == ContentFavoriteProvider::Modrinth
                && favorite.project_id == "same-id"
        }));
        assert!(favorites.iter().any(|favorite| {
            favorite.provider == ContentFavoriteProvider::Curseforge
                && favorite.project_id == "same-id"
        }));
    }

    #[tokio::test]
    async fn removing_a_content_favorite_is_idempotent() {
        let pool = pool().await;
        remove(ContentFavoriteProvider::Modrinth, "missing", &pool)
            .await
            .expect("remove missing favorite");
        assert!(list(&pool).await.expect("list favorites").is_empty());
    }

    #[tokio::test]
    async fn content_favorites_reject_invalid_database_values_and_sort_by_saved_time()
     {
        let pool = pool().await;
        sqlx::query(
			"INSERT INTO content_favorites (provider, project_id, content_type, saved_at) VALUES ('modrinth', 'older', 'mod', 1)",
		)
		.execute(&pool)
		.await
		.expect("insert valid favorite");
        sqlx::query(
			"INSERT INTO content_favorites (provider, project_id, content_type, saved_at) VALUES ('curseforge', 'newer', 'shader', 2)",
		)
		.execute(&pool)
		.await
		.expect("insert valid favorite");

        assert!(sqlx::query(
			"INSERT INTO content_favorites (provider, project_id, content_type, saved_at) VALUES ('unknown', 'bad-provider', 'mod', 3)",
		)
		.execute(&pool)
		.await
		.is_err());
        assert!(sqlx::query(
			"INSERT INTO content_favorites (provider, project_id, content_type, saved_at) VALUES ('modrinth', 'bad-type', 'modpack', 3)",
		)
		.execute(&pool)
		.await
		.is_err());

        let favorites = list(&pool).await.expect("list favorites");
        assert_eq!(
            favorites
                .iter()
                .map(|favorite| favorite.project_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    #[tokio::test]
    async fn readding_after_remove_gets_a_new_saved_time() {
        let pool = pool().await;
        let first = add(
            ContentFavoriteInput {
                provider: ContentFavoriteProvider::Modrinth,
                project_id: "sodium".to_string(),
                content_type: ContentFavoriteType::Mod,
            },
            &pool,
        )
        .await
        .expect("add favorite");
        remove(ContentFavoriteProvider::Modrinth, "sodium", &pool)
            .await
            .expect("remove favorite");
        tokio::time::sleep(Duration::from_millis(2)).await;
        let second = add(
            ContentFavoriteInput {
                provider: ContentFavoriteProvider::Modrinth,
                project_id: "sodium".to_string(),
                content_type: ContentFavoriteType::Mod,
            },
            &pool,
        )
        .await
        .expect("re-add favorite");

        assert!(second.saved_at > first.saved_at);
    }
}
