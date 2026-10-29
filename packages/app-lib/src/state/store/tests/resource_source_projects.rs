//! What the resource source projects collection promises, and what the
//! favorites read makes of it.

use super::support;
use crate::state::content_types::{
    ContentFavoriteProvider, ContentFavoriteType,
};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::content_favorites::{
    ContentFavoriteKey, ContentFavoriteRecord, ContentFavorites,
};
use crate::state::store::resource_source_projects::{
    ResourceSourceProjectKey, ResourceSourceProjectRecord,
    ResourceSourceProjects,
};

/// A database whose only provider project already maps to a resource.
async fn seeded() -> Handle {
    let pool = support::pool().await;
    support::add_resource(&pool, "resource").await;
    sqlx::query(
        "INSERT INTO resource_source_projects (source, project_id, resource_id)
         VALUES ('modrinth', 'sodium', 'resource')",
    )
    .execute(&pool)
    .await
    .unwrap();
    Handle::new(pool)
}

#[tokio::test]
async fn a_mapping_round_trips_and_removes() {
    let handle = support::handle().await;
    support::add_resource(handle.pool(), "resource").await;
    let project = ResourceSourceProjectKey {
        source: "modrinth".to_string(),
        project_id: "sodium".to_string(),
    };
    let record = ResourceSourceProjectRecord {
        resource_id: "resource".to_string(),
    };

    ResourceSourceProjects::set(&handle, &project, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        ResourceSourceProjects::get(&handle, &project)
            .await
            .unwrap(),
        Some(record)
    );

    ResourceSourceProjects::set(&handle, &project, None)
        .await
        .unwrap();
    assert_eq!(
        ResourceSourceProjects::get(&handle, &project)
            .await
            .unwrap(),
        None
    );
    assert!(
        ResourceSourceProjects::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_favorite_carries_the_resource_its_project_maps_to() {
    let handle = seeded().await;

    let favorite = ContentFavoriteKey {
        provider: ContentFavoriteProvider::Modrinth,
        project_id: "sodium".to_string(),
    };
    ContentFavorites::set(
        &handle,
        &favorite,
        Some(&ContentFavoriteRecord {
            content_type: ContentFavoriteType::Mod,
            saved_at: 1,
        }),
    )
    .await
    .expect("save favorite");

    let favorites = ContentFavorites::get_all(&handle)
        .await
        .expect("read favorites");
    assert_eq!(favorites.len(), 1);
    let (favorite, _) = &favorites[0];
    let project = ResourceSourceProjects::get(
        &handle,
        &ResourceSourceProjectKey {
            source: favorite.provider.as_str().to_string(),
            project_id: favorite.project_id.clone(),
        },
    )
    .await
    .expect("read the project the favorite maps to");
    assert_eq!(
        project.map(|project| project.resource_id),
        Some("resource".to_string())
    );
}
