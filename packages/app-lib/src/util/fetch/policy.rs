#[cfg(test)]
use super::client;
use super::model::{DownloadRoute, ProxyPolicy, ResourceClass};
#[cfg(test)]
use super::routes::route;
use super::routes::{effective_route_authority, route_host};
use crate::util::download::constants::{
    API_KEY_HEADER, FETCH_AUTHORIZATION, FETCH_COOKIE, FETCH_GITHUB_COM,
    FETCH_MELOONG_COM, FETCH_MOMOT_RS, FETCH_OPTIFINE_NET,
    FETCH_PROXY_AUTHORIZATION, FETCH_RETRY_DELAYS, FETCH_RETRY_JITTER_MAX,
    FETCH_RETRY_JITTER_MIN, FETCH_V2_SESSION, FETCH_V2_USER, FETCH_V3_FRIEND,
    FETCH_V3_FRIENDS, FETCH_V3_NOTIFICATION, MAX_FAILURE_COOLDOWN,
    RANGE_SPLITTING_DISABLE_THRESHOLD, RETRY_AFTER_CAP,
};
use crate::util::download::route_health::{
    ROUTE_HEALTH, RouteHealth, RouteHealthKey,
};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rand::Rng;
#[cfg(test)]
use reqwest::StatusCode;
use reqwest::{Method, header};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::error::Error;
use std::sync::LazyLock;
use std::time::{self, Instant};
use url::Url;

pub(crate) fn route_health_key(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> Option<RouteHealthKey> {
    crate::util::download::route_health::route_health_key(route, resource)
}

fn update_ewma(current: &mut Option<f64>, sample: f64) {
    crate::util::download::route_health::update_ewma(current, sample);
}

pub(crate) fn persisted_route_health(
    key: &RouteHealthKey,
    proxy: ProxyPolicy,
) -> RouteHealth {
    crate::util::download::route_health::persisted_route_health(key, proxy)
}

pub(crate) fn fetch_retry_delay(attempt: usize) -> time::Duration {
    let base = FETCH_RETRY_DELAYS
        .get(attempt.saturating_sub(1))
        .copied()
        .unwrap_or(*FETCH_RETRY_DELAYS.last().unwrap());
    let jitter = rand::thread_rng()
        .gen_range(FETCH_RETRY_JITTER_MIN..=FETCH_RETRY_JITTER_MAX);
    time::Duration::from_secs_f64(base.as_secs_f64() * jitter)
}

pub(crate) fn retry_after(
    response: &reqwest::Response,
) -> Option<time::Duration> {
    retry_after_headers(response.headers())
}

pub(crate) fn retry_after_headers(
    headers: &header::HeaderMap,
) -> Option<time::Duration> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(clamp_failure_cooldown(time::Duration::from_secs(
            seconds,
        )));
    }

    let retry_at = DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&Utc);
    let seconds = retry_at.signed_duration_since(Utc::now()).num_seconds();
    Some(clamp_failure_cooldown(time::Duration::from_secs(
        seconds.max(0) as u64,
    )))
}

pub(crate) fn clamp_failure_cooldown(
    cooldown: time::Duration,
) -> time::Duration {
    cooldown.min(RETRY_AFTER_CAP)
}

pub(crate) fn is_sensitive_header(name: &str) -> bool {
    name.eq_ignore_ascii_case(FETCH_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_PROXY_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_COOKIE)
        || name.eq_ignore_ascii_case(API_KEY_HEADER)
}

pub(crate) fn header_requires_official_only(name: &str) -> bool {
    name.eq_ignore_ascii_case(FETCH_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_PROXY_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_COOKIE)
}

