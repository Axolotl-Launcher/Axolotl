use std::path::PathBuf;

use crate::state::{DirectoryInfo, Instance};

use super::DirectLinkedLaunch;

/// Runtime storage adapters keep mode-specific path rules out of callers.
#[derive(Debug, Clone)]
pub(crate) enum InstanceRuntimeAdapter {
    AxolotlManaged { game_dir: PathBuf },
    MinecraftShared { direct: DirectLinkedLaunch },
    MinecraftIsolated { direct: DirectLinkedLaunch },
}

impl InstanceRuntimeAdapter {
    /// Entry point selecting the adapter for an instance.
    pub(crate) fn for_instance(
        instance: &Instance,
        directories: &DirectoryInfo,
    ) -> crate::Result<Self> {
        if let Some(external) = Self::external_for_instance(instance)? {
            return Ok(external);
        }

        Ok(Self::AxolotlManaged {
            game_dir: directories.instance_game_dir(instance),
        })
    }

    pub(crate) fn external_for_instance(
        instance: &Instance,
    ) -> crate::Result<Option<Self>> {
        if let Some(direct) = DirectLinkedLaunch::from_instance(instance)? {
            return Ok(Some(Self::from_direct(direct)));
        }

        let Some(direct) =
            DirectLinkedLaunch::from_game_dir_override(instance)?
        else {
            return Ok(None);
        };
        Ok(Some(Self::from_direct(direct)))
    }

    fn from_direct(direct: DirectLinkedLaunch) -> Self {
        let isolated = match direct.game_dir_mode {
            Some(super::ExternalGameDirMode::Isolated) => true,
            Some(super::ExternalGameDirMode::Shared) => false,
            Some(super::ExternalGameDirMode::Automatic) | None => {
                match direct.dialect {
                    super::LinkedLauncherDialect::Pcl
                    | super::LinkedLauncherDialect::PclCe => true,
                    super::LinkedLauncherDialect::Generic => true,
                    super::LinkedLauncherDialect::Hmcl => false,
                }
            }
        };
        if isolated {
            Self::MinecraftIsolated { direct }
        } else {
            Self::MinecraftShared { direct }
        }
    }

    pub(crate) fn direct_link(&self) -> Option<&DirectLinkedLaunch> {
        match self {
            Self::AxolotlManaged { .. } => None,
            Self::MinecraftShared { direct }
            | Self::MinecraftIsolated { direct } => Some(direct),
        }
    }

    pub(crate) fn game_dir(&self) -> PathBuf {
        match self {
            Self::AxolotlManaged { game_dir } => game_dir.clone(),
            // Explicit roots don't depend on the merged version manifest.
            Self::MinecraftShared { direct }
                if direct.game_dir_mode
                    == Some(super::ExternalGameDirMode::Shared) =>
            {
                direct.dot_minecraft.clone()
            }
            Self::MinecraftIsolated { direct }
                if direct.game_dir_mode
                    == Some(super::ExternalGameDirMode::Isolated)
                    || direct.dialect
                        == super::LinkedLauncherDialect::Generic =>
            {
                direct.version_dir()
            }
            Self::MinecraftShared { direct } => match direct.resolve() {
                Ok(resolved) => resolved.game_dir,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Falling back after the external instance version chain could not be resolved"
                    );
                    direct.dot_minecraft.clone()
                }
            },
            Self::MinecraftIsolated { direct } => match direct.resolve() {
                Ok(resolved) => resolved.game_dir,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Falling back to the external instance version directory"
                    );
                    direct.version_dir()
                }
            },
        }
    }

    pub(crate) fn libraries_dir(&self, directories: &DirectoryInfo) -> PathBuf {
        self.direct_link().map_or_else(
            || directories.libraries_dir(),
            DirectLinkedLaunch::libraries_dir,
        )
    }

    pub(crate) fn assets_dir(&self, directories: &DirectoryInfo) -> PathBuf {
        self.direct_link().map_or_else(
            || directories.assets_dir(),
            DirectLinkedLaunch::assets_dir,
        )
    }

    pub(crate) fn game_assets_dir(
        &self,
        directories: &DirectoryInfo,
        legacy: bool,
    ) -> PathBuf {
        if !legacy {
            return self.assets_dir(directories);
        }

        self.direct_link().map_or_else(
            || directories.legacy_assets_dir(),
            |direct| direct.assets_dir().join("virtual").join("legacy"),
        )
    }

    pub(crate) fn log_configs_dir(
        &self,
        directories: &DirectoryInfo,
    ) -> PathBuf {
        self.direct_link().map_or_else(
            || directories.log_configs_dir(),
            DirectLinkedLaunch::log_configs_dir,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::LinkedLauncherDialect;

    fn directories(root: &std::path::Path) -> DirectoryInfo {
        DirectoryInfo {
            settings_dir: root.join("settings"),
            config_dir: root.join("config"),
            app_identifier: "test".to_string(),
        }
    }

    #[test]
    fn managed_legacy_launch_uses_expanded_resources_directory() {
        let root = tempfile::tempdir().unwrap();
        let directories = directories(root.path());
        let runtime = InstanceRuntimeAdapter::AxolotlManaged {
            game_dir: root.path().join("instance"),
        };

        assert_eq!(
            runtime.game_assets_dir(&directories, true),
            directories.legacy_assets_dir()
        );
        assert_eq!(
            runtime.game_assets_dir(&directories, false),
            directories.assets_dir()
        );
    }

    #[test]
    fn linked_legacy_launch_uses_standard_virtual_assets_directory() {
        let root = tempfile::tempdir().unwrap();
        let directories = directories(root.path());
        let dot_minecraft = root.path().join(".minecraft");
        let runtime = InstanceRuntimeAdapter::MinecraftShared {
            direct: DirectLinkedLaunch {
                dot_minecraft: dot_minecraft.clone(),
                launcher_root: None,
                version_id: "1.6.4".to_string(),
                version_json: None,
                dialect: LinkedLauncherDialect::Generic,
                game_dir_mode: None,
            },
        };

        assert_eq!(
            runtime.game_assets_dir(&directories, true),
            dot_minecraft.join("assets").join("virtual").join("legacy")
        );
        assert_eq!(
            runtime.game_assets_dir(&directories, false),
            dot_minecraft.join("assets")
        );
    }
}
