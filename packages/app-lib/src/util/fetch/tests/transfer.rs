//! Fetch transfer tests.

use super::super::transfer::{
    H2AttemptResult, resume_checkpointed_download_with,
    select_h2_download_route, try_h2_download,
};
use super::super::*;
use super::fixtures;
use super::fixtures::*;
use crate::ErrorKind;
use crate::util::download::constants::{
    MAX_SEGMENT_CONCURRENCY, ROUTE_PROBE_CONCURRENCY,
    SEGMENTED_DOWNLOAD_THRESHOLD,
};
use bytes::Bytes;
use reqwest::{StatusCode, header};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

#[tokio::test]
async fn failed_checkpoint_ranges_restart_full_download_but_io_and_cancel_keep_checkpoints()
 {
    use super::super::super::download::h2_download::{
        H2DownloadFailure, H2DownloadOutcome,
    };
    let directory = tempfile::tempdir().unwrap();
    let part = directory.path().join("file.part");
    let destination = directory.path().join("file");
    let journal = super::super::super::download::range_journal::path(&part);
    let request = DownloadRequest::new(
        "https://resume.invalid/file",
        ResourceClass::Other,
    )
    .with_integrity(Integrity::sha1("expected"));
    let route = official_route(&request.url, ResourceClass::Other);
    for failure in [
        H2DownloadFailure::RangeUnsupported,
        H2DownloadFailure::Protocol,
        H2DownloadFailure::Io,
    ] {
        tokio::fs::write(&part, b"sparse-part").await.unwrap();
        tokio::fs::write(&journal, b"checkpoint").await.unwrap();
        let result = resume_checkpointed_download_with(
            &request,
            &[route.clone()],
            &part,
            |_| async {
                Ok(Some(H2DownloadOutcome::Fallback {
                    failure,
                    preserve_partial: failure
                        != H2DownloadFailure::RangeUnsupported,
                }))
            },
        )
        .await;
        if failure == H2DownloadFailure::Io {
            assert!(result.is_err());
            assert!(part.exists() && journal.exists());
        } else {
            assert!(result.unwrap().is_none());
            assert!(!part.exists() && !journal.exists());
            tokio::fs::write(&part, b"complete full response")
                .await
                .unwrap();
            finalize_download(&part, &destination).await.unwrap();
            assert_eq!(
                tokio::fs::read(&destination).await.unwrap(),
                b"complete full response"
            );
        }
    }
    assert!(
        resume_checkpointed_download_with(
            &request,
            &[route],
            &part,
            |_| async { Ok(Some(H2DownloadOutcome::Canceled)) }
        )
        .await
        .is_err()
    );
    assert!(part.exists() && journal.exists());
}
use std::sync::atomic::Ordering;
use std::time::Duration;

#[tokio::test]
async fn java_h2_probe_discovers_size_before_selecting_independent_ranges() {
    let size = 16 * 1024 * 1024;
    let request = DownloadRequest::new(
        "https://java-size-probe.invalid/runtime.zip",
        ResourceClass::Java,
    )
    .with_integrity(Integrity::sha1("verified-at-completion"));
    let mut route =
        direct_test_route(request.url.clone(), DownloadRouteSource::Official);
    route.supports_range = true;
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
        let mut server = h2::server::handshake(server_io).await.unwrap();
        let (request, mut respond) = server.accept().await.unwrap().unwrap();
        assert_eq!(request.headers()[header::RANGE], "bytes=0-0");
        let response = http::Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes 0-0/{size}"))
            .header(header::CONTENT_LENGTH, 1)
            .body(())
            .unwrap();
        respond
            .send_response(response, false)
            .unwrap()
            .send_data(Bytes::from_static(&[31]), true)
            .unwrap();
        while server.accept().await.is_some() {}
    });
    let (sender, driver) = h2::client::handshake(client_io).await.unwrap();
    let connection = super::super::super::download::h2_pool::SharedH2Connection::cache_for_test(&route, sender).await;
    let driver =
        super::super::super::download::h2_pool::spawn_connection_driver(
            &connection,
            driver,
            Duration::from_secs(60),
        );
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("runtime.zip");
    let part = suffixed_path(&destination, ".part");
    let semaphore = FetchSemaphore(Semaphore::new(64));
    let result = try_h2_download(
        &request,
        route,
        super::super::super::download::native::NativeH2Policy {
            allow_cold_connection: true,
            abort_if_slow: true,
            expected_speed: None,
        },
        &destination,
        &part,
        &semaphore,
    )
    .await
    .unwrap();
    assert!(matches!(result, H2AttemptResult::Fallback {
        prefer_http1: true,
        discovered_size: Some(discovered),
        failed_nonofficial: None,
    } if discovered == size));
    assert!(!destination.exists());
    assert!(!connection.is_dead());
    assert!(!connection.is_retiring());
    assert_eq!(semaphore.0.available_permits(), 64);
    driver.abort();
    server.abort();
}

