//! Fetch client tests.

use super::super::*;
use super::fixtures::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn tianpao_metadata_redirects_are_blocked_before_following() {
    let (url, requests, server) = spawn_http_fixture(
        HttpFixture::new("302 Found", Vec::new())
            .with_headers("Location: https://mod.tianpao.top/unknown\r\n"),
    )
    .await;
    let client = reqwest_client_builder().no_proxy().build().unwrap();
    let error = client.get(url).send().await.unwrap_err();
    assert!(error.is_redirect());
    assert_eq!(requests.load(Ordering::Relaxed), 1);
    server.abort();
}
