use std::collections::HashSet;

use futures::{Stream, stream};
use uuid::Uuid;

use super::MinecraftSkinVariant;
use crate::state::store::Handle;
use crate::state::store::custom_minecraft_skin_textures::{
    CustomMinecraftSkinTextureRecord, CustomMinecraftSkinTextures,
};
use crate::state::store::custom_minecraft_skins::{
    CustomMinecraftSkinKey, CustomMinecraftSkinRecord, CustomMinecraftSkins,
    SkinVariant as StoredSkinVariant,
};
use crate::state::store::offline_minecraft_skins::{
    OfflineMinecraftSkinRecord, OfflineMinecraftSkins,
};
use crate::state::store::{Collection as _, atom};

pub mod mojang_api;

/// Represents a saved skin row for a Minecraft player.
///
/// The same player and `texture_key` always point to the same saved skin.
/// Changing the model variant or cape updates that saved skin instead of
/// creating a second copy. Bundled default skins with a cape are also stored
/// here so the cape can stay associated with the default skin card.
#[derive(Debug, Clone)]
pub struct CustomMinecraftSkin {
    /// The key for the skin texture, which is akin to a hash that identifies it.
    pub texture_key: String,
    /// The variant of the skin model.
    pub variant: MinecraftSkinVariant,
    /// The UUID of the cape that this skin uses, which should match one of the
    /// cape UUIDs the player has in its profile.
    ///
    /// If `None`, the skin is saved without a cape.
    pub cape_id: Option<Uuid>,
    /// The saved skin display order within this player's saved skins.
    pub display_order: i64,
}

/// The skin selected locally for an offline Minecraft account.
#[derive(Debug, Clone)]
pub struct OfflineMinecraftSkin {
    pub texture_key: String,
    pub variant: MinecraftSkinVariant,
}

fn user_key(minecraft_user_id: Uuid) -> String {
    minecraft_user_id.as_hyphenated().to_string()
}

impl OfflineMinecraftSkin {
    pub async fn get(
        minecraft_user_id: Uuid,
        handle: &Handle,
    ) -> crate::Result<Option<Self>> {
        let selected =
            OfflineMinecraftSkins::get(handle, &user_key(minecraft_user_id))
                .await?;

        Ok(selected.map(|record| Self {
            texture_key: record.texture_key,
            variant: from_stored_variant(record.variant),
        }))
    }

    pub async fn set(
        minecraft_user_id: Uuid,
        texture_key: &str,
        variant: MinecraftSkinVariant,
        handle: &Handle,
    ) -> crate::Result<()> {
        let record = OfflineMinecraftSkinRecord {
            texture_key: texture_key.to_string(),
            variant: to_stored_variant(variant),
        };

        OfflineMinecraftSkins::set(
            handle,
            &user_key(minecraft_user_id),
            Some(&record),
        )
        .await
    }

    pub async fn clear(
        minecraft_user_id: Uuid,
        handle: &Handle,
    ) -> crate::Result<()> {
        OfflineMinecraftSkins::set(handle, &user_key(minecraft_user_id), None)
            .await
    }

