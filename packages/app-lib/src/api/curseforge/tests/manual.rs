//! CurseForge manual tests.

use super::fixtures::{
    assert_stage6_job_is_unresolved, create_stage6_instance,
    import_stage6_manual_download, insert_stage6_waiting_manual_job,
    reconcile_and_simulate_resume, skipped_curseforge_manual_item,
    stage6_manual_download, stage8_curseforge_file, stage8_integrity_metadata,
    stage8_legacy_manual_download, stage8_pending_keys,
    waiting_manual_job_record,
};
use crate::api::curseforge::dto::{
    CurseForgeFileHash, CurseForgeInstallRequest, CurseForgeManualDownload,
};
use crate::api::curseforge::manual::{
    CurseForgeManualDownloadIntegrityMetadata,
    curseforge_waiting_job_instance_ids,
    ensure_manual_download_integrity_metadata_with,
    find_manual_download_candidate, import_pending_manual_download_file,
    import_pending_manual_download_file_with_integrity_resolver,
    import_pending_manual_download_from_path,
    reconcile_curseforge_waiting_jobs_for_instance_with_state,
    reconcile_persisted_curseforge_waiting_jobs_with_resume,
    verify_manual_download_candidate_with_integrity,
};
use crate::api::curseforge::selection::{
    curseforge_file_page_url, persist_manual_download,
};
use crate::api::curseforge::verify::{
    compute_fingerprint, record_installed_curseforge_file_with_permit,
};
use crate::install::InstallJobEventKind;
use crate::state::{
    ContentProviderRef, ContentSourceKind, CurseForgeFileId,
    CurseForgeProjectId, InstanceLink, ModLoader, ProjectType,
};
use crate::util::fetch::sha1_file_async;
use crate::{ErrorKind, State};
use std::collections::HashSet;

#[test]
fn curseforge_final_manual_completion_recovers_and_resumes() {
    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
    let mut resume_count = 0;

    let result = reconcile_and_simulate_resume(
        &mut items,
        &HashSet::new(),
        &materialized,
        &mut resume_count,
    );

    assert_eq!(result.recovered.len(), 1);
    assert_eq!(result.materialized_exact_match_count, 1);
    assert_eq!(resume_count, 1);
    assert_eq!(
        items[0].status,
        crate::install::model::DownloadItemStatus::Completed
    );
}

#[test]
fn curseforge_missing_pending_without_materialization_does_not_resume() {
    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let mut resume_count = 0;

    let result = reconcile_and_simulate_resume(
        &mut items,
        &HashSet::new(),
        &HashSet::new(),
        &mut resume_count,
    );

    assert!(result.recovered.is_empty());
    assert_eq!(result.inconsistent.len(), 1);
    assert_eq!(
        result.inconsistent[0].reason,
        "pending_missing_but_not_materialized"
    );
    assert_eq!(resume_count, 0);
    assert_eq!(
        items[0].status,
        crate::install::model::DownloadItemStatus::Skipped
    );
}

#[test]
fn curseforge_pending_still_exists_does_not_resume() {
    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let pending = HashSet::from([("1".to_string(), "10".to_string())]);
    let mut resume_count = 0;

    let result = reconcile_and_simulate_resume(
        &mut items,
        &pending,
        &HashSet::new(),
        &mut resume_count,
    );

    assert!(result.recovered.is_empty());
    assert_eq!(result.unresolved_pending_count, 1);
    assert!(result.inconsistent.is_empty());
    assert_eq!(resume_count, 0);
}

#[test]
fn curseforge_partial_manual_completion_does_not_resume() {
    let mut items = vec![
        skipped_curseforge_manual_item("mods/a.jar", "1", "10"),
        skipped_curseforge_manual_item("mods/b.jar", "2", "20"),
        skipped_curseforge_manual_item("mods/c.jar", "3", "30"),
    ];
    let keys = [
        ("1".to_string(), "10".to_string()),
        ("2".to_string(), "20".to_string()),
        ("3".to_string(), "30".to_string()),
    ];
    let mut pending = keys.iter().cloned().collect::<HashSet<_>>();
    let mut materialized = HashSet::new();
    let mut resume_count = 0;

    for (index, key) in keys.into_iter().enumerate() {
        assert!(pending.remove(&key));
        assert!(materialized.insert(key));
        let result = reconcile_and_simulate_resume(
            &mut items,
            &pending,
            &materialized,
            &mut resume_count,
        );

        assert_eq!(result.recovered.len(), 1);
        assert!(items[..=index].iter().all(|item| {
            item.status == crate::install::model::DownloadItemStatus::Completed
        }));
        assert!(items[index + 1..].iter().all(|item| {
            item.status == crate::install::model::DownloadItemStatus::Skipped
        }));
        assert_eq!(resume_count, usize::from(index == 2));
    }
}

#[test]
fn curseforge_repeated_reconciliation_is_idempotent() {
    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
    let mut resume_count = 0;
    let mut waiting_for_user = true;
    let mut recovered_count = 0;

    for _ in 0..3 {
        if !waiting_for_user {
            continue;
        }
        let result = reconcile_and_simulate_resume(
            &mut items,
            &HashSet::new(),
            &materialized,
            &mut resume_count,
        );
        recovered_count += result.recovered.len();
        if resume_count > 0 {
            waiting_for_user = false;
        }
    }

    assert_eq!(recovered_count, 1);
    assert_eq!(resume_count, 1);
}

