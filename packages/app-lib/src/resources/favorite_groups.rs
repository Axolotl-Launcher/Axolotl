//! User-created groups for saved resources. Membership is keyed by
//! `source:project_id`, so a favorite can be grouped without resolving the
//! resource it maps to.

use super::*;
use crate::event::emit::emit_favorite_groups_changed;
use crate::state::favorite_groups;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const MAX_FAVORITE_GROUP_NAME_LENGTH: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteGroup {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteGroupMembership {
    pub favorite_key: String,
    pub group_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteGroupsSnapshot {
    pub groups: Vec<FavoriteGroup>,
    pub memberships: Vec<FavoriteGroupMembership>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteGroupMembershipChange {
    pub favorite_key: String,
    pub add_group_ids: Vec<String>,
    pub remove_group_ids: Vec<String>,
}

fn validate_group_name(name: &str) -> ResourceResult<&str> {
    let name = name.trim();

    if name.is_empty() {
        return Err(ResourceError::InvalidRequest(
            "Group name cannot be empty".to_string(),
        ));
    }

    if name.chars().count() > MAX_FAVORITE_GROUP_NAME_LENGTH {
        return Err(ResourceError::InvalidRequest(format!(
            "Group name cannot exceed {MAX_FAVORITE_GROUP_NAME_LENGTH} characters"
        )));
    }

    if name.eq_ignore_ascii_case("none") {
        return Err(ResourceError::InvalidRequest(
            "Group name cannot be None".to_string(),
        ));
    }

    Ok(name)
}

fn normalize_membership_changes(
    mut changes: Vec<FavoriteGroupMembershipChange>,
) -> ResourceResult<Vec<FavoriteGroupMembershipChange>> {
    let mut favorite_keys = HashSet::new();
    for change in &mut changes {
        if !favorite_keys.insert(change.favorite_key.clone()) {
            return Err(ResourceError::InvalidRequest(format!(
                "Duplicate favorite {} in group membership update",
                change.favorite_key
            )));
        }

        let mut add_group_ids = HashSet::new();
        change
            .add_group_ids
            .retain(|group_id| add_group_ids.insert(group_id.clone()));

        let mut remove_group_ids = HashSet::new();
        change
            .remove_group_ids
            .retain(|group_id| remove_group_ids.insert(group_id.clone()));
    }

    Ok(changes)
}

pub fn list() -> FavoriteGroupsSnapshot {
    FavoriteGroupsSnapshot {
        groups: favorite_groups::list()
            .into_iter()
            .map(|group| FavoriteGroup {
                id: group.id,
                name: group.name,
            })
            .collect(),
        memberships: favorite_groups::memberships()
            .into_iter()
            .map(|(favorite_key, group_ids)| FavoriteGroupMembership {
                favorite_key,
                group_ids,
            })
            .collect(),
    }
}

pub async fn create(name: String) -> ResourceResult<FavoriteGroup> {
    let name = validate_group_name(&name)?;
    let id = Uuid::new_v4().to_string();
    favorite_groups::create(&id, name);
    // Creating a group changes the library even when it has no members yet;
    // without this event the frontend keeps its stale list until the next
    // membership change and empty groups never appear.
    emit_favorite_groups_changed(&[]).await?;

    Ok(FavoriteGroup {
        id,
        name: name.to_string(),
    })
}

pub async fn rename(
    id: String,
    new_name: String,
) -> ResourceResult<FavoriteGroup> {
    let new_name = validate_group_name(&new_name)?;

    let Some(favorite_keys) = favorite_groups::rename(&id, new_name) else {
        return Err(ResourceError::InvalidRequest(format!(
            "Unknown favorite group {id}"
        )));
    };

    emit_favorite_groups_changed(&favorite_keys).await?;

    Ok(FavoriteGroup {
        id,
        name: new_name.to_string(),
    })
}

pub async fn delete(id: String) -> ResourceResult<()> {
    let Some(favorite_keys) = favorite_groups::delete(&id) else {
        return Err(ResourceError::InvalidRequest(format!(
            "Unknown favorite group {id}"
        )));
    };

    emit_favorite_groups_changed(&favorite_keys).await?;

    Ok(())
}

pub async fn update_memberships(
    changes: Vec<FavoriteGroupMembershipChange>,
) -> ResourceResult<()> {
    if changes.is_empty() {
        return Ok(());
    }

    let changes = normalize_membership_changes(changes)?;

    favorite_groups::update_memberships(
        &changes
            .iter()
            .map(|change| {
                (
                    change.favorite_key.clone(),
                    change.add_group_ids.clone(),
                    change.remove_group_ids.clone(),
                )
            })
            .collect::<Vec<_>>(),
    )?;

    let favorite_keys = changes
        .into_iter()
        .map(|change| change.favorite_key)
        .collect::<Vec<_>>();
    emit_favorite_groups_changed(&favorite_keys).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        FavoriteGroupMembershipChange, MAX_FAVORITE_GROUP_NAME_LENGTH,
        normalize_membership_changes, validate_group_name,
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
    fn group_name_validation_rejects_long_names() {
        assert!(
            validate_group_name(
                &"a".repeat(MAX_FAVORITE_GROUP_NAME_LENGTH + 1)
            )
            .is_err()
        );
    }

    #[test]
    fn membership_changes_reject_duplicate_favorites() {
        let changes = vec![
            FavoriteGroupMembershipChange {
                favorite_key: "modrinth:sodium".to_string(),
                add_group_ids: vec!["first".to_string()],
                remove_group_ids: vec![],
            },
            FavoriteGroupMembershipChange {
                favorite_key: "modrinth:sodium".to_string(),
                add_group_ids: vec!["second".to_string()],
                remove_group_ids: vec![],
            },
        ];

        assert!(normalize_membership_changes(changes).is_err());
    }

    #[test]
    fn membership_changes_deduplicate_group_ids() {
        let changes =
            normalize_membership_changes(vec![FavoriteGroupMembershipChange {
                favorite_key: "modrinth:sodium".to_string(),
                add_group_ids: vec![
                    "a".to_string(),
                    "b".to_string(),
                    "a".to_string(),
                ],
                remove_group_ids: vec!["c".to_string(), "c".to_string()],
            }])
            .unwrap();

        assert_eq!(changes[0].add_group_ids, &["a", "b"]);
        assert_eq!(changes[0].remove_group_ids, &["c"]);
    }
}
