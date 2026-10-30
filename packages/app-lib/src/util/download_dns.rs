#[cfg(test)]
use crate::util::download::constants::DOWNLOAD_DNS_TEST_DNS_LOOKUP_FAILED;
use crate::util::download::constants::{
    DNS_TIMEOUT_GRACE, DOH_CONNECT_TIMEOUT,
    DOWNLOAD_DNS_APPLICATION_DNS_MESSAGE,
    DOWNLOAD_DNS_CANDIDATE_DNS_HOST_QUERY_CAPACITY_EXHAUSTED,
    DOWNLOAD_DNS_CANDIDATE_DNS_LOCK_WAIT_TIMED_OUT,
    DOWNLOAD_DNS_CONFLICTING_DOH_CNAME_ANSWERS,
    DOWNLOAD_DNS_CYCLIC_DOH_CNAME_CHAIN,
    DOWNLOAD_DNS_DNS_HOST_OVERRIDE_MUST_BE_A_HOSTNAME_WITHOUT_A,
    DOWNLOAD_DNS_DNS_RESPONSE_CONTAINED_NO_ADDRESSES,
    DOWNLOAD_DNS_DOH_CNAME_OWNER_ALSO_CONTAINS_ADDRESS_RECORDS,
    DOWNLOAD_DNS_DOH_RESPONSE_CONTAINED_NO_ADDRESSES,
    DOWNLOAD_DNS_DOH_RESPONSE_IS_TOO_LARGE,
    DOWNLOAD_DNS_INVALID_OR_UNSUCCESSFUL_DOH_DNS_RESPONSE,
    DOWNLOAD_DNS_MISSING_DNS_QUESTION,
    DOWNLOAD_DNS_NO_CANDIDATE_DNS_ANSWERS_BEFORE_DEADLINE,
    DOWNLOAD_DNS_NO_DOH_PROVIDERS_AVAILABLE,
    DOWNLOAD_DNS_NO_USABLE_DOH_ADDRESS_FAMILY_BEFORE_DEADLINE,
    DOWNLOAD_DNS_SYSTEM_CANDIDATE_DNS_LOOKUP_TIMED_OUT,
};
use futures::stream::{FuturesUnordered, StreamExt};
use hickory_resolver::proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{Name as DnsName, RData, RecordType},
};
use parking_lot::Mutex;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub use crate::util::download::constants::BUILTIN_DOH_SERVERS;
use crate::util::download::constants::{
    DNS_CACHE_TTL as CACHE_TTL, DNS_CANDIDATE_CACHE_TTL as CANDIDATE_CACHE_TTL,
    DNS_CONNECTION_FAILURES_BEFORE_REFRESH as CONNECTION_FAILURES_BEFORE_REFRESH,
    DNS_FAILURE_CACHE_TTL as FAILURE_CACHE_TTL,
    DNS_FAMILY_COLLECTION_GRACE as FAMILY_COLLECTION_GRACE, DNS_LOOKUP_TIMEOUT,
    DNS_SUCCESSFUL_ADDRESS_CACHE_TTL as SUCCESSFUL_ADDRESS_CACHE_TTL,
    DOH_FALLBACK_TIMEOUT, DOH_REQUEST_TIMEOUT, MAX_DNS_MESSAGE_BYTES,
    MAX_METADATA_HOSTS,
};

