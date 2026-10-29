//! Fetch model tests.

use super::super::*;

#[test]
fn segmented_download_flag_is_buildable_per_request() {
    let request = DownloadRequest::new(
        "https://example.com/a.jar",
        ResourceClass::Modpack,
    );
    assert!(request.allow_segmented_download);
    let request = request.with_segmented_download(false);
    assert!(!request.allow_segmented_download);
}

#[test]
fn batch_requests_can_disable_http1_ranges_without_disabling_h2() {
    let request = DownloadRequest::new(
        "https://example.com/content.jar",
        ResourceClass::Modpack,
    )
    .with_http1_segmented_download(false);

    assert!(request.allow_segmented_download);
    assert!(!request.allow_http1_segmented_download);
}

#[test]
fn batch_policy_splits_ten_megabyte_items_with_a_known_hash() {
    for (size, segmented) in
        [(9_999_999, false), (10_000_000, true), (139_963_795, true)]
    {
        let request = DownloadRequest::new(
            "https://example.com/content.jar",
            ResourceClass::Modpack,
        )
        .with_integrity(Integrity::sha1("expected").with_size(size))
        .with_batch_download_policy();
        assert!(request.allow_segmented_download);
        assert_eq!(request.allow_http1_segmented_download, segmented);
    }
    let request = DownloadRequest::new(
        "https://example.com/content.jar",
        ResourceClass::Modpack,
    )
    .with_integrity(Integrity::default().with_size(10_000_000))
    .with_batch_download_policy();
    assert!(!request.allow_http1_segmented_download);
}

#[test]
fn h2_range_concurrency_is_explicit_per_request() {
    let request = DownloadRequest::new(
        "https://example.com/pack.mrpack",
        ResourceClass::Modpack,
    );
    assert_eq!(request.h2_range_concurrency, None);
    assert_eq!(
        request.with_h2_range_concurrency(8).h2_range_concurrency,
        Some(8)
    );
}
