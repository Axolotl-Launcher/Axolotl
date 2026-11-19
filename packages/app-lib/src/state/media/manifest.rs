//! Screenshot identities are user relationships, not a persisted file index.

use super::{Media, collection};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ScreenshotIdentity {
    pub(crate) id: String,
    pub(crate) instance_id: String,
    pub(crate) file_name: String,
    #[serde(default)]
    pub(crate) fingerprint: Option<String>,
    #[serde(default)]
    pub(crate) group_id: Option<String>,
    /// Only an explicit edit needs to retain the original authoring date.
    #[serde(default)]
    pub(crate) authored_at: Option<i64>,
}

impl ScreenshotIdentity {
    pub(crate) fn for_file(instance_id: &str, file_name: &str) -> Self {
        let digest = Sha256::digest(format!(
            "axolotl-screenshot-v1\0{instance_id}\0{file_name}"
        ));
        let mut bytes: [u8; 16] =
            digest[..16].try_into().expect("digest prefix");
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self {
            id: uuid::Uuid::from_bytes(bytes).to_string(),
            instance_id: instance_id.into(),
            file_name: file_name.into(),
            fingerprint: None,
            group_id: None,
            authored_at: None,
        }
    }
}

impl Media {
    pub(crate) async fn screenshots(
        &self,
    ) -> crate::Result<Vec<ScreenshotIdentity>> {
        let data = self.read().await?;
        Ok(data["screenshots"].as_object().expect("identity map").iter().filter_map(|(id, value)| {
            if !super::supported(value) { return None; }
            let mut identity: ScreenshotIdentity = match serde_json::from_value(value.clone()) {
                Ok(identity) => identity,
                Err(error) => { tracing::warn!(screenshot = id, %error, "Preserving an unreadable screenshot identity"); return None; }
            };
            identity.id = id.clone();
            Some(identity)
        }).collect())
    }

    pub(crate) async fn anchor(
        &self,
        identities: Vec<ScreenshotIdentity>,
    ) -> crate::Result<()> {
        self.update(move |data| {
            for identity in identities {
                let entries = collection(data, "screenshots");
                if entries.iter().any(|(id, value)| id != &identity.id
                    && value.get("instance_id").and_then(Value::as_str) == Some(&identity.instance_id)
                    && value.get("file_name").and_then(Value::as_str) == Some(&identity.file_name)) {
                    return Err(crate::ErrorKind::InputError("Screenshot location has an unreadable or conflicting identity".into()).into());
                }
                if let Some(current) = entries.get_mut(&identity.id) {
                    ensure_identity(current)?;
                    if current["instance_id"].as_str() != Some(&identity.instance_id) || current["file_name"].as_str() != Some(&identity.file_name) {
                        return Err(crate::ErrorKind::InputError("Screenshot location changed".into()).into());
                    }
                    // A scan or edit must not restore a stale group selection.
                    if let Some(hash) = &identity.fingerprint { current["fingerprint"] = json!(hash); }
                    if let Some(time) = identity.authored_at { current["authored_at"] = json!(time); }
                } else { write_identity(entries, &identity)?; }
            }
            Ok(())
        }).await
    }

    pub(crate) async fn forget(
        &self,
        instance: &str,
        file_name: &str,
    ) -> crate::Result<()> {
        let instance = instance.to_owned();
        let file_name = file_name.to_owned();
        self.update(move |data| {
            let entries = collection(data, "screenshots");
            for value in entries.values().filter(|value| {
                value.get("instance_id").and_then(Value::as_str)
                    == Some(&instance)
                    && value.get("file_name").and_then(Value::as_str)
                        == Some(&file_name)
            }) {
                ensure_identity(value)?;
            }
            entries.retain(|_, value| {
                !(value.get("instance_id").and_then(Value::as_str)
                    == Some(&instance)
                    && value.get("file_name").and_then(Value::as_str)
                        == Some(&file_name))
            });
            Ok(())
        })
        .await
    }

