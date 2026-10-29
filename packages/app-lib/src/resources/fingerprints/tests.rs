use super::*;
use std::sync::atomic::Ordering;

async fn file(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("mods")).unwrap();
    let path = temp.path().join("mods/a.jar");
    std::fs::write(&path, bytes).unwrap();
    (temp, path)
}

#[tokio::test]
async fn ten_refreshes_of_an_unchanged_file_do_not_read_or_rehash_bytes() {
    let (temp, _) = file(b"mod bytes").await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    let first = service
        .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
        .await
        .unwrap();
    assert_eq!(
        first.record.as_ref().unwrap().sha1(),
        sha1_smol::Sha1::from(b"mod bytes").hexdigest()
    );
    assert_eq!(
        first.identity.fingerprint,
        Some(crate::util::fingerprint::compute_fingerprint(b"mod bytes"))
    );
    assert_eq!(service.read_count.load(Ordering::Relaxed), 1);
    for _ in 0..10 {
        let refreshed = service
            .resolve(
                temp.path(),
                "mods/a.jar",
                first.record.as_ref(),
                false,
                &cancel,
            )
            .await
            .unwrap();
        assert_eq!(refreshed.record, first.record);
    }
    assert_eq!(service.read_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn same_size_replacement_is_detected_without_watcher_events() {
    let (temp, path) = file(b"original").await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    let first = service
        .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
        .await
        .unwrap();
    let old_time = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, b"replaced").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old_time + std::time::Duration::from_secs(1))
        .unwrap();
    let changed = service
        .resolve(
            temp.path(),
            "mods/a.jar",
            first.record.as_ref(),
            false,
            &cancel,
        )
        .await
        .unwrap();
    assert_ne!(
        changed.record.as_ref().unwrap().sha1(),
        first.record.as_ref().unwrap().sha1()
    );
    assert_eq!(
        changed.identity.integrity.size,
        first.identity.integrity.size
    );
    assert_eq!(service.read_count.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn changed_root_and_explicit_integrity_verification_require_a_read() {
    let (temp, _) = file(b"bytes").await;
    let (other, _) = file(b"other").await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    let first = service
        .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
        .await
        .unwrap();
    let second = service
        .resolve(
            other.path(),
            "mods/a.jar",
            first.record.as_ref(),
            false,
            &cancel,
        )
        .await
        .unwrap();
    assert_ne!(
        first.record.as_ref().unwrap().sha1(),
        second.record.as_ref().unwrap().sha1()
    );
    service
        .resolve(
            other.path(),
            "mods/a.jar",
            second.record.as_ref(),
            true,
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(service.read_count.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn a_trusted_sha1_is_reused_while_missing_algorithms_are_backfilled() {
    let (temp, _) = file(b"bytes").await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    let full = service
        .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
        .await
        .unwrap();
    let mut partial = full.record.unwrap();
    partial
        .hashes
        .retain(|hash| hash.algorithm == HashAlgorithm::Sha1);
    partial.curseforge_fingerprint = None;
    let read_count = service.read_count.load(Ordering::Relaxed);
    let scanned = service
        .resolve_scan(temp.path(), "mods/a.jar", Some(&partial), false, &cancel)
        .await
        .unwrap();
    assert_eq!(scanned.record.as_ref(), Some(&partial));
    assert_eq!(
        service.read_count.load(Ordering::Relaxed),
        read_count,
        "scanning does not request missing recognition algorithms"
    );
    let backfilled = service
        .resolve(temp.path(), "mods/a.jar", Some(&partial), false, &cancel)
        .await
        .unwrap();
    assert_eq!(
        backfilled
            .record
            .as_ref()
            .unwrap()
            .hashes
            .iter()
            .filter(|hash| hash.algorithm == HashAlgorithm::Sha1)
            .count(),
        1
    );
    assert_eq!(backfilled.record.as_ref().unwrap().sha1(), partial.sha1());
    assert!(complete(backfilled.record.as_ref().unwrap()));
}

#[tokio::test]
async fn pre_canceled_work_does_not_read_a_file() {
    let (temp, _) = file(b"bytes").await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        service
            .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
            .await,
        Err(ResourceError::Canceled)
    ));
    assert_eq!(service.read_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn concurrent_readers_share_one_fingerprint() {
    let (temp, _) = file(&vec![0x51; 4 * 1024 * 1024]).await;
    let service = FileFingerprints::default();
    let cancel = CancellationToken::new();
    let results = futures::future::join_all((0..8).map(|_| {
        service.resolve(temp.path(), "mods/a.jar", None, false, &cancel)
    }))
    .await;
    assert!(results.iter().all(Result::is_ok));
    assert_eq!(service.read_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn changed_during_read_is_retried_once_and_never_saved_as_old_evidence() {
    for repeated in [false, true] {
        let (temp, path) = file(b"original").await;
        let service = FileFingerprints::default();
        let cancel = CancellationToken::new();
        let mut attempt = 0;
        *service.after_read.lock().unwrap() = Some(Box::new(move || {
            attempt += 1;
            if repeated || attempt == 1 {
                let time =
                    std::fs::metadata(&path).unwrap().modified().unwrap();
                std::fs::write(&path, format!("changed-{attempt}")).unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(time + std::time::Duration::from_secs(1))
                    .unwrap();
            }
        }));
        let result = service
            .resolve(temp.path(), "mods/a.jar", None, false, &cancel)
            .await;
        assert_eq!(service.read_count.load(Ordering::Relaxed), 2);
        if repeated {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().record.unwrap().sha1(),
                sha1_smol::Sha1::from(b"changed-1").hexdigest()
            );
        }
    }
}
