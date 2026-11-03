//! What the resource source projects collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::resource_source_projects::{
    ResourceSourceProjectKey, ResourceSourceProjectRecord,
    ResourceSourceProjects,
};

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
