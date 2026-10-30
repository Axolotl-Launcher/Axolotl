#[cfg(target_os = "windows")]
mod windows {
    use super::super::{DirectoryInfo, Settings};
    use crate::state::store::Handle;
    use std::path::{Path, PathBuf};
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    const INSTALLER_REGISTRY_KEY: &str = "Software\\ghs\\Axolotl Launcher";
    const PENDING_RESOURCE_DIRECTORY_VALUE: &str = "PendingResourceDirectory";

    #[derive(Debug, Eq, PartialEq)]
    enum PendingDirectoryDecision {
        Apply {
            custom_dir: String,
            prev_custom_dir: String,
        },
        Clear,
        Ignore,
    }

    fn normalize_directory(path: &str) -> Option<String> {
        let path = path.trim();
        if path.is_empty() {
            return None;
        }

        let path = PathBuf::from(path);
        if !path.is_absolute() || path.parent().is_none() {
            return None;
        }

        let mut normalized = path.to_string_lossy().replace('/', "\\");
        while normalized.len() > 3 && normalized.ends_with('\\') {
            normalized.pop();
        }

        Some(normalized)
    }

    fn decide_pending_directory(
        pending_directory: Option<&str>,
        default_directory: &Path,
        settings_initialized: bool,
        portable: bool,
    ) -> PendingDirectoryDecision {
        if portable {
            return PendingDirectoryDecision::Ignore;
        }

        let Some(pending_directory) = pending_directory else {
            return PendingDirectoryDecision::Ignore;
        };

        if settings_initialized {
            return PendingDirectoryDecision::Clear;
        }

        let Some(custom_dir) = normalize_directory(pending_directory) else {
            return PendingDirectoryDecision::Clear;
        };
        let Some(prev_custom_dir) =
            normalize_directory(&default_directory.to_string_lossy())
        else {
            return PendingDirectoryDecision::Clear;
        };

        if custom_dir.eq_ignore_ascii_case(&prev_custom_dir) {
            return PendingDirectoryDecision::Clear;
        }

        PendingDirectoryDecision::Apply {
            custom_dir,
            prev_custom_dir,
        }
    }

    fn open_installer_registry_key() -> std::io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            INSTALLER_REGISTRY_KEY,
            KEY_READ | KEY_SET_VALUE,
        )
    }

    fn clear_pending_directory(key: &RegKey) -> crate::Result<()> {
        match key.delete_value(PENDING_RESOURCE_DIRECTORY_VALUE) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn apply_pending_installer_directory(
        settings: &mut Settings,
        handle: &Handle,
        app_identifier: &str,
    ) -> crate::Result<()> {
        if std::env::var_os("THESEUS_CONFIG_DIR").is_some() {
            return Ok(());
        }

        let key = match open_installer_registry_key() {
            Ok(key) => key,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let pending_directory: Option<String> =
            match key.get_value(PENDING_RESOURCE_DIRECTORY_VALUE) {
                Ok(value) => Some(value),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };
        let default_directory = DirectoryInfo::initial_settings_dir_path(
            app_identifier,
        )
        .ok_or(crate::ErrorKind::FSError(
            "Could not find valid config dir".to_string(),
        ))?;
        let settings_initialized =
            settings.custom_dir.is_some() || settings.prev_custom_dir.is_some();

        match decide_pending_directory(
            pending_directory.as_deref(),
            &default_directory,
            settings_initialized,
            false,
        ) {
            PendingDirectoryDecision::Apply {
                custom_dir,
                prev_custom_dir,
            } => {
                tracing::info!(
                    "Applying the application directory selected by the installer"
                );
                settings.custom_dir = Some(custom_dir);
                settings.prev_custom_dir = Some(prev_custom_dir);
                settings.update(handle).await?;
                clear_pending_directory(&key)?;
            }
            PendingDirectoryDecision::Clear => {
                clear_pending_directory(&key)?;
            }
            PendingDirectoryDecision::Ignore => {}
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn default_directory() -> PathBuf {
            PathBuf::from(r"C:\Users\Test\AppData\Roaming\red.ghs.axolotl")
        }

        #[test]
        fn applies_a_custom_installer_directory() {
            assert_eq!(
                decide_pending_directory(
                    Some(r"D:\Minecraft\Axolotl"),
                    &default_directory(),
                    false,
                    false,
                ),
                PendingDirectoryDecision::Apply {
                    custom_dir: r"D:\Minecraft\Axolotl".to_string(),
                    prev_custom_dir: default_directory()
                        .to_string_lossy()
                        .to_string(),
                }
            );
        }

        #[test]
        fn clears_a_default_installer_directory() {
            assert_eq!(
                decide_pending_directory(
                    Some(r"C:\Users\Test\AppData\Roaming\red.ghs.axolotl\"),
                    &default_directory(),
                    false,
                    false,
                ),
                PendingDirectoryDecision::Clear
            );
        }

        #[test]
        fn does_not_override_existing_settings() {
            assert_eq!(
                decide_pending_directory(
                    Some(r"D:\Minecraft\Axolotl"),
                    &default_directory(),
                    true,
                    false,
                ),
                PendingDirectoryDecision::Clear
            );
        }

        #[test]
        fn clears_invalid_relative_and_root_directories() {
            for path in ["relative", r"C:\", ""] {
                assert_eq!(
                    decide_pending_directory(
                        Some(path),
                        &default_directory(),
                        false,
                        false,
                    ),
                    PendingDirectoryDecision::Clear
                );
            }
        }

        #[test]
        fn portable_mode_leaves_the_pending_value_for_the_installed_app() {
            assert_eq!(
                decide_pending_directory(
                    Some(r"D:\Minecraft\Axolotl"),
                    &default_directory(),
                    false,
                    true,
                ),
                PendingDirectoryDecision::Ignore
            );
        }

        #[test]
        fn applied_state_can_restore_the_default_directory() {
            let PendingDirectoryDecision::Apply {
                custom_dir: _,
                prev_custom_dir,
            } = decide_pending_directory(
                Some(r"D:\Minecraft\Axolotl"),
                &default_directory(),
                false,
                false,
            )
            else {
                panic!("expected installer directory to be applied");
            };

            assert_eq!(prev_custom_dir, default_directory().to_string_lossy());
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::apply_pending_installer_directory;

#[cfg(not(target_os = "windows"))]
pub async fn apply_pending_installer_directory(
    _settings: &mut super::Settings,
    _handle: &crate::state::store::Handle,
    _app_identifier: &str,
) -> crate::Result<()> {
    Ok(())
}
