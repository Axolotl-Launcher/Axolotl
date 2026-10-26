use serde::Serialize;
use theseus::{InstallSource, PackageFrontend};

/// How this installation should be updated, for the frontend's update UI.
///
/// This is deliberately independent of the `updater` feature: development
/// builds compile the no-op updater, but the settings panel still needs to
/// know that, for example, an AUR-installed launcher is updated by pacman.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCapability {
    source: &'static str,
    /// Whether the launcher may update itself: either it owns its files or it
    /// can drive the package manager that owns them.
    self_update: bool,
    /// Shell command users can run to update, when one can be named safely.
    command: Option<String>,
    /// Claimed package names when several databases claim this executable.
    owners: Vec<String>,
}

pub fn current() -> UpdateCapability {
    let source = theseus::detect_install_source();
    UpdateCapability {
        source: source.as_str(),
        self_update: source.self_update_supported(),
        command: update_command(source),
        owners: conflict_owners(source),
    }
}

fn update_command(source: &InstallSource) -> Option<String> {
    match source {
        InstallSource::Managed(package) => {
            if !is_safe_package_name(&package.name) {
                return None;
            }
            Some(match package.frontend? {
                PackageFrontend::Apt => {
                    format!("sudo apt install --only-upgrade {}", package.name)
                }
                PackageFrontend::Dnf => {
                    format!("sudo dnf upgrade {}", package.name)
                }
                PackageFrontend::Zypper => {
                    format!("sudo zypper update {}", package.name)
                }
                PackageFrontend::Pacman => {
                    format!("sudo pacman -S {}", package.name)
                }
                PackageFrontend::Yay => format!("yay -S {}", package.name),
                PackageFrontend::Paru => format!("paru -S {}", package.name),
            })
        }
        // The app ID comes from the environment, so only accept the
        // restricted character set Flatpak IDs may contain before showing a
        // command the user is likely to paste into a shell.
        InstallSource::Flatpak => Some(
            std::env::var("FLATPAK_ID")
                .ok()
                .filter(|id| is_valid_flatpak_id(id))
                .map(|id| format!("flatpak update {id}"))
                .unwrap_or_else(|| "flatpak update".to_string()),
        ),
        _ => None,
    }
}

fn conflict_owners(source: &InstallSource) -> Vec<String> {
    match source {
        InstallSource::Conflict(packages) => packages
            .iter()
            .map(|package| {
                format!("{} ({})", package.name, package.database.as_str())
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Package names are copied into a command the user is told to run, so
/// anything outside the character set package databases use is rejected.
fn is_safe_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+')
        })
}

fn is_valid_flatpak_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id.contains('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Reading the capability runs the local package-database queries, which can
/// take a few hundred milliseconds on a large system, so it runs off the main
/// thread.
#[tauri::command]
pub async fn get_update_capability() -> UpdateCapability {
    match tauri::async_runtime::spawn_blocking(current).await {
        Ok(capability) => capability,
        Err(error) => {
            tracing::warn!("Failed to read the update capability: {error}");
            UpdateCapability {
                source: "unknown",
                self_update: false,
                command: None,
                owners: Vec::new(),
            }
        }
    }
}