#[tokio::test]
async fn warm_h2_archive_hint_cannot_bypass_large_file_connections() {
    let request = DownloadRequest::new(
        "https://warm-archive-policy.invalid/pack.mrpack",
        ResourceClass::Modpack,
    )
    .with_integrity(Integrity::sha1("expected").with_size(9_999_999))
    .with_h2_range_concurrency(16);
    let mut route =
        direct_test_route(request.url.clone(), DownloadRouteSource::Official);
    route.supports_range = true;
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
        let mut server = h2::server::handshake(server_io).await.unwrap();
        while server.accept().await.is_some() {}
    });
    let (sender, driver) = h2::client::handshake(client_io).await.unwrap();
    let connection = super::super::super::download::h2_pool::SharedH2Connection::cache_for_test(&route, sender).await;
    let driver =
        super::super::super::download::h2_pool::spawn_connection_driver(
            &connection,
            driver,
            Duration::from_secs(60),
        );
    let directory = tempfile::tempdir().unwrap();
    let part = directory.path().join("archive.part");
    let routes = [route.clone()];
    assert!(
        select_h2_download_route(&request, &routes, &part)
            .await
            .is_some()
    );
    for size in [10_000_000, 27_137_250, 100_855_703] {
        let large = request
            .clone()
            .with_integrity(Integrity::sha1("expected").with_size(size));
        assert!(
            select_h2_download_route(&large, &routes, &part)
                .await
                .is_none()
        );
        let explicitly_shared = large.with_http1_segmented_download(false);
        assert!(
            select_h2_download_route(&explicitly_shared, &routes, &part)
                .await
                .is_some()
        );
    }
    assert!(!connection.is_retiring());
    driver.abort();
    server.abort();
}

#[tokio::test]
async fn file_download_times_out_when_a_successful_response_stalls() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let (url, server) = spawn_stream_server(Vec::new(), 4, true).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("stalled.bin");

    let error = test_download(&url, &destination, 4).await.unwrap_err();

    assert!(matches!(
        error.raw.as_ref(),
        ErrorKind::NetworkError(_) | ErrorKind::FetchError(_)
    ));
    assert!(!destination.exists());
    assert!(!suffixed_path(&destination, ".part").exists());
    server.abort();
}

#[tokio::test]
async fn file_download_allows_data_that_keeps_arriving_slowly() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let chunks = (0..4)
        .map(|byte| (Duration::from_millis(100), vec![byte]))
        .collect();
    let (url, server) = spawn_stream_server(chunks, 4, false).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("slow.bin");

    let result = test_download(&url, &destination, 4).await.unwrap();

    assert_eq!(result.size, 4);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), [0, 1, 2, 3]);
    server.abort();
}

#[tokio::test]
async fn file_download_completes_normally() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let (url, server) =
        spawn_stream_server(vec![(Duration::ZERO, b"done".to_vec())], 4, false)
            .await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("complete.bin");

    let result = test_download(&url, &destination, 4).await.unwrap();

    assert_eq!(result.size, 4);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"done");
    server.abort();
}

#[tokio::test]
async fn segmentation_disabled_downloads_use_a_single_connection() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let (url, requests, normal_requests, server) =
        spawn_range_server(data, RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("batch.bin");

    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_segmented_download(false)
            .with_integrity(Integrity::default().with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, size as u64);
    assert_eq!(requests.load(Ordering::Relaxed), 1);
    assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
    server.abort();
}

