//! Fetch tail tests.

use crate::util::download::constants::{
    MAX_SEGMENT_CONCURRENCY, SEGMENTED_DOWNLOAD_THRESHOLD,
};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Semaphore;

use super::super::*;
use super::fixtures;
use super::fixtures::*;

#[tokio::test]
async fn stalled_tail_range_is_hedged_and_temp_files_are_cleaned() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, _, server) = spawn_range_server(
        data,
        RangeServerBehavior {
            stall_first_range: true,
            ..Default::default()
        },
    )
    .await;
    let route = DownloadRoute {
        url: url.clone(),
        source: DownloadRouteSource::Alternate,
        is_mirror: false,
        allow_sensitive_headers: false,
        supports_range: true,
        proxy: ProxyPolicy::Direct,
    };
    let request = DownloadRequest::new(&url, ResourceClass::Other)
        .with_integrity(Integrity::sha1(hash).with_size(size as u64));
    let directory = tempfile::tempdir().unwrap();
    let part_path = directory.path().join("hedged.part");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(8));
    let started = Instant::now();
    let outcome = try_segmented_download(
        SegmentedDownloadContext::new(
            &request,
            &route,
            &[],
            size as u64,
            &part_path,
            &semaphore,
            None,
            &client,
            &client,
            1,
            1,
            false,
        ),
        None,
    )
    .await;
    assert!(matches!(outcome, SegmentedDownloadOutcome::Success(_)));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(requests.load(Ordering::Relaxed) > 2);
    assert_eq!(
        verify_file(&part_path, &request.integrity).await.unwrap(),
        size as u64
    );
    for range_index in 0..MAX_SEGMENT_CONCURRENCY {
        for candidate_index in 0..2 {
            assert!(
                !tail_candidate_path(&part_path, range_index, candidate_index,)
                    .exists()
            );
        }
    }
    server.abort();
}
