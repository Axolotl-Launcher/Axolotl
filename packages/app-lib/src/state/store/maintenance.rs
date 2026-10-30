//! Database maintenance statements stay behind the storage boundary.
use super::Handle;
use serde::Serialize;
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn integrity_report_detects_foreign_key_damage_without_exposing_rows()
    {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id))").execute(&pool).await.unwrap();
        let handle = Handle::new(pool.clone());
        let healthy = check(&handle).await.unwrap();
        assert!(healthy.integrity_ok);
        assert_eq!(healthy.foreign_key_issue_count, 0);
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO child VALUES (1,99)")
            .execute(&pool)
            .await
            .unwrap();
        let broken = check(&handle).await.unwrap();
        assert!(broken.integrity_ok);
        assert_eq!(broken.foreign_key_issue_count, 1);
        assert!(
            !serde_json::to_string(&broken)
                .unwrap()
                .contains("parent_id")
        );
        pool.close().await;
    }
}
#[derive(Serialize)]
pub struct IntegrityReport {
    pub integrity_ok: bool,
    pub integrity_issue_count: usize,
    pub foreign_key_issue_count: usize,
}
pub(crate) async fn check(handle: &Handle) -> crate::Result<IntegrityReport> {
    let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(handle.pool())
        .await?;
    let foreign = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(handle.pool())
        .await?;
    let issues = integrity.iter().filter(|v| v.as_str() != "ok").count();
    Ok(IntegrityReport {
        integrity_ok: issues == 0,
        integrity_issue_count: issues,
        foreign_key_issue_count: foreign.len(),
    })
}

pub(crate) fn database_path(handle: &Handle) -> std::path::PathBuf {
    handle.pool().connect_options().get_filename().to_path_buf()
}
