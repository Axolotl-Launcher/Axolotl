use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de};

use crate::{
    State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    pack::{
        import::{self, finish_import},
        install_from::{self, CreatePackDescription, PackDependency},
    },
    util::io,
};

// instance.cfg
// https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/MinecraftInstance.cpp
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
#[serde(untagged)]
enum MMCInstanceEnum {
    General(MMCInstanceGeneral),
    Instance(MMCInstance),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MMCInstanceGeneral {
    pub general: MMCInstance,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct MMCInstance {
    pub java_path: Option<String>,
    pub jvm_args: Option<String>,

    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_bool")]
    pub managed_pack: Option<bool>,

    #[serde(rename = "ManagedPackID")]
    pub managed_pack_id: Option<String>,
    pub managed_pack_type: Option<MMCManagedPackType>,
    #[serde(rename = "ManagedPackVersionID")]
    pub managed_pack_version_id: Option<String>,
    pub managed_pack_version_name: Option<String>,

    #[serde(rename = "iconKey")]
    pub icon_key: Option<String>,
    #[serde(rename = "name")]
    pub name: Option<String>,
}

// serde_ini reads 'true' and 'false' as strings, so we need to convert them to booleans
fn deserialize_optional_bool<'de, D>(
    deserializer: D,
) -> Result<Option<bool>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(string) => match string.as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(de::Error::custom("expected 'true' or 'false'")),
        },
        None => Ok(None),
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MMCManagedPackType {
    Modrinth,
    Flame,
    ATLauncher,
    #[serde(other)]
    Unknown,
}

// mmc-pack.json
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MMCPack {
    pub(crate) components: Vec<MMCComponent>,
    pub(crate) format_version: u32,
}