    pub async fn clear_if_texture(
        minecraft_user_id: Uuid,
        texture_key: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        let key = user_key(minecraft_user_id);
        let mut unit = atom(handle).await?;
        let selected = OfflineMinecraftSkins::atom_get(&mut unit, &key).await?;

        if selected.is_some_and(|record| record.texture_key == texture_key) {
            OfflineMinecraftSkins::atom_set(&mut unit, &key, None).await?;
        }

        unit.commit().await
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CustomMinecraftSkinInsertPosition {
    Top,
    Bottom,
    At(i64),
}

fn to_skin(
    texture_key: String,
    record: CustomMinecraftSkinRecord,
) -> crate::Result<CustomMinecraftSkin> {
    let cape_id = record
        .cape_id
        .map(|cape_id| Uuid::parse_str(&cape_id))
        .transpose()?;

    Ok(CustomMinecraftSkin {
        texture_key,
        variant: from_stored_variant(record.variant),
        cape_id,
        display_order: record.display_order,
    })
}

impl CustomMinecraftSkin {
    pub async fn add(
        minecraft_user_id: Uuid,
        texture_key: &str,
        texture: &[u8],
        variant: MinecraftSkinVariant,
        cape_id: Option<Uuid>,
        insert_position: CustomMinecraftSkinInsertPosition,
        handle: &Handle,
    ) -> crate::Result<()> {
        let minecraft_user_uuid = user_key(minecraft_user_id);
        let mut unit = atom(handle).await?;
        let mut saved = CustomMinecraftSkins::atom_get_scoped(
            &mut unit,
            &minecraft_user_uuid,
        )
        .await?;

        let existing_order = saved
            .iter()
            .find(|(key, _)| key.texture_key == texture_key)
            .map(|(_, record)| record.display_order);

        let display_order = match existing_order {
            Some(display_order) => display_order,
            None => match insert_position {
                CustomMinecraftSkinInsertPosition::Top => {
                    for (_, record) in &mut saved {
                        record.display_order += 1;
                    }

                    0
                }
                CustomMinecraftSkinInsertPosition::Bottom => saved
                    .iter()
                    .map(|(_, record)| record.display_order)
                    .max()
                    .map_or(0, |display_order| display_order + 1),
                CustomMinecraftSkinInsertPosition::At(display_order) => {
                    for (_, record) in &mut saved {
                        if record.display_order >= display_order {
                            record.display_order += 1;
                        }
                    }

                    display_order
                }
            },
        };

        let skin = CustomMinecraftSkinRecord {
            variant: to_stored_variant(variant),
            cape_id: cape_id.map(|id| id.hyphenated().to_string()),
            display_order,
        };
        let key = CustomMinecraftSkinKey {
            minecraft_user_uuid: minecraft_user_uuid.clone(),
            texture_key: texture_key.to_string(),
        };
        let saved_at = saved
            .iter()
            .position(|(key, _)| key.texture_key == texture_key);
        match saved_at {
            Some(index) => saved[index].1 = skin,
            None => saved.push((key, skin)),
        }

        CustomMinecraftSkinTextures::atom_set(
            &mut unit,
            &texture_key.to_string(),
            Some(&CustomMinecraftSkinTextureRecord {
                texture: texture.to_vec(),
            }),
        )
        .await?;
        CustomMinecraftSkins::atom_set_scoped(
            &mut unit,
            &minecraft_user_uuid,
            &saved,
        )
        .await?;

        unit.commit().await
    }

    pub async fn get_by_texture(
        minecraft_user_id: Uuid,
        texture_key: &str,
        handle: &Handle,
    ) -> crate::Result<Option<Self>> {
        let key = CustomMinecraftSkinKey {
            minecraft_user_uuid: user_key(minecraft_user_id),
            texture_key: texture_key.to_string(),
        };
        let saved = CustomMinecraftSkins::get(handle, &key).await?;

        saved
            .map(|record| to_skin(texture_key.to_string(), record))
            .transpose()
    }

    pub async fn get_many(
        minecraft_user_id: Uuid,
        offset: u32,
        count: u32,
        handle: &Handle,
    ) -> crate::Result<impl Stream<Item = Self>> {
        let saved = CustomMinecraftSkins::get_scoped(
            handle,
            &user_key(minecraft_user_id),
        )
        .await?;

        let skins = saved
            .into_iter()
            .skip(offset as usize)
            .take(count as usize)
            .map(|(key, record)| to_skin(key.texture_key, record))
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(stream::iter(skins))
    }

    pub async fn get_all(
        minecraft_user_id: Uuid,
        handle: &Handle,
    ) -> crate::Result<impl Stream<Item = Self>> {
        // Limit ourselves to 2048 skins, so that memory usage even when storing base64
        // PNG data of a 64x64 texture with random pixels stays around ~150 MiB
        Self::get_many(minecraft_user_id, 0, 2048, handle).await
    }

    pub async fn texture_blob(
        &self,
        handle: &Handle,
    ) -> crate::Result<Vec<u8>> {
        let texture =
            CustomMinecraftSkinTextures::get(handle, &self.texture_key)
                .await?
                .map(|record| record.texture);

        texture.ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "The texture of the saved skin {} is missing",
                self.texture_key
            ))
            .into()
        })
    }

    pub async fn remove(
        &self,
        minecraft_user_id: Uuid,
        handle: &Handle,
    ) -> crate::Result<()> {
        let key = CustomMinecraftSkinKey {
            minecraft_user_uuid: user_key(minecraft_user_id),
            texture_key: self.texture_key.clone(),
        };

        CustomMinecraftSkins::set(handle, &key, None).await
    }

    pub async fn set_order(
        minecraft_user_id: Uuid,
        texture_keys: &[String],
        handle: &Handle,
    ) -> crate::Result<()> {
        let minecraft_user_uuid = user_key(minecraft_user_id);
        let mut unit = atom(handle).await?;
        let mut saved = CustomMinecraftSkins::atom_get_scoped(
            &mut unit,
            &minecraft_user_uuid,
        )
        .await?;

        let existing_keys = saved
            .iter()
            .map(|(key, _)| key.texture_key.clone())
            .collect::<HashSet<_>>();
        let mut seen_keys = HashSet::new();
        let mut ordered_keys = Vec::with_capacity(saved.len());

        for texture_key in texture_keys {
            if seen_keys.insert(texture_key.clone())
                && existing_keys.contains(texture_key)
            {
                ordered_keys.push(texture_key.clone());
            }
        }

        for (key, _) in &saved {
            if seen_keys.insert(key.texture_key.clone()) {
                ordered_keys.push(key.texture_key.clone());
            }
        }

        for (display_order, texture_key) in ordered_keys.into_iter().enumerate()
        {
            if let Some((_, record)) = saved
                .iter_mut()
                .find(|(key, _)| key.texture_key == texture_key)
            {
                record.display_order = display_order as i64;
            }
        }

        CustomMinecraftSkins::atom_set_scoped(
            &mut unit,
            &minecraft_user_uuid,
            &saved,
        )
        .await?;

        unit.commit().await
    }
}

fn to_stored_variant(variant: MinecraftSkinVariant) -> StoredSkinVariant {
    match variant {
        MinecraftSkinVariant::Classic => StoredSkinVariant::Classic,
        MinecraftSkinVariant::Slim => StoredSkinVariant::Slim,
        MinecraftSkinVariant::Unknown => StoredSkinVariant::Unknown,
    }
}

/// The model a stored skin names. Every stored value is one this build knows:
/// the column only accepts these three.
fn from_stored_variant(variant: StoredSkinVariant) -> MinecraftSkinVariant {
    match variant {
        StoredSkinVariant::Classic => MinecraftSkinVariant::Classic,
        StoredSkinVariant::Slim => MinecraftSkinVariant::Slim,
        StoredSkinVariant::Unknown => MinecraftSkinVariant::Unknown,
    }
}
