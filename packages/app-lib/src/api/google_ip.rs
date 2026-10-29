//! Direct-connect IP selection for Google Translate.
//!
//! `translate-pa.googleapis.com` is pinned in memory to a probed IPv4 so the
//! HTTPS URL, Host header, and TLS SNI stay unchanged while TCP connects to
//! the selected IP. The system hosts file, IPv6, and TLS certificate
//! verification are never used or modified.

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::state::store::Collection as _;
use crate::state::store::derived_db::DerivedDb;
use crate::state::store::google_translate_ip_cache::{
    GoogleTranslateIpCache, GoogleTranslateIpCacheRecord,
};
use crate::util::proxy::ProxyConfig;
use crate::{ErrorKind, State};
use futures::{StreamExt, stream};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// IP list source: Ponderfly/GoogleTranslateIpCheck, MIT License.
// https://github.com/Ponderfly/GoogleTranslateIpCheck
const IP_LIST_URL: &str = "https://ghfast.top/https://raw.githubusercontent.com/Ponderfly/GoogleTranslateIpCheck/refs/heads/master/src/GoogleTranslateIpCheck/GoogleTranslateIpCheck/ip.txt";
const GOOGLE_TRANSLATE_HOST: &str = "translate-pa.googleapis.com";
const TOP_IPS: usize = 20;
const SCAN_BATCH_SIZE: usize = 1000;
const PROBE_CONCURRENCY: usize = 32;
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20);
const CACHE_REFRESH_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GoogleTranslateIp {
    pub ip: String,
    pub latency_ms: u64,
}

#[derive(Debug)]
struct GoogleIpRuntime {
    candidates: Vec<GoogleTranslateIp>,
    current: usize,
    client: Option<Client>,
    pinned_ip: Option<String>,
}

impl GoogleIpRuntime {
    fn current_ip(&self) -> Option<IpAddr> {
        self.candidates
            .get(self.current)
            .and_then(|candidate| parse_ipv4(&candidate.ip))
    }
}

struct RefreshHandle {
    task: tokio::task::JoinHandle<()>,
    done: tokio::sync::watch::Receiver<bool>,
}

static RUNTIME: LazyLock<Mutex<Option<GoogleIpRuntime>>> =
    LazyLock::new(|| Mutex::new(None));
static REFRESH_TASK: LazyLock<Mutex<Option<Arc<RefreshHandle>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Returns a reqwest client pinned to the currently selected Google Translate
/// IPv4 address.
pub async fn google_translation_client() -> crate::Result<Client> {
    let proxy = State::get().await?.proxy_config().await.unwrap_or_default();
    loop {
        let mut runtime = RUNTIME.lock().await;
        match runtime.as_mut() {
            Some(runtime) if runtime.current < runtime.candidates.len() => {
                let pinned_matches = runtime
                    .candidates
                    .get(runtime.current)
                    .is_some_and(|candidate| {
                        runtime.pinned_ip.as_deref()
                            == Some(candidate.ip.as_str())
                    });
                if let Some(client) = runtime.client.as_ref()
                    && pinned_matches
                {
                    return Ok(client.clone());
                }
                let Some(ip) = runtime.current_ip() else {
                    runtime.current += 1;
                    runtime.client = None;
                    runtime.pinned_ip = None;
                    continue;
                };
                let client = client_for(ip, &proxy);
                runtime.client = Some(client.clone());
                runtime.pinned_ip = Some(ip.to_string());
                return Ok(client);
            }
            Some(_) => {
                drop(runtime);
                let task = start_refresh().await;
                wait_for_refresh(&task).await;
                return refreshed_client().await;
            }
            None => {
                drop(runtime);
                return initialize().await;
            }
        }
    }
}

