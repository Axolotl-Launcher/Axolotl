//! Fetch probe tests.

use crate::util::download::constants::{
    FILE_VALIDATION_CONCURRENCY, ROUTE_PROBE_BYTES, ROUTE_PROBE_CONCURRENCY,
};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Semaphore;

use super::super::*;
use super::fixtures::*;

#[test]
fn route_probe_requires_a_twenty_five_percent_improvement() {
    assert!(!probe_is_meaningfully_faster(1_249_999, 1_000_000));
    assert!(probe_is_meaningfully_faster(1_250_000, 1_000_000));
    assert!(!probe_is_meaningfully_faster(u64::MAX, u64::MAX));
    assert_eq!(
        measured_bytes_per_second(
            ROUTE_PROBE_BYTES,
            Duration::from_millis(250),
        ),
        1024 * 1024,
    );
}

#[tokio::test]
async fn route_probe_selects_a_faster_distinct_authority() {
    let data = Arc::new(vec![7_u8; ROUTE_PROBE_BYTES as usize * 2]);
    let (url, requests, _, server) =
        spawn_range_server(data.clone(), RangeServerBehavior::default()).await;
    let current = route(
        "https://route-probe-current.invalid/file.jar".to_string(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    let mut candidate = route(url, DownloadRouteSource::Alternate, false, true);
    candidate.proxy = ProxyPolicy::Direct;
    let candidates = vec![candidate.clone()];
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let semaphore = FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY));

    let probe = probe_faster_route(
        &current,
        &candidates,
        64 * 1024,
        data.len() as u64,
        None,
        None,
        None,
        &semaphore,
        &client,
        &client,
        ResourceClass::Other,
        0,
    )
    .await
    .expect("the local range route should be measurably faster");

    assert_eq!(probe.route, candidate);
    assert!(probe.bytes_per_second >= 80 * 1024);
    assert_eq!(requests.load(Ordering::Relaxed), 1);

    let alias = route(
        "https://route-probe-alias.invalid/file.jar".to_string(),
        DownloadRouteSource::Mcim,
        true,
        true,
    );
    remember_effective_route_authority(&alias, &candidate.url);
    assert!(
        probe_faster_route(
            &alias,
            &candidates,
            1,
            data.len() as u64,
            None,
            None,
            None,
            &semaphore,
            &client,
            &client,
            ResourceClass::Other,
            0,
        )
        .await
        .is_none()
    );
    assert_eq!(requests.load(Ordering::Relaxed), 1);
    server.abort();
}

#[tokio::test]
async fn recovery_probe_prefers_measured_http1_without_recording_protocol_failure()
 {
    let data = Arc::new(vec![9_u8; 512 * 1024]);
    let (target_url, target_requests, _, target_server) =
        spawn_range_server(data.clone(), Default::default()).await;
    let (url, redirect_requests, redirect_server) =
        spawn_redirect_server(target_url.clone(), Duration::ZERO).await;
    let mut primary =
        route(url.clone(), DownloadRouteSource::Official, false, true);
    primary.proxy = ProxyPolicy::Direct;
    let request = DownloadRequest::new(&url, ResourceClass::Other)
        .with_integrity(Integrity::default().with_size(data.len() as u64));
    let semaphore = FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
    let selected = probe_recovery_route(
        &request,
        &primary,
        &[],
        1024,
        data.len() as u64,
        0,
        true,
        None,
        &semaphore,
    )
    .await
    .unwrap();
    let original = url_authority(&url).unwrap();
    let target = url_authority(&target_url).unwrap();
    assert_ne!(original, target);
    assert_eq!(selected.effective_authority, target);
    assert!(authority_uses_http1_fallback_for(&original, primary.proxy));
    assert!(authority_uses_http1_fallback_for(&target, primary.proxy));
    for authority in [&original, &target] {
        assert!(!H2_FALLBACK_AUTHORITIES.lock().contains_key(
            &super::super::super::download::proxy_context::authority_key(
                authority,
                primary.proxy
            )
        ));
    }
    assert_eq!(redirect_requests.load(Ordering::Relaxed), 1);
    assert_eq!(target_requests.load(Ordering::Relaxed), 1);
    redirect_server.abort();
    target_server.abort();
}

#[tokio::test]
async fn recovery_probe_compares_http1_and_alternate_cdn() {
    for http1_is_faster in [false, true] {
        let data = Arc::new(vec![9_u8; 512 * 1024]);
        let (url, primary_requests, _, primary_server) = spawn_range_server(
            data.clone(),
            RangeServerBehavior {
                slow_body: !http1_is_faster,
                ..Default::default()
            },
        )
        .await;
        let (alternate_url, alternate_requests, _, alternate_server) =
            spawn_range_server(
                data.clone(),
                RangeServerBehavior {
                    slow_body: http1_is_faster,
                    ..Default::default()
                },
            )
            .await;
        let mut primary =
            route(url.clone(), DownloadRouteSource::Official, false, true);
        primary.proxy = ProxyPolicy::Direct;
        let mut alternate =
            route(alternate_url, DownloadRouteSource::Alternate, false, true);
        alternate.proxy = ProxyPolicy::Direct;
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(
                Integrity::sha1(sha1_smol::Sha1::from(&data[..]).hexdigest())
                    .with_size(data.len() as u64),
            );
        let semaphore =
            FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
        let selected = probe_recovery_route(
            &request,
            &primary,
            std::slice::from_ref(&alternate),
            1024,
            data.len() as u64,
            0,
            true,
            None,
            &semaphore,
        )
        .await
        .unwrap();
        assert_eq!(
            selected.route,
            if http1_is_faster {
                primary.clone()
            } else {
                alternate
            }
        );
        assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
        assert_eq!(alternate_requests.load(Ordering::Relaxed), 1);
        assert_eq!(
            authority_uses_http1_fallback_for(
                &url_authority(&url).unwrap(),
                primary.proxy
            ),
            http1_is_faster
        );
        primary_server.abort();
        alternate_server.abort();
    }
}
