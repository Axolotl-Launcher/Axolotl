//! What the cache collection promises.

use super::support;
use crate::state::store::cache::{Cache, CacheKey, CacheRecord};
use crate::state::store::{Collection as _, Handle};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn key(data_type: &str, id: &str) -> CacheKey {
    CacheKey {
        data_type: data_type.to_string(),
        id: id.to_string(),
    }
}

fn record(
    alias: Option<&str>,
    data: Option<&str>,
    expires: i64,
) -> CacheRecord {
    CacheRecord {
        alias: alias.map(str::to_string),
        data: data.map(str::to_string),
        expires,
    }
}

async fn stored_ids(pool: &sqlx::SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM cache ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_row_round_trips_under_its_key() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let project = key("project", "project-id");
    let row = record(Some("slug"), Some("{\"id\":\"project-id\"}"), 42);

    Cache::set(&handle, &project, Some(&row)).await.unwrap();

    assert_eq!(Cache::get(&handle, &project).await.unwrap(), Some(row));
}

#[tokio::test]
async fn a_key_of_one_type_is_not_a_key_of_another() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    let report = record(None, None, 1);
    let loaders = record(Some("fabric"), Some("{\"id\":\"loaders\"}"), 2);
    Cache::set(&handle, &key("report_types", "0"), Some(&report))
        .await
        .unwrap();
    Cache::set(&handle, &key("loaders", "0"), Some(&loaders))
        .await
        .unwrap();

    assert_eq!(
        Cache::get(&handle, &key("report_types", "0"))
            .await
            .unwrap(),
        Some(report)
    );
    assert_eq!(
        Cache::get(&handle, &key("loaders", "0")).await.unwrap(),
        Some(loaders)
    );
    assert_eq!(
        Cache::get(&handle, &key("loaders", "1")).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn setting_no_record_removes_the_row() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let project = key("project", "project-id");

    Cache::set(&handle, &project, Some(&record(None, None, 1)))
        .await
        .unwrap();
    Cache::set(&handle, &project, None).await.unwrap();

    assert_eq!(Cache::get(&handle, &project).await.unwrap(), None);
}

#[tokio::test]
async fn writing_a_row_takes_its_alias_from_the_row_that_held_it() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    Cache::set(
        &handle,
        &key("curseforge_project", "42"),
        Some(&record(Some("atm10sky"), None, 1)),
    )
    .await
    .unwrap();
    Cache::set(
        &handle,
        &key("curseforge_project", "84"),
        Some(&record(Some("atm10sky"), None, 2)),
    )
    .await
    .unwrap();

    assert_eq!(stored_ids(&pool).await, vec!["84"]);
}

#[tokio::test]
async fn an_alias_is_held_by_one_row_of_each_type() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    for data_type in ["report_types", "loaders"] {
        Cache::set(
            &handle,
            &key(data_type, data_type),
            Some(&record(Some("shared-alias"), None, 1)),
        )
        .await
        .unwrap();
    }

    assert_eq!(stored_ids(&pool).await, vec!["loaders", "report_types"]);
}

#[tokio::test]
async fn rows_without_an_alias_do_not_replace_each_other() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);

    for id in ["id-a", "id-b"] {
        Cache::set(
            &handle,
            &key("report_types", id),
            Some(&record(None, None, 1)),
        )
        .await
        .unwrap();
    }

    assert_eq!(stored_ids(&pool).await, vec!["id-a", "id-b"]);
}

#[tokio::test]
async fn a_payload_above_the_bound_is_not_read() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    let payload = format!("{{\"pad\":\"{}\"}}", "x".repeat(1024 * 1024));
    sqlx::query(
        "
        INSERT INTO cache (id, data_type, alias, data, expires)
        VALUES ('big', 'modpack_files', NULL, ?, 1)
        ",
    )
    .bind(&payload)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        Cache::get(&handle, &key("modpack_files", "big"))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_scope_reads_the_rows_of_its_type() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    for (data_type, id) in
        [("project", "a"), ("project", "b"), ("version", "c")]
    {
        Cache::set(&handle, &key(data_type, id), Some(&record(None, None, 1)))
            .await
            .unwrap();
    }

    let scoped = Cache::get_scoped(&handle, &"project".to_string())
        .await
        .unwrap();

    assert_eq!(scoped.len(), 2);
    assert!(scoped.iter().all(|(key, _)| key.data_type == "project"));
}

#[tokio::test]
async fn a_scoped_replacement_keeps_the_rows_it_does_not_name() {
    let pool = support::pool().await;
    let handle = handle_of(&pool);
    for (data_type, id) in [("project", "a"), ("version", "a")] {
        Cache::set(&handle, &key(data_type, id), Some(&record(None, None, 1)))
            .await
            .unwrap();
    }

    Cache::set_scoped(
        &handle,
        &"project".to_string(),
        &[(key("project", "b"), record(None, None, 2))],
    )
    .await
    .unwrap();

    assert_eq!(
        Cache::get(&handle, &key("project", "a")).await.unwrap(),
        None
    );
    assert!(
        Cache::get(&handle, &key("project", "b"))
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        Cache::get(&handle, &key("version", "a"))
            .await
            .unwrap()
            .is_some()
    );
}