    pub(crate) async fn relocate(
        &self,
        id: &str,
        instance: &str,
        file_name: &str,
    ) -> crate::Result<()> {
        let id = id.to_owned();
        let instance = instance.to_owned();
        let file_name = file_name.to_owned();
        self.update(move |data| {
            if collection(data,"screenshots").iter().any(|(other,raw)|other!=&id && raw.get("instance_id").and_then(Value::as_str)==Some(&instance) && raw.get("file_name").and_then(Value::as_str)==Some(&file_name)) {
                return Err(crate::ErrorKind::InputError("Screenshot destination has an opaque or conflicting identity".into()).into());
            }
            let value = collection(data, "screenshots")
                .get_mut(&id)
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Screenshot identity disappeared".into(),
                    )
                })?;
            if !super::supported(value) {
                return Err(crate::ErrorKind::InputError(
                    "Unsupported screenshot identity".into(),
                )
                .into());
            }
            let _: ScreenshotIdentity = serde_json::from_value(value.clone())?;
            value["instance_id"] = json!(instance);
            value["file_name"] = json!(file_name);
            Ok(())
        })
        .await
    }

    pub(crate) async fn groups(&self) -> crate::Result<Vec<(String, String)>> {
        let data = self.read().await?;
        let mut groups = data["groups"]
            .as_object()
            .expect("group map")
            .iter()
            .filter_map(|(id, value)| {
                if !super::supported(value) {
                    return None;
                }
                Some((
                    value.get("display_order")?.as_i64()?,
                    id.clone(),
                    value.get("name")?.as_str()?.to_owned(),
                ))
            })
            .collect::<Vec<_>>();
        groups.sort();
        Ok(groups.into_iter().map(|(_, id, name)| (id, name)).collect())
    }

    pub(crate) async fn create_group(
        &self,
        id: String,
        name: String,
        members: Vec<ScreenshotIdentity>,
    ) -> crate::Result<()> {
        self.update(move |data| {
            let groups = collection(data, "groups");
            if groups.contains_key(&id) {
                return Err(crate::ErrorKind::InputError(
                    "Screenshot group already exists".into(),
                )
                .into());
            }
            for group in groups.values_mut() {
                ensure_group(group)?;
                let order = group
                    .get("display_order")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "A screenshot group uses a newer shape".into(),
                        )
                    })?;
                group["display_order"] = json!(order.saturating_add(1));
            }
            groups.insert(id.clone(), json!({"name":name,"display_order":0}));
            for member in members {
                write_membership(
                    collection(data, "screenshots"),
                    &member,
                    Some(id.clone()),
                )?;
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn rename_group(
        &self,
        id: &str,
        name: String,
    ) -> crate::Result<()> {
        let id = id.to_owned();
        self.update(move |data| {
            let group =
                collection(data, "groups").get_mut(&id).ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Unknown screenshot group".into(),
                    )
                })?;
            ensure_group(group)?;
            group["name"] = json!(name);
            Ok(())
        })
        .await
    }

    pub(crate) async fn delete_group(
        &self,
        id: &str,
    ) -> crate::Result<HashSet<String>> {
        let affected = self
            .screenshots()
            .await?
            .into_iter()
            .filter(|row| row.group_id.as_deref() == Some(id))
            .map(|row| row.instance_id)
            .collect();
        let id = id.to_owned();
        self.update(move |data| {
            let groups = collection(data, "groups");
            if !groups.get(&id).is_some_and(|value| {
                value.get("name").is_some_and(Value::is_string)
            }) {
                return Err(crate::ErrorKind::InputError(
                    "Unknown screenshot group".into(),
                )
                .into());
            }
            ensure_group(&groups[&id])?;
            for row in collection(data, "screenshots").values().filter(|row| {
                row.get("group_id").and_then(Value::as_str) == Some(&id)
            }) {
                ensure_identity(row)?;
            }
            collection(data, "groups").remove(&id);
            for row in collection(data, "screenshots").values_mut() {
                if row.get("group_id").and_then(Value::as_str) == Some(&id) {
                    row["group_id"] = Value::Null;
                }
            }
            Ok(())
        })
        .await?;
        Ok(affected)
    }

    pub(crate) async fn memberships(
        &self,
        updates: Vec<(ScreenshotIdentity, Option<String>)>,
    ) -> crate::Result<()> {
        self.update(move |data| {
            for (identity, group) in updates {
                if let Some(id) = &group
                    && !collection(data, "groups").get(id).is_some_and(
                        |value| value.get("name").is_some_and(Value::is_string),
                    )
                {
                    return Err(crate::ErrorKind::InputError(
                        "Unknown screenshot group".into(),
                    )
                    .into());
                }
                if let Some(id) = &group {
                    ensure_group(&collection(data, "groups")[id])?;
                }
                write_membership(
                    collection(data, "screenshots"),
                    &identity,
                    group,
                )?;
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn import_groups(
        &self,
        groups: Vec<(String, String, Vec<ScreenshotIdentity>)>,
    ) -> crate::Result<()> {
        self.update(move |data| {
            let mut order = collection(data, "groups")
                .values()
                .filter_map(|value| {
                    value.get("display_order").and_then(Value::as_i64)
                })
                .max()
                .unwrap_or(-1)
                .saturating_add(1);
            for (id, name, members) in groups {
                let group = collection(data, "groups")
                    .entry(id.clone())
                    .or_insert_with(
                        || json!({"name":name.clone(),"display_order":order}),
                    );
                ensure_group(group)?;
                group["name"] = json!(name);
                order = order.saturating_add(1);
                for member in members {
                    write_membership(
                        collection(data, "screenshots"),
                        &member,
                        Some(id.clone()),
                    )?;
                }
            }
            Ok(())
        })
        .await
    }
}

fn write_membership(
    entries: &mut Map<String, Value>,
    identity: &ScreenshotIdentity,
    group: Option<String>,
) -> crate::Result<()> {
    if entries.iter().any(|(id, raw)| {
        id != &identity.id
            && raw.get("instance_id").and_then(Value::as_str)
                == Some(&identity.instance_id)
            && raw.get("file_name").and_then(Value::as_str)
                == Some(&identity.file_name)
    }) {
        return Err(crate::ErrorKind::InputError(
            "Screenshot location has an opaque or conflicting identity".into(),
        )
        .into());
    }
    if !entries.contains_key(&identity.id) {
        write_identity(entries, identity)?;
    }
    let current = entries.get_mut(&identity.id).expect("inserted identity");
    ensure_identity(current)?;
    if current["instance_id"].as_str() != Some(&identity.instance_id)
        || current["file_name"].as_str() != Some(&identity.file_name)
    {
        return Err(crate::ErrorKind::InputError(
            "Screenshot location changed while grouping".into(),
        )
        .into());
    }
    current["group_id"] = json!(group);
    if let Some(hash) = &identity.fingerprint {
        current["fingerprint"] = json!(hash);
    }
    Ok(())
}

fn ensure_identity(value: &Value) -> crate::Result<()> {
    if !super::supported(value) {
        return Err(crate::ErrorKind::InputError(
            "Unsupported screenshot identity".into(),
        )
        .into());
    }
    let _: ScreenshotIdentity = serde_json::from_value(value.clone())?;
    Ok(())
}

fn ensure_group(value: &Value) -> crate::Result<()> {
    if !super::supported(value)
        || !value.get("name").is_some_and(Value::is_string)
        || value.get("display_order").and_then(Value::as_i64).is_none()
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported screenshot group".into(),
        )
        .into());
    }
    Ok(())
}

