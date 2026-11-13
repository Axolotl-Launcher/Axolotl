use std::path::{Path, PathBuf};

use serde::Deserialize;

#[cfg(target_os = "windows")]
pub fn read_pcl_registry() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey("SOFTWARE\\PCL").ok()?;
    let value: String = key.get_value("LaunchFolders").ok()?;
    tracing::debug!(raw = %value, "read_pcl_registry: read LaunchFolders from HKCU\\SOFTWARE\\PCL");
    Some(value)
}

#[cfg(not(target_os = "windows"))]
pub fn read_pcl_registry() -> Option<String> {
    None
}

#[derive(Debug, Deserialize)]
struct PclCeConfig {
    #[serde(rename = "LaunchFolders")]
    launch_folders: Option<String>,
}

fn read_pclce_config() -> Option<String> {
    let path = dirs::data_dir()?.join("PCLCE").join("config.v1.json");
    read_pclce_config_at(&path)
}

fn read_pclce_config_at(path: &Path) -> Option<String> {
    tracing::debug!(path = %path.display(), "read_pclce_config: attempting to read config file");
    let content = std::fs::read_to_string(path).inspect_err(|e| {
        tracing::debug!(path = %path.display(), error = %e, "read_pclce_config: failed to read file");
    }).ok()?;
    let config: PclCeConfig = serde_json::from_str(&content).inspect_err(|e| {
        tracing::debug!(path = %path.display(), error = %e, "read_pclce_config: failed to parse JSON");
    }).ok()?;
    let launch_folders = config.launch_folders.as_deref().unwrap_or("");
    tracing::debug!(launch_folders = %launch_folders, "read_pclce_config: parsed LaunchFolders");
    config.launch_folders
}

fn parse_pcl_folders(raw: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for entry in raw.split('|') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some((name, path)) = entry.split_once('>') {
            let path = PathBuf::from(path.trim());
            let exists = path.is_dir();
            tracing::debug!(
                entry = %entry,
                name = %name.trim(),
                path = %path.display(),
                exists = exists,
                "parse_pcl_folders: entry"
            );
            if exists {
                result.push((
                    name.trim().to_string(),
                    path.to_string_lossy().to_string(),
                ));
            }
        } else {
            tracing::debug!(entry = %entry, "parse_pcl_folders: malformed entry (no '>' separator)");
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    tracing::debug!(count = result.len(), raw = %raw, "parse_pcl_folders: done");
    result
}

pub fn config_exists() -> bool {
    let exists = read_pclce_config().is_some();
    tracing::debug!(exists = exists, "config_exists");
    exists
}

pub fn get_pcl_instances() -> Vec<(String, String)> {
    let raw = read_pcl_registry().unwrap_or_default();
    let instances = parse_pcl_folders(&raw);
    tracing::info!(count = instances.len(), "get_pcl_instances");
    instances
}

pub fn get_pclce_instances() -> Vec<(String, String)> {
    let Some(data_dir) = dirs::data_dir() else {
        return Vec::new();
    };
    get_pclce_instances_from_config(
        &data_dir.join("PCLCE").join("config.v1.json"),
    )
}

pub(super) fn get_pclce_instances_from_config(
    path: &Path,
) -> Vec<(String, String)> {
    let raw = read_pclce_config_at(path).unwrap_or_default();
    let instances = parse_pcl_folders(&raw);
    tracing::info!(count = instances.len(), "get_pclce_instances");
    instances
}

/// Checks if a `.minecraft` folder exists next to the launcher (i.e. at
/// `base_path/.minecraft`) and returns it as a `(name, path)` pair suitable
/// for merging into the GameDir list.
pub fn get_local_dotminecraft(base_path: &Path) -> Option<(String, String)> {
    let dot_mc = base_path.join(".minecraft");
    if dot_mc.is_dir() {
        tracing::debug!(
            path = %dot_mc.display(),
            "get_local_dotminecraft: found .minecraft next to launcher"
        );
        Some((
            ".minecraft".to_string(),
            dot_mc.to_string_lossy().to_string(),
        ))
    } else {
        None
    }
}
