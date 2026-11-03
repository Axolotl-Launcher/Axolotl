use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    ArtifactIntegrity, HistoryId, InstalledEntryId, PlanId, ResourceArtifact,
    ResourceDependencyKind, ResourceId, ResourceRef, ResourceType, SelectionId,
    SourceArtifactRef, SourceId, SourceReleaseRef,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InstallScope {
    Instance,
    World { relative_path: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResourceOwnership {
    UserAdded,
    PackManaged { pack_member_id: String },
    LocalDiscovered,
}

/// Entry identity is stable across renames, recognition, updates, and rollback.
/// It is independent of hashes, project identity, and installation scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledResource {
    pub entry_id: InstalledEntryId,
    pub instance_id: String,
    pub resource_id: Option<ResourceId>,
    pub artifact: Option<SourceArtifactRef>,
    pub known_artifacts: Vec<SourceArtifactRef>,
    pub integrity: ArtifactIntegrity,
    pub relative_path: String,
    pub original_file_name: Option<String>,
    pub resource_type: ResourceType,
    pub scope: InstallScope,
    pub enabled: bool,
    pub missing: bool,
    pub ownership: ResourceOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityState {
    Compatible,
    Incompatible,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityDimension {
    GameVersion,
    Loader,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityAssessment {
    pub game_version: CompatibilityState,
    pub loader: CompatibilityState,
    pub reasons: Vec<String>,
}

/// An exception applies only to this selection and frozen release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityException {
    pub selection_id: SelectionId,
    pub release: SourceReleaseRef,
    pub dimensions: Vec<CompatibilityDimension>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReleaseSelection {
    Automatic,
    Release { reference: SourceReleaseRef },
    Artifact { reference: SourceArtifactRef },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSelection {
    pub id: SelectionId,
    pub resource: ResourceRef,
    pub release: ReleaseSelection,
    pub resource_type: ResourceType,
    pub scope: InstallScope,
    pub entry_id: Option<InstalledEntryId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceOperationIntent {
    Install,
    Update,
    SwitchVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyNodeState {
    Satisfied,
    SelectedRoot,
    Install,
    Update,
    Disabled,
    Missing,
    Conflict,
    Unresolved,
    OptionalCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDependencyNode {
    pub id: SelectionId,
    /// Preserve each explicit choice when several roots share one resource node.
    #[serde(default)]
    pub root_selection_ids: Vec<SelectionId>,
    #[serde(default)]
    pub resource_type: Option<ResourceType>,
    pub resource: ResourceRef,
    pub scope: InstallScope,
    pub state: DependencyNodeState,
    pub artifact: Option<ResourceArtifact>,
    pub installed_entries: Vec<InstalledEntryId>,
    pub compatibility: CompatibilityAssessment,
    #[serde(default)]
    pub declared_compatibility: Option<super::ReleaseCompatibility>,
}

/// Preserve all parent requirements. Requiredness is derived from active paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDependencyEdge {
    pub parent: SelectionId,
    pub child: SelectionId,
    pub declared_by: SourceArtifactRef,
    pub kind: ResourceDependencyKind,
    #[serde(default)]
    pub release_selection: Option<ReleaseSelection>,
    pub version_constraint: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceActionKind {
    Install,
    Replace,
    Enable,
    Keep,
}

/// Actions are per actual file entry; shared resource nodes must not swallow
/// actions for duplicate files or data packs installed in different worlds.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntryAction {
    pub entry_id: InstalledEntryId,
    pub node_id: SelectionId,
    pub kind: ResourceActionKind,
    pub current: Option<InstalledResource>,
    pub target: ResourceArtifact,
    pub target_relative_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceIssueCode {
    SourceUnavailable,
    IdentityUnconfirmed,
    VersionConflict,
    CompatibilityUnknown,
    CompatibilityMismatch,
    ArtifactMismatch,
    InvalidPath,
    MissingDependency,
    DependencyAnalysisFailed,
    StaleRevision,
    ManualDownloadRequired,
    FileNotWritable,
    FileWriteSkipped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceConflictReason {
    ConflictingReleases,
    IncompatibleResource,
    IncompatibleInstalledDependent,
    DependencyVersion,
    VersionConstraints,
    InstalledDependent,
    InstalledDependencyDisabled,
    InstalledDependencyMissing,
    InstalledDependencyVersion,
    NotNewerRelease,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceConflictDetails {
    pub reason: ResourceConflictReason,
    pub subject: Option<String>,
    pub related: Option<String>,
    pub required: Option<String>,
    pub actual: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePlanIssue {
    pub code: ResourceIssueCode,
    pub selection_id: Option<SelectionId>,
    pub entry_id: Option<InstalledEntryId>,
    pub blocking: bool,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ResourceConflictDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ResourceIssueContext>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceIssueContext {
    pub dependency: bool,
    pub subject: Option<String>,
    pub source: Option<SourceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseSelectionBasis {
    FixedByUser,
    InstalledCompatible,
    ComparableVersion,
    PublishedAt,
    EquivalentPreferredSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSelectionReason {
    pub node_id: SelectionId,
    pub basis: ReleaseSelectionBasis,
}

/// A durable plan freezes everything execution needs, without cache lookups
/// or another automatic version selection after review or process restart.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceOperationPlan {
    pub schema_version: super::ResourceContractVersion,
    pub id: PlanId,
    pub revision: u32,
    pub instance_id: String,
    pub instance_revision: u64,
    pub intent: ResourceOperationIntent,
    pub selections: Vec<ResourceSelection>,
    pub active_roots: Vec<SelectionId>,
    pub nodes: Vec<ResourceDependencyNode>,
    pub edges: Vec<ResourceDependencyEdge>,
    pub actions: Vec<ResourceEntryAction>,
    pub issues: Vec<ResourcePlanIssue>,
    pub exceptions: Vec<CompatibilityException>,
    pub selection_reasons: Vec<ReleaseSelectionReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResourcePlanDecision {
    IgnoreDependencies,
    SkipUnwritableFiles,
    AddRequired {
        node_id: SelectionId,
    },
    AddOptional {
        node_id: SelectionId,
    },
    RemoveOptional {
        node_id: SelectionId,
    },
    RemoveRoot {
        selection_id: SelectionId,
    },
    AcceptCompatibility {
        exception: CompatibilityException,
    },
    SelectRelease {
        node_id: SelectionId,
        selection: ReleaseSelection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceHistoryEntry {
    pub id: HistoryId,
    pub entry_id: InstalledEntryId,
    pub instance_id: String,
    pub recorded_at: DateTime<Utc>,
    pub previous: InstalledResource,
    pub target: InstalledResource,
    pub previous_nodes: Vec<ResourceDependencyNode>,
    pub target_nodes: Vec<ResourceDependencyNode>,
    pub previous_dependencies: Vec<ResourceDependencyEdge>,
    pub target_dependencies: Vec<ResourceDependencyEdge>,
    pub backup_relative_path: Option<String>,
    pub backup_integrity: Option<ArtifactIntegrity>,
    pub backup_available: bool,
    #[serde(default)]
    pub backup_retired: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceHistoryCleanupPlan {
    pub id: PlanId,
    pub revision: u32,
    pub instance_id: String,
    pub instance_revision: u64,
    pub entries: Vec<ResourceHistoryEntry>,
    pub issues: Vec<ResourcePlanIssue>,
    #[serde(default)]
    pub files: Vec<ResourceBackupFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBackupFile {
    pub id: HistoryId,
    pub history_id: Option<HistoryId>,
    pub relative_path: String,
    #[serde(default)]
    pub original_file_name: Option<String>,
    pub integrity: ArtifactIntegrity,
    pub rollback_available: bool,
    pub recognized: bool,
    pub exists: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceHistoryCleanupResult {
    pub removed: Vec<HistoryId>,
    pub failures: Vec<(HistoryId, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_details_preserve_existing_plan_payloads_and_round_trip() {
        let legacy = serde_json::json!({
            "code": "versionConflict",
            "selectionId": "root",
            "entryId": null,
            "blocking": true,
            "message": "incompatible resource is active"
        });
        let mut issue: ResourcePlanIssue =
            serde_json::from_value(legacy.clone()).unwrap();
        assert!(issue.conflict.is_none());
        assert!(issue.context.is_none());
        assert_eq!(serde_json::to_value(&issue).unwrap(), legacy);
        issue.conflict = Some(ResourceConflictDetails {
            reason: ResourceConflictReason::IncompatibleResource,
            subject: Some("Canvas".into()),
            related: Some("Sodium.jar".into()),
            required: Some("*".into()),
            actual: None,
        });
        issue.context = Some(ResourceIssueContext {
            dependency: false,
            subject: Some("Canvas".into()),
            source: Some(SourceId::modrinth()),
        });
        let serialized = serde_json::to_value(&issue).unwrap();
        assert_eq!(serialized["conflict"]["reason"], "incompatibleResource");
        assert_eq!(
            serde_json::from_value::<ResourcePlanIssue>(serialized).unwrap(),
            issue
        );
    }
}
