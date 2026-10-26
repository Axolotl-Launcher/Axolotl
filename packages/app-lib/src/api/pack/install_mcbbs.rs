//! Installer for MCBBS modpacks.
//!
//! MCBBS packs are zips carrying either an `mcbbs.packmeta` file or a
//! `manifest.json` with an `addons` array. Game and loader versions come from
//! the addons list, bundled content ships in `overrides/`, and optional
//! launch settings come from `launchInfo`.

use std::path::PathBuf;

use serde::Deserialize;

use super::archive_util;
use crate::State;
use crate::data::ModLoader;
use crate::install::{
    InstallPhaseDetails, InstallPhaseId, InstallProgressReporter,
};
use crate::pack::detect::{CURSEFORGE_MANIFEST, MCBBS_MANIFEST};
use crate::state::{
    AppliedContentSetPatch, ContentSourceKind, EditInstance,
    InstanceInstallStage, InstanceLaunchOverridesPatch, InstanceLink,
};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct McbbsManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    addons: Vec<McbbsAddon>,
    #[serde(default)]
    files: Vec<McbbsFile>,
    #[serde(default)]
    launch_info: Option<McbbsLaunchInfo>,
}

#[derive(Deserialize, Debug)]
struct McbbsAddon {
    id: String,
    version: String,
}

/// A `files` entry; `curse` entries carry CurseForge project/file ids while
/// `addition` entries ship inside the overrides folder and need no download.
#[derive(Deserialize, Debug)]
struct McbbsFile {
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default, alias = "projectID", alias = "projectId")]
    project_id: Option<u32>,
    #[serde(default, alias = "fileID", alias = "fileId")]
    file_id: Option<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct McbbsLaunchInfo {
    #[serde(default)]
    java_argument: Option<serde_json::Value>,
}

