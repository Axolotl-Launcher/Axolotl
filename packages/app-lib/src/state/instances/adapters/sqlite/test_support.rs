use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

/// An in-memory database with the two tables every instance adapter test needs.
pub(crate) async fn test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite pool");
    sqlx::query("CREATE TABLE instances (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .expect("instances table");
    sqlx::query("CREATE TABLE instance_content_sets (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .expect("instance_content_sets table");
    pool
}
