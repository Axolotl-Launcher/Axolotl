//! Installer for HMCL modpacks.
//!
//! HMCL packs are zips carrying a `modpack.json` with the pack name and game
//! version, optionally with an MCBBS-style `addons` array declaring loaders;
//! bundled content ships in a `minecraft/` folder that maps onto the
//! instance's game directory.

use std::path::PathBuf;

use serde::Deserialize;

use super::archive_util;
use crate::State;
use crate::data::ModLoader;
use crate::install::{
    InstallPhaseDetails, InstallPhaseId, InstallProgressReporter,
};
use crate::pack::detect::HMCL_MANIFEST;
use crate::state::{
    AppliedContentSetPatch, ContentSourceKind, EditInstance,
    InstanceInstallStage, InstanceLink,
};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HmclManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    game_version: Option<String>,
    #[serde(default)]
    addons: Vec<HmclAddon>,
}

#[derive(Deserialize, Debug)]
struct HmclAddon {
    id: String,
    version: String,
}

pub(crate) async fn install_hmcl_pack_with_reporter(
    instance_id: String,
    archive_path: PathBuf,
    base_folder: String,
    source_filename: Option<String>,
    reporter: InstallProgressReporter,
) -> crate::Result<()> {
    let state = State::get().await?;
    let manifest_json = archive_util::read_archive_entry_to_string(
        archive_path.clone(),
        format!("{base_folder}{HMCL_MANIFEST}"),
    )
    .await?;
    let manifest: HmclManifest = serde_json::from_str(&manifest_json)?;

    let mut game_version = manifest
        .game_version
        .clone()
        .filter(|version| !version.trim().is_empty());
    let mut loader = ModLoader::Vanilla;
    let mut loader_version = None;
    let mut optifine_version = None;
    let mut lite_loader_version = None;
    for addon in &manifest.addons {
        match addon.id.to_ascii_lowercase().as_str() {
            "game" => {
                if game_version.is_none() {
                    game_version = Some(addon.version.clone());
                }
            }
            "forge" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::Forge;
                loader_version = Some(addon.version.clone());
            }
            "neoforge" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::NeoForge;
                loader_version = Some(addon.version.clone());
            }
            "fabric" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::Fabric;
                loader_version = Some(addon.version.clone());
            }
            "quilt" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::Quilt;
                loader_version = Some(addon.version.clone());
            }
            "cleanroom" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::Cleanroom;
                loader_version = Some(addon.version.clone());
            }
            "legacy_fabric" | "legacyfabric" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "HMCL modpack declares multiple primary loaders"
                            .to_string(),
                    )
                    .into());
                }
                loader = ModLoader::LegacyFabric;
                loader_version = Some(addon.version.clone());
            }
            "lite_loader" | "liteloader" => {
                lite_loader_version = Some(addon.version.clone());
            }
            "optifine" => optifine_version = Some(addon.version.clone()),
            "labymod" => {
                return Err(crate::ErrorKind::InputError(
                    "Unsupported loader LabyMod: Axolotl does not install, update, or repair LabyMod instances"
                        .to_string(),
                )
                .into());
            }
            other => {
                return Err(crate::ErrorKind::InputError(format!(
                    "Unsupported HMCL loader component {other} {}",
                    addon.version
                ))
                .into());
            }
        }
    }
    let Some(game_version) = game_version else {
        return Err(crate::ErrorKind::InputError(
            "HMCL modpack did not specify a Minecraft version".to_string(),
        )
        .into());
    };

    let mut lite_loader_as_adjunct = None;
    if let Some(lite_loader_version) = lite_loader_version {
        match loader {
            ModLoader::Vanilla => {
                loader = ModLoader::LiteLoader;
                loader_version = Some(lite_loader_version);
            }
            ModLoader::Forge => {
                lite_loader_as_adjunct = Some(lite_loader_version);
            }
            _ => {
                return Err(crate::ErrorKind::InputError(format!(
                    "LiteLoader is not supported with {}",
                    loader.as_str()
                ))
                .into());
            }
        }
    }

    let mut optifine_as_mod = None;
    let mut requires_optifabric = false;
    if let Some(optifine_version) = optifine_version {
        match loader {
            ModLoader::Vanilla => {
                loader = ModLoader::OptiFine;
                loader_version = Some(optifine_version);
            }
            ModLoader::Forge | ModLoader::NeoForge => {
                optifine_as_mod = Some(optifine_version);
            }
            ModLoader::Fabric | ModLoader::LegacyFabric => {
                optifine_as_mod = Some(optifine_version);
                requires_optifabric = true;
            }
            _ => {
                return Err(crate::ErrorKind::InputError(format!(
                    "OptiFine is not supported with {}",
                    loader.as_str()
                ))
                .into());
            }
        }
    }

    let pack_name = manifest
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            source_filename.as_ref().map(|name| {
                std::path::Path::new(name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
        })
        .unwrap_or_else(|| "HMCL Modpack".to_string());
    let pack_details = InstallPhaseDetails::Modpack {
        project_id: None,
        version_id: None,
        title: Some(pack_name.clone()),
    };
    reporter
        .update(InstallPhaseId::ResolvingPack, None, pack_details.clone())
        .await?;

    let lite_loader_as_adjunct = if let Some(requested_version) =
        lite_loader_as_adjunct
    {
        Some(
            crate::launcher::get_loader_version_from_profile(
                &game_version,
                ModLoader::LiteLoader,
                Some(&requested_version),
            )
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "No LiteLoader version {requested_version} supports Minecraft {game_version}"
                ))
            })?,
        )
    } else {
        None
    };
    let optifabric_version = if requires_optifabric {
        Some(
            crate::install::runner::resolve_optifabric_version(&game_version)
                .await?,
        )
    } else {
        None
    };

    let resolved_loader_version = if loader != ModLoader::Vanilla {
        crate::launcher::get_loader_version_from_profile(
            &game_version,
            loader,
            loader_version.as_deref(),
        )
        .await?
    } else {
        None
    };

    crate::api::instance::edit(
        &instance_id,
        EditInstance {
            install_stage: Some(InstanceInstallStage::PackInstalling),
            name: Some(pack_name.clone()),
            link: Some(InstanceLink::ImportedModpack {
                project_id: None,
                version_id: None,
                name: Some(pack_name.clone()),
                version_number: manifest.version.clone(),
                filename: source_filename,
            }),
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::ImportedModpack),
                game_version: Some(game_version.clone()),
                protocol_version: Some(None),
                loader: Some(loader),
                loader_version: Some(
                    resolved_loader_version.map(|version| version.id),
                ),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    let minecraft_install =
        super::parallel_minecraft_install::ParallelMinecraftInstall::start(
            instance_id.clone(),
            reporter.clone(),
        );

    reporter
        .update(
            InstallPhaseId::ExtractingOverrides,
            None,
            pack_details.clone(),
        )
        .await?;
    let instance_path =
        crate::api::instance::get_full_path(&instance_id).await?;
    let archive_cancellation = reporter.cancellation_token();
    let (_, archive_replacements) =
        archive_util::materialize_archive_subdir_for_instance(
            instance_id.clone(),
            archive_cancellation.clone(),
            archive_path,
            format!("{base_folder}minecraft/"),
            instance_path.clone(),
        )
        .await?;

    let post_archive_result: crate::Result<()> = async {
        minecraft_install.join().await?;

        if let Some(lite_loader_version) = lite_loader_as_adjunct {
            super::install_mcbbs::install_liteloader_component(
                &state,
                &instance_id,
                &game_version,
                loader,
                &lite_loader_version,
            )
            .await?;
        }
        if let Some(optifine_version) = optifine_as_mod {
            super::install_mcbbs::install_optifine_mod(
                &state,
                &instance_id,
                reporter.cancellation_token(),
                &game_version,
                &optifine_version,
                &instance_path,
            )
            .await?;
            super::install_mcbbs::record_optifine_component(
                &instance_id,
                &optifine_version,
            )
            .await?;
        }
        if let Some(optifabric_version) = optifabric_version {
            super::install_mcbbs::install_optifabric_component(
                &instance_id,
                &game_version,
                &optifabric_version,
            )
            .await?;
        }

        reporter.clear_context().await?;
        Ok(())
    }
    .await;
    archive_util::settle_staged_archive_install(
        instance_id,
        archive_cancellation,
        archive_replacements,
        post_archive_result,
        "HMCL archive contents",
    )
    .await
}
