//! Fetch throttle tests.

use crate::util::download::constants::H2_FALLBACK_TTL;
#[cfg(test)]
use std::time::Duration;
use std::time::{self, Instant};

use super::super::*;
use super::fixtures;

#[test]
fn h2_fallback_outlives_immediate_retry_cleanup() {
    let _guard = fixtures::H2_FALLBACK_TEST_LOCK.lock();
    let authority = "h2-cooldown.example:443";
    record_authority_h2_failure(authority);
    assert!(authority_uses_http1_fallback(authority));
    let key = crate::util::download::proxy_context::authority_key(
        authority,
        ProxyPolicy::System,
    );
    let remaining = H2_FALLBACK_AUTHORITIES
        .lock()
        .get(&key)
        .map(|until| until.saturating_duration_since(Instant::now()))
        .unwrap();
    assert!(remaining <= H2_FALLBACK_TTL);
    assert!(remaining >= time::Duration::from_secs(4 * 60));
    H2_FALLBACK_AUTHORITIES.lock().remove(&key);
}

#[test]
fn global_download_dns_override_can_be_registered_and_cleared() {
    set_download_dns_host_override("request-host.test", "resolver-target.test")
        .unwrap();
    assert_eq!(
        DOWNLOAD_DNS_RESOLVER
            .host_override("request-host.test")
            .as_deref(),
        Some("resolver-target.test"),
    );
    clear_download_dns_host_override("request-host.test").unwrap();
    assert!(
        DOWNLOAD_DNS_RESOLVER
            .host_override("request-host.test")
            .is_none()
    );
}

#[test]
fn h2_fallback_authority_marking() {
    let _guard = fixtures::H2_FALLBACK_TEST_LOCK.lock();
    record_authority_h2_failure("example.com:443");
    assert!(authority_uses_http1_fallback("example.com:443"));
    assert!(!authority_uses_http1_fallback("other.com:443"));
    H2_FALLBACK_AUTHORITIES.lock().clear();
}
