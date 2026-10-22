use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("metadata provider error: {0}")]
    Provider(String),
    #[error("project `{0}` was not found")]
    ProjectNotFound(String),
    #[error("version `{0}` was not found")]
    VersionNotFound(String),
    #[error("version `{version_id}` does not belong to project `{project_id}`")]
    VersionProjectMismatch {
        version_id: String,
        project_id: String,
    },
    #[error("no compatible version was found for project `{0}`")]
    NoCompatibleVersion(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Mod,
    Plugin,
    DataPack,
    ResourcePack,
    Shader,
    ModPack,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolutionPreferences {
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolveContentRequest {
    pub project_id: String,
    pub version_id: Option<String>,
    pub content_type: ContentType,
    #[serde(default)]
    pub selected: ResolutionPreferences,
    #[serde(default)]
    pub target: ResolutionPreferences,
    #[serde(default)]
    pub existing_project_ids: Vec<String>,
    #[serde(default)]
    pub excluded_project_ids: Vec<String>,
    #[serde(default)]
    pub force_project_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolveContentPlan {
    pub primary: ResolvedContent,
    pub dependencies: Vec<ResolvedContent>,
    pub skipped: Vec<SkippedContent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedContent {
    pub project_id: String,
    pub version_id: String,
    pub dependent_on_version_id: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ContentMetadata>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkippedContent {
    pub project_id: String,
    pub version_id: Option<String>,
    pub dependent_on_version_id: Option<String>,
    pub reason: SkippedReason,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkippedReason {
    AlreadyInstalled,
    DuplicateProject,
    ConflictingDependency,
    NoCompatibleVersion,
    MissingVersion,
    QuiltFabricApi,
    ExcludedByUser,
    DependencyCycle,
    DependencyDepthExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub date_published: DateTime<Utc>,
    #[serde(default = "default_version_type")]
    pub version_type: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
}

fn default_version_type() -> String {
    "release".to_string()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}
