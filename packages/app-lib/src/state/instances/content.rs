use crate::state::{
    ContentItemUpdate, ContentProvider, ContentProviderRef, License, Project,
    ProjectType, Version, VersionEnvironment,
};
use serde::{Deserialize, Serialize};

use super::ContentSourceKind;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItem {
    pub file_name: String,
    pub file_path: String,
    pub id: String,
    pub size: u64,
    pub enabled: bool,
    pub project_type: ProjectType,
    pub project: Option<ContentItemProject>,
    pub version: Option<ContentItemVersion>,
    pub owner: Option<ContentItemOwner>,
    pub update: Option<ContentItemUpdate>,
    pub date_added: Option<String>,
    pub provider_refs: Vec<ContentProviderRef>,
    pub origin_provider: Option<ContentProvider>,
    /// Available durable resource history or a real legacy update backup;
    /// `file_name` identifies the previous file displayed by the rollback action.
    pub rollback: Option<ContentItemRollback>,
    /// Version-level environment (client/server/singleplayer) from the
    /// Modrinth v3 version API. `None` when the file has no Modrinth
    /// version match (e.g. CurseForge-only content).
    pub environment: Option<VersionEnvironment>,
    /// Local content source kind (local file, CurseForge pack member, ...).
    pub source_kind: Option<ContentSourceKind>,
    /// True when the file is not linked to any online project (no Modrinth
    /// hash match and no CurseForge reference).
    pub external: bool,
    /// Loader derived from the installed version's loaders when a Modrinth
    /// match exists, falling back to the locally parsed mod metadata.
    pub loader: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemRollback {
    pub file_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemProject {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    pub license: Option<License>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemVersion {
    pub id: String,
    pub version_number: String,
    pub file_name: String,
    pub date_published: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemOwner {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OwnerType {
    User,
    Organization,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinkedModpackInfo {
    pub project: Project,
    pub version: Version,
    pub owner: Option<ContentItemOwner>,
    pub update: Option<ContentItemUpdate>,
    pub update_version: Option<Version>,
}
