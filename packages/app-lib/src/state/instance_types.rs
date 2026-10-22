use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceInstallStage {
    Installed,
    MinecraftInstalling,
    PackInstalled,
    PackInstalling,
    NotInstalled,
}

impl InstanceInstallStage {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Installed => "installed",
            Self::MinecraftInstalling => "minecraft_installing",
            Self::PackInstalled => "pack_installed",
            Self::PackInstalling => "pack_installing",
            Self::NotInstalled => "not_installed",
        }
    }

    pub fn from_str(val: &str) -> Self {
        match val {
            "installed" => Self::Installed,
            "minecraft_installing" => Self::MinecraftInstalling,
            "installing" => Self::MinecraftInstalling,
            "pack_installed" => Self::PackInstalled,
            "pack_installing" => Self::PackInstalling,
            "not_installed" => Self::NotInstalled,
            _ => Self::NotInstalled,
        }
    }
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd,
)]
#[serde(rename_all = "snake_case")]
pub enum LauncherFeatureVersion {
    None,
    MigratedServerLastPlayTime,
    MigratedLaunchHooks,
}

impl LauncherFeatureVersion {
    pub const MOST_RECENT: Self = Self::MigratedLaunchHooks;

    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::None => "none",
            Self::MigratedServerLastPlayTime => {
                "migrated_server_last_play_time"
            }
            Self::MigratedLaunchHooks => "migrated_launch_hooks",
        }
    }

    pub fn from_str(val: &str) -> Self {
        match val {
            "none" => Self::None,
            "migrated_server_last_play_time" => {
                Self::MigratedServerLastPlayTime
            }
            "migrated_launch_hooks" => Self::MigratedLaunchHooks,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModLoader {
    Vanilla,
    Forge,
    Fabric,
    Quilt,
    #[serde(rename = "neoforge", alias = "neo_forge", alias = "neo")]
    NeoForge,
    #[serde(rename = "optifine", alias = "opti_fine")]
    OptiFine,
    Cleanroom,
    LiteLoader,
    LegacyFabric,
    Babric,
}

impl ModLoader {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neoforge",
            Self::OptiFine => "optifine",
            Self::Cleanroom => "cleanroom",
            Self::LiteLoader => "lite_loader",
            Self::LegacyFabric => "legacy_fabric",
            Self::Babric => "babric",
        }
    }

    pub fn as_meta_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neo",
            // OptiFine has no Daedalus metadata; versions resolve through
            // launcher::optifine instead of the meta server.
            Self::OptiFine => "optifine",
            Self::Cleanroom => "cleanroom",
            Self::LiteLoader => "lite_loader",
            Self::LegacyFabric => "legacy_fabric",
            Self::Babric => "babric",
        }
    }

    pub fn try_from_string(val: &str) -> crate::Result<Self> {
        match val {
            "vanilla" => Ok(Self::Vanilla),
            "forge" => Ok(Self::Forge),
            "fabric" => Ok(Self::Fabric),
            "quilt" => Ok(Self::Quilt),
            "neoforge" | "neo_forge" | "neo" => Ok(Self::NeoForge),
            "optifine" | "opti_fine" => Ok(Self::OptiFine),
            "cleanroom" => Ok(Self::Cleanroom),
            "lite_loader" | "liteloader" => Ok(Self::LiteLoader),
            "legacy_fabric" | "legacyfabric" => Ok(Self::LegacyFabric),
            "babric" => Ok(Self::Babric),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unsupported loader {other}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentFile {
    pub hash: String,
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub modrinth: Option<ModrinthFileMatch>,
    pub provider_refs: Vec<crate::state::ContentProviderRef>,
    pub origin_provider: Option<crate::state::ContentProvider>,
    pub update: Option<crate::state::ContentItemUpdate>,
    pub project_type: ProjectType,
    /// JSON-encoded `LocalModMetadata` extracted from the JAR's embedded
    /// mod metadata file. Populated when Modrinth hash lookup provides
    /// no match for the SHA1. Used as fallback display data.
    pub local_mod_data: Option<String>,
    /// Absolute path of the cached extracted icon; empty string marks a file
    /// that was checked but has no icon.
    pub icon_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModrinthFileMatch {
    pub project_id: crate::state::ModrinthProjectId,
    pub version_id: crate::state::ModrinthVersionId,
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Mod,
    DataPack,
    ResourcePack,
    #[serde(alias = "shader")]
    ShaderPack,
    Schematic,
    WorldSave,
}

impl ProjectType {
    pub fn get_from_loaders(loaders: Vec<String>) -> Option<Self> {
        if loaders.iter().any(|x| {
            [
                "fabric",
                "forge",
                "quilt",
                "neoforge",
                "cleanroom",
                "lite_loader",
                "legacy_fabric",
                "babric",
            ]
            .contains(&&**x)
        }) {
            Some(ProjectType::Mod)
        } else if loaders.iter().any(|x| x == "datapack") {
            Some(ProjectType::DataPack)
        } else if loaders.iter().any(|x| ["iris", "optifine"].contains(&&**x)) {
            Some(ProjectType::ShaderPack)
        } else if loaders
            .iter()
            .any(|x| ["vanilla", "canvas", "minecraft"].contains(&&**x))
        {
            Some(ProjectType::ResourcePack)
        } else if loaders.iter().any(|x| x == "litematica") {
            Some(ProjectType::Schematic)
        } else {
            None
        }
    }

    pub fn get_from_parent_folder(path: impl AsRef<Path>) -> Option<Self> {
        Self::from_folder_name(
            path.as_ref()
                .parent()?
                .file_name()?
                .to_str()
                .unwrap_or_default(),
        )
    }

    pub(crate) fn from_folder_name(folder_name: &str) -> Option<Self> {
        match folder_name {
            "mods" => Some(ProjectType::Mod),
            "datapacks" => Some(ProjectType::DataPack),
            "resourcepacks" => Some(ProjectType::ResourcePack),
            "shaderpacks" => Some(ProjectType::ShaderPack),
            "schematics" => Some(ProjectType::Schematic),
            "saves" => Some(ProjectType::WorldSave),
            _ => None,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            ProjectType::Mod => "mod",
            ProjectType::DataPack => "datapack",
            ProjectType::ResourcePack => "resourcepack",
            ProjectType::ShaderPack => "shader",
            ProjectType::Schematic => "schematic",
            ProjectType::WorldSave => "world_save",
        }
    }

    pub fn get_folder(&self) -> &'static str {
        match self {
            ProjectType::Mod => "mods",
            ProjectType::DataPack => "datapacks",
            ProjectType::ResourcePack => "resourcepacks",
            ProjectType::ShaderPack => "shaderpacks",
            ProjectType::Schematic => "schematics",
            ProjectType::WorldSave => "saves",
        }
    }

    pub fn get_loaders(&self) -> &'static [&'static str] {
        match self {
            ProjectType::Mod => &["fabric", "forge", "quilt", "neoforge"],
            ProjectType::DataPack => &["datapack"],
            ProjectType::ResourcePack => &["vanilla", "canvas", "minecraft"],
            ProjectType::ShaderPack => &["iris", "optifine"],
            ProjectType::Schematic => &["litematica"],
            ProjectType::WorldSave => &["vanilla"],
        }
    }

    pub fn iterator() -> impl Iterator<Item = ProjectType> {
        [
            ProjectType::Mod,
            ProjectType::DataPack,
            ProjectType::ResourcePack,
            ProjectType::ShaderPack,
            ProjectType::Schematic,
            ProjectType::WorldSave,
        ]
        .iter()
        .copied()
    }
}

impl From<ProjectType> for content_management::ContentType {
    fn from(project_type: ProjectType) -> Self {
        match project_type {
            ProjectType::Mod => Self::Mod,
            ProjectType::DataPack => Self::DataPack,
            ProjectType::ResourcePack => Self::ResourcePack,
            ProjectType::ShaderPack => Self::Shader,
            // Schematic and WorldSave are local-only types that never go through
            // Modrinth API resolution; map to Mod as a reasonable default.
            ProjectType::Schematic => Self::Mod,
            ProjectType::WorldSave => Self::Mod,
        }
    }
}
