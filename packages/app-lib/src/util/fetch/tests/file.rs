//! Fetch file tests.

use super::super::*;
use crate::ErrorKind;
use crate::util::fetch::model::attach_verified_integrity;

#[tokio::test]
async fn post_verification_path_replacement_never_receives_a_trusted_proof() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("staged");
    tokio::fs::write(&path, b"original").await.unwrap();
    let integrity =
        Integrity::sha1(sha1_smol::Sha1::from(b"original").hexdigest());
    verify_file(&path, &integrity).await.unwrap();
    let replacement = directory.path().join("replacement");
    tokio::fs::write(&replacement, b"modified").await.unwrap();
    finalize_download(&replacement, &path).await.unwrap();
    let result = DownloadResult {
        path: path.clone(),
        url: "https://proof.invalid/file".into(),
        source: DownloadRouteSource::Official,
        size: 8,
        attempts: 1,
        fallback_count: 0,
        verified_sha1: None,
        verified_sha512: None,
        verified_file: None,
    };
    let result = attach_verified_integrity(result, &integrity).await;
    assert!(result.verified_file.is_none());
    assert!(verify_file(&path, &integrity).await.is_err());
}

#[test]
fn size_mismatch_is_lenient_when_a_hash_matches() {
    let matching = ComputedIntegrity {
        size: 122_777,
        sha1: Some("abc".to_string()),
        ..ComputedIntegrity::default()
    };
    let hashed = Integrity {
        size: Some(122_778),
        sha1: Some("abc".to_string()),
        ..Integrity::default()
    };
    assert!(
        verify_computed_integrity(&hashed, &matching).is_ok(),
        "a hash match must win over an off-by-a-few-bytes size claim"
    );
    let size_only = Integrity {
        size: Some(122_778),
        ..Integrity::default()
    };
    assert!(
        verify_computed_integrity(&size_only, &matching).is_err(),
        "without a hash the size claim is still enforced"
    );
}

#[test]
fn identifies_hash_integrity_failures_without_treating_size_errors_as_hash_failures()
 {
    let hash_error: crate::Error = ErrorKind::OtherError(
        "Incorrect sha1 hash for download: expected != actual".to_string(),
    )
    .into();
    assert!(is_integrity_error(&hash_error));

    let typed_hash_error: crate::Error =
        ErrorKind::HashError("expected".to_string(), "actual".to_string())
            .into();
    assert!(is_integrity_error(&typed_hash_error));

    let size_error: crate::Error = ErrorKind::OtherError(
        "Incorrect size for download: 10 != 20".to_string(),
    )
    .into();
    assert!(!is_integrity_error(&size_error));
}

#[tokio::test]
async fn finalize_download_moves_a_part_file_into_a_new_destination() {
    let directory = tempfile::tempdir().unwrap();
    let part = directory.path().join("file.part");
    let destination = directory.path().join("file.bin");
    tokio::fs::write(&part, b"downloaded").await.unwrap();

    finalize_download(&part, &destination).await.unwrap();

    assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"downloaded");
    assert!(!part.exists(), "the part file should be consumed");
}

#[tokio::test]
async fn finalize_download_replaces_an_existing_destination() {
    let directory = tempfile::tempdir().unwrap();
    let part = directory.path().join("file.part");
    let destination = directory.path().join("file.bin");
    tokio::fs::write(&part, b"complete").await.unwrap();
    tokio::fs::write(&destination, b"partial").await.unwrap();

    finalize_download(&part, &destination).await.unwrap();

    assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"complete");
    assert!(!part.exists(), "the part file should be consumed");
}

#[tokio::test]
async fn finalize_download_keeps_the_destination_when_the_part_file_is_missing()
{
    let directory = tempfile::tempdir().unwrap();
    let part = directory.path().join("missing.part");
    let destination = directory.path().join("file.bin");
    tokio::fs::write(&destination, b"previous").await.unwrap();

    finalize_download(&part, &destination).await.unwrap_err();

    assert_eq!(
        tokio::fs::read(&destination).await.unwrap(),
        b"previous",
        "a failed commit must not remove the existing destination"
    );
}

#[tokio::test]
async fn finalize_download_aborts_when_the_flush_fails() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("file.bin");
    tokio::fs::write(&destination, b"previous").await.unwrap();
    // A directory cannot be opened for writing, so the flush fails for a
    // reason other than a missing file.
    let part = directory.path().join("unflushable.part");
    tokio::fs::create_dir(&part).await.unwrap();

    let error = finalize_download(&part, &destination).await.unwrap_err();

    assert!(
        !format!("{error}").is_empty(),
        "a failed flush must surface an error"
    );
    assert_eq!(
        tokio::fs::read(&destination).await.unwrap(),
        b"previous",
        "a failed flush must not replace the destination"
    );
}

#[tokio::test]
async fn verifies_streaming_integrity_algorithms() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"axolotl download").unwrap();
    let integrity = Integrity {
        size: Some(16),
        sha1: Some("90e438ead880c77ea2d7e726b5aa74e6d21a805f".to_string()),
        sha512: Some("2dcd3e0a9f198e9ef892a28ed6534dd154be2bd13531c961c0852aa6f1e24f633d2cd8288d3cf13c6a482a87c822b74a4901a2aa64292f4a371c5ebfea392c1b".to_string()),
        sha256: Some("120561bc60d59ebe2a08fc229ff2b1eb06b20c4211d21a17c15dd80790f48672".to_string()),
        md5: Some("30018bb52add8c6dbc5d4149c1325df0".to_string()),
        content: ContentValidation::None,
    };
    assert_eq!(verify_file(file.path(), &integrity).await.unwrap(), 16);
}
