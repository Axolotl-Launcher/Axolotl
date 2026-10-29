//! What the instance file steps promise: which files survive a rollback, and
//! which stored metadata answers for the bytes a file carries.

use super::*;

async fn pool_with_files(
    rows: &[(&str, &str, &str, i64, Option<&str>)],
) -> sqlx::SqlitePool {
    let pool =
        crate::state::instances::adapters::sqlite::test_support::test_pool()
            .await;
    sqlx::raw_sql(
        "
        CREATE TABLE instance_files (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            sha1 TEXT NOT NULL,
            size INTEGER NOT NULL,
            missing INTEGER NOT NULL DEFAULT 0,
            added_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            local_mod_data TEXT NULL,
            icon_path TEXT NULL
        );
        CREATE TABLE instance_content_entries (
            id TEXT PRIMARY KEY,
            file_id TEXT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("file tables");

    for (id, relative_path, sha1, size, local_mod_data) in rows {
        sqlx::query(
            "INSERT INTO instance_files
                (id, instance_id, relative_path, file_name, enabled, sha1, size,
                 missing, added_at, modified_at, local_mod_data)
             VALUES (?, 'instance', ?, 'file.jar', 1, ?, ?, 0, 1, 1, ?)",
        )
        .bind(*id)
        .bind(*relative_path)
        .bind(*sha1)
        .bind(*size)
        .bind(*local_mod_data)
        .execute(&pool)
        .await
        .unwrap();
    }

    pool
}

async fn remaining_files(pool: &sqlx::SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM instance_files ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_file_an_entry_names_survives_the_rollback_and_an_entryless_one_does_not()
 {
    let pool = pool_with_files(&[
        ("named", "mods/named.jar", "sha1:a", 10, None),
        ("entryless", "mods/entryless.jar", "sha1:b", 20, None),
    ])
    .await;
    sqlx::query(
        "INSERT INTO instance_content_entries (id, file_id) VALUES ('entry', 'named')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    delete_unreferenced_file_by_id_in_transaction("instance", "named", &mut tx)
        .await
        .unwrap();
    delete_unreferenced_file_by_id_in_transaction(
        "instance",
        "entryless",
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(remaining_files(&pool).await, ["named"]);
}

#[tokio::test]
async fn the_metadata_of_files_with_the_same_bytes_reads_back() {
    let pool = pool_with_files(&[
        (
            "same",
            "mods/same.jar",
            "sha1:a",
            10,
            Some("{\"name\":\"same\"}"),
        ),
        (
            "other",
            "mods/other.jar",
            "sha1:a",
            20,
            Some("{\"name\":\"other\"}"),
        ),
        (
            "gone",
            "mods/gone.jar",
            "sha1:a",
            10,
            Some("{\"name\":\"gone\"}"),
        ),
    ])
    .await;
    sqlx::query("UPDATE instance_files SET missing = 1 WHERE id = 'gone'")
        .execute(&pool)
        .await
        .unwrap();

    let cached = get_local_mod_data_for_artifact("sha1:a", 10, &pool)
        .await
        .unwrap();

    assert_eq!(cached, ["{\"name\":\"same\"}"]);
}

#[tokio::test]
async fn the_metadata_recorded_for_a_file_reads_back() {
    let pool =
        pool_with_files(&[("file", "mods/file.jar", "sha1:a", 10, None)]).await;

    let mut tx = pool.begin().await.unwrap();
    set_local_mod_data_in_transaction(
        "file",
        Some("{\"name\":\"recorded\"}"),
        &mut tx,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let cached = get_local_mod_data_for_artifact("sha1:a", 10, &pool)
        .await
        .unwrap();

    assert_eq!(cached, ["{\"name\":\"recorded\"}"]);
}