#[tokio::test]
async fn file_download_drops_a_missing_route_before_later_rounds() {
    let (missing_url, missing_requests, missing_server) =
        spawn_http_fixture(HttpFixture::new("404 Not Found", Vec::new())).await;
    let (fallback_url, fallback_requests, fallback_server) =
        spawn_http_fixture(HttpFixture::new("200 OK", b"done")).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("fallback.bin");

    let result = download_to_path(
        DownloadRequest::new(&missing_url, ResourceClass::Other)
            .with_candidate_urls([fallback_url])
            .with_integrity(Integrity::default().with_size(4)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, 4);
    assert_eq!(missing_requests.load(Ordering::Relaxed), 1);
    assert_eq!(fallback_requests.load(Ordering::Relaxed), 1);
    missing_server.abort();
    fallback_server.abort();
}

#[tokio::test]
async fn file_download_bounds_server_error_retries_to_three_rounds() {
    let (url, requests, server) = spawn_http_fixture(HttpFixture::new(
        "503 Service Unavailable",
        Vec::new(),
    ))
    .await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("unavailable.bin");
    let request_url = format!("{url}?token=secret#fragment");

    let error = test_download(&request_url, &destination, 4)
        .await
        .unwrap_err();
    let diagnostic = error.to_string();

    assert_eq!(requests.load(Ordering::Relaxed), 3);
    assert!(diagnostic.contains("after 3/3 attempts"));
    assert!(diagnostic.contains("category=http"));
    assert!(diagnostic.contains("status=Some(503)"));
    assert!(!diagnostic.contains("secret"));
    assert!(!diagnostic.contains("fragment"));
    assert!(!destination.exists());
    server.abort();
}

#[tokio::test]
async fn rate_limited_route_cools_down_and_switches_without_waiting() {
    let (limited_url, limited_requests, limited_server) = spawn_http_fixture(
        HttpFixture::new("429 Too Many Requests", Vec::new())
            .with_headers("Retry-After: 60\r\n"),
    )
    .await;
    let (fallback_url, fallback_requests, fallback_server) =
        spawn_http_fixture(HttpFixture::new("200 OK", b"done")).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("rate-limit-fallback.bin");
    let started = Instant::now();

    let result = download_to_path(
        DownloadRequest::new(&limited_url, ResourceClass::Other)
            .with_candidate_urls([fallback_url])
            .with_integrity(Integrity::default().with_size(4)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, 4);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(limited_requests.load(Ordering::Relaxed), 1);
    assert_eq!(fallback_requests.load(Ordering::Relaxed), 1);
    limited_server.abort();
    fallback_server.abort();
}

#[tokio::test]
async fn canceling_a_file_download_drops_the_response_read_promptly() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let (url, server) = spawn_stream_server(Vec::new(), 4, true).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("canceled.bin");
    let task =
        tokio::spawn(async move { test_download(&url, &destination, 4).await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    let join_error = tokio::time::timeout(Duration::from_millis(100), task)
        .await
        .expect("canceled download should stop promptly")
        .unwrap_err();

    assert!(join_error.is_cancelled());
    server.abort();
}

#[tokio::test]
async fn tianpao_unknown_downloads_preserve_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.bin");
    tokio::fs::write(&path, b"existing").await.unwrap();
    let error = download_to_path(
        DownloadRequest::new(
            "https://mod.tianpao.top/unknown",
            ResourceClass::Modpack,
        ),
        &path,
        &FetchSemaphore(Semaphore::new(1)),
        None,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("retired"));
    assert_eq!(tokio::fs::read(path).await.unwrap(), b"existing");
}

#[tokio::test]
async fn mod_validator_changes_do_not_disable_the_next_pack_ranges() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    let size = 12 * 1024 * 1024;
    let data = Arc::new(vec![23_u8; size]);
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, normal_requests, server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            vary_mod_validators: true,
            ..Default::default()
        },
    )
    .await;
    let directory = tempfile::tempdir().unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(32));

    for file in ["first.jar", "second.jar", "next.mrpack"] {
        let mut file_url = url::Url::parse(&url).unwrap();
        file_url.set_path(file);
        let file_url = file_url.to_string();
        let destination = directory.path().join(file);
        let part = suffixed_path(&destination, ".part");
        let _disk =
            super::super::super::download::local_resources::set_test_disk_kind(
                &part,
                sysinfo::DiskKind::SSD,
            );
        let integrity = if file.ends_with(".mrpack") {
            Integrity::sha1(&hash).with_size(size as u64)
        } else {
            Integrity::default().with_size(size as u64)
        };
        let request = DownloadRequest::new(&file_url, ResourceClass::Other)
            .with_integrity(integrity);
        let before = requests.load(Ordering::Relaxed);
        let result =
            download_benchmark_fixture(request, &destination, &semaphore)
                .await
                .unwrap();
        assert_eq!(result.size, size as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        let file_route =
            route(file_url, DownloadRouteSource::Official, false, true);
        assert!(
            range_splitting_allowed(&file_route),
            "object version changes must not blacklist the authority"
        );
        if file.ends_with(".mrpack") {
            assert!(
                requests.load(Ordering::Relaxed) - before >= 4,
                "the next pack must still use independent ranges"
            );
            assert_eq!(normal_requests.load(Ordering::Relaxed), 2);
        }
    }

    assert_eq!(semaphore.0.available_permits(), 32);
    server.abort();
}

