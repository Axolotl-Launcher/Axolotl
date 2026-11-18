//! Saved skin selections and order are manifest data; texture bodies are files.

use super::{Media, collection};
use crate::state::MinecraftSkinVariant;
use crate::state::minecraft_skins::{
    CustomMinecraftSkin, CustomMinecraftSkinInsertPosition,
    OfflineMinecraftSkin,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) fn variant(value: &Value) -> Option<MinecraftSkinVariant> {
    match value.as_str()? {
        "CLASSIC" => Some(MinecraftSkinVariant::Classic),
        "SLIM" => Some(MinecraftSkinVariant::Slim),
        "UNKNOWN" => Some(MinecraftSkinVariant::Unknown),
        _ => None,
    }
}

impl Media {
    pub(crate) async fn skin_add(
        &self,
        player: Uuid,
        key: &str,
        bytes: &[u8],
        model: MinecraftSkinVariant,
        cape: Option<Uuid>,
        position: CustomMinecraftSkinInsertPosition,
    ) -> crate::Result<()> {
        let digest = format!("{:x}", Sha256::digest(bytes));
        crate::state::persistence::document::write_blob(
            &self.texture_path(&digest)?,
            bytes.to_vec(),
        )
        .await?;
        let player = player.to_string();
        let key = key.to_owned();
        self.update(move |data| {
            let players = collection(data, "saved_skins");
            let saved = players
                .entry(player)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Unsupported saved skin collection".into(),
                    )
                })?;
            let existing = saved
                .get(&key)
                .and_then(|value| value.get("display_order"))
                .and_then(Value::as_i64);
            let order = match existing {
                Some(order) => order,
                None => {
                    let order = match position {
                        CustomMinecraftSkinInsertPosition::Top => 0,
                        CustomMinecraftSkinInsertPosition::Bottom => saved
                            .values()
                            .filter_map(|value| {
                                value
                                    .get("display_order")
                                    .and_then(Value::as_i64)
                            })
                            .max()
                            .unwrap_or(-1)
                            .saturating_add(1),
                        CustomMinecraftSkinInsertPosition::At(order) => order,
                    };
                    if !matches!(
                        position,
                        CustomMinecraftSkinInsertPosition::Bottom
                    ) {
                        for value in saved.values_mut() {
                            if !super::supported(value)
                                || value
                                    .get("variant")
                                    .and_then(variant)
                                    .is_none()
                            {
                                return Err(crate::ErrorKind::InputError(
                                    "Unsupported saved skin".into(),
                                )
                                .into());
                            }
                            let current = value
                                .get("display_order")
                                .and_then(Value::as_i64)
                                .ok_or_else(|| {
                                    crate::ErrorKind::InputError(
                                        "Unsupported saved skin order".into(),
                                    )
                                })?;
                            if current >= order {
                                value["display_order"] =
                                    json!(current.saturating_add(1));
                            }
                        }
                    }
                    order
                }
            };
            let entry = saved.entry(key.clone()).or_insert_with(|| json!({}));
            if !super::supported(entry)
                || entry
                    .get("variant")
                    .is_some_and(|value| variant(value).is_none())
            {
                return Err(crate::ErrorKind::InputError(
                    "Unsupported saved skin".into(),
                )
                .into());
            }
            entry["variant"] = serde_json::to_value(model)?;
            entry["cape_id"] = json!(cape);
            entry["display_order"] = json!(order);
            let texture = collection(data, "textures")
                .entry(key)
                .or_insert_with(|| json!({}));
            if !super::supported(texture) {
                return Err(crate::ErrorKind::InputError(
                    "Unsupported skin texture".into(),
                )
                .into());
            }
            texture["sha256"] = json!(digest);
            Ok(())
        })
        .await
    }

    pub(crate) async fn skins(
        &self,
        player: Uuid,
    ) -> crate::Result<Vec<CustomMinecraftSkin>> {
        let data = self.read().await?;
        let mut skins = Vec::new();
        if let Some(saved) = data["saved_skins"]
            .get(player.to_string())
            .and_then(Value::as_object)
        {
            for (key, raw) in saved {
                if !super::supported(raw) {
                    continue;
                }
                let Some(model) = raw.get("variant").and_then(variant) else {
                    continue;
                };
                let Some(order) =
                    raw.get("display_order").and_then(Value::as_i64)
                else {
                    continue;
                };
                let cape_id = match raw.get("cape_id") {
                    Some(Value::String(value)) => {
                        match Uuid::parse_str(value) {
                            Ok(id) => Some(id),
                            Err(_) => continue,
                        }
                    }
                    Some(Value::Null) | None => None,
                    _ => continue,
                };
                skins.push(CustomMinecraftSkin {
                    texture_key: key.clone(),
                    variant: model,
                    cape_id,
                    display_order: order,
                });
            }
        }
        skins.sort_by(|a, b| {
            (a.display_order, &a.texture_key)
                .cmp(&(b.display_order, &b.texture_key))
        });
        Ok(skins)
    }

    pub(crate) async fn skin_texture(
        &self,
        key: &str,
    ) -> crate::Result<Vec<u8>> {
        let data = self.read().await?;
        let digest = data["textures"]
            .get(key)
            .filter(|value| super::supported(value))
            .and_then(|value| value.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                crate::ErrorKind::FSError(
                    "Saved skin texture is missing".into(),
                )
            })?;
        let bytes = crate::util::io::read(self.texture_path(digest)?).await?;
        if format!("{:x}", Sha256::digest(&bytes)) != digest {
            return Err(crate::ErrorKind::FSError(
                "Saved skin texture is corrupted".into(),
            )
            .into());
        }
        Ok(bytes)
    }

    pub(crate) fn texture_path(&self, digest: &str) -> crate::Result<PathBuf> {
        if digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(crate::ErrorKind::InputError(
                "Invalid skin content identity".into(),
            )
            .into());
        }
        Ok(self
            .persistence
            .root()
            .join("data/media/skins")
            .join(format!("{digest}.png")))
    }

    pub(crate) async fn skin_remove(
        &self,
        player: Uuid,
        key: &str,
    ) -> crate::Result<()> {
        let player = player.to_string();
        let key = key.to_owned();
        self.update(move |data| {
            if let Some(saved) = collection(data, "saved_skins")
                .get_mut(&player)
                .and_then(Value::as_object_mut)
            {
                if saved
                    .get(&key)
                    .is_some_and(|value| !super::supported(value))
                {
                    return Err(crate::ErrorKind::InputError(
                        "Unsupported saved skin".into(),
                    )
                    .into());
                }
                if saved
                    .get(&key)
                    .and_then(|value| value.get("variant"))
                    .is_some_and(|value| variant(value).is_none())
                {
                    return Err(crate::ErrorKind::InputError(
                        "Unsupported saved skin".into(),
                    )
                    .into());
                }
                saved.remove(&key);
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn skin_order(
        &self,
        player: Uuid,
        keys: &[String],
    ) -> crate::Result<()> {
        let player = player.to_string();
        let keys = keys.to_vec();
        self.update(move |data| {
            let Some(saved) = collection(data, "saved_skins")
                .get_mut(&player)
                .and_then(Value::as_object_mut)
            else {
                return Ok(());
            };
            let mut current = saved
                .iter()
                .map(|(key, value)| {
                    let order = value
                        .get("display_order")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| {
                            crate::ErrorKind::InputError(
                                "Unsupported skin order".into(),
                            )
                        })?;
                    if !super::supported(value)
                        || value.get("variant").and_then(variant).is_none()
                    {
                        return Err(crate::ErrorKind::InputError(
                            "Unsupported skin model".into(),
                        )
                        .into());
                    }
                    Ok((order, key.clone()))
                })
                .collect::<crate::Result<Vec<_>>>()?;
            current.sort();
            let mut seen = HashSet::new();
            let order = keys
                .into_iter()
                .chain(current.into_iter().map(|(_, key)| key))
                .filter(|key| {
                    saved.contains_key(key) && seen.insert(key.clone())
                })
                .collect::<Vec<_>>();
            for (index, key) in order.into_iter().enumerate() {
                saved[&key]["display_order"] = json!(index as i64);
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn offline_skin(
        &self,
        player: Uuid,
    ) -> crate::Result<Option<OfflineMinecraftSkin>> {
        let data = self.read().await?;
        let Some(raw) = data["offline_skins"].get(player.to_string()) else {
            return Ok(None);
        };
        if !super::supported(raw) {
            return Ok(None);
        }
        Ok(
            match (
                raw.get("texture_key").and_then(Value::as_str),
                raw.get("variant").and_then(variant),
            ) {
                (Some(key), Some(model)) => Some(OfflineMinecraftSkin {
                    texture_key: key.into(),
                    variant: model,
                }),
                _ => None,
            },
        )
    }

    pub(crate) async fn set_offline_skin(
        &self,
        player: Uuid,
        key: Option<String>,
        model: MinecraftSkinVariant,
        only_texture: Option<String>,
    ) -> crate::Result<()> {
        let player = player.to_string();
        self.update(move |data| {
            let entries = collection(data, "offline_skins");
            if let Some(current) = entries.get(&player) {
                if !super::supported(current)
                    || current.get("variant").and_then(variant).is_none()
                {
                    return Err(crate::ErrorKind::InputError(
                        "Unsupported offline skin".into(),
                    )
                    .into());
                }
                if only_texture.as_ref().is_some_and(|key| {
                    current.get("texture_key").and_then(Value::as_str)
                        != Some(key)
                }) {
                    return Ok(());
                }
            } else if only_texture.is_some() {
                return Ok(());
            }
            if let Some(key) = key {
                let entry = entries.entry(player).or_insert_with(|| json!({}));
                entry["texture_key"] = json!(key);
                entry["variant"] = serde_json::to_value(model)?;
            } else {
                entries.remove(&player);
            }
            Ok(())
        })
        .await
    }
}
