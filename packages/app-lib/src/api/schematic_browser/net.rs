//! Network layer: per-host admission, circuit breaking, retries and log
//! redaction.
//!
//! Deliberately does **not** use `state.api_semaphore`: that semaphore is a
//! single process-wide pool shared by 110 call sites across 34 files and has no
//! host dimension, so a browse burst here would starve Modrinth/CurseForge/
//! Mojang metadata requests.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use parking_lot::Mutex as ParkingMutex;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::error::{NetworkKind, SchematicSourceError, SourceResult};

/// Maximum simultaneous requests to one host.
const PER_HOST_CONCURRENCY: usize = 3;
/// How many callers may wait for a slot before we start rejecting.
const PER_HOST_QUEUE_DEPTH: usize = 2;
/// Consecutive failures before the host is considered down.
const BREAKER_FAILURE_THRESHOLD: u32 = 3;
/// How long a tripped breaker stays open.
const BREAKER_OPEN_FOR: Duration = Duration::from_secs(60);
/// Base delay for the exponential retry backoff.
const RETRY_BASE_DELAY: Duration = Duration::from_millis(400);
/// Upper bound on a single retry delay.
const RETRY_MAX_DELAY: Duration = Duration::from_secs(8);
/// Requests a host allows inside one window, when it publishes a quota.
const QUOTA_LIMIT: u32 = 50;
/// Default window when the host does not tell us how long its quota lasts.
const QUOTA_WINDOW: Duration = Duration::from_secs(60);
/// Margin: stop spending once the reported remainder gets this low.
const QUOTA_RESERVE: u32 = 5;
/// Total attempts, including the first one.
const MAX_ATTEMPTS: usize = 3;

/// Key that scopes admission and breaker state.
///
/// Uses the authority plus the proxy policy so a proxied and a direct request to
/// the same host are accounted separately — they leave the machine through
/// different egress paths and have different failure modes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HostKey {
    pub authority: String,
    pub proxied: bool,
}

impl HostKey {
    pub fn from_url(url: &str, proxied: bool) -> Self {
        let authority = url
            .split("://")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .unwrap_or(url)
            .to_ascii_lowercase();
        Self { authority, proxied }
    }

    pub fn label(&self) -> String {
        if self.proxied {
            format!("{} (proxied)", self.authority)
        } else {
            self.authority.clone()
        }
    }
}

struct HostState {
    permits: tokio::sync::OwnedSemaphorePermit,
}

struct HostLimiter {
    semaphore: std::sync::Arc<Semaphore>,
}

struct BreakerState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

/// What a host said about its own rate limit.
///
/// Reden publishes `x-ratelimit-limit` / `remaining` / `reset`; the reset value
/// is an absolute Unix timestamp, not a duration.
#[derive(Clone, Debug, Default)]
pub struct QuotaHint {
    pub limit: Option<u32>,
    pub remaining: Option<u32>,
    /// Unix seconds at which the current window resets.
    pub reset_at: Option<i64>,
}

#[derive(Default)]
struct QuotaState {
    /// Requests spent since the window started, when the host reports no quota.
    spent: u32,
    window_started: Option<Instant>,
    /// Last reported remainder, refreshed by each response.
    remaining: Option<u32>,
    reset_at: Option<i64>,
}

static QUOTAS: LazyLock<Mutex<HashMap<HostKey, QuotaState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static LIMITERS: LazyLock<ParkingMutex<HashMap<HostKey, HostLimiter>>> =
    LazyLock::new(|| ParkingMutex::new(HashMap::new()));
static BREAKERS: LazyLock<Mutex<HashMap<HostKey, BreakerState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn limiter_for(key: &HostKey) -> std::sync::Arc<Semaphore> {
    let mut limiters = LIMITERS.lock();
    limiters
        .entry(key.clone())
        .or_insert_with(|| HostLimiter {
            semaphore: std::sync::Arc::new(Semaphore::new(
                PER_HOST_CONCURRENCY,
            )),
        })
        .semaphore
        .clone()
}