fn make_metadata_room<T>(
    cache: &mut HashMap<String, T>,
    host: &str,
    fetched_at: impl Fn(&T) -> Instant,
) {
    if !cache.contains_key(host)
        && cache.len() >= MAX_METADATA_HOSTS
        && let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, value)| fetched_at(value))
            .map(|(host, _)| host.clone())
    {
        cache.remove(&oldest);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DnsSource {
    System,
    Doh(Arc<str>),
}

#[derive(Clone, Debug)]
pub(crate) struct DnsAddress {
    pub(crate) ip: IpAddr,
    /// Effective expiry includes every CNAME on the path to this address.
    pub(crate) expires_at: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct DnsAnswer {
    pub(crate) queried_host: String,
    /// One provider's ordered chain; never joined with another response.
    pub(crate) cname_chain: Vec<String>,
    pub(crate) addresses: Vec<DnsAddress>,
    pub(crate) fetched_at: Instant,
    pub(crate) source: DnsSource,
}

impl DnsAnswer {
    fn socket_addresses(&self) -> Vec<SocketAddr> {
        self.addresses
            .iter()
            .map(|address| SocketAddr::new(address.ip, 0))
            .collect()
    }
}

#[derive(Clone)]
struct CandidateSnapshot {
    answers: Vec<DnsAnswer>,
    fetched_at: Instant,
}

#[derive(Clone)]
struct CachedFailure {
    kind: std::io::ErrorKind,
    message: String,
    failed_at: Instant,
}

#[derive(Clone)]
struct CachedAddresses {
    addresses: Vec<IpAddr>,
    resolved_at: Instant,
    ttl: Duration,
    consecutive_connection_failures: u8,
}

impl CachedAddresses {
    fn is_fresh(&self) -> bool {
        self.resolved_at.elapsed() < self.ttl
    }
}

#[derive(Clone)]
pub struct DownloadDnsResolver {
    reliability: Arc<Mutex<HashMap<IpAddr, f64>>>,
    successful_addresses: Arc<Mutex<HashMap<String, HashMap<IpAddr, Instant>>>>,
    last_resolved: Arc<Mutex<HashMap<String, CachedAddresses>>>,
    last_failed: Arc<Mutex<HashMap<String, CachedFailure>>>,
    /// Locks only a single hostname's lookup. The map is held just long
    /// enough to obtain the per-host lock, never while DNS is awaited.
    resolving_hosts: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    host_overrides: Arc<Mutex<HashMap<String, String>>>,
    doh_enabled: bool,
    doh_endpoints: Arc<Vec<Arc<str>>>,
    doh_clients: Arc<Vec<reqwest::Client>>,
    doh_provider_weights: Arc<Mutex<HashMap<String, u8>>>,
    dns_answers: Arc<Mutex<HashMap<String, Vec<DnsAnswer>>>>,
    candidate_snapshots: Arc<Mutex<HashMap<String, CandidateSnapshot>>>,
    candidate_hosts: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    #[cfg(test)]
    test_addresses: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
    #[cfg(test)]
    test_lookup_delays: Arc<Mutex<HashMap<String, Duration>>>,
    #[cfg(test)]
    test_lookup_errors: Arc<Mutex<HashMap<String, std::io::ErrorKind>>>,
    #[cfg(test)]
    test_lookup_counts: Arc<Mutex<HashMap<String, usize>>>,
}

impl Default for DownloadDnsResolver {
    fn default() -> Self {
        Self {
            reliability: Arc::default(),
            successful_addresses: Arc::default(),
            last_resolved: Arc::default(),
            last_failed: Arc::default(),
            resolving_hosts: Arc::default(),
            host_overrides: Arc::default(),
            doh_enabled: false,
            doh_endpoints: Arc::new(vec![Arc::from(BUILTIN_DOH_SERVERS[0])]),
            doh_clients: Arc::new(vec![
                reqwest::Client::builder()
                    .no_proxy()
                    .timeout(DOH_REQUEST_TIMEOUT)
                    .connect_timeout(DOH_CONNECT_TIMEOUT)
                    .build()
                    .expect(
                        "DNS bootstrap client configuration should be valid",
                    ),
            ]),
            doh_provider_weights: Arc::default(),
            dns_answers: Arc::default(),
            candidate_snapshots: Arc::default(),
            candidate_hosts: Arc::default(),
            #[cfg(test)]
            test_addresses: Arc::default(),
            #[cfg(test)]
            test_lookup_delays: Arc::default(),
            #[cfg(test)]
            test_lookup_errors: Arc::default(),
            #[cfg(test)]
            test_lookup_counts: Arc::default(),
        }
    }
}

impl DownloadDnsResolver {
    pub fn with_doh(enabled: bool) -> crate::Result<Self> {
        Self::with_doh_and_proxy(enabled, None)
    }

    pub fn with_doh_and_proxy(
        enabled: bool,
        proxy: Option<&crate::util::proxy::ProxyConfig>,
    ) -> crate::Result<Self> {
        let endpoints = if enabled {
            BUILTIN_DOH_SERVERS
                .iter()
                .map(|endpoint| Arc::from(*endpoint))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut doh_clients = Vec::with_capacity(endpoints.len());
        for _ in &endpoints {
            let builder = match proxy {
                Some(proxy) => proxy.apply(reqwest::Client::builder())?,
                None => reqwest::Client::builder().no_proxy(),
            };
            doh_clients.push(
                builder
                    .timeout(DOH_REQUEST_TIMEOUT)
                    .connect_timeout(DOH_CONNECT_TIMEOUT)
                    .read_timeout(DOH_REQUEST_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(crate::Error::from)?,
            );
        }
        Ok(Self {
            doh_enabled: enabled,
            doh_endpoints: Arc::new(endpoints),
            doh_clients: Arc::new(doh_clients),
            doh_provider_weights: Arc::default(),
            ..Self::default()
        })
    }

    pub fn doh_enabled(&self) -> bool {
        self.doh_enabled
    }

    /// Resolves `host` through `resolver_host` while preserving the original
    /// URL host for HTTP Host headers and TLS SNI.
    #[allow(dead_code)]
    pub fn set_host_override(
        &self,
        host: &str,
        resolver_host: &str,
    ) -> Result<(), &'static str> {
        let host = normalize_host(host)?;
        let resolver_host = normalize_host(resolver_host)?;
        let mut overrides = self.host_overrides.lock();
        if host == resolver_host {
            overrides.remove(&host);
        } else {
            overrides.insert(host.clone(), resolver_host);
        }
        drop(overrides);
        self.last_resolved.lock().remove(&host);
        self.last_failed.lock().remove(&host);
        self.candidate_snapshots.lock().remove(&host);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_host_override(&self, host: &str) -> Result<(), &'static str> {
        let host = normalize_host(host)?;
        self.host_overrides.lock().remove(&host);
        self.last_resolved.lock().remove(&host);
        self.last_failed.lock().remove(&host);
        self.candidate_snapshots.lock().remove(&host);
        Ok(())
    }

    pub fn host_override(&self, host: &str) -> Option<String> {
        let host = normalize_host(host).ok()?;
        self.host_overrides.lock().get(&host).cloned()
    }

    fn resolution_host(&self, host: &str) -> String {
        self.host_override(host).unwrap_or_else(|| host.to_string())
    }
    pub fn record_result(&self, address: IpAddr, result: f64) {
        let mut reliability = self.reliability.lock();
        reliability
            .entry(address)
            .and_modify(|value| *value = *value * 0.5 + result * 0.5)
            .or_insert(result * 0.5);
    }

    pub fn record_host_success(&self, host: &str, address: IpAddr) {
        let mut cached = self.last_resolved.lock();
        if let Some(entry) = cached
            .get_mut(host)
            .filter(|entry| entry.addresses.contains(&address))
        {
            entry.consecutive_connection_failures = 0;
            drop(cached);
            self.record_result(address, 0.5);
            let mut successful = self.successful_addresses.lock();
            let addresses = successful.entry(host.to_string()).or_default();
            addresses.retain(|_, timestamp| {
                timestamp.elapsed() < SUCCESSFUL_ADDRESS_CACHE_TTL
            });
            addresses.insert(address, Instant::now());
        }
    }

    pub fn resolved_addresses(&self, host: &str) -> Vec<IpAddr> {
        self.last_resolved
            .lock()
            .get(host)
            .filter(|entry| entry.is_fresh())
            .map(|entry| &entry.addresses)
            .cloned()
            .unwrap_or_default()
    }

    /// Marks a failed connection attempt for `host`. The second consecutive
    /// failure expires its cache entry, so the next request or prewarm does a
    /// fresh lookup. A single transient failure keeps the hot cache intact.
    /// Returns whether this call expired the entry.
    pub fn record_connection_failure(&self, host: &str) -> bool {
        let mut cached = self.last_resolved.lock();
        let Some(entry) = cached.get_mut(host) else {
            return false;
        };
        entry.consecutive_connection_failures =
            entry.consecutive_connection_failures.saturating_add(1);
        if entry.consecutive_connection_failures
            < CONNECTION_FAILURES_BEFORE_REFRESH
        {
            return false;
        }
        entry.resolved_at = Instant::now() - CACHE_TTL;
        true
    }

    fn doh_cache_ttl(&self, host: &str, addresses: &[SocketAddr]) -> Duration {
        let now = Instant::now();
        self.dns_answers
            .lock()
            .get(host)
            .into_iter()
            .flatten()
            .flat_map(|answer| &answer.addresses)
            .filter(|address| {
                addresses.iter().any(|socket| socket.ip() == address.ip)
            })
            .map(|address| address.expires_at.saturating_duration_since(now))
            .min()
            .unwrap_or(CACHE_TTL)
            .min(CACHE_TTL)
    }

    fn cache_addresses(
        &self,
        host: String,
        addresses: Vec<SocketAddr>,
        ttl: Duration,
    ) {
        self.last_resolved.lock().insert(
            host,
            CachedAddresses {
                addresses: addresses
                    .iter()
                    .map(|address| address.ip())
                    .collect(),
                resolved_at: Instant::now(),
                ttl,
                consecutive_connection_failures: 0,
            },
        );
    }

    fn resolving_lock(&self, host: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.resolving_hosts
            .lock()
            .entry(host.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    async fn lookup_addresses(
        &self,
        resolution_host: &str,
    ) -> std::io::Result<(Vec<SocketAddr>, Duration)> {
        #[cfg(test)]
        {
            *self
                .test_lookup_counts
                .lock()
                .entry(resolution_host.to_string())
                .or_default() += 1;
            let delay =
                self.test_lookup_delays.lock().get(resolution_host).copied();
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            if let Some(kind) =
                self.test_lookup_errors.lock().get(resolution_host).copied()
            {
                return Err(std::io::Error::new(
                    kind,
                    DOWNLOAD_DNS_TEST_DNS_LOOKUP_FAILED,
                ));
            }
            if let Some(addresses) =
                self.test_addresses.lock().get(resolution_host).cloned()
            {
                let ttl = if self.doh_enabled {
                    self.doh_cache_ttl(resolution_host, &addresses)
                } else {
                    CACHE_TTL
                };
                return Ok((addresses, ttl));
            }
        }
        if self.doh_enabled {
            match self.lookup_doh(resolution_host).await {
                Ok(addresses) => {
                    let ttl = self.doh_cache_ttl(resolution_host, &addresses);
                    return Ok((addresses, ttl));
                }
                Err(error) => tracing::warn!(
                    host = resolution_host,
                    error = ?error,
                    "DoH lookup failed; falling back to system DNS"
                ),
            }
        }
        tokio::net::lookup_host((resolution_host, 0))
            .await
            .map(|addresses| (addresses.collect(), CACHE_TTL))
    }

    async fn lookup_doh(&self, host: &str) -> std::io::Result<Vec<SocketAddr>> {
        resolve_families(
            self.lookup_doh_family(host, RecordType::A),
            self.lookup_doh_family(host, RecordType::AAAA),
        )
        .await
    }

    fn doh_provider_candidates(&self) -> Vec<(Arc<str>, reqwest::Client)> {
        let mut indices = (0..self.doh_endpoints.len()).collect::<Vec<_>>();
        let weights = self.doh_provider_weights.lock();
        indices.sort_by(|left, right| {
            weights
                .get(self.doh_endpoints[*right].as_ref())
                .copied()
                .unwrap_or_default()
                .cmp(
                    &weights
                        .get(self.doh_endpoints[*left].as_ref())
                        .copied()
                        .unwrap_or_default(),
                )
                .then_with(|| left.cmp(right))
        });
        indices
            .into_iter()
            .take(BUILTIN_DOH_SERVERS.len())
            .map(|index| {
                (
                    Arc::clone(&self.doh_endpoints[index]),
                    self.doh_clients[index].clone(),
                )
            })
            .collect()
    }

    fn record_doh_provider_success(&self, endpoint: &str) {
        let mut weights = self.doh_provider_weights.lock();
        let weight = weights.entry(endpoint.to_string()).or_default();
        *weight = weight.saturating_add(1);
    }

    async fn lookup_doh_family(
        &self,
        host: &str,
        record_type: RecordType,
    ) -> std::io::Result<Vec<SocketAddr>> {
        let providers = self.doh_provider_candidates();
        let mut requests = FuturesUnordered::new();
        for (endpoint, client) in providers {
            let query = doh_query(host, record_type)?;
            requests.push(async move {
                let result =
                    Self::lookup_doh_provider(&client, &endpoint, &query).await;
                (endpoint, result)
            });
        }
        let mut last_error = None;
        while let Some((endpoint, result)) = requests.next().await {
            match result {
                Ok(answer) => {
                    self.record_doh_provider_success(&endpoint);
                    let addresses = answer.socket_addresses();
                    self.remember_answer(host, answer);
                    return Ok(addresses);
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            std::io::Error::other(DOWNLOAD_DNS_NO_DOH_PROVIDERS_AVAILABLE)
        }))
    }

    async fn lookup_doh_provider(
        client: &reqwest::Client,
        endpoint: &str,
        query: &Message,
    ) -> std::io::Result<DnsAnswer> {
        let body = query.to_vec().map_err(std::io::Error::other)?;
        let mut response = client
            .post(endpoint)
            .header(
                reqwest::header::CONTENT_TYPE,
                DOWNLOAD_DNS_APPLICATION_DNS_MESSAGE,
            )
            .header(
                reqwest::header::ACCEPT,
                DOWNLOAD_DNS_APPLICATION_DNS_MESSAGE,
            )
            .body(body)
            .send()
            .await
            .map_err(std::io::Error::other)?
            .error_for_status()
            .map_err(std::io::Error::other)?;
        let mut bytes = Vec::new();
        while let Some(chunk) =
            response.chunk().await.map_err(std::io::Error::other)?
        {
            if bytes.len() + chunk.len() > MAX_DNS_MESSAGE_BYTES {
                return Err(std::io::Error::other(
                    DOWNLOAD_DNS_DOH_RESPONSE_IS_TOO_LARGE,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        parse_doh_metadata(&bytes, query, DnsSource::Doh(Arc::from(endpoint)))
    }

    fn remember_answer(&self, host: &str, answer: DnsAnswer) {
        let now = Instant::now();
        let mut cache = self.dns_answers.lock();
        cache.retain(|_, answers| {
            answers.iter().any(|answer| {
                now.saturating_duration_since(answer.fetched_at) < CACHE_TTL
                    && answer
                        .addresses
                        .iter()
                        .any(|address| address.expires_at > now)
            })
        });
        make_metadata_room(&mut cache, host, |answers| {
            answers
                .iter()
                .map(|answer| answer.fetched_at)
                .max()
                .unwrap_or(now)
        });
        let answers = cache.entry(host.to_string()).or_default();
        answers.retain(|old| {
            old.addresses.iter().any(|address| address.expires_at > now)
                && !(old.source == answer.source
                    && old.addresses.first().map(|a| a.ip.is_ipv4())
                        == answer.addresses.first().map(|a| a.ip.is_ipv4()))
        });
        // Retain the just-fetched TTL-zero answer for ordinary cache expiry;
        // candidate readers filter expired addresses before using metadata.
        answers.push(answer);
        if answers.len() > BUILTIN_DOH_SERVERS.len() * 2 {
            answers.remove(0);
        }
    }

    fn cached_candidates(&self, host: &str) -> Option<Vec<DnsAnswer>> {
        let now = Instant::now();
        let cache = self.candidate_snapshots.lock();
        let snapshot = cache.get(host)?;
        if now.saturating_duration_since(snapshot.fetched_at)
            >= CANDIDATE_CACHE_TTL
        {
            return None;
        }
        let mut answers = snapshot.answers.clone();
        for answer in &mut answers {
            answer.addresses.retain(|address| address.expires_at > now);
        }
        answers.retain(|answer| !answer.addresses.is_empty());
        (!answers.is_empty()).then_some(answers)
    }

    fn remember_candidates(&self, host: String, snapshot: CandidateSnapshot) {
        let now = Instant::now();
        let mut cache = self.candidate_snapshots.lock();
        cache.retain(|_, value| {
            now.saturating_duration_since(value.fetched_at)
                < CANDIDATE_CACHE_TTL
        });
        make_metadata_room(&mut cache, &host, |value| value.fetched_at);
        cache.insert(host, snapshot);
    }

    /// Collects independent provider answers within the caller's overall
    /// deadline. Ordinary resolution still returns the first usable provider
    /// in each family and never waits on this candidate-query lock.
    pub(crate) async fn candidate_snapshot(
        &self,
        host: &str,
        cancellation: &CancellationToken,
        deadline: Instant,
    ) -> std::io::Result<Vec<DnsAnswer>> {
        let host = normalize_host(host).map_err(std::io::Error::other)?;
        let operation = async {
            if let Some(answers) = self.cached_candidates(&host) {
                return Ok(answers);
            }
            let host_lock = {
                let mut hosts = self.candidate_hosts.lock();
                if hosts.len() >= MAX_METADATA_HOSTS {
                    hosts.retain(|_, lock| Arc::strong_count(lock) > 1);
                }
                if !hosts.contains_key(&host)
                    && hosts.len() >= MAX_METADATA_HOSTS
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        DOWNLOAD_DNS_CANDIDATE_DNS_HOST_QUERY_CAPACITY_EXHAUSTED,
                    ));
                }
                hosts
                    .entry(host.clone())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                    .clone()
            };
            let _guard =
                tokio::time::timeout_at(deadline.into(), host_lock.lock())
                    .await
                    .map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            DOWNLOAD_DNS_CANDIDATE_DNS_LOCK_WAIT_TIMED_OUT,
                        )
                    })?;
            if let Some(answers) = self.cached_candidates(&host) {
                return Ok(answers);
            }
            let resolution_host = self.resolution_host(&host);
            if !self.doh_enabled {
                let addresses = tokio::time::timeout_at(
                    deadline.into(),
                    self.resolve_host(&host),
                )
                .await
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        DOWNLOAD_DNS_SYSTEM_CANDIDATE_DNS_LOOKUP_TIMED_OUT,
                    )
                })??;
                let now = Instant::now();
                // System lookup has no authoritative TTL. Reuse the age of
                // the existing cache rather than refreshing it on every call.
                let expires_at = self
                    .last_resolved
                    .lock()
                    .get(&host)
                    .map(|entry| entry.resolved_at + CACHE_TTL)
                    .unwrap_or(now);
                let answer = DnsAnswer {
                    queried_host: resolution_host,
                    cname_chain: Vec::new(),
                    addresses: addresses
                        .into_iter()
                        .map(|ip| DnsAddress { ip, expires_at })
                        .collect(),
                    fetched_at: now,
                    source: DnsSource::System,
                };
                self.remember_candidates(
                    host.clone(),
                    CandidateSnapshot {
                        answers: vec![answer.clone()],
                        fetched_at: now,
                    },
                );
                return Ok(vec![answer]);
            }

            let mut answers = self
                .dns_answers
                .lock()
                .get(&resolution_host)
                .cloned()
                .unwrap_or_default();
            let now = Instant::now();
            for answer in &mut answers {
                answer.addresses.retain(|address| address.expires_at > now);
            }
            answers.retain(|answer| !answer.addresses.is_empty());
            let mut requests = FuturesUnordered::new();
            for (endpoint, client) in self
                .doh_provider_candidates()
                .into_iter()
                .take(BUILTIN_DOH_SERVERS.len())
            {
                for record_type in [RecordType::A, RecordType::AAAA] {
                    let query = doh_query(&resolution_host, record_type)?;
                    let endpoint = endpoint.clone();
                    let client = client.clone();
                    requests.push(async move {
                        Self::lookup_doh_provider(&client, &endpoint, &query)
                            .await
                    });
                }
            }
            let mut last_error = None;
            loop {
                let result = tokio::select! {
                    _ = tokio::time::sleep_until(deadline.into()) => break,
                    result = requests.next() => result,
                };
                match result {
                    Some(Ok(answer)) => {
                        self.remember_answer(&resolution_host, answer.clone());
                        answers.retain(|old| {
                            !(old.source == answer.source
                                && old
                                    .addresses
                                    .first()
                                    .map(|a| a.ip.is_ipv4())
                                    == answer
                                        .addresses
                                        .first()
                                        .map(|a| a.ip.is_ipv4()))
                        });
                        answers.push(answer);
                    }
                    Some(Err(error)) => last_error = Some(error),
                    None => break,
                }
            }
            let now = Instant::now();
            for answer in &mut answers {
                answer.addresses.retain(|address| address.expires_at > now);
            }
            answers.retain(|answer| !answer.addresses.is_empty());
            if answers.is_empty() {
                return Err(last_error.unwrap_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        DOWNLOAD_DNS_NO_CANDIDATE_DNS_ANSWERS_BEFORE_DEADLINE,
                    )
                }));
            }
            self.remember_candidates(
                host.clone(),
                CandidateSnapshot {
                    answers: answers.clone(),
                    fetched_at: Instant::now(),
                },
            );
            Ok(answers)
        };
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted, "Candidate DNS lookup cancelled",
            )),
            result = operation => result,
        }
    }

    fn cached_failure(&self, host: &str) -> Option<std::io::Error> {
        self.last_failed
            .lock()
            .get(host)
            .filter(|failure| failure.failed_at.elapsed() < FAILURE_CACHE_TTL)
            .map(|failure| {
                std::io::Error::new(failure.kind, failure.message.clone())
            })
    }

    async fn refresh(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        tokio::time::timeout(DNS_LOOKUP_TIMEOUT, self.refresh_inner(host))
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("DNS lookup for {host} timed out"),
                )
            })?
    }

    async fn refresh_inner(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        let host = normalize_host(host).map_err(std::io::Error::other)?;
        let cached = self.resolved_addresses(&host);
        if !cached.is_empty() {
            return Ok(cached);
        }
        if let Some(error) = self.cached_failure(&host) {
            return Err(error);
        }
        let host_lock = self.resolving_lock(&host);
        let _guard = host_lock.lock().await;
        let cached = self.resolved_addresses(&host);
        if !cached.is_empty() {
            return Ok(cached);
        }
        if let Some(error) = self.cached_failure(&host) {
            return Err(error);
        }
        let resolution_host = self.resolution_host(&host);
        let lookup = tokio::time::timeout(
            DOH_REQUEST_TIMEOUT + DNS_TIMEOUT_GRACE,
            self.lookup_addresses(&resolution_host),
        )
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("DNS lookup for {host} timed out"),
            ))
        })
        .and_then(|(addresses, ttl)| {
            if addresses.is_empty() {
                Err(std::io::Error::other(
                    DOWNLOAD_DNS_DNS_RESPONSE_CONTAINED_NO_ADDRESSES,
                ))
            } else {
                Ok((addresses, ttl))
            }
        });
        let (mut addresses, ttl) = match lookup {
            Ok(addresses) => addresses,
            Err(error) => {
                self.last_failed.lock().insert(
                    host,
                    CachedFailure {
                        kind: error.kind(),
                        message: error.to_string(),
                        failed_at: Instant::now(),
                    },
                );
                return Err(error);
            }
        };
        addresses = self.order_addresses(&host, addresses);
        let resolved = addresses.iter().map(|address| address.ip()).collect();
        self.cache_addresses(host, addresses, ttl);
        Ok(resolved)
    }

    /// Resolves a host ahead of the first request so batch downloads can
    /// share a single ordered address list. Failed lookups are briefly cached
    /// so a batch shares the failure instead of repeating the same request.
    pub async fn pre_resolve(&self, host: &str) {
        let _ = self.refresh(host).await;
    }

    pub(crate) async fn resolve_host(
        &self,
        host: &str,
    ) -> std::io::Result<Vec<IpAddr>> {
        self.refresh(host).await
    }

    #[cfg(test)]
    pub(crate) fn set_test_addresses(
        &self,
        host: &str,
        addresses: Vec<SocketAddr>,
    ) {
        self.test_addresses
            .lock()
            .insert(host.to_string(), addresses);
    }

    #[cfg(test)]
    fn set_test_lookup_delay(&self, host: &str, delay: Duration) {
        self.test_lookup_delays
            .lock()
            .insert(host.to_string(), delay);
    }

    #[cfg(test)]
    fn expire_cache(&self, host: &str) {
        if let Some(entry) = self.last_resolved.lock().get_mut(host) {
            entry.resolved_at = Instant::now() - CACHE_TTL;
        }
    }

    fn score(&self, address: IpAddr) -> f64 {
        self.reliability
            .lock()
            .get(&address)
            .copied()
            .unwrap_or_default()
    }

    fn order_addresses(
        &self,
        host: &str,
        mut addresses: Vec<SocketAddr>,
    ) -> Vec<SocketAddr> {
        let mut seen = std::collections::HashSet::new();
        addresses.retain(|address| seen.insert(address.ip()));

        let successful = self
            .successful_addresses
            .lock()
            .get(host)
            .cloned()
            .unwrap_or_default();
        let mut cached = addresses
            .iter()
            .filter(|address| {
                successful.get(&address.ip()).is_some_and(|timestamp| {
                    timestamp.elapsed() < SUCCESSFUL_ADDRESS_CACHE_TTL
                })
            })
            .copied()
            .collect::<Vec<_>>();
        cached.sort_by_key(|address| {
            std::cmp::Reverse(successful.get(&address.ip()).copied())
        });
        let uncached = addresses
            .into_iter()
            .filter(|address| {
                successful.get(&address.ip()).is_none_or(|timestamp| {
                    timestamp.elapsed() >= SUCCESSFUL_ADDRESS_CACHE_TTL
                })
            })
            .collect::<Vec<_>>();
        let v6 = uncached
            .iter()
            .copied()
            .filter(|address| address.is_ipv6())
            .collect::<Vec<_>>();
        let v4 = uncached
            .iter()
            .copied()
            .filter(|address| address.is_ipv4())
            .collect::<Vec<_>>();
        let mut ordered = cached;
        let mut v6_index = 0;
        let mut v4_index = 0;
        while v6_index < v6.len() || v4_index < v4.len() {
            if v6_index < v6.len() {
                ordered.push(v6[v6_index]);
                v6_index += 1;
            }
            if v4_index < v4.len() {
                ordered.push(v4[v4_index]);
                v4_index += 1;
            }
        }
        ordered
    }
}