#[tokio::test]
async fn verified_files_keep_32_ranges_on_unknown_disks_when_validators_differ()
{
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    let size = 16 * 1024 * 1024;
    let data = Arc::new(vec![31_u8; size]);
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, normal_requests, server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            vary_range_validators: true,
            ..Default::default()
        },
    )
    .await;
    let directory = tempfile::tempdir().unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(64));

    for (resource, valid_hash, known_size) in [
        (ResourceClass::Modpack, true, true),
        (ResourceClass::MinecraftLibrary, true, true),
        (ResourceClass::Java, true, false),
        (ResourceClass::Modrinth, true, true),
        (ResourceClass::CurseForge, false, true),
    ] {
        let destination = directory
            .path()
            .join(format!("file-{resource:?}-{valid_hash}"));
        let part = suffixed_path(&destination, ".part");
        let _disk =
            super::super::super::download::local_resources::set_test_disk_kind(
                &part,
                sysinfo::DiskKind::Unknown(0),
            );
        let mut integrity = Integrity::sha1(if valid_hash {
            &hash
        } else {
            "0000000000000000000000000000000000000000"
        });
        if known_size {
            integrity.size = Some(size as u64);
        }
        let request =
            DownloadRequest::new(&url, resource).with_integrity(integrity);
        let before = requests.load(Ordering::Relaxed);
        let result =
            download_benchmark_fixture(request, &destination, &semaphore).await;
        assert!(
            requests.load(Ordering::Relaxed) - before >= 32,
            "{resource:?}, known_size={known_size}: fewer than 32 requests"
        );
        if valid_hash {
            assert_eq!(
                normal_requests.load(Ordering::Relaxed),
                usize::from(!known_size),
                "{resource:?}"
            );
        }
        if valid_hash {
            assert_eq!(result.unwrap().size, size as u64);
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        } else {
            assert!(
                result.is_err(),
                "metadata tolerance must never publish content with the wrong hash"
            );
            assert!(!destination.exists());
        }
        assert_eq!(semaphore.0.available_permits(), 64);
        normal_requests.store(0, Ordering::Relaxed);
    }
    server.abort();
}

#[tokio::test]
async fn ignored_ranges_fall_back_to_one_full_file_write() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let (url, requests, normal_requests, server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            ignore_range: true,
            ..Default::default()
        },
    )
    .await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("ignored-range.bin");
    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::default().with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(8)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, size as u64);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    assert!(requests.load(Ordering::Relaxed) >= 2);
    assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
    for index in 0..MAX_SEGMENT_CONCURRENCY {
        assert!(
            !segment_path(&suffixed_path(&destination, ".part"), index)
                .exists()
        );
    }
    server.abort();
}

