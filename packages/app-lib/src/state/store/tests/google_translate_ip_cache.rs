//! What the Google Translate IP cache promises: the addresses a scan probed
//! are read back fastest first.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::google_translate_ip_cache::{
    GoogleTranslateIpCache, GoogleTranslateIpCacheRecord,
};

fn probed(latency_ms: i64, created_at: i64) -> GoogleTranslateIpCacheRecord {
    GoogleTranslateIpCacheRecord {
        latency_ms,
        created_at,
    }
}

#[tokio::test]
async fn probed_addresses_round_trip_fastest_first() {
    let (_directory, handle) = support::derived().await;

    GoogleTranslateIpCache::set_all(
        &handle,
        &[
            ("8.8.8.8".to_string(), probed(20, 10)),
            ("1.1.1.1".to_string(), probed(10, 10)),
        ],
    )
    .await
    .unwrap();

    assert_eq!(
        GoogleTranslateIpCache::get_all(&handle).await.unwrap(),
        vec![
            ("1.1.1.1".to_string(), probed(10, 10)),
            ("8.8.8.8".to_string(), probed(20, 10)),
        ]
    );
    assert_eq!(
        GoogleTranslateIpCache::get(&handle, &"1.1.1.1".to_string())
            .await
            .unwrap(),
        Some(probed(10, 10))
    );
}