fn join_arguments(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(value) => vec![value.clone()],
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) async fn install_mcbbs_pack_with_reporter(
    instance_id: String,
    archive_path: PathBuf,
    base_folder: String,
    source_filename: Option<String>,
    reporter: InstallProgressReporter,
) -> crate::Result<()> {
    let state = State::get().await?;

    let manifest_json = match archive_util::read_archive_entry_to_string(
        archive_path.clone(),
        format!("{base_folder}{MCBBS_MANIFEST}"),
    )
    .await
    {
        Ok(contents) => contents,
        Err(_) => {
            archive_util::read_archive_entry_to_string(
                archive_path.clone(),
                format!("{base_folder}{CURSEFORGE_MANIFEST}"),
            )
            .await?
        }
    };
    let manifest: McbbsManifest = serde_json::from_str(&manifest_json)?;

    let mut game_version = None;
    let mut loader = ModLoader::Vanilla;
    let mut loader_version = None;
    let mut optifine_version = None;
    let mut lite_loader_version = None;
    for addon in &manifest.addons {
        match addon.id.to_ascii_lowercase().as_str() {
            "game" => game_version = Some(addon.version.clone()),
            "forge" => {
                if loader != ModLoader::Vanilla {
                    return Err(crate::ErrorKind::InputError(
                        "MCBBS modpack declares multiple primary loaders"
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
                        "MCBBS modpack declares multiple primary loaders"
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
                        "MCBBS modpack declares multiple primary loaders"
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
                        "MCBBS modpack declares multiple primary loaders"
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
                        "MCBBS modpack declares multiple primary loaders"
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
                        "MCBBS modpack declares multiple primary loaders"
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
                    "Unsupported MCBBS loader component {other} {}",
                    addon.version
                ))
                .into());
            }
        }
    }
    let Some(game_version) = game_version else {
        return Err(crate::ErrorKind::InputError(
            "MCBBS modpack did not specify a Minecraft version".to_string(),
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
        .unwrap_or_else(|| "MCBBS Modpack".to_string());
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

    let launch_overrides =
        manifest.launch_info.as_ref().and_then(|launch_info| {
            let jvm_args = launch_info
                .java_argument
                .as_ref()
                .map(join_arguments)
                .filter(|args| !args.is_empty())?;
            Some(InstanceLaunchOverridesPatch {
                extra_launch_args: Some(Some(jvm_args)),
                ..InstanceLaunchOverridesPatch::default()
            })
        });

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
            launch_overrides,
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

    let curse_files = manifest
        .files
        .iter()
        .filter(|file| {
            file.type_
                .as_deref()
                .is_none_or(|kind| kind.eq_ignore_ascii_case("curse"))
        })
        .filter_map(|file| {
            Some(crate::api::curseforge::CurseForgeManifestFile {
                project_id: file.project_id?,
                file_id: file.file_id?,
                required: true,
            })
        })
        .collect::<Vec<_>>();
    if !curse_files.is_empty() {
        let content_loader = (loader != ModLoader::Vanilla
            && loader != ModLoader::OptiFine)
            .then(|| loader.as_str().to_string());
        crate::api::curseforge::install_local_manifest_files(
            &instance_id,
            curse_files,
            false,
            &game_version,
            content_loader.as_deref(),
            pack_details.clone(),
            &reporter,
        )
        .await?;
    }

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
            format!("{base_folder}overrides/"),
            instance_path.clone(),
        )
        .await?;

    let post_archive_result: crate::Result<()> = async {
        minecraft_install.join().await?;

        if let Some(lite_loader_version) = lite_loader_as_adjunct {
            install_liteloader_component(
                &state,
                &instance_id,
                &game_version,
                loader,
                &lite_loader_version,
            )
            .await?;
        }
        if let Some(optifine_version) = optifine_as_mod {
            install_optifine_mod(
                &state,
                &instance_id,
                reporter.cancellation_token(),
                &game_version,
                &optifine_version,
                &instance_path,
            )
            .await?;
            record_optifine_component(&instance_id, &optifine_version).await?;
        }
        if let Some(optifabric_version) = optifabric_version {
            install_optifabric_component(
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
        "MCBBS overrides",
    )
    .await
}

pub(crate) async fn record_optifine_component(
    instance_id: &str,
    version: &str,
) -> crate::Result<()> {
    record_loader_component(
        instance_id,
        crate::state::LoaderComponentKind::OptiFine,
        version,
        Some(serde_json::json!({ "source": "pack" })),
    )
    .await
}

pub(crate) async fn record_loader_component(
    instance_id: &str,
    kind: crate::state::LoaderComponentKind,
    version: &str,
    provider_metadata: Option<serde_json::Value>,
) -> crate::Result<()> {
    let state = State::get().await?;
    let metadata =
        crate::api::instance::get(instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let mut components = metadata.loader_components;
    components.retain(|component| component.kind != kind);
    components.push(crate::state::LoaderComponent {
        instance_id: instance_id.to_string(),
        kind,
        version: Some(version.to_string()),
        role: crate::state::LoaderComponentRole::Adjunct,
        provider_metadata,
    });
    crate::state::instances::commands::replace_instance_loader_components(
        instance_id,
        &components,
        &state.pool,
    )
    .await
}

pub(crate) async fn install_liteloader_component(
    state: &State,
    instance_id: &str,
    game_version: &str,
    primary_loader: ModLoader,
    resolved_version: &daedalus::modded::LoaderVersion,
) -> crate::Result<()> {
    let metadata =
        crate::api::instance::get(instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let version = crate::install::runner::install_liteloader_adjunct_resolved(
        state,
        &metadata,
        game_version,
        primary_loader,
        resolved_version,
    )
    .await?;
    record_loader_component(
        instance_id,
        crate::state::LoaderComponentKind::LiteLoader,
        &version,
        Some(serde_json::json!({ "source": "pack" })),
    )
    .await
}

pub(crate) async fn install_optifabric_component(
    instance_id: &str,
    game_version: &str,
    version: &str,
) -> crate::Result<()> {
    let version = crate::install::runner::install_optifabric_file(
        instance_id,
        game_version,
        version,
    )
    .await?;
    record_loader_component(
        instance_id,
        crate::state::LoaderComponentKind::OptiFabric,
        &version,
        Some(serde_json::json!({
            "projectId": crate::install::runner::OPTIFABRIC_CURSEFORGE_PROJECT_ID,
            "provider": "curseforge"
        })),
    )
    .await
}

/// Installs OptiFine into the instance's mods folder for packs that pair it
/// with Forge or NeoForge. Requires the instance's Minecraft install to have
/// completed so the client jar and a Java runtime are available.
pub(crate) async fn install_optifine_mod(
    state: &State,
    instance_id: &str,
    cancellation: tokio_util::sync::CancellationToken,
    game_version: &str,
    optifine_version: &str,
    instance_path: &std::path::Path,
) -> crate::Result<()> {
    let metadata =
        crate::api::instance::get(instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let version_jar = match &metadata.applied_content_set.loader_version {
        Some(loader_version) => format!("{game_version}-{loader_version}"),
        None => game_version.to_string(),
    };
    let loader_client_jar = state
        .directories
        .version_dir(&version_jar)
        .join(format!("{version_jar}.jar"));
    let client_jar = if loader_client_jar.is_file() {
        loader_client_jar
    } else {
        state
            .directories
            .version_dir(game_version)
            .join(format!("{game_version}.jar"))
    };

    let (manifest, version_index) =
        crate::launcher::resolve_minecraft_manifest(game_version, state)
            .await?;
    let version_info = crate::launcher::download::download_version_info(
        state,
        &manifest.versions[version_index],
        ModLoader::Vanilla,
        None,
        None,
        None,
        None,
    )
    .await?;
    let java_key = version_info
        .java_version
        .as_ref()
        .map_or(8, |java| java.major_version);
    let java = crate::api::jre::find_java_for_version(java_key)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "No Java {java_key} runtime is available for the OptiFine installer"
            ))
        })?;

    crate::launcher::optifine::install_optifine_as_mod(
        state,
        instance_id,
        cancellation,
        std::path::Path::new(&java.path),
        game_version,
        optifine_version,
        &client_jar,
        &instance_path.join("mods"),
    )
    .await?;
    Ok(())
}
