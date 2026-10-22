use std::{
    io::{Cursor, Read, Write},
    path::Path,
};

use serde_json::json;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    ErrorKind, State,
    state::minecraft_skins::{CustomMinecraftSkin, OfflineMinecraftSkin},
};

use super::{Credentials, png_util};

pub(crate) const OFFLINE_SKIN_PACK_FILE_NAME: &str = "Axolotl Offline Skin.zip";
pub(crate) const OFFLINE_SKIN_PACK_LEGACY_ID: &str = "Axolotl Offline Skin.zip";
pub(crate) const OFFLINE_SKIN_PACK_MODERN_ID: &str =
    "file/Axolotl Offline Skin.zip";

#[derive(Debug, Clone, Copy)]
pub(crate) struct OfflineSkinPackOptions {
    pub enabled_pack_id: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MinecraftReleaseVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl MinecraftReleaseVersion {
    fn parse(version: &str) -> Option<Self> {
        let mut parts = version.split('.');
        let major = leading_number(parts.next()?)?;
        let minor = leading_number(parts.next()?)?;
        let patch = parts.next().and_then(leading_number).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
        })
    }

    fn supports_resource_packs(self) -> bool {
        self.major > 1 || self.minor >= 6
    }

    fn uses_modern_pack_id(self) -> bool {
        self.major > 1 || self.minor >= 13
    }

    fn uses_modern_player_texture_paths(self) -> bool {
        self.major > 1
            || self.minor > 19
            || (self.minor == 19 && self.patch >= 3)
    }

    fn needs_legacy_skin_height(self) -> bool {
        self.major == 1 && matches!(self.minor, 6 | 7)
    }

    fn resource_pack_format(self) -> u32 {
        if self.major > 1 {
            return match self.minor {
                0..=1 => 84,
                2 => 88,
                _ => 97,
            };
        }

        match self.minor {
            0..=8 => 1,
            9..=10 => 2,
            11..=12 => 3,
            13..=14 => 4,
            15 => 5,
            16 if self.patch <= 1 => 5,
            16 => 6,
            17 => 7,
            18 => 8,
            19 if self.patch <= 2 => 9,
            19 if self.patch == 3 => 12,
            19 => 13,
            20 if self.patch <= 1 => 15,
            20 if self.patch == 2 => 18,
            20 if self.patch <= 4 => 22,
            20 => 32,
            21 if self.patch <= 1 => 34,
            21 if self.patch <= 3 => 42,
            21 if self.patch == 4 => 46,
            21 if self.patch == 5 => 55,
            21 if self.patch == 6 => 63,
            21 if self.patch <= 8 => 64,
            21 if self.patch <= 10 => 69,
            21 => 75,
            _ => 75,
        }
    }
}

