use super::model::ProxyPolicy;
#[cfg(test)]
use crate::ErrorKind;
use crate::util::download::constants::{
    FILE_VALIDATION_CONCURRENCY, H2_FALLBACK_TTL, H2_SLOW_FALLBACK_TTL,
    MAX_GLOBAL_TAIL_HEDGES, ROUTE_PROBE_CONCURRENCY,
};
use crate::util::download_dns::DownloadDnsResolver;
use parking_lot::Mutex;
#[cfg(test)]
use reqwest::header;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Debug)]
pub struct IoSemaphore(pub Semaphore);

#[derive(Debug)]
pub struct FetchSemaphore(pub Semaphore);

pub(crate) static DOWNLOAD_DNS_RESOLVER: LazyLock<Arc<DownloadDnsResolver>> =
    LazyLock::new(|| Arc::new(DownloadDnsResolver::default()));

/// Forces download requests to resolve `host` through `resolver_host` while
/// preserving `host` in the URL, HTTP Host header, and TLS SNI.

#[allow(dead_code)]
pub fn set_download_dns_host_override(
    host: &str,
    resolver_host: &str,
) -> crate::Result<()> {
    DOWNLOAD_DNS_RESOLVER
        .set_host_override(host, resolver_host)
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
}

#[allow(dead_code)]
pub fn clear_download_dns_host_override(host: &str) -> crate::Result<()> {
    DOWNLOAD_DNS_RESOLVER
        .clear_host_override(host)
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
}

pub(crate) static TAIL_HEDGE_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(MAX_GLOBAL_TAIL_HEDGES));

static FILE_VALIDATION_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(FILE_VALIDATION_CONCURRENCY));

pub(crate) static ROUTE_PROBE_SEMAPHORE: LazyLock<FetchSemaphore> =
    LazyLock::new(|| FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)));

pub(crate) async fn acquire_native_validation_permit()
-> crate::Result<Option<SemaphorePermit<'static>>> {
    Ok(Some(FILE_VALIDATION_SEMAPHORE.acquire().await?))
}

pub(crate) static H2_FALLBACK_AUTHORITIES: LazyLock<
    Mutex<HashMap<String, Instant>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

static HTTP1_PREFERENCE_AUTHORITIES: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn authority_uses_http1_fallback(authority: &str) -> bool {
    authority_uses_http1_fallback_for(authority, ProxyPolicy::System)
}

pub(crate) fn authority_uses_http1_fallback_for(
    authority: &str,
    proxy: ProxyPolicy,
) -> bool {
    let authority =
        crate::util::download::proxy_context::authority_key(authority, proxy);
    {
        let mut preferences = HTTP1_PREFERENCE_AUTHORITIES.lock();
        preferences.retain(|_, until| *until > Instant::now());
        if preferences.contains_key(&authority) {
            return true;
        }
    }
    let mut fallbacks = H2_FALLBACK_AUTHORITIES.lock();
    match fallbacks.get(&authority) {
        Some(until) if *until > Instant::now() => true,
        Some(_) => {
            fallbacks.remove(&authority);
            false
        }
        None => false,
    }
}

pub(crate) fn record_authority_h2_failure(authority: &str) {
    record_authority_h2_failure_for(authority, ProxyPolicy::System);
}

pub(crate) fn record_authority_h2_failure_for(
    authority: &str,
    proxy: ProxyPolicy,
) {
    let now = Instant::now();
    let mut fallbacks = H2_FALLBACK_AUTHORITIES.lock();
    fallbacks.retain(|_, until| *until > now);
    fallbacks.insert(
        crate::util::download::proxy_context::authority_key(authority, proxy),
        now + H2_FALLBACK_TTL,
    );
}

pub(crate) fn prefer_measured_http1(authority: &str, proxy: ProxyPolicy) {
    let mut preferences = HTTP1_PREFERENCE_AUTHORITIES.lock();
    preferences.retain(|_, until| *until > Instant::now());
    preferences.insert(
        crate::util::download::proxy_context::authority_key(authority, proxy),
        Instant::now() + H2_SLOW_FALLBACK_TTL,
    );
}

pub(crate) fn configured_semaphore_limit(semaphore: &FetchSemaphore) -> usize {
    if let Some(state) = crate::State::get_if_initialized()
        && (std::ptr::eq(
            &raw const state.fetch_semaphore.0,
            &raw const semaphore.0,
        ) || std::ptr::eq(
            &raw const state.download_semaphore.0,
            &raw const semaphore.0,
        ) || std::ptr::eq(
            &raw const state.api_semaphore.0,
            &raw const semaphore.0,
        ))
    {
        return state.download_concurrency();
    }
    semaphore.0.available_permits().max(1)
}