/// Marks the current pinned IP as failed so the next call moves to a cached
/// backup or triggers a background rescan.
pub async fn mark_current_failed() {
    let mut runtime = RUNTIME.lock().await;
    if let Some(runtime) = runtime.as_mut() {
        let failed_ip = runtime.current_ip();
        runtime.current = runtime.current.saturating_add(1);
        runtime.client = None;
        runtime.pinned_ip = None;
        tracing::warn!(
            ip = ?failed_ip.map(|ip| ip.to_string()),
            next_index = runtime.current,
            pool_size = runtime.candidates.len(),
            "Google Translate IP marked failed; switching to next cached IP"
        );
    }
}

/// Returns the number of cached Google Translate IPs currently available.
pub async fn ip_pool_size() -> usize {
    {
        let runtime = RUNTIME.lock().await;
        if let Some(runtime) = runtime.as_ref() {
            return runtime.candidates.len();
        }
    }
    let Ok(state) = State::get().await else {
        tracing::warn!(
            "Unable to resolve launcher state for Google Translate IP cache"
        );
        return 0;
    };
    let derived = &state.derived;
    let stale = cache_is_stale(derived).await;
    let cached = load_cache(derived).await;
    let size = cached.len();
    if cached.is_empty() {
        tracing::warn!(
            "Google Translate IP cache is missing or empty; starting background refresh"
        );
        let _ = start_refresh().await;
    } else if stale {
        tracing::info!(
            size,
            "Google Translate IP cache is stale; starting background refresh"
        );
        let _ = start_refresh().await;
    } else {
        tracing::info!(size, "Google Translate IP cache loaded from database");
    }
    size
}

async fn initialize() -> crate::Result<Client> {
    let proxy = State::get().await?.proxy_config().await.unwrap_or_default();
    preload().await;
    {
        let mut runtime = RUNTIME.lock().await;
        if let Some(runtime) = runtime.as_mut()
            && runtime.current < runtime.candidates.len()
        {
            if let Some(client) = runtime.client.as_ref() {
                return Ok(client.clone());
            }
            if let Some(ip) = runtime.current_ip() {
                let client = client_for(ip, &proxy);
                runtime.client = Some(client.clone());
                runtime.pinned_ip = Some(ip.to_string());
                return Ok(client);
            }
        }
    }
    let task = start_refresh().await;
    wait_for_refresh(&task).await;
    refreshed_client().await
}

/// Warms the in-memory Google Translate IP pool without blocking startup.
///
/// Loads the cached Top 20 and verifies the first usable IP. A missing, empty,
/// stale, or fully failed cache only starts a background refresh and returns
/// immediately.
pub async fn preload() {
    let proxy = match State::get().await {
        Ok(state) => state.proxy_config().await.ok().unwrap_or_default(),
        Err(_) => ProxyConfig::default(),
    };
    {
        let runtime = RUNTIME.lock().await;
        if runtime
            .as_ref()
            .is_some_and(|runtime| runtime.current < runtime.candidates.len())
        {
            tracing::debug!("Google Translate IP pool already initialized");
            return;
        }
    }

    let Ok(state) = State::get().await else {
        tracing::warn!(
            "Unable to resolve launcher state during Google Translate IP preload"
        );
        return;
    };
    let derived = &state.derived;
    let candidates = load_cache(derived).await;
    let stale = cache_is_stale(derived).await;
    tracing::info!(
        cached = candidates.len(),
        "Preloading Google Translate IP pool"
    );
    if candidates.is_empty() {
        tracing::warn!(
            "Google Translate IP cache missing or empty during preload; starting background refresh"
        );
        let _ = start_refresh().await;
        return;
    }
    if stale {
        tracing::info!(
            "Google Translate IP cache stale during preload; starting background refresh"
        );
        let _ = start_refresh().await;
    }

    for (index, candidate) in candidates.iter().enumerate() {
        let Some(ip) = parse_ipv4(&candidate.ip) else {
            continue;
        };
        let Some(latency_ms) = probe(ip).await else {
            tracing::debug!(ip = %ip, "Cached Google Translate IP failed preload verification");
            continue;
        };
        tracing::info!(
            ip = %ip,
            latency_ms,
            "Cached Google Translate IP verified during preload"
        );
        let client = client_for(ip, &proxy);
        let mut runtime = RUNTIME.lock().await;
        *runtime = Some(GoogleIpRuntime {
            candidates,
            current: index,
            client: Some(client.clone()),
            pinned_ip: Some(ip.to_string()),
        });
        return;
    }

    tracing::warn!(
        "All cached Google Translate IPs failed preload verification; starting background refresh"
    );
    let _ = start_refresh().await;
}