pub(crate) fn requires_modrinth_auth(
    method: &Method,
    header: Option<(&str, &str)>,
    uri_path: Option<&str>,
) -> bool {
    if method != Method::GET
        || header.is_some_and(|(name, _)| is_sensitive_header(name))
    {
        return true;
    }

    uri_path.is_some_and(|path| {
        matches!(path, FETCH_V2_USER | FETCH_V3_FRIENDS)
            || path.starts_with(FETCH_V2_SESSION)
            || path.starts_with(FETCH_V3_FRIEND)
            || path.starts_with(FETCH_V3_NOTIFICATION)
    })
}

pub(crate) fn record_route_success(
    route: &DownloadRoute,
    resource: ResourceClass,
    ttfb: time::Duration,
    bytes: u64,
    transfer_elapsed: time::Duration,
    remote_addr: Option<std::net::SocketAddr>,
) {
    if let (Some(host), Some(remote_addr)) = (route_host(route), remote_addr) {
        crate::util::download::proxy_context::resolver(route.proxy)
            .record_host_success(&host, remote_addr.ip());
    }
    if let Some(key) = route_health_key(route, resource) {
        let baseline = persisted_route_health(&key, route.proxy);
        let throughput_bps = (!transfer_elapsed.is_zero())
            .then(|| bytes as f64 / transfer_elapsed.as_secs_f64());
        crate::util::download::native_breaker::record_success(route);
        crate::util::download::native_reputation::record_success(
            key.family.as_str(),
            &key.authority,
            route.proxy,
            ttfb.as_secs_f64() * 1000.0,
            throughput_bps,
        );
        let mut health = ROUTE_HEALTH.lock();
        let entry = health.entry(key).or_insert(baseline);
        entry.success_samples = entry.success_samples.saturating_add(1);
        entry.consecutive_failures = 0;
        entry.cooldown_until = None;
        update_ewma(&mut entry.ttfb_ms, ttfb.as_secs_f64() * 1000.0);
        if !transfer_elapsed.is_zero() {
            update_ewma(
                &mut entry.throughput_bps,
                bytes as f64 / transfer_elapsed.as_secs_f64(),
            );
        }
    }
}

pub(crate) fn record_route_transfer_success(
    route: &DownloadRoute,
    resource: ResourceClass,
    bytes: u64,
    transfer_elapsed: time::Duration,
) {
    if transfer_elapsed.is_zero() {
        return;
    }
    let Some(key) = route_health_key(route, resource) else {
        return;
    };
    let baseline = persisted_route_health(&key, route.proxy);
    let throughput_bps = bytes as f64 / transfer_elapsed.as_secs_f64();
    crate::util::download::native_breaker::record_success(route);
    crate::util::download::native_reputation::record_transfer_success(
        key.family.as_str(),
        &key.authority,
        route.proxy,
        throughput_bps,
    );
    let mut health = ROUTE_HEALTH.lock();
    let entry = health.entry(key).or_insert(baseline);
    entry.success_samples = entry.success_samples.saturating_add(1);
    entry.consecutive_failures = 0;
    entry.cooldown_until = None;
    update_ewma(&mut entry.throughput_bps, throughput_bps);
}

pub(crate) fn record_route_failure(
    route: &DownloadRoute,
    resource: ResourceClass,
    cooldown: Option<time::Duration>,
) {
    if cooldown.is_some()
        && let Some(state) = crate::State::get_if_initialized()
    {
        state.record_download_throttle();
    }
    record_route_health_failure(route, resource, cooldown);
}

pub(crate) fn record_dns_connection_failure(
    route: &DownloadRoute,
    error: &reqwest::Error,
) -> Option<String> {
    if !error.is_connect() && !error.is_timeout() {
        return None;
    }
    let host = error
        .url()
        .and_then(Url::host_str)
        .map(str::to_owned)
        .or_else(|| route_host(route))?;
    crate::util::download::proxy_context::resolver(route.proxy)
        .record_connection_failure(&host)
        .then_some(host)
}

pub(crate) fn record_native_transfer_failure(
    route: &DownloadRoute,
    cooldown: Option<time::Duration>,
) {
    crate::util::download::native_breaker::record_failure_with_cooldown(
        route, cooldown,
    );
}

