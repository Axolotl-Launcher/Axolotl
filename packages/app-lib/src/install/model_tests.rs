use super::*;
use crate::state::{
    ContentSet, ContentSetStatus, ContentSourceKind, Instance,
    InstanceLaunchOverrides, LauncherFeatureVersion, ReleaseChannel,
};
use chrono::TimeDelta;

#[test]
fn modpack_game_directory_survives_request_persistence_and_old_jobs_default_to_managed()
 {
    let old = serde_json::json!({
        "type": "create_modpack_instance",
        "location": { "type": "fromFile", "path": "pack.mrpack" }
    });
    let old: InstallRequest = serde_json::from_value(old).unwrap();
    assert!(matches!(
        old,
        InstallRequest::CreateModpackInstance {
            game_dir_override: None,
            ..
        }
    ));
    let request = InstallRequest::CreateModpackInstance {
        location: CreatePackLocation::FromFile {
            path: "pack.mrpack".into(),
        },
        post_install_edit: None,
        game_dir_override: Some("/games/.minecraft/versions/Pack".into()),
    };
    let restored: InstallRequest =
        serde_json::from_str(&serde_json::to_string(&request).unwrap())
            .unwrap();
    assert!(
        matches!(restored, InstallRequest::CreateModpackInstance { game_dir_override: Some(path), .. } if path == "/games/.minecraft/versions/Pack")
    );
}

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
    set_secondary_progress(&mut job, InstallPhaseId::DownloadingContent, 0, 16);

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
                launcher_feature_version: LauncherFeatureVersion::MOST_RECENT,
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
            "https://www.curseforge.com/minecraft/mc-mods/example".to_string(),
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
    let mut job = InstallJobState::new(InstallRequest::CreateModpackInstance {
        location: CreatePackLocation::FromFile {
            path: PathBuf::from("test.mrpack"),
        },
        post_install_edit: None,
        game_dir_override: None,
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
    let mut job = InstallJobState::new(InstallRequest::CreateModpackInstance {
        location: CreatePackLocation::FromFile {
            path: PathBuf::from("test.mrpack"),
        },
        post_install_edit: None,
        game_dir_override: None,
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
fn settled_resource_files_clear_live_downloads() {
    for event in [
        InstallJobEventKind::ContentFileCompleted {
            path: "mods/test.jar".into(),
            bytes: 10,
        },
        InstallJobEventKind::ContentFileFailed {
            path: "mods/test.jar".into(),
            reason: "batch failed".into(),
            project_id: None,
            version_id: None,
        },
        InstallJobEventKind::ContentFileSkipped {
            path: "mods/test.jar".into(),
            reason: "read only".into(),
            project_id: None,
            version_id: None,
            manual_url: None,
        },
    ] {
        let mut job = job_state();
        job.active_downloads.insert(
            "mods/test.jar".into(),
            eta_test_download("mods/test.jar", 5, 10, 1),
        );
        job.record_event(event);
        assert!(job.active_downloads.is_empty());
        assert_ne!(
            job.download_items()[0].status,
            DownloadItemStatus::Downloading
        );
    }
}

fn eta_test_download(
    path: &str,
    bytes: u64,
    total: u64,
    speed: u64,
) -> ActiveDownloadState {
    ActiveDownloadState {
        name: path.to_string(),
        url: format!("https://example.com/{path}"),
        source: "official".to_string(),
        bytes_downloaded: bytes,
        bytes_total: Some(total),
        attempt: 1,
        max_attempts: 3,
        status: DownloadItemStatus::Downloading,
        last_reported_bytes: bytes,
        last_progress_at: Utc::now(),
        speed_bytes_per_second: Some(speed),
        speed_sample_started_at: Utc::now(),
        speed_sample_started_bytes: bytes,
    }
}

fn start_eta_test_batch(
    job: &mut InstallJobState,
    paths: &[&str],
    total: u64,
    settled: u64,
) {
    job.record_event(InstallJobEventKind::ContentDownloadStarted {
        files: paths.len() as u64,
        bytes: Some(total),
    });
    for path in paths {
        job.record_event(InstallJobEventKind::ContentFileQueued {
            path: (*path).to_string(),
            bytes_total: None,
            max_attempts: 3,
        });
    }
    job.set_progress(
        InstallPhaseId::DownloadingContent,
        Some(InstallProgress {
            current: 0,
            total: paths.len() as u64,
            secondary: Some(InstallProgressSecondary {
                current: settled,
                total,
            }),
        }),
        InstallPhaseDetails::Empty,
    );
}

#[test]
fn content_eta_counts_resumed_bytes_once_and_excludes_parallel_requests() {
    let mut job = job_state();
    start_eta_test_batch(
        &mut job,
        &["mods/done.jar", "mods/resumed.jar"],
        2_000,
        1_000,
    );
    job.record_event(InstallJobEventKind::ContentFileCompleted {
        path: "mods/done.jar".to_string(),
        bytes: 1_000,
    });
    for (path, bytes, total, speed) in [
        ("mods/done.jar", 1_500, 1_500, 900),
        ("mods/resumed.jar", 600, 1_000, 100),
        ("libraries/client.jar", 9_000, 10_000, 1_000),
    ] {
        job.active_downloads.insert(
            path.to_string(),
            eta_test_download(path, bytes, total, speed),
        );
    }
    let summary = job.download_summary();
    assert_eq!(summary.bytes_downloaded, 1_600);
    assert_eq!(summary.speed_bytes_per_second, Some(2_000));
    assert_eq!(summary.eta_seconds, Some(4));

    // Some callbacks already include the in-flight bytes. Taking the
    // maximum, rather than adding to that callback, avoids double counts.
    job.progress
        .progress
        .as_mut()
        .unwrap()
        .secondary
        .as_mut()
        .unwrap()
        .current = 1_600;
    let summary = job.download_summary();
    assert_eq!(summary.bytes_downloaded, 1_600);
    assert_eq!(summary.eta_seconds, Some(4));
}

#[test]
fn content_bytes_include_stalled_and_verifying_requests_without_speed() {
    for status in [
        DownloadItemStatus::Downloading,
        DownloadItemStatus::Verifying,
        DownloadItemStatus::Writing,
    ] {
        let mut job = job_state();
        start_eta_test_batch(&mut job, &["mods/a.jar"], 1_000, 0);
        let mut active = eta_test_download("mods/a.jar", 600, 1_000, 0);
        active.status = status;
        active.last_progress_at = Utc::now() - TimeDelta::seconds(4);
        active.speed_bytes_per_second = None;
        job.active_downloads
            .insert("mods/a.jar".to_string(), active);

        let summary = job.download_summary();
        assert_eq!(summary.bytes_downloaded, 600, "{status:?}");
        assert_eq!(summary.speed_bytes_per_second, None);
        assert_eq!(summary.eta_seconds, None);
    }
}

#[test]
fn content_bytes_replace_retry_offsets_and_do_not_count_settlement_twice() {
    let mut job = job_state();
    start_eta_test_batch(
        &mut job,
        &["mods/done.jar", "mods/retry.jar"],
        2_000,
        1_000,
    );
    job.record_event(InstallJobEventKind::ContentFileCompleted {
        path: "mods/done.jar".to_string(),
        bytes: 1_000,
    });
    for (attempt, bytes) in [(1, 600), (2, 0), (2, 400), (3, 700)] {
        let mut active = eta_test_download("mods/retry.jar", bytes, 1_000, 100);
        active.attempt = attempt;
        job.active_downloads.insert("mods/retry.jar".into(), active);
        assert_eq!(job.download_summary().bytes_downloaded, 1_000 + bytes);
    }
    job.record_event(InstallJobEventKind::ContentFileCompleted {
        path: "mods/retry.jar".to_string(),
        bytes: 1_000,
    });
    job.active_downloads.insert(
        "mods/retry.jar".into(),
        eta_test_download("mods/retry.jar", 1_000, 1_000, 100),
    );
    assert_eq!(job.download_summary().bytes_downloaded, 2_000);
}

#[test]
fn content_eta_hides_estimate_when_only_parallel_requests_are_fresh() {
    let mut job = job_state();
    start_eta_test_batch(&mut job, &["mods/a.jar"], 1_000, 0);
    let mut content = eta_test_download("mods/a.jar", 600, 1_000, 100);
    content.last_progress_at = Utc::now() - TimeDelta::seconds(4);
    job.active_downloads
        .insert("mods/a.jar".to_string(), content);
    job.active_downloads.insert(
        "libraries/client.jar".to_string(),
        eta_test_download("libraries/client.jar", 9_000, 10_000, 200),
    );
    let summary = job.download_summary();
    assert_eq!(summary.bytes_downloaded, 600);
    assert_eq!(summary.speed_bytes_per_second, Some(200));
    assert_eq!(summary.eta_seconds, None);

    job.active_downloads
        .get_mut("mods/a.jar")
        .unwrap()
        .last_progress_at = Utc::now();
    let first_summary = job.download_summary();
    assert_eq!(first_summary.bytes_downloaded, 600);
    assert_eq!(first_summary.eta_seconds, Some(4));
}

#[test]
fn content_eta_cache_tracks_new_batches_without_changing_clones() {
    let mut job = job_state();
    start_eta_test_batch(&mut job, &["mods/first.jar"], 1_000, 0);
    job.active_downloads.insert(
        "mods/first.jar".to_string(),
        eta_test_download("mods/first.jar", 600, 1_000, 100),
    );
    assert_eq!(job.download_summary().eta_seconds, Some(4));
    let cloned = job.clone();

    start_eta_test_batch(&mut job, &["mods/second.jar"], 2_000, 0);
    job.active_downloads.insert(
        "mods/second.jar".to_string(),
        eta_test_download("mods/second.jar", 500, 2_000, 100),
    );
    let summary = job.download_summary();
    assert_eq!(summary.bytes_downloaded, 500);
    assert_eq!(summary.eta_seconds, Some(15));
    let cloned_summary = cloned.download_summary();
    assert_eq!(cloned_summary.bytes_downloaded, 600);
    assert_eq!(cloned_summary.eta_seconds, Some(4));

    job.record_event(InstallJobEventKind::ContentFileCompleted {
        path: "mods/second.jar".to_string(),
        bytes: 2_000,
    });
    assert_eq!(job.download_summary().eta_seconds, None);

    job.active_downloads.clear();
    job.active_downloads.insert(
        "client.jar".to_string(),
        eta_test_download("client.jar", 400, 1_000, 200),
    );
    job.set_progress(
        InstallPhaseId::DownloadingMinecraft,
        Some(InstallProgress {
            current: 400,
            total: 1_000,
            secondary: None,
        }),
        InstallPhaseDetails::Empty,
    );
    assert_eq!(job.download_summary().eta_seconds, Some(3));
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

    let restored: InstallJobState = serde_json::from_value(serialized).unwrap();
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
    let job =
        InstallJobState::new(InstallRequest::UpdateManagedCurseForgeModpack {
            instance_id: "instance".to_string(),
            file_id: 456,
        });

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
        assert_eq!(InstallJobStatus::from_stored_str(status.as_str()), status);
        assert!(!status.is_finished());
    }
}

#[test]
fn every_status_is_listed_once() {
    let mut seen = Vec::new();
    for status in InstallJobStatus::ALL {
        assert!(!seen.contains(&status), "{status:?} is listed twice");
        seen.push(status);
    }
    assert_eq!(seen.len(), 8, "a new status must be added to ALL");
}

#[test]
fn the_guards_ask_for_the_statuses_that_still_occupy_an_instance() {
    assert_eq!(
        InstallJobStatus::unfinished(),
        ["queued", "running", "canceling", "waiting_for_user"]
    );
}

#[test]
fn listing_open_jobs_keeps_failed_and_interrupted_ones() {
    let open: Vec<_> = InstallJobStatus::ALL
        .into_iter()
        .filter(|status| !status.is_resolved())
        .collect();

    assert_eq!(
        open,
        [
            InstallJobStatus::Queued,
            InstallJobStatus::Running,
            InstallJobStatus::Canceling,
            InstallJobStatus::WaitingForUser,
            InstallJobStatus::Failed,
            InstallJobStatus::Interrupted,
        ]
    );
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
        serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
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
        serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
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
    let restored: InstallContinuationState =
        serde_json::from_value(serde_json::to_value(&continuation).unwrap())
            .unwrap();
    assert_eq!(restored, continuation);
    let InstallContinuationState::ChangeContent { version, actions } = restored
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
    let request: InstallRequest = serde_json::from_value(serde_json::json!({
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
        serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
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
        serde_json::from_value(serde_json::to_value(&result).unwrap()).unwrap();
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