// https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/Component.h
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MMCComponent {
    pub uid: String,

    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub dependency_only: bool,

    #[serde(default)]
    pub important: bool,
    #[serde(default)]
    pub disabled: bool,

    pub cached_name: Option<String>,
    pub cached_version: Option<String>,

    #[serde(default)]
    pub cached_requires: Vec<MMCComponentRequirement>,
    #[serde(default)]
    pub cached_conflicts: Vec<MMCComponentRequirement>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MMCComponentRequirement {
    pub uid: String,
    pub equals_version: Option<String>,
    pub suggests: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
#[serde(untagged)]
enum MMCLauncherEnum {
    General(MMCLauncherGeneral),
    Instance(MMCLauncher),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MMCLauncherGeneral {
    pub general: MMCLauncher,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct MMCLauncher {
    instance_dir: String,
}

// Checks if if its a folder, and the folder contains instance.cfg and mmc-pack.json, and they both parse
#[tracing::instrument]
pub async fn is_valid_mmc(instance_folder: PathBuf) -> bool {
    let instance_cfg = instance_folder.join("instance.cfg");
    let mmc_pack = instance_folder.join("mmc-pack.json");

    let Ok((mmc_pack, _)) = io::read_any_encoding_to_string(&mmc_pack).await
    else {
        return false;
    };

    load_instance_cfg(&instance_cfg).await.is_ok()
        && serde_json::from_str::<MMCPack>(&mmc_pack).is_ok()
}

#[tracing::instrument]
pub async fn get_instances_subpath(config: PathBuf) -> Option<String> {
    let launcher = io::read_any_encoding_to_string(&config).await.ok()?.0;
    let launcher: MMCLauncherEnum = serde_ini::from_str(&launcher).ok()?;
    match launcher {
        MMCLauncherEnum::General(p) => Some(p.general.instance_dir),
        MMCLauncherEnum::Instance(p) => Some(p.instance_dir),
    }
}

// Loading the INI (instance.cfg) file
async fn load_instance_cfg(file_path: &Path) -> crate::Result<MMCInstance> {
    match serde_ini::from_str::<MMCInstanceEnum>(
        &io::read_any_encoding_to_string(file_path).await?.0,
    )? {
        MMCInstanceEnum::General(instance_cfg) => Ok(instance_cfg.general),
        MMCInstanceEnum::Instance(instance_cfg) => Ok(instance_cfg),
    }
}

// #[tracing::instrument]
pub(crate) async fn import_mmc_instance_dir(
    mmc_instance_path: PathBuf,
    icons_dir: Option<PathBuf>,
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
) -> crate::Result<()> {
    let mmc_pack = serde_json::from_str::<MMCPack>(
        &io::read_any_encoding_to_string(
            &mmc_instance_path.join("mmc-pack.json"),
        )
        .await?
        .0,
    )?;

    let instance_cfg =
        load_instance_cfg(&mmc_instance_path.join("instance.cfg")).await?;

    // Validate custom components before creating the imported runtime. Keep
    // their metadata and bundled libraries after the ZIP scratch is removed.
    crate::launcher::multimc::MultiMcProfile::from_root(
        mmc_instance_path.clone(),
    )
    .await?;
    if !symlink {
        let destination = crate::api::instance::get_full_path(instance_id)
            .await?
            .join(crate::launcher::multimc::PROFILE_DIR);
        let files = retained_runtime_files(&mmc_instance_path).await?;
        import::copy_files_with_progress(
            &destination,
            files,
            &State::get().await?.io_semaphore,
            reporter.clone(),
            details.clone(),
        )
        .await?;
    }

    // Re-cache icon
    let icon = if let Some(icon_key) = instance_cfg.icon_key {
        let mut icon = None;
        for icon_dir in
            icons_dir.iter().chain(std::iter::once(&mmc_instance_path))
        {
            let icon_path = icon_dir.join(&icon_key);
            icon = import::recache_icon(icon_path).await?;
            if icon.is_none() {
                let icon_path = icon_dir.join(format!("{icon_key}.png"));
                icon = import::recache_icon(icon_path).await?;
            }
            if icon.is_some() {
                break;
            }
        }
        icon
    } else {
        None
    };

    // Create description from instance.cfg
    let mut description = CreatePackDescription {
        icon,
        override_title: instance_cfg.name,
        project_id: None,
        version_id: None,
        instance_id: instance_id.to_string(),
        source_filename: None,
    };

    let minecraft_folder = minecraft_directory(&mmc_instance_path).await?;

    // Managed pack
    if instance_cfg.managed_pack.unwrap_or(false) {
        match instance_cfg.managed_pack_type {
            Some(MMCManagedPackType::Modrinth) => {
                description.project_id = instance_cfg.managed_pack_id;
                description.version_id = instance_cfg.managed_pack_version_id;

                // Modrinth Managed Pack
                // Kept separate as we may in the future want to add special handling for modrinth managed packs
                import_mmc_unmanaged(instance_id, minecraft_folder, "Imported Modrinth Modpack".to_string(), description, mmc_pack, reporter, details, symlink).await?;
            }
            Some(MMCManagedPackType::Flame | MMCManagedPackType::ATLauncher) => {
                // For flame/atlauncher managed packs
                // Treat as unmanaged, but with 'minecraft' folder instead of '.minecraft'
                import_mmc_unmanaged(instance_id, minecraft_folder, "Imported Modpack".to_string(), description, mmc_pack, reporter, details, symlink).await?;
            },
            Some(_) => {
                // For managed packs that aren't modrinth, flame, atlauncher
                // Treat as unmanaged
                import_mmc_unmanaged(instance_id, minecraft_folder, "ImportedModpack".to_string(), description, mmc_pack, reporter, details, symlink).await?;
            },
            _ => return Err(crate::ErrorKind::InputError("Instance is managed, but managed pack type not specified in instance.cfg".to_string()).into())
        }
    } else {
        // Directly import unmanaged pack
        import_mmc_unmanaged(
            instance_id,
            minecraft_folder,
            "Imported Modpack".to_string(),
            description,
            mmc_pack,
            reporter,
            details,
            symlink,
        )
        .await?;
    }
    Ok(())
}

async fn retained_runtime_files(
    root: &Path,
) -> crate::Result<Vec<(PathBuf, PathBuf)>> {
    let mut files = Vec::new();
    for name in ["mmc-pack.json", "instance.cfg"] {
        files.push((root.join(name), PathBuf::from(name)));
    }
    for directory in ["patches", "libraries", "jarmods"] {
        let source = root.join(directory);
        if source.is_dir() {
            for file in import::get_all_subfiles(&source, false).await? {
                let metadata = tokio::fs::symlink_metadata(&file).await?;
                if io::is_symlink_or_reparse(&metadata) {
                    continue;
                }
                let relative = file
                    .strip_prefix(root)
                    .map_err(|_| {
                        crate::ErrorKind::InputError(
                            "Invalid MultiMC runtime file path".to_string(),
                        )
                    })?
                    .to_path_buf();
                files.push((file, relative));
            }
        }
    }
    Ok(files)
}

async fn minecraft_directory(root: &Path) -> crate::Result<PathBuf> {
    let minecraft = root.join("minecraft");
    if minecraft.is_dir() {
        return Ok(minecraft);
    }
    let minecraft = root.join(".minecraft");
    // Exports made before the first launch can contain only components and
    // JAR mods. There is no game data to copy until a game directory exists.
    io::create_dir_all(&minecraft).await?;
    Ok(minecraft)
}

async fn import_mmc_unmanaged(
    instance_id: &str,
    minecraft_folder: PathBuf,
    backup_name: String,
    description: CreatePackDescription,
    mmc_pack: MMCPack,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
) -> crate::Result<()> {
    let dependencies = mmc_dependencies(&mmc_pack)?;

    let profile = crate::launcher::multimc::MultiMcProfile::from_root(
        minecraft_folder
            .parent()
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "MultiMC instance has no root directory".to_string(),
                )
            })?
            .to_path_buf(),
    )
    .await?;
    let local_loader_versions = profile.local_loader_versions()?;
    install_from::set_instance_information_with_local_loader_versions(
        instance_id.to_string(),
        &description,
        &backup_name,
        None,
        &dependencies,
        &local_loader_versions,
    )
    .await?;

    // Moves .minecraft folder over (ie: overrides such as resourcepacks, mods, etc)
    let state = State::get().await?;
    finish_import(
        instance_id,
        minecraft_folder,
        &state.io_semaphore,
        reporter,
        details,
        symlink,
    )
    .await?;
    Ok(())
}

pub(crate) fn mmc_dependencies(
    mmc_pack: &MMCPack,
) -> crate::Result<std::collections::HashMap<PackDependency, String>> {
    let mut dependencies = std::collections::HashMap::new();
    let has_legacy_fabric = mmc_pack
        .components
        .iter()
        .filter(|component| !component.disabled)
        .any(|component| {
            component
                .uid
                .to_ascii_lowercase()
                .starts_with("net.legacyfabric")
        });
    let has_cleanroom = mmc_pack
        .components
        .iter()
        .filter(|component| !component.disabled)
        .any(|component| {
            component
                .uid
                .to_ascii_lowercase()
                .starts_with("com.cleanroommc")
        });
    for component in mmc_pack
        .components
        .iter()
        .filter(|component| !component.disabled)
    {
        let uid = component.uid.to_ascii_lowercase();
        if uid.contains("labymod") {
            return Err(crate::ErrorKind::InputError(
                "Unsupported loader LabyMod: Axolotl does not install, update, or repair LabyMod instances"
                    .to_string(),
            )
            .into());
        }
        let dependency = if uid.starts_with("net.fabricmc.fabric-loader") {
            Some(if has_legacy_fabric {
                PackDependency::LegacyFabric
            } else {
                PackDependency::FabricLoader
            })
        } else if uid.starts_with("net.legacyfabric") {
            Some(PackDependency::LegacyFabric)
        } else if uid.starts_with("net.minecraftforge") {
            (!has_cleanroom).then_some(PackDependency::Forge)
        } else if uid.starts_with("net.neoforged") {
            Some(PackDependency::NeoForge)
        } else if uid.starts_with("org.quiltmc.quilt-loader") {
            Some(PackDependency::QuiltLoader)
        } else if uid.starts_with("com.cleanroommc") {
            Some(PackDependency::Cleanroom)
        } else if uid.contains("liteloader") {
            Some(PackDependency::LiteLoader)
        } else if uid.contains("optifabric") {
            Some(PackDependency::OptiFabric)
        } else if uid.contains("optifine") {
            Some(PackDependency::OptiFine)
        } else if uid.starts_with("net.minecraft") {
            Some(PackDependency::Minecraft)
        } else {
            None
        };
        if let Some(dependency) = dependency {
            let version = component
                .version
                .clone()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "MultiMC component {} is missing its version",
                    component.uid
                ))
            })?;
            dependencies.insert(dependency, version);
        }
    }
    Ok(dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn component_only_export_gets_an_empty_game_directory() {
        let root = tempfile::tempdir().unwrap();
        let directory = minecraft_directory(root.path()).await.unwrap();
        assert!(directory.is_dir());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        std::fs::write(directory.join("options.txt"), b"existing").unwrap();
        assert_eq!(minecraft_directory(root.path()).await.unwrap(), directory);
        assert_eq!(
            std::fs::read(directory.join("options.txt")).unwrap(),
            b"existing"
        );
        std::fs::create_dir(root.path().join("minecraft")).unwrap();
        assert_eq!(
            minecraft_directory(root.path()).await.unwrap(),
            root.path().join("minecraft")
        );
    }

    #[tokio::test]
    async fn retained_runtime_includes_jar_mods_after_source_is_removed() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        for (relative, bytes) in [
            (
                "mmc-pack.json",
                "{\"formatVersion\":1,\"components\":[{\"uid\":\"net.minecraft\",\"version\":\"b1.7.3\"},{\"uid\":\"bta\"}]}",
            ),
            ("instance.cfg", "name=BTA"),
            (
                "patches/bta.json",
                "{\"+jarMods\":[{\"name\":\"bta.jar\"}]}",
            ),
            ("jarmods/bta.jar", "retained jar mod"),
            (
                "jarmods/resources/lang/en_US.lang",
                "retained directory resource",
            ),
            ("libraries/custom.jar", "retained library"),
            ("minecraft/saves/world/level.dat", "game data"),
        ] {
            let path = source.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let game = target.path().join("minecraft");
        let retained = game.join(crate::launcher::multimc::PROFILE_DIR);
        let files = retained_runtime_files(source.path()).await.unwrap();
        assert_eq!(files.len(), 6);
        for (source, relative) in files {
            let destination = retained.join(relative);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(source, destination).unwrap();
        }
        source.close().unwrap();
        assert_eq!(
            std::fs::read(retained.join("jarmods/bta.jar")).unwrap(),
            b"retained jar mod"
        );
        assert!(
            crate::launcher::multimc::MultiMcProfile::load(
                &game,
                "b1.7.3",
                crate::state::ModLoader::Vanilla,
                None
            )
            .await
            .unwrap()
            .is_some()
        );
    }

    fn component(uid: &str, version: &str) -> MMCComponent {
        MMCComponent {
            uid: uid.to_string(),
            version: Some(version.to_string()),
            dependency_only: false,
            important: true,
            disabled: false,
            cached_name: None,
            cached_version: None,
            cached_requires: Vec::new(),
            cached_conflicts: Vec::new(),
        }
    }

    #[test]
    fn dependencies_preserve_legacy_fabric_and_optifine_components() {
        let pack = MMCPack {
            format_version: 1,
            components: vec![
                component("net.minecraft", "1.8.9"),
                component("net.legacyfabric.intermediary", "1.8.9"),
                component("net.fabricmc.fabric-loader", "0.13.1.4"),
                component("optifine.OptiFine", "1.8.9_HD_U_M6_pre2"),
                component("optifabric.OptiFabric", "1.13.16"),
            ],
        };
        let dependencies = mmc_dependencies(&pack).unwrap();

        assert_eq!(
            dependencies.get(&PackDependency::LegacyFabric),
            Some(&"0.13.1.4".to_string())
        );
        assert_eq!(
            dependencies.get(&PackDependency::OptiFine),
            Some(&"1.8.9_HD_U_M6_pre2".to_string())
        );
        assert_eq!(
            dependencies.get(&PackDependency::OptiFabric),
            Some(&"1.13.16".to_string())
        );
    }

    #[test]
    fn dependencies_reject_labymod_components() {
        let pack = MMCPack {
            format_version: 1,
            components: vec![component("net.labymod.LabyMod", "4.4.20")],
        };

        assert!(mmc_dependencies(&pack).is_err());
    }

    #[test]
    fn dependencies_ignore_disabled_components() {
        let mut disabled = component("com.cleanroommc.cleanroom", "0.3.0");
        disabled.disabled = true;
        let mut disabled_laby = component("net.labymod.LabyMod", "4.4.20");
        disabled_laby.disabled = true;
        let pack = MMCPack {
            format_version: 1,
            components: vec![
                component("net.minecraft", "1.12.2"),
                component("net.minecraftforge", "14.23.5.2860"),
                disabled,
                disabled_laby,
            ],
        };
        let dependencies = mmc_dependencies(&pack).unwrap();
        assert_eq!(
            dependencies.get(&PackDependency::Forge).map(String::as_str),
            Some("14.23.5.2860")
        );
        assert!(!dependencies.contains_key(&PackDependency::Cleanroom));
    }
}