fn write_identity(
    entries: &mut Map<String, Value>,
    identity: &ScreenshotIdentity,
) -> crate::Result<()> {
    let existing = entries
        .entry(identity.id.clone())
        .or_insert_with(|| json!({}));
    if !super::supported(existing) {
        return Err(crate::ErrorKind::InputError(
            "Unsupported screenshot identity".into(),
        )
        .into());
    }
    let object = existing.as_object_mut().ok_or_else(|| {
        crate::ErrorKind::InputError("Unsupported screenshot identity".into())
    })?;
    for (key, value) in serde_json::to_value(identity)?
        .as_object()
        .expect("identity object")
    {
        object.insert(key.clone(), value.clone());
    }
    Ok(())
}

/// A hash identifies content, not identity. Match only unique missing/new
/// pairs and never consume an equal-content sibling's organization.
pub(crate) fn unique_renames(
    existing: &[ScreenshotIdentity],
    present: &HashSet<String>,
    new: &[(String, String)],
) -> Vec<(String, String)> {
    let mut old = HashMap::<String, Vec<&ScreenshotIdentity>>::new();
    for identity in existing {
        if let Some(hash) = &identity.fingerprint {
            old.entry(hash.clone()).or_default().push(identity);
        }
    }
    let mut incoming = HashMap::<String, Vec<&String>>::new();
    for (name, hash) in new {
        incoming.entry(hash.clone()).or_default().push(name);
    }
    old.into_iter()
        .filter_map(|(hash, identities)| {
            if identities.len() != 1
                || present.contains(&identities[0].file_name)
            {
                return None;
            }
            let names = incoming.get(&hash)?;
            if names.len() != 1 {
                return None;
            }
            Some((identities[0].id.clone(), names[0].clone()))
        })
        .collect()
}
