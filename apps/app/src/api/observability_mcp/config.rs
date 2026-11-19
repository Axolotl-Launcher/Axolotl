use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};

/// Diagnostics MCP preferences. Separate from `mcp.json` so a user can open
/// one server without the other, or run both side by side.
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub struct Preferences {
    pub enabled: bool,
    #[serde(default)]
    pub require_auth: bool,
}

pub fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    theseus::DirectoryInfo::initial_settings_dir_path(&app.config().identifier)
        .map(|path| path.join("observability-mcp.json"))
        .ok_or_else(|| {
            "Cannot locate observability MCP settings directory".into()
        })
}

pub(super) fn save_preferences(
    app: &tauri::AppHandle,
    preferences: Preferences,
) -> Result<(), String> {
    let path = config_path(app)?;
    let parent = path
        .parent()
        .ok_or("Invalid observability MCP settings path")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| error.to_string())?;
    temp.write_all(
        &serde_json::to_vec(&preferences).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    temp.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temp.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Preferences;

    #[test]
    fn authentication_is_opt_in() {
        assert!(!Preferences::default().require_auth);
        let previous: Preferences =
            serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(!previous.require_auth);
        let explicit: Preferences =
            serde_json::from_str(r#"{"enabled":true,"require_auth":true}"#)
                .unwrap();
        assert!(explicit.require_auth);
    }
}