/// Records an HTTP status that asks the client to slow down. `429` and `503`
/// cool down their own bucket, so their pause is not reset by unrelated
/// connection failures on the same route.
pub(crate) fn record_native_status_failure(
    route: &DownloadRoute,
    status: reqwest::StatusCode,
    retry_after: Option<time::Duration>,
) {
    if !crate::util::download::native_breaker::record_response_status_failure(
        route,
        status.as_u16(),
        retry_after,
    ) {
        crate::util::download::native_breaker::record_failure_with_cooldown(
            route,
            retry_after,
        );
    }
}

pub(crate) fn record_route_health_failure(
    route: &DownloadRoute,
    resource: ResourceClass,
    cooldown: Option<time::Duration>,
) {
    if let Some(key) = route_health_key(route, resource) {
        let baseline = persisted_route_health(&key, route.proxy);
        crate::util::download::native_reputation::record_failure(
            key.family.as_str(),
            &key.authority,
            route.proxy,
        );
        let mut health = ROUTE_HEALTH.lock();
        let entry = health.entry(key).or_insert(baseline);
        entry.consecutive_failures =
            entry.consecutive_failures.saturating_add(1);
        if let Some(cooldown) = cooldown {
            entry.cooldown_until =
                Some(Instant::now() + cooldown.min(MAX_FAILURE_COOLDOWN));
        }
    }
}

pub(crate) static RANGE_SPLITTING_PROTOCOL_FAILURES: LazyLock<
    Mutex<HashMap<String, u32>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) static RANGE_SPLITTING_SUPPORTED: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

pub(crate) fn range_splitting_authority(
    route: &DownloadRoute,
) -> Option<String> {
    effective_route_authority(route)
}

pub(crate) fn range_splitting_allowed(route: &DownloadRoute) -> bool {
    let host = route_host(route).unwrap_or_default();
    if [
        FETCH_GITHUB_COM,
        FETCH_OPTIFINE_NET,
        FETCH_MOMOT_RS,
        FETCH_MELOONG_COM,
    ]
    .iter()
    .any(|blocked| host.contains(blocked))
    {
        return false;
    }

    range_splitting_authority(route).is_none_or(|authority| {
        let authority = crate::util::download::proxy_context::authority_key(
            &authority,
            route.proxy,
        );
        RANGE_SPLITTING_PROTOCOL_FAILURES
            .lock()
            .get(&authority)
            .copied()
            .unwrap_or(0)
            < RANGE_SPLITTING_DISABLE_THRESHOLD
    })
}

/// Records a range protocol failure for the route's server. A single failure
/// is treated as a transient blip, but once a server repeatedly mishandles
/// range requests, range splitting is disabled for it for this session so
/// later downloads skip the doomed segmented probe.
pub(crate) fn disable_range_splitting(route: &DownloadRoute) {
    let Some(authority) = range_splitting_authority(route) else {
        return;
    };
    let authority = crate::util::download::proxy_context::authority_key(
        &authority,
        route.proxy,
    );
    let mut failures = RANGE_SPLITTING_PROTOCOL_FAILURES.lock();
    let count = failures.entry(authority.clone()).or_insert(0);
    *count += 1;
    if *count == RANGE_SPLITTING_DISABLE_THRESHOLD {
        RANGE_SPLITTING_SUPPORTED.lock().remove(&authority);
        tracing::info!(
            authority,
            "Disabling range splitting for a server after repeated range protocol failures"
        );
    }
}

pub(crate) fn record_range_splitting_success(route: &DownloadRoute) {
    if let Some(authority) = range_splitting_authority(route) {
        RANGE_SPLITTING_SUPPORTED.lock().insert(
            crate::util::download::proxy_context::authority_key(
                &authority,
                route.proxy,
            ),
        );
    }
}
