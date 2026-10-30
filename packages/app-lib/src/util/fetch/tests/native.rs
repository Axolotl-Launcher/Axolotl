//! Fetch native tests.

use crate::util::download::constants::FILE_VALIDATION_CONCURRENCY;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Semaphore;
use url::Url;

use super::super::super::download::constants::MAX_REDIRECT_LOCATION_BYTES;
use super::super::transfer::{H2AttemptResult, try_h2_download};
use super::super::*;
use super::fixtures::*;

#[test]
fn redirect_locations_are_bounded_ascii_values() {
    assert!(is_safe_redirect_location("/download/file.jar"));
    assert!(!is_safe_redirect_location("/\u{4e0b}\u{8f7d}/file.jar"));
    assert!(!is_safe_redirect_location(
        &"a".repeat(MAX_REDIRECT_LOCATION_BYTES + 1)
    ));
}

#[test]
fn malformed_official_cdn_redirects_reuse_the_original_encoded_path() {
    let original = Url::parse(
        "https://cdn-alt.modrinth.com/data/project/versions/version/%E9%87%91%E5%90%88%E6%AC%A2_1.21%2B.zip",
    )
    .unwrap();
    for host in ["cdn-alt.modrinth.com", "cdn.modrinth.com"] {
        let redirect = Url::parse(&format!(
            "https://{host}/data/project/versions/version/\u{91c8}\u{91c8}.zip",
        ))
        .unwrap();

        let repaired = repair_official_cdn_redirect(
            &original,
            &redirect,
            &format!("https://{host}/data/project/versions/version/\u{91c8}\u{91c8}.zip"),
        )
        .unwrap();

        assert_eq!(
            repaired.as_str(),
            format!(
                "https://cdn.modrinth.com/data/project/versions/version/%E9%87%91%E5%90%88%E6%AC%A2_1.21%2B.zip"
            )
        );
    }
}

#[derive(Clone, Copy)]
enum H2FailureFixture {
    Slow,
    Reset,
    ReceiveTimeout,
}