/// Acquire a slot for this host.
///
/// Rejects rather than queueing indefinitely: a browse burst that piles up behind
/// a slow host must not delay everything else. `tokio::sync::Semaphore` exposes
/// no queue-depth counter, so the depth is tracked here alongside it.
pub async fn acquire(
    key: &HostKey,
    cancel: &CancellationToken,
) -> SourceResult<tokio::sync::OwnedSemaphorePermit> {
    check_breaker(key)?;
    // Concurrency says how many run at once; the quota says how many may be sent
    // inside the host's window. Both have to hold.
    check_quota(key)?;

    let semaphore = limiter_for(key);

    // Count the caller as waiting *before* trying to acquire.
    //
    // The previous version only counted when `available_permits()` read zero,
    // which left a race: several callers could each observe a free permit, then
    // all queue inside `acquire_owned` without ever incrementing the counter,
    // bypassing the depth limit. Counting first and decrementing on drop keeps
    // the bound honest regardless of timing.
    {
        let mut waiting = WAITING.lock().unwrap();
        let count = waiting.entry(key.clone()).or_insert(0usize);
        if *count >= PER_HOST_CONCURRENCY + PER_HOST_QUEUE_DEPTH {
            return Err(SchematicSourceError::RateLimited {
                retry_after_ms: 1_000,
            });
        }
        *count += 1;
    }
    let guard = WaitingGuard { key: key.clone() };

    let acquire = semaphore.acquire_owned();
    let result = tokio::select! {
        _ = cancel.cancelled() => Err(SchematicSourceError::Canceled),
        result = acquire => result.map_err(|_| SchematicSourceError::network(
            NetworkKind::Unknown,
            "host limiter closed",
        )),
    };
    drop(guard);
    // Only count the request once the caller actually holds a permit. A
    // rejected or cancelled acquire never sends anything, so charging the
    // quota for it would let local exhaustion starve the host.
    if result.is_ok() {
        record_request(key);
    }
    result
}

/// Decrements the wait counter even if the caller is cancelled mid-wait.
struct WaitingGuard {
    key: HostKey,
}

impl Drop for WaitingGuard {
    fn drop(&mut self) {
        let mut waiting = WAITING.lock().unwrap();
        if let Some(count) = waiting.get_mut(&self.key) {
            *count = count.saturating_sub(1);
        }
    }
}

static WAITING: LazyLock<Mutex<HashMap<HostKey, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Refuse a request when the host's own quota is nearly spent.
///
/// Per-host concurrency only bounds how many requests run at once; it says
/// nothing about how many were sent inside the host's window. Reden publishes a
/// 50-per-window quota, so without this check a burst of pages would simply
/// exceed it and collect 429s.
fn check_quota(key: &HostKey) -> SourceResult<()> {
    let quotas = QUOTAS.lock().unwrap();
    let Some(state) = quotas.get(key) else {
        return Ok(());
    };

    // When the host told us when its window resets, trust that; otherwise fall
    // back to a local window.
    if let Some(reset_at) = state.reset_at {
        let now = chrono::Utc::now().timestamp();
        if now >= reset_at {
            return Ok(());
        }
        if let Some(remaining) = state.remaining
            && remaining <= QUOTA_RESERVE
        {
            return Err(SchematicSourceError::RateLimited {
                retry_after_ms: ((reset_at - now).max(1) as u64) * 1_000,
            });
        }
        return Ok(());
    }

    if state.spent >= QUOTA_LIMIT.saturating_sub(QUOTA_RESERVE) {
        let elapsed = state
            .window_started
            .map(|started| started.elapsed())
            .unwrap_or_default();
        let remaining = QUOTA_WINDOW.saturating_sub(elapsed);
        return Err(SchematicSourceError::RateLimited {
            retry_after_ms: remaining.as_millis().max(1) as u64,
        });
    }
    Ok(())
}

/// Account for one request against the host's quota.
fn record_request(key: &HostKey) {
    let mut quotas = QUOTAS.lock().unwrap();
    let state = quotas.entry(key.clone()).or_default();
    let now = Instant::now();
    match state.window_started {
        Some(started) if started.elapsed() < QUOTA_WINDOW => {
            state.spent = state.spent.saturating_add(1);
        }
        _ => {
            state.window_started = Some(now);
            state.spent = 1;
        }
    }
}

/// Update what the host told us about its remaining quota.
pub fn record_quota_hint(key: &HostKey, hint: QuotaHint) {
    let mut quotas = QUOTAS.lock().unwrap();
    let state = quotas.entry(key.clone()).or_default();
    if let Some(remaining) = hint.remaining {
        state.remaining = Some(remaining);
    }
    if let Some(reset_at) = hint.reset_at {
        state.reset_at = Some(reset_at);
    }
    let _ = hint.limit;
}

fn check_breaker(key: &HostKey) -> SourceResult<()> {
    let mut breakers = BREAKERS.lock().unwrap();
    let Some(state) = breakers.get_mut(key) else {
        return Ok(());
    };
    match state.open_until {
        Some(until) if Instant::now() < until => {
            Err(SchematicSourceError::CircuitOpen { host: key.label() })
        }
        Some(_) => {
            // Cooldown elapsed: reset the counter, matching upstream's breaker
            // which has no half-open probe state.
            state.open_until = None;
            state.consecutive_failures = 0;
            Ok(())
        }
        None => Ok(()),
    }
}

/// Record the outcome of one attempt.
pub fn record_result(key: &HostKey, failed: bool) {
    let mut breakers = BREAKERS.lock().unwrap();
    let state = breakers.entry(key.clone()).or_insert(BreakerState {
        consecutive_failures: 0,
        open_until: None,
    });
    if failed {
        state.consecutive_failures += 1;
        if state.consecutive_failures >= BREAKER_FAILURE_THRESHOLD {
            state.open_until = Some(Instant::now() + BREAKER_OPEN_FOR);
        }
    } else {
        state.consecutive_failures = 0;
        state.open_until = None;
    }
}

