//! Database maintenance statements stay behind the storage boundary.
mod restore;
use super::Handle;
pub(crate) use restore::{finish_restore, restore_snapshot};
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
    let mut connection = handle.pool().acquire().await?;
    let schemas: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_database_list WHERE name <> 'temp'",
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut issues = 0;
    let mut foreign_issues = 0;
    for schema in schemas {
        let schema = schema.replace('"', "\"\"");
        let integrity: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(
            format!("PRAGMA \"{schema}\".integrity_check").as_str(),
        ))
        .fetch_all(&mut *connection)
        .await?;
        issues += integrity
            .iter()
            .filter(|value| value.as_str() != "ok")
            .count();
        foreign_issues += sqlx::query(sqlx::AssertSqlSafe(
            format!("PRAGMA \"{schema}\".foreign_key_check").as_str(),
        ))
        .fetch_all(&mut *connection)
        .await?
        .len();
    }
    if handle
        .pool()
        .connect_options()
        .get_filename()
        .file_name()
        .is_some_and(|name| name == "system.db")
    {
        foreign_issues +=
            super::databases::cross_domain_issues(&mut connection).await?;
    }
    Ok(IntegrityReport {
        integrity_ok: issues == 0,
        integrity_issue_count: issues,
        foreign_key_issue_count: foreign_issues,
    })
}

pub(crate) fn database_path(handle: &Handle) -> std::path::PathBuf {
    handle.pool().connect_options().get_filename().to_path_buf()
}

pub(crate) async fn launch_override_documents(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT json(overrides) FROM instance_launch_overrides",
    )
    .fetch_all(pool)
    .await?)
}
pub(crate) async fn legacy_launch_arguments(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<String>> {
    Ok(
        sqlx::query_scalar("SELECT json(extra_launch_args) FROM settings")
            .fetch_all(pool)
            .await?,
    )
}
pub(crate) async fn content_versions(
    pool: &sqlx::SqlitePool,
    instance: &str,
) -> crate::Result<Vec<(String, String, Option<String>)>> {
    Ok(sqlx::query_as("SELECT game_version,loader,loader_version FROM instance_content_sets WHERE instance_id=?").bind(instance).fetch_all(pool).await?)
}
