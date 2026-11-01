use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::{ImportOverrides, instance_json};
use crate::{
    State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    launcher::get_loader_version_from_profile,
    pack::{
        import::finish_import,
        install_from::{self, CreatePackDescription, PackDependency},
    },
    state::ModLoader,
};

/// Import a generic launcher instance folder into an Axolotl profile.
///
/// Runs in four stages: resolve the source folder, validate that it contains
/// a detectable Minecraft version, register the instance metadata, then copy
/// (or symlink) the files into the profile.
pub async fn import_generic(
    instance_folder: PathBuf,
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
    overrides: &ImportOverrides,
    instance_path: Option<PathBuf>, // For compatible mode: path to versions/<version>/
) -> crate::Result<()> {
    let (name, dotminecraft, json_path) = resolve_source_paths(
        &instance_folder,
        instance_path.as_deref(),
        symlink,
    );

    let info = detect_instance_info(&json_path, overrides).await?;
    tracing::debug!(
        source = %dotminecraft.display(),
        metadata = %json_path.display(),
        "Resolved generic import content and version paths"
    );
    register_instance(instance_id, &name, &info).await?;
    copy_instance_files(instance_id, &dotminecraft, reporter, details, symlink)
        .await
}

fn resolve_source_paths(
    instance_folder: &Path,
    instance_path: Option<&Path>,
    symlink: bool,
) -> (String, PathBuf, PathBuf) {
    let (name, dotminecraft, json_path) = if let Some(inst_path) = instance_path
    {
        let name = inst_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "imported".to_string());
        tracing::debug!(
            "import_generic: compatible mode - dotminecraft={}, json_path={}",
            instance_folder.display(),
            inst_path.display()
        );
        let content_root = if symlink {
            instance_folder.to_path_buf()
        } else {
            super::resolve_import_game_root(inst_path)
        };
        (name, content_root, inst_path.to_path_buf())
    } else {
        let (name, dotminecraft) = resolve_dotminecraft(instance_folder);
        let json_path = dotminecraft.clone(); // JSON detection will scan dotminecraft
        (name, dotminecraft, json_path)
    };

    (name, dotminecraft, json_path)
}

/// Stage 1 — resolve the name and the `.minecraft` directory of an imported
/// instance folder. Falls back to the folder itself when there is no nested
/// `.minecraft` subdirectory.
pub(crate) fn resolve_dotminecraft(
    instance_folder: &Path,
) -> (String, PathBuf) {
    let name = instance_folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".to_string());

    let dotminecraft = instance_folder.join(".minecraft");
    if dotminecraft.is_dir() {
        tracing::debug!(
            "import_generic: using .minecraft subdir at {}",
            dotminecraft.display()
        );
        (name, dotminecraft)
    } else {
        tracing::debug!(
            "import_generic: using folder directly at {}",
            instance_folder.display()
        );
        (name, instance_folder.to_path_buf())
    }
}

/// Stage 2 — validate the folder contains a Minecraft version JSON.
async fn detect_instance_info(
    dotminecraft: &Path,
    overrides: &ImportOverrides,
) -> crate::Result<instance_json::InstanceInfo> {
    tracing::debug!(
        "import_generic: about to detect instance_json at dotminecraft={}",
        dotminecraft.display()
    );
    let Some(mut info) = instance_json::detect(dotminecraft) else {
        let Some(game_version) = overrides
            .game_version
            .as_ref()
            .filter(|version| !version.trim().is_empty())
        else {
            tracing::warn!(
                "import_generic: instance_json::detect returned None for {}",
                dotminecraft.display()
            );
            return Err(crate::ErrorKind::InputError(
                "Could not detect Minecraft version. Make sure the folder contains a valid version JSON."
                    .into(),
            )
            .into());
        };
        return Ok(instance_json::InstanceInfo {
            vanilla_name: game_version.clone(),
            loader: overrides
                .loader
                .filter(|loader| *loader != ModLoader::Vanilla)
                .map(|loader| loader.as_str().to_string()),
            loader_version: overrides
                .loader_version
                .clone()
                .filter(|version| !version.is_empty() && version != "latest"),
            adjuncts: Vec::new(),
        });
    };

    if let Some(game_version) = overrides
        .game_version
        .as_ref()
        .filter(|version| !version.trim().is_empty())
    {
        info.vanilla_name.clone_from(game_version);
    }
    if let Some(loader) = overrides.loader {
        if loader == ModLoader::Vanilla {
            info.loader = None;
            info.loader_version = None;
            info.adjuncts.clear();
        } else {
            info.loader = Some(loader.as_str().to_string());
            info.loader_version = None;
        }
    }
    if let Some(loader_version) = overrides
        .loader_version
        .as_ref()
        .filter(|version| !version.is_empty() && *version != "latest")
    {
        info.loader_version = Some(loader_version.clone());
    }

    Ok(info)
}

