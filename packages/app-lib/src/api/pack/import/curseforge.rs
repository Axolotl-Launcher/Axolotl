use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    prelude::ModLoader,
    state::{AppliedContentSetPatch, EditInstance, InstanceInstallStage},
    util::{
        fetch::{fetch, write_cached_icon},
        io,
    },
};

use super::{finish_import, instance_json, recache_icon};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstance {
    pub name: Option<String>,
    pub base_mod_loader: Option<MinecraftInstanceModLoader>,
    pub profile_image_path: Option<PathBuf>,
    pub installed_modpack: Option<InstalledModpack>,
    pub game_version: String, // Minecraft game version. Non-prioritized, use this if Vanilla
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstanceModLoader {
    pub name: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModpack {
    pub thumbnail_url: Option<String>,
}

fn parse_curseforge_loader(
    loader_name: &str,
    game_version: &str,
) -> Option<(ModLoader, String)> {
    let loader_name = loader_name.trim();
    if loader_name.eq_ignore_ascii_case("labymod")
        || loader_name.to_ascii_lowercase().starts_with("labymod-")
    {
        return None;
    }
    let family = crate::api::curseforge::loader_family(loader_name);
    let loader = match family {
        "forge" => ModLoader::Forge,
        "fabric" => ModLoader::Fabric,
        "quilt" => ModLoader::Quilt,
        "neo" | "neoforge" => ModLoader::NeoForge,
        _ => return None,
    };
    let detected_version =
        loader_name.strip_prefix(family)?.strip_prefix('-')?.trim();
    if detected_version.is_empty() {
        return None;
    }
    let version = instance_json::normalize_imported_loader_version(
        loader.as_str(),
        game_version,
        detected_version,
    );
    (!version.is_empty()).then_some((loader, version))
}

// Check if folder has a minecraftinstance.json that parses
pub async fn is_valid_curseforge(instance_folder: PathBuf) -> bool {
    let minecraft_instance = serde_json::from_str::<MinecraftInstance>(
        &io::read_any_encoding_to_string(
            &instance_folder.join("minecraftinstance.json"),
        )
        .await
        .unwrap_or(("".into(), encoding_rs::UTF_8))
        .0,
    );
    minecraft_instance.is_ok()
}

pub async fn import_curseforge(
    curseforge_instance_folder: PathBuf, // instance's folder
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
) -> crate::Result<()> {
    // Load minecraftinstance.json
    let minecraft_instance = serde_json::from_str::<MinecraftInstance>(
        &io::read_any_encoding_to_string(
            &curseforge_instance_folder.join("minecraftinstance.json"),
        )
        .await
        .unwrap_or(("".into(), encoding_rs::UTF_8))
        .0,
    )?;
    let override_title = minecraft_instance.name;
    let backup_name = format!(
        "Curseforge-{}",
        curseforge_instance_folder
            .file_name()
            .map_or("Unknown".to_string(), |a| a.to_string_lossy().to_string())
    );

    let state = State::get().await?;
    // Recache Curseforge Icon if it exists
    let mut icon = None;

    if let Some(icon_path) = minecraft_instance.profile_image_path.clone() {
        icon = recache_icon(icon_path).await?;
    } else if let Some(InstalledModpack {
        thumbnail_url: Some(thumbnail_url),
    }) = minecraft_instance.installed_modpack.clone()
    {
        let icon_bytes =
            fetch(&thumbnail_url, None, None, None, &state.fetch_semaphore)
                .await?;
        let filename = thumbnail_url.rsplit('/').next_back();
        if let Some(filename) = filename {
            icon = Some(
                write_cached_icon(
                    filename,
                    &state.directories.caches_dir(),
                    icon_bytes,
                    &state.io_semaphore,
                )
                .await?,
            );
        }
    }

    // base mod loader is always None for vanilla
    if let Some(instance_mod_loader) = minecraft_instance.base_mod_loader {
        let game_version = minecraft_instance.game_version;

        let parsed_loader =
            parse_curseforge_loader(&instance_mod_loader.name, &game_version);
        let (mod_loader, requested_loader_version) = parsed_loader.ok_or_else(|| {
            let loader_name = instance_mod_loader.name.trim();
            let message = if loader_name.eq_ignore_ascii_case("labymod")
                || loader_name.to_ascii_lowercase().starts_with("labymod-")
            {
                "Unsupported loader LabyMod: Axolotl does not install, update, or repair LabyMod instances".to_string()
            } else {
                format!(
                    "Unsupported loader {loader_name}: the instance was not imported as Vanilla"
                )
            };
            crate::ErrorKind::InputError(message)
        })?;

        let loader_version = crate::launcher::get_loader_version_from_profile(
            &game_version,
            mod_loader,
            Some(&requested_loader_version),
        )
        .await?;
        if loader_version.is_none() {
            return Err(crate::ErrorKind::InputError(format!(
                "CurseForge instance loader version {requested_loader_version} is not available for {} {game_version}",
                mod_loader.as_str(),
            ))
            .into());
        }

        crate::api::instance::edit(
            instance_id,
            EditInstance {
                install_stage: Some(InstanceInstallStage::PackInstalling),
                name: Some(
                    override_title
                        .clone()
                        .unwrap_or_else(|| backup_name.to_string()),
                ),
                icon_path: Some(
                    icon.clone().map(|x| x.to_string_lossy().to_string()),
                ),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: None,
                    game_version: Some(game_version.clone()),
                    protocol_version: Some(None),
                    loader: Some(mod_loader),
                    loader_version: Some(loader_version.clone().map(|x| x.id)),
                }),
                ..EditInstance::default()
            },
        )
        .await?;
    } else {
        crate::api::instance::edit(
            instance_id,
            EditInstance {
                name: Some(
                    override_title
                        .clone()
                        .unwrap_or_else(|| backup_name.to_string()),
                ),
                icon_path: Some(
                    icon.clone().map(|x| x.to_string_lossy().to_string()),
                ),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: None,
                    game_version: Some(minecraft_instance.game_version.clone()),
                    protocol_version: Some(None),
                    loader: Some(ModLoader::Vanilla),
                    loader_version: Some(None),
                }),
                ..EditInstance::default()
            },
        )
        .await?;
    }

    // Copy in contained folders as overrides
    let state = State::get().await?;
    finish_import(
        instance_id,
        curseforge_instance_folder,
        &state.io_semaphore,
        reporter,
        details,
        symlink,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_curseforge_instance_loaders() {
        for (name, game_version, loader, version) in [
            ("forge-47.4.22", "1.20.1", ModLoader::Forge, "47.4.22"),
            (
                "fabric-0.16.10-1.21.1",
                "1.21.1",
                ModLoader::Fabric,
                "0.16.10",
            ),
            ("quilt-0.26.4-1.20.1", "1.20.1", ModLoader::Quilt, "0.26.4"),
            (
                "neoforge-21.4.157",
                "1.21.4",
                ModLoader::NeoForge,
                "21.4.157",
            ),
        ] {
            assert_eq!(
                parse_curseforge_loader(name, game_version),
                Some((loader, version.to_string())),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_unknown_curseforge_instance_loader() {
        assert_eq!(parse_curseforge_loader("unknown-1.0", "1.20.1"), None);
    }

    #[test]
    fn rejects_labymod_curseforge_instance_loader() {
        assert_eq!(parse_curseforge_loader("labymod-4.4.20", "1.20.1"), None);
    }
}
