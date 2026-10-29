//! What the content store's instance rows promise: which object a path is made
//! of now, and which paths an instance currently holds.

use super::*;

async fn pool_with_tables() -> sqlx::SqlitePool {
    let pool =
        crate::state::instances::adapters::sqlite::test_support::test_pool()
            .await;
    sqlx::raw_sql(
        "
        CREATE TABLE store_instance_files (
            instance_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            digest TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            PRIMARY KEY (instance_id, relative_path)
        );
        CREATE TABLE instance_files (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            relative_path TEXT NOT NULL
        );
        CREATE TABLE instance_content_entries (
            id TEXT PRIMARY KEY,
            file_id TEXT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("content store tables");
    pool
}

async fn row(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
    relative_path: &str,
) -> Option<(String, i64)> {
    sqlx::query_as(
        "SELECT digest, created_at FROM store_instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_object_a_path_is_made_of_reads_back() {
    let pool = pool_with_tables().await;

    let mut tx = pool.begin().await.unwrap();
    record_instance_file(&mut tx, "instance", "mods/a.jar", "sha512:a", 10, 10)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        get_instance_file_digest("instance", "mods/a.jar", &pool)
            .await
            .unwrap(),
        Some("sha512:a".to_string())
    );
    assert_eq!(
        get_instance_file_digest("instance", "mods/b.jar", &pool)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn recording_a_path_again_replaces_the_object_but_keeps_its_first_stamp()
{
    let pool = pool_with_tables().await;

    let mut tx = pool.begin().await.unwrap();
    record_instance_file(&mut tx, "instance", "mods/a.jar", "sha512:a", 10, 10)
        .await
        .unwrap();
    record_instance_file(&mut tx, "instance", "mods/a.jar", "sha512:b", 20, 20)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        row(&pool, "instance", "mods/a.jar").await,
        Some(("sha512:b".to_string(), 10))
    );
}

#[tokio::test]
async fn the_paths_an_instance_holds_come_from_its_entries_and_its_store_rows()
{
    let pool = pool_with_tables().await;
    sqlx::query(
        "INSERT INTO instance_files (id, instance_id, relative_path)
         VALUES ('file', 'instance', 'mods/a.jar')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO instance_content_entries (id, file_id) VALUES ('entry', 'file')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO store_instance_files
            (instance_id, relative_path, digest, created_at, modified_at)
         VALUES ('instance', 'config/x.txt', 'sha512:x', 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut paths = get_current_paths("instance", &pool).await.unwrap();
    paths.sort();

    assert_eq!(paths, ["config/x.txt", "mods/a.jar"]);
}
