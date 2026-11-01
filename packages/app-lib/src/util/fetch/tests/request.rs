//! Fetch request tests.

use crate::ErrorKind;
use crate::util::download::constants::{
    FILE_VALIDATION_CONCURRENCY, METADATA_HEDGE_DELAY, ROUTE_PROBE_CONCURRENCY,
};
use crate::util::download::modrinth_redirect::is_official_redirect as is_official_modrinth_cdn_redirect;
use bytes::Bytes;
use reqwest::Method;
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Semaphore;

use super::super::*;
use super::fixtures::*;

#[tokio::test]
async fn metadata_hedging_returns_the_first_valid_response() {
    let (primary_url, primary_requests, primary_server) = spawn_http_fixture(
        HttpFixture::new("200 OK", br#"{"source":"primary"}"#)
            .with_headers("Content-Type: application/json\r\n")
            .with_delay(Duration::from_millis(300)),
    )
    .await;
    let (secondary_url, secondary_requests, secondary_server) =
        spawn_http_fixture(
            HttpFixture::new("200 OK", br#"{"source":"secondary"}"#)
                .with_headers("Content-Type: application/json\r\n"),
        )
        .await;
    let routes = [
        direct_test_route(primary_url, DownloadRouteSource::Official),
        direct_test_route(secondary_url, DownloadRouteSource::Mcim),
    ];
    let validate = |bytes: &Bytes| -> crate::Result<()> {
        serde_json::from_slice::<serde_json::Value>(bytes)?;
        Ok(())
    };

    let bytes = fetch_hedged_metadata(
        &routes,
        ResourceClass::Metadata,
        None,
        None,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        &INSECURE_REQWEST_CLIENT,
        &validate,
    )
    .await
    .unwrap();

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({ "source": "secondary" })
    );
    assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
    assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
    primary_server.abort();
    secondary_server.abort();
}

#[tokio::test]
async fn metadata_hedging_rejects_a_fast_invalid_primary() {
    let (primary_url, primary_requests, primary_server) = spawn_http_fixture(
        HttpFixture::new("200 OK", b"not-json")
            .with_headers("Content-Type: application/json\r\n"),
    )
    .await;
    let (secondary_url, secondary_requests, secondary_server) =
        spawn_http_fixture(
            HttpFixture::new("200 OK", br#"{"valid":true}"#)
                .with_headers("Content-Type: application/json\r\n"),
        )
        .await;
    let routes = [
        direct_test_route(primary_url, DownloadRouteSource::Official),
        direct_test_route(secondary_url, DownloadRouteSource::Mcim),
    ];
    let validate = |bytes: &Bytes| -> crate::Result<()> {
        serde_json::from_slice::<serde_json::Value>(bytes)?;
        Ok(())
    };

    let bytes = fetch_hedged_metadata(
        &routes,
        ResourceClass::Metadata,
        None,
        None,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        &INSECURE_REQWEST_CLIENT,
        &validate,
    )
    .await
    .unwrap();

    assert_eq!(bytes, Bytes::from_static(br#"{"valid":true}"#));
    assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
    assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
    primary_server.abort();
    secondary_server.abort();
}

#[tokio::test]
async fn metadata_hedging_does_not_start_a_loser_for_a_fast_primary() {
    let (primary_url, primary_requests, primary_server) = spawn_http_fixture(
        HttpFixture::new("200 OK", br#"{"valid":true}"#)
            .with_headers("Content-Type: application/json\r\n"),
    )
    .await;
    let (secondary_url, secondary_requests, secondary_server) =
        spawn_http_fixture(
            HttpFixture::new("200 OK", br#"{"valid":false}"#)
                .with_headers("Content-Type: application/json\r\n"),
        )
        .await;
    let routes = [
        direct_test_route(primary_url, DownloadRouteSource::Official),
        direct_test_route(secondary_url, DownloadRouteSource::Mcim),
    ];
    let validate = |bytes: &Bytes| -> crate::Result<()> {
        serde_json::from_slice::<serde_json::Value>(bytes)?;
        Ok(())
    };

    let _ = fetch_hedged_metadata(
        &routes,
        ResourceClass::Metadata,
        None,
        None,
        &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
        &INSECURE_REQWEST_CLIENT,
        &validate,
    )
    .await
    .unwrap();
    tokio::time::sleep(METADATA_HEDGE_DELAY * 2).await;

    assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
    assert_eq!(secondary_requests.load(Ordering::Relaxed), 0);
    primary_server.abort();
    secondary_server.abort();
}

#[tokio::test]
async fn xml_metadata_hedging_bypasses_slow_or_invalid_official_responses() {
    #[derive(Deserialize)]
    struct XmlVersion {
        version: String,
    }
    for (body, delay) in [
        (
            b"<metadata><version>old</version></metadata>".as_slice(),
            Duration::from_secs(2),
        ),
        (b"<html>upstream error</html>".as_slice(), Duration::ZERO),
    ] {
        let (primary_url, primary_requests, primary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", body).with_delay(delay),
            )
            .await;
        let valid = b"<metadata><version>21.1.200</version></metadata>";
        let (secondary_url, secondary_requests, secondary_server) =
            spawn_http_fixture(HttpFixture::new("200 OK", valid.as_slice()))
                .await;
        let routes = [
            direct_test_route(primary_url, DownloadRouteSource::Official),
            direct_test_route(secondary_url, DownloadRouteSource::Bmclapi),
        ];
        let validate = |bytes: &Bytes| -> crate::Result<()> {
            quick_xml::de::from_reader::<_, XmlVersion>(bytes.as_ref())
                .map(|_| ())
                .map_err(|error| {
                    ErrorKind::OtherError(error.to_string()).as_error()
                })
        };
        let bytes = tokio::time::timeout(
            Duration::from_secs(1),
            fetch_hedged_metadata(
                &routes,
                ResourceClass::Loader,
                None,
                None,
                &FetchSemaphore(Semaphore::new(2)),
                &INSECURE_REQWEST_CLIENT,
                &validate,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let parsed: XmlVersion =
            quick_xml::de::from_reader(bytes.as_ref()).unwrap();
        assert_eq!(parsed.version, "21.1.200");
        assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
        assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
        primary_server.abort();
        secondary_server.abort();
    }
}

#[tokio::test]
async fn typed_metadata_fetch_validates_xml_without_initializing_application_state()
 {
    #[derive(Deserialize)]
    struct XmlVersion {
        version: String,
    }
    let (url, requests, server) = spawn_http_fixture(HttpFixture::new(
        "200 OK",
        b"<metadata><version>21.1.200</version></metadata>".as_slice(),
    ))
    .await;
    let clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: crate::util::proxy::ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    let parsed = super::super::super::download::proxy_context::with_snapshot(
        clients,
        fetch_metadata(
            &url,
            |bytes| {
                quick_xml::de::from_reader::<_, XmlVersion>(bytes.as_ref())
                    .map_err(|error| {
                        ErrorKind::OtherError(error.to_string()).as_error()
                    })
            },
            &FetchSemaphore(Semaphore::new(2)),
        ),
    )
    .await
    .unwrap();
    assert_eq!(parsed.version, "21.1.200");
    assert_eq!(requests.load(Ordering::Relaxed), 1);
    server.abort();
}

#[test]
fn modrinth_cdn_redirects_only_fall_back_to_official_cdn() {
    assert!(is_official_modrinth_cdn_redirect(Some(
        "https://cdn.modrinth.com/data/project/versions/version/file.jar"
    )));
    assert!(is_official_modrinth_cdn_redirect(Some(
        "https://cdn-alt.modrinth.com/data/project/versions/version/file.jar"
    )));
    assert!(is_official_modrinth_cdn_redirect(Some(
        "https://CDN.MODRINTH.COM/data/project/versions/version/file.jar"
    )));
    assert!(!is_official_modrinth_cdn_redirect(Some(
        "https://cache.mcimirror.top/data/project/versions/version/file.jar"
    )));
    assert!(!is_official_modrinth_cdn_redirect(Some(
        "https://cdn.modrinth.com.evil.example/file.jar"
    )));
    assert!(!is_official_modrinth_cdn_redirect(Some(
        "https://cdn.modrinth.com@evil.example/file.jar"
    )));
    assert!(!is_official_modrinth_cdn_redirect(Some(
        "https://cdn-alt.modrinth.com/\u{4e0b}\u{8f7d}/file.jar"
    )));
    assert!(!is_official_modrinth_cdn_redirect(None));
}

#[tokio::test]
async fn nonempty_json_fetch_rejects_empty_collections() {
    let clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: crate::util::proxy::ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    super::super::super::download::proxy_context::with_snapshot(
        clients,
        async {
            let semaphore =
                FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));

            let (url, requests, server) =
                spawn_json_server("[]".to_string()).await;
            let result = fetch_json_nonempty::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
            )
            .await;
            assert!(
                result.is_err(),
                "an empty JSON array must be rejected as invalid data"
            );
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();

            let (url, requests, server) =
                spawn_json_server("[1,2,3]".to_string()).await;
            let result = fetch_json_nonempty::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
            )
            .await;
            let values =
                result.expect("a non-empty JSON array should be accepted");
            assert_eq!(values.len(), 3);
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();
        },
    )
    .await;
}

#[tokio::test]
async fn regular_json_fetch_still_accepts_empty_collections() {
    let clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: crate::util::proxy::ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    super::super::super::download::proxy_context::with_snapshot(
        clients,
        async {
            let semaphore =
                FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));

            let (url, requests, server) =
                spawn_json_server("[]".to_string()).await;
            let result = fetch_json::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
            )
            .await;
            let values = result
                .expect("regular JSON fetches keep accepting empty arrays");
            assert!(values.is_empty());
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();
        },
    )
    .await;
}
