use serde::Deserialize;

/// The two `mcmod.info` root formats accepted by legacy Forge.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum McmodInfo {
    Array(Vec<McmodInfoEntry>),
    Object {
        #[serde(rename = "modList")]
        mod_list: Vec<McmodInfoEntry>,
    },
}

impl McmodInfo {
    pub(crate) fn entries(self) -> Vec<McmodInfoEntry> {
        match self {
            Self::Array(entries) | Self::Object { mod_list: entries } => {
                entries
            }
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct McmodInfoEntry {
    pub modid: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    #[serde(default, alias = "authorList")]
    pub authors: Option<Vec<String>>,
    #[serde(rename = "logoFile")]
    pub logo_file: Option<String>,
    pub url: Option<String>,
    /// Minecraft version targeted by this mod (single version, not a range).
    pub mcversion: Option<String>,
    #[serde(default, rename = "useDependencyInformation")]
    pub use_dependency_information: bool,
    #[serde(default, rename = "requiredMods")]
    pub required_mods: Vec<String>,
}
