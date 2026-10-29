use crate::state::instances::adapters::sqlite::attached_world_data as rows;
use crate::worlds::{DisplayStatus, WorldType};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct AttachedWorldData {
    pub display_status: DisplayStatus,
    pub project_id: Option<String>,
    pub content_kind: Option<String>,
}

impl AttachedWorldData {
    pub async fn get_for_world(
        instance_id: &str,
        world_type: WorldType,
        world_id: &str,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<Option<Self>> {
        let row = rows::get_for_world(
            instance_id,
            world_type.as_str(),
            world_id,
            exec,
        )
        .await?;

        Ok(row.map(|row| Self {
            display_status: DisplayStatus::from_string(&row.display_status),
            project_id: row.project_id,
            content_kind: row.content_kind,
        }))
    }

    pub async fn get_all_for_instance(
        instance_id: &str,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<HashMap<(WorldType, String), Self>> {
        let rows = rows::get_all_for_instance(instance_id, exec).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    (WorldType::from_string(&row.world_type), row.world_id),
                    Self {
                        display_status: DisplayStatus::from_string(
                            &row.display_status,
                        ),
                        project_id: row.project_id,
                        content_kind: row.content_kind,
                    },
                )
            })
            .collect())
    }

    pub async fn remove_for_world(
        instance_id: &str,
        world_type: WorldType,
        world_id: &str,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        rows::remove_for_world(instance_id, world_type.as_str(), world_id, exec)
            .await
    }
}

pub async fn set_display_status(
    instance_id: &str,
    world_type: WorldType,
    world_id: &str,
    display_status: DisplayStatus,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    rows::set_display_status(
        instance_id,
        world_type.as_str(),
        world_id,
        display_status.as_str(),
        exec,
    )
    .await
}

pub async fn set_project_id(
    instance_id: &str,
    world_type: WorldType,
    world_id: &str,
    project_id: &str,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    rows::set_project_id(
        instance_id,
        world_type.as_str(),
        world_id,
        project_id,
        exec,
    )
    .await
}

pub async fn set_content_kind(
    instance_id: &str,
    world_type: WorldType,
    world_id: &str,
    content_kind: &str,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    rows::set_content_kind(
        instance_id,
        world_type.as_str(),
        world_id,
        content_kind,
        exec,
    )
    .await
}