/// Stage 3 — register the instance metadata (name, game version, loaders)
/// with the app database.
async fn register_instance(
    instance_id: &str,
    name: &str,
    info: &instance_json::InstanceInfo,
) -> crate::Result<()> {
    tracing::debug!(
        "import_generic: detect result: vanilla_name={} loader={:?} loader_version={:?}",
        info.vanilla_name,
        info.loader,
        info.loader_version
    );

    let description = CreatePackDescription {
        icon: None,
        override_title: Some(name.to_string()),
        project_id: None,
        version_id: None,
        instance_id: instance_id.to_string(),
        source_filename: None,
    };
    let dependencies = build_dependencies(info).await?;

    tracing::debug!(
        "import_generic: setting instance info with dependencies={:?}",
        dependencies
    );
    install_from::set_instance_information(
        instance_id.to_string(),
        &description,
        "Imported from folder",
        None,
        &dependencies,
        false,
    )
    .await?;
    Ok(())
}

/// Builds the dependency map from the detected game version and loader,
/// resolving the loader version from the metadata API when it is missing.
async fn build_dependencies(
    info: &instance_json::InstanceInfo,
) -> crate::Result<HashMap<PackDependency, String>> {
    let mut dependencies =
        HashMap::from([(PackDependency::Minecraft, info.vanilla_name.clone())]);
    let Some(ref loader) = info.loader else {
        tracing::debug!("import_generic: no loader detected, will be Vanilla");
        return Ok(dependencies);
    };
    let components = std::iter::once((
        loader.as_str(),
        info.loader_version.as_ref(),
    ))
    .chain(info.adjuncts.iter().filter_map(|(loader, version)| {
        (loader != "optifabric").then_some((loader.as_str(), version.as_ref()))
    }));
    for (loader, version) in components {
        if loader.eq_ignore_ascii_case("labymod") {
            return Err(crate::ErrorKind::InputError(
                "Unsupported loader LabyMod: Axolotl does not install, update, or repair LabyMod instances"
                    .to_string(),
            )
            .into());
        }
        let dep = loader_dependency(loader).ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unsupported loader {loader}: the instance was not imported as Vanilla"
            ))
            .as_error()
        })?;
        let loader_version = resolve_loader_version(
            &info.vanilla_name,
            loader,
            version.map(String::as_str),
        )
        .await;
        match loader_version {
            Some(version) => {
                dependencies.insert(dep, version);
            }
            None => {
                return Err(crate::ErrorKind::InputError(format!(
                    "Could not resolve {loader} for Minecraft {}; the instance was not imported as Vanilla",
                    info.vanilla_name
                ))
                .into());
            }
        }
    }
    if let Some((_, version)) = info
        .adjuncts
        .iter()
        .find(|(loader, _)| loader == "optifabric")
    {
        let version = version.as_ref().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Imported OptiFabric component is missing its version"
                    .to_string(),
            )
        })?;
        dependencies.insert(PackDependency::OptiFabric, version.clone());
    }
    Ok(dependencies)
}

/// Maps a detected loader name to a dependency the launcher can install.
fn loader_dependency(loader: &str) -> Option<PackDependency> {
    match loader {
        "forge" => Some(PackDependency::Forge),
        "neoforge" => Some(PackDependency::NeoForge),
        "fabric" => Some(PackDependency::FabricLoader),
        "quilt" => Some(PackDependency::QuiltLoader),
        "optifine" => Some(PackDependency::OptiFine),
        "cleanroom" => Some(PackDependency::Cleanroom),
        "lite_loader" | "liteloader" => Some(PackDependency::LiteLoader),
        "legacy_fabric" | "legacyfabric" => Some(PackDependency::LegacyFabric),
        _ => None,
    }
}

