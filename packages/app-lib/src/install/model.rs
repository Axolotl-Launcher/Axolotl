use crate::api::curseforge::{
    CurseForgeInstallRequest, CurseForgeWorldInstallRequest,
};
use crate::api::pack::import::ImportLauncherType;
use crate::api::pack::install_from::{CreatePackInstance, CreatePackLocation};
use crate::state::{
    ContentDependencyEdge, ContentEntry, ContentProvider, ContentProviderRef,
    ContentUpdateCheck, InstanceFile, InstanceInstallStage, InstanceLink,
    InstanceMetadata, InstanceUpgradeAction, InstanceUpgradeEnvironment,
    InstanceUpgradeIssue, InstanceUpgradeIssueCode, InstanceUpgradeItem,
    InstanceUpgradeSolution, InstanceUpgradeSourceFile, ModLoader, PackMember,
};
use chrono::{DateTime, Utc};
use content_management::{ContentType, ResolutionPreferences};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

pub type InstallModpackPreview = CreatePackInstance;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallPauseReason {
    MissingRequiredContent {
        failed_files: u64,
        paths: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallContinuationState {
    InstallingPackToExistingInstance {
        disabled_project_ids: Vec<String>,
    },
    ChangeContent {
        #[serde(default)]
        version: u32,
        actions: Vec<ContentChangeAction>,
    },
}

pub const CONTENT_CHANGE_PLAN_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeTarget {
    pub content_id: String,
    pub target_release_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentChangeIntent {
    UpdateOne {
        content_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_release_id: Option<String>,
    },
    UpdateSelected {
        targets: Vec<ContentChangeTarget>,
    },
    UpdateAllUserAdded,
    SwitchVersion {
        content_id: String,
        target_release_id: String,
    },
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, Eq, PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentChangeOperation {
    #[default]
    Update,
    SwitchVersion,
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, Eq, PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentChangeActionStatus {
    #[default]
    Pending,
    Prepared,
    Downloaded,
    WaitingForUser,
    Applying,
    Completed,
    Skipped,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentChangeFileRole {
    Primary,
    Dependency,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentChangeFileIntegrity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeFile {
    pub id: String,
    pub role: ContentChangeFileRole,
    pub provider: ContentProvider,
    pub project_id: String,
    pub release_id: String,
    pub file_name: String,
    pub target_relative_path: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_download_url: Option<String>,
    #[serde(default)]
    pub integrity: ContentChangeFileIntegrity,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeDependency {
    pub parent_file_id: String,
    pub child_file_id: String,
    pub provider: ContentProvider,
    pub project_id: String,
    pub release_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::state::instances::ContentDependencyKind>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeAction {
    pub content_id: String,
    #[serde(default)]
    pub operation: ContentChangeOperation,
    pub provider: ContentProvider,
    pub project_id: Option<String>,
    pub expected_release_id: Option<String>,
    pub target_release_id: String,
    pub relative_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_provider_file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_provider_file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_relative_path: Option<String>,
    #[serde(default)]
    pub files: Vec<ContentChangeFile>,
    #[serde(default)]
    pub dependencies: Vec<ContentChangeDependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrinth_plan: Option<content_management::ResolveContentPlan>,
    #[serde(default)]
    pub status: ContentChangeActionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    // Read old continuations that only persisted a completion flag. New
    // continuations use `status`, but retaining this field keeps old jobs
    // retryable without changing their frozen target release.
    #[serde(default)]
    pub completed: bool,
}

impl ContentChangeAction {
    pub fn effective_status(&self) -> ContentChangeActionStatus {
        if self.completed && self.status == ContentChangeActionStatus::Pending {
            ContentChangeActionStatus::Completed
        } else {
            self.status
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.effective_status(),
            ContentChangeActionStatus::Completed
                | ContentChangeActionStatus::Skipped
        )
    }

    pub fn set_status(&mut self, status: ContentChangeActionStatus) {
        self.status = status;
        self.completed = matches!(
            status,
            ContentChangeActionStatus::Completed
                | ContentChangeActionStatus::Skipped
        );
        if status != ContentChangeActionStatus::Failed {
            self.error = None;
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeActionSnapshot {
    pub content_id: String,
    pub operation: ContentChangeOperation,
    pub target_release_id: String,
    pub final_relative_path: Option<String>,
    pub status: ContentChangeActionStatus,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ContentChangeSnapshot {
    pub intent: ContentChangeIntent,
    pub content_ids: Vec<String>,
    pub plan_version: Option<u32>,
    pub actions: Vec<ContentChangeActionSnapshot>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallJobState {
    pub schema_version: u32,
    pub request: InstallRequest,
    pub target: InstallTarget,
    pub cleanup: InstallCleanup,
    pub progress: InstallProgressState,
    pub paths: InstallJobPaths,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<InstallErrorContext>,
    #[serde(default)]
    pub events: Vec<InstallJobEvent>,
    /// Live request state used for progress snapshots. Download resumption is
    /// based on persisted events and filesystem state, not partial byte counts.
    #[serde(skip)]
    pub active_downloads: HashMap<String, ActiveDownloadState>,
    #[serde(default)]
    pub display: Option<InstallJobDisplay>,
    pub rollback: Option<InstallRollbackState>,
    pub error: Option<InstallErrorView>,
    #[serde(default)]
    pub rollback_error: Option<InstallErrorView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_reason: Option<InstallPauseReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<InstallContinuationState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_content: Option<MissingModpackContentState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_missing_content_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade_result: Option<InstanceUpgradeResult>,
    /// Incrementally maintained download items, mirroring the replay of
    /// `events`. Skipped during (de)serialization because it is derivable
    /// from `events`; rebuilt lazily on first access after a load.
    #[serde(skip)]
    download_items_cache: std::sync::Mutex<Option<Vec<DownloadItemSnapshot>>>,
    #[serde(skip)]
    summary_cache: std::sync::Mutex<Option<DownloadJobSummary>>,
}

impl Clone for InstallJobState {
    fn clone(&self) -> Self {
        Self {
            schema_version: self.schema_version,
            request: self.request.clone(),
            target: self.target.clone(),
            cleanup: self.cleanup.clone(),
            progress: self.progress.clone(),
            paths: self.paths.clone(),
            context: self.context.clone(),
            events: self.events.clone(),
            active_downloads: self.active_downloads.clone(),
            display: self.display.clone(),
            rollback: self.rollback.clone(),
            error: self.error.clone(),
            rollback_error: self.rollback_error.clone(),
            pause_reason: self.pause_reason.clone(),
            continuation: self.continuation.clone(),
            missing_content: self.missing_content.clone(),
            skipped_missing_content_paths: self
                .skipped_missing_content_paths
                .clone(),
            upgrade_result: self.upgrade_result.clone(),
            download_items_cache: std::sync::Mutex::new(None),
            summary_cache: std::sync::Mutex::new(None),
        }
    }
}

impl InstallJobState {
    pub fn new(request: InstallRequest) -> Self {
        let target = request.target();
        let cleanup = request.cleanup();
        let kind = request.kind();
        // Content installs do not create or prepare an instance. Start them in
        // the content phase so the download center never presents them as an
        // instance setup task while the worker is being scheduled.
        let phase = initial_phase_for_request(&request);

        Self {
            schema_version: 1,
            request,
            target,
            cleanup,
            progress: InstallProgressState {
                phase,
                progress: None,
                details: InstallPhaseDetails::Empty,
                parallel: None,
            },
            paths: InstallJobPaths::default(),
            context: None,
            events: vec![InstallJobEvent {
                at: Utc::now(),
                kind: InstallJobEventKind::JobQueued { kind },
            }],
            active_downloads: HashMap::new(),
            display: None,
            rollback: None,
            error: None,
            rollback_error: None,
            pause_reason: None,
            continuation: None,
            missing_content: None,
            skipped_missing_content_paths: Vec::new(),
            upgrade_result: None,
            download_items_cache: std::sync::Mutex::new(None),
            summary_cache: std::sync::Mutex::new(None),
        }
    }

    /// Invalidates the lazily built download item and summary caches.
    pub(crate) fn invalidate_download_caches(&mut self) {
        *self.download_items_cache.lock().unwrap() = None;
        *self.summary_cache.lock().unwrap() = None;
    }

    pub fn record_event(&mut self, kind: InstallJobEventKind) {
        if matches!(
            &kind,
            InstallJobEventKind::JobStarted
                | InstallJobEventKind::JobSucceeded { .. }
                | InstallJobEventKind::JobCanceled { .. }
                | InstallJobEventKind::WaitingForUser { .. }
        ) {
            self.active_downloads.clear();
        }
        self.events.push(InstallJobEvent {
            at: Utc::now(),
            kind,
        });
        self.invalidate_download_caches();
    }

    pub fn compact_transient_download_events(&mut self) {
        self.events.retain(|event| {
            !matches!(
                event.kind,
                InstallJobEventKind::DownloadRequestStarted { .. }
                    | InstallJobEventKind::DownloadRequestFinished { .. }
                    | InstallJobEventKind::DownloadRequestFailed { .. }
            )
        });
        self.invalidate_download_caches();
    }

    pub fn set_context(&mut self, context: Option<InstallErrorContext>) {
        self.context = context;
    }

    pub fn set_progress(
        &mut self,
        phase: InstallPhaseId,
        mut progress: Option<InstallProgress>,
        details: InstallPhaseDetails,
    ) {
        let same_phase = self.progress.phase == phase;
        if !same_phase
            || matches!(&self.progress.details, InstallPhaseDetails::Empty)
                && !matches!(&details, InstallPhaseDetails::Empty)
        {
            self.record_event(InstallJobEventKind::PhaseStarted {
                phase,
                details: details.clone(),
            });
        }

        if same_phase
            && let Some(new_progress) = &mut progress
            && let Some(old_progress) = &self.progress.progress
            && let (Some(old_secondary), Some(new_secondary)) =
                (&old_progress.secondary, &new_progress.secondary)
            && old_secondary.total == new_secondary.total
        {
            new_progress.secondary = Some(InstallProgressSecondary {
                current: old_secondary.current.max(new_secondary.current),
                total: new_secondary.total,
            });
        }
        self.progress.phase = phase;
        self.progress.progress = progress;
        self.progress.details = details;
    }
}

pub(crate) fn initial_phase_for_request(
    request: &InstallRequest,
) -> InstallPhaseId {
    match request {
        InstallRequest::InstallContent { .. }
        | InstallRequest::InstallCurseForgeContent { .. }
        | InstallRequest::InstallCurseForgeWorld { .. }
        | InstallRequest::InstallContentBatch { .. }
        | InstallRequest::ChangeContent { .. } => {
            InstallPhaseId::DownloadingContent
        }
        InstallRequest::ExportResource {
            resolution: Some(_),
            ..
        } => InstallPhaseId::Verifying,
        InstallRequest::ExportResource { .. } => InstallPhaseId::Exporting,
        _ => InstallPhaseId::PreparingInstance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ContentSet, ContentSetStatus, ContentSourceKind, Instance,
        InstanceLaunchOverrides, LauncherFeatureVersion, ReleaseChannel,
    };
    use chrono::TimeDelta;

    fn job_state() -> InstallJobState {
        InstallJobState::new(InstallRequest::CreateInstance {
            name: "Test".to_string(),
            game_version: "1.21.1".to_string(),
            loader: ModLoader::Vanilla,
            loader_version: None,
            adjuncts: Vec::new(),
            icon_path: None,
            link: InstanceLink::Unmanaged,
            game_dir_override: None,
        })
    }

    fn set_secondary_progress(
        job: &mut InstallJobState,
        phase: InstallPhaseId,
        current: u64,
        total: u64,
    ) {
        job.set_progress(
            phase,
            Some(InstallProgress {
                current: 0,
                total: 1,
                secondary: Some(InstallProgressSecondary { current, total }),
            }),
            InstallPhaseDetails::Empty,
        );
    }

    fn secondary_progress(job: &InstallJobState) -> (u64, u64) {
        let secondary = job
            .progress
            .progress
            .as_ref()
            .and_then(|progress| progress.secondary.as_ref())
            .unwrap();
        (secondary.current, secondary.total)
    }

    #[test]
    fn same_phase_same_secondary_total_keeps_progress_monotonic() {
        let mut job = job_state();
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingContent,
            268,
            300,
        );
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingContent,
            200,
            300,
        );

        assert_eq!(secondary_progress(&job), (268, 300));
    }

    #[test]
    fn different_phase_does_not_retain_secondary_progress() {
        let mut job = job_state();
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingContent,
            268,
            300,
        );
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingMinecraft,
            0,
            16,
        );

        assert_eq!(secondary_progress(&job), (0, 16));
    }

    #[test]
    fn same_phase_different_secondary_total_starts_new_counter() {
        let mut job = job_state();
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingContent,
            268,
            300,
        );
        set_secondary_progress(
            &mut job,
            InstallPhaseId::DownloadingContent,
            0,
            16,
        );

        assert_eq!(secondary_progress(&job), (0, 16));
    }

    #[test]
    fn legacy_rollback_state_defaults_missing_content_revision() {
        let now = Utc::now();
        let instance_id = "instance".to_string();
        let content_set_id = "content-set".to_string();
        let mut job =
            InstallJobState::new(InstallRequest::InstallExistingInstance {
                instance_id: instance_id.clone(),
                force: false,
            });
        job.rollback = Some(InstallRollbackState {
            instance: InstanceMetadata {
                synced_options: Default::default(),
                instance: Instance {
                    id: instance_id.clone(),
                    path: "instance".to_string(),
                    applied_content_set_id: Some(content_set_id.clone()),
                    install_stage: InstanceInstallStage::Installed,
                    launcher_feature_version:
                        LauncherFeatureVersion::MOST_RECENT,
                    update_channel: ReleaseChannel::Release,
                    name: "Test".to_string(),
                    icon_path: None,
                    symlink_target: None,
                    linked_launcher: None,
                    linked_launcher_root: None,
                    linked_dot_minecraft: None,
                    linked_version_id: None,
                    linked_version_json_path: None,
                    linked_game_dir_mode: None,
                    game_dir_override: None,
                    created: now,
                    modified: now,
                    last_played: None,
                    pinned_at: None,
                    submitted_time_played: 0,
                    recent_time_played: 0,
                },
                applied_content_set: ContentSet {
                    id: content_set_id,
                    instance_id: instance_id.clone(),
                    name: "Test".to_string(),
                    source_kind: ContentSourceKind::Local,
                    status: ContentSetStatus::Available,
                    game_version: "1.21.1".to_string(),
                    protocol_version: None,
                    loader: ModLoader::Vanilla,
                    loader_version: None,
                    revision: 7,
                    created: now,
                    modified: now,
                },
                link: InstanceLink::Unmanaged,
                groups: Vec::new(),
                launch_overrides: InstanceLaunchOverrides::empty(instance_id),
                loader_components: Vec::new(),
            },
            install_stage: InstanceInstallStage::Installed,
            content: None,
        });

        let mut legacy = serde_json::to_value(job).unwrap();
        legacy["rollback"]["instance"]["applied_content_set"]
            .as_object_mut()
            .unwrap()
            .remove("revision");

        let restored: InstallJobState = serde_json::from_value(legacy).unwrap();

        assert_eq!(
            restored
                .rollback
                .unwrap()
                .instance
                .applied_content_set
                .revision,
            0
        );
    }

    #[test]
    fn download_summary_uses_content_events_and_live_progress() {
        let mut job = job_state();
        job.record_event(InstallJobEventKind::ContentDownloadStarted {
            files: 3,
            bytes: Some(300),
        });
        job.record_event(InstallJobEventKind::ContentFileDownloadAttempt {
            path: "mods/a.jar".to_string(),
            bytes_total: Some(100),
            attempt: 2,
            max_attempts: 3,
        });
        job.record_event(InstallJobEventKind::DownloadRequestStarted {
            path: "mods/a.jar".to_string(),
            name: "a.jar".to_string(),
            url: "https://mod.mcimirror.top/data/a.jar".to_string(),
            source: "mcim".to_string(),
            bytes_total: Some(100),
            attempt: 1,
            max_attempts: 4,
        });
        job.record_event(InstallJobEventKind::ContentFileCompleted {
            path: "mods/a.jar".to_string(),
            bytes: 100,
        });
        job.record_event(InstallJobEventKind::ContentFileDownloadAttempt {
            path: "mods/manual.jar".to_string(),
            bytes_total: Some(120),
            attempt: 3,
            max_attempts: 3,
        });
        job.record_event(InstallJobEventKind::ContentFileSkipped {
            path: "mods/manual.jar".to_string(),
            reason: "manual download required".to_string(),
            project_id: Some("123".to_string()),
            version_id: Some("456".to_string()),
            manual_url: Some(
                "https://www.curseforge.com/minecraft/mc-mods/example"
                    .to_string(),
            ),
        });
        job.record_event(InstallJobEventKind::ContentFileFailed {
            path: "mods/failed.jar".to_string(),
            reason: "network request failed".to_string(),
            project_id: Some("789".to_string()),
            version_id: Some("987".to_string()),
        });
        job.set_progress(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 3,
                total: 3,
                secondary: Some(InstallProgressSecondary {
                    current: 220,
                    total: 300,
                }),
            }),
            InstallPhaseDetails::Empty,
        );

        let summary = job.download_summary();
        assert_eq!(summary.files_completed, 3);
        assert_eq!(summary.files_total, Some(3));
        assert_eq!(summary.bytes_downloaded, 220);
        assert_eq!(summary.bytes_total, Some(300));
        let items = job.download_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].status, DownloadItemStatus::Completed);
        assert_eq!(items[0].attempt, Some(1));
        assert_eq!(items[0].max_attempts, Some(4));
        assert_eq!(
            items[0].request_url.as_deref(),
            Some("https://mod.mcimirror.top/data/a.jar")
        );
        assert_eq!(items[0].source.as_deref(), Some("mcim"));
        assert_eq!(items[1].status, DownloadItemStatus::Skipped);
        assert_eq!(items[1].attempt, Some(3));
        assert_eq!(items[1].max_attempts, Some(3));
        assert_eq!(items[1].project_id.as_deref(), Some("123"));
        assert_eq!(items[1].version_id.as_deref(), Some("456"));
        assert!(items[1].manual_url.is_some());
        assert_eq!(items[2].status, DownloadItemStatus::Failed);
        assert_eq!(items[2].project_id.as_deref(), Some("789"));
        assert_eq!(items[2].version_id.as_deref(), Some("987"));
        assert!(items[2].manual_url.is_none());
    }

    #[test]
    fn required_content_items_are_queued_before_download_requests_start() {
        let mut job = job_state();
        job.record_event(InstallJobEventKind::ContentDownloadStarted {
            files: 2,
            bytes: Some(300),
        });
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: "mods/a.jar".to_string(),
            bytes_total: Some(100),
            max_attempts: 4,
        });
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: "mods/b.jar".to_string(),
            bytes_total: Some(200),
            max_attempts: 6,
        });

        let items = job.download_items();
        assert_eq!(items.len(), 2);
        assert!(
            items
                .iter()
                .all(|item| item.status == DownloadItemStatus::Queued)
        );
        assert!(items.iter().all(|item| item.attempt == Some(0)));
        assert_eq!(items[0].max_attempts, Some(4));
        assert_eq!(items[1].bytes_total, Some(200));
        assert!(items.iter().all(|item| item.request_url.is_none()));
        assert!(items.iter().all(|item| item.source.is_none()));
    }

    #[test]
    fn recovery_validation_mode_is_derived_from_typed_resume_history() {
        let mut job =
            InstallJobState::new(InstallRequest::CreateModpackInstance {
                location: CreatePackLocation::FromFile {
                    path: PathBuf::from("test.mrpack"),
                },
                post_install_edit: None,
            });
        job.missing_content = Some(MissingModpackContentState::default());
        job.set_progress(
            InstallPhaseId::DownloadingContent,
            None,
            InstallPhaseDetails::Empty,
        );
        assert_eq!(
            job.execution_mode(InstallJobStatus::Running),
            InstallJobExecutionMode::Normal
        );

        job.record_event(InstallJobEventKind::WaitingForUser {
            reason: InstallPauseReason::MissingRequiredContent {
                failed_files: 1,
                paths: vec!["mods/missing.jar".to_string()],
            },
        });
        assert_eq!(
            job.execution_mode(InstallJobStatus::WaitingForUser),
            InstallJobExecutionMode::Normal
        );
        job.record_event(InstallJobEventKind::JobQueued {
            kind: InstallJobKind::CreateModpackInstance,
        });
        assert_eq!(
            job.execution_mode(InstallJobStatus::Queued),
            InstallJobExecutionMode::RecoveryValidation
        );
        assert_eq!(
            job.execution_mode(InstallJobStatus::Running),
            InstallJobExecutionMode::RecoveryValidation
        );

        job.set_progress(
            InstallPhaseId::ExtractingOverrides,
            None,
            InstallPhaseDetails::Empty,
        );
        assert_eq!(
            job.execution_mode(InstallJobStatus::Running),
            InstallJobExecutionMode::Normal
        );
    }

    #[test]
    fn resumed_existing_file_moves_from_queued_to_verifying() {
        let mut job =
            InstallJobState::new(InstallRequest::CreateModpackInstance {
                location: CreatePackLocation::FromFile {
                    path: PathBuf::from("test.mrpack"),
                },
                post_install_edit: None,
            });
        let path = "mods/missing.jar".to_string();
        job.missing_content = Some(MissingModpackContentState::default());
        job.set_progress(
            InstallPhaseId::DownloadingContent,
            None,
            InstallPhaseDetails::Empty,
        );
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: path.clone(),
            bytes_total: Some(100),
            max_attempts: 4,
        });
        job.record_event(InstallJobEventKind::WaitingForUser {
            reason: InstallPauseReason::MissingRequiredContent {
                failed_files: 1,
                paths: vec![path.clone()],
            },
        });
        job.record_event(InstallJobEventKind::ContentFileRecovered {
            path: path.clone(),
            bytes: 100,
        });
        job.record_event(InstallJobEventKind::JobQueued {
            kind: InstallJobKind::CreateModpackInstance,
        });
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: path.clone(),
            bytes_total: Some(100),
            max_attempts: 4,
        });
        assert_eq!(job.download_items()[0].status, DownloadItemStatus::Queued);

        job.record_event(InstallJobEventKind::ContentFileVerificationStarted {
            path,
        });
        assert_eq!(
            job.execution_mode(InstallJobStatus::Running),
            InstallJobExecutionMode::RecoveryValidation
        );
        assert_eq!(
            job.download_items()[0].status,
            DownloadItemStatus::Verifying
        );
    }

    #[test]
    fn all_successful_required_content_items_are_completed() {
        let mut job = job_state();
        for (path, bytes) in [("mods/a.jar", 100), ("mods/b.jar", 200)] {
            job.record_event(InstallJobEventKind::ContentFileQueued {
                path: path.to_string(),
                bytes_total: Some(bytes),
                max_attempts: 4,
            });
            job.record_event(InstallJobEventKind::ContentFileCompleted {
                path: path.to_string(),
                bytes,
            });
        }

        let items = job.download_items();
        assert_eq!(items.len(), 2);
        assert!(
            items
                .iter()
                .all(|item| item.status == DownloadItemStatus::Completed)
        );
    }

    #[test]
    fn skipped_required_content_item_is_not_failed() {
        let mut job = job_state();
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: "mods/client-unsupported.jar".to_string(),
            bytes_total: Some(100),
            max_attempts: 2,
        });
        job.record_event(InstallJobEventKind::ContentFileSkipped {
            path: "mods/client-unsupported.jar".to_string(),
            reason: "unsupported on client".to_string(),
            project_id: None,
            version_id: None,
            manual_url: None,
        });

        let items = job.download_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, DownloadItemStatus::Skipped);
        assert_ne!(items[0].status, DownloadItemStatus::Failed);
    }

    #[test]
    fn multiple_required_content_failures_have_individual_failed_items() {
        let mut job = job_state();
        for path in ["mods/a.jar", "mods/b.jar", "mods/c.jar"] {
            job.record_event(InstallJobEventKind::ContentFileQueued {
                path: path.to_string(),
                bytes_total: Some(100),
                max_attempts: 4,
            });
        }
        for path in ["mods/a.jar", "mods/c.jar"] {
            job.record_event(InstallJobEventKind::ContentFileFailed {
                path: path.to_string(),
                reason: "download failed".to_string(),
                project_id: None,
                version_id: None,
            });
        }
        job.record_event(InstallJobEventKind::ContentFileCompleted {
            path: "mods/b.jar".to_string(),
            bytes: 100,
        });

        let items = job.download_items();
        assert_eq!(items.len(), 3);
        assert_eq!(
            items
                .iter()
                .filter(|item| item.status == DownloadItemStatus::Failed)
                .count(),
            2
        );
        assert_eq!(items[1].status, DownloadItemStatus::Completed);
    }

    #[test]
    fn request_events_create_items_for_java_and_minecraft_downloads() {
        let mut job = job_state();
        job.record_event(InstallJobEventKind::DownloadRequestStarted {
            path: "java/runtime-manifest.json".to_string(),
            name: "runtime-manifest.json".to_string(),
            url: "https://piston-meta.mojang.com/v1/runtime.json".to_string(),
            source: "official".to_string(),
            bytes_total: Some(1024),
            attempt: 1,
            max_attempts: 2,
        });
        job.record_event(InstallJobEventKind::DownloadRequestFinished {
            path: "java/runtime-manifest.json".to_string(),
            bytes: 1024,
        });

        let items = job.download_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "runtime-manifest.json");
        assert_eq!(items[0].status, DownloadItemStatus::Verifying);
        assert_eq!(items[0].bytes_downloaded, 1024);
        assert_eq!(items[0].bytes_total, Some(1024));
        assert_eq!(items[0].source.as_deref(), Some("official"));

        job.compact_transient_download_events();
        assert!(job.events.iter().all(|event| !matches!(
            event.kind,
            InstallJobEventKind::DownloadRequestStarted { .. }
                | InstallJobEventKind::DownloadRequestFinished { .. }
                | InstallJobEventKind::DownloadRequestFailed { .. }
        )));
    }

    #[test]
    fn active_downloads_expose_live_bytes_and_hide_stale_eta() {
        let mut job = job_state();
        job.set_progress(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: 400,
                total: 1_000,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        );
        job.active_downloads.insert(
            "client.jar".to_string(),
            ActiveDownloadState {
                name: "client.jar".to_string(),
                url: "https://piston-data.mojang.com/client.jar".to_string(),
                source: "official".to_string(),
                bytes_downloaded: 400,
                bytes_total: Some(1_000),
                attempt: 1,
                max_attempts: 3,
                status: DownloadItemStatus::Downloading,
                last_reported_bytes: 400,
                last_progress_at: Utc::now(),
                speed_bytes_per_second: Some(200),
                speed_sample_started_at: Utc::now(),
                speed_sample_started_bytes: 400,
            },
        );

        let items = job.download_items();
        assert_eq!(items[0].bytes_downloaded, 400);
        assert_eq!(items[0].status, DownloadItemStatus::Downloading);
        let summary = job.download_summary();
        assert_eq!(summary.speed_bytes_per_second, Some(200));
        assert_eq!(summary.eta_seconds, Some(3));

        job.active_downloads.get_mut("client.jar").unwrap().status =
            DownloadItemStatus::Verifying;
        let verifying = job.download_summary();
        assert_eq!(verifying.speed_bytes_per_second, None);
        assert_eq!(verifying.eta_seconds, None);
        job.active_downloads.get_mut("client.jar").unwrap().status =
            DownloadItemStatus::Downloading;

        job.active_downloads
            .get_mut("client.jar")
            .unwrap()
            .last_progress_at = Utc::now() - TimeDelta::seconds(4);
        let stalled = job.download_summary();
        assert_eq!(stalled.speed_bytes_per_second, None);
        assert_eq!(stalled.eta_seconds, None);
        job.record_event(InstallJobEventKind::JobCanceled {
            phase: InstallPhaseId::DownloadingMinecraft,
        });
        assert!(job.active_downloads.is_empty());
    }

    #[test]
    fn active_downloads_are_not_persisted() {
        let mut job = job_state();
        job.active_downloads.insert(
            "client.jar".to_string(),
            ActiveDownloadState {
                name: "client.jar".to_string(),
                url: "https://piston-data.mojang.com/client.jar".to_string(),
                source: "official".to_string(),
                bytes_downloaded: 400,
                bytes_total: Some(1_000),
                attempt: 1,
                max_attempts: 3,
                status: DownloadItemStatus::Downloading,
                last_reported_bytes: 400,
                last_progress_at: Utc::now(),
                speed_bytes_per_second: Some(200),
                speed_sample_started_at: Utc::now(),
                speed_sample_started_bytes: 400,
            },
        );

        let serialized = serde_json::to_value(&job).unwrap();
        assert!(serialized.get("active_downloads").is_none());

        let restored: InstallJobState =
            serde_json::from_value(serialized).unwrap();
        assert!(restored.active_downloads.is_empty());
    }

    #[test]
    fn canceling_a_job_clears_active_download_requests() {
        let mut job = job_state();
        job.record_event(InstallJobEventKind::DownloadRequestStarted {
            path: "mods/a.jar".to_string(),
            name: "a.jar".to_string(),
            url: "https://cdn.modrinth.com/data/a.jar".to_string(),
            source: "official".to_string(),
            bytes_total: Some(1024),
            attempt: 1,
            max_attempts: 2,
        });
        job.record_event(InstallJobEventKind::JobCanceled {
            phase: InstallPhaseId::DownloadingContent,
        });

        let items = job.download_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, DownloadItemStatus::Canceled);
    }

    #[test]
    fn minecraft_download_progress_includes_byte_details() {
        let mut job = job_state();
        job.set_progress(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: 125,
                total: 500,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        );

        let summary = job.download_summary();
        assert_eq!(summary.bytes_downloaded, 125);
        assert_eq!(summary.bytes_total, Some(500));
    }

    #[test]
    fn download_summary_tracks_metrics_and_java_progress() {
        let mut job = job_state();
        job.set_progress(
            InstallPhaseId::PreparingJava,
            Some(InstallProgress {
                current: 1_024,
                total: 2_048,
                secondary: None,
            }),
            InstallPhaseDetails::Java {
                major_version: 21,
                step: InstallJavaStep::Downloading,
            },
        );
        job.events.last_mut().unwrap().at = Utc::now() - TimeDelta::seconds(2);
        job.record_event(InstallJobEventKind::DownloadMetrics {
            source: "bmclapi".to_string(),
            fallback_count: u64::MAX,
        });
        job.record_event(InstallJobEventKind::DownloadMetrics {
            source: "official".to_string(),
            fallback_count: 1,
        });

        let summary = job.download_summary();
        assert_eq!(summary.bytes_downloaded, 1_024);
        assert_eq!(summary.bytes_total, Some(2_048));
        assert_eq!(summary.source.as_deref(), Some("official"));
        assert_eq!(summary.fallback_count, u64::MAX);
        assert_eq!(summary.speed_bytes_per_second, None);
        assert_eq!(summary.eta_seconds, None);
    }

    #[test]
    fn curseforge_instance_jobs_use_the_curseforge_provider() {
        let job = InstallJobState::new(InstallRequest::CreateInstance {
            name: "CurseForge pack".to_string(),
            game_version: "1.20.1".to_string(),
            loader: ModLoader::Forge,
            loader_version: Some("latest".to_string()),
            adjuncts: Vec::new(),
            icon_path: None,
            link: InstanceLink::CurseForgeModpack {
                project_id: "123".to_string(),
                version_id: "456".to_string(),
            },
            game_dir_override: None,
        });

        assert_eq!(job.provider(), InstallJobProvider::CurseForge);
    }

    #[test]
    fn curseforge_managed_modpack_updates_are_tracked_as_pack_install_jobs() {
        let job = InstallJobState::new(
            InstallRequest::UpdateManagedCurseForgeModpack {
                instance_id: "instance".to_string(),
                file_id: 456,
            },
        );

        assert_eq!(job.provider(), InstallJobProvider::CurseForge);
        assert_eq!(
            job.request.kind(),
            InstallJobKind::InstallPackToExistingInstance
        );
        assert!(job.request.completes_instance_install_stage());
        assert!(matches!(
            job.request.target(),
            InstallTarget::ExistingInstance { .. }
        ));
        assert!(matches!(
            job.request.cleanup(),
            InstallCleanup::RestoreExistingInstance { .. }
        ));
    }

    #[test]
    fn deleted_instance_is_exposed_by_download_snapshot_state() {
        let mut job = job_state();
        assert!(!job.instance_deleted());
        job.record_event(InstallJobEventKind::TargetInstanceDeleted {
            instance_id: "deleted-instance".to_string(),
        });
        assert!(job.instance_deleted());
    }

    #[test]
    fn canceling_and_waiting_statuses_round_trip() {
        for status in [
            InstallJobStatus::Canceling,
            InstallJobStatus::WaitingForUser,
        ] {
            assert_eq!(
                InstallJobStatus::from_stored_str(status.as_str()),
                status
            );
            assert!(!status.is_finished());
        }
    }

    #[test]
    fn waiting_job_state_round_trip_preserves_pause_and_file_items() {
        let mut job = job_state();
        for path in ["mods/complete.jar", "mods/missing.jar"] {
            job.record_event(InstallJobEventKind::ContentFileQueued {
                path: path.to_string(),
                bytes_total: Some(100),
                max_attempts: 4,
            });
        }
        job.record_event(InstallJobEventKind::ContentFileCompleted {
            path: "mods/complete.jar".to_string(),
            bytes: 100,
        });
        job.record_event(InstallJobEventKind::ContentFileFailed {
            path: "mods/missing.jar".to_string(),
            reason: "network failure".to_string(),
            project_id: None,
            version_id: None,
        });
        let reason = InstallPauseReason::MissingRequiredContent {
            failed_files: 1,
            paths: vec!["mods/missing.jar".to_string()],
        };
        job.pause_reason = Some(reason.clone());
        job.continuation =
            Some(InstallContinuationState::InstallingPackToExistingInstance {
                disabled_project_ids: vec!["project-a".to_string()],
            });
        job.record_event(InstallJobEventKind::WaitingForUser {
            reason: reason.clone(),
        });

        let restored: InstallJobState =
            serde_json::from_str(&serde_json::to_string(&job).unwrap())
                .unwrap();
        let items = restored.download_items();

        assert_eq!(restored.pause_reason, Some(reason));
        assert_eq!(restored.continuation, job.continuation);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].status, DownloadItemStatus::Completed);
        assert_eq!(items[1].status, DownloadItemStatus::Failed);
    }

    fn upgrade_execution() -> InstanceUpgradeExecution {
        let environment = InstanceUpgradeEnvironment {
            game_version: "1.21.1".to_string(),
            mod_loader: ModLoader::Fabric,
            mod_loader_version: Some("0.16.0".to_string()),
            shader_runtime: crate::state::ShaderRuntime::Iris,
        };
        InstanceUpgradeExecution {
            source_revision: 5,
            source_files: Vec::new(),
            source_environment: environment.clone(),
            target_environment: environment,
            items: Vec::new(),
            solution: InstanceUpgradeSolution {
                kind: crate::state::InstanceUpgradeSolutionKind::Custom,
                selections: Vec::new(),
                dependency_changes: Vec::new(),
                warnings: Vec::new(),
            },
            warnings: Vec::new(),
            source_watch: None,
        }
    }

    fn upgrade_request(mode: SharedUpgradeMode) -> InstallRequest {
        InstallRequest::UpgradeUnmanagedInstance {
            instance_id: "source".to_string(),
            plan_id: "plan".to_string(),
            execution: upgrade_execution(),
            create_full_backup: true,
            shared_upgrade_mode: mode,
            display_names: InstanceUpgradeDisplayNames::default(),
        }
    }

    #[test]
    fn instance_upgrade_direct_targets_existing_instance() {
        assert_eq!(
            upgrade_request(SharedUpgradeMode::Direct).target(),
            InstallTarget::ExistingInstance {
                instance_id: "source".to_string()
            }
        );
    }

    #[test]
    fn instance_upgrade_snapshot_source_identity_comes_from_request() {
        let job = InstallJobState::new(upgrade_request(
            SharedUpgradeMode::CopyAndUpgrade,
        ));
        assert_eq!(job.source_instance_id().as_deref(), Some("source"));
        let other = InstallJobState::new(InstallRequest::DownloadJava {
            vendor: "test".to_string(),
            version: 21,
        });
        assert_eq!(other.source_instance_id(), None);
    }

    #[test]
    fn instance_upgrade_direct_uses_technical_rollback() {
        assert_eq!(
            upgrade_request(SharedUpgradeMode::Direct).cleanup(),
            InstallCleanup::RestoreExistingInstance {
                instance_id: "source".to_string()
            }
        );
    }

    #[test]
    fn instance_upgrade_copy_targets_new_physical_instance() {
        assert_eq!(
            upgrade_request(SharedUpgradeMode::CopyAndUpgrade).target(),
            InstallTarget::NewInstance { instance_id: None }
        );
    }

    #[test]
    fn instance_upgrade_copy_failure_deletes_incomplete_copy() {
        assert_eq!(
            upgrade_request(SharedUpgradeMode::CopyAndUpgrade).cleanup(),
            InstallCleanup::DeleteNewInstance { instance_id: None }
        );
    }

    #[test]
    fn instance_upgrade_is_persistent_install_job_kind() {
        let request = upgrade_request(SharedUpgradeMode::Direct);
        assert_eq!(request.kind(), InstallJobKind::UpgradeUnmanagedInstance);
        assert_eq!(request.kind().as_str(), "upgrade_unmanaged_instance");
        assert_eq!(
            InstallJobKind::from_stored_str("upgrade_unmanaged_instance"),
            InstallJobKind::UpgradeUnmanagedInstance
        );
    }

    #[test]
    fn content_change_request_is_persistent_and_round_trips() {
        let request = InstallRequest::ChangeContent {
            instance_id: "instance".to_string(),
            intent: ContentChangeIntent::SwitchVersion {
                content_id: "entry".to_string(),
                target_release_id: "target".to_string(),
            },
            display_title: "Example".to_string(),
            display_icon: None,
        };
        assert_eq!(request.kind(), InstallJobKind::ChangeContent);
        assert_eq!(request.kind().as_str(), "change_content");
        assert_eq!(
            InstallJobKind::from_stored_str("change_content"),
            InstallJobKind::ChangeContent
        );
        assert_eq!(request.cleanup(), InstallCleanup::None);
        assert_eq!(
            request.target(),
            InstallTarget::ExistingInstance {
                instance_id: "instance".to_string()
            }
        );

        let restored: InstallRequest =
            serde_json::from_value(serde_json::to_value(&request).unwrap())
                .unwrap();
        assert!(matches!(
            restored,
            InstallRequest::ChangeContent {
                intent: ContentChangeIntent::SwitchVersion {
                    content_id,
                    target_release_id
                },
                ..
            } if content_id == "entry" && target_release_id == "target"
        ));
    }

    #[test]
    fn content_change_snapshot_uses_persisted_action_ids() {
        let mut state = InstallJobState::new(InstallRequest::ChangeContent {
            instance_id: "instance".to_string(),
            intent: ContentChangeIntent::UpdateAllUserAdded,
            display_title: "Update content".to_string(),
            display_icon: None,
        });
        assert_eq!(
            state.content_change().unwrap().content_ids,
            Vec::<String>::new()
        );
        state.continuation = Some(InstallContinuationState::ChangeContent {
            version: CONTENT_CHANGE_PLAN_VERSION,
            actions: vec![ContentChangeAction {
                content_id: "entry".to_string(),
                operation: ContentChangeOperation::Update,
                provider: ContentProvider::Modrinth,
                project_id: Some("project".to_string()),
                expected_release_id: Some("old".to_string()),
                target_release_id: "new".to_string(),
                relative_path: Some("mods/example.jar".to_string()),
                current_provider_file_name: Some("example.jar".to_string()),
                target_provider_file_name: Some("example-new.jar".to_string()),
                final_relative_path: Some("mods/example-new.jar".to_string()),
                files: Vec::new(),
                dependencies: Vec::new(),
                modrinth_plan: None,
                status: ContentChangeActionStatus::Completed,
                error: None,
                completed: true,
            }],
        });
        assert_eq!(
            state.content_change().unwrap().content_ids,
            vec!["entry".to_string()]
        );
        let restored: InstallJobState =
            serde_json::from_value(serde_json::to_value(&state).unwrap())
                .unwrap();
        assert!(matches!(
            restored.continuation,
            Some(InstallContinuationState::ChangeContent {
                version: CONTENT_CHANGE_PLAN_VERSION,
                actions,
            }) if actions[0].is_complete()
        ));
    }

    #[test]
    fn legacy_content_change_continuation_keeps_its_frozen_target() {
        let continuation: InstallContinuationState =
            serde_json::from_value(serde_json::json!({
                "type": "change_content",
                "actions": [{
                    "content_id": "entry",
                    "provider": "modrinth",
                    "project_id": "project",
                    "expected_release_id": "old",
                    "target_release_id": "frozen-target",
                    "relative_path": "mods/example.jar",
                    "completed": true
                }]
            }))
            .unwrap();

        let InstallContinuationState::ChangeContent { version, actions } =
            continuation
        else {
            panic!("wrong continuation variant");
        };
        assert_eq!(version, 0);
        assert_eq!(actions[0].target_release_id, "frozen-target");
        assert_eq!(
            actions[0].effective_status(),
            ContentChangeActionStatus::Completed
        );
    }

    #[test]
    fn instance_upgrade_completes_install_stage() {
        assert!(
            upgrade_request(SharedUpgradeMode::Direct)
                .completes_instance_install_stage()
        );
    }

    #[test]
    fn legacy_actions_keep_source_entry_and_pinned_target_identity() {
        let continuation: InstallContinuationState = serde_json::from_value(serde_json::json!({
            "type": "change_content",
            "actions": [
                {
                    "content_id": "mr-entry", "provider": "modrinth",
                    "project_id": "42", "expected_release_id": "6",
                    "target_release_id": "7", "relative_path": "mods/中文-one.jar.disabled",
                    "modrinth_plan": {
                        "primary": { "project_id": "42", "version_id": "7", "dependent_on_version_id": null },
                        "dependencies": [], "skipped": []
                    },
                    "completed": true
                },
                {
                    "content_id": "cf-entry", "provider": "curseforge",
                    "project_id": "42", "expected_release_id": "6",
                    "target_release_id": "7", "relative_path": "mods/中文-two.jar",
                    "completed": false
                }
            ]
        })).unwrap();
        let restored: InstallContinuationState = serde_json::from_value(
            serde_json::to_value(&continuation).unwrap(),
        )
        .unwrap();
        assert_eq!(restored, continuation);
        let InstallContinuationState::ChangeContent { version, actions } =
            restored
        else {
            panic!("wrong continuation variant");
        };
        assert_eq!(version, 0);
        assert_eq!(actions.len(), 2);
        assert_ne!(actions[0].content_id, actions[1].content_id);
        assert_eq!(actions[0].provider, ContentProvider::Modrinth);
        assert_eq!(actions[1].provider, ContentProvider::CurseForge);
        for action in &actions {
            assert_eq!(action.target_release_id, "7");
            assert!(action.files.is_empty());
        }
        assert_eq!(
            actions[0].relative_path.as_deref(),
            Some("mods/中文-one.jar.disabled")
        );
        assert!(actions[0].is_complete());
        assert!(!actions[1].is_complete());
        assert!(actions[0].modrinth_plan.as_ref().unwrap().primary.required);
        assert_eq!(
            actions[0]
                .modrinth_plan
                .as_ref()
                .unwrap()
                .primary
                .version_id,
            "7"
        );
    }

    #[test]
    fn legacy_install_request_keeps_exact_version_and_default_preferences() {
        let request: InstallRequest =
            serde_json::from_value(serde_json::json!({
                "type": "install_content", "instance_id": "instance",
                "project_id": "YL57xq9U", "version_id": "G5dd9TM4",
                "content_type": "mod", "display_title": "Iris"
            }))
            .unwrap();
        let restored: InstallRequest =
            serde_json::from_value(serde_json::to_value(&request).unwrap())
                .unwrap();
        let InstallRequest::InstallContent {
            project_id,
            version_id,
            selected,
            excluded_project_ids,
            display_icon,
            ..
        } = restored
        else {
            panic!("wrong install request variant");
        };
        assert_eq!(project_id, "YL57xq9U");
        assert_eq!(version_id.as_deref(), Some("G5dd9TM4"));
        assert_eq!(selected, ResolutionPreferences::default());
        assert!(excluded_project_ids.is_empty());
        assert!(display_icon.is_none());
    }

    #[test]
    fn instance_upgrade_request_round_trip_freezes_solution() {
        let mut request = upgrade_request(SharedUpgradeMode::Direct);
        let InstallRequest::UpgradeUnmanagedInstance { display_names, .. } =
            &mut request
        else {
            panic!("wrong request variant");
        };
        display_names.backup = Some("实例（升级前备份）".to_string());
        display_names.should_auto_rename = true;
        let value = serde_json::to_value(&request).unwrap();
        let restored: InstallRequest = serde_json::from_value(value).unwrap();
        let InstallRequest::UpgradeUnmanagedInstance {
            plan_id,
            execution,
            display_names,
            ..
        } = restored
        else {
            panic!("wrong request variant");
        };
        assert_eq!(plan_id, "plan");
        assert_eq!(execution.source_revision, 5);
        assert_eq!(display_names.backup.as_deref(), Some("实例（升级前备份）"));
        assert!(display_names.should_auto_rename);
        assert_eq!(
            execution.solution.kind,
            crate::state::InstanceUpgradeSolutionKind::Custom
        );
    }

    #[test]
    fn old_instance_upgrade_request_defaults_display_names() {
        let mut value =
            serde_json::to_value(upgrade_request(SharedUpgradeMode::Direct))
                .unwrap();
        value.as_object_mut().unwrap().remove("display_names");
        let restored: InstallRequest = serde_json::from_value(value).unwrap();
        let InstallRequest::UpgradeUnmanagedInstance { display_names, .. } =
            restored
        else {
            panic!("wrong request variant");
        };
        assert_eq!(display_names, InstanceUpgradeDisplayNames::default());
    }

    #[test]
    fn instance_upgrade_progress_phases_round_trip() {
        for phase in [
            InstallPhaseId::CreatingBackup,
            InstallPhaseId::StagingContent,
            InstallPhaseId::DownloadingContent,
            InstallPhaseId::ApplyingContent,
            InstallPhaseId::UpdatingLoader,
            InstallPhaseId::Verifying,
            InstallPhaseId::Completed,
        ] {
            assert_eq!(
                serde_json::from_value::<InstallPhaseId>(
                    serde_json::to_value(phase).unwrap()
                )
                .unwrap(),
                phase
            );
        }
    }

    #[test]
    fn instance_upgrade_external_change_event_round_trip() {
        let event = InstallJobEventKind::UpgradeExternalChange {
            relative_path: "mods/user.jar".to_string(),
            kind: InstanceUpgradeExternalChangeKind::Modified,
        };
        let restored: InstallJobEventKind =
            serde_json::from_value(serde_json::to_value(&event).unwrap())
                .unwrap();
        assert!(matches!(
            restored,
            InstallJobEventKind::UpgradeExternalChange {
                kind: InstanceUpgradeExternalChangeKind::Modified,
                ..
            }
        ));
    }

    #[test]
    fn instance_upgrade_result_round_trip_preserves_backup_and_skips() {
        let execution = upgrade_execution();
        let result = InstanceUpgradeResult {
            plan_id: "plan".to_string(),
            source_instance_id: "source".to_string(),
            target_instance_id: "target".to_string(),
            backup_instance_id: Some("backup".to_string()),
            source_environment: Some(execution.source_environment.clone()),
            target_environment: Some(execution.target_environment.clone()),
            solution: execution.solution,
            compatibility_warnings: Vec::new(),
            compatibility_warning_details: Vec::new(),
            external_changes: vec![InstanceUpgradeExternalChange {
                relative_path: "mods/user.jar".to_string(),
                kind: InstanceUpgradeExternalChangeKind::Added,
            }],
            skipped_due_to_external_conflict: vec!["mods/user.jar".to_string()],
        };
        let restored: InstanceUpgradeResult =
            serde_json::from_value(serde_json::to_value(&result).unwrap())
                .unwrap();
        assert_eq!(restored.backup_instance_id.as_deref(), Some("backup"));
        assert_eq!(
            restored.source_environment,
            Some(execution.source_environment)
        );
        assert_eq!(
            restored.target_environment,
            Some(execution.target_environment)
        );
        assert_eq!(restored.external_changes.len(), 1);
        assert_eq!(restored.skipped_due_to_external_conflict.len(), 1);
    }

    #[test]
    fn old_instance_upgrade_result_defaults_additive_fields() {
        let execution = upgrade_execution();
        let result = InstanceUpgradeResult {
            plan_id: "plan".to_string(),
            source_instance_id: "source".to_string(),
            target_instance_id: "target".to_string(),
            backup_instance_id: None,
            source_environment: Some(execution.source_environment),
            target_environment: Some(execution.target_environment),
            solution: execution.solution,
            compatibility_warnings: Vec::new(),
            compatibility_warning_details: Vec::new(),
            external_changes: Vec::new(),
            skipped_due_to_external_conflict: Vec::new(),
        };
        let mut value = serde_json::to_value(result).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("sourceEnvironment");
        object.remove("targetEnvironment");
        object.remove("compatibilityWarningDetails");

        let restored: InstanceUpgradeResult =
            serde_json::from_value(value).unwrap();
        assert_eq!(restored.source_environment, None);
        assert_eq!(restored.target_environment, None);
        assert!(restored.compatibility_warning_details.is_empty());
    }

    #[test]
    fn old_install_job_state_defaults_upgrade_result_to_none() {
        let job = job_state();
        let mut value = serde_json::to_value(job).unwrap();
        value.as_object_mut().unwrap().remove("upgrade_result");
        let restored: InstallJobState = serde_json::from_value(value).unwrap();
        assert!(restored.upgrade_result.is_none());
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallJobEvent {
    pub at: DateTime<Utc>,
    pub kind: InstallJobEventKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallInterruptReason {
    AppClosed,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallJobEventKind {
    JobQueued {
        kind: InstallJobKind,
    },
    JobStarted,
    JobSucceeded {
        instance_id: Option<String>,
    },
    JobCanceled {
        phase: InstallPhaseId,
    },
    WaitingForUser {
        reason: InstallPauseReason,
    },
    PhaseStarted {
        phase: InstallPhaseId,
        details: InstallPhaseDetails,
    },
    ContentDownloadStarted {
        files: u64,
        bytes: Option<u64>,
    },
    ContentFileQueued {
        path: String,
        bytes_total: Option<u64>,
        max_attempts: u32,
    },
    ContentFileBrowserOptions {
        path: String,
        urls: Vec<String>,
    },
    ContentFileVerificationStarted {
        path: String,
    },
    ContentFileWritingStarted {
        path: String,
    },
    ContentFileRecovered {
        path: String,
        bytes: u64,
    },
    ContentFileDownloadAttempt {
        path: String,
        bytes_total: Option<u64>,
        attempt: u32,
        max_attempts: u32,
    },
    DownloadRequestStarted {
        path: String,
        name: String,
        url: String,
        source: String,
        bytes_total: Option<u64>,
        attempt: u32,
        max_attempts: u32,
    },
    DownloadRequestFinished {
        path: String,
        bytes: u64,
    },
    DownloadRequestFailed {
        path: String,
    },
    ContentFileSkipped {
        path: String,
        reason: String,
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default)]
        version_id: Option<String>,
        #[serde(default)]
        manual_url: Option<String>,
    },
    ContentFileFailed {
        path: String,
        reason: String,
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default)]
        version_id: Option<String>,
    },
    ContentFileCompleted {
        path: String,
        bytes: u64,
    },
    DownloadMetrics {
        source: String,
        fallback_count: u64,
    },
    UpgradeExternalChange {
        relative_path: String,
        kind: InstanceUpgradeExternalChangeKind,
    },
    UpgradeItemSkipped {
        relative_path: String,
        reason: String,
    },
    TargetInstanceDeleted {
        instance_id: String,
    },
    Interrupted {
        reason: InstallInterruptReason,
        phase: InstallPhaseId,
    },
    Failed {
        phase: InstallPhaseId,
        code: String,
        message: String,
    },
    RollbackStarted {
        cleanup: InstallCleanup,
    },
    RollbackCompleted,
    RollbackFailed {
        message: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallRequest {
    CreateInstance {
        name: String,
        game_version: String,
        loader: ModLoader,
        loader_version: Option<String>,
        #[serde(default)]
        adjuncts: Vec<crate::state::LoaderComponent>,
        icon_path: Option<String>,
        link: InstanceLink,
        #[serde(default)]
        game_dir_override: Option<String>,
    },
    CreateModpackInstance {
        location: CreatePackLocation,
        #[serde(default)]
        post_install_edit: Option<InstallPostInstallEdit>,
    },
    ImportInstance {
        launcher_type: ImportLauncherType,
        base_path: PathBuf,
        instance_folder: String,
        #[serde(default)]
        instance_path: Option<String>,
        #[serde(default)]
        symlink: bool,
        #[serde(default)]
        game_version: Option<String>,
        #[serde(default)]
        loader: Option<ModLoader>,
        #[serde(default)]
        loader_version: Option<String>,
        #[serde(default)]
        game_dir_override: Option<String>,
    },
    DuplicateInstance {
        source_instance_id: String,
    },
    InstallExistingInstance {
        instance_id: String,
        force: bool,
    },
    UpgradeUnmanagedInstance {
        instance_id: String,
        plan_id: String,
        execution: InstanceUpgradeExecution,
        create_full_backup: bool,
        shared_upgrade_mode: SharedUpgradeMode,
        #[serde(default)]
        display_names: InstanceUpgradeDisplayNames,
    },
    InstallPackToExistingInstance {
        instance_id: String,
        location: CreatePackLocation,
        #[serde(default)]
        post_install_edit: Option<InstallPostInstallEdit>,
    },
    UpdateManagedCurseForgeModpack {
        instance_id: String,
        file_id: u32,
    },
    InstallContent {
        instance_id: String,
        project_id: String,
        version_id: Option<String>,
        content_type: ContentType,
        #[serde(default)]
        selected: ResolutionPreferences,
        #[serde(default)]
        excluded_project_ids: Vec<String>,
        display_title: String,
        #[serde(default)]
        display_icon: Option<String>,
    },
    InstallCurseForgeContent {
        request: CurseForgeInstallRequest,
        display_title: String,
        #[serde(default)]
        display_icon: Option<String>,
    },
    InstallCurseForgeWorld {
        request: CurseForgeWorldInstallRequest,
        display_title: String,
        #[serde(default)]
        display_icon: Option<String>,
    },
    InstallContentBatch {
        instance_id: String,
        items: Vec<InstallContentBatchItem>,
        display_title: String,
        #[serde(default)]
        display_icon: Option<String>,
    },
    ChangeContent {
        instance_id: String,
        intent: ContentChangeIntent,
        display_title: String,
        #[serde(default)]
        display_icon: Option<String>,
    },
    ExportResource {
        plan: content_management::resources::ResourceExportPlan,
        output_path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolution: Option<
            Box<crate::resources::export::job::ExportResolutionCheckpoint>,
        >,
    },
    DownloadJava {
        vendor: String,
        version: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallContentBatchItem {
    Modrinth {
        project_id: String,
        version_id: Option<String>,
        content_type: ContentType,
        #[serde(default)]
        selected: ResolutionPreferences,
        #[serde(default)]
        excluded_project_ids: Vec<String>,
        #[serde(default)]
        force_project_ids: Vec<String>,
    },
    CurseForge {
        request: CurseForgeInstallRequest,
    },
    CurseForgeWorld {
        request: CurseForgeWorldInstallRequest,
    },
    /// A resource operation plan has already resolved every release and
    /// artifact. The worker must execute this snapshot without re-querying.
    FrozenResource {
        plan: content_management::resources::ResourceOperationPlan,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstallPostInstallEdit {
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with::rust::double_option"
    )]
    pub icon_path: Option<Option<String>>,
    pub link: Option<InstanceLink>,
}

impl InstallRequest {
    pub(crate) fn completes_instance_install_stage(&self) -> bool {
        match self {
            Self::CreateInstance { .. }
            | Self::CreateModpackInstance { .. }
            | Self::ImportInstance { .. }
            | Self::DuplicateInstance { .. }
            | Self::InstallExistingInstance { .. }
            | Self::UpgradeUnmanagedInstance { .. }
            | Self::InstallPackToExistingInstance { .. }
            | Self::UpdateManagedCurseForgeModpack { .. } => true,
            Self::InstallContent { .. }
            | Self::InstallCurseForgeContent { .. }
            | Self::InstallCurseForgeWorld { .. }
            | Self::InstallContentBatch { .. }
            | Self::ChangeContent { .. }
            | Self::ExportResource { .. }
            | Self::DownloadJava { .. } => false,
        }
    }

    pub fn kind(&self) -> InstallJobKind {
        match self {
            Self::CreateInstance { .. } => InstallJobKind::CreateInstance,
            Self::CreateModpackInstance { .. } => {
                InstallJobKind::CreateModpackInstance
            }
            Self::ImportInstance { .. } => InstallJobKind::ImportInstance,
            Self::DuplicateInstance { .. } => InstallJobKind::DuplicateInstance,
            Self::InstallExistingInstance { .. } => {
                InstallJobKind::InstallExistingInstance
            }
            Self::UpgradeUnmanagedInstance { .. } => {
                InstallJobKind::UpgradeUnmanagedInstance
            }
            Self::InstallPackToExistingInstance { .. } => {
                InstallJobKind::InstallPackToExistingInstance
            }
            Self::UpdateManagedCurseForgeModpack { .. } => {
                InstallJobKind::InstallPackToExistingInstance
            }
            Self::InstallContent { .. } => InstallJobKind::InstallContent,
            Self::InstallCurseForgeContent { .. } => {
                InstallJobKind::InstallContent
            }
            Self::InstallCurseForgeWorld { .. } => {
                InstallJobKind::InstallContent
            }
            Self::InstallContentBatch { items, .. } => {
                if items.iter().any(|item| matches!(item, InstallContentBatchItem::FrozenResource { plan } if plan.intent != crate::resources::ResourceOperationIntent::Install)) {
                    InstallJobKind::ChangeContent
                } else {
                    InstallJobKind::InstallContent
                }
            },
            Self::ChangeContent { .. } => InstallJobKind::ChangeContent,
            Self::ExportResource { .. } => InstallJobKind::ExportResource,
            Self::DownloadJava { .. } => InstallJobKind::DownloadJava,
        }
    }

    pub fn target(&self) -> InstallTarget {
        match self {
            Self::UpgradeUnmanagedInstance {
                instance_id,
                shared_upgrade_mode,
                ..
            } => match shared_upgrade_mode {
                SharedUpgradeMode::Direct => InstallTarget::ExistingInstance {
                    instance_id: instance_id.clone(),
                },
                SharedUpgradeMode::CopyAndUpgrade => {
                    InstallTarget::NewInstance { instance_id: None }
                }
            },
            Self::InstallExistingInstance { instance_id, .. }
            | Self::InstallPackToExistingInstance { instance_id, .. }
            | Self::UpdateManagedCurseForgeModpack { instance_id, .. }
            | Self::InstallContent { instance_id, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: instance_id.clone(),
                }
            }
            Self::InstallCurseForgeContent { request, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: request.instance_id.clone(),
                }
            }
            Self::InstallCurseForgeWorld { request, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: request.instance_id.clone(),
                }
            }
            Self::InstallContentBatch { instance_id, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: instance_id.clone(),
                }
            }
            Self::ExportResource { plan, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: plan.instance_id.clone(),
                }
            }
            Self::ChangeContent { instance_id, .. } => {
                InstallTarget::ExistingInstance {
                    instance_id: instance_id.clone(),
                }
            }
            _ => InstallTarget::NewInstance { instance_id: None },
        }
    }

    pub fn cleanup(&self) -> InstallCleanup {
        match self {
            Self::UpgradeUnmanagedInstance {
                instance_id,
                shared_upgrade_mode,
                ..
            } => match shared_upgrade_mode {
                SharedUpgradeMode::Direct => {
                    InstallCleanup::RestoreExistingInstance {
                        instance_id: instance_id.clone(),
                    }
                }
                SharedUpgradeMode::CopyAndUpgrade => {
                    InstallCleanup::DeleteNewInstance { instance_id: None }
                }
            },
            Self::InstallExistingInstance { instance_id, .. }
            | Self::InstallPackToExistingInstance { instance_id, .. }
            | Self::UpdateManagedCurseForgeModpack { instance_id, .. } => {
                InstallCleanup::RestoreExistingInstance {
                    instance_id: instance_id.clone(),
                }
            }
            Self::InstallContent { .. } => InstallCleanup::None,
            Self::InstallCurseForgeContent { .. } => InstallCleanup::None,
            Self::InstallCurseForgeWorld { .. } => InstallCleanup::None,
            Self::InstallContentBatch { .. } => InstallCleanup::None,
            Self::ChangeContent { .. } => InstallCleanup::None,
            Self::ExportResource { .. } => InstallCleanup::None,
            _ => InstallCleanup::DeleteNewInstance { instance_id: None },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobKind {
    CreateInstance,
    CreateModpackInstance,
    ImportInstance,
    DuplicateInstance,
    InstallExistingInstance,
    UpgradeUnmanagedInstance,
    InstallPackToExistingInstance,
    InstallContent,
    ChangeContent,
    DownloadJava,
    ExportResource,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobProvider {
    Modrinth,
    CurseForge,
    Minecraft,
    Java,
    Application,
    Local,
}

impl InstallJobProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curse_forge",
            Self::Minecraft => "minecraft",
            Self::Java => "java",
            Self::Application => "application",
            Self::Local => "local",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadItemStatus {
    Queued,
    WorkerStarted,
    WaitingForResource,
    Connecting,
    Downloading,
    Verifying,
    Writing,
    Metadata,
    WaitingForDatabase,
    Finalizing,
    WaitingForUser,
    Completed,
    Skipped,
    Failed,
    Canceled,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActiveDownloadState {
    pub name: String,
    pub url: String,
    pub source: String,
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub status: DownloadItemStatus,
    #[serde(default)]
    pub last_reported_bytes: u64,
    pub last_progress_at: DateTime<Utc>,
    #[serde(default)]
    pub speed_bytes_per_second: Option<u64>,
    #[serde(default = "Utc::now")]
    pub speed_sample_started_at: DateTime<Utc>,
    #[serde(default)]
    pub speed_sample_started_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadItemSnapshot {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    pub status: DownloadItemStatus,
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    #[serde(default)]
    pub attempt: Option<u32>,
    #[serde(default)]
    pub max_attempts: Option<u32>,
    pub error: Option<String>,
    pub manual_url: Option<String>,
    pub request_url: Option<String>,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DownloadJobSummary {
    pub files_completed: u64,
    pub files_total: Option<u64>,
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub speed_bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
    pub source: Option<String>,
    pub fallback_count: u64,
}

impl InstallJobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateInstance => "create_instance",
            Self::CreateModpackInstance => "create_modpack_instance",
            Self::ImportInstance => "import_instance",
            Self::DuplicateInstance => "duplicate_instance",
            Self::InstallExistingInstance => "install_existing_instance",
            Self::UpgradeUnmanagedInstance => "upgrade_unmanaged_instance",
            Self::InstallPackToExistingInstance => {
                "install_pack_to_existing_instance"
            }
            Self::InstallContent => "install_content",
            Self::ChangeContent => "change_content",
            Self::ExportResource => "export_resource",
            Self::DownloadJava => "download_java",
        }
    }

    pub fn from_stored_str(value: &str) -> Self {
        match value {
            "create_modpack_instance" => Self::CreateModpackInstance,
            "import_instance" => Self::ImportInstance,
            "duplicate_instance" => Self::DuplicateInstance,
            "install_existing_instance" => Self::InstallExistingInstance,
            "upgrade_unmanaged_instance" => Self::UpgradeUnmanagedInstance,
            "install_pack_to_existing_instance" => {
                Self::InstallPackToExistingInstance
            }
            "install_content" => Self::InstallContent,
            "change_content" => Self::ChangeContent,
            "export_resource" => Self::ExportResource,
            "download_java" => Self::DownloadJava,
            _ => Self::CreateInstance,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobStatus {
    Queued,
    Running,
    Canceling,
    WaitingForUser,
    Succeeded,
    Failed,
    Interrupted,
    Canceled,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobExecutionMode {
    Normal,
    RecoveryValidation,
}

impl InstallJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Canceling => "canceling",
            Self::WaitingForUser => "waiting_for_user",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Canceled => "canceled",
        }
    }

    pub fn from_stored_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "canceling" => Self::Canceling,
            "waiting_for_user" => Self::WaitingForUser,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "interrupted" => Self::Interrupted,
            "canceled" => Self::Canceled,
            _ => Self::Queued,
        }
    }

    pub fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Interrupted | Self::Canceled
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallTarget {
    NewInstance { instance_id: Option<String> },
    ExistingInstance { instance_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallCleanup {
    DeleteNewInstance { instance_id: Option<String> },
    RestoreExistingInstance { instance_id: String },
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallProgressState {
    pub phase: InstallPhaseId,
    pub progress: Option<InstallProgress>,
    pub details: InstallPhaseDetails,
    /// A concurrently running secondary track (e.g. the Minecraft core
    /// download while modpack content is being installed). The frontend
    /// renders it as a separate progress bar alongside the main phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<InstallParallelProgress>,
}

/// Progress of a parallel install track that runs while the main phase
/// advances. Self-contained so the UI can render an independent bar.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallParallelProgress {
    pub phase: InstallPhaseId,
    pub current: u64,
    pub total: u64,
    pub details: InstallPhaseDetails,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallPhaseId {
    Exporting,
    PreparingInstance,
    CreatingBackup,
    StagingContent,
    ResolvingPack,
    DownloadingPackFile,
    ReadingPackManifest,
    DownloadingContent,
    ExtractingOverrides,
    ResolvingMinecraft,
    ResolvingLoader,
    PreparingJava,
    DownloadingMinecraft,
    RunningLoaderProcessors,
    ApplyingContent,
    UpdatingLoader,
    Verifying,
    Finalizing,
    Completed,
    RollingBack,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallProgress {
    pub current: u64,
    pub total: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<InstallProgressSecondary>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallProgressSecondary {
    pub current: u64,
    pub total: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJavaStep {
    Resolving,
    FetchingMetadata,
    Downloading,
    Extracting,
    Validating,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallPhaseDetails {
    Empty,
    Instance {
        name: String,
    },
    Minecraft {
        game_version: String,
        loader: ModLoader,
    },
    Java {
        major_version: u32,
        step: InstallJavaStep,
    },
    Modpack {
        project_id: Option<String>,
        version_id: Option<String>,
        title: Option<String>,
    },
    Import {
        launcher_type: ImportLauncherType,
        instance_folder: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstallJobPaths {
    pub staging_dir: Option<PathBuf>,
    pub final_instance_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug, bon::Builder)]
#[builder(start_fn = new)]
pub struct InstallErrorContext {
    #[builder(start_fn, into)]
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub target_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub entry_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub cache_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub sqlite_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub minecraft_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub loader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(into)]
    pub arch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallJobDisplay {
    pub title: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallRollbackState {
    pub instance: InstanceMetadata,
    pub install_stage: InstanceInstallStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<InstallContentRollbackSnapshot>,
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, Eq, PartialEq,
)]
#[serde(rename_all = "snake_case")]
pub enum InstallContentRollbackScope {
    #[default]
    PackManaged,
    AllContent,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallContentRollbackStage {
    Planned,
    Ready,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallContentRollbackSnapshot {
    pub staging_id: String,
    pub stage: InstallContentRollbackStage,
    #[serde(default)]
    pub scope: InstallContentRollbackScope,
    pub files: Vec<InstallRollbackFile>,
    pub entries: Vec<InstallRollbackContentEntry>,
    #[serde(default)]
    pub dependency_edges: Vec<ContentDependencyEdge>,
    pub pack_members: Vec<PackMember>,
    #[serde(default)]
    pub replacement_paths: Vec<String>,
    #[serde(default)]
    pub protected_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallRollbackFile {
    pub relative_path: String,
    pub staged_name: String,
    pub existed: bool,
    pub physical_sha1: Option<String>,
    pub physical_size: Option<u64>,
    pub instance_file: Option<InstanceFile>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallRollbackContentEntry {
    pub entry: ContentEntry,
    pub provider_refs: Vec<InstallRollbackProviderRef>,
    pub update_check: Option<ContentUpdateCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallRollbackProviderRef {
    pub provider_ref: ContentProviderRef,
    pub origin: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SharedUpgradeMode {
    Direct,
    CopyAndUpgrade,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeDisplayNames {
    #[serde(default)]
    pub backup: Option<String>,
    #[serde(default)]
    pub copy: Option<String>,
    #[serde(default)]
    pub upgraded_target: Option<String>,
    #[serde(default)]
    pub should_auto_rename: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeExecution {
    pub source_revision: u64,
    pub source_files: Vec<InstanceUpgradeSourceFile>,
    pub source_environment: InstanceUpgradeEnvironment,
    pub target_environment: InstanceUpgradeEnvironment,
    pub items: Vec<InstanceUpgradeItem>,
    pub solution: InstanceUpgradeSolution,
    pub warnings: Vec<InstanceUpgradeIssue>,
    #[serde(default)]
    pub source_watch: Option<InstanceUpgradeWatchBaseline>,
}

impl InstanceUpgradeExecution {
    pub(crate) fn final_physical_decision(
        &self,
        item: &InstanceUpgradeItem,
    ) -> (InstanceUpgradeAction, bool) {
        self.solution
            .selections
            .iter()
            .find(|selection| selection.content_id == item.content_id)
            .map(|selection| {
                (
                    selection.action,
                    selection.action != InstanceUpgradeAction::Disable
                        && selection.enabled,
                )
            })
            .unwrap_or_else(|| {
                (
                    item.resolution.action,
                    item.resolution.action != InstanceUpgradeAction::Disable
                        && item.current_enabled,
                )
            })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeWatchBaseline {
    pub epoch: u64,
    pub generation: u64,
    pub dirty_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceUpgradeExternalChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeExternalChange {
    pub relative_path: String,
    pub kind: InstanceUpgradeExternalChangeKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeCompatibilityWarning {
    pub code: InstanceUpgradeIssueCode,
    pub relative_path: Option<String>,
    pub content_id: Option<String>,
    pub provider: Option<ContentProvider>,
    pub project_id: Option<String>,
    pub conflicting_project_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpgradeResult {
    pub plan_id: String,
    pub source_instance_id: String,
    pub target_instance_id: String,
    pub backup_instance_id: Option<String>,
    #[serde(default)]
    pub source_environment: Option<InstanceUpgradeEnvironment>,
    #[serde(default)]
    pub target_environment: Option<InstanceUpgradeEnvironment>,
    pub solution: InstanceUpgradeSolution,
    pub compatibility_warnings: Vec<InstanceUpgradeIssue>,
    #[serde(default)]
    pub compatibility_warning_details: Vec<InstanceUpgradeCompatibilityWarning>,
    #[serde(default)]
    pub external_changes: Vec<InstanceUpgradeExternalChange>,
    #[serde(default)]
    pub skipped_due_to_external_conflict: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MissingModpackContentState {
    pub files: Vec<MissingModpackFileState>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MissingModpackFileState {
    pub item_id: String,
    pub manifest_path: String,
    pub target_path: String,
    pub expected_size: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub download_urls: Vec<String>,
    pub browser_urls: Vec<String>,
    #[serde(default)]
    pub validate_as_jar: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallErrorView {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<InstallPhaseId>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<InstallApiErrorDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<InstallErrorContext>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallApiErrorDetails {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

impl InstallErrorView {
    pub fn from_error(
        code: &str,
        phase: InstallPhaseId,
        error: &crate::Error,
        context: Option<InstallErrorContext>,
    ) -> Self {
        Self {
            code: code.to_string(),
            phase: Some(phase),
            message: error.to_string(),
            api: match error.raw.as_ref() {
                crate::ErrorKind::LabrinthError(error) => {
                    Some(InstallApiErrorDetails {
                        error: error.error.clone(),
                        status: error.status,
                        method: error.method.clone(),
                        url: error.url.clone(),
                        route: error.route.clone(),
                    })
                }
                crate::ErrorKind::HttpError {
                    status,
                    method,
                    url,
                } => Some(InstallApiErrorDetails {
                    error: "http_error".to_string(),
                    status: Some(*status),
                    method: Some(method.clone()),
                    url: Some(url.clone()),
                    route: None,
                }),
                _ => None,
            },
            context,
        }
    }

    pub fn from_message(
        code: &str,
        phase: InstallPhaseId,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            phase: Some(phase),
            message: message.into(),
            api: None,
            context: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallJobSnapshot {
    pub job_id: Uuid,
    pub instance_id: Option<String>,
    pub source_instance_id: Option<String>,
    pub instance_deleted: bool,
    pub kind: InstallJobKind,
    pub status: InstallJobStatus,
    pub execution_mode: InstallJobExecutionMode,
    pub provider: InstallJobProvider,
    pub target: InstallTarget,
    pub phase: InstallPhaseId,
    pub progress: Option<InstallProgress>,
    pub details: InstallPhaseDetails,
    pub parallel: Option<InstallParallelProgress>,
    pub display: Option<InstallJobDisplay>,
    pub error: Option<InstallErrorView>,
    pub rollback_error: Option<InstallErrorView>,
    pub pause_reason: Option<InstallPauseReason>,
    pub upgrade_result: Option<InstanceUpgradeResult>,
    pub content_change: Option<ContentChangeSnapshot>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub finished: Option<DateTime<Utc>>,
    pub summary: DownloadJobSummary,
    pub items: Vec<DownloadItemSnapshot>,
}

impl InstallJobState {
    pub fn content_change(&self) -> Option<ContentChangeSnapshot> {
        if let InstallRequest::InstallContentBatch { items, .. } = &self.request
            && let Some(plan) = items.iter().find_map(|item| match item { InstallContentBatchItem::FrozenResource { plan } if plan.intent != crate::resources::ResourceOperationIntent::Install => Some(plan), _ => None }) {
                let targets: Vec<_> = plan.actions.iter().filter(|action| action.current.is_some()).map(|action| ContentChangeTarget { content_id: action.entry_id.as_str().into(), target_release_id: action.target.reference.release.release_id.as_str().into() }).collect();
                let status = if self.events.iter().any(|event| matches!(event.kind, InstallJobEventKind::JobSucceeded { .. })) { ContentChangeActionStatus::Completed } else if self.error.is_some() { ContentChangeActionStatus::Failed } else { ContentChangeActionStatus::Prepared };
                return Some(ContentChangeSnapshot {
                    intent: if plan.intent == crate::resources::ResourceOperationIntent::SwitchVersion && !targets.is_empty() { ContentChangeIntent::SwitchVersion { content_id: targets[0].content_id.clone(), target_release_id: targets[0].target_release_id.clone() } } else { ContentChangeIntent::UpdateSelected { targets } },
                    content_ids: plan.actions.iter().map(|action| action.entry_id.as_str().into()).collect(),
                    plan_version: Some(plan.revision),
                    actions: plan.actions.iter().map(|action| ContentChangeActionSnapshot { content_id: action.entry_id.as_str().into(), operation: if plan.intent == crate::resources::ResourceOperationIntent::SwitchVersion { ContentChangeOperation::SwitchVersion } else { ContentChangeOperation::Update }, target_release_id: action.target.reference.release.release_id.as_str().into(), final_relative_path: Some(action.target_relative_path.clone()), status, error: self.error.as_ref().map(|error| error.message.clone()) }).collect(),
                });
            }
        let InstallRequest::ChangeContent { intent, .. } = &self.request else {
            return None;
        };
        let (content_ids, plan_version, actions) = match &self.continuation {
            Some(InstallContinuationState::ChangeContent {
                version,
                actions,
            }) => (
                actions
                    .iter()
                    .map(|action| action.content_id.clone())
                    .collect(),
                Some(*version),
                actions
                    .iter()
                    .map(|action| ContentChangeActionSnapshot {
                        content_id: action.content_id.clone(),
                        operation: action.operation,
                        target_release_id: action.target_release_id.clone(),
                        final_relative_path: action.final_relative_path.clone(),
                        status: action.effective_status(),
                        error: action.error.clone(),
                    })
                    .collect(),
            ),
            _ => match intent {
                ContentChangeIntent::UpdateOne { content_id, .. }
                | ContentChangeIntent::SwitchVersion { content_id, .. } => {
                    (vec![content_id.clone()], None, Vec::new())
                }
                ContentChangeIntent::UpdateSelected { targets } => (
                    targets
                        .iter()
                        .map(|target| target.content_id.clone())
                        .collect(),
                    None,
                    Vec::new(),
                ),
                ContentChangeIntent::UpdateAllUserAdded => {
                    (Vec::new(), None, Vec::new())
                }
            },
        };
        Some(ContentChangeSnapshot {
            intent: intent.clone(),
            content_ids,
            plan_version,
            actions,
        })
    }

    pub fn source_instance_id(&self) -> Option<String> {
        match &self.request {
            InstallRequest::UpgradeUnmanagedInstance {
                instance_id, ..
            } => Some(instance_id.clone()),
            _ => None,
        }
    }

    pub fn execution_mode(
        &self,
        status: InstallJobStatus,
    ) -> InstallJobExecutionMode {
        if !matches!(
            status,
            InstallJobStatus::Queued | InstallJobStatus::Running
        ) || self.progress.phase != InstallPhaseId::DownloadingContent
            || self.missing_content.is_none()
            || !matches!(
                &self.request,
                InstallRequest::CreateModpackInstance { .. }
                    | InstallRequest::InstallPackToExistingInstance { .. }
            )
        {
            return InstallJobExecutionMode::Normal;
        }

        let last_missing_pause = self.events.iter().rposition(|event| {
            matches!(
                &event.kind,
                InstallJobEventKind::WaitingForUser {
                    reason: InstallPauseReason::MissingRequiredContent { .. }
                }
            )
        });
        let last_queued = self.events.iter().rposition(|event| {
            matches!(&event.kind, InstallJobEventKind::JobQueued { .. })
        });
        if last_missing_pause
            .zip(last_queued)
            .is_some_and(|(paused, queued)| queued > paused)
        {
            InstallJobExecutionMode::RecoveryValidation
        } else {
            InstallJobExecutionMode::Normal
        }
    }

    pub fn instance_deleted(&self) -> bool {
        self.events.iter().any(|event| {
            matches!(
                event.kind,
                InstallJobEventKind::TargetInstanceDeleted { .. }
            )
        })
    }

    pub fn provider(&self) -> InstallJobProvider {
        match &self.request {
            InstallRequest::CreateModpackInstance { location, .. }
            | InstallRequest::InstallPackToExistingInstance {
                location, ..
            } => match location {
                CreatePackLocation::FromVersionId { .. } => {
                    InstallJobProvider::Modrinth
                }
                CreatePackLocation::FromFile { .. } => {
                    InstallJobProvider::Local
                }
            },
            InstallRequest::CreateInstance { link, .. } => match link {
                InstanceLink::CurseForgeModpack { .. } => {
                    InstallJobProvider::CurseForge
                }
                _ => InstallJobProvider::Minecraft,
            },
            InstallRequest::InstallExistingInstance { .. } => {
                InstallJobProvider::Minecraft
            }
            InstallRequest::UpgradeUnmanagedInstance { .. } => {
                InstallJobProvider::Application
            }
            InstallRequest::InstallContent { .. } => {
                InstallJobProvider::Modrinth
            }
            InstallRequest::ChangeContent { .. } => {
                InstallJobProvider::Application
            }
            InstallRequest::InstallContentBatch { items, .. } => {
                if items.iter().all(|item| {
                    matches!(item, InstallContentBatchItem::Modrinth { .. })
                }) {
                    InstallJobProvider::Modrinth
                } else if items.iter().all(|item| {
                    matches!(
                        item,
                        InstallContentBatchItem::CurseForge { .. }
                            | InstallContentBatchItem::CurseForgeWorld { .. }
                    )
                }) {
                    InstallJobProvider::CurseForge
                } else {
                    InstallJobProvider::Application
                }
            }
            InstallRequest::InstallCurseForgeContent { .. }
            | InstallRequest::InstallCurseForgeWorld { .. }
            | InstallRequest::UpdateManagedCurseForgeModpack { .. } => {
                InstallJobProvider::CurseForge
            }
            InstallRequest::ExportResource { .. } => InstallJobProvider::Local,
            InstallRequest::DownloadJava { .. } => InstallJobProvider::Java,
            InstallRequest::ImportInstance { .. }
            | InstallRequest::DuplicateInstance { .. } => {
                InstallJobProvider::Local
            }
        }
    }

    /// Replays `events` into download item snapshots. This is the expensive
    /// part of `download_items` and its result is cached; the cheap
    /// `active_downloads` overlay is applied live by the caller.
    fn download_items_from_events(&self) -> Vec<DownloadItemSnapshot> {
        let mut items = Vec::<DownloadItemSnapshot>::new();
        let mut indices = HashMap::<String, usize>::new();
        for event in &self.events {
            match &event.kind {
                InstallJobEventKind::ContentFileQueued {
                    path,
                    bytes_total,
                    max_attempts,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Queued;
                        item.bytes_downloaded = 0;
                        item.bytes_total = *bytes_total;
                        item.attempt = Some(0);
                        item.max_attempts = Some(*max_attempts);
                        item.error = None;
                        item.request_url = None;
                        item.source = None;
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: path.clone(),
                            project_id: None,
                            version_id: None,
                            status: DownloadItemStatus::Queued,
                            bytes_downloaded: 0,
                            bytes_total: *bytes_total,
                            attempt: Some(0),
                            max_attempts: Some(*max_attempts),
                            error: None,
                            manual_url: None,
                            request_url: None,
                            source: None,
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::ContentFileBrowserOptions {
                    path,
                    urls,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.manual_url = urls.first().cloned();
                    }
                }
                InstallJobEventKind::ContentFileVerificationStarted {
                    path,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Verifying;
                        item.error = None;
                    }
                }
                InstallJobEventKind::ContentFileWritingStarted { path } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Writing;
                        item.error = None;
                    }
                }
                InstallJobEventKind::ContentFileRecovered { path, bytes } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Completed;
                        item.bytes_downloaded = *bytes;
                        item.bytes_total = item.bytes_total.or(Some(*bytes));
                        item.error = None;
                    }
                }
                InstallJobEventKind::ContentFileDownloadAttempt {
                    path,
                    bytes_total,
                    attempt,
                    max_attempts,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Queued;
                        item.bytes_downloaded = 0;
                        item.bytes_total = *bytes_total;
                        item.attempt = Some(*attempt);
                        item.max_attempts = Some(*max_attempts);
                        item.error = None;
                        item.request_url = None;
                        item.source = None;
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: path.clone(),
                            project_id: None,
                            version_id: None,
                            status: DownloadItemStatus::Queued,
                            bytes_downloaded: 0,
                            bytes_total: *bytes_total,
                            attempt: Some(*attempt),
                            max_attempts: Some(*max_attempts),
                            error: None,
                            manual_url: None,
                            request_url: None,
                            source: None,
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::DownloadRequestStarted {
                    path,
                    name,
                    url,
                    source,
                    bytes_total,
                    attempt,
                    max_attempts,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Downloading;
                        item.bytes_total = item.bytes_total.or(*bytes_total);
                        item.attempt = Some(*attempt);
                        item.max_attempts = Some(*max_attempts);
                        item.error = None;
                        item.request_url = Some(url.clone());
                        item.source = Some(source.clone());
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: name.clone(),
                            project_id: None,
                            version_id: None,
                            status: DownloadItemStatus::Downloading,
                            bytes_downloaded: 0,
                            bytes_total: *bytes_total,
                            attempt: Some(*attempt),
                            max_attempts: Some(*max_attempts),
                            error: None,
                            manual_url: None,
                            request_url: Some(url.clone()),
                            source: Some(source.clone()),
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::DownloadRequestFinished {
                    path,
                    bytes,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        // Network transfer completion is not content
                        // finalization. `ContentFileCompleted` is the event
                        // that confirms verification and DB registration.
                        item.status = DownloadItemStatus::Verifying;
                        item.bytes_downloaded = *bytes;
                        item.bytes_total = item.bytes_total.or(Some(*bytes));
                    }
                }
                InstallJobEventKind::DownloadRequestFailed { path } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Failed;
                    }
                }
                InstallJobEventKind::ContentFileCompleted { path, bytes } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Completed;
                        item.bytes_downloaded = *bytes;
                        item.bytes_total = Some(*bytes);
                        item.error = None;
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: path.clone(),
                            project_id: None,
                            version_id: None,
                            status: DownloadItemStatus::Completed,
                            bytes_downloaded: *bytes,
                            bytes_total: Some(*bytes),
                            attempt: None,
                            max_attempts: None,
                            error: None,
                            manual_url: None,
                            request_url: None,
                            source: None,
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::ContentFileSkipped {
                    path,
                    reason,
                    project_id,
                    version_id,
                    manual_url,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Skipped;
                        item.bytes_downloaded = 0;
                        item.project_id = project_id.clone();
                        item.version_id = version_id.clone();
                        item.error = Some(reason.clone());
                        item.manual_url = manual_url.clone();
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: path.clone(),
                            project_id: project_id.clone(),
                            version_id: version_id.clone(),
                            status: DownloadItemStatus::Skipped,
                            bytes_downloaded: 0,
                            bytes_total: None,
                            attempt: None,
                            max_attempts: None,
                            error: Some(reason.clone()),
                            manual_url: manual_url.clone(),
                            request_url: None,
                            source: None,
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::ContentFileFailed {
                    path,
                    reason,
                    project_id,
                    version_id,
                } => {
                    if let Some(item) = indices
                        .get(path)
                        .and_then(|&index| items.get_mut(index))
                    {
                        item.status = DownloadItemStatus::Failed;
                        item.bytes_downloaded = 0;
                        item.project_id = project_id.clone();
                        item.version_id = version_id.clone();
                        item.error = Some(reason.clone());
                    } else {
                        let index = items.len();
                        items.push(DownloadItemSnapshot {
                            id: path.clone(),
                            name: path.clone(),
                            project_id: project_id.clone(),
                            version_id: version_id.clone(),
                            status: DownloadItemStatus::Failed,
                            bytes_downloaded: 0,
                            bytes_total: None,
                            attempt: None,
                            max_attempts: None,
                            error: Some(reason.clone()),
                            manual_url: None,
                            request_url: None,
                            source: None,
                        });
                        indices.insert(path.clone(), index);
                    }
                }
                InstallJobEventKind::JobCanceled { .. } => {
                    for item in &mut items {
                        if matches!(
                            item.status,
                            DownloadItemStatus::Queued
                                | DownloadItemStatus::WorkerStarted
                                | DownloadItemStatus::WaitingForResource
                                | DownloadItemStatus::Connecting
                                | DownloadItemStatus::Downloading
                                | DownloadItemStatus::Verifying
                                | DownloadItemStatus::Writing
                                | DownloadItemStatus::Metadata
                                | DownloadItemStatus::WaitingForDatabase
                                | DownloadItemStatus::Finalizing
                        ) {
                            item.status = DownloadItemStatus::Canceled;
                        }
                    }
                }
                _ => {}
            }
        }
        items
    }

    /// Applies the live `active_downloads` overlay on top of an event-replay
    /// snapshot, producing the final item list.
    fn overlay_active_downloads(
        &self,
        mut items: Vec<DownloadItemSnapshot>,
    ) -> Vec<DownloadItemSnapshot> {
        let mut indices = HashMap::<String, usize>::new();
        for (index, item) in items.iter().enumerate() {
            indices.insert(item.id.clone(), index);
        }
        let terminal = self.events.iter().rev().any(|event| {
            matches!(
                event.kind,
                InstallJobEventKind::JobSucceeded { .. }
                    | InstallJobEventKind::JobCanceled { .. }
            )
        });
        if !terminal {
            for (id, active) in &self.active_downloads {
                if let Some(item) =
                    indices.get(id).and_then(|&index| items.get_mut(index))
                {
                    item.name = active.name.clone();
                    item.status = active.status;
                    item.bytes_downloaded = active.bytes_downloaded;
                    item.bytes_total = active.bytes_total;
                    item.attempt = Some(active.attempt);
                    item.max_attempts = Some(active.max_attempts);
                    item.error = None;
                    item.request_url = Some(active.url.clone());
                    item.source = Some(active.source.clone());
                } else {
                    items.push(DownloadItemSnapshot {
                        id: id.clone(),
                        name: active.name.clone(),
                        project_id: None,
                        version_id: None,
                        status: active.status,
                        bytes_downloaded: active.bytes_downloaded,
                        bytes_total: active.bytes_total,
                        attempt: Some(active.attempt),
                        max_attempts: Some(active.max_attempts),
                        error: None,
                        manual_url: None,
                        request_url: Some(active.url.clone()),
                        source: Some(active.source.clone()),
                    });
                }
            }
        }
        items
    }

    pub fn download_items(&self) -> Vec<DownloadItemSnapshot> {
        let cached = self
            .download_items_cache
            .lock()
            .expect("download items cache lock poisoned")
            .clone();
        let from_events = match cached {
            Some(items) => items,
            None => {
                let items = self.download_items_from_events();
                *self
                    .download_items_cache
                    .lock()
                    .expect("download items cache lock poisoned") =
                    Some(items.clone());
                items
            }
        };
        self.overlay_active_downloads(from_events)
    }

    /// Replays `events` into the event-derived summary base (settled files,
    /// byte totals, source, fallback count). This is the expensive part of
    /// `download_summary` and its result is cached; live progress and speed
    /// are applied by the caller.
    fn summary_from_events(&self) -> DownloadJobSummary {
        let mut summary = DownloadJobSummary::default();
        let mut settled_content = HashMap::<String, u64>::new();
        for event in &self.events {
            match &event.kind {
                InstallJobEventKind::ContentDownloadStarted {
                    files,
                    bytes,
                } => {
                    summary.files_total = Some(*files);
                    summary.bytes_total = *bytes;
                }

                InstallJobEventKind::ContentFileCompleted { path, bytes }
                | InstallJobEventKind::ContentFileRecovered { path, bytes } => {
                    // 同一个文件在暂停/恢复后可能再次产生 settled 事件。
                    // 按 path 保存最终状态，避免重复累计。
                    settled_content.insert(path.clone(), *bytes);
                }

                InstallJobEventKind::ContentFileSkipped { path, .. }
                | InstallJobEventKind::ContentFileFailed { path, .. } => {
                    // Failed / Skipped 算作已处理，但不贡献完成字节。
                    // 如果之后恢复成功，同一个 path 会被 Recovered 覆盖成真实大小。
                    settled_content.insert(path.clone(), 0);
                }

                InstallJobEventKind::DownloadMetrics {
                    source,
                    fallback_count,
                } => {
                    summary.source = Some(source.clone());
                    summary.fallback_count =
                        summary.fallback_count.saturating_add(*fallback_count);
                }

                _ => {}
            }
        }
        summary.files_completed = settled_content.len() as u64;
        summary.bytes_downloaded = settled_content
            .values()
            .copied()
            .fold(0_u64, u64::saturating_add);
        summary
    }

    pub fn download_summary(&self) -> DownloadJobSummary {
        let cached = self
            .summary_cache
            .lock()
            .expect("download summary cache lock poisoned")
            .clone();
        let mut summary = match cached {
            Some(summary) => summary,
            None => {
                let summary = self.summary_from_events();
                *self
                    .summary_cache
                    .lock()
                    .expect("download summary cache lock poisoned") =
                    Some(summary.clone());
                summary
            }
        };
        if let Some(progress) = &self.progress.progress {
            if self.progress.phase == InstallPhaseId::DownloadingContent {
                summary.files_completed = progress.current;
                summary.files_total = Some(progress.total);

                if let Some(bytes) = &progress.secondary {
                    summary.bytes_downloaded = bytes.current;
                    summary.bytes_total = Some(bytes.total);
                }
            } else if self.progress.phase
                == InstallPhaseId::DownloadingMinecraft
                || self.progress.phase == InstallPhaseId::DownloadingPackFile
                || matches!(
                    &self.progress.details,
                    InstallPhaseDetails::Java {
                        step: InstallJavaStep::Downloading,
                        ..
                    }
                )
            {
                summary.bytes_downloaded = progress.current;
                summary.bytes_total = Some(progress.total);
            }
        }
        let actively_downloading = matches!(
            self.progress.phase,
            InstallPhaseId::DownloadingPackFile
                | InstallPhaseId::DownloadingContent
                | InstallPhaseId::DownloadingMinecraft
        ) || matches!(
            &self.progress.details,
            InstallPhaseDetails::Java {
                step: InstallJavaStep::Downloading,
                ..
            }
        );
        let now = Utc::now();
        let active_speed = self
            .active_downloads
            .values()
            .filter(|download| {
                download.status == DownloadItemStatus::Downloading
                    && now
                        .signed_duration_since(download.last_progress_at)
                        .num_milliseconds()
                        < 3_000
            })
            .filter_map(|download| download.speed_bytes_per_second)
            .fold(0_u64, u64::saturating_add);
        if actively_downloading && active_speed > 0 {
            summary.speed_bytes_per_second = Some(active_speed);
            summary.eta_seconds = summary.bytes_total.and_then(|total| {
                total
                    .saturating_sub(summary.bytes_downloaded)
                    .checked_add(active_speed - 1)
                    .and_then(|remaining| remaining.checked_div(active_speed))
            });
        }
        summary
    }
}
