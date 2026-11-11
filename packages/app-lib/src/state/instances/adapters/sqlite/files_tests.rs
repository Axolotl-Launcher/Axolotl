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

/// A file-backed pool the concurrency tests can put more than one connection on.
async fn concurrent_pool(path: &std::path::Path) -> sqlx::SqlitePool {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(2));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .expect("file-backed pool");
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
            icon_path TEXT NULL,
            UNIQUE (instance_id, relative_path)
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("file tables");
    sqlx::query(
        "INSERT INTO instance_files
            (id, instance_id, relative_path, file_name, enabled, sha1, size,
             missing, added_at, modified_at)
         VALUES ('file', 'instance', 'mods/a.jar', 'a.jar', 1, 'sha1:a', 1, 0, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("seed file");
    pool
}

#[tokio::test]
async fn rename_instance_file_waits_for_a_concurrent_writer() {
    let directory = tempfile::tempdir().unwrap();
    let pool = concurrent_pool(&directory.path().join("app.db")).await;

    // Hold the write lock before the helper starts. A deferred read-then-write
    // would read this snapshot, then fail with SQLITE_BUSY_SNAPSHOT once the
    // holder commits; BEGIN IMMEDIATE waits instead.
    let holder = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();

    let rename = tokio::spawn({
        let pool = pool.clone();
        async move {
            rename_instance_file(
                "instance",
                "mods/a.jar",
                "mods/a.jar.disabled",
                "a.jar",
                false,
                &pool,
            )
            .await
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    holder.commit().await.unwrap();

    let renamed = rename
        .await
        .unwrap()
        .expect("rename succeeds")
        .expect("file renamed");
    assert_eq!(renamed.relative_path, "mods/a.jar.disabled");
    assert!(!renamed.enabled);
}

#[tokio::test]
async fn upsert_instance_file_waits_for_a_concurrent_writer() {
    let directory = tempfile::tempdir().unwrap();
    let pool = concurrent_pool(&directory.path().join("app.db")).await;

    let holder = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();

    let upsert = tokio::spawn({
        let pool = pool.clone();
        async move {
            upsert_instance_file_from_parts(
                UpsertInstanceFile {
                    instance_id: "instance",
                    relative_path: "mods/a.jar",
                    file_name: "a.jar",
                    enabled: false,
                    sha1: "sha1:b",
                    size: 2,
                    missing: false,
                    local_mod_data: None,
                    icon_path: None,
                },
                &pool,
            )
            .await
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    holder.commit().await.unwrap();

    let updated = upsert.await.unwrap().expect("upsert succeeds");
    assert_eq!(updated.sha1, "sha1:b");
    assert!(!updated.enabled);
}
