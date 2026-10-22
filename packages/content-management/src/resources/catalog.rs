use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    ArtifactIntegrity, ResourceId, ResourceRef, SourceArtifactRef, SourceId,
    SourceProjectRef, SourceReleaseRef,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceType {
    Mod,
    Plugin,
    DataPack,
    ResourcePack,
    Shader,
    ModPack,
    World,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceCapability {
    Search,
    Project,
    Description,
    Releases,
    IdentifyHash,
    IdentifyFingerprint,
    Dependencies,
    Download,
    Changelog,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSource {
    pub id: SourceId,
    pub types: Vec<ResourceType>,
    pub capabilities: Vec<SourceCapability>,
    pub available: bool,
    pub supported_search_sorts: Vec<super::ipc::ResourceSearchSort>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProjectMetadata {
    #[serde(default)]
    pub environments: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub gallery: Vec<String>,
    pub project: SourceProjectRef,
    pub slug: Option<String>,
    pub title: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub types: Vec<ResourceType>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub downloads: Option<u64>,
    pub followers: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
    /// When the project first became public, distinct from creating it or releasing a file.
    #[serde(default)]
    pub project_published_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub first_published_at: Option<DateTime<Utc>>,
    pub latest_published_at: Option<DateTime<Utc>>,
}

/// Includes only confirmed source projects; candidate groups live in search.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(default)]
    pub confirmed_projects: Vec<SourceProjectRef>,
    #[serde(default)]
    pub source_failures: Vec<super::ipc::ResourceSourceFailure>,
    pub id: Option<ResourceId>,
    pub default_project: SourceProjectRef,
    pub projects: Vec<SourceProjectMetadata>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextFormat {
    Markdown,
    Html,
    Plain,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceText {
    pub source: SourceId,
    pub format: TextFormat,
    pub body: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
    Unknown,
}

/// Empty compatibility metadata means unknown, not universal compatibility.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCompatibility {
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRelease {
    pub reference: SourceReleaseRef,
    pub version: String,
    pub channel: ReleaseChannel,
    pub published_at: Option<DateTime<Utc>>,
    pub compatibility: ReleaseCompatibility,
    pub artifacts: Vec<ResourceArtifact>,
    pub equivalent_releases: Vec<SourceReleaseRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactRole {
    Primary,
    Attachment,
    ServerPack,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ArtifactAcquisition {
    PublicDownload { urls: Vec<String> },
    ManualDownload { page_url: String },
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceDependencyKind {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DependencyTarget {
    SourceRelease {
        source: SourceId,
        #[serde(alias = "release_id")]
        release_id: super::SourceReleaseId,
    },
    FileName {
        name: String,
    },
    Resource {
        reference: ResourceRef,
    },
    Release {
        reference: SourceReleaseRef,
    },
    Artifact {
        reference: SourceArtifactRef,
    },
    LocalMod {
        id: String,
        constraint: Option<String>,
    },
}

/// Declarations belong to the exact selected artifact, not the latest project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDependency {
    pub target: DependencyTarget,
    pub kind: ResourceDependencyKind,
    pub version_constraint: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceArtifact {
    pub reference: SourceArtifactRef,
    pub file_name: Option<String>,
    pub role: ArtifactRole,
    pub available: Option<bool>,
    pub integrity: ArtifactIntegrity,
    pub acquisition: ArtifactAcquisition,
    pub dependencies: Vec<ArtifactDependency>,
}