fn leading_number(value: &str) -> Option<u32> {
    let digits = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

/// Builds (or removes) the resource pack that replaces vanilla's local default
/// player texture with the selected offline skin.
pub(crate) async fn prepare_offline_skin_resource_pack(
    credentials: &Credentials,
    instance_path: &Path,
    game_version: &str,
    client_jar: &Path,
) -> crate::Result<OfflineSkinPackOptions> {
    let resource_pack_dir = instance_path.join("resourcepacks");
    let resource_pack_path =
        resource_pack_dir.join(OFFLINE_SKIN_PACK_FILE_NAME);
    let Some(version) = MinecraftReleaseVersion::parse(game_version) else {
        remove_pack_if_present(&resource_pack_path).await?;
        return Ok(OfflineSkinPackOptions {
            enabled_pack_id: None,
        });
    };

    if !credentials.is_offline() || !version.supports_resource_packs() {
        remove_pack_if_present(&resource_pack_path).await?;
        return Ok(OfflineSkinPackOptions {
            enabled_pack_id: None,
        });
    }

    let state = State::get().await?;
    let Some(offline_skin) =
        OfflineMinecraftSkin::get(credentials.offline_profile.id, &state.pool)
            .await?
    else {
        remove_pack_if_present(&resource_pack_path).await?;
        return Ok(OfflineSkinPackOptions {
            enabled_pack_id: None,
        });
    };

    let Some(saved_skin) = CustomMinecraftSkin::get_by_texture(
        credentials.offline_profile.id,
        &offline_skin.texture_key,
        &state.pool,
    )
    .await?
    else {
        OfflineMinecraftSkin::clear(
            credentials.offline_profile.id,
            &state.pool,
        )
        .await?;
        remove_pack_if_present(&resource_pack_path).await?;
        return Ok(OfflineSkinPackOptions {
            enabled_pack_id: None,
        });
    };

    let mut texture = saved_skin.texture_blob(&state.pool).await?;
    if version.needs_legacy_skin_height()
        && png_util::dimensions(&texture)?.1 == 64
    {
        texture = png_util::to_legacy_client_texture(&texture)?.to_vec();
    }

    let client_jar = client_jar.to_path_buf();
    let target_version = game_version.to_owned();
    let format = tokio::task::spawn_blocking(move || {
        read_pack_format_from_jar(&client_jar, &target_version)
    })
    .await
    .ok()
    .flatten();
    let zip_data = build_resource_pack(&texture, version, format)?;
    tokio::fs::create_dir_all(&resource_pack_dir).await?;
    tokio::fs::write(&resource_pack_path, zip_data).await?;

    Ok(OfflineSkinPackOptions {
        enabled_pack_id: Some(if version.uses_modern_pack_id() {
            OFFLINE_SKIN_PACK_MODERN_ID
        } else {
            OFFLINE_SKIN_PACK_LEGACY_ID
        }),
    })
}

async fn remove_pack_if_present(path: &Path) -> crate::Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn read_pack_format_from_jar(
    path: &Path,
    target_version: &str,
) -> Option<[u32; 2]> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let entry = archive.by_name("version.json").ok()?;
    let metadata: serde_json::Value =
        serde_json::from_reader(entry.take(65_536)).ok()?;
    if metadata.get("id")?.as_str()? != target_version {
        return None;
    }
    let pack = metadata.get("pack_version")?;
    let major = pack
        .get("resource_major")
        .or_else(|| pack.get("resource"))?
        .as_u64()?;
    let minor = pack
        .get("resource_minor")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    Some([u32::try_from(major).ok()?, u32::try_from(minor).ok()?])
}

fn build_resource_pack(
    texture: &[u8],
    version: MinecraftReleaseVersion,
    format: Option<[u32; 2]>,
) -> crate::Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    let pack_format = format
        .map_or_else(|| version.resource_pack_format(), |format| format[0]);
    let mut metadata = json!({
        "pack": {
            "pack_format": pack_format,
            "description": "Axolotl Launcher offline skin"
        }
    });

    if version.major >= 26 || (version.minor == 21 && version.patch >= 9) {
        let minor = format.map_or_else(
            || u32::from(version.major == 26 && version.minor >= 3),
            |format| format[1],
        );
        let pack = metadata["pack"].as_object_mut().unwrap();
        pack.remove("pack_format");
        pack.insert("min_format".into(), json!([pack_format, minor]));
        pack.insert("max_format".into(), json!([pack_format, minor]));
    }

    start_zip_file(&mut zip, "pack.mcmeta", options)?;
    zip.write_all(&serde_json::to_vec(&metadata)?)?;

    if version.uses_modern_player_texture_paths() {
        for model in ["slim", "wide"] {
            for skin_name in [
                "alex", "ari", "efe", "kai", "makena", "noor", "steve",
                "sunny", "zuri",
            ] {
                let path = format!(
                    "assets/minecraft/textures/entity/player/{model}/{skin_name}.png"
                );
                start_zip_file(&mut zip, &path, options)?;
                zip.write_all(texture)?;
            }
        }
    } else {
        // Replace both models. This keeps the standard offline UUID stable, so
        // changing a skin cannot split single-player saves into a new player.
        for path in [
            "assets/minecraft/textures/entity/steve.png",
            "assets/minecraft/textures/entity/alex.png",
        ] {
            start_zip_file(&mut zip, path, options)?;
            zip.write_all(texture)?;
        }
    }

    zip.finish().map(Cursor::into_inner).map_err(zip_error)
}

