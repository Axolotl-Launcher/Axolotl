//! What the favorites the resource layer saves promise: one row per saved
//! favorite, and a removal that touches only the project it names.

use super::support;
use crate::resources::favorites::{self, SavedFavorite};
use crate::resources::{
    ResourceId, ResourceType, SourceId, SourceProjectId, SourceProjectRef,
};
use sqlx::SqlitePool;

fn project(source: &str, project_id: &str) -> SourceProjectRef {
    SourceProjectRef {
        source: SourceId::new(source).unwrap(),
        project_id: SourceProjectId::new(project_id).unwrap(),
    }
}

fn resource(id: &str) -> Option<ResourceId> {
    Some(ResourceId::new(id).unwrap())
}

/// The resource `source`'s `project_id` maps to.
async fn link(
    pool: &SqlitePool,
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

/// A favorite as the resource layer saves it.
async fn save(
    pool: &SqlitePool,
    source: &str,
    project_id: &str,
    content_type: &str,
    saved_at: i64,
) {
    sqlx::query(
        "INSERT INTO resource_favorites (source, project_id, content_type, saved_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(source)
    .bind(project_id)
    .bind(content_type)
    .bind(saved_at)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn removing_one_favorite_leaves_the_projects_of_the_same_resource() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    link(&pool, "modrinth", "sodium", "resource").await;
    link(&pool, "curseforge", "sodium", "resource").await;
    save(&pool, "modrinth", "sodium", "mod", 100).await;
    save(&pool, "curseforge", "sodium", "mod", 200).await;

    favorites::remove_project(&pool, &project("modrinth", "sodium"))
        .await
        .expect("remove one favorite");

    assert_eq!(
        favorites::saved(&pool).await.expect("read favorites"),
        vec![SavedFavorite {
            project: project("curseforge", "sodium"),
            content_type: ResourceType::Mod,
            saved_at: 200,
            resource_id: resource("resource"),
        }]
    );
}

#[tokio::test]
async fn every_saved_favorite_is_a_row_with_the_time_it_was_saved() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    link(&pool, "modrinth", "sodium", "resource").await;
    link(&pool, "curseforge", "sodium", "resource").await;
    save(&pool, "modrinth", "sodium", "mod", 100).await;
    save(&pool, "curseforge", "sodium", "mod", 200).await;

    assert_eq!(
        favorites::saved(&pool).await.expect("read favorites"),
        vec![
            SavedFavorite {
                project: project("curseforge", "sodium"),
                content_type: ResourceType::Mod,
                saved_at: 200,
                resource_id: resource("resource"),
            },
            SavedFavorite {
                project: project("modrinth", "sodium"),
                content_type: ResourceType::Mod,
                saved_at: 100,
                resource_id: resource("resource"),
            },
        ]
    );
}

#[tokio::test]
async fn a_resource_saved_as_two_content_types_keeps_both_rows() {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    link(&pool, "modrinth", "sodium", "resource").await;
    link(&pool, "curseforge", "sodium", "resource").await;
    save(&pool, "modrinth", "sodium", "mod", 100).await;
    save(&pool, "curseforge", "sodium", "shader", 200).await;

    assert_eq!(
        favorites::saved(&pool).await.expect("read favorites"),
        vec![
            SavedFavorite {
                project: project("curseforge", "sodium"),
                content_type: ResourceType::Shader,
                saved_at: 200,
                resource_id: resource("resource"),
            },
            SavedFavorite {
                project: project("modrinth", "sodium"),
                content_type: ResourceType::Mod,
                saved_at: 100,
                resource_id: resource("resource"),
            },
        ]
    );
}
