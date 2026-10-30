//! What the saved favorites promise: one row per provider project, a save
//! that keeps the time the favorite was first saved, and a removal that
//! leaves the other projects of the same resource alone.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::resource_favorites::{
    ResourceFavoriteKey, ResourceFavoriteRecord, ResourceFavorites,
};

fn key(source: &str, project_id: &str) -> ResourceFavoriteKey {
    ResourceFavoriteKey {
        source: source.to_string(),
        project_id: project_id.to_string(),
    }
}

fn record(content_type: &str, saved_at: i64) -> ResourceFavoriteRecord {
    ResourceFavoriteRecord {
        content_type: content_type.to_string(),
        saved_at,
    }
}

/// One provider project mapped to the resource the test added.
async fn project(
    pool: &sqlx::SqlitePool,
    source: &str,
    project_id: &str,
    resource_id: &str,
) {
    sqlx::query(
        "INSERT INTO resource_source_projects (source, project_id, resource_id)
         VALUES (?, ?, ?)",
    )
    .bind(source)
    .bind(project_id)
    .bind(resource_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn a_favorite_is_read_back_by_its_project() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    project(&pool, "modrinth", "sodium", "resource").await;
    let handle = Handle::new(pool);

    ResourceFavorites::set(
        &handle,
        &key("modrinth", "sodium"),
        Some(&record("mod", 100)),
    )
    .await
    .unwrap();

    assert_eq!(
        ResourceFavorites::get(&handle, &key("modrinth", "sodium"))
            .await
            .unwrap(),
        Some(record("mod", 100))
    );
    assert_eq!(
        ResourceFavorites::get_all(&handle).await.unwrap(),
        vec![(key("modrinth", "sodium"), record("mod", 100))]
    );
}

#[tokio::test]
async fn saving_a_favorite_again_keeps_the_time_it_was_first_saved() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    project(&pool, "modrinth", "sodium", "resource").await;
    let handle = Handle::new(pool);

    ResourceFavorites::save(&handle, &key("modrinth", "sodium"), "mod", 100)
        .await
        .unwrap();
    ResourceFavorites::save(
        &handle,
        &key("modrinth", "sodium"),
        "resourcepack",
        200,
    )
    .await
    .unwrap();

    assert_eq!(
        ResourceFavorites::get(&handle, &key("modrinth", "sodium"))
            .await
            .unwrap(),
        Some(record("resourcepack", 100))
    );
}

#[tokio::test]
async fn removing_a_favorite_leaves_the_projects_of_the_same_resource() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    project(&pool, "modrinth", "sodium", "resource").await;
    project(&pool, "curseforge", "999", "resource").await;
    let handle = Handle::new(pool);
    ResourceFavorites::set(
        &handle,
        &key("modrinth", "sodium"),
        Some(&record("mod", 100)),
    )
    .await
    .unwrap();
    ResourceFavorites::set(
        &handle,
        &key("curseforge", "999"),
        Some(&record("mod", 200)),
    )
    .await
    .unwrap();

    ResourceFavorites::set(&handle, &key("modrinth", "sodium"), None)
        .await
        .unwrap();

    assert_eq!(
        ResourceFavorites::get_all(&handle).await.unwrap(),
        vec![(key("curseforge", "999"), record("mod", 200))]
    );
}
