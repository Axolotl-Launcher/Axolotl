use std::{io::Cursor, path::Path, sync::Arc, time::Duration};

use reqwest::{Client, StatusCode};
use tokio::{fs::File, io::AsyncReadExt};
use url::Url;

use super::png_util;
use crate::{ErrorKind, util::io};

const MAX_TEXTURE_BYTES: usize = 256 * 1024;

fn texture_key(url: &Url) -> Option<&str> {
    let key = url.path().strip_prefix("/texture/")?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str() == Some("textures.minecraft.net")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && (1..=64).contains(&key.len())
        && key.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(key)
}

fn preview_data_url(bytes: &[u8]) -> Option<Arc<Url>> {
    let (width, height) = png_util::dimensions(bytes).ok()?;
    if !(1..=1024).contains(&width) || !(1..=1024).contains(&height) {
        return None;
    }

    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    reader.next_frame(&mut pixels).ok()?;
    png_util::blob_to_data_url(bytes)
}

async fn download(client: &Client, key: &str) -> crate::Result<Vec<u8>> {
    let mut response = client
        .get(format!("https://textures.minecraft.net/texture/{key}"))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    if response.status() != StatusCode::OK
        || response
            .content_length()
            .is_some_and(|size| size > MAX_TEXTURE_BYTES as u64)
    {
        return Err(ErrorKind::InvalidPng.into());
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_TEXTURE_BYTES {
            return Err(ErrorKind::InvalidPng.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) async fn load(
    cache_dir: &Path,
    client: Option<&Client>,
    url: &Url,
) -> Option<Arc<Url>> {
    let key = texture_key(url)?;
    let path = cache_dir.join(format!("{key}.png"));
    if let Ok(file) = File::open(&path).await {
        let mut bytes = Vec::new();
        if file
            .take((MAX_TEXTURE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .is_ok()
            && bytes.len() <= MAX_TEXTURE_BYTES
            && let Some(texture) = preview_data_url(&bytes)
        {
            return Some(texture);
        }
    }

    let client = client?;
    let bytes = match download(client, key).await {
        Ok(bytes) => bytes,
        Err(_) => {
            tracing::warn!("Could not download Minecraft cape preview texture");
            return None;
        }
    };
    let texture = preview_data_url(&bytes)?;
    if let Err(error) = async {
        io::create_dir_all(cache_dir).await?;
        io::write(&path, &bytes).await
    }
    .await
    {
        tracing::warn!(%error, "Could not cache Minecraft cape preview texture");
    }
    Some(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPE_URL: &str = "http://textures.minecraft.net/texture/9e507afc56359978a3eb3e32367042b853cddd0995d17d0da995662913fb00f7";

    #[test]
    fn only_accepts_mojang_texture_keys() {
        assert!(texture_key(&Url::parse(CAPE_URL).unwrap()).is_some());
        for url in [
            "http://textures.minecraft.net.example.org/texture/abcd",
            "http://user@textures.minecraft.net/texture/abcd",
            "http://textures.minecraft.net:8080/texture/abcd",
            "https://textures.minecraft.net/texture/abcd?token=bad",
            "https://textures.minecraft.net/texture/%2fabcd",
        ] {
            assert_eq!(texture_key(&Url::parse(url).unwrap()), None, "{url}");
        }
    }

    #[tokio::test]
    async fn cache_only_reads_valid_textures_without_a_client() {
        let directory = tempfile::tempdir().unwrap();
        let url = Url::parse(CAPE_URL).unwrap();
        let key = texture_key(&url).unwrap();
        let path = directory.path().join(format!("{key}.png"));
        assert!(load(directory.path(), None, &url).await.is_none());

        let bytes = include_bytes!("assets/test/notch_normalized.png");
        tokio::fs::write(&path, &bytes[..32]).await.unwrap();
        assert!(load(directory.path(), None, &url).await.is_none());

        tokio::fs::write(&path, bytes).await.unwrap();
        assert_eq!(
            load(directory.path(), None, &url).await,
            png_util::blob_to_data_url(bytes),
        );
    }
}