#[tokio::test]
async fn canceling_segmented_download_releases_permits_and_temp_files() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 4) as usize;
    let data = Arc::new(vec![13_u8; size]);
    let (url, _, _, server) = spawn_range_server(
        data,
        RangeServerBehavior {
            slow_body: true,
            ..Default::default()
        },
    )
    .await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("canceled-segments.bin");
    let destination_for_task = destination.clone();
    let semaphore = Arc::new(FetchSemaphore(Semaphore::new(8)));
    let semaphore_for_task = Arc::clone(&semaphore);
    let task = tokio::spawn(async move {
        download_to_path(
            DownloadRequest::new(url, ResourceClass::Other)
                .with_integrity(Integrity::default().with_size(size as u64)),
            destination_for_task,
            &semaphore_for_task,
            None,
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_millis(25)).await;

    assert_eq!(semaphore.0.available_permits(), 8);
    let part_path = suffixed_path(&destination, ".part");
    assert!(!part_path.exists());
    for index in 0..MAX_SEGMENT_CONCURRENCY {
        assert!(!segment_path(&part_path, index).exists());
    }
    server.abort();
}

#[tokio::test]
async fn valid_existing_destination_is_reused_without_network_request() {
    let data = Arc::new(b"already complete".to_vec());
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, _, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("existing.bin");
    tokio::fs::write(&destination, &data[..]).await.unwrap();

    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(data.len() as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.attempts, 0);
    assert_eq!(requests.load(Ordering::Relaxed), 0);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    server.abort();
}

#[tokio::test]
async fn invalid_existing_destination_is_redownloaded() {
    let data = Arc::new(b"correct content".to_vec());
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, _, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("stale.bin");
    tokio::fs::write(&destination, b"wrong content")
        .await
        .unwrap();

    download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(data.len() as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(requests.load(Ordering::Relaxed), 1);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    server.abort();
}

#[tokio::test]
async fn unknown_size_and_small_files_use_one_connection() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    for (expected_size, integrity) in [
        (None, Integrity::default()),
        (Some(1024_u64), Integrity::default().with_size(1024)),
    ] {
        let data = Arc::new(vec![11_u8; 1024]);
        let (url, requests, normal_requests, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("single.bin");
        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(integrity),
            &destination,
            &FetchSemaphore(Semaphore::new(8)),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.size, expected_size.unwrap_or(1024));
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
        server.abort();
    }
}

#[tokio::test]
async fn single_connection_downloads_resume_from_partial_files() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = 512 * 1024_usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, normal_requests, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resumed.bin");
    let part_path = suffixed_path(&destination, ".part");
    tokio::fs::write(&part_path, &data[..size / 2])
        .await
        .unwrap();
    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, size as u64);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    assert_eq!(
        requests.load(Ordering::Relaxed),
        1,
        "the resumed download should finish with one range request"
    );
    assert_eq!(
        normal_requests.load(Ordering::Relaxed),
        0,
        "the resumed download should not restart from the beginning"
    );
    assert!(!part_path.exists());
    server.abort();
}

#[tokio::test]
async fn stale_partial_files_with_wrong_content_self_heal() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = 512 * 1024_usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, _, _, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("stale.bin");
    let part_path = suffixed_path(&destination, ".part");
    tokio::fs::write(&part_path, vec![0xAB_u8; size / 2])
        .await
        .unwrap();
    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, size as u64);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    server.abort();
}

#[tokio::test]
async fn switching_sources_does_not_resume_another_sources_partial_file() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = 512 * 1024_usize;
    let expected = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let stale = Arc::new(vec![0xAB_u8; size - 4096]);
    let hash = sha1_smol::Sha1::from(&expected[..]).hexdigest();
    let (stale_url, stale_requests, stale_normal_requests, stale_server) =
        spawn_range_server(stale, RangeServerBehavior::default()).await;
    let (
        official_url,
        official_requests,
        official_normal_requests,
        official_server,
    ) = spawn_range_server(expected.clone(), RangeServerBehavior::default())
        .await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("source-isolated.bin");

    let result = download_to_path(
        DownloadRequest::new(&stale_url, ResourceClass::Other)
            .with_candidate_urls([official_url])
            .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.source, DownloadRouteSource::Alternate);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *expected);
    assert_eq!(stale_requests.load(Ordering::Relaxed), 1);
    assert_eq!(stale_normal_requests.load(Ordering::Relaxed), 1);
    assert_eq!(official_requests.load(Ordering::Relaxed), 1);
    assert_eq!(
        official_normal_requests.load(Ordering::Relaxed),
        1,
        "the fallback source must restart instead of resuming mirror bytes",
    );
    stale_server.abort();
    official_server.abort();
}

#[tokio::test]
async fn segmented_fallback_keeps_partial_files_for_resume() {
    let _guard = fixtures::RANGE_SPLITTING_TEST_LOCK.lock().await;
    RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
    let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
    let data = Arc::new(
        (0..size)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>(),
    );
    let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
    let (url, requests, normal_requests, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("fallback-resume.bin");
    let part_path = suffixed_path(&destination, ".part");
    tokio::fs::write(&part_path, &data[..size / 4])
        .await
        .unwrap();
    // A single global permit forces the segmented attempt to fall back
    // before transferring anything; the preserved partial data must
    // survive that fallback and drive a resumed single connection.
    let result = download_to_path(
        DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
        &destination,
        &FetchSemaphore(Semaphore::new(1)),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.size, size as u64);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    assert_eq!(
        requests.load(Ordering::Relaxed),
        1,
        "the fallback should finish with one resumed range request"
    );
    assert_eq!(
        normal_requests.load(Ordering::Relaxed),
        0,
        "the preserved partial data should not be discarded"
    );
    server.abort();
}
