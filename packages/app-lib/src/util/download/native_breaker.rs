//! Short-lived route circuit breaker for native transfers.

use crate::util::download::constants::{
    NATIVE_BREAKER_JITTER_MAX, NATIVE_BREAKER_JITTER_MIN,
    NATIVE_BREAKER_MAX_EXPONENT,
};
use crate::util::fetch::{DownloadRoute, ProxyPolicy};
use parking_lot::Mutex;
use rand::Rng;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::util::download::constants::{
    NATIVE_BREAKER_DEFAULT_COOLDOWN as DEFAULT_COOLDOWN,
    NATIVE_BREAKER_FAILURE_THRESHOLD as FAILURE_THRESHOLD,
    NATIVE_BREAKER_MAX_COOLDOWN as MAX_COOLDOWN,
};
/// Why a route failed. Each cause keeps its own failure counter and cooldown,
/// so a burst of `429` responses cannot delay a route that is failing to
/// connect for an unrelated reason.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum BreakerCause {
    #[default]
    Transfer,
    RateLimited,
    ServiceUnavailable,
}

impl BreakerCause {
    fn from_status(status: u16) -> Option<Self> {
        match status {
            429 => Some(Self::RateLimited),
            503 => Some(Self::ServiceUnavailable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BreakerKey {
    authority: String,
    proxy: ProxyPolicy,
}

#[derive(Default)]
struct BreakerState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

#[derive(Default)]
struct BreakerEntry {
    causes: HashMap<BreakerCause, BreakerState>,
}

static BREAKERS: LazyLock<Mutex<HashMap<BreakerKey, BreakerEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn key(route: &DownloadRoute) -> Option<BreakerKey> {
    Some(BreakerKey {
        authority: super::proxy_context::authority_key(
            &crate::util::fetch::url_authority(&route.url)?,
            route.proxy,
        ),
        proxy: route.proxy,
    })
}

/// Expires a cooldown that has elapsed and reports whether the route is still
/// paused for this cause.
fn active(state: &mut BreakerState) -> bool {
    match state.open_until {
        Some(until) if until > Instant::now() => true,
        Some(_) => {
            state.open_until = None;
            state.consecutive_failures = 0;
            false
        }
        None => false,
    }
}

pub(crate) fn is_open(route: &DownloadRoute) -> bool {
    let Some(key) = key(route) else {
        return false;
    };
    let mut breakers = BREAKERS.lock();
    let Some(entry) = breakers.get_mut(&key) else {
        return false;
    };
    entry.causes.values_mut().any(active)
}

pub(crate) fn should_skip(
    route: &DownloadRoute,
    has_healthy_alternate: bool,
) -> bool {
    if !has_healthy_alternate {
        return false;
    }
    is_open(route)
}

/// Clears the cooldown for a route after a transfer succeeded. A server-provided
/// cooldown stays in place: `429` and `503` ask the client to slow down for a
/// specific period, and an unrelated success on the same route is not evidence
/// that the pause has expired.
pub(crate) fn record_success(route: &DownloadRoute) {
    let Some(key) = key(route) else {
        return;
    };
    if let Some(entry) = BREAKERS.lock().get_mut(&key) {
        entry.causes.remove(&BreakerCause::Transfer);
    }
}

pub(crate) fn record_failure(route: &DownloadRoute) {
    record_failure_with_cooldown(route, None);
}

/// Records a failed transfer. `retry_after` carries the server-provided
/// `Retry-After` when one was present; otherwise the cooldown grows
/// exponentially with the failure count and carries jitter so parallel
/// transfers do not retry in lockstep.
pub(crate) fn record_failure_with_cooldown(
    route: &DownloadRoute,
    retry_after: Option<Duration>,
) {
    record_failure_for_cause(route, retry_after, BreakerCause::Transfer);
}

/// Records a response status that asks the client to slow down. Returns
/// whether the status opened a cause-specific cooldown.
pub(crate) fn record_response_status_failure(
    route: &DownloadRoute,
    status: u16,
    retry_after: Option<Duration>,
) -> bool {
    let Some(cause) = BreakerCause::from_status(status) else {
        return false;
    };
    record_failure_for_cause(route, retry_after, cause);
    true
}

fn record_failure_for_cause(
    route: &DownloadRoute,
    retry_after: Option<Duration>,
    cause: BreakerCause,
) {
    let Some(key) = key(route) else {
        return;
    };
    let mut breakers = BREAKERS.lock();
    let state = breakers
        .entry(key)
        .or_default()
        .causes
        .entry(cause)
        .or_default();
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    if state.consecutive_failures < FAILURE_THRESHOLD {
        return;
    }
    let requested = retry_after
        .unwrap_or_else(|| backoff_cooldown(state.consecutive_failures));
    state.open_until = Some(Instant::now() + requested.min(MAX_COOLDOWN));
}

fn backoff_cooldown(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures
        .saturating_sub(FAILURE_THRESHOLD)
        .min(NATIVE_BREAKER_MAX_EXPONENT);
    let base = DEFAULT_COOLDOWN.saturating_mul(1 << exponent);
    let jitter = rand::thread_rng()
        .gen_range(NATIVE_BREAKER_JITTER_MIN..=NATIVE_BREAKER_JITTER_MAX);
    Duration::from_secs_f64(base.as_secs_f64() * jitter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fetch::DownloadRouteSource;

    fn route(host: &str) -> DownloadRoute {
        DownloadRoute {
            url: format!("https://{host}/file"),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        }
    }

    fn breakers_open_for(
        route: &DownloadRoute,
        cause: BreakerCause,
    ) -> Duration {
        let key = key(route).expect("route key");
        let breakers = BREAKERS.lock();
        let until = breakers
            .get(&key)
            .and_then(|entry| entry.causes.get(&cause))
            .and_then(|state| state.open_until);
        match until {
            Some(until) => until.saturating_duration_since(Instant::now()),
            None => Duration::ZERO,
        }
    }

    #[test]
    fn unique_route_is_never_skipped() {
        let route = route("unique-breaker.example");
        for _ in 0..FAILURE_THRESHOLD {
            record_failure(&route);
        }
        assert!(should_skip(&route, true));
        assert!(!should_skip(&route, false));
        record_success(&route);
    }

    #[test]
    fn a_success_does_not_release_a_server_provided_cooldown() {
        let route = route("server-cooldown-breaker.example");
        for _ in 0..FAILURE_THRESHOLD {
            assert!(record_response_status_failure(
                &route,
                429,
                Some(Duration::from_secs(30))
            ));
        }
        assert!(is_open(&route));

        record_success(&route);

        assert!(
            is_open(&route),
            "a rate-limit cooldown must survive an unrelated success"
        );
        assert!(
            breakers_open_for(&route, BreakerCause::RateLimited)
                > Duration::from_secs(20),
            "the server-provided pause is still running"
        );
        record_success(&route);
        record_success(&route);
    }

    #[test]
    fn a_success_clears_the_transfer_cooldown() {
        let route = route("transfer-cooldown-breaker.example");
        for _ in 0..FAILURE_THRESHOLD {
            record_failure(&route);
        }
        assert!(is_open(&route));

        record_success(&route);

        assert!(
            !is_open(&route),
            "a completed transfer clears the transfer cooldown"
        );
    }

    #[test]
    fn a_provided_retry_after_is_not_truncated_to_a_second() {
        let route = route("clamped-breaker.example");
        for _ in 0..(FAILURE_THRESHOLD - 1) {
            record_failure_with_cooldown(&route, None);
        }
        record_failure_with_cooldown(&route, Some(Duration::from_secs(30)));

        let opened_for = breakers_open_for(&route, BreakerCause::Transfer);
        assert!(
            opened_for > Duration::from_secs(3),
            "a 30s Retry-After should stay well above one second: {opened_for:?}"
        );
        assert!(opened_for <= Duration::from_secs(30));
        record_success(&route);
    }

    #[test]
    fn an_excessive_retry_after_is_capped() {
        let route = route("capped-breaker.example");
        for _ in 0..(FAILURE_THRESHOLD - 1) {
            record_failure_with_cooldown(&route, None);
        }
        record_failure_with_cooldown(&route, Some(Duration::from_secs(3600)));

        let opened_for = breakers_open_for(&route, BreakerCause::Transfer);
        assert!(
            opened_for <= MAX_COOLDOWN,
            "cooldown should be capped: {opened_for:?}"
        );
        record_success(&route);
    }

    #[test]
    fn failure_causes_keep_separate_cooldowns() {
        let route = route("cause-breaker.example");
        for _ in 0..FAILURE_THRESHOLD {
            record_failure(&route);
        }
        for _ in 0..FAILURE_THRESHOLD {
            assert!(record_response_status_failure(
                &route,
                429,
                Some(Duration::from_secs(30))
            ));
        }
        assert!(is_open(&route));

        let transfer = breakers_open_for(&route, BreakerCause::Transfer);
        let rate_limited = breakers_open_for(&route, BreakerCause::RateLimited);
        let unavailable =
            breakers_open_for(&route, BreakerCause::ServiceUnavailable);
        assert!(
            rate_limited > transfer,
            "the 429 bucket should keep its own longer cooldown: {rate_limited:?} vs {transfer:?}"
        );
        assert!(
            rate_limited > Duration::from_secs(20),
            "the 429 bucket should keep the server-provided cooldown: {rate_limited:?}"
        );
        assert_eq!(
            unavailable,
            Duration::ZERO,
            "an untouched cause must not inherit another cause's cooldown"
        );
        assert!(record_response_status_failure(
            &route,
            503,
            Some(Duration::from_secs(30))
        ));
        assert!(
            breakers_open_for(&route, BreakerCause::ServiceUnavailable)
                < Duration::from_secs(10)
        );
        assert!(!record_response_status_failure(&route, 500, None));
        record_success(&route);
    }

    #[test]
    fn exponential_backoff_grows_with_consecutive_failures() {
        let low = backoff_cooldown(FAILURE_THRESHOLD);
        let high = backoff_cooldown(FAILURE_THRESHOLD + 4);
        assert!(
            high > low,
            "backoff should grow with failures: {low:?} then {high:?}"
        );
        assert!(high <= MAX_COOLDOWN);
    }

    #[test]
    fn concurrent_failures_do_not_corrupt_the_breaker_map() {
        let handles = (0..8)
            .map(|index| {
                std::thread::spawn(move || {
                    let threaded =
                        route(&format!("threaded-breaker-{index}.example"));
                    for _ in 0..64 {
                        record_failure(&threaded);
                        record_failure_with_cooldown(
                            &threaded,
                            Some(Duration::from_secs(5)),
                        );
                        let _ = is_open(&threaded);
                        let _ = should_skip(&threaded, true);
                        record_success(&threaded);
                    }
                    // A shared authority is touched by every thread as well,
                    // so bucket replacement is exercised concurrently.
                    let shared = route("shared-breaker.example");
                    for _ in 0..64 {
                        record_failure(&shared);
                        record_success(&shared);
                    }
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().expect("breaker worker panicked");
        }
        let shared = route("shared-breaker.example");
        assert!(!is_open(&shared));
    }
}