#[test]
fn curseforge_startup_selects_only_reconcilable_jobs() {
    let curseforge_job = waiting_manual_job_record(
        "curseforge-instance",
        crate::install::model::InstallRequest::InstallCurseForgeContent {
            request: CurseForgeInstallRequest {
                instance_id: "curseforge-instance".to_string(),
                project_id: 1,
                file_id: 10,
                project_type: "mod".to_string(),
                ownership_kind:
                    crate::state::instances::ContentOwnershipKind::PackManaged,
                manual_operation_kind: crate::state::instances::ManualDownloadOperationKind::PackInstall,
                game_version: None,
                mod_loader_type: None,
                world_name: None,
                install_dependencies: true,
                excluded_dependency_project_ids: Vec::new(),
                    force_dependency_project_ids: Vec::new(),
                dependency_plan_id: None,
                defer_persistence: false,
                verification_tx: None,
                pre_resolved_relative_path: None,
    expected_file_name: None,
            },
            display_title: "CurseForge".to_string(),
            display_icon: None,
        },
    );
    let modrinth_job = waiting_manual_job_record(
        "modrinth-instance",
        crate::install::model::InstallRequest::InstallPackToExistingInstance {
            instance_id: "modrinth-instance".to_string(),
            location: crate::api::pack::install_from::CreatePackLocation::FromVersionId {
                project_id: "project".to_string(),
                version_id: "version".to_string(),
                title: "Modrinth".to_string(),
                icon_url: None,
            },
            post_install_edit: None,
        },
    );
    assert_eq!(
        curseforge_waiting_job_instance_ids(&[curseforge_job, modrinth_job,]),
        vec!["curseforge-instance".to_string()]
    );

    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let materialized = HashSet::from([("1".to_string(), "10".to_string())]);
    let mut resume_count = 0;

    let result = reconcile_and_simulate_resume(
        &mut items,
        &HashSet::new(),
        &materialized,
        &mut resume_count,
    );

    assert_eq!(result.recovered.len(), 1);
    assert_eq!(resume_count, 1);
}

