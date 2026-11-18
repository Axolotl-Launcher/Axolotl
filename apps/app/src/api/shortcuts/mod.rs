use crate::api::Result;
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Error as IoError},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, Runtime};
use url::Url;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux::{SHORTCUT_EXTENSION, SHORTCUT_ICON_EXTENSION, create_shortcut};
#[cfg(target_os = "macos")]
use macos::{SHORTCUT_EXTENSION, SHORTCUT_ICON_EXTENSION, create_shortcut};
#[cfg(target_os = "windows")]
use windows::{
    SHORTCUT_EXTENSION, SHORTCUT_ICON_EXTENSION, create_shortcut,
    refresh_shortcut_icon_cache,
};

const DEFAULT_SHORTCUT_ICON: &[u8] = include_bytes!("../../../icons/icon.png");

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("shortcuts")
        .invoke_handler(tauri::generate_handler![create_instance_shortcut])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn create_instance_shortcut<R: Runtime>(
    app: AppHandle<R>,
    instance_name: String,
    instance_id: String,
    output_path: Option<PathBuf>,
    server: Option<String>,
    singleplayer_world: Option<String>,
) -> Result<PathBuf> {
    if server.is_some() && singleplayer_world.is_some() {
        return Err(std::io::Error::other(
            "shortcut cannot launch both a server and a singleplayer world",
        )
        .into());
    }

    let launch_url =
        instance_launch_url(&instance_id, server, singleplayer_world);

    let output_path = match output_path {
        Some(output_path) => output_path,
        None => app.path().desktop_dir()?.join(format!(
            "{}.{}",
            desktop_shortcut_name(&instance_name),
            SHORTCUT_EXTENSION
        )),
    };
    let output_path = shortcut_path_with_extension(output_path);
    let output_path_existed =
        tokio::fs::try_exists(&output_path).await.unwrap_or(false);
    let instance =
        theseus::instance::get(&instance_id).await?.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("unknown instance: {instance_id}"),
            )
        })?;
    let icon_source = instance.instance.icon_path.as_deref().map(Path::new);
    let shortcut_icon =
        sync_instance_shortcut_icon(&app, &instance_id, icon_source, false)
            .await?;

    if let Err(error) = create_shortcut(
        &instance_name,
        &launch_url,
        &output_path,
        shortcut_icon.as_deref(),
        &app.config().identifier,
    )
    .await
    {
        cleanup_shortcut_artifact(&output_path, output_path_existed).await;
        return Err(error);
    }

    Ok(output_path)
}

pub(crate) async fn sync_instance_shortcut_icon<R: Runtime>(
    app: &AppHandle<R>,
    instance_id: &str,
    source_path: Option<&Path>,
    only_if_exists: bool,
) -> Result<Option<PathBuf>> {
    let icon_path = shortcut_icon_path(app, instance_id)?;
    if only_if_exists && !tokio::fs::try_exists(&icon_path).await? {
        return Ok(None);
    }

    let source = match source_path {
        Some(source_path) => match tokio::fs::read(source_path).await {
            Ok(source) => source,
            Err(error) => {
                tracing::warn!(
                    "failed to read instance shortcut icon {}: {}; using the launcher icon",
                    source_path.display(),
                    error
                );
                DEFAULT_SHORTCUT_ICON.to_vec()
            }
        },
        None => DEFAULT_SHORTCUT_ICON.to_vec(),
    };

    let encoded = tokio::task::spawn_blocking(move || {
        encode_shortcut_icon(&source).or_else(|source_error| {
            tracing::warn!(
                "failed to encode instance shortcut icon: {}; using the launcher icon",
                source_error
            );
            encode_shortcut_icon(DEFAULT_SHORTCUT_ICON)
        })
    })
    .await
    .map_err(|error| {
        IoError::other(format!(
            "failed to join shortcut icon encoding task: {error}"
        ))
    })??;

    if let Some(parent) = icon_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&icon_path, encoded).await?;

    #[cfg(target_os = "windows")]
    refresh_shortcut_icon_cache(&icon_path);

    Ok(Some(icon_path))
}

fn shortcut_icon_path<R: Runtime>(
    app: &AppHandle<R>,
    instance_id: &str,
) -> Result<PathBuf> {
    let digest = Sha256::digest(instance_id.as_bytes());
    Ok(app
        .path()
        .app_data_dir()?
        .join("instance-shortcut-icons")
        .join(format!("{digest:x}.{SHORTCUT_ICON_EXTENSION}")))
}

fn decode_shortcut_icon(source: &[u8]) -> std::io::Result<DynamicImage> {
    image::load_from_memory(source).map_err(IoError::other)
}

