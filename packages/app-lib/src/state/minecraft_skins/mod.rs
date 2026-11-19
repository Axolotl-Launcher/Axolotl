use super::MinecraftSkinVariant;
use crate::state::media::Media;
use crate::state::store::Handle;
use futures::{Stream, stream};
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy)]
pub enum CustomMinecraftSkinInsertPosition {
    Top,
    Bottom,
    At(i64),
}

fn media(handle: &Handle) -> Media {
    Media::new(&handle.persistence())
}

impl OfflineMinecraftSkin {
    pub async fn get(
        player: Uuid,
        handle: &Handle,
    ) -> crate::Result<Option<Self>> {
        media(handle).offline_skin(player).await
    }
    pub async fn set(
        player: Uuid,
        texture: &str,
        variant: MinecraftSkinVariant,
        handle: &Handle,
    ) -> crate::Result<()> {
        media(handle)
            .set_offline_skin(player, Some(texture.to_owned()), variant, None)
            .await
    }
    pub async fn clear(player: Uuid, handle: &Handle) -> crate::Result<()> {
        media(handle)
            .set_offline_skin(player, None, MinecraftSkinVariant::Unknown, None)
            .await
    }
    pub async fn clear_if_texture(
        player: Uuid,
        texture: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        media(handle)
            .set_offline_skin(
                player,
                None,
                MinecraftSkinVariant::Unknown,
                Some(texture.to_owned()),
            )
            .await
    }
}

impl CustomMinecraftSkin {
    pub async fn add(
        player: Uuid,
        key: &str,
        texture: &[u8],
        variant: MinecraftSkinVariant,
        cape: Option<Uuid>,
        position: CustomMinecraftSkinInsertPosition,
        handle: &Handle,
    ) -> crate::Result<()> {
        media(handle)
            .skin_add(player, key, texture, variant, cape, position)
            .await
    }
    pub async fn get_by_texture(
        player: Uuid,
        key: &str,
        handle: &Handle,
    ) -> crate::Result<Option<Self>> {
        Ok(media(handle)
            .skins(player)
            .await?
            .into_iter()
            .find(|skin| skin.texture_key == key))
    }
    pub async fn get_many(
        player: Uuid,
        offset: u32,
        count: u32,
        handle: &Handle,
    ) -> crate::Result<impl Stream<Item = Self>> {
        Ok(stream::iter(
            media(handle)
                .skins(player)
                .await?
                .into_iter()
                .skip(offset as usize)
                .take(count as usize),
        ))
    }
    pub async fn get_all(
        player: Uuid,
        handle: &Handle,
    ) -> crate::Result<impl Stream<Item = Self>> {
        Self::get_many(player, 0, 2048, handle).await
    }
    pub async fn texture_blob(
        &self,
        handle: &Handle,
    ) -> crate::Result<Vec<u8>> {
        media(handle).skin_texture(&self.texture_key).await
    }
    pub async fn remove(
        &self,
        player: Uuid,
        handle: &Handle,
    ) -> crate::Result<()> {
        media(handle).skin_remove(player, &self.texture_key).await
    }
    pub async fn set_order(
        player: Uuid,
        keys: &[String],
        handle: &Handle,
    ) -> crate::Result<()> {
        media(handle).skin_order(player, keys).await
    }
}
