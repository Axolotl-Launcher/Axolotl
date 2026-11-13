use crate::event::emit::emit_instance_groups_changed;
use crate::state::instance_groups;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const MAX_GROUP_NAME_LENGTH: usize = 256;
pub const FAVORITES_GROUP_ID: &str = instance_groups::FAVORITES_GROUP_ID;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceGroupMembershipUpdate {
    pub instance_id: String,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceGroupMembershipChange {
    pub instance_id: String,
    pub add_group_ids: Vec<String>,
    pub remove_group_ids: Vec<String>,
}

fn validate_group_name(name: &str) -> crate::Result<&str> {
    let name = name.trim();

    if name.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Group name cannot be empty".to_string(),
        )
        .into());
    }

    if name.chars().count() > MAX_GROUP_NAME_LENGTH {
        return Err(crate::ErrorKind::InputError(format!(
            "Group name cannot exceed {MAX_GROUP_NAME_LENGTH} characters"
        ))
        .into());
    }

    if name.eq_ignore_ascii_case("none") {
        return Err(crate::ErrorKind::InputError(
            "Group name cannot be None".to_string(),
        )
        .into());
    }

    Ok(name)
}

fn normalize_membership_updates(
    mut updates: Vec<InstanceGroupMembershipUpdate>,
) -> crate::Result<Vec<InstanceGroupMembershipUpdate>> {
    let mut instance_ids = HashSet::new();
    for update in &mut updates {
        if !instance_ids.insert(update.instance_id.clone()) {
            return Err(crate::ErrorKind::InputError(format!(
                "Duplicate instance {} in group membership update",
                update.instance_id
            ))
            .into());
        }

        let mut group_ids = HashSet::new();
        update
            .group_ids
            .retain(|group_id| group_ids.insert(group_id.clone()));
    }

    Ok(updates)
}

pub async fn list_groups() -> crate::Result<Vec<InstanceGroup>> {
    Ok(instance_groups::list()
        .into_iter()
        .map(|group| InstanceGroup {
            id: group.id,
            name: group.name,
        })
        .collect())
}

pub async fn create_group(name: String) -> crate::Result<InstanceGroup> {
    let name = validate_group_name(&name)?;
    let id = Uuid::new_v4().to_string();
    instance_groups::create(&id, name);
    // Creating a group changes the library even when it has no members yet;
    // without this event the frontend keeps its stale list until the next
    // membership change and empty groups never appear.
    emit_instance_groups_changed(&[]).await?;

    Ok(InstanceGroup {
        id,
        name: name.to_string(),
    })
}

pub async fn set_group_order(group_ids: Vec<String>) -> crate::Result<()> {
    instance_groups::set_order(&group_ids);
    Ok(())
}

pub async fn set_group_memberships(
    updates: Vec<InstanceGroupMembershipUpdate>,
) -> crate::Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let updates = normalize_membership_updates(updates)?;

    // Note: the instances themselves are deliberately not validated here.
    // JSON-backed instances (`local:` ids) have no row in the `instances` table,
    // and requiring one is what used to make grouping them fail outright.
    let unique_group_ids = updates
        .iter()
        .flat_map(|update| update.group_ids.iter())
        .collect::<HashSet<_>>();
    for group_id in unique_group_ids {
        if !instance_groups::group_exists(group_id) {
            return Err(crate::ErrorKind::InputError(format!(
                "Unknown instance group {group_id}"
            ))
            .into());
        }
    }

    instance_groups::set_memberships(
        &updates
            .iter()
            .map(|update| {
                (update.instance_id.clone(), update.group_ids.clone())
            })
            .collect::<Vec<_>>(),
    );

    let instance_ids = updates
        .into_iter()
        .map(|update| update.instance_id)
        .collect::<Vec<_>>();
    emit_instance_groups_changed(&instance_ids).await?;

    Ok(())
}

pub async fn update_group_memberships(
    updates: Vec<InstanceGroupMembershipChange>,
) -> crate::Result<()> {
    if updates.is_empty() {
        return Ok(());
    }
    instance_groups::update_memberships(
        &updates
            .iter()
            .map(|update| {
                (
                    update.instance_id.clone(),
                    update.add_group_ids.clone(),
                    update.remove_group_ids.clone(),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    let instance_ids = updates
        .into_iter()
        .map(|update| update.instance_id)
        .collect::<Vec<_>>();
    emit_instance_groups_changed(&instance_ids).await?;
    Ok(())
}

pub async fn rename_group(
    id: String,
    new_name: String,
) -> crate::Result<InstanceGroup> {
    if id == FAVORITES_GROUP_ID {
        return Err(crate::ErrorKind::InputError(
            "Favorites cannot be renamed".to_string(),
        )
        .into());
    }

    let new_name = validate_group_name(&new_name)?;

    let Some(instance_ids) = instance_groups::rename(&id, new_name) else {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown instance group {id}"
        ))
        .into());
    };

    emit_instance_groups_changed(&instance_ids).await?;

    Ok(InstanceGroup {
        id,
        name: new_name.to_string(),
    })
}

pub async fn delete_group(id: String) -> crate::Result<()> {
    if id == FAVORITES_GROUP_ID {
        return Err(crate::ErrorKind::InputError(
            "Favorites cannot be deleted".to_string(),
        )
        .into());
    }

    let Some(instance_ids) = instance_groups::delete(&id) else {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown instance group {id}"
        ))
        .into());
    };

    emit_instance_groups_changed(&instance_ids).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        FAVORITES_GROUP_ID, InstanceGroupMembershipUpdate,
        MAX_GROUP_NAME_LENGTH, normalize_membership_updates,
        validate_group_name,
    };

    #[test]
    fn group_name_validation_trims_valid_names() {
        assert_eq!(validate_group_name("  My group  ").unwrap(), "My group");
    }

    #[test]
    fn group_name_validation_rejects_empty_names() {
        assert!(validate_group_name("   ").is_err());
    }

    #[test]
    fn group_name_validation_rejects_reserved_name() {
        assert!(validate_group_name("NoNe").is_err());
    }

    #[test]
    fn group_name_validation_allows_favorites() {
        assert_eq!(validate_group_name("Favorites").unwrap(), "Favorites");
    }

    #[test]
    fn group_name_validation_rejects_long_names() {
        assert!(
            validate_group_name(&"a".repeat(MAX_GROUP_NAME_LENGTH + 1))
                .is_err()
        );
    }

    #[test]
    fn favorites_group_id_is_stable() {
        assert_eq!(FAVORITES_GROUP_ID, "group:favorites");
    }

    #[test]
    fn membership_updates_reject_duplicate_instances() {
        let updates = vec![
            InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec!["first".to_string()],
            },
            InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec!["second".to_string()],
            },
        ];

        assert!(normalize_membership_updates(updates).is_err());
    }

    #[test]
    fn membership_updates_deduplicate_groups() {
        let updates =
            normalize_membership_updates(vec![InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec![
                    "a".to_string(),
                    "b".to_string(),
                    "a".to_string(),
                    "b".to_string(),
                ],
            }])
            .unwrap();

        assert_eq!(updates[0].group_ids, &["a", "b"]);
    }
}
