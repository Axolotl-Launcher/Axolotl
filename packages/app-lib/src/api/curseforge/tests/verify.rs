//! CurseForge verify tests.

use super::fixtures::{
    murmur2, stage6_state, stage8_curseforge_file, stage8_integrity_metadata,
    stage8_legacy_manual_download,
};
use crate::api::curseforge::dto::CurseForgeFileHash;
use crate::api::curseforge::manual::{
    ensure_manual_download_integrity_metadata_with,
    verify_manual_download_candidate_with_integrity,
    verify_recognized_curseforge_file,
};
use crate::api::curseforge::verify::{
    ActiveCurseForgeDownload, CurseForgeDatabaseTask,
    CurseForgeFingerprintHasher, CurseForgePendingCompletionProof,
    CurseForgeVerificationContext, CurseForgeVerificationTask,
    compute_fingerprint, fingerprint_and_sha1_file,
    is_curseforge_fingerprint_whitespace, persist_curseforge_database_batch,
    receive_curseforge_database_batch, spawn_curseforge_database_worker,
    verify_and_record_curseforge_modpack_file,
    verify_installed_curseforge_file,
};
use crate::install::InstallPhaseDetails;
use crate::state::{ContentSourceKind, ProjectType};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn staged_digest_proof_is_reused_only_for_the_unchanged_file() {
    let directory = tempfile::tempdir().unwrap();
    let staged = directory.path().join("content.installing.download");
    let other = directory.path().join("other.installing.download");
    tokio::fs::write(&staged, b"verified staged bytes")
        .await
        .unwrap();
    let sha1 = sha1_smol::Sha1::from(b"verified staged bytes").hexdigest();
    let file = stage8_curseforge_file(
        1,
        2,
        "content.jar",
        21,
        vec![CurseForgeFileHash {
            value: sha1.clone(),
            algo: 1,
        }],
        0,
    );
    let download = crate::util::fetch::DownloadResult {
        path: staged.clone(),
        url: "https://staged-proof.invalid/file".into(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        size: 21,
        attempts: 1,
        fallback_count: 0,
        verified_sha1: Some(sha1.clone()),
        verified_sha512: None,
        verified_file:
            crate::util::download::verified_file::VerifiedFile::capture(
                &staged, 21,
            )
            .await,
    };
    assert_eq!(
        download
            .verified_file
            .as_ref()
            .unwrap()
            .matches(&staged, 21)
            .await,
        cfg!(unix)
    );
    let (verified, scans) = crate::util::download::verified_file::track_scans(
        verify_installed_curseforge_file(&staged, &file, None, Some(&download)),
    )
    .await;
    let verified = verified.unwrap();
    assert_eq!(scans, usize::from(!cfg!(unix)));
    assert_eq!(verified.sha1, sha1);
    assert_eq!(
        verified.pending_completion,
        CurseForgePendingCompletionProof::AuthoritativeSha1
    );
    let serialized = serde_json::to_value(&download).unwrap();
    let restored: crate::util::fetch::DownloadResult =
        serde_json::from_value(serialized).unwrap();
    assert!(restored.verified_file.is_none());
    let (rescanned, scans) = crate::util::download::verified_file::track_scans(
        verify_installed_curseforge_file(&staged, &file, None, Some(&restored)),
    )
    .await;
    assert!(rescanned.is_ok());
    assert_eq!(scans, 1);
    tokio::fs::write(&other, b"corrupted other bytes")
        .await
        .unwrap();
    assert!(
        verify_installed_curseforge_file(&other, &file, None, Some(&download))
            .await
            .is_err()
    );
    tokio::fs::write(&staged, b"corrupted staged file")
        .await
        .unwrap();
    let handle = std::fs::File::options().write(true).open(&staged).unwrap();
    handle
        .set_times(std::fs::FileTimes::new().set_modified(
            std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(123456),
        ))
        .unwrap();
    assert!(
        verify_installed_curseforge_file(&staged, &file, None, Some(&download))
            .await
            .is_err()
    );
    assert!(
        verify_installed_curseforge_file(&staged, &file, None, Some(&restored))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn staged_digest_proof_never_skips_fingerprint_verification() {
    let directory = tempfile::tempdir().unwrap();
    let staged = directory.path().join("content.installing.download");
    tokio::fs::write(&staged, b"verified staged bytes")
        .await
        .unwrap();
    let file = stage8_curseforge_file(1, 2, "content.jar", 21, vec![], 1);
    let download = crate::util::fetch::DownloadResult {
        path: staged.clone(),
        url: "https://staged-proof.invalid/file".into(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        size: 21,
        attempts: 1,
        fallback_count: 0,
        verified_sha1: Some("trusted-sha1".into()),
        verified_sha512: None,
        verified_file:
            crate::util::download::verified_file::VerifiedFile::capture(
                &staged, 21,
            )
            .await,
    };
    let result =
        verify_installed_curseforge_file(&staged, &file, None, Some(&download))
            .await;
    assert!(result.err().unwrap().to_string().contains("fingerprint"));
}

#[tokio::test]
async fn curseforge_database_worker_cancels_while_queue_is_empty() {
    let cancellation = CancellationToken::new();
    let context = CurseForgeVerificationContext {
        reporter: None,
        loading_bar: None,
        details: InstallPhaseDetails::Empty,
        files_done: Arc::new(AtomicU64::new(0)),
        bytes_done: Arc::new(AtomicU64::new(0)),
        active_downloads: Arc::new(AtomicU64::new(0)),
        total_files: 1,
        total_bytes: 1,
        cancellation: cancellation.clone(),
    };
    let (_sender, receiver) = mpsc::channel(1);
    let worker = spawn_curseforge_database_worker(receiver, context);

    cancellation.cancel();
    let error = tokio::time::timeout(Duration::from_millis(100), worker)
        .await
        .expect("an idle database worker should stop immediately")
        .unwrap()
        .unwrap_err();

    assert!(error.to_string().contains("canceled"));
}

#[tokio::test]
async fn curseforge_database_batch_is_retained_when_canceled_mid_batch() {
    let cancellation = CancellationToken::new();
    let (sender, mut receiver) = mpsc::channel(2);
    sender.send(7_u8).await.unwrap();
    let trigger = cancellation.clone();
    let cancel_task = tokio::spawn(async move {
        tokio::task::yield_now().await;
        trigger.cancel();
    });

    let batch = tokio::time::timeout(
        Duration::from_millis(100),
        receive_curseforge_database_batch(&mut receiver, &cancellation),
    )
    .await
    .expect("partial database batch should be returned on cancellation")
    .unwrap()
    .unwrap();
    cancel_task.await.unwrap();

    assert_eq!(batch, vec![7]);
}

#[test]
fn active_curseforge_download_counter_is_restored_on_drop() {
    let counter = Arc::new(AtomicU64::new(0));
    {
        let _active = ActiveCurseForgeDownload::start(counter.clone());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }
    assert_eq!(counter.load(Ordering::Relaxed), 0);

    let mut active = ActiveCurseForgeDownload::start(counter.clone());
    active.finish();
    active.finish();
    assert_eq!(counter.load(Ordering::Relaxed), 0);
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn failed_curseforge_verification_never_replaces_existing_file() {
    let _state = stage6_state().await;
    let directory = tempfile::tempdir().unwrap();
    let download_path = directory.path().join("new.installing.download");
    let full_path = directory.path().join("existing.jar");
    crate::util::io::write(&download_path, b"invalid replacement")
        .await
        .unwrap();
    crate::util::io::write(&full_path, b"existing content")
        .await
        .unwrap();
    let expected_sha1 =
        sha1_smol::Sha1::from(b"expected replacement").hexdigest();
    let file = stage8_curseforge_file(
        1,
        2,
        "existing.jar",
        19,
        vec![CurseForgeFileHash {
            value: expected_sha1,
            algo: 1,
        }],
        0,
    );
    let (database_tx, _database_rx) = mpsc::channel(1);

    let error = verify_and_record_curseforge_modpack_file(
        CurseForgeVerificationTask {
            instance_id: "verification-order-test".to_string(),
            relative_path: "mods/existing.jar".to_string(),
            download_path: download_path.clone(),
            full_path: full_path.clone(),
            file,
            project_type: ProjectType::Mod,
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            expected_bytes: 19,
            download_result: None,
            cancellation: CancellationToken::new(),
        },
        database_tx,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().to_ascii_lowercase().contains("hash"));
    assert_eq!(
        crate::util::io::read(&full_path).await.unwrap(),
        b"existing content"
    );
    assert!(!download_path.exists());
}

#[cfg(not(feature = "tauri"))]
#[tokio::test]
async fn canceled_curseforge_database_batch_restores_previous_file() {
    let _state = stage6_state().await;
    let directory = tempfile::tempdir().unwrap();
    let full_path = directory.path().join("content.jar");
    let previous_path =
        directory.path().join("content.jar.installing.previous");
    crate::util::io::write(&full_path, b"new content")
        .await
        .unwrap();
    crate::util::io::write(&previous_path, b"old content")
        .await
        .unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let context = CurseForgeVerificationContext {
        reporter: None,
        loading_bar: None,
        details: InstallPhaseDetails::Empty,
        files_done: Arc::new(AtomicU64::new(0)),
        bytes_done: Arc::new(AtomicU64::new(0)),
        active_downloads: Arc::new(AtomicU64::new(0)),
        total_files: 1,
        total_bytes: 11,
        cancellation,
    };
    let batch = vec![CurseForgeDatabaseTask {
        instance_id: "database-rollback-test".to_string(),
        record: crate::state::instances::commands::ProjectFileRecord {
            relative_path: "mods/content.jar".to_string(),
            sha1: String::new(),
            size: 11,
            project_type: ProjectType::Mod,
            source_kind: ContentSourceKind::CurseForge,
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            provider_ref: None,
            origin: true,
            known_modrinth_project_id: None,
            known_modrinth_version_id: None,
        },
        verified_pending: None,
        expected_bytes: 11,
        full_path: full_path.clone(),
        previous_path: Some(previous_path.clone()),
    }];

    persist_curseforge_database_batch(&batch, &context)
        .await
        .unwrap_err();

    assert_eq!(
        crate::util::io::read(&full_path).await.unwrap(),
        b"old content"
    );
    assert!(!previous_path.exists());
}

#[test]
fn fingerprint_ignores_curseforge_whitespace() {
    assert_eq!(
        compute_fingerprint(b"abc\r\n def\t"),
        compute_fingerprint(b"abcdef")
    );
}

#[test]
fn streaming_fingerprint_matches_reference_across_chunk_boundaries() {
    let data = (0..4099)
        .map(|index| match index % 31 {
            0 => b' ',
            1 => b'\n',
            2 => b'\r',
            3 => b'\t',
            _ => (index % 251) as u8,
        })
        .collect::<Vec<_>>();
    let normalized = data
        .iter()
        .copied()
        .filter(|byte| !is_curseforge_fingerprint_whitespace(*byte))
        .collect::<Vec<_>>();
    let expected = murmur2(&normalized, 1);

    for chunk_size in [1, 2, 3, 4, 5, 31, 256, 1024] {
        let mut hasher =
            CurseForgeFingerprintHasher::new(normalized.len() as u32);
        for chunk in data.chunks(chunk_size) {
            hasher.update(chunk);
        }
        assert_eq!(hasher.finish(), expected, "chunk size {chunk_size}");
    }
    assert_eq!(compute_fingerprint(&data), expected);
}

#[tokio::test]
async fn file_fingerprint_streams_sha1_and_supports_cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large-fingerprint.jar");
    let mut data = vec![b'a'; 256 * 1024 + 17];
    data[3] = b' ';
    data[256 * 1024 - 1] = b'\n';
    crate::util::io::write(&path, &data).await.unwrap();

    let (size, sha1, fingerprint) =
        fingerprint_and_sha1_file(&path, None).await.unwrap();
    assert_eq!(size, data.len() as u64);
    assert_eq!(sha1, sha1_smol::Sha1::from(&data).hexdigest());
    assert_eq!(fingerprint, compute_fingerprint(&data));

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = fingerprint_and_sha1_file(&path, Some(&cancellation))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("canceled"));
}

#[tokio::test]
async fn curseforge_hydrated_fingerprint_verifies_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fingerprint.jar");
    let bytes = b"legacy fingerprint content";
    crate::util::io::write(&path, bytes).await.unwrap();
    let download = stage8_legacy_manual_download(13, 301, "fingerprint.jar");
    let metadata = stage8_integrity_metadata(
        &download,
        Vec::new(),
        bytes.len() as u64,
        compute_fingerprint(bytes) as u64,
    );
    let hydrated = ensure_manual_download_integrity_metadata_with(
        &download,
        move |project_id, file_id| async move {
            assert_eq!((project_id, file_id), (13, 301));
            Ok(metadata)
        },
    )
    .await
    .unwrap();

    assert!(
        verify_manual_download_candidate_with_integrity(
            &path, &hydrated, true,
        )
        .await
        .unwrap()
        .is_some()
    );
}

#[tokio::test]
async fn curseforge_fingerprint_recognition_rechecks_current_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recognized.jar");
    let matched_bytes = b"matched fingerprint bytes";
    let expected_fingerprint = compute_fingerprint(matched_bytes) as u64;
    crate::util::io::write(&path, matched_bytes).await.unwrap();
    assert!(
        verify_recognized_curseforge_file(&path, expected_fingerprint)
            .await
            .unwrap()
            .is_some()
    );

    crate::util::io::write(&path, b"changed after fingerprint match")
        .await
        .unwrap();
    assert!(
        verify_recognized_curseforge_file(&path, expected_fingerprint)
            .await
            .unwrap()
            .is_none()
    );
}
