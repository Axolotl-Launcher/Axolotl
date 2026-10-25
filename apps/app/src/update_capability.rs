use serde::Serialize;
use theseus::InstallSource;

/// How this installation should be updated, for the frontend's update UI.
///
/// This is deliberately independent of the `updater` feature: development
/// builds compile the no-op updater, but the settings panel still needs to
/// know that, for example, an AUR-installed launcher is updated by pacman.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCapability {
    source: &'static str,
    /// Whether the launcher may replace its own files. System-owned installs
    /// (pacman, Flatpak, Nix) must be updated by their package manager.
    self_update: bool,
    /// Shell command users can run to update, when one can be named safely.
    command: Option<String>,
}

pub fn current() -> UpdateCapability {
    let source = theseus::detect_install_source();
    UpdateCapability {
        source: source.as_str(),
        self_update: source.self_update_supported(),
        command: update_command(source),
    }
}

fn update_command(source: InstallSource) -> Option<String> {
    match source {
        InstallSource::Pacman => {
            Some("yay -S axolotl-launcher-bin".to_string())
        }
        InstallSource::Flatpak => Some(
            std::env::var("FLATPAK_ID")
                .ok()
                .filter(|id| !id.is_empty())
                .map(|id| format!("flatpak update {id}"))
                .unwrap_or_else(|| "flatpak update".to_string()),
        ),
        _ => None,
    }
}

#[tauri::command]
pub fn get_update_capability() -> UpdateCapability {
    current()
}