async fn check_h2_fallback(
    resume: bool,
    with_alternate: bool,
    failure: H2FailureFixture,
) {
    let data = Arc::new(vec![7_u8; 256 * 1024]);
    let (url, requests, normal_requests, http1_server) = spawn_range_server(
        data.clone(),
        RangeServerBehavior {
            fail_first_range: with_alternate,
            ..Default::default()
        },
    )
    .await;
    let mut primary =
        route(url.clone(), DownloadRouteSource::Official, false, true);
    primary.proxy = ProxyPolicy::Direct;
    let mut routes = vec![primary.clone()];
    let mut alternate_server = None;
    let mut alternate_requests = None;
    if with_alternate {
        let (url, requests, _, server) =
            spawn_range_server(data.clone(), Default::default()).await;
        let mut alternate =
            route(url, DownloadRouteSource::Alternate, false, true);
        alternate.proxy = ProxyPolicy::Direct;
        routes.push(alternate);
        alternate_server = Some(server);
        alternate_requests = Some(requests);
    }
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
    let h2_data = data.clone();
    let finish_healthy = Arc::new(tokio::sync::Notify::new());
    let finish_server = finish_healthy.clone();
    let h2_server = tokio::spawn(async move {
        let mut server = h2::server::handshake(server_io).await.unwrap();
        let (_, mut healthy) = server.accept().await.unwrap().unwrap();
        let mut healthy_body = healthy
            .send_response(http::Response::new(()), false)
            .unwrap();
        healthy_body
            .send_data(bytes::Bytes::from_static(&[3; 1024]), false)
            .unwrap();
        let healthy_sender = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = finish_server.notified() => break,
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        healthy_body.send_data(bytes::Bytes::from_static(&[3; 1024]), false).unwrap();
                    }
                }
            }
            healthy_body
                .send_data(bytes::Bytes::from_static(b"done"), true)
                .unwrap();
        });
        let (_, mut respond) = server.accept().await.unwrap().unwrap();
        let response = http::Response::builder()
            .header("content-length", h2_data.len())
            .body(())
            .unwrap();
        let mut body = respond.send_response(response, false).unwrap();
        let sender = tokio::spawn(async move {
            match failure {
                H2FailureFixture::Reset => {
                    body.send_reset(h2::Reason::CANCEL);
                    return;
                }
                H2FailureFixture::ReceiveTimeout => {
                    std::future::pending::<()>().await;
                    drop(body);
                    return;
                }
                H2FailureFixture::Slow => {}
            }
            for chunk in h2_data.chunks(1024) {
                if body
                    .send_data(bytes::Bytes::copy_from_slice(chunk), false)
                    .is_err()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        while server.accept().await.is_some() {}
        sender.abort();
        healthy_sender.abort();
    });
    let (sender, driver) = h2::client::handshake(client_io).await.unwrap();
    let connection = super::super::super::download::h2_pool::SharedH2Connection::cache_for_test(&primary, sender).await;
    let driver =
        super::super::super::download::h2_pool::spawn_connection_driver(
            &connection,
            driver,
            Duration::from_secs(60),
        );
    let healthy_activity = connection.track_stream();
    let mut scheduler = super::super::super::download::authority_scheduler::AuthorityScheduler::default();
    scheduler.add_connection(connection.clone());
    let mut healthy_body = connection
        .open(
            http::Request::builder()
                .uri("https://test.invalid/healthy")
                .body(())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body();
    let healthy_download = tokio::spawn(async move {
        let _activity = healthy_activity;
        let mut received = Vec::new();
        while let Some(chunk) = healthy_body.data().await {
            let chunk = chunk.unwrap();
            healthy_body
                .flow_control()
                .release_capacity(chunk.len())
                .unwrap();
            received.extend_from_slice(&chunk);
        }
        received
    });
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("fallback.bin");
    let part = suffixed_path(&destination, ".part");
    let integrity = if resume {
        Integrity::sha1(sha1_smol::Sha1::from(&data[..]).hexdigest())
            .with_size(data.len() as u64)
    } else {
        Integrity::default().with_size(data.len() as u64)
    };
    let request = DownloadRequest::new(&url, ResourceClass::Other)
        .with_integrity(integrity);
    let semaphore = FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
    let mut policy = super::super::super::download::native::h2_policy(
        &primary,
        request.integrity.size,
    )
    .await
    .unwrap();
    if !matches!(failure, H2FailureFixture::Slow) {
        policy.abort_if_slow = false;
    }
    assert!(matches!(
        try_h2_download(
            &request,
            primary.clone(),
            policy,
            &destination,
            &part,
            &semaphore
        )
        .await
        .unwrap(),
        H2AttemptResult::Fallback { .. }
    ));
    assert!(!connection.is_retiring());
    assert!(
        !connection.is_dead(),
        "healthy stream must keep its driver alive"
    );
    assert!(
        super::super::super::download::h2_pool::has_live_connection(&primary)
            .await
    );
    assert!(scheduler.select_connection().is_some());
    connection.release_stream_reservation();
    scheduler.prune();
    assert!(scheduler.select_connection().is_some());
    connection.release_stream_reservation();
    finish_healthy.notify_one();
    let healthy_bytes =
        tokio::time::timeout(Duration::from_secs(3), healthy_download)
            .await
            .unwrap()
            .unwrap();
    assert!(healthy_bytes.len() > 4);
    assert!(
        healthy_bytes[..healthy_bytes.len() - 4]
            .iter()
            .all(|byte| *byte == 3)
    );
    assert!(healthy_bytes.ends_with(b"done"));
    connection.retire();
    tokio::time::timeout(Duration::from_secs(3), driver)
        .await
        .unwrap()
        .unwrap();
    assert!(connection.is_dead());
    assert!(!authority_uses_http1_fallback_for(
        &url_authority(&url).unwrap(),
        primary.proxy
    ));
    if resume {
        let len = tokio::fs::metadata(&part).await.unwrap().len();
        assert!(len > 0 && len < data.len() as u64);
        assert_eq!(tokio::fs::read(&part).await.unwrap(), data[..len as usize]);
    } else {
        assert!(!part.exists());
    }
    let result = run_native_download_attempts(
        request,
        &destination,
        &semaphore,
        None,
        routes,
        part,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.size, data.len() as u64);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
    assert!(requests.load(Ordering::Relaxed) > 0);
    if resume && !with_alternate {
        assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
    }
    if let Some(requests) = alternate_requests {
        assert!(requests.load(Ordering::Relaxed) > 0);
    }
    if let Some(server) = alternate_server {
        server.abort();
    }
    h2_server.abort();
    http1_server.abort();
}

#[tokio::test]
async fn slow_h2_fallback_restarts_single_source_over_http1() {
    check_h2_fallback(false, false, H2FailureFixture::Slow).await;
}

#[tokio::test]
async fn slow_h2_fallback_resumes_and_verifies_partial_over_http1() {
    check_h2_fallback(true, false, H2FailureFixture::Slow).await;
}

#[tokio::test]
async fn slow_h2_fallback_can_recover_using_alternate_cdn() {
    check_h2_fallback(true, true, H2FailureFixture::Slow).await;
}

#[tokio::test]
async fn reset_h2_stream_does_not_interrupt_healthy_sibling() {
    check_h2_fallback(false, false, H2FailureFixture::Reset).await;
}

#[tokio::test]
async fn timed_out_h2_stream_does_not_interrupt_healthy_sibling() {
    check_h2_fallback(false, false, H2FailureFixture::ReceiveTimeout).await;
}

#[test]
fn redirect_hops_rebuild_the_same_range_header() {
    let original = byte_range_header_value(Some(1024), Some(2047));
    let redirected = byte_range_header_value(Some(1024), Some(2047));
    assert_eq!(original.as_deref(), Some("bytes=1024-2047"));
    assert_eq!(redirected, original);
}
