use serde::{Deserialize, Serialize};

use crate::state::{ContentProvider, ModLoader, ProjectType};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderRuntime {
    Iris,
    OptiFine,
    None,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeEnvironment {
    pub game_version: String,
    pub mod_loader: ModLoader,
    pub mod_loader_version: Option<String>,
    pub shader_runtime: ShaderRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeItemStatus {
    UpgradeAvailable,
    AlreadyCompatible,
    NoCompatibleRelease,
    PrereleaseOnly,
    Unidentified,
    DependencyConflict,
    MissingRequiredDependency,
    IncompatibleDependency,
    UnsupportedContentType,
    NoCompatibleShaderRuntime,
    ShaderRuntimeMissing,
    ShaderRuntimeUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeAction {
    Upgrade,
    Keep,
    Disable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeResolution {
    pub content_id: String,
    pub action: InstanceUpgradeAction,
    #[serde(default)]
    pub allow_prerelease: bool,
    #[serde(default)]
    pub confirmed_prerelease_dependencies:
        Vec<InstanceUpgradePrereleaseConfirmation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeResolutionResult {
    pub content_id: String,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeResolutionBatchResult {
    pub plan: InstanceUpgradePlan,
    pub requested_count: usize,
    pub applied: Vec<InstanceUpgradeResolutionResult>,
    pub skipped: Vec<InstanceUpgradeResolutionResult>,
    pub failed: Vec<InstanceUpgradeResolutionResult>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradePrereleaseConfirmation {
    pub provider: ContentProvider,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeItem {
    pub content_id: String,
    pub relative_path: String,
    pub project_type: ProjectType,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub current_release_id: Option<String>,
    pub current_enabled: bool,
    pub auto_dependency: bool,
    pub status: InstanceUpgradeItemStatus,
    pub resolution: InstanceUpgradeResolution,
    pub candidate_release_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeIssueCode {
    PrereleaseOnly,
    Unidentified,
    DependencyConflict,
    MissingRequiredDependency,
    IncompatibleDependency,
    UnsupportedContentType,
    NoCompatibleRelease,
    NoCompatibleShaderRuntime,
    ShaderRuntimeMissing,
    ShaderRuntimeUnknown,
    SearchLimitReached,
    KeepIncompatible,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeIssue {
    pub code: InstanceUpgradeIssueCode,
    pub message: String,
    pub content_id: Option<String>,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub conflicting_project_id: Option<String>,
    #[serde(default)]
    pub dependency_requirements: Vec<InstanceUpgradeDependencyRequirement>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeDependencyRequirement {
    pub root_content_id: String,
    pub root_provider: ContentProvider,
    pub root_project_id: String,
    pub parent_provider: ContentProvider,
    pub parent_project_id: String,
    pub parent_release_id: String,
    pub dependency_provider: ContentProvider,
    pub dependency_project_id: String,
    pub required_release_id: Option<String>,
    pub candidate_release_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeDependencyChangeKind {
    Add,
    Upgrade,
    Keep,
    Remove,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeDependencyChange {
    /// Existing physical ContentEntry targeted by this change, when present.
    #[serde(default)]
    pub existing_content_id: Option<String>,
    pub provider: ContentProvider,
    pub project_id: String,
    pub current_release_id: Option<String>,
    pub target_release_id: Option<String>,
    pub kind: InstanceUpgradeDependencyChangeKind,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeSourceFile {
    pub relative_path: String,
    pub sha1: String,
    pub size: u64,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeSelection {
    pub content_id: String,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub current_release_id: Option<String>,
    pub target_release_id: Option<String>,
    pub action: InstanceUpgradeAction,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeSolutionKind {
    Newest,
    MinimalChange,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeSolution {
    pub kind: InstanceUpgradeSolutionKind,
    pub selections: Vec<InstanceUpgradeSelection>,
    pub dependency_changes: Vec<InstanceUpgradeDependencyChange>,
    pub warnings: Vec<InstanceUpgradeIssue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeSolutionChoice {
    Newest,
    MinimalChange,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeFixedConstraint {
    pub content_id: String,
    pub provider: ContentProvider,
    pub project_id: String,
    pub version_id: String,
}

/// Stages of creating an upgrade plan, in the order they run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradePlanningPhase {
    ReadingContent,
    CheckingCandidates,
    Solving,
    Validating,
}

/// One content item while its plan is still being created. `status` is
/// absent until the item has been checked against its candidate releases.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeItemProgress {
    pub content_id: String,
    pub relative_path: String,
    pub project_type: ProjectType,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub status: Option<InstanceUpgradeItemStatus>,
}

/// Reported while an upgrade plan is created. It only describes the work in
/// progress; the finished [`InstanceUpgradePlan`] is the result to act on.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstanceUpgradePlanningProgress {
    /// `total` is zero while the amount of work is not known yet.
    Phase {
        phase: InstanceUpgradePlanningPhase,
        completed: usize,
        total: usize,
    },
    /// The items being planned, first as identified and again once each has
    /// been checked against its candidate releases.
    Items {
        items: Vec<InstanceUpgradeItemProgress>,
    },
    /// Candidate releases of one project, a root or a dependency, were checked.
    /// `stable` counts compatible releases on the stable channel; whether a
    /// prerelease or shader runtime is acceptable is decided per item later.
    #[serde(rename_all = "camelCase")]
    Candidates {
        provider: ContentProvider,
        project_id: String,
        stable: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradePlan {
    pub id: String,
    pub instance_id: String,
    pub source_revision: u64,
    #[serde(default)]
    pub source_files: Vec<InstanceUpgradeSourceFile>,
    pub source_environment: InstanceUpgradeEnvironment,
    pub target_environment: InstanceUpgradeEnvironment,
    pub items: Vec<InstanceUpgradeItem>,
    pub dependency_changes: Vec<InstanceUpgradeDependencyChange>,
    pub warnings: Vec<InstanceUpgradeIssue>,
    pub blocking_issues: Vec<InstanceUpgradeIssue>,
    pub newest_solution: Option<InstanceUpgradeSolution>,
    pub minimal_change_solution: Option<InstanceUpgradeSolution>,
    pub selected_solution: Option<InstanceUpgradeSolution>,
    #[serde(default)]
    pub custom_constraints: Vec<InstanceUpgradeFixedConstraint>,
}
