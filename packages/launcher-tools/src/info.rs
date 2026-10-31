use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct LauncherInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
}

impl LauncherInfo {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            name: "Axolotl Launcher".into(),
            version: version.into(),
            platform: std::env::consts::OS.into(),
        }
    }
}
