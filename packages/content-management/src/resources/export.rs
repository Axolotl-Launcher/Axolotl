use serde::{Deserialize, Serialize};

use super::{
    ArtifactIntegrity, PlanId, RequestId, ResourceArtifact, ResourceIssueCode,
    SessionId, SourceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceExportFormat {
    Modrinth,
    GenericZip,
    CurseForge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPackageInfo {
    pub name: String,
    pub version: String,
    pub summary: String,
    pub author: Option<String>,
}

/// Relative to the actual game directory, never a provider's original filename.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFileSnapshot {
    pub relative_path: String,
    pub integrity: ArtifactIntegrity,
    pub revision: u32,
    pub identifiable: bool,
    #[serde(default)]
    pub modified_at: Option<u64>,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub category_selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCategory {
    pub id: String,
    pub default_selected: bool,
    pub paths: Vec<String>,
    pub children: Vec<ExportCategory>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub modified_at: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDirectorySnapshot {
    pub relative_path: String,
    pub modified_at: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceExportScan {
    pub session_id: SessionId,
    pub instance_id: String,
    pub instance_revision: u32,
    pub files: Vec<ExportFileSnapshot>,
    pub categories: Vec<ExportCategory>,
    pub excluded_paths: Vec<String>,
    pub directories: Vec<ExportDirectorySnapshot>,
    pub requirements: ExportGameRequirements,
}

/// A successful no-match response is distinct from a failed request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportSourceResolution {
    Matched {
        source: SourceId,
        artifacts: Vec<ResourceArtifact>,
    },
    NoMatch {
        source: SourceId,
    },
    Failed {
        source: SourceId,
        code: ResourceIssueCode,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResolvedFile {
    pub snapshot: ExportFileSnapshot,
    pub resolutions: Vec<ExportSourceResolution>,
    #[serde(default)]
    pub local_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportResolutionState {
    Refreshing,
    Ready,
    NeedsDecision,
    Canceled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResolutionSession {
    pub id: SessionId,
    pub request_id: RequestId,
    pub selection_revision: u32,
    pub instance_revision: u32,
    pub state: ExportResolutionState,
    pub files: Vec<ExportResolvedFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRefreshProgress {
    pub session_id: SessionId,
    pub request_id: RequestId,
    pub selection_revision: u32,
    pub completed: usize,
    pub total: usize,
    pub phase: String,
    #[serde(default)]
    pub overall_completed: usize,
    #[serde(default)]
    pub overall_total: usize,
    #[serde(default)]
    pub current_file_fraction: f64,
}

/// An explicit fallback is bound to the failed source and exact file snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEmbeddedFailureChoice {
    pub snapshot: ExportFileSnapshot,
    pub source: SourceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactEnvironmentRequirement {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEnvironment {
    pub client: ArtifactEnvironmentRequirement,
    pub server: ArtifactEnvironmentRequirement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportFileAction {
    Embed {
        snapshot: ExportFileSnapshot,
    },
    Download {
        snapshot: ExportFileSnapshot,
        artifact: ResourceArtifact,
        environment: ArtifactEnvironment,
    },
    CurseForgeReference {
        snapshot: ExportFileSnapshot,
        artifact: ResourceArtifact,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportGameRequirements {
    pub game_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

/// Execution uses this exact set. Format changes never select new versions,
/// add dependencies, or expand directories after the snapshot was reviewed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceExportPlan {
    pub schema_version: super::ResourceContractVersion,
    pub id: PlanId,
    pub revision: u32,
    pub session_id: SessionId,
    pub selection_revision: u32,
    pub instance_id: String,
    pub instance_revision: u32,
    pub format: ResourceExportFormat,
    pub package_info: ExportPackageInfo,
    pub requirements: ExportGameRequirements,
    pub files: Vec<ExportFileAction>,
    pub embedded_failure_choices: Vec<ExportEmbeddedFailureChoice>,
}
