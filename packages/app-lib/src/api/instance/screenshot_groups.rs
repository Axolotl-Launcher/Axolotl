use crate::State;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::media::{Media, manifest::ScreenshotIdentity};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::OwnedMutexGuard;
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
    Ok(Media::new(&state.persistence)
        .groups()
        .await?
        .into_iter()
        .map(|(id, name)| ScreenshotGroup { id, name })
        .collect())
}

pub async fn create_screenshot_group(
    name: String,
    screenshot_ids: Vec<String>,
) -> crate::Result<ScreenshotGroup> {
    let name = validate_group_name(&name)?.to_owned();
    let state = State::get().await?;
    let (members, _locks) =
        resolve_members(&state, normalize_screenshot_ids(screenshot_ids)?)
            .await?;
    let instances = members
        .iter()
        .map(|member| member.instance_id.clone())
        .collect::<HashSet<_>>();
    let id = Uuid::new_v4().to_string();
    Media::new(&state.persistence)
        .create_group(id.clone(), name.clone(), members)
        .await?;
    emit_screenshot_updates(instances).await;
    Ok(ScreenshotGroup { id, name })
}

pub async fn rename_screenshot_group(
    id: String,
    new_name: String,
) -> crate::Result<ScreenshotGroup> {
    let name = validate_group_name(&new_name)?.to_owned();
    let state = State::get().await?;
    Media::new(&state.persistence)
        .rename_group(&id, name.clone())
        .await?;
    Ok(ScreenshotGroup { id, name })
}

pub async fn delete_screenshot_group(id: String) -> crate::Result<()> {
    let state = State::get().await?;
    let affected = Media::new(&state.persistence).delete_group(&id).await?;
    emit_screenshot_updates(affected).await;
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
    let (members, _locks) = resolve_members(
        &state,
        updates
            .iter()
            .map(|update| update.screenshot_id.clone())
            .collect(),
    )
    .await?;
    let affected = members
        .iter()
        .map(|member| member.instance_id.clone())
        .collect::<HashSet<_>>();
    Media::new(&state.persistence)
        .memberships(
            members
                .into_iter()
                .zip(updates)
                .map(|(identity, update)| (identity, update.group_id))
                .collect(),
        )
        .await?;
    emit_screenshot_updates(affected).await;
    Ok(())
}

pub async fn import_screenshot_groups(
    groups: Vec<ScreenshotGroupImport>,
) -> crate::Result<()> {
    if groups.is_empty() {
        return Ok(());
    }
    let mut group_ids = HashSet::new();
    let mut ids = HashSet::new();
    for group in &groups {
        validate_group_name(&group.name)?;
        normalize_screenshot_ids(group.screenshot_ids.clone())?;
        if group.id.trim().is_empty() || !group_ids.insert(group.id.clone()) {
            return Err(crate::ErrorKind::InputError(
                "Screenshot group import contains an invalid group ID".into(),
            )
            .into());
        }
        ids.extend(group.screenshot_ids.iter().cloned());
    }
    let state = State::get().await?;
    let (members, _locks) =
        resolve_members(&state, ids.into_iter().collect()).await?;
    let affected = members
        .iter()
        .map(|member| member.instance_id.clone())
        .collect::<HashSet<_>>();
    let by_id = members
        .into_iter()
        .map(|member| (member.id.clone(), member))
        .collect::<HashMap<_, _>>();
    let imported = groups
        .into_iter()
        .map(|group| {
            (
                group.id,
                group.name.trim().to_owned(),
                group
                    .screenshot_ids
                    .into_iter()
                    .map(|id| by_id[&id].clone())
                    .collect(),
            )
        })
        .collect();
    Media::new(&state.persistence)
        .import_groups(imported)
        .await?;
    emit_screenshot_updates(affected).await;
    Ok(())
}

// IDs for ordinary files are deterministic; only explicit organization anchors
// them durably. Hashing here enables later external rename recovery.
async fn resolve_members(
    state: &State,
    ids: Vec<String>,
) -> crate::Result<(Vec<ScreenshotIdentity>, Vec<OwnedMutexGuard<()>>)> {
    let media = Media::new(&state.persistence);
    let mut existing = media
        .screenshots()
        .await?
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
    let scanned = if ids.iter().any(|id| !existing.contains_key(id)) {
        super::screenshots::list_all_screenshots().await?
    } else {
        Vec::new()
    };
    // The scan may have reconciled renamed anchored identities.
    existing = media
        .screenshots()
        .await?
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect();
    let mut paths = HashMap::new();
    for file in scanned {
        paths.insert(
            file.id.clone(),
            super::screenshots::ScreenshotKey {
                instance_id: file.instance_id.clone(),
                file_name: file.file_name.clone(),
            },
        );
        existing.entry(file.id.clone()).or_insert_with(|| {
            ScreenshotIdentity::for_file(&file.instance_id, &file.file_name)
        });
    }
    let mut members = ids
        .into_iter()
        .map(|id| {
            existing.get(&id).cloned().ok_or_else(|| {
                crate::ErrorKind::InputError(format!("Unknown screenshot {id}"))
                    .into()
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let mut instances = members
        .iter()
        .map(|member| member.instance_id.clone())
        .collect::<Vec<_>>();
    instances.sort();
    instances.dedup();
    let mut locks = Vec::new();
    for instance in instances {
        locks.push(state.lock_instance_screenshots(&instance).await);
    }
    for member in &mut members {
        if member.fingerprint.is_none() {
            let key = paths.get(&member.id).cloned().unwrap_or_else(|| {
                super::screenshots::ScreenshotKey {
                    instance_id: member.instance_id.clone(),
                    file_name: member.file_name.clone(),
                }
            });
            let path = super::screenshots::get_screenshot_path(&key).await?;
            member.fingerprint =
                Some(crate::util::fetch::sha1_file_async(&path).await?.1);
        }
    }
    Ok((members, locks))
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
