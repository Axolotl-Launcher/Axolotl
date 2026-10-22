use crate::state::instance_groups;
use crate::state::instances::{
    ContentSet, Instance, InstanceLaunchOverrides, InstanceLink,
    LoaderComponent,
    adapters::sqlite::{instance_rows, loader_component_rows},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceMetadata {
    pub instance: Instance,
    pub applied_content_set: ContentSet,
    pub link: InstanceLink,
    pub groups: Vec<String>,
    pub launch_overrides: InstanceLaunchOverrides,
    #[serde(default)]
    pub loader_components: Vec<LoaderComponent>,
    #[serde(default)]
    pub synced_options: crate::state::InstanceSyncedOptions,
}

pub(crate) async fn get_instance(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceMetadata>> {
    get_instance_metadata(instance_id, pool).await
}

pub(crate) async fn get_instance_metadata(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceMetadata>> {
    let Some(record) =
        instance_rows::get_instance_metadata_by_id(instance_id, pool).await?
    else {
        return Ok(None);
    };
    let loader_components =
        loader_component_rows::list_loader_components(instance_id, pool)
            .await?;
    let mut metadata = InstanceMetadata::from_record(record, loader_components);
    metadata.synced_options =
        instance_rows::get_instance_synced_options(&metadata.instance.id, pool)
            .await?;
    Ok(Some(metadata))
}

pub(crate) async fn get_instances_metadata(
    instance_ids: &[&str],
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadata>> {
    let records =
        instance_rows::get_instance_metadata_many(instance_ids, pool).await?;
    hydrate_instance_records(records, pool).await
}

pub(crate) async fn list_instances(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadata>> {
    let records = instance_rows::list_instance_metadata(pool).await?;
    hydrate_instance_records(records, pool).await
}

async fn hydrate_instance_records(
    records: Vec<instance_rows::InstanceMetadataRecord>,
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceMetadata>> {
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let ids = records
        .iter()
        .map(|record| record.instance.id.as_str())
        .collect::<Vec<_>>();
    let mut components =
        loader_component_rows::list_loader_components_many(&ids, pool).await?;
    let preferences =
        instance_rows::get_instance_synced_options_many(&ids, pool).await?;
    let mut metadata = Vec::with_capacity(records.len());
    for record in records {
        // Clone for repeated ids in get_many; preserve the requested order and duplicates.
        let loader_components = components
            .entry(record.instance.id.clone())
            .or_default()
            .clone();
        let options = preferences
            .get(&record.instance.id)
            .copied()
            .unwrap_or_default();
        let mut item = InstanceMetadata::from_record(record, loader_components);
        item.synced_options = options;
        metadata.push(item);
    }
    Ok(metadata)
}

impl InstanceMetadata {
    fn from_record(
        record: instance_rows::InstanceMetadataRecord,
        loader_components: Vec<LoaderComponent>,
    ) -> Self {
        let groups = instance_groups::group_ids_for(&record.instance.id);
        Self {
            instance: record.instance,
            applied_content_set: record.applied_content_set,
            link: record.link,
            groups,
            launch_overrides: record.launch_overrides,
            loader_components,
            synced_options: crate::state::InstanceSyncedOptions::default(),
        }
    }
}