#[test]
fn curseforge_wrong_instance_or_release_never_recovers() {
    let mut items =
        vec![skipped_curseforge_manual_item("mods/one.jar", "1", "10")];
    let other_instance_or_release = HashSet::from([
        ("1".to_string(), "11".to_string()),
        ("2".to_string(), "10".to_string()),
    ]);
    let mut resume_count = 0;

    let result = reconcile_and_simulate_resume(
        &mut items,
        &HashSet::new(),
        &other_instance_or_release,
        &mut resume_count,
    );

    assert!(result.recovered.is_empty());
    assert_eq!(result.inconsistent.len(), 1);
    assert_eq!(resume_count, 0);
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_pending_is_scoped_by_instance() {
    let (state, instance_a) = create_stage6_instance("scope A").await;
    let (_, instance_b) = create_stage6_instance("scope B").await;
    let completed = stage6_manual_download(
        101,
        1001,
        "scope-one.jar",
        b"scope-one-content",
    );
    let other_release = stage6_manual_download(
        101,
        1002,
        "scope-two.jar",
        b"scope-two-content",
    );
    persist_manual_download(&instance_a, &completed)
        .await
        .unwrap();
    persist_manual_download(&instance_a, &other_release)
        .await
        .unwrap();
    persist_manual_download(&instance_b, &completed)
        .await
        .unwrap();
    let job_a = insert_stage6_waiting_manual_job(
        &state,
        &instance_a,
        &[
            ("mods/scope-one.jar", "101", "1001"),
            ("mods/scope-two.jar", "101", "1002"),
        ],
    )
    .await;
    let job_b = insert_stage6_waiting_manual_job(
        &state,
        &instance_b,
        &[("mods/scope-one.jar", "101", "1001")],
    )
    .await;

    import_stage6_manual_download(
        &instance_a,
        &completed,
        b"scope-one-content",
    )
    .await;

    let pending_a = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        &instance_a,
        &state.pool,
    )
    .await
    .unwrap();
    let pending_b = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        &instance_b,
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(pending_a.len(), 1);
    assert_eq!(pending_a[0].provider_project_id, "101");
    assert_eq!(pending_a[0].provider_release_id, "1002");
    assert_eq!(pending_b.len(), 1);
    assert_eq!(pending_b[0].provider_project_id, "101");
    assert_eq!(pending_b[0].provider_release_id, "1001");

    let reconciled_a = crate::install::store::get_required(job_a, &state)
        .await
        .unwrap();
    assert_eq!(
        reconciled_a.status,
        crate::install::model::InstallJobStatus::WaitingForUser
    );
    let items_a = reconciled_a.snapshot().items;
    assert_eq!(
        items_a
            .iter()
            .find(|item| item.id == "mods/scope-one.jar")
            .unwrap()
            .status,
        crate::install::model::DownloadItemStatus::Completed
    );
    assert_eq!(
        items_a
            .iter()
            .find(|item| item.id == "mods/scope-two.jar")
            .unwrap()
            .status,
        crate::install::model::DownloadItemStatus::Skipped
    );
    assert_eq!(
        reconciled_a
            .state
            .events
            .iter()
            .filter(|event| matches!(
                event.kind,
                InstallJobEventKind::ContentFileRecovered { .. }
            ))
            .count(),
        1
    );
    assert_stage6_job_is_unresolved(&state, job_b, "mods/scope-one.jar").await;
    crate::install::store::dismiss(job_a, &state).await.unwrap();
    crate::install::store::dismiss(job_b, &state).await.unwrap();
    crate::api::instance::remove(&instance_a).await.unwrap();
    crate::api::instance::remove(&instance_b).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_missing_pending_without_materialization_keeps_real_job_waiting()
 {
    let (state, instance_id) =
        create_stage6_instance("missing materialization").await;
    let job_id = insert_stage6_waiting_manual_job(
        &state,
        &instance_id,
        &[("mods/missing.jar", "201", "2001")],
    )
    .await;

    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        &instance_id,
        &state,
    )
    .await
    .unwrap();

    assert_stage6_job_is_unresolved(&state, job_id, "mods/missing.jar").await;
    crate::install::store::dismiss(job_id, &state)
        .await
        .unwrap();
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_pending_still_exists_keeps_real_job_waiting() {
    let (state, instance_id) = create_stage6_instance("pending").await;
    let download =
        stage6_manual_download(301, 3001, "pending.jar", b"pending-content");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let job_id = insert_stage6_waiting_manual_job(
        &state,
        &instance_id,
        &[("mods/pending.jar", "301", "3001")],
    )
    .await;

    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        &instance_id,
        &state,
    )
    .await
    .unwrap();

    assert_stage6_job_is_unresolved(&state, job_id, "mods/pending.jar").await;
    crate::install::store::dismiss(job_id, &state)
        .await
        .unwrap();
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_wrong_release_keeps_real_job_waiting() {
    let (state, instance_id) = create_stage6_instance("wrong release").await;
    let other_release = stage6_manual_download(
        401,
        4002,
        "other-release.jar",
        b"other-release-content",
    );
    persist_manual_download(&instance_id, &other_release)
        .await
        .unwrap();
    import_stage6_manual_download(
        &instance_id,
        &other_release,
        b"other-release-content",
    )
    .await;
    let job_id = insert_stage6_waiting_manual_job(
        &state,
        &instance_id,
        &[("mods/target-release.jar", "401", "4001")],
    )
    .await;

    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        &instance_id,
        &state,
    )
    .await
    .unwrap();

    assert_stage6_job_is_unresolved(&state, job_id, "mods/target-release.jar")
        .await;
    crate::install::store::dismiss(job_id, &state)
        .await
        .unwrap();
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_startup_reconciliation_recovers_and_resumes_once() {
    let (state, instance_id) = create_stage6_instance("startup").await;
    let download =
        stage6_manual_download(501, 5001, "startup.jar", b"startup-content");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    import_stage6_manual_download(&instance_id, &download, b"startup-content")
        .await;
    let job_id = insert_stage6_waiting_manual_job(
        &state,
        &instance_id,
        &[("mods/startup.jar", "501", "5001")],
    )
    .await;
    let resume_count =
        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state_for_resume = std::sync::Arc::clone(&state);
    let resume_count_for_callback = std::sync::Arc::clone(&resume_count);
    let mut resume_job = move |job_id| {
        let state = std::sync::Arc::clone(&state_for_resume);
        let resume_count = std::sync::Arc::clone(&resume_count_for_callback);
        async move {
            let current =
                crate::install::store::get_required(job_id, &state).await?;
            let claimed = crate::install::store::update_status_if(
                job_id,
                crate::install::model::InstallJobStatus::WaitingForUser,
                crate::install::model::InstallJobStatus::Queued,
                &current.state,
                &state,
            )
            .await?;
            assert!(claimed.is_some(), "resume status claim must succeed once");
            resume_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    };

    for _ in 0..3 {
        reconcile_persisted_curseforge_waiting_jobs_with_resume(
            &state,
            &mut resume_job,
        )
        .await
        .unwrap();
    }

    let reconciled = crate::install::store::get_required(job_id, &state)
        .await
        .unwrap();
    assert_eq!(
        reconciled.status,
        crate::install::model::InstallJobStatus::Queued
    );
    assert_eq!(
        reconciled
            .snapshot()
            .items
            .iter()
            .find(|item| item.id == "mods/startup.jar")
            .unwrap()
            .status,
        crate::install::model::DownloadItemStatus::Completed
    );
    assert_eq!(
        reconciled
            .state
            .events
            .iter()
            .filter(|event| matches!(
                event.kind,
                InstallJobEventKind::ContentFileRecovered { .. }
            ))
            .count(),
        1
    );
    assert_eq!(resume_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    crate::install::store::dismiss(job_id, &state)
        .await
        .unwrap();
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[test]
fn manual_downloads_open_the_official_download_page() {
    assert_eq!(
        curseforge_file_page_url(
            Some("https://www.curseforge.com/minecraft/mc-mods/example"),
            12345,
        ),
        Some(
            "https://www.curseforge.com/minecraft/mc-mods/example/download/12345"
                .to_string()
        )
    );
    assert_eq!(
        curseforge_file_page_url(
            Some("https://www.curseforge.com/minecraft/mc-mods/example/files/12345?tab=files"),
            12345,
        ),
        Some(
            "https://www.curseforge.com/minecraft/mc-mods/example/download/12345"
                .to_string()
        )
    );
    assert_eq!(
        curseforge_file_page_url(Some("https://example.com/project"), 12345),
        Some("https://example.com/project".to_string())
    );
}

#[test]
fn browser_duplicate_download_names_match_the_expected_file() {
    assert!(crate::util::downloads::browser_download_file_name_matches(
        "example-mod (1).jar",
        "example-mod.jar"
    ));
    assert!(crate::util::downloads::browser_download_file_name_matches(
        "EXAMPLE-MOD.JAR",
        "example-mod.jar"
    ));
    assert!(!crate::util::downloads::browser_download_file_name_matches(
        "example-mod-fabric.jar",
        "example-mod.jar"
    ));
    assert!(!crate::util::downloads::browser_download_file_name_matches(
        "example-mod.jar.crdownload",
        "example-mod.jar"
    ));
}

#[tokio::test]
async fn curseforge_metadata_less_exact_name_is_not_trusted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("example-mod.jar");
    crate::util::io::write(&path, b"wrong exact-name jar")
        .await
        .unwrap();
    let download = stage8_legacy_manual_download(1, 2, "example-mod.jar");

    let error =
        verify_manual_download_candidate_with_integrity(&path, &download, true)
            .await
            .expect_err("filename alone must not verify a candidate");

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "The required CurseForge file has no usable integrity metadata"
    ));
}

#[tokio::test]
async fn curseforge_hydration_is_scoped_to_exact_release() {
    let download = stage8_legacy_manual_download(11, 101, "exact-release.jar");
    let error = ensure_manual_download_integrity_metadata_with(
        &download,
        |project_id, file_id| async move {
            assert_eq!(project_id, 11);
            assert_eq!(file_id, 101);
            Ok(CurseForgeManualDownloadIntegrityMetadata {
                project_id,
                file_id: 102,
                hashes: vec![CurseForgeFileHash {
                    value: "wrong-release-sha1".to_string(),
                    algo: 1,
                }],
                file_length: 1,
                file_fingerprint: 0,
            })
        },
    )
    .await
    .expect_err("metadata for another file must be rejected");

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "CurseForge returned metadata for a different project or file"
    ));
}