fn doh_query(host: &str, record_type: RecordType) -> std::io::Result<Message> {
    let mut name = DnsName::from_ascii(host).map_err(std::io::Error::other)?;
    name.set_fqdn(true);
    let mut message = Message::query();
    message.metadata.id = rand::random();
    message.metadata.recursion_desired = true;
    message.add_query(Query::query(name, record_type));
    Ok(message)
}

#[cfg(test)]
fn parse_doh_response(
    bytes: &[u8],
    query: &Message,
) -> std::io::Result<Vec<SocketAddr>> {
    parse_doh_metadata(bytes, query, DnsSource::Doh(Arc::from("test")))
        .map(|answer| answer.socket_addresses())
}

fn parse_doh_metadata(
    bytes: &[u8],
    query: &Message,
    source: DnsSource,
) -> std::io::Result<DnsAnswer> {
    if bytes.len() > MAX_DNS_MESSAGE_BYTES {
        return Err(std::io::Error::other(
            DOWNLOAD_DNS_DOH_RESPONSE_IS_TOO_LARGE,
        ));
    }
    let response = Message::from_vec(bytes).map_err(std::io::Error::other)?;
    if response.metadata.id != query.metadata.id
        || response.metadata.message_type != MessageType::Response
        || response.queries != query.queries
        || response.metadata.truncation
        || response.metadata.response_code != ResponseCode::NoError
    {
        return Err(std::io::Error::other(
            DOWNLOAD_DNS_INVALID_OR_UNSUCCESSFUL_DOH_DNS_RESPONSE,
        ));
    }
    let question = query.queries.first().ok_or_else(|| {
        std::io::Error::other(DOWNLOAD_DNS_MISSING_DNS_QUESTION)
    })?;
    let now = Instant::now();
    let mut name = question.name().clone();
    let mut visited = std::collections::HashSet::new();
    let mut chain = Vec::new();
    let mut chain_ttl = u32::MAX;
    loop {
        if !visited.insert(name.clone()) {
            return Err(std::io::Error::other(
                DOWNLOAD_DNS_CYCLIC_DOH_CNAME_CHAIN,
            ));
        }
        let mut next = None;
        for answer in response.answers.iter().filter(|answer| {
            answer.name == name && answer.dns_class == question.query_class()
        }) {
            if let RData::CNAME(alias) = &answer.data {
                if next.as_ref().is_some_and(|target| target != &alias.0) {
                    return Err(std::io::Error::other(
                        DOWNLOAD_DNS_CONFLICTING_DOH_CNAME_ANSWERS,
                    ));
                }
                next = Some(alias.0.clone());
                chain_ttl = chain_ttl.min(answer.ttl);
            }
        }
        let Some(next) = next else {
            break;
        };
        if response.answers.iter().any(|record| {
            record.name == name
                && record.dns_class == question.query_class()
                && matches!(record.data, RData::A(_) | RData::AAAA(_))
        }) {
            return Err(std::io::Error::other(
                DOWNLOAD_DNS_DOH_CNAME_OWNER_ALSO_CONTAINS_ADDRESS_RECORDS,
            ));
        }
        chain.push(next.to_utf8().trim_end_matches('.').to_ascii_lowercase());
        name = next;
    }
    let mut addresses = response
        .answers
        .iter()
        .filter(|record| {
            record.name == name
                && record.record_type() == question.query_type()
                && record.dns_class == question.query_class()
        })
        .filter_map(|record| {
            let ip = match &record.data {
                RData::A(address) => IpAddr::V4(address.0),
                RData::AAAA(address) => IpAddr::V6(address.0),
                _ => return None,
            };
            Some(DnsAddress {
                ip,
                expires_at: now
                    + Duration::from_secs(record.ttl.min(chain_ttl) as u64),
            })
        })
        .collect::<Vec<_>>();
    addresses.sort_by_key(|address| (address.ip, address.expires_at));
    addresses.dedup_by_key(|address| address.ip);
    if addresses.is_empty() {
        return Err(std::io::Error::other(
            DOWNLOAD_DNS_DOH_RESPONSE_CONTAINED_NO_ADDRESSES,
        ));
    }
    Ok(DnsAnswer {
        queried_host: question
            .name()
            .to_utf8()
            .trim_end_matches('.')
            .to_ascii_lowercase(),
        cname_chain: chain,
        addresses,
        fetched_at: now,
        source,
    })
}