async fn refreshed_client() -> crate::Result<Client> {
    let proxy = State::get().await?.proxy_config().await.unwrap_or_default();
    let mut runtime = RUNTIME.lock().await;
    if let Some(runtime) = runtime.as_mut()
        && let Some(ip) = runtime
            .candidates
            .first()
            .and_then(|candidate| parse_ipv4(&candidate.ip))
    {
        runtime.current = 0;
        let client = client_for(ip, &proxy);
        runtime.client = Some(client.clone());
        runtime.pinned_ip = Some(ip.to_string());
        return Ok(client);
    }
    Err(ErrorKind::OtherError(
        "GOOGLE_IP_UNAVAILABLE: no usable Google Translate IP found"
            .to_string(),
    )
    .into())
}

fn client_for(ip: IpAddr, proxy: &ProxyConfig) -> Client {
    tracing::info!(ip = %ip, "Pinning Google Translate requests to IP");
    let builder = Client::builder()
        .resolve(GOOGLE_TRANSLATE_HOST, SocketAddr::new(ip, 443))
        .timeout(Duration::from_secs(20))
        .user_agent(crate::launcher_user_agent());
    proxy
        .apply(builder)
        .expect("google translate proxy configuration should be valid")
        .build()
        .expect("google translate client configuration should be valid")
}

fn parse_ipv4(value: &str) -> Option<IpAddr> {
    value.trim().parse::<IpAddr>().ok().filter(IpAddr::is_ipv4)
}

fn parse_ip_list(content: &str) -> Vec<IpAddr> {
    let mut seen = HashSet::new();
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let ip = line.parse::<IpAddr>().ok()?;
            if !ip.is_ipv4() || !seen.insert(ip) {
                return None;
            }
            Some(ip)
        })
        .collect()
}

fn rank_candidates(
    mut candidates: Vec<GoogleTranslateIp>,
) -> Vec<GoogleTranslateIp> {
    let mut seen = HashSet::new();
    candidates.retain(|candidate| {
        parse_ipv4(&candidate.ip).is_some() && seen.insert(candidate.ip.clone())
    });
    candidates.sort_by_key(|candidate| candidate.latency_ms);
    candidates.truncate(TOP_IPS);
    candidates
}

async fn scan_batched(ips: &[IpAddr]) -> Vec<GoogleTranslateIp> {
    let mut offset = 0;
    while offset < ips.len() {
        let end = (offset + SCAN_BATCH_SIZE).min(ips.len());
        let batch = &ips[offset..end];
        let found = current_pool_size().await;
        tracing::info!(
            start = offset + 1,
            end,
            total = ips.len(),
            found,
            "Probing Google Translate IP batch"
        );
        let mut stream = stream::iter(batch.iter().copied())
            .map(|ip| async move { (ip, probe(ip).await) })
            .buffer_unordered(PROBE_CONCURRENCY);
        let mut full = false;
        while let Some((ip, latency)) = stream.next().await {
            let Some(latency_ms) = latency else {
                continue;
            };
            let size = insert_candidate(ip, latency_ms).await;
            tracing::info!(
                ip = %ip,
                latency_ms,
                size,
                "Google Translate IP added to pool"
            );
            if size >= TOP_IPS {
                full = true;
                break;
            }
        }
        drop(stream);
        if full || current_pool_size().await >= TOP_IPS {
            break;
        }
        offset = end;
    }
    let candidates = current_candidates().await;
    tracing::info!(
        candidates = candidates.len(),
        "Google Translate IP scan complete"
    );
    candidates
}

