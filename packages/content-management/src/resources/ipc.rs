//! Typed IPC payloads. Commands are registered only when their services exist.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceSearchSort {
    ProjectPublished,
    Relevance,
    Downloads,
    Followers,
    FirstPublished,
    LatestPublished,
    Created,
    ProjectUpdated,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTarget {
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub channels: Vec<ReleaseChannel>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSearchQuery {
    #[serde(default)]
    pub filter: Option<ResourceSearchExpression>,
    pub text: String,
    pub types: Vec<ResourceType>,
    pub sources: Vec<SourceId>,
    pub target: ResourceTarget,
    pub categories: Vec<String>,
    pub sort: ResourceSearchSort,
    pub page_size: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceSearchExpression {
    All {
        terms: Vec<ResourceSearchExpression>,
    },
    Any {
        terms: Vec<ResourceSearchExpression>,
    },
    Predicate {
        field: String,
        values: Vec<String>,
        excluded: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TotalPrecision {
    Exact,
    Estimate,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTotal {
    pub value: Option<u64>,
    pub precision: TotalPrecision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSourceFailure {
    pub source: SourceId,
    pub code: ResourceIssueCode,
    pub message: String,
    pub retryable: bool,
}

/// The service owns per-source cursors and buffered results behind the opaque
/// cursor. Callers must not construct a shared platform offset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePage<T> {
    pub request_id: RequestId,
    pub entries: Vec<T>,
    pub cursor: Option<String>,
    pub has_more: bool,
    pub total: ResourceTotal,
    pub source_failures: Vec<ResourceSourceFailure>,
    pub exhausted_sources: Vec<SourceId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSearchResult {
    pub entry: ResourceSearchEntry,
    pub projects: Vec<SourceProjectMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceChangelog {
    pub release: SourceReleaseRef,
    pub published_at: Option<DateTime<Utc>>,
    pub text: ResourceText,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCacheInfo {
    pub schema_version: u32,
    pub size_bytes: u64,
    pub entry_count: u64,
}

/// Command names retain Tauri conventions; every argument and nested model
/// uses camelCase. This enum is a contract, not an executable dispatcher.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "command",
    content = "args",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ResourceCommand {
    ResourceSearch {
        query: ResourceSearchQuery,
        cursor: Option<String>,
        request_id: RequestId,
    },
    ResourceGet {
        reference: ResourceRef,
        source: Option<SourceId>,
        request_id: RequestId,
    },
    ResourceGetDescription {
        reference: ResourceRef,
        source: SourceId,
        request_id: RequestId,
    },
    ResourceListReleases {
        reference: ResourceRef,
        target: ResourceTarget,
        cursor: Option<String>,
        request_id: RequestId,
    },
    ResourceGetRelease {
        reference: SourceReleaseRef,
        request_id: RequestId,
    },
    ResourceGetChangelog {
        release_ref: SourceReleaseRef,
        request_id: RequestId,
    },
    ResourceListChangelogs {
        resource_ref: ResourceRef,
        cursor: Option<String>,
        request_id: RequestId,
    },
    ResourcePrepareOperation {
        instance_id: String,
        intent: ResourceOperationIntent,
        selections: Vec<ResourceSelection>,
        request_id: RequestId,
    },
    ResourceRevisePlan {
        plan_id: PlanId,
        revision: u32,
        decisions: Vec<ResourcePlanDecision>,
        request_id: RequestId,
    },
    ResourceQueuePlan {
        plan_id: PlanId,
        revision: u32,
    },
    ResourceListHistory {
        instance_id: String,
        entry_id: InstalledEntryId,
    },
    ResourceScanBackups {
        instance_id: String,
    },
    ResourcePrepareHistoryCleanup {
        instance_id: String,
        history_ids: Vec<HistoryId>,
    },
    ResourceExecuteHistoryCleanup {
        cleanup_plan_id: PlanId,
        revision: u32,
    },
    ResourceRollback {
        instance_id: String,
        entry_id: InstalledEntryId,
        history_id: HistoryId,
    },
    ResourceExportScan {
        instance_id: String,
        request_id: RequestId,
    },
    ResourceExportRefresh {
        instance_id: String,
        session_id: SessionId,
        selection_revision: u32,
        selected_files: Vec<ExportFileSnapshot>,
        request_id: RequestId,
    },
    ResourceExportRefreshRetry {
        session_id: SessionId,
        selection_revision: u32,
        failed_refs: Vec<ExportEmbeddedFailureChoice>,
        request_id: RequestId,
    },
    ResourceExportPlan {
        session_id: SessionId,
        selection_revision: u32,
        format: ResourceExportFormat,
        package_info: ExportPackageInfo,
        embedded_failure_choices: Vec<ExportEmbeddedFailureChoice>,
    },
    ResourceExportExecute {
        plan_id: PlanId,
        revision: u32,
        output_path: String,
    },
    ResourceExportCancel {
        request_id_or_job_id: String,
    },
    ResourceCacheInfo,
    ResourceCacheClear,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResourceCancellationTarget {
    Request { id: RequestId },
    Job { id: JobId },
}

/// Response variants pair with command names; queued work returns the existing
/// task system's job identity rather than introducing a second task model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "result", rename_all = "snake_case")]
pub enum ResourceResponse {
    ResourceSearch(ResourcePage<ResourceSearchResult>),
    ResourceGet(Resource),
    ResourceGetDescription(ResourceText),
    ResourceListReleases(ResourcePage<ResourceRelease>),
    ResourceGetRelease(ResourceRelease),
    ResourceGetChangelog(ResourceChangelog),
    ResourceListChangelogs(ResourcePage<ResourceChangelog>),
    ResourcePrepareOperation(ResourceOperationPlan),
    ResourceRevisePlan(ResourceOperationPlan),
    ResourceQueuePlan(ResourceJobSnapshot),
    ResourceListHistory(Vec<ResourceHistoryEntry>),
    ResourceScanBackups(Vec<ResourceBackupFile>),
    ResourcePrepareHistoryCleanup(ResourceHistoryCleanupPlan),
    ResourceExecuteHistoryCleanup(ResourceHistoryCleanupResult),
    ResourceRollback(ResourceJobSnapshot),
    ResourceExportScan(ResourceExportScan),
    ResourceExportRefresh(ExportResolutionSession),
    ResourceExportRefreshRetry(ExportResolutionSession),
    ResourceExportPlan(ResourceExportPlan),
    ResourceExportExecute(ResourceJobSnapshot),
    ResourceExportCancel,
    ResourceCacheInfo(ResourceCacheInfo),
    ResourceCacheClear,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceJobSnapshot {
    pub job_id: String,
    pub status: String,
    #[serde(flatten)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

/// Every asynchronous event carries request identity and its plan/selection
/// revision so a late response cannot overwrite a newer UI draft.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEvent {
    pub request_id: RequestId,
    pub plan_id: Option<PlanId>,
    pub session_id: Option<SessionId>,
    pub revision: u32,
    pub progress: Option<ResourceProgress>,
    pub source_failures: Vec<ResourceSourceFailure>,
    pub identity_status: Option<IdentityStatus>,
    pub error: Option<ResourceIssueCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceProgress {
    pub completed: u32,
    pub total: Option<u32>,
}