fn start_zip_file(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    path: &str,
    options: SimpleFileOptions,
) -> crate::Result<()> {
    zip.start_file(path, options).map_err(zip_error)
}

fn zip_error(error: zip::result::ZipError) -> crate::Error {
    ErrorKind::OtherError(format!(
        "Failed to build offline skin resource pack: {error}"
    ))
    .as_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_versions_and_selects_pack_formats() {
        let version = MinecraftReleaseVersion::parse("1.19.2").unwrap();
        assert_eq!(version.resource_pack_format(), 9);
        assert!(!version.uses_modern_player_texture_paths());

        let version = MinecraftReleaseVersion::parse("1.19.3").unwrap();
        assert_eq!(version.resource_pack_format(), 12);
        assert!(version.uses_modern_player_texture_paths());

        let version = MinecraftReleaseVersion::parse("1.21.8").unwrap();
        assert_eq!(version.resource_pack_format(), 64);
        assert!(version.uses_modern_pack_id());
    }

    #[test]
    fn generated_packs_use_release_specific_modern_formats() {
        for (release, format) in
            [("26.1", [84, 0]), ("26.2", [88, 0]), ("26.3", [97, 1])]
        {
            let pack = build_resource_pack(
                b"texture",
                MinecraftReleaseVersion::parse(release).unwrap(),
                None,
            )
            .unwrap();
            let mut archive = zip::ZipArchive::new(Cursor::new(pack)).unwrap();
            let mut entry = archive.by_name("pack.mcmeta").unwrap();
            let metadata: serde_json::Value =
                serde_json::from_reader(&mut entry).unwrap();
            assert_eq!(metadata["pack"]["min_format"], json!(format));
            assert_eq!(metadata["pack"]["max_format"], json!(format));
            assert!(metadata["pack"].get("pack_format").is_none());
        }
    }

    #[test]
    fn snapshot_pack_format_comes_from_the_actual_client_jar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("client.jar");
        let mut zip = ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file("version.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"26.3-snapshot-6","pack_version":{"resource_major":94,"resource_minor":0}}"#).unwrap();
        zip.finish().unwrap();
        let format = read_pack_format_from_jar(&path, "26.3-snapshot-6");
        assert_eq!(format, Some([94, 0]));
        assert_eq!(read_pack_format_from_jar(&path, "26.3"), None);
        let pack = build_resource_pack(
            b"texture",
            MinecraftReleaseVersion::parse("26.3-snapshot-6").unwrap(),
            format,
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(pack)).unwrap();
        let metadata: serde_json::Value =
            serde_json::from_reader(archive.by_name("pack.mcmeta").unwrap())
                .unwrap();
        assert_eq!(metadata["pack"]["min_format"], json!([94, 0]));
        assert_eq!(metadata["pack"]["max_format"], json!([94, 0]));
    }

    #[test]
    fn rejects_snapshot_versions_without_a_release_shape() {
        assert_eq!(MinecraftReleaseVersion::parse("25w31a"), None);
    }

    #[test]
    fn generated_pack_replaces_all_modern_default_player_textures() {
        let texture = include_bytes!("assets/default/MissingNo.png");
        let version = MinecraftReleaseVersion::parse("1.19.3").unwrap();
        let pack = build_resource_pack(texture, version, None).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(pack)).unwrap();

        assert!(archive.by_name("pack.mcmeta").is_ok());
        assert!(
            archive
                .by_name(
                    "assets/minecraft/textures/entity/player/slim/alex.png"
                )
                .is_ok()
        );
        assert!(
            archive
                .by_name(
                    "assets/minecraft/textures/entity/player/wide/zuri.png"
                )
                .is_ok()
        );
        assert_eq!(archive.len(), 19);
    }
}