async fn insert_candidate(ip: IpAddr, latency_ms: u64) -> usize {
    let mut runtime = RUNTIME.lock().await;
    let runtime = runtime.get_or_insert_with(|| GoogleIpRuntime {
        candidates: Vec::new(),
        current: 0,
        client: None,
        pinned_ip: None,
    });
    let candidate = GoogleTranslateIp {
        ip: ip.to_string(),
        latency_ms,
    };
    if runtime
        .candidates
        .iter()
        .any(|existing| existing.ip == candidate.ip)
    {
        return runtime.candidates.len();
    }
    let index = runtime
        .candidates
        .binary_search_by(|existing| {
            existing.latency_ms.cmp(&candidate.latency_ms)
        })
        .unwrap_or_else(|index| index);
    runtime.candidates.insert(index, candidate);
    let became_best = index == 0;
    runtime.candidates.truncate(TOP_IPS);
    if became_best {
        runtime.current = 0;
        runtime.client = None;
        runtime.pinned_ip = None;
    }
    runtime.candidates.len()
}

async fn current_pool_size() -> usize {
    RUNTIME
        .lock()
        .await
        .as_ref()
        .map_or(0, |runtime| runtime.candidates.len())
}

async fn current_candidates() -> Vec<GoogleTranslateIp> {
    RUNTIME
        .lock()
        .await
        .as_ref()
        .map(|runtime| runtime.candidates.clone())
        .unwrap_or_default()
}

#[cfg(test)]
async fn scan_batched_with<F, Fut>(
    ips: &[IpAddr],
    mut probe: F,
) -> Vec<GoogleTranslateIp>
where
    F: FnMut(IpAddr) -> Fut,
    Fut: std::future::Future<Output = Option<u64>>,
{
    let mut candidates = Vec::new();
    let mut offset = 0;
    while offset < ips.len() && candidates.len() < TOP_IPS {
        let end = (offset + SCAN_BATCH_SIZE).min(ips.len());
        let batch = &ips[offset..end];
        tracing::info!(
            start = offset + 1,
            end,
            total = ips.len(),
            found = candidates.len(),
            "Probing Google Translate IP batch"
        );
        let probed = stream::iter(batch.iter().copied())
            .map(|ip| {
                let future = probe(ip);
                async move { (ip, future.await) }
            })
            .buffer_unordered(PROBE_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        candidates.extend(probed.into_iter().filter_map(|(ip, latency)| {
            latency.map(|latency_ms| GoogleTranslateIp {
                ip: ip.to_string(),
                latency_ms,
            })
        }));
        candidates = rank_candidates(candidates);
        offset = end;
    }
    tracing::info!(
        candidates = candidates.len(),
        "Google Translate IP scan complete"
    );
    candidates
}

async fn probe(ip: IpAddr) -> Option<u64> {
    let proxy = match State::get().await {
        Ok(state) => state.proxy_config().await.ok().unwrap_or_default(),
        Err(_) => ProxyConfig::default(),
    };
    let builder = Client::builder()
        .resolve(GOOGLE_TRANSLATE_HOST, SocketAddr::new(ip, 443))
        .connect_timeout(PROBE_CONNECT_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .user_agent(crate::launcher_user_agent());
    let client = match proxy.apply(builder) {
        Ok(builder) => builder.build().ok()?,
        Err(_) => {
            tracing::warn!(
                "Failed to apply proxy config for probe, using direct"
            );
            Client::builder()
                .resolve(GOOGLE_TRANSLATE_HOST, SocketAddr::new(ip, 443))
                .connect_timeout(PROBE_CONNECT_TIMEOUT)
                .timeout(PROBE_TIMEOUT)
                .user_agent(crate::launcher_user_agent())
                .no_proxy()
                .build()
                .ok()?
        }
    };
    let started = Instant::now();
    let response = client
        .get(format!("https://{GOOGLE_TRANSLATE_HOST}/"))
        .send()
        .await;
    let Ok(response) = response else {
        tracing::debug!(ip = %ip, "Google Translate IP probe failed");
        return None;
    };
    drop(response);
    let latency_ms = started.elapsed().as_millis() as u64;
    tracing::debug!(ip = %ip, latency_ms, "Google Translate IP probe succeeded");
    Some(latency_ms)
}

async fn download_ip_list() -> crate::Result<Vec<IpAddr>> {
    let proxy = State::get().await?.proxy_config().await.unwrap_or_default();
    tracing::info!(url = IP_LIST_URL, "Downloading Google Translate IP list");
    let builder = Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(crate::launcher_user_agent());
    let client = match proxy.apply(builder) {
        Ok(builder) => builder.build()?,
        Err(e) => {
            tracing::warn!(%e, "Failed to apply proxy config for IP list download, using direct");
            Client::builder()
                .timeout(DOWNLOAD_TIMEOUT)
                .user_agent(crate::launcher_user_agent())
                .no_proxy()
                .build()?
        }
    };
    let response = client.get(IP_LIST_URL).send().await?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Google Translate IP list download failed"
        );
        return Err(ErrorKind::OtherError(format!(
            "GOOGLE_IP_LIST_FAILED: Ponderfly IP list returned HTTP {}",
            response.status()
        ))
        .into());
    }
    let text = response.text().await?;
    let ips = parse_ip_list(&text);
    tracing::info!(
        bytes = text.len(),
        ips = ips.len(),
        "Google Translate IP list downloaded"
    );
    if ips.is_empty() {
        return Err(ErrorKind::OtherError(
            "GOOGLE_IP_LIST_FAILED: Ponderfly IP list contained no IPv4 addresses"
                .to_string(),
        )
        .into());
    }
    Ok(ips)
}

