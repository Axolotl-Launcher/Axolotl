use super::{business_error, page, serialize};
use launcher_tools::{Command, Loader, ToolError};
use serde_json::{Value, json};
use theseus::data::{EditInstance, InstanceMetadata, ModLoader};

pub(super) fn summary(metadata: InstanceMetadata) -> Value {
    let instance = metadata.instance;
    let content = metadata.applied_content_set;
    json!({
        "instance_id":instance.id,
        "name":instance.name,
        "path":instance.path,
        "game_dir_override":instance.game_dir_override,
        "install_stage":instance.install_stage,
        "game_version":content.game_version,
        "loader":content.loader,
        "loader_version":content.loader_version,
        "groups":metadata.groups,
        "pinned":instance.pinned_at.is_some(),
        "created":instance.created,
        "modified":instance.modified,
        "last_played":instance.last_played,
        "played_seconds":instance.submitted_time_played.saturating_add(instance.recent_time_played),
        "linked_launcher":instance.linked_launcher,
    })
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    use theseus::instance;
    match command {
        Command::InstanceList {
            offset,
            limit,
            search,
        } => {
            let mut instances =
                instance::list().await.map_err(business_error)?;
            if let Some(search) = search {
                let search = search.to_lowercase();
                instances.retain(|item| {
                    item.instance.name.to_lowercase().contains(&search)
                });
            }
            instances.sort_by(|a, b| a.instance.id.cmp(&b.instance.id));
            page(instances.into_iter().map(summary).collect(), offset, limit)
        }
        Command::InstanceGet { instance_id } => {
            let instance = instance::get(&instance_id)
                .await
                .map_err(business_error)?
                .ok_or_else(|| {
                    ToolError::new("not_found", "Instance not found")
                })?;
            Ok(summary(instance))
        }
        Command::InstanceCreate {
            name,
            game_version,
            loader,
            loader_version,
        } => {
            let loader = match loader {
                Loader::Vanilla => ModLoader::Vanilla,
                Loader::Fabric => ModLoader::Fabric,
                Loader::Forge => ModLoader::Forge,
                Loader::Quilt => ModLoader::Quilt,
                Loader::NeoForge => ModLoader::NeoForge,
            };
            let job = theseus::install::create_instance(
                name.trim().into(),
                game_version,
                loader,
                loader_version,
                None,
                theseus::data::InstanceLink::Unmanaged,
            )
            .await
            .map_err(business_error)?;
            Ok(super::jobs::summary(job))
        }
        Command::InstanceRename { instance_id, name } => {
            let instance = instance::edit(
                &instance_id,
                EditInstance {
                    name: Some(name.trim().into()),
                    ..Default::default()
                },
            )
            .await
            .map_err(business_error)?;
            Ok(summary(instance))
        }
        Command::InstanceSetPinned {
            instance_id,
            pinned,
        } => Ok(summary(
            instance::set_pinned(&instance_id, pinned)
                .await
                .map_err(business_error)?,
        )),
        Command::InstanceGroupList {} => {
            serialize(instance::list_groups().await.map_err(business_error)?)
        }
        Command::InstanceGroupCreate { name } => serialize(
            instance::create_group(name).await.map_err(business_error)?,
        ),
        Command::InstanceGroupRename { group_id, name } => serialize(
            instance::rename_group(group_id, name)
                .await
                .map_err(business_error)?,
        ),
        Command::InstanceGroupDelete { group_id } => {
            instance::delete_group(group_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"deleted":true}))
        }
        Command::InstanceSetGroups {
            instance_id,
            group_ids,
        } => {
            // The shared grouping API also accepts externally indexed instances;
            // MCP addresses only instances returned by instance_list/get.
            if instance::get(&instance_id)
                .await
                .map_err(business_error)?
                .is_none()
            {
                return Err(ToolError::new("not_found", "Instance not found"));
            }
            instance::set_group_memberships(vec![
                instance::InstanceGroupMembershipUpdate {
                    instance_id: instance_id.clone(),
                    group_ids,
                },
            ])
            .await
            .map_err(business_error)?;
            let instance = instance::get(&instance_id)
                .await
                .map_err(business_error)?
                .ok_or_else(|| {
                    ToolError::new(
                        "not_found",
                        "Instance not found after updating groups",
                    )
                })?;
            Ok(summary(instance))
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Command is not an instance operation",
        )),
    }
}
