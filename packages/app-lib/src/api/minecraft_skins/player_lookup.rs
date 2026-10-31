//! Public player textures, independent of the selected account's credentials.

use std::time::Duration;

use base64::Engine;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{MinecraftSkinVariant, UrlOrBlob, png_util};
use crate::{
    State,
    util::mojang::{mojang_service_url, should_use_mojang_mirror},
};

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PlayerSkinLookup {
    Found {
        name: String,
        uuid: Uuid,
        variant: MinecraftSkinVariant,
        texture: String,
        preview_texture: String,
    },
    InvalidInput,
    NotFound,
    NoCustomSkin,
    RateLimited,
    Unavailable,
}

#[derive(Deserialize)]
struct Profile {
    id: Uuid,
    name: String,
    #[serde(default)]
    properties: Vec<Property>,
}

#[derive(Deserialize)]
struct Property {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct TexturePayload {
    textures: Textures,
}

#[derive(Deserialize)]
struct Textures {
    #[serde(rename = "SKIN")]
    skin: Option<Texture>,
}

#[derive(Deserialize)]
struct Texture {
    url: String,
    metadata: Option<TextureMetadata>,
}

#[derive(Deserialize)]
struct TextureMetadata {
    model: Option<String>,
}

fn valid_name(input: &str) -> bool {
    (1..=16).contains(&input.len())
        && input
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn player_id(input: &str) -> Result<Option<Uuid>, PlayerSkinLookup> {
    if matches!(input.len(), 32 | 36) {
        return Uuid::parse_str(input)
            .map(Some)
            .map_err(|_| PlayerSkinLookup::InvalidInput);
    }
    valid_name(input)
        .then_some(None)
        .ok_or(PlayerSkinLookup::InvalidInput)
}

fn texture_url(input: &str) -> Option<Url> {
    let mut url = Url::parse(input).ok()?;
    let hash = url.path().strip_prefix("/texture/")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str() != Some("textures.minecraft.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || hash.len() != 64
        || !hash.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    url.set_scheme("https").ok()?;
    Some(url)
}

#[derive(Clone, Copy)]
enum RequestKind {
    Name,
    Profile,
    Texture,
}

impl RequestKind {
    fn stage(self) -> &'static str {
        match self {
            Self::Name => "name_lookup",
            Self::Profile => "profile_lookup",
            Self::Texture => "texture_download",
        }
    }
}

fn unavailable(stage: &'static str, reason: &'static str) -> PlayerSkinLookup {
    tracing::warn!(stage, reason, "Player skin lookup failed");
    PlayerSkinLookup::Unavailable
}

fn check_status(
    status: StatusCode,
    kind: RequestKind,
) -> Result<(), PlayerSkinLookup> {
    match status {
        StatusCode::OK => Ok(()),
        StatusCode::NO_CONTENT | StatusCode::NOT_FOUND
            if !matches!(kind, RequestKind::Texture) =>
        {
            Err(PlayerSkinLookup::NotFound)
        }
        StatusCode::TOO_MANY_REQUESTS => Err(PlayerSkinLookup::RateLimited),
        _ => {
            tracing::warn!(
                stage = kind.stage(),
                status = status.as_u16(),
                "Player skin request returned an unexpected status"
            );
            Err(PlayerSkinLookup::Unavailable)
        }
    }
}

async fn read_limited(
    mut response: Response,
    limit: usize,
    stage: &'static str,
) -> Result<Vec<u8>, PlayerSkinLookup> {
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(unavailable(stage, "response exceeds size limit"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| unavailable(stage, "could not read response body"))?
    {
        if bytes.len() + chunk.len() > limit {
            return Err(unavailable(stage, "response exceeds size limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn request(
    client: &Client,
    url: &str,
    kind: RequestKind,
) -> Result<Response, PlayerSkinLookup> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(stage = kind.stage(), error = %error.without_url(), "Player skin request failed");
            PlayerSkinLookup::Unavailable
        })?;
    check_status(response.status(), kind)?;
    Ok(response)
}

async fn lookup(input: &str) -> Result<PlayerSkinLookup, PlayerSkinLookup> {
    let input = input.trim();
    let id = player_id(input)?;
    let state = State::get().await.map_err(|_| {
        unavailable("initialization", "could not access application state")
    })?;
    let client = state.configured_http_client();
    let mirror = should_use_mojang_mirror();
    let id = match id {
        Some(id) => id,
        None => {
            let url = format!(
                "https://api.mojang.com/users/profiles/minecraft/{input}"
            );
            let response = request(
                &client,
                &mojang_service_url(&url, mirror),
                RequestKind::Name,
            )
            .await?;
            let profile: Profile = serde_json::from_slice(
                &read_limited(response, 64 * 1024, RequestKind::Name.stage())
                    .await?,
            )
            .map_err(|_| unavailable("name_lookup", "invalid profile JSON"))?;
            profile.id
        }
    };
    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        id.simple()
    );
    let response = request(
        &client,
        &mojang_service_url(&url, mirror),
        RequestKind::Profile,
    )
    .await?;
    let profile: Profile = serde_json::from_slice(
        &read_limited(response, 64 * 1024, RequestKind::Profile.stage())
            .await?,
    )
    .map_err(|_| unavailable("profile_lookup", "invalid profile JSON"))?;
    if profile.id != id || !valid_name(&profile.name) {
        return Err(unavailable(
            "profile_lookup",
            "profile identity mismatch or invalid name",
        ));
    }
    let property = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or(PlayerSkinLookup::NoCustomSkin)?;
    let payload = base64::engine::general_purpose::STANDARD
        .decode(&property.value)
        .map_err(|_| {
            unavailable("texture_metadata", "invalid base64 texture property")
        })?;
    let payload: TexturePayload = serde_json::from_slice(&payload)
        .map_err(|_| unavailable("texture_metadata", "invalid texture JSON"))?;
    let texture = payload
        .textures
        .skin
        .ok_or(PlayerSkinLookup::NoCustomSkin)?;
    let url = texture_url(&texture.url).ok_or_else(|| {
        unavailable("texture_metadata", "invalid texture URL")
    })?;
    let variant = match texture.metadata.and_then(|m| m.model).as_deref() {
        Some("slim") => MinecraftSkinVariant::Slim,
        None | Some("classic") => MinecraftSkinVariant::Classic,
        _ => {
            return Err(unavailable(
                "texture_metadata",
                "unsupported skin model",
            ));
        }
    };
    let response = request(&client, url.as_str(), RequestKind::Texture).await?;
    let bytes =
        read_limited(response, 1024 * 1024, RequestKind::Texture.stage())
            .await?;
    let (width, height) = png_util::dimensions(&bytes)
        .map_err(|_| unavailable("texture_validation", "invalid PNG"))?;
    if width != 64 || !matches!(height, 32 | 64) {
        return Err(unavailable(
            "texture_validation",
            "unsupported PNG dimensions",
        ));
    }
    let preview = super::normalize_skin_texture_for_preview(&UrlOrBlob::Blob(
        bytes.clone().into(),
    ))
    .await
    .map_err(|_| {
        unavailable("preview_normalization", "could not normalize skin PNG")
    })?;
    Ok(PlayerSkinLookup::Found {
        name: profile.name,
        uuid: id,
        variant,
        texture: png_util::blob_to_data_url(bytes)
            .ok_or_else(|| {
                unavailable("texture_encoding", "could not encode skin PNG")
            })?
            .to_string(),
        preview_texture: png_util::blob_to_data_url(preview)
            .ok_or_else(|| {
                unavailable("preview_encoding", "could not encode preview PNG")
            })?
            .to_string(),
    })
}

pub async fn lookup_player_skin(input: &str) -> PlayerSkinLookup {
    lookup(input).await.unwrap_or_else(|error| error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_profiles_and_missing_textures_have_distinct_errors() {
        for status in [StatusCode::NOT_FOUND, StatusCode::NO_CONTENT] {
            for kind in [RequestKind::Name, RequestKind::Profile] {
                assert!(matches!(
                    check_status(status, kind),
                    Err(PlayerSkinLookup::NotFound)
                ));
            }
            assert!(matches!(
                check_status(status, RequestKind::Texture),
                Err(PlayerSkinLookup::Unavailable)
            ));
        }
        for kind in [
            RequestKind::Name,
            RequestKind::Profile,
            RequestKind::Texture,
        ] {
            assert!(check_status(StatusCode::OK, kind).is_ok());
            assert!(matches!(
                check_status(StatusCode::TOO_MANY_REQUESTS, kind),
                Err(PlayerSkinLookup::RateLimited)
            ));
            assert!(matches!(
                check_status(StatusCode::INTERNAL_SERVER_ERROR, kind),
                Err(PlayerSkinLookup::Unavailable)
            ));
        }
    }

    #[test]
    fn accepts_names_and_both_uuid_formats_but_rejects_urls_and_paths() {
        for name in ["Notch", "_Player123", "a"] {
            assert!(player_id(name).unwrap().is_none());
        }
        for id in [
            "069a79f444e94726a5befca90e38aaf5",
            "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        ] {
            assert!(player_id(id).unwrap().is_some());
        }
        for input in [
            "",
            "../Notch",
            "a b",
            "https://example.com",
            "玩家",
            "abcdefghijklmnopq",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert!(player_id(input).is_err(), "{input}");
        }
    }

    #[test]
    fn only_accepts_official_texture_hash_urls_and_upgrades_to_https() {
        let hash = "a".repeat(64);
        let path = format!("textures.minecraft.net/texture/{hash}");
        assert_eq!(
            texture_url(&format!("http://{path}")).unwrap().scheme(),
            "https"
        );
        for url in [
            format!("https://evil.test/texture/{hash}"),
            format!("https://user@{path}"),
            format!("https://{path}?x=1"),
            "https://textures.minecraft.net/texture/short".into(),
        ] {
            assert!(texture_url(&url).is_none(), "{url}");
        }
    }
}