async fn refresh_in_background() {
    tracing::info!("Starting background Google Translate IP refresh");
    let result: crate::Result<()> = async {
        let ips = download_ip_list().await?;
        let candidates = scan_batched(&ips).await;
        if candidates.is_empty() {
            return Err(ErrorKind::OtherError(
                "GOOGLE_IP_UNAVAILABLE: no usable Google Translate IP found"
                    .to_string(),
            )
            .into());
        }

        let state = State::get().await?;
        if let Err(error) = save_cache(&state.derived, &candidates).await {
            tracing::warn!(%error, "Unable to persist Google Translate IP cache");
        }
        Ok(())
    }
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, "Background Google Translate IP refresh failed");
    } else {
        tracing::info!("Google Translate IP cache refreshed");
    }
}

async fn start_refresh() -> Arc<RefreshHandle> {
    let mut guard = REFRESH_TASK.lock().await;
    if let Some(handle) = guard.as_ref()
        && !handle.task.is_finished()
    {
        return handle.clone();
    }
    let (done_tx, done_rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(async move {
        refresh_in_background().await;
        let _ = done_tx.send(true);
    });
    let handle = Arc::new(RefreshHandle {
        task,
        done: done_rx,
    });
    *guard = Some(handle.clone());
    handle
}

async fn wait_for_refresh(handle: &RefreshHandle) {
    if handle.task.is_finished() {
        return;
    }
    let mut done = handle.done.clone();
    let _ = done.wait_for(|done| *done).await;
}

async fn load_cache(derived: &DerivedDb) -> Vec<GoogleTranslateIp> {
    let rows = match derived
        .read(async |handle| GoogleTranslateIpCache::get_all(handle).await)
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(
                %error,
                "Unable to read Google Translate IP cache from database; treating it as empty"
            );
            return Vec::new();
        }
    };
    let candidates = rows
        .into_iter()
        .take(TOP_IPS)
        .map(|(ip, record)| GoogleTranslateIp {
            ip,
            latency_ms: record.latency_ms.max(0) as u64,
        })
        .collect();
    rank_candidates(candidates)
}

async fn save_cache(
    derived: &DerivedDb,
    candidates: &[GoogleTranslateIp],
) -> crate::Result<()> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let records = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.ip.clone(),
                GoogleTranslateIpCacheRecord {
                    latency_ms: candidate.latency_ms as i64,
                    created_at,
                },
            )
        })
        .collect::<Vec<_>>();
    derived
        .write(async |handle| {
            GoogleTranslateIpCache::set_all(handle, &records).await
        })
        .await?;
    tracing::info!(
        count = candidates.len(),
        "Google Translate IP cache saved to database"
    );
    Ok(())
}