fn encode_png(icon: &DynamicImage, size: u32) -> std::io::Result<Vec<u8>> {
    let icon = icon.resize_exact(size, size, FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    icon.write_to(&mut output, ImageFormat::Png)
        .map_err(IoError::other)?;
    Ok(output.into_inner())
}

#[cfg(target_os = "linux")]
fn encode_shortcut_icon(source: &[u8]) -> std::io::Result<Vec<u8>> {
    encode_png(&decode_shortcut_icon(source)?, 256)
}

#[cfg(target_os = "windows")]
fn encode_shortcut_icon(source: &[u8]) -> std::io::Result<Vec<u8>> {
    let png = encode_png(&decode_shortcut_icon(source)?, 256)?;
    let mut output = Vec::with_capacity(22 + png.len());

    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&[0, 0, 0, 0]);
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&32_u16.to_le_bytes());
    output.extend_from_slice(&(png.len() as u32).to_le_bytes());
    output.extend_from_slice(&22_u32.to_le_bytes());
    output.extend_from_slice(&png);

    Ok(output)
}

#[cfg(target_os = "macos")]
fn encode_shortcut_icon(source: &[u8]) -> std::io::Result<Vec<u8>> {
    let icon = decode_shortcut_icon(source)?;
    let chunks = [
        (*b"ic07", encode_png(&icon, 128)?),
        (*b"ic08", encode_png(&icon, 256)?),
        (*b"ic09", encode_png(&icon, 512)?),
        (*b"ic10", encode_png(&icon, 1024)?),
    ];
    let total_length = 8_usize
        + chunks
            .iter()
            .map(|(_, data)| 8_usize + data.len())
            .sum::<usize>();
    let mut output = Vec::with_capacity(total_length);
    output.extend_from_slice(b"icns");
    output.extend_from_slice(&(total_length as u32).to_be_bytes());
    for (chunk_type, data) in chunks {
        output.extend_from_slice(&chunk_type);
        output.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        output.extend_from_slice(&data);
    }

    Ok(output)
}

fn instance_launch_url(
    instance_id: &str,
    server: Option<String>,
    singleplayer_world: Option<String>,
) -> Url {
    let mut launch_url =
        Url::parse("axolotl://launch").expect("static launch URL should parse");

    launch_url
        .query_pairs_mut()
        .append_pair("instance_id", instance_id);

    if let Some(server) = server {
        launch_url.query_pairs_mut().append_pair("server", &server);
    } else if let Some(singleplayer_world) = singleplayer_world {
        launch_url
            .query_pairs_mut()
            .append_pair("singleplayer_world", &singleplayer_world);
    }

    launch_url
}

fn desktop_shortcut_name(instance_name: &str) -> String {
    let sanitized: String = instance_name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim().trim_end_matches(['.', ' ']);
    let instance_name = if sanitized.is_empty() {
        "Instance"
    } else {
        sanitized
    };

    format!("{} - {instance_name}", theseus::brand::SHORT_PRODUCT_NAME)
}

fn shortcut_path_with_extension(mut path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_none_or(|current_extension| current_extension != SHORTCUT_EXTENSION)
    {
        path.set_extension(SHORTCUT_EXTENSION);
    }

    path
}

async fn cleanup_shortcut_artifact(path: &Path, existed: bool) {
    if existed {
        return;
    }

    let result = match tokio::fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => {
            tokio::fs::remove_dir_all(path).await
        }
        _ => tokio::fs::remove_file(path).await,
    };

    if let Err(error) = result
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            "failed to clean up shortcut artifact {}: {}",
            path.display(),
            error
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_url_uses_the_registered_scheme_and_encodes_values() {
        let url = instance_launch_url(
            "instance id/测试",
            Some("example.org:25565".to_string()),
            None,
        );

        assert_eq!(
            url.as_str(),
            "axolotl://launch?instance_id=instance+id%2F%E6%B5%8B%E8%AF%95&server=example.org%3A25565"
        );
    }

    #[test]
    fn desktop_shortcut_name_is_safe_on_all_supported_platforms() {
        assert_eq!(
            desktop_shortcut_name("My: Instance?/ "),
            "Axolotl - My_ Instance__"
        );
        assert_eq!(desktop_shortcut_name(" ... "), "Axolotl - Instance");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_shortcut_icon_contains_a_png_image() {
        let icon = encode_shortcut_icon(DEFAULT_SHORTCUT_ICON).unwrap();

        assert_eq!(&icon[0..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(u32::from_le_bytes(icon[18..22].try_into().unwrap()), 22);
        assert_eq!(&icon[22..30], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_le_bytes(icon[14..18].try_into().unwrap()) as usize,
            icon.len() - 22
        );
    }
}
