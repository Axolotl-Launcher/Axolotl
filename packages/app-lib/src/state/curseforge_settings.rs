//! Persistence for the CurseForge API key set in the launcher.
//!
//! An empty value means the launcher does not override the key it was built
//! with, which is the default for every database.

use sqlx::{Executor, Sqlite};

const API_KEY_QUERY: &str =
    "SELECT curseforge_api_key FROM settings WHERE id = 0";

pub(crate) fn normalize(api_key: Option<String>) -> Option<String> {
    api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

pub async fn get<'a, E>(exec: E) -> crate::Result<Option<String>>
where
    E: Executor<'a, Database = Sqlite> + Copy,
{
    let api_key: String =
        sqlx::query_scalar(API_KEY_QUERY).fetch_one(exec).await?;
    Ok(normalize(Some(api_key)))
}

pub async fn set<'a, E>(exec: E, api_key: Option<&str>) -> crate::Result<()>
where
    E: Executor<'a, Database = Sqlite> + Copy,
{
    let api_key = normalize(api_key.map(str::to_string));
    sqlx::query("UPDATE settings SET curseforge_api_key = ? WHERE id = 0")
        .bind(api_key.unwrap_or_default())
        .execute(exec)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn defaults_to_no_override_and_round_trips() {
        let pool = test_pool().await;
        assert_eq!(get(&pool).await.unwrap(), None);

        set(&pool, Some(" test-key ")).await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), Some("test-key".to_string()));

        set(&pool, None).await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), None);
    }

    #[tokio::test]
    async fn blank_values_clear_the_override() {
        let pool = test_pool().await;
        set(&pool, Some("test-key")).await.unwrap();
        set(&pool, Some("   ")).await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), None);
    }

    #[tokio::test]
    async fn upgrade_keeps_settings_without_override() {
        let pool = test_pool().await;
        sqlx::query(
            "UPDATE settings SET locale = 'zh-CN', developer_mode = 1 WHERE id = 0",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Rebuild the pre-migration shape: the column did not exist yet.
        sqlx::query("ALTER TABLE settings DROP COLUMN curseforge_api_key")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(include_str!(
            "../../migrations/20261022120000_curseforge-api-key.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(get(&pool).await.unwrap(), None);
        let (locale, developer_mode): (String, bool) = sqlx::query_as(
            "SELECT locale, developer_mode FROM settings WHERE id = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(locale, "zh-CN");
        assert!(developer_mode);
    }
}
