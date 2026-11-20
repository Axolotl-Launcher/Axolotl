use crate::state::persistence::Persistence;
use crate::state::user_state::worlds;
use crate::worlds::{DisplayStatus, WorldType};
use serde_json::json;
use std::collections::HashMap;
#[derive(Debug, Clone, Default)]
pub struct AttachedWorldData {
    pub display_status: DisplayStatus,
    pub project_id: Option<String>,
    pub content_kind: Option<String>,
}
impl AttachedWorldData {
    pub(crate) async fn get_for_world(
        instance: &str,
        kind: WorldType,
        world: &str,
        persistence: &Persistence,
    ) -> crate::Result<Option<Self>> {
        Ok(worlds::get_all(persistence, instance)
            .await?
            .remove(&(kind, world.to_owned())))
    }
    pub(crate) async fn get_all_for_instance(
        instance: &str,
        persistence: &Persistence,
    ) -> crate::Result<HashMap<(WorldType, String), Self>> {
        worlds::get_all(persistence, instance).await
    }
    pub(crate) async fn remove_for_world(
        instance: &str,
        kind: WorldType,
        world: &str,
        persistence: &Persistence,
    ) -> crate::Result<()> {
        worlds::remove(persistence, instance, kind, world).await
    }
}
pub(crate) async fn set_display_status(
    instance: &str,
    kind: WorldType,
    world: &str,
    status: DisplayStatus,
    persistence: &Persistence,
) -> crate::Result<()> {
    worlds::set(
        persistence,
        instance,
        kind,
        world,
        "display_status",
        json!(status.as_str()),
    )
    .await
}
pub(crate) async fn set_project_id(
    instance: &str,
    kind: WorldType,
    world: &str,
    project: &str,
    persistence: &Persistence,
) -> crate::Result<()> {
    worlds::set(
        persistence,
        instance,
        kind,
        world,
        "project_id",
        json!(project),
    )
    .await
}
pub(crate) async fn set_content_kind(
    instance: &str,
    kind: WorldType,
    world: &str,
    content: &str,
    persistence: &Persistence,
) -> crate::Result<()> {
    worlds::set(
        persistence,
        instance,
        kind,
        world,
        "content_kind",
        json!(content),
    )
    .await
}