/// Resolves a missing loader version by asking the metadata API for the
/// latest version compatible with the detected game version.
async fn resolve_loader_version(
    game_version: &str,
    loader: &str,
    requested_version: Option<&str>,
) -> Option<String> {
    if requested_version
        .is_some_and(|version| !version.is_empty() && version != "latest")
    {
        return requested_version.map(str::to_string);
    }
    let mod_loader = match loader {
        "forge" => Some(ModLoader::Forge),
        "neoforge" => Some(ModLoader::NeoForge),
        "fabric" => Some(ModLoader::Fabric),
        "quilt" => Some(ModLoader::Quilt),
        "optifine" => Some(ModLoader::OptiFine),
        "cleanroom" => Some(ModLoader::Cleanroom),
        "lite_loader" | "liteloader" => Some(ModLoader::LiteLoader),
        "legacy_fabric" | "legacyfabric" => Some(ModLoader::LegacyFabric),
        _ => None,
    }?;
    tracing::debug!(
        "import_generic: loader={} has no version, resolving latest for game_version={}",
        loader,
        game_version
    );
    match get_loader_version_from_profile(game_version, mod_loader, None).await
    {
        Ok(Some(lv)) => {
            tracing::debug!(
                "import_generic: resolved latest loader version: {}",
                lv.id
            );
            Some(lv.id)
        }
        Ok(None) => {
            tracing::warn!(
                "import_generic: no loader version found for {} {}",
                mod_loader.as_str(),
                game_version
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                "import_generic: failed to resolve loader version: {e}",
            );
            None
        }
    }
}