/// Exponential backoff with jitter, capped.
pub fn retry_delay(attempt: usize) -> Duration {
    let exponent = attempt.saturating_sub(1).min(8) as u32;
    let base = RETRY_BASE_DELAY.saturating_mul(1u32 << exponent);
    let capped = base.min(RETRY_MAX_DELAY);
    let jitter = rand::random::<f64>() * 0.3 + 0.85;
    capped.mul_f64(jitter)
}

/// Run `operation` with retries, honouring cancellation and the breaker.
///
/// Retries only when the error is retryable; a 403 (rejected `Origin`) or a
/// parse failure fails immediately because repeating it cannot help.
pub async fn with_retries<T, F, Fut>(
    key: &HostKey,
    cancel: &CancellationToken,
    mut operation: F,
) -> SourceResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = SourceResult<T>>,
{
    let mut attempt = 1;
    loop {
        if cancel.is_cancelled() {
            return Err(SchematicSourceError::Canceled);
        }
        let permit = acquire(key, cancel).await?;
        let result = operation().await;
        drop(permit);

        match result {
            Ok(value) => {
                record_result(key, false);
                return Ok(value);
            }
            Err(error) if error.is_retryable() && attempt < MAX_ATTEMPTS => {
                record_result(key, true);
                let delay = match &error {
                    SchematicSourceError::RateLimited { retry_after_ms } => {
                        Duration::from_millis(*retry_after_ms)
                    }
                    _ => retry_delay(attempt),
                };
                tokio::select! {
                    _ = cancel.cancelled() => {
                        return Err(SchematicSourceError::Canceled)
                    }
                    _ = tokio::time::sleep(delay) => {}
                }
                attempt += 1;
            }
            Err(error) => {
                record_result(key, true);
                return Err(error);
            }
        }
    }
}

/// Hide credentials before a URL reaches the logs.
///
/// `sanitize_url_for_log` upstream only strips the userinfo, query and fragment;
/// it cannot see a token embedded in a path segment, which is exactly where the
/// netdisk share tokens live.
pub fn redact_url(url: &str) -> String {
    let mut redacted =
        crate::util::fetch::sanitize_url_for_log(url).to_string();
    redacted = redact_path_segments(&redacted);
    redacted
}

/// Replace share-token path segments with a placeholder.
///
/// Handles the `/s/<token>` shape used by netdisk share links.
pub fn redact_path_segments(url: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut redact_next = false;
    for segment in url.split('/') {
        if redact_next && !segment.is_empty() {
            parts.push("<redacted>".to_string());
            redact_next = false;
            continue;
        }
        redact_next = segment == "s";
        parts.push(segment.to_string());
    }
    parts.join("/")
}

/// Header names whose values must never be logged.
const SENSITIVE_HEADERS: &[&str] = &["cookie", "set-cookie", "authorization"];

/// Render headers for logging with credentials masked.
pub fn redact_headers<'a, I>(headers: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    headers
        .into_iter()
        .map(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            if SENSITIVE_HEADERS.contains(&lower.as_str()) {
                (name.to_string(), "<redacted>".to_string())
            } else {
                (name.to_string(), value.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_key_uses_authority_and_proxy_flag() {
        let direct = HostKey::from_url("https://api.redenmc.com/x/y", false);
        let proxied = HostKey::from_url("https://api.redenmc.com/x/y", true);
        assert_eq!(direct.authority, "api.redenmc.com");
        assert_ne!(direct, proxied, "proxy policy must scope the key");
    }

    #[test]
    fn retry_delay_grows_then_caps() {
        let first = retry_delay(1);
        let third = retry_delay(3);
        assert!(third > first);
        assert!(retry_delay(20) <= RETRY_MAX_DELAY.mul_f64(1.2));
    }

    #[test]
    fn redaction_hides_share_tokens_and_headers() {
        let url = "https://pan.quark.cn/s/abc123?pwd=x";
        let redacted = redact_url(url);
        assert!(!redacted.contains("abc123"), "token leaked: {redacted}");
        assert!(!redacted.contains("pwd"));

        let headers = redact_headers(vec![
            ("Cookie", "session=secret"),
            ("User-Agent", "Axolotl/1.10.1"),
        ]);
        assert_eq!(headers[0].1, "<redacted>");
        assert_eq!(headers[1].1, "Axolotl/1.10.1");
    }

    #[test]
    fn breaker_opens_after_threshold_and_resets() {
        let key = HostKey::from_url("https://breaker-test.invalid/x", false);
        for _ in 0..BREAKER_FAILURE_THRESHOLD {
            record_result(&key, true);
        }
        assert!(matches!(
            check_breaker(&key),
            Err(SchematicSourceError::CircuitOpen { .. })
        ));

        // A success closes it again.
        record_result(&key, false);
        assert!(check_breaker(&key).is_ok());
    }
}