#[tokio::test]
async fn curseforge_existing_integrity_skips_hydration() {
    let mut sha1_download =
        stage8_legacy_manual_download(12, 201, "existing-sha1.jar");
    sha1_download.hashes = vec![CurseForgeFileHash {
        value: "existing-sha1".to_string(),
        algo: 1,
    }];
    let hydrated = ensure_manual_download_integrity_metadata_with(
        &sha1_download,
        |_, _| async {
            Err(ErrorKind::InputError(
                "metadata resolver must not run".to_string(),
            )
            .into())
        },
    )
    .await
    .unwrap();
    assert_eq!(hydrated.hashes[0].value, "existing-sha1");

    let mut fingerprint_download =
        stage8_legacy_manual_download(12, 202, "existing-fingerprint.jar");
    fingerprint_download.file_fingerprint = 12345;
    let hydrated = ensure_manual_download_integrity_metadata_with(
        &fingerprint_download,
        |_, _| async {
            Err(ErrorKind::InputError(
                "metadata resolver must not run".to_string(),
            )
            .into())
        },
    )
    .await
    .unwrap();
    assert_eq!(hydrated.file_fingerprint, 12345);
}

#[tokio::test]
async fn curseforge_manual_materialization_is_copy_isolated() {
    let source_directory = tempfile::tempdir().unwrap();
    let destination_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join("manual.jar");
    let destination = destination_directory.path().join("manual.jar");
    let verified_bytes = b"verified manual bytes";
    crate::util::io::write(&source, verified_bytes)
        .await
        .unwrap();
    let (verified_size, verified_sha1) =
        sha1_file_async(&source).await.unwrap();

    crate::state::materialize_verified_project_download_copy(
        &source,
        &destination,
        verified_size,
        &verified_sha1,
    )
    .await
    .unwrap();
    crate::util::io::write(&source, b"mutated source content")
        .await
        .unwrap();

    assert_eq!(
        crate::util::io::read(&destination).await.unwrap(),
        verified_bytes
    );
    assert_eq!(
        sha1_file_async(&destination).await.unwrap(),
        (verified_size, verified_sha1)
    );
}