/// Stage 4 — copy (or symlink) the source files into the instance profile.
async fn copy_instance_files(
    instance_id: &str,
    dotminecraft: &Path,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    tracing::debug!(
        "import_generic: finishing import for instance_id={}",
        instance_id
    );
    finish_import(
        instance_id,
        dotminecraft.to_path_buf(),
        &state.io_semaphore,
        reporter,
        details,
        symlink,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn isolated_copy_import_preserves_mods_config_and_scripts_at_the_profile_root()
     {
        let temp = tempfile::tempdir().unwrap();
        let minecraft = temp.path().join(".minecraft");
        let version = minecraft.join("versions/RLCraft");
        let destination = temp.path().join("profiles/RLCraft");
        for (path, bytes) in [
            ("mods/example.jar", "rlcraft mod"),
            ("config/example.cfg", "rlcraft config"),
            ("scripts/recipes.zs", "rlcraft scripts"),
            ("options.txt", "rlcraft options"),
            (
                "RLCraft.json",
                r#"{"id":"RLCraft","inheritsFrom":"1.12.2"}"#,
            ),
            ("RLCraft.jar", "client jar"),
        ] {
            let path = version.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        std::fs::create_dir_all(minecraft.join("mods")).unwrap();
        std::fs::write(minecraft.join("mods/unrelated.jar"), "shared mod")
            .unwrap();
        let (_, content, metadata) =
            resolve_source_paths(&minecraft, Some(&version), false);
        assert_eq!(content, version);
        assert_eq!(metadata, version);
        assert_eq!(
            detect_instance_info(&metadata, &ImportOverrides::default())
                .await
                .unwrap()
                .vanilla_name,
            "1.12.2"
        );
        let files = super::super::collect_dotminecraft_files(&content)
            .await
            .unwrap();
        assert_eq!(files.len(), 4);
        let semaphore =
            crate::util::fetch::IoSemaphore(tokio::sync::Semaphore::new(1));
        for (source, relative) in files {
            super::super::copy_import_file_atomically(
                &source,
                &destination.join(relative),
                &semaphore,
            )
            .await
            .unwrap();
        }
        for (path, bytes) in [
            ("mods/example.jar", "rlcraft mod"),
            ("config/example.cfg", "rlcraft config"),
            ("scripts/recipes.zs", "rlcraft scripts"),
            ("options.txt", "rlcraft options"),
        ] {
            assert_eq!(
                std::fs::read_to_string(destination.join(path)).unwrap(),
                bytes
            );
            assert_eq!(
                std::fs::read_to_string(version.join(path)).unwrap(),
                bytes
            );
        }
        assert!(!destination.join("mods/unrelated.jar").exists());
        assert!(!destination.join("versions").exists());
        assert!(!destination.join("RLCraft.json").exists());
        assert!(!destination.join("RLCraft.jar").exists());
    }

    #[test]
    fn shared_copy_import_keeps_metadata_in_the_selected_version() {
        let temp = tempfile::tempdir().unwrap();
        let minecraft = temp.path().join(".minecraft");
        let version = minecraft.join("versions/Shared");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::create_dir_all(minecraft.join("mods")).unwrap();
        let (_, content, metadata) =
            resolve_source_paths(&minecraft, Some(&version), false);
        assert_eq!(content, minecraft);
        assert_eq!(metadata, version);
    }

    #[test]
    fn existing_symbolic_link_import_path_selection_is_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let minecraft = temp.path().join(".minecraft");
        let version = minecraft.join("versions/Linked");
        std::fs::create_dir_all(version.join("mods")).unwrap();
        let (_, content, metadata) =
            resolve_source_paths(&minecraft, Some(&version), true);
        assert_eq!(content, minecraft);
        assert_eq!(metadata, version);
    }

    #[test]
    fn loader_dependency_maps_supported_loaders() {
        assert_eq!(loader_dependency("forge"), Some(PackDependency::Forge));
        assert_eq!(
            loader_dependency("neoforge"),
            Some(PackDependency::NeoForge)
        );
        assert_eq!(
            loader_dependency("fabric"),
            Some(PackDependency::FabricLoader)
        );
        assert_eq!(
            loader_dependency("quilt"),
            Some(PackDependency::QuiltLoader)
        );
        assert_eq!(
            loader_dependency("optifine"),
            Some(PackDependency::OptiFine)
        );
        assert_eq!(
            loader_dependency("cleanroom"),
            Some(PackDependency::Cleanroom)
        );
        assert_eq!(
            loader_dependency("lite_loader"),
            Some(PackDependency::LiteLoader)
        );
        assert_eq!(
            loader_dependency("legacy_fabric"),
            Some(PackDependency::LegacyFabric)
        );
    }

    #[test]
    fn loader_dependency_rejects_unsupported_loaders() {
        for loader in ["labymod", "unknown", "vanilla"] {
            assert_eq!(loader_dependency(loader), None, "{loader}");
        }
    }

    #[tokio::test]
    async fn detect_overrides_replace_missing_detection() {
        let directory = tempfile::tempdir().unwrap();
        let overrides = ImportOverrides {
            game_version: Some("1.20.1".to_string()),
            loader: Some(ModLoader::Fabric),
            loader_version: Some("0.15.11".to_string()),
        };

        let info = detect_instance_info(directory.path(), &overrides)
            .await
            .unwrap();

        assert_eq!(info.vanilla_name, "1.20.1");
        assert_eq!(info.loader.as_deref(), Some("fabric"));
        assert_eq!(info.loader_version.as_deref(), Some("0.15.11"));
    }

    #[tokio::test]
    async fn detect_overrides_ignore_blank_and_latest_loader_version() {
        let directory = tempfile::tempdir().unwrap();
        for loader_version in ["", "latest"] {
            let overrides = ImportOverrides {
                game_version: Some("1.20.1".to_string()),
                loader: Some(ModLoader::Fabric),
                loader_version: Some(loader_version.to_string()),
            };

            let info = detect_instance_info(directory.path(), &overrides)
                .await
                .unwrap();

            assert_eq!(info.loader_version, None);
        }
    }

    #[tokio::test]
    async fn build_dependencies_uses_override_values() {
        let info = instance_json::InstanceInfo {
            vanilla_name: "1.20.1".to_string(),
            loader: Some("fabric".to_string()),
            loader_version: Some("0.15.11".to_string()),
            adjuncts: Vec::new(),
        };

        let dependencies = build_dependencies(&info).await.unwrap();

        assert_eq!(
            dependencies.get(&PackDependency::Minecraft),
            Some(&"1.20.1".to_string())
        );
        assert_eq!(
            dependencies.get(&PackDependency::FabricLoader),
            Some(&"0.15.11".to_string())
        );
    }

    #[tokio::test]
    async fn labymod_and_unknown_loaders_never_become_vanilla() {
        for loader in ["labymod", "unknown_loader"] {
            let info = instance_json::InstanceInfo {
                vanilla_name: "1.20.1".to_string(),
                loader: Some(loader.to_string()),
                loader_version: Some("1.0".to_string()),
                adjuncts: Vec::new(),
            };
            let error = build_dependencies(&info).await.unwrap_err();
            assert!(error.to_string().contains("Unsupported loader"));
        }
    }
}
