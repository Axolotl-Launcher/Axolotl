use super::{LauncherBackend, page};
use base64::{Engine, engine::general_purpose::STANDARD};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use theseus::minecraft_skins::{
    self as s, Cape, MinecraftSkinVariant as Variant, Skin,
};
fn error(e: impl std::fmt::Display) -> ToolError {
    tracing::warn!(%e,"MCP skin/cape operation failed");
    ToolError::new(
        "skin_operation_failed",
        "Skin/cape operation failed; inspect active account support and sign-in state",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skin_and_cape_summaries_do_not_export_texture_urls_or_payloads() {
        let texture = std::sync::Arc::new(
            url::Url::parse("https://example.com/texture?secret=hidden")
                .unwrap(),
        );
        let skin = Skin {
            texture_key: "skin-key".into(),
            name: Some("Test".into()),
            section: None,
            variant: Variant::Classic,
            cape_id: None,
            texture: texture.clone(),
            source: s::SkinSource::Custom,
            is_equipped: true,
        };
        let cape = Cape {
            id: uuid::Uuid::nil(),
            name: "Cape".into(),
            texture,
            is_equipped: false,
        };
        for value in [skin_summary(skin), cape_summary(cape)] {
            assert!(!value.to_string().contains("secret"));
            assert!(!value.to_string().contains("https:"));
        }
    }
    #[test]
    fn skin_input_rejects_wrong_sizes_and_non_png() {
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(64, 64)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        assert!(png(&STANDARD.encode(out.into_inner())).is_ok());
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(32, 32)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        assert!(png(&STANDARD.encode(out.into_inner())).is_err());
        assert!(png(&STANDARD.encode(b"not a PNG")).is_err());
        assert!(png("bad-base64!").is_err());
    }
    use std::io::Cursor;
}
fn skin_summary(s: Skin) -> Value {
    json!({"skin_id":s.texture_key,"name":s.name,"section":s.section,"variant":s.variant,"cape_id":s.cape_id,"source":s.source,"equipped":s.is_equipped})
}
fn cape_summary(c: Cape) -> Value {
    json!({"cape_id":c.id,"name":c.name,"equipped":c.is_equipped})
}
async fn skin(id: &str) -> Result<Skin, ToolError> {
    s::get_available_skins_with_cache(true)
        .await
        .map_err(error)?
        .into_iter()
        .find(|s| s.texture_key.as_ref() == id)
        .ok_or_else(|| {
            ToolError::new(
                "skin_not_found",
                "Refresh skin_list for the active account",
            )
        })
}
async fn cape(id: Option<uuid::Uuid>) -> Result<Option<Cape>, ToolError> {
    match id{None=>Ok(None),Some(id)=>s::get_available_capes_with_cache(true).await.map_err(error)?.into_iter().find(|c|c.id==id).map(Some).ok_or_else(||ToolError::new("cape_not_found","Cape does not belong to active cached profile; refresh cape_list"))}
}
fn variant(v: &str) -> Result<Variant, ToolError> {
    match v {
        "classic" => Ok(Variant::Classic),
        "slim" => Ok(Variant::Slim),
        _ => Err(error("variant")),
    }
}
fn png(data: &str) -> Result<s::Bytes, ToolError> {
    let bytes = STANDARD.decode(data).map_err(error)?;
    if bytes.len() > 256 * 1024 {
        return Err(error("PNG size"));
    }
    let decoder = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(error)?;
    let dimensions = decoder.into_dimensions().map_err(error)?;
    if dimensions.0 != 64
        || ![32, 64].contains(&dimensions.1)
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return Err(error("skin dimensions"));
    }
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(error)?;
    Ok(bytes.into())
}
async fn flush() -> Result<(), ToolError> {
    let id = theseus::minecraft_auth::get_default_user(false)
        .await
        .map_err(error)?
        .ok_or_else(|| error("account"))?;
    let profile = theseus::minecraft_auth::users_with_cache(false, true)
        .await
        .map_err(error)?
        .into_iter()
        .find(|user| user.account_id == id)
        .ok_or_else(|| error("account"))?
        .profile
        .id;
    s::flush_pending_skin_change_for_profile(profile)
        .await
        .map_err(error)
}
pub(super) async fn execute(
    _: &LauncherBackend,
    command: Command,
    _: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::SkinList {
            refresh,
            offset,
            limit,
        } => page(
            s::get_available_skins_with_cache(!refresh.unwrap_or(false))
                .await
                .map_err(error)?
                .into_iter()
                .map(skin_summary)
                .collect(),
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        ),
        Command::CapeList {
            refresh,
            offset,
            limit,
        } => page(
            s::get_available_capes_with_cache(!refresh.unwrap_or(false))
                .await
                .map_err(error)?
                .into_iter()
                .map(cape_summary)
                .collect(),
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        ),
        Command::SkinGetTexture { skin_id } => {
            let skin = skin(&skin_id).await?;
            let bytes = s::normalize_skin_texture_for_preview(
                &s::UrlOrBlob::Url((*skin.texture).clone()),
            )
            .await
            .map_err(error)?;
            Ok(
                json!({"image":{"data":STANDARD.encode(bytes),"mime_type":"image/png"},"context":{"skin_id":skin_id}}),
            )
        }
        Command::SkinImport {
            png_base64,
            variant: v,
            cape_id,
        } => {
            let result = s::add_and_equip_custom_skin(
                png(&png_base64)?,
                variant(&v)?,
                cape(cape_id).await?,
            )
            .await
            .map_err(error)?;
            flush().await?;
            Ok(skin_summary(result))
        }
        Command::SkinSave {
            skin_id,
            png_base64,
            variant: v,
            cape_id,
            replace_texture,
        } => {
            let current = skin(&skin_id).await?;
            Ok(skin_summary(
                s::save_custom_skin(
                    current,
                    png(&png_base64)?,
                    variant(&v)?,
                    cape(cape_id).await?,
                    replace_texture,
                )
                .await
                .map_err(error)?,
            ))
        }
        Command::SkinEquip { skin_id } => {
            s::equip_skin(skin(&skin_id).await?).await.map_err(error)?;
            flush().await?;
            Ok(json!({"equipped":true,"skin_id":skin_id}))
        }
        Command::SkinDelete { skin_id } => {
            s::remove_custom_skin(skin(&skin_id).await?)
                .await
                .map_err(error)?;
            Ok(json!({"removed":true,"skin_id":skin_id}))
        }
        Command::SkinUnequip {} => {
            s::unequip_skin().await.map_err(error)?;
            flush().await?;
            Ok(json!({"reset_to_default":true}))
        }
        Command::SkinSetOrder { skin_ids } => {
            let mut seen = std::collections::HashSet::new();
            for id in &skin_ids {
                if !seen.insert(id) {
                    return Err(error("duplicate"));
                }
                skin(id).await?;
            }
            s::set_custom_skin_order(skin_ids).await.map_err(error)?;
            Ok(json!({"saved":true}))
        }
        Command::CapeEquip { cape_id } => {
            s::set_equipped_cape(Some(cape_id)).await.map_err(error)?;
            Ok(json!({"equipped":true,"cape_id":cape_id}))
        }
        Command::CapeUnequip {} => {
            s::set_equipped_cape(None).await.map_err(error)?;
            Ok(json!({"unequipped":true}))
        }
        _ => Err(error("command")),
    }
}
