//! Fetch segmented tests.

use crate::util::download::constants::{
    FILE_VALIDATION_CONCURRENCY, MAX_SEGMENT_CONCURRENCY,
    SEGMENTED_DOWNLOAD_THRESHOLD,
};
use crate::util::download_manager::DownloadSpeedTracker;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;

use super::super::*;
use super::fixtures;
use super::fixtures::*;

#[tokio::test]
async fn segmented_download_uses_parallel_validated_ranges() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    RANGE_SPLITTING_SUPPORTED.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2 + 1024 * 1024) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, normal_requests, server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            slow_body: true,
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
    let part_path = directory.path().join("fixture.part");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(8));
    let progress_samples = Arc::new(Mutex::new(Vec::new()));
    let samples = Arc::clone(&progress_samples);
    let mut progress: Box<FetchProgressFn<'_>> = Box::new(move |bytes, _| {
        samples.lock().push(bytes);
        Box::pin(async { Ok(()) })
    });
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
        Some(&mut progress),
    )
    .await;
    match outcome {
        SegmentedDownloadOutcome::Success(result) => {
            assert_eq!(result.size, size as u64);
        }
        _ => panic!("segmented fixture download did not succeed"),
    }
    let samples = progress_samples.lock();
    assert!(samples.first().is_some_and(|bytes| *bytes <= 512 * 1024));
    assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(samples.iter().all(|bytes| *bytes <= size as u64));
    drop(samples);
    assert!(requests.load(Ordering::Relaxed) >= 2);
    assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
    assert!(RANGE_SPLITTING_SUPPORTED.lock().contains(
        &super::super::super::download::proxy_context::authority_key(
            &range_splitting_authority(&route).unwrap(),
            route.proxy
        )
    ));
    assert_eq!(
        verify_file(&part_path, &request.integrity).await.unwrap(),
        size as u64
    );
    for index in 0..MAX_SEGMENT_CONCURRENCY {
        assert!(
            !segment_path(&part_path, index).exists(),
            "direct range output must not create segment files"
        );
    }
    server.abort();
}

#[tokio::test]
async fn mirror_ranges_start_concurrently_and_retries_reuse_redirect() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (target_url, range_requests, normal_requests, range_server) =
        spawn_range_server(
            data,
            RangeServerBehavior {
                fail_first_range: true,
                ..Default::default()
            },
        )
        .await;
    let (redirect_url, redirect_requests, redirect_server) =
        spawn_redirect_server(target_url, Duration::from_millis(50)).await;
    let route = DownloadRoute {
        url: redirect_url.clone(),
        source: DownloadRouteSource::Mcim,
        is_mirror: true,
        allow_sensitive_headers: false,
        supports_range: true,
        proxy: ProxyPolicy::Direct,
    };
    let request = DownloadRequest::new(&redirect_url, ResourceClass::Modrinth)
        .with_integrity(Integrity::sha1(hash).with_size(size as u64));
    let directory = tempfile::tempdir().unwrap();
    let part_path = directory.path().join("redirect.part");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let outcome = try_segmented_download(
        SegmentedDownloadContext::new(
            &request,
            &route,
            &[],
            size as u64,
            &part_path,
            &FetchSemaphore(Semaphore::new(8)),
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
    assert_eq!(redirect_requests.load(Ordering::Relaxed), 2);
    assert!(range_requests.load(Ordering::Relaxed) > 2);
    assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
    redirect_server.abort();
    range_server.abort();
}

#[tokio::test]
async fn temporary_range_failure_resumes_without_disabling_segments() {
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
            fail_first_range: true,
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
    let part_path = directory.path().join("retry.part");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let outcome = try_segmented_download(
        SegmentedDownloadContext::new(
            &request,
            &route,
            &[],
            size as u64,
            &part_path,
            &FetchSemaphore(Semaphore::new(8)),
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
    assert!(requests.load(Ordering::Relaxed) > 2);
    assert_eq!(
        verify_file(&part_path, &request.integrity).await.unwrap(),
        size as u64
    );
    server.abort();
}

#[tokio::test]
async fn invalid_content_range_disables_range_splitting_after_repeats() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let data = Arc::new(vec![7_u8; 1024 * 1024]);
    let (url, _, _, server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            wrong_content_range: true,
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
    let directory = tempfile::tempdir().unwrap();
    let part_path = directory.path().join("fixture.part");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
    let (progress, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let permit = acquire_native_connection(&route, &semaphore).await.unwrap();
    let speed = DownloadSpeedTracker::default();
    let validator = Mutex::new(None);
    let output = crate::util::download::range_output::RangeOutput::create(
        &part_path,
        data.len() as u64,
    )
    .await
    .unwrap();
    let result = download_segment(
        &route,
        DownloadRange::new(0, 0, data.len() as u64 - 1),
        data.len() as u64,
        None,
        None,
        None,
        &part_path,
        &output,
        permit,
        &client,
        &client,
        progress,
        &speed,
        &validator,
        None,
        &semaphore,
        &AtomicUsize::new(0),
        &AtomicBool::new(false),
        &DownloadRequest::new(&route.url, ResourceClass::Other),
    )
    .await;
    assert!(matches!(
        result,
        Err(SegmentDownloadError::Protocol("invalid Content-Range"))
    ));
    disable_range_splitting(&route);
    assert!(
        range_splitting_allowed(&route),
        "a single protocol failure should not disable range splitting"
    );
    disable_range_splitting(&route);
    assert!(
        !range_splitting_allowed(&route),
        "repeated protocol failures should disable range splitting"
    );
    server.abort();
}
