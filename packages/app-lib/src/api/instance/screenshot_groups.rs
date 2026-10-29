use crate::State;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::instances::adapters::sqlite::{
    screenshot_groups, screenshot_rows,
};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

const MAX_GROUP_NAME_LENGTH: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenshotGroup {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenshotGroupMembershipUpdate {
    pub screenshot_id: String,
    pub group_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenshotGroupImport {
    pub id: String,
    pub name: String,
    pub screenshot_ids: Vec<String>,
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
    Ok(name)
}

pub async fn list_screenshot_groups() -> crate::Result<Vec<ScreenshotGroup>> {
    let state = State::get().await?;
    Ok(screenshot_rows::list_groups(&state.pool)
        .await?
        .into_iter()
        .map(|group| ScreenshotGroup {
            id: group.id,
            name: group.name,
        })
        .collect())
}

pub async fn create_screenshot_group(
    name: String,
    screenshot_ids: Vec<String>,
) -> crate::Result<ScreenshotGroup> {
    let name = validate_group_name(&name)?;
    let state = State::get().await?;
    let id = Uuid::new_v4().to_string();
    let mut tx = state.pool.begin().await?;

    screenshot_groups::create_group(&mut tx, &id, name).await?;

    let instance_ids = set_group_members(
        &id,
        normalize_screenshot_ids(screenshot_ids)?,
        &mut tx,
    )
    .await?;
    tx.commit().await?;
    emit_screenshot_updates(instance_ids).await;

    Ok(ScreenshotGroup {
        id,
        name: name.to_string(),
    })
}

pub async fn rename_screenshot_group(
    id: String,
    new_name: String,
) -> crate::Result<ScreenshotGroup> {
    let new_name = validate_group_name(&new_name)?;
    let state = State::get().await?;
    let mut tx = state.pool.begin().await?;
    if !screenshot_groups::rename_group(&mut tx, &id, new_name).await? {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown screenshot group {id}"
        ))
        .into());
    }
    tx.commit().await?;

    Ok(ScreenshotGroup {
        id,
        name: new_name.to_string(),
    })
}

pub async fn delete_screenshot_group(id: String) -> crate::Result<()> {
    let state = State::get().await?;
    let mut tx = state.pool.begin().await?;
    let instance_ids =
        screenshot_groups::get_group_instance_ids(&id, &mut *tx).await?;
    if !screenshot_groups::delete_group(&mut tx, &id).await? {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown screenshot group {id}"
        ))
        .into());
    }
    tx.commit().await?;
    emit_screenshot_updates(instance_ids).await;
    Ok(())
}

pub async fn set_screenshot_group_memberships(
    updates: Vec<ScreenshotGroupMembershipUpdate>,
) -> crate::Result<()> {
    let updates = normalize_membership_updates(updates)?;
    if updates.is_empty() {
        return Ok(());
    }

    let state = State::get().await?;
    let mut tx = state.pool.begin().await?;
    let group_ids = updates
        .iter()
        .filter_map(|update| update.group_id.as_deref())
        .collect::<HashSet<_>>();
    for group_id in group_ids {
        ensure_group_exists(group_id, &mut tx).await?;
    }

    let mut instance_ids = HashSet::new();
    for update in updates {
        let instance_id =
            screenshot_instance_id(&update.screenshot_id, &mut tx).await?;
        instance_ids.insert(instance_id);
        screenshot_groups::set_membership(
            &mut tx,
            &update.screenshot_id,
            update.group_id.as_deref(),
        )
        .await?;
    }
    tx.commit().await?;
    emit_screenshot_updates(instance_ids).await;
    Ok(())
}

pub async fn import_screenshot_groups(
    groups: Vec<ScreenshotGroupImport>,
) -> crate::Result<()> {
    if groups.is_empty() {
        return Ok(());
    }

    let state = State::get().await?;
    let mut tx = state.pool.begin().await?;
    let base_order =
        screenshot_groups::get_next_display_order(&mut *tx).await?;
    let mut group_ids = HashSet::new();
    let mut affected_instance_ids = HashSet::new();

    for (index, group) in groups.into_iter().enumerate() {
        if group.id.trim().is_empty() || !group_ids.insert(group.id.clone()) {
            return Err(crate::ErrorKind::InputError(
                "Screenshot group import contains an invalid group ID"
                    .to_string(),
            )
            .into());
        }
        let name = validate_group_name(&group.name)?;
        let display_order = base_order + index as i64;
        screenshot_groups::upsert_imported_group(
            &mut tx,
            &group.id,
            name,
            display_order,
        )
        .await?;
        affected_instance_ids.extend(
            set_group_members(
                &group.id,
                normalize_screenshot_ids(group.screenshot_ids)?,
                &mut tx,
            )
            .await?,
        );
    }
    tx.commit().await?;
    emit_screenshot_updates(affected_instance_ids).await;
    Ok(())
}

fn normalize_screenshot_ids(
    screenshot_ids: Vec<String>,
) -> crate::Result<Vec<String>> {
    let mut unique = HashSet::new();
    if screenshot_ids
        .iter()
        .all(|screenshot_id| unique.insert(screenshot_id.clone()))
    {
        return Ok(screenshot_ids);
    }
    Err(crate::ErrorKind::InputError(
        "Screenshot group contains duplicate screenshots".to_string(),
    )
    .into())
}

fn normalize_membership_updates(
    updates: Vec<ScreenshotGroupMembershipUpdate>,
) -> crate::Result<Vec<ScreenshotGroupMembershipUpdate>> {
    let mut screenshot_ids = HashSet::new();
    if updates
        .iter()
        .all(|update| screenshot_ids.insert(update.screenshot_id.clone()))
    {
        return Ok(updates);
    }
    Err(crate::ErrorKind::InputError(
        "Screenshot membership update contains duplicate screenshots"
            .to_string(),
    )
    .into())
}

async fn set_group_members(
    group_id: &str,
    screenshot_ids: Vec<String>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<HashSet<String>> {
    let mut instance_ids = HashSet::new();
    for screenshot_id in screenshot_ids {
        instance_ids.insert(screenshot_instance_id(&screenshot_id, tx).await?);
        screenshot_groups::upsert_membership(tx, &screenshot_id, group_id)
            .await?;
    }
    Ok(instance_ids)
}

async fn ensure_group_exists(
    group_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    if !screenshot_groups::group_exists(group_id, &mut **tx).await? {
        return Err(crate::ErrorKind::InputError(format!(
            "Unknown screenshot group {group_id}"
        ))
        .into());
    }
    Ok(())
}

async fn screenshot_instance_id(
    screenshot_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<String> {
    screenshot_groups::get_screenshot_instance_id(screenshot_id, &mut **tx)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unknown screenshot {screenshot_id}"
            ))
        })
        .map_err(Into::into)
}

async fn emit_screenshot_updates(
    instance_ids: impl IntoIterator<Item = String>,
) {
    for instance_id in instance_ids {
        let _ = emit_instance(
            &instance_id,
            InstancePayloadType::ScreenshotsUpdated,
        )
        .await;
    }
}