async fn resolve_families(
    ipv4: impl std::future::Future<Output = std::io::Result<Vec<SocketAddr>>>,
    ipv6: impl std::future::Future<Output = std::io::Result<Vec<SocketAddr>>>,
) -> std::io::Result<Vec<SocketAddr>> {
    let mut families = FuturesUnordered::new();
    families.push(futures::future::Either::Left(ipv4));
    families.push(futures::future::Either::Right(ipv6));
    let mut deadline = tokio::time::Instant::now() + DOH_FALLBACK_TIMEOUT;
    let mut addresses = Vec::new();
    let mut last_error = None;
    loop {
        match tokio::time::timeout_at(deadline, families.next()).await {
            Ok(Some(Ok(found))) if !found.is_empty() => {
                if addresses.is_empty() {
                    deadline = deadline.min(
                        tokio::time::Instant::now() + FAMILY_COLLECTION_GRACE,
                    );
                }
                addresses.extend(found);
            }
            Ok(Some(Err(error))) => last_error = Some(error),
            Ok(Some(Ok(_))) => {}
            Ok(None) | Err(_) => break,
        }
    }
    if !addresses.is_empty() {
        Ok(addresses)
    } else {
        Err(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                DOWNLOAD_DNS_NO_USABLE_DOH_ADDRESS_FAMILY_BEFORE_DEADLINE,
            )
        }))
    }
}

