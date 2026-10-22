use serde::{Deserialize, Serialize};

use crate::State;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrashAnalysisAiSettings {
    pub enabled: bool,
    pub provider_id: String,
    pub model_id: String,
    pub ai_source: String,
}

pub async fn get_crash_analysis_ai_settings()
-> crate::Result<CrashAnalysisAiSettings> {
    let state = State::get().await?;
    let row = sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT enabled, provider_id, model_id, ai_source FROM crash_analysis_ai_settings WHERE id = 0",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(CrashAnalysisAiSettings {
        enabled: row.0 != 0,
        provider_id: row.1,
        model_id: row.2,
        ai_source: row.3,
    })
}

pub async fn update_crash_analysis_ai_settings(
    settings: CrashAnalysisAiSettings,
) -> crate::Result<()> {
    let state = State::get().await?;
    save_settings(&state.pool, settings).await
}

async fn save_settings(
    pool: &sqlx::SqlitePool,
    settings: CrashAnalysisAiSettings,
) -> crate::Result<()> {
    let ai_source = normalize_ai_source(&settings.ai_source);
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "UPDATE crash_analysis_ai_settings SET enabled = ?, provider_id = ?, model_id = ?, ai_source = ? WHERE id = 0",
    )
    .bind(settings.enabled)
    .bind(settings.provider_id.trim())
    .bind(settings.model_id.trim())
    .bind(ai_source)
    .execute(&mut *transaction)
    .await?;
    sqlx::query("UPDATE log_share_settings SET ai_source = ? WHERE id = 0")
        .bind(ai_source)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

fn normalize_ai_source(ai_source: &str) -> &str {
    if ai_source == "custom" {
        "custom"
    } else {
        "logshare"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saves_authoritative_source_and_mirror_atomically() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE crash_analysis_ai_settings (id INTEGER PRIMARY KEY, enabled BOOLEAN, provider_id TEXT, model_id TEXT, ai_source TEXT)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO crash_analysis_ai_settings VALUES (0, 0, '', '', 'logshare')").execute(&pool).await.unwrap();
        let settings = CrashAnalysisAiSettings {
            enabled: true,
            provider_id: " provider ".into(),
            model_id: " model ".into(),
            ai_source: "custom".into(),
        };
        assert!(save_settings(&pool, settings.clone()).await.is_err());
        let source: String = sqlx::query_scalar(
            "SELECT ai_source FROM crash_analysis_ai_settings",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source, "logshare");
        sqlx::query("CREATE TABLE log_share_settings (id INTEGER PRIMARY KEY, ai_source TEXT)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO log_share_settings VALUES (0, 'logshare')")
            .execute(&pool)
            .await
            .unwrap();
        save_settings(&pool, settings).await.unwrap();
        let row: (String, String, String, String) = sqlx::query_as("SELECT a.provider_id, a.model_id, a.ai_source, s.ai_source FROM crash_analysis_ai_settings a JOIN log_share_settings s ON s.id = a.id").fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "provider".into(),
                "model".into(),
                "custom".into(),
                "custom".into()
            )
        );
    }
}