async fn cache_is_stale(derived: &DerivedDb) -> bool {
    let Ok(rows) = derived
        .read(async |handle| GoogleTranslateIpCache::get_all(handle).await)
        .await
    else {
        return true;
    };
    let Some(created_at) =
        rows.into_iter().map(|(_, record)| record.created_at).max()
    else {
        return true;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    now.saturating_sub(created_at) >= CACHE_REFRESH_AGE.as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_unique_ipv4_addresses() {
        let content = "# comment\n\n1.2.3.4\r\n2001:db8::1\n1.2.3.4\n8.8.8.8\n";
        assert_eq!(
            parse_ip_list(content),
            vec![
                "1.2.3.4".parse::<IpAddr>().unwrap(),
                "8.8.8.8".parse::<IpAddr>().unwrap(),
            ]
        );
    }

    #[test]
    fn ranks_and_truncates_candidates() {
        let candidates = (0..25)
            .map(|index| GoogleTranslateIp {
                ip: format!("10.0.{index}.1"),
                latency_ms: 1000 - index,
            })
            .collect();
        let ranked = rank_candidates(candidates);
        assert_eq!(ranked.len(), TOP_IPS);
        assert_eq!(ranked.first().unwrap().latency_ms, 976);
        assert!(
            ranked
                .windows(2)
                .all(|window| window[0].latency_ms <= window[1].latency_ms)
        );
    }

    #[tokio::test]
    async fn stops_scanning_after_first_batch_reaches_top() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let ips = (0..2500)
            .map(|index| {
                format!("10.{}.{}.1", index / 256, index % 256)
                    .parse::<IpAddr>()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let probed = Arc::new(AtomicUsize::new(0));
        let probe_count = probed.clone();
        let candidates = scan_batched_with(&ips, move |_ip| {
            let probe_count = probe_count.clone();
            async move {
                probe_count.fetch_add(1, Ordering::Relaxed);
                Some(10)
            }
        })
        .await;

        assert_eq!(candidates.len(), TOP_IPS);
        assert_eq!(probed.load(Ordering::Relaxed), SCAN_BATCH_SIZE);
    }

    #[tokio::test]
    async fn continues_to_next_batch_when_first_is_insufficient() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let ips = (0..2000)
            .map(|index| {
                let prefix = if index < SCAN_BATCH_SIZE { 10 } else { 11 };
                let octet = index % SCAN_BATCH_SIZE;
                format!("{prefix}.{}.{}.1", octet / 256, octet % 256,)
                    .parse::<IpAddr>()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let probed = Arc::new(AtomicUsize::new(0));
        let probe_count = probed.clone();
        let candidates = scan_batched_with(&ips, move |ip| {
            let probe_count = probe_count.clone();
            async move {
                probe_count.fetch_add(1, Ordering::Relaxed);
                let IpAddr::V4(ipv4) = ip else {
                    return None;
                };
                if ipv4.octets()[0] == 11 {
                    Some(5)
                } else {
                    None
                }
            }
        })
        .await;

        assert_eq!(probed.load(Ordering::Relaxed), 2000);
        assert_eq!(candidates.len(), TOP_IPS);
    }

    #[tokio::test]
    async fn cache_round_trip() {
        use crate::state::store::derived_db::FILE_NAME;

        let directory = tempfile::tempdir().unwrap();
        let derived = DerivedDb::open(directory.path().join(FILE_NAME)).await;
        let candidates = vec![
            GoogleTranslateIp {
                ip: "1.1.1.1".to_string(),
                latency_ms: 10,
            },
            GoogleTranslateIp {
                ip: "8.8.8.8".to_string(),
                latency_ms: 20,
            },
        ];
        save_cache(&derived, &candidates).await.unwrap();
        assert_eq!(load_cache(&derived).await, candidates);
    }
}