#[tokio::test]
async fn curseforge_localized_candidate_still_requires_integrity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("[测试]example-mod (1).jar");
    crate::util::io::write(&path, b"expected curseforge file")
        .await
        .unwrap();
    let (_, sha1) = sha1_file_async(&path).await.unwrap();
    let download = CurseForgeManualDownload {
        project_id: 1,
        file_id: 2,
        file_name: "example-mod.jar".to_string(),
        ownership_kind:
            crate::state::instances::ContentOwnershipKind::PackManaged,
        operation_kind:
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
        website_url: None,
        project_type: "mod".to_string(),
        project_slug: String::new(),
        target_folder: "mods".to_string(),
        hashes: vec![CurseForgeFileHash {
            value: sha1,
            algo: 1,
        }],
        file_length: 24,
        file_fingerprint: 0,
    };

    assert!(
        find_manual_download_candidate(directory.path(), &download)
            .await
            .unwrap()
            .is_some()
    );
    crate::util::io::write(&path, b"same-length-wrong-bytes!")
        .await
        .unwrap();
    assert!(
        find_manual_download_candidate(directory.path(), &download)
            .await
            .unwrap()
            .is_none()
    );
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_legacy_pending_imports_after_exact_metadata_hydration() {
    let (state, instance_id) = create_stage6_instance("legacy hydration").await;
    let download =
        stage8_legacy_manual_download(14, 401, "legacy-hydration.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join(&download.file_name);
    let bytes = b"legacy hydrated content";
    crate::util::io::write(&source, bytes).await.unwrap();
    let metadata = stage8_integrity_metadata(
        &download,
        vec![CurseForgeFileHash {
            value: sha1_smol::Sha1::from(bytes).hexdigest(),
            algo: 1,
        }],
        bytes.len() as u64,
        0,
    );

    let imported = import_pending_manual_download_file_with_integrity_resolver(
        &instance_id,
        download.project_id,
        download.file_id,
        source.clone(),
        move |project_id, file_id| async move {
            assert_eq!((project_id, file_id), (14, 401));
            Ok(metadata)
        },
    )
    .await
    .unwrap();

    assert!(source.exists());
    assert_eq!(imported.relative_path, "mods/legacy-hydration.jar");
    assert!(
        crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_id,
            &state.pool,
        )
        .await
        .unwrap()
        .is_empty()
    );
    let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
        .await
        .unwrap();
    assert!(snapshot.items.iter().any(|item| {
        item.provider_project_id.as_deref() == Some("14")
            && item.provider_release_id.as_deref() == Some("401")
            && item.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
            && item.content.is_some()
    }));
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_missing_integrity_after_hydration_keeps_pending_unresolved()
{
    let (state, instance_id) =
        create_stage6_instance("missing hydrated integrity").await;
    let download =
        stage8_legacy_manual_download(15, 501, "missing-integrity.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let job_id = insert_stage6_waiting_manual_job(
        &state,
        &instance_id,
        &[("mods/missing-integrity.jar", "15", "501")],
    )
    .await;
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join(&download.file_name);
    crate::util::io::write(&source, b"unverifiable content")
        .await
        .unwrap();
    let metadata = stage8_integrity_metadata(&download, Vec::new(), 20, 0);

    let fetch_error =
        import_pending_manual_download_file_with_integrity_resolver(
            &instance_id,
            download.project_id,
            download.file_id,
            source.clone(),
            |project_id, file_id| async move {
                assert_eq!((project_id, file_id), (15, 501));
                Err(ErrorKind::InputError(
                    "simulated CurseForge metadata fetch failure".to_string(),
                )
                .into())
            },
        )
        .await
        .expect_err("metadata fetch failures must block import");
    assert!(matches!(
        fetch_error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "simulated CurseForge metadata fetch failure"
    ));

    let error = import_pending_manual_download_file_with_integrity_resolver(
        &instance_id,
        download.project_id,
        download.file_id,
        source.clone(),
        move |project_id, file_id| async move {
            assert_eq!((project_id, file_id), (15, 501));
            Ok(metadata)
        },
    )
    .await
    .expect_err("missing authoritative integrity must block import");

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "The required CurseForge file has no usable integrity metadata"
    ));
    assert!(source.exists());
    assert_eq!(
        crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_id,
            &state.pool,
        )
        .await
        .unwrap()
        .len(),
        1
    );
    let target = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join("mods/missing-integrity.jar");
    assert!(!target.exists());
    assert_stage6_job_is_unresolved(
        &state,
        job_id,
        "mods/missing-integrity.jar",
    )
    .await;
    let job = crate::install::store::get_required(job_id, &state)
        .await
        .unwrap();
    assert_eq!(
        job.state
            .events
            .iter()
            .filter(|event| matches!(
                event.kind,
                InstallJobEventKind::ContentFileRecovered { .. }
            ))
            .count(),
        0
    );
    crate::install::store::dismiss(job_id, &state)
        .await
        .unwrap();
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_generic_record_does_not_complete_pending() {
    let (state, instance_id) =
        create_stage6_instance("generic record provenance").await;
    let download = stage8_legacy_manual_download(16, 601, "generic-record.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let relative_path = "mods/generic-record.jar";
    let full_path = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"generic recorded bytes";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
    let provider_ref = ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(16).unwrap(),
        file_id: Some(CurseForgeFileId::new(601).unwrap()),
    };

    crate::state::record_project_file_atomic(
        &instance_id,
        relative_path,
        &sha1,
        bytes.len() as u64,
        ProjectType::Mod,
        ContentSourceKind::CurseForge,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        Some(&provider_ref),
        true,
        None,
        &state,
    )
    .await
    .unwrap();

    assert!(
        stage8_pending_keys(&state, &instance_id)
            .await
            .contains(&("16".to_string(), "601".to_string()))
    );
    let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
        .await
        .unwrap();
    assert!(snapshot.items.iter().any(|item| {
        item.provider_project_id.as_deref() == Some("16")
            && item.provider_release_id.as_deref() == Some("601")
            && item.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
            && item.content.is_some()
    }));
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_verified_record_completes_only_exact_pending() {
    let (state, instance_a) = create_stage6_instance("verified record A").await;
    let (_, instance_b) = create_stage6_instance("verified record B").await;
    let file_1 = stage8_legacy_manual_download(17, 701, "one.jar");
    let file_2 = stage8_legacy_manual_download(17, 702, "two.jar");
    persist_manual_download(&instance_a, &file_1).await.unwrap();
    persist_manual_download(&instance_a, &file_2).await.unwrap();
    persist_manual_download(&instance_b, &file_1).await.unwrap();
    let relative_path = "mods/one.jar";
    let full_path = crate::api::instance::get_full_path(&instance_a)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"verified exact record";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let (_, sha1) = sha1_file_async(&full_path).await.unwrap();

    crate::state::record_verified_curseforge_project_file_atomic(
        &instance_a,
        relative_path,
        &sha1,
        bytes.len() as u64,
        ProjectType::Mod,
        ContentSourceKind::CurseForge,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        CurseForgeProjectId::new(17).unwrap(),
        CurseForgeFileId::new(701).unwrap(),
        true,
        &state,
    )
    .await
    .unwrap();

    assert_eq!(
        stage8_pending_keys(&state, &instance_a).await,
        HashSet::from([("17".to_string(), "702".to_string())])
    );
    assert_eq!(
        stage8_pending_keys(&state, &instance_b).await,
        HashSet::from([("17".to_string(), "701".to_string())])
    );
    crate::api::instance::remove(&instance_a).await.unwrap();
    crate::api::instance::remove(&instance_b).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_batch_completes_only_authoritatively_verified_pending() {
    let (state, instance_id) =
        create_stage6_instance("verified record batch").await;
    let verified = stage8_legacy_manual_download(170, 701, "verified.jar");
    let generic = stage8_legacy_manual_download(170, 702, "generic.jar");
    persist_manual_download(&instance_id, &verified)
        .await
        .unwrap();
    persist_manual_download(&instance_id, &generic)
        .await
        .unwrap();
    let records = [
        crate::state::instances::commands::ProjectFileRecord {
            relative_path: "mods/verified.jar".to_string(),
            sha1: "verified-sha1".to_string(),
            size: 10,
            project_type: ProjectType::Mod,
            source_kind: ContentSourceKind::CurseForge,
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            provider_ref: Some(ContentProviderRef::CurseForge {
                project_id: CurseForgeProjectId::new(170).unwrap(),
                file_id: Some(CurseForgeFileId::new(701).unwrap()),
            }),
            origin: true,
            known_modrinth_project_id: None,
            known_modrinth_version_id: None,
        },
        crate::state::instances::commands::ProjectFileRecord {
            relative_path: "mods/generic.jar".to_string(),
            sha1: "generic-sha1".to_string(),
            size: 11,
            project_type: ProjectType::Mod,
            source_kind: ContentSourceKind::CurseForge,
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            provider_ref: Some(ContentProviderRef::CurseForge {
                project_id: CurseForgeProjectId::new(170).unwrap(),
                file_id: Some(CurseForgeFileId::new(702).unwrap()),
            }),
            origin: true,
            known_modrinth_project_id: None,
            known_modrinth_version_id: None,
        },
    ];

    crate::state::instances::commands::record_project_files_with_verified_curseforge_atomic(
        &instance_id,
        &records,
        &[(
            CurseForgeProjectId::new(170).unwrap(),
            CurseForgeFileId::new(701).unwrap(),
        )],
        &state,
    )
    .await
    .unwrap();

    assert_eq!(
        stage8_pending_keys(&state, &instance_id).await,
        HashSet::from([("170".to_string(), "702".to_string())])
    );
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_manual_materialization_detects_source_change() {
    let (state, instance_id) =
        create_stage6_instance("manual copy TOCTOU").await;
    let download = stage8_legacy_manual_download(18, 801, "toctou.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join("toctou.jar");
    crate::util::io::write(&source, b"original verified bytes")
        .await
        .unwrap();
    let (verified_size, verified_sha1) =
        sha1_file_async(&source).await.unwrap();
    crate::util::io::write(&source, b"changed after verification")
        .await
        .unwrap();
    let destination = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join("mods/toctou.jar");
    let previous_bytes = b"previous destination";
    crate::util::io::write(&destination, previous_bytes)
        .await
        .unwrap();

    crate::state::materialize_verified_project_download_copy(
        &source,
        &destination,
        verified_size,
        &verified_sha1,
    )
    .await
    .expect_err("changed source must fail copied-byte identity");

    assert_eq!(
        crate::util::io::read(&destination).await.unwrap(),
        previous_bytes
    );
    assert!(
        stage8_pending_keys(&state, &instance_id)
            .await
            .contains(&("18".to_string(), "801".to_string()))
    );
    let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
        .await
        .unwrap();
    assert!(!snapshot.items.iter().any(|item| {
        item.provider_project_id.as_deref() == Some("18")
            && item.provider_release_id.as_deref() == Some("801")
            && item.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
            && item.content.is_some()
    }));
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_automatic_sha1_record_can_complete_pending() {
    let (state, instance_id) = create_stage6_instance("automatic SHA1").await;
    let download = stage8_legacy_manual_download(19, 901, "automatic-sha1.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let relative_path = "mods/automatic-sha1.jar";
    let full_path = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"automatic SHA1 bytes";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
    let file = stage8_curseforge_file(
        19,
        901,
        "automatic-sha1.jar",
        bytes.len() as u64,
        vec![CurseForgeFileHash {
            value: sha1,
            algo: 1,
        }],
        0,
    );

    record_installed_curseforge_file_with_permit(
        &instance_id,
        relative_path,
        &full_path,
        &file,
        ProjectType::Mod,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        None,
        &state,
    )
    .await
    .unwrap();

    assert!(stage8_pending_keys(&state, &instance_id).await.is_empty());
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_automatic_fingerprint_record_can_complete_pending() {
    let (state, instance_id) =
        create_stage6_instance("automatic fingerprint").await;
    let download =
        stage8_legacy_manual_download(20, 1001, "automatic-fingerprint.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let relative_path = "mods/automatic-fingerprint.jar";
    let full_path = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"automatic fingerprint bytes";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let file = stage8_curseforge_file(
        20,
        1001,
        "automatic-fingerprint.jar",
        bytes.len() as u64,
        Vec::new(),
        compute_fingerprint(bytes) as u64,
    );

    record_installed_curseforge_file_with_permit(
        &instance_id,
        relative_path,
        &full_path,
        &file,
        ProjectType::Mod,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        None,
        &state,
    )
    .await
    .unwrap();

    assert!(stage8_pending_keys(&state, &instance_id).await.is_empty());
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_automatic_weak_record_keeps_pending() {
    let (state, instance_id) =
        create_stage6_instance("automatic weak metadata").await;
    let download = stage8_legacy_manual_download(21, 1101, "automatic-md5.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let relative_path = "mods/automatic-md5.jar";
    let full_path = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"automatic MD5-only bytes";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let file = stage8_curseforge_file(
        21,
        1101,
        "automatic-md5.jar",
        bytes.len() as u64,
        vec![CurseForgeFileHash {
            value: "validated-md5".to_string(),
            algo: 2,
        }],
        0,
    );

    record_installed_curseforge_file_with_permit(
        &instance_id,
        relative_path,
        &full_path,
        &file,
        ProjectType::Mod,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        None,
        &state,
    )
    .await
    .unwrap();

    assert!(
        stage8_pending_keys(&state, &instance_id)
            .await
            .contains(&("21".to_string(), "1101".to_string()))
    );
    let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
        .await
        .unwrap();
    assert!(snapshot.items.iter().any(|item| {
        item.provider_project_id.as_deref() == Some("21")
            && item.provider_release_id.as_deref() == Some("1101")
            && item.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
    }));
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_rollback_identity_record_does_not_complete_pending() {
    let (state, instance_id) =
        create_stage6_instance("rollback provenance").await;
    let download = stage8_legacy_manual_download(22, 1201, "rollback.jar");
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let relative_path = "mods/rollback.jar";
    let full_path = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join(relative_path);
    let bytes = b"historical rollback bytes";
    crate::util::io::write(&full_path, bytes).await.unwrap();
    let (_, sha1) = sha1_file_async(&full_path).await.unwrap();
    let provider_ref = ContentProviderRef::CurseForge {
        project_id: CurseForgeProjectId::new(22).unwrap(),
        file_id: Some(CurseForgeFileId::new(1201).unwrap()),
    };

    crate::state::record_project_file_atomic(
        &instance_id,
        relative_path,
        &sha1,
        bytes.len() as u64,
        ProjectType::Mod,
        ContentSourceKind::CurseForge,
        crate::state::instances::ContentOwnershipKind::PackManaged,
        Some(&provider_ref),
        true,
        None,
        &state,
    )
    .await
    .unwrap();

    assert!(
        stage8_pending_keys(&state, &instance_id)
            .await
            .contains(&("22".to_string(), "1201".to_string()))
    );
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_non_pending_import_fails_before_verification() {
    let (state, instance_id) =
        create_stage6_instance("non-pending import").await;
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join("invalid.jar");
    crate::util::io::write(&source, b"not the required file")
        .await
        .unwrap();

    let error = import_pending_manual_download_file(
        &instance_id,
        601,
        6001,
        source.clone(),
    )
    .await
    .expect_err("missing pending identity must be rejected");

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "The selected CurseForge file is not pending for this instance"
    ));
    assert!(source.exists());
    assert!(
        crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &instance_id,
            &state.pool,
        )
        .await
        .unwrap()
        .is_empty()
    );
    let target = crate::api::instance::get_full_path(&instance_id)
        .await
        .unwrap()
        .join("mods/invalid.jar");
    assert!(!target.exists());
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn curseforge_wrong_pending_file_fails_integrity() {
    let (state, instance_id) =
        create_stage6_instance("wrong pending file").await;
    let download = stage6_manual_download(
        701,
        7001,
        "expected.jar",
        b"required pending bytes",
    );
    persist_manual_download(&instance_id, &download)
        .await
        .unwrap();
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join("selected.jar");
    crate::util::io::write(&source, b"wrong pending bytes")
        .await
        .unwrap();

    let error = import_pending_manual_download_file(
        &instance_id,
        download.project_id,
        download.file_id,
        source.clone(),
    )
    .await
    .expect_err("wrong pending file must fail integrity");

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::InputError(message)
            if message
                == "The selected file does not match the required CurseForge file"
    ));
    assert!(source.exists());
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        &instance_id,
        &state.pool,
    )
    .await
    .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].provider_project_id, "701");
    assert_eq!(pending[0].provider_release_id, "7001");
    let snapshot = crate::api::instance::get_content_snapshot(&instance_id)
        .await
        .unwrap();
    assert!(!snapshot.items.iter().any(|item| {
        item.provider_project_id.as_deref() == Some("701")
            && item.provider_release_id.as_deref() == Some("7001")
            && item.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
            && item.content.is_some()
    }));
    crate::api::instance::remove(&instance_id).await.unwrap();
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn manual_import_completes_persisted_pack_member() {
    crate::event::EventState::init().await.unwrap();
    let state_root = tempfile::tempdir().unwrap().keep();
    let state = State::init_for_test(state_root.to_string_lossy().to_string())
        .await
        .unwrap();
    let created = crate::api::instance::create(
        format!("CurseForge drag {}", uuid::Uuid::new_v4()),
        "1.12.2".to_string(),
        ModLoader::Forge,
        Some("14.23.5.2860".to_string()),
        None,
        InstanceLink::Unmanaged,
        None,
        None,
    )
    .await
    .unwrap();
    let source_directory = tempfile::tempdir().unwrap();
    let source = source_directory.path().join("example-mod.jar");
    let bytes = b"verified curseforge drag";
    crate::util::io::write(&source, bytes).await.unwrap();
    let (_, sha1) = sha1_file_async(&source).await.unwrap();
    let download = CurseForgeManualDownload {
        project_id: 123,
        file_id: 456,
        file_name: "example-mod.jar".to_string(),
        ownership_kind:
            crate::state::instances::ContentOwnershipKind::PackManaged,
        operation_kind:
            crate::state::instances::ManualDownloadOperationKind::PackInstall,
        website_url: None,
        project_type: "mod".to_string(),
        project_slug: "example-mod".to_string(),
        target_folder: "mods".to_string(),
        hashes: vec![CurseForgeFileHash {
            value: sha1,
            algo: 1,
        }],
        file_length: bytes.len() as u64,
        file_fingerprint: 0,
    };
    persist_manual_download(&created.instance.id, &download)
        .await
        .unwrap();

    crate::util::io::write(&source, b"same-length-wrong-bytes!")
        .await
        .unwrap();
    assert!(
        import_pending_manual_download_from_path(
            &created.instance.id,
            &source,
        )
        .await
        .is_err()
    );
    assert_eq!(
        crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
            &created.instance.id,
            &state.pool,
        )
        .await
        .unwrap()
        .len(),
        1
    );
    let selected = source_directory.path().join("renamed-local-file.jar");
    crate::util::io::write(&selected, bytes).await.unwrap();

    let relative_path = import_pending_manual_download_file(
        &created.instance.id,
        123,
        456,
        selected.clone(),
    )
    .await
    .unwrap()
    .relative_path;
    let snapshot =
        crate::api::instance::get_content_snapshot(&created.instance.id)
            .await
            .unwrap();
    let imported = snapshot
        .items
        .iter()
        .find(|item| {
            item.provider_project_id.as_deref() == Some("123")
                && item.provider_release_id.as_deref() == Some("456")
        })
        .unwrap();

    assert!(snapshot.pending_manual_downloads.is_empty());
    assert_eq!(
        imported.ownership_kind,
        crate::state::instances::ContentOwnershipKind::PackManaged
    );
    assert_eq!(
        imported.materialization_state,
        crate::state::instances::PackMemberMaterializationState::Present
    );
    assert_eq!(imported.expected_relative_path, relative_path);
    let instance_path = state
        .directories
        .instance_game_dir(&created.instance)
        .join(relative_path);
    assert_eq!(crate::util::io::read(&instance_path).await.unwrap(), bytes);
    assert!(source.exists());
    assert!(selected.exists());
    crate::util::io::write(&selected, b"mutated selected source")
        .await
        .unwrap();
    assert_eq!(crate::util::io::read(instance_path).await.unwrap(), bytes);
}
