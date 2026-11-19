use sqlx::SqlitePool;

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct InstanceScreenshotSource {
    pub id: String,
    pub name: String,
    pub path: String,
}

pub(crate) async fn get_instance_screenshot_source(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceScreenshotSource>> {
    Ok(sqlx::query_as::<_, InstanceScreenshotSource>(
        "SELECT id, name, path FROM instances WHERE id = ?",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?)
}

pub(crate) async fn list_screenshot_sources(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceScreenshotSource>> {
    Ok(sqlx::query_as::<_, InstanceScreenshotSource>(
        "SELECT id, name, path FROM instances ORDER BY name, id",
    )
    .fetch_all(pool)
    .await?)
}