fn normalize_host(host: &str) -> Result<String, &'static str> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.contains(['/', ':', '@', '[', ']'])
        || host.split('.').any(str::is_empty)
    {
        return Err(
            DOWNLOAD_DNS_DNS_HOST_OVERRIDE_MUST_BE_A_HOSTNAME_WITHOUT_A,
        );
    }
    Ok(host)
}

impl Resolve for DownloadDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let resolver = self.clone();
        Box::pin(async move {
            let addresses = resolver
                .refresh(&host)
                .await?
                .into_iter()
                .map(|address| SocketAddr::new(address, 0))
                .collect::<Vec<_>>();
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
#[path = "download_dns/tests.rs"]
mod metadata_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn doh_configuration_uses_the_builtin_provider_pool() {
        let resolver = DownloadDnsResolver::with_doh(true).unwrap();
        assert!(resolver.doh_enabled());
        assert_eq!(resolver.doh_endpoints.len(), BUILTIN_DOH_SERVERS.len());
        assert_eq!(resolver.doh_endpoints[0].as_ref(), BUILTIN_DOH_SERVERS[0]);
    }

    #[test]
    fn disabled_doh_accepts_an_invalid_draft_endpoint() {
        let resolver = DownloadDnsResolver::with_doh(false).unwrap();
        assert!(!resolver.doh_enabled());
    }

    #[tokio::test]
    async fn unavailable_doh_falls_back_to_system_dns_and_caches_addresses() {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut resolver = DownloadDnsResolver::with_doh(true).unwrap();
        resolver.doh_endpoints =
            Arc::new(vec![Arc::from(format!("http://{address}/dns-query"))]);
        resolver.doh_clients = Arc::new(vec![
            reqwest::Client::builder().no_proxy().build().unwrap(),
        ]);
        let addresses = tokio::time::timeout(
            DNS_LOOKUP_TIMEOUT,
            resolver.resolve_host("localhost"),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!addresses.is_empty());
        assert!(addresses.iter().all(|address| address.is_loopback()));
        assert_eq!(resolver.resolved_addresses("localhost"), addresses);
    }

    #[tokio::test]
    async fn http_requests_recover_when_doh_is_unavailable() {
        let dead_listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = dead_listener.local_addr().unwrap();
        drop(dead_listener);
        let mut resolver = DownloadDnsResolver::with_doh(true).unwrap();
        resolver.doh_endpoints =
            Arc::new(vec![Arc::from(format!("http://{address}/dns-query"))]);
        resolver.doh_clients = Arc::new(vec![
            reqwest::Client::builder().no_proxy().build().unwrap(),
        ]);
        let (port, server) = spawn_ipv4_server().await;
        let response = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(DNS_LOOKUP_TIMEOUT)
            .dns_resolver(Arc::new(resolver))
            .build()
            .unwrap()
            .get(format!("http://localhost:{port}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(response, "ok");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn system_dns_fallback_cache_ignores_previous_doh_ttl() {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint: Arc<str> = Arc::from(format!(
            "http://{}/dns-query",
            listener.local_addr().unwrap()
        ));
        drop(listener);
        let now = Instant::now();
        for expires_at in [
            now - Duration::from_secs(1),
            now,
            now + Duration::from_secs(60),
        ] {
            let mut resolver = DownloadDnsResolver::with_doh(true).unwrap();
            resolver.doh_endpoints = Arc::new(vec![endpoint.clone()]);
            resolver.doh_clients = Arc::new(vec![
                reqwest::Client::builder().no_proxy().build().unwrap(),
            ]);
            resolver.remember_answer(
                "localhost",
                DnsAnswer {
                    queried_host: "localhost".into(),
                    cname_chain: Vec::new(),
                    addresses: vec![
                        DnsAddress {
                            ip: Ipv4Addr::LOCALHOST.into(),
                            expires_at,
                        },
                        DnsAddress {
                            ip: Ipv6Addr::LOCALHOST.into(),
                            expires_at,
                        },
                    ],
                    fetched_at: now,
                    source: DnsSource::Doh(endpoint.clone()),
                },
            );
            let results = futures::future::join_all(
                (0..8).map(|_| resolver.resolve_host("localhost")),
            )
            .await;
            let addresses = results[0].as_ref().unwrap();
            assert!(!addresses.is_empty());
            assert!(
                results
                    .iter()
                    .all(|result| result.as_ref().unwrap() == addresses)
            );
            assert_eq!(resolver.resolved_addresses("localhost"), *addresses);
            assert_eq!(
                resolver.last_resolved.lock()["localhost"].ttl,
                CACHE_TTL
            );
            assert_eq!(resolver.test_lookup_counts.lock()["localhost"], 1);
            assert_eq!(
                resolver.dns_answers.lock()["localhost"][0].addresses[0]
                    .expires_at,
                expires_at
            );
        }
    }

    #[tokio::test]
    async fn stalled_doh_leaves_time_for_system_dns_fallback() {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut resolver = DownloadDnsResolver::with_doh(true).unwrap();
        resolver.doh_endpoints = Arc::new(vec![Arc::from(format!(
            "http://{}/dns-query",
            listener.local_addr().unwrap()
        ))]);
        resolver.doh_clients = Arc::new(vec![
            reqwest::Client::builder().no_proxy().build().unwrap(),
        ]);
        let addresses = tokio::time::timeout(
            DNS_LOOKUP_TIMEOUT,
            resolver.resolve_host("localhost"),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!addresses.is_empty());
        assert!(addresses.iter().all(|address| address.is_loopback()));
    }

    #[test]
    fn wire_queries_and_answers_round_trip_and_validate_the_question() {
        use hickory_resolver::proto::rr::{
            Record,
            rdata::{A, AAAA, CNAME},
        };
        for record_type in [RecordType::A, RecordType::AAAA] {
            let query = doh_query("cdn.example", record_type).unwrap();
            let decoded = Message::from_vec(&query.to_vec().unwrap()).unwrap();
            assert_eq!(decoded.queries, query.queries);
            assert!(decoded.metadata.recursion_desired);
            let alias = DnsName::from_ascii("edge.example").unwrap();
            let ip = if record_type == RecordType::A {
                IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))
            } else {
                IpAddr::V6(Ipv6Addr::LOCALHOST)
            };
            let data = match ip {
                IpAddr::V4(ip) => RData::A(A(ip)),
                IpAddr::V6(ip) => RData::AAAA(AAAA(ip)),
            };
            let mut response = query.clone();
            response.metadata.message_type = MessageType::Response;
            response
                .add_answer(Record::from_rdata(
                    query.queries.first().unwrap().name().clone(),
                    60,
                    RData::CNAME(CNAME(alias.clone())),
                ))
                .add_answer(Record::from_rdata(alias, 60, data));
            assert_eq!(
                parse_doh_response(&response.to_vec().unwrap(), &query)
                    .unwrap(),
                vec![SocketAddr::new(ip, 0)]
            );
            response.metadata.id = query.metadata.id.wrapping_add(1);
            assert!(
                parse_doh_response(&response.to_vec().unwrap(), &query)
                    .is_err()
            );
        }
        assert!(
            parse_doh_response(
                b"{\"Answer\":[]}",
                &doh_query("cdn.example", RecordType::A).unwrap()
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn usable_family_survives_a_failed_or_stalled_other_family() {
        let address = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 0));
        let failed = async { Err(std::io::Error::other("AAAA failed")) };
        assert_eq!(
            resolve_families(async { Ok(vec![address]) }, failed)
                .await
                .unwrap(),
            vec![address]
        );
        let addresses = tokio::time::timeout(
            FAMILY_COLLECTION_GRACE + Duration::from_secs(1),
            resolve_families(
                async { Ok(vec![address]) },
                std::future::pending(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(addresses, vec![address]);
    }

    #[tokio::test]
    async fn empty_fast_family_does_not_cancel_slower_usable_family() {
        let address = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 0));
        let addresses = resolve_families(async { Ok(Vec::new()) }, async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            Ok(vec![address])
        })
        .await
        .unwrap();
        assert_eq!(addresses, vec![address]);
    }

    #[tokio::test]
    async fn usable_ipv6_does_not_wait_for_stalled_ipv4() {
        let address = SocketAddr::from((Ipv6Addr::LOCALHOST, 0));
        let addresses = tokio::time::timeout(
            FAMILY_COLLECTION_GRACE + Duration::from_secs(1),
            resolve_families(std::future::pending(), async {
                Ok(vec![address])
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(addresses, vec![address]);
    }

    #[tokio::test]
    async fn late_ipv4_addresses_are_kept_when_ipv6_arrives_first() {
        let ipv4 = SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 0));
        let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, 0));
        let addresses = resolve_families(
            async {
                tokio::time::sleep(Duration::from_millis(400)).await;
                Ok(vec![ipv4])
            },
            async { Ok(vec![ipv6]) },
        )
        .await
        .unwrap();
        assert!(addresses.contains(&ipv4));
        assert!(addresses.contains(&ipv6));
    }

    #[tokio::test]
    async fn failed_lookup_is_shared_across_a_batch_and_expires() {
        let resolver = DownloadDnsResolver::default();
        let host = "failed-batch.test";
        resolver
            .test_lookup_errors
            .lock()
            .insert(host.into(), std::io::ErrorKind::TimedOut);
        resolver.set_test_lookup_delay(host, Duration::from_millis(20));
        let results =
            futures::future::join_all((0..45).map(|_| resolver.refresh(host)))
                .await;
        assert!(results.iter().all(Result::is_err));
        assert_eq!(resolver.test_lookup_counts.lock().get(host), Some(&1));
        resolver.last_failed.lock().get_mut(host).unwrap().failed_at -=
            FAILURE_CACHE_TTL;
        resolver.test_lookup_errors.lock().remove(host);
        resolver.set_test_addresses(
            host,
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
        );
        assert!(resolver.refresh(host).await.is_ok());
        assert_eq!(resolver.test_lookup_counts.lock().get(host), Some(&2));
    }

    #[tokio::test]
    async fn lookup_and_host_lock_waits_have_deadlines() {
        let resolver = DownloadDnsResolver::default();
        let host = "deadline.test";
        resolver.set_test_addresses(
            host,
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
        );
        resolver.set_test_lookup_delay(host, DNS_LOOKUP_TIMEOUT * 2);
        let error = resolver.refresh(host).await.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        let lock = resolver.resolving_lock("locked.test");
        let _guard = lock.lock().await;
        assert_eq!(
            resolver.refresh("locked.test").await.unwrap_err().kind(),
            std::io::ErrorKind::TimedOut
        );
    }

    async fn spawn_ipv4_server() -> (u16, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                )
                .await
                .unwrap();
        });
        (port, handle)
    }

    async fn request_with_resolver(
        resolver: DownloadDnsResolver,
        host: &str,
        port: u16,
    ) -> String {
        reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .dns_resolver(Arc::new(resolver))
            .build()
            .unwrap()
            .get(format!("http://{host}:{port}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    #[test]
    fn returns_both_protocol_families_in_preferred_order() {
        let resolver = DownloadDnsResolver::default();
        let ipv4 = SocketAddr::from((Ipv4Addr::new(203, 0, 113, 10), 0));
        let ipv6 = SocketAddr::from((Ipv6Addr::LOCALHOST, 0));
        assert_eq!(
            resolver.order_addresses("api.modrinth.com", vec![ipv6, ipv4]),
            vec![ipv6, ipv4]
        );
    }

    #[test]
    fn selects_the_most_reliable_address_within_a_family() {
        let resolver = DownloadDnsResolver::default();
        let slower = SocketAddr::from((Ipv4Addr::new(203, 0, 113, 10), 0));
        let faster = SocketAddr::from((Ipv4Addr::new(203, 0, 113, 11), 0));
        resolver.cache_addresses(
            "cdn.example.com".to_string(),
            vec![slower, faster],
            CACHE_TTL,
        );
        resolver.record_host_success("cdn.example.com", faster.ip());

        assert_eq!(
            resolver.order_addresses("cdn.example.com", vec![slower, faster]),
            vec![faster, slower]
        );
    }

    #[test]
    fn only_records_the_address_that_completed_the_request() {
        let resolver = DownloadDnsResolver::default();
        let failed = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));
        let succeeded = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 11));
        resolver.cache_addresses(
            "cdn.example.com".to_string(),
            vec![SocketAddr::new(failed, 0), SocketAddr::new(succeeded, 0)],
            CACHE_TTL,
        );

        resolver.record_host_success("cdn.example.com", succeeded);

        assert_eq!(resolver.score(failed), 0.0);
        assert!(resolver.score(succeeded) > 0.0);
    }

    #[test]
    fn host_success_does_not_refresh_an_expired_dns_entry() {
        let resolver = DownloadDnsResolver::default();
        let address = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 11));
        resolver.cache_addresses(
            "expired-success.test".to_string(),
            vec![SocketAddr::new(address, 0)],
            CACHE_TTL,
        );
        resolver.expire_cache("expired-success.test");

        resolver.record_host_success("expired-success.test", address);

        assert!(
            resolver
                .resolved_addresses("expired-success.test")
                .is_empty()
        );
    }

    #[test]
    fn late_success_does_not_undo_failure_expiration() {
        let resolver = DownloadDnsResolver::default();
        let address = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 12));
        resolver.cache_addresses(
            "late-success.test".to_string(),
            vec![SocketAddr::new(address, 0)],
            CACHE_TTL,
        );
        assert!(!resolver.record_connection_failure("late-success.test"));
        assert!(resolver.record_connection_failure("late-success.test"));

        resolver.record_host_success("late-success.test", address);

        assert!(resolver.resolved_addresses("late-success.test").is_empty());
    }

    #[tokio::test]
    async fn one_request_falls_back_when_the_first_ip_refuses_connection() {
        let resolver = DownloadDnsResolver::default();
        let refused = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 2), 0));
        let available = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        resolver.set_test_addresses("multi.test", vec![refused, available]);
        resolver.record_result(refused.ip(), 1.0);
        let (port, server) = spawn_ipv4_server().await;

        let body = request_with_resolver(resolver, "multi.test", port).await;

        assert_eq!(body, "ok");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn reqwest_uses_a_third_resolved_address_after_two_refusals() {
        let resolver = DownloadDnsResolver::default();
        resolver.set_test_addresses(
            "third-address.test",
            ["127.0.0.2", "127.0.0.3", "127.0.0.1"]
                .into_iter()
                .map(|ip| SocketAddr::new(ip.parse().unwrap(), 0))
                .collect(),
        );
        let (port, server) = spawn_ipv4_server().await;
        let body = tokio::time::timeout(
            Duration::from_secs(6),
            request_with_resolver(resolver, "third-address.test", port),
        )
        .await
        .unwrap();
        assert_eq!(body, "ok");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn one_request_falls_back_from_ipv6_to_ipv4() {
        let resolver = DownloadDnsResolver::default();
        let unavailable_v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, 0));
        let available_v4 = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        resolver.set_test_addresses(
            "dual-stack.test",
            vec![unavailable_v6, available_v4],
        );
        resolver.record_result(unavailable_v6.ip(), 1.0);
        let (port, server) = spawn_ipv4_server().await;

        let body =
            request_with_resolver(resolver, "dual-stack.test", port).await;

        assert_eq!(body, "ok");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn expired_cache_refreshes_to_the_current_addresses() {
        let resolver = DownloadDnsResolver::default();
        let old = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 2), 0));
        let current = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        resolver.set_test_addresses("ttl-refresh.test", vec![old]);
        resolver.pre_resolve("ttl-refresh.test").await;
        resolver.set_test_addresses("ttl-refresh.test", vec![current]);

        assert_eq!(
            resolver.resolved_addresses("ttl-refresh.test"),
            vec![old.ip()]
        );
        resolver.expire_cache("ttl-refresh.test");
        resolver.pre_resolve("ttl-refresh.test").await;

        assert_eq!(
            resolver.resolved_addresses("ttl-refresh.test"),
            vec![current.ip()]
        );
    }

    #[tokio::test]
    async fn redirected_connection_failure_expires_target_dns_only() {
        use crate::util::fetch::{
            DownloadClients, DownloadRoute, DownloadRouteSource, ProxyPolicy,
            record_dns_connection_failure,
        };

        let clients =
            DownloadClients::build(&Default::default(), false, false).unwrap();
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        for host in ["redirect-origin.test", "redirect-target.test"] {
            clients.system_dns.cache_addresses(
                host.into(),
                vec![address],
                CACHE_TTL,
            );
            clients.direct_dns.cache_addresses(
                host.into(),
                vec![address],
                CACHE_TTL,
            );
        }
        let listener = tokio::net::TcpListener::bind(address).await.unwrap();
        let refused = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .no_proxy()
            .resolve("redirect-target.test", refused)
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
            .get(format!(
                "http://redirect-target.test:{}/file",
                refused.port()
            ))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_connect() || error.is_timeout());
        let route = DownloadRoute {
            url: "https://redirect-origin.test/file".into(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::System,
        };
        crate::util::download::proxy_context::with_snapshot(
            clients.clone(),
            async {
                assert!(
                    record_dns_connection_failure(&route, &error).is_none()
                );
                assert_eq!(
                    record_dns_connection_failure(&route, &error).as_deref(),
                    Some("redirect-target.test")
                );
            },
        )
        .await;
        assert!(
            clients
                .system_dns
                .resolved_addresses("redirect-target.test")
                .is_empty()
        );
        assert_eq!(
            clients
                .system_dns
                .resolved_addresses("redirect-origin.test"),
            vec![address.ip()]
        );
        assert_eq!(
            clients
                .direct_dns
                .resolved_addresses("redirect-target.test"),
            vec![address.ip()]
        );
    }

    #[tokio::test]
    async fn repeated_connection_failures_refresh_the_cached_address() {
        let resolver = DownloadDnsResolver::default();
        let old = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 2), 0));
        let current = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        resolver.set_test_addresses("failure-refresh.test", vec![old]);
        resolver.pre_resolve("failure-refresh.test").await;
        resolver.set_test_addresses("failure-refresh.test", vec![current]);

        assert!(!resolver.record_connection_failure("failure-refresh.test"));
        assert!(resolver.record_connection_failure("failure-refresh.test"));
        assert!(
            resolver
                .resolved_addresses("failure-refresh.test")
                .is_empty()
        );
        resolver.pre_resolve("failure-refresh.test").await;

        assert_eq!(
            resolver.resolved_addresses("failure-refresh.test"),
            vec![current.ip()]
        );
        let (port, server) = spawn_ipv4_server().await;
        assert_eq!(
            request_with_resolver(resolver, "failure-refresh.test", port).await,
            "ok"
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn pre_resolving_one_host_does_not_block_another_host() {
        let resolver = DownloadDnsResolver::default();
        resolver.set_test_addresses(
            "slow-resolution.test",
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
        );
        resolver.set_test_lookup_delay(
            "slow-resolution.test",
            Duration::from_millis(100),
        );
        resolver.set_test_addresses(
            "fast-resolution.test",
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
        );

        let slow_resolver = resolver.clone();
        let slow = tokio::spawn(async move {
            slow_resolver.pre_resolve("slow-resolution.test").await;
        });
        tokio::task::yield_now().await;
        tokio::time::timeout(
            Duration::from_millis(50),
            resolver.pre_resolve("fast-resolution.test"),
        )
        .await
        .expect("an unrelated DNS lookup must not wait for the slow host");

        assert!(
            !resolver
                .resolved_addresses("fast-resolution.test")
                .is_empty()
        );
        slow.await.unwrap();
    }

    #[tokio::test]
    async fn host_override_uses_the_target_hosts_addresses() {
        let resolver = DownloadDnsResolver::default();
        let (port, server) = spawn_ipv4_server().await;
        resolver.set_test_addresses(
            "resolver-target.test",
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
        );
        resolver
            .set_host_override("REQUEST-HOST.TEST.", "resolver-target.test")
            .unwrap();

        let body =
            request_with_resolver(resolver.clone(), "request-host.test", port)
                .await;

        assert_eq!(body, "ok");
        assert_eq!(
            resolver.host_override("request-host.test").as_deref(),
            Some("resolver-target.test"),
        );
        assert!(!resolver.resolved_addresses("request-host.test").is_empty());
        server.await.unwrap();
    }

    #[test]
    fn clearing_override_removes_the_cached_request_host_addresses() {
        let resolver = DownloadDnsResolver::default();
        resolver
            .set_host_override("request-host.test", "resolver-target.test")
            .unwrap();
        resolver.cache_addresses(
            "request-host.test".to_string(),
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 0))],
            CACHE_TTL,
        );

        resolver.clear_host_override("request-host.test").unwrap();

        assert!(resolver.host_override("request-host.test").is_none());
        assert!(resolver.resolved_addresses("request-host.test").is_empty());
        assert!(normalize_host("https://resolver-target.test").is_err());
        assert!(normalize_host("resolver-target.test:443").is_err());
    }

    #[test]
    fn default_resolver_does_not_override_tianpao() {
        assert!(
            DownloadDnsResolver::default()
                .host_override("mod.tianpao.top")
                .is_none()
        );
    }

    #[test]
    fn default_resolver_does_not_override_current_modrinth_cdn() {
        assert!(
            DownloadDnsResolver::default()
                .host_override("cdn.modrinth.com")
                .is_none()
        );
    }
}
