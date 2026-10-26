//! Shared per-authority HTTP/2 connections for file downloads.
//!
//! `reqwest`'s connection pool opens a fresh TCP+TLS connection for every
//! request that arrives while no idle connection is available, so a batch of
//! concurrent downloads to one CDN costs one handshake per file. This module
//! maintains multiplexed HTTP/2 connections per authority and expands the
//! pool for busy file downloads within the shared resource budgets.

use crate::util::download::constants::H2_ADDITIONAL_CONNECTIONS_ENABLED as ADDITIONAL_H2_CONNECTIONS_ENABLED;
use crate::util::download::constants::H2_CONNECTION_ATTEMPTS;
use crate::util::download::constants::{
    DOWNLOAD_SCHEDULING_GRACE, H2_ADMISSION_LOG_THRESHOLD,
    H2_CONNECTION_WINDOW, H2_INITIAL_STREAM_WINDOW, H2_POOL_ADMISSION,
    H2_POOL_ENDPOINT_CHANGED_BEFORE_H2_STREAM_ASSIGNMENT,
    H2_POOL_ENDPOINT_CHANGED_WHILE_WAITING_FOR_THE_H2_CONNECTION_SLOT,
    H2_POOL_ENDPOINT_DID_NOT_NEGOTIATE_H2,
    H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_H2_EXPANSION,
    H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_THE_H2_HANDSHAKE,
    H2_POOL_H2_CONNECTION_DEADLINE_EXHAUSTED, H2_POOL_HEADERS,
    H2_POOL_HTTP_2_POLICY_REQUIRES_AN_EXISTING_SHARED_CONNECTION,
    H2_POOL_HTTP_2_POOL_HAS_NO_LIVE_CONNECTION,
    H2_POOL_HTTP_2_ROUTE_HAS_NO_AUTHORITY,
    H2_POOL_PINNED_TCP_CONNECTION_TIMED_OUT,
    H2_POOL_SHARED_HTTP_2_CONNECTION_IS_NOT_COVERED_BY_THE, HTTPS_PORT,
};
#[cfg(test)]
use crate::util::download::constants::{
    H2_CONNECTION_PROBE_TIMEOUT, H2_DNS_REFRESH_TIMEOUT,
    H2_POOL_TEST_INVALID_443,
};
use bytes::Bytes;
use futures::stream::{FuturesUnordered, StreamExt};
use h2::client::SendRequest;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Notify;
use tokio_rustls::TlsConnector;

use super::endpoint_selection::{EndpointLease, Protocol};
use crate::util::fetch::DownloadRoute;

use super::constants::{
    H2_CONNECT_TIMEOUT as CONNECT_TIMEOUT,
    H2_CONNECTION_WAIT_TIMEOUT as CONNECTION_WAIT_TIMEOUT,
    H2_HAPPY_EYEBALLS_IPV4_DELAY as HAPPY_EYEBALLS_IPV4_DELAY,
    H2_HAPPY_EYEBALLS_IPV6_DELAY as HAPPY_EYEBALLS_IPV6_DELAY,
    H2_IDLE_EVICTION_TIMEOUT as IDLE_EVICTION_TIMEOUT,
    H2_STREAM_READY_TIMEOUT as STREAM_READY_TIMEOUT,
    H2_TLS_HANDSHAKE_TIMEOUT as TLS_HANDSHAKE_TIMEOUT,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum H2ConnectFailureKind {
    Tcp,
    Tls,
    Protocol,
}

#[derive(Debug)]
pub(crate) struct H2ConnectError {
    pub(crate) kind: H2ConnectFailureKind,
    detail: String,
}

impl std::fmt::Display for H2ConnectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for H2ConnectError {}

impl H2ConnectError {
    fn new(kind: H2ConnectFailureKind, detail: String) -> Self {
        Self { kind, detail }
    }
}

/// A live shared HTTP/2 connection to one authority.
pub struct SharedH2Connection {
    authority: String,
    budget_scope: String,
    pub(crate) endpoint_lease: Option<EndpointLease>,
    pub(crate) connection_id: u64,
    pub(crate) remote_addr: Option<std::net::SocketAddr>,
    sender: Mutex<SendRequest<Bytes>>,
    // One permit represents the shared TCP/TLS connection, not every H2
    // stream opened through it.
    physical_budget: Mutex<Option<super::native_budget::NativeBudgetPermit>>,
    /// Set to true by the driver task when the connection terminates.
    dead: Arc<std::sync::atomic::AtomicBool>,
    retiring: std::sync::atomic::AtomicBool,
    drained: Arc<Notify>,
    /// Number of application streams currently assigned to this connection.
    /// This is deliberately separate from HTTP/2's peer stream accounting: it
    /// lets an asset batch distribute work across sibling TCP connections.
    active_streams: Arc<AtomicUsize>,
    bytes_transferred: AtomicUsize,
    reserved_streams: Arc<AtomicUsize>,
    successful_responses: AtomicUsize,
    last_failure: Mutex<Option<std::time::Instant>>,
    last_activity: Arc<Mutex<std::time::Instant>>,
    evict: Arc<Notify>,
}

pub(crate) struct H2StreamActivity {
    _endpoint_feedback:
        Option<super::endpoint_selection::metrics::EndpointTransferGuard>,
    active_streams: Arc<AtomicUsize>,
    last_activity: Arc<Mutex<std::time::Instant>>,
    drained: Arc<Notify>,
}

impl Drop for H2StreamActivity {
    fn drop(&mut self) {
        if self.active_streams.fetch_sub(1, Ordering::AcqRel) == 1 {
            *self
                .last_activity
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                std::time::Instant::now();
            self.drained.notify_one();
        }
    }
}

impl SharedH2Connection {
    fn new(
        authority: String,
        sender: SendRequest<Bytes>,
        physical_budget: Option<super::native_budget::NativeBudgetPermit>,
    ) -> Self {
        Self {
            budget_scope: authority.clone(),
            endpoint_lease: None,
            connection_id: {
                static NEXT_ID: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(1);
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            },
            authority,
            remote_addr: None,
            sender: Mutex::new(sender),
            physical_budget: Mutex::new(physical_budget),
            dead: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            retiring: std::sync::atomic::AtomicBool::new(false),
            drained: Arc::new(Notify::new()),
            active_streams: Arc::new(AtomicUsize::new(0)),
            bytes_transferred: AtomicUsize::new(0),
            reserved_streams: Arc::new(AtomicUsize::new(0)),
            successful_responses: AtomicUsize::new(0),
            last_failure: Mutex::new(None),
            last_activity: Arc::new(Mutex::new(std::time::Instant::now())),
            evict: Arc::new(Notify::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(sender: SendRequest<Bytes>) -> Self {
        Self::new(H2_POOL_TEST_INVALID_443.to_string(), sender, None)
    }

    #[cfg(test)]
    pub(crate) fn mark_success_for_test(&self) {
        self.successful_responses.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(
        route: &DownloadRoute,
        sender: SendRequest<Bytes>,
    ) -> Arc<Self> {
        let authority = crate::util::fetch::url_authority(&route.url).unwrap();
        let budget = super::native_budget::acquire(route).await.unwrap();
        let connection =
            Arc::new(Self::new(authority.clone(), sender, Some(budget)));
        let (key, _) = connection_identity(route).unwrap();
        let slot = connection_slot(&key).await;
        *slot.lock().await = Some(connection.clone());
        connection
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn is_retiring(&self) -> bool {
        self.retiring.load(Ordering::Acquire)
    }

    pub(crate) fn retire(&self) {
        let _sender = self.sender.lock().unwrap_or_else(|p| p.into_inner());
        self.retiring.store(true, Ordering::Release);
        self.drained.notify_one();
    }

    pub(crate) fn active_streams(&self) -> usize {
        self.active_streams.load(Ordering::Acquire)
    }

    pub(crate) fn record_bytes(&self, bytes: usize) {
        self.bytes_transferred.fetch_add(bytes, Ordering::Relaxed);
        if let Some(lease) = &self.endpoint_lease
            && let Some(clients) = super::proxy_context::clients()
        {
            clients.endpoints.observe_payload(
                lease,
                bytes as u64,
                std::time::Instant::now(),
                super::local_resources::network_pressure().blocks_expansion()
                    || super::endpoint_selection::tuning::TRIALS
                        .snapshot(std::time::Instant::now())
                        .1
                        .is_some(),
            );
        }
    }

    pub(crate) fn bytes_transferred_for_scheduler(&self) -> usize {
        self.bytes_transferred.load(Ordering::Relaxed)
    }

    pub(crate) fn assigned_streams_for_scheduler(&self) -> usize {
        self.active_streams() + self.reserved_streams.load(Ordering::Acquire)
    }

    pub(crate) fn reserve_stream_for_scheduler(&self) {
        self.reserved_streams.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn release_stream_reservation(&self) {
        let _ = self.reserved_streams.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |reserved| reserved.checked_sub(1),
        );
    }

    pub(crate) fn track_stream(&self) -> H2StreamActivity {
        *self
            .last_activity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            std::time::Instant::now();
        self.active_streams.fetch_add(1, Ordering::AcqRel);
        self.release_stream_reservation();
        H2StreamActivity {
            _endpoint_feedback: self.endpoint_lease.as_ref().and_then(|lease|
                super::proxy_context::clients().map(|clients|
                    super::endpoint_selection::metrics::EndpointTransferGuard::new(&clients.endpoints, lease.clone()))),
            active_streams: Arc::clone(&self.active_streams),
            last_activity: Arc::clone(&self.last_activity),
            drained: Arc::clone(&self.drained),
        }
    }

    fn is_idle_expired(&self) -> bool {
        self.active_streams() == 0
            && self
                .last_activity
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .elapsed()
                >= IDLE_EVICTION_TIMEOUT
    }

    pub(crate) fn is_idle_expired_for_scheduler(&self) -> bool {
        self.is_idle_expired()
    }

    fn evict(&self) {
        self.dead.store(true, Ordering::Release);
        self.evict.notify_one();
    }

    pub(crate) fn evict_for_scheduler(&self) {
        self.evict();
    }

    pub(crate) fn record_stream_failure(&self) {
        *self
            .last_failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(std::time::Instant::now());
    }

    fn healthy_for_expansion(&self) -> bool {
        self.successful_responses.load(Ordering::Acquire) > 0
            && self
                .last_failure
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_none_or(|failure| {
                    failure.elapsed()
                        >= super::authority_scheduler::STABILITY_WINDOW
                })
    }

    pub(crate) fn healthy_for_scheduler(&self) -> bool {
        self.healthy_for_expansion()
    }

    fn has_physical_budget(&self) -> bool {
        self.physical_budget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }

    fn release_physical_budget(&self) {
        self.physical_budget
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }

    /// Sends a request on the shared connection and awaits the response
    /// headers, yielding the response and its receive stream. Each call
    /// opens an independent multiplexed stream.
    pub async fn open(
        &self,
        request: http::Request<()>,
    ) -> crate::Result<http::Response<h2::RecvStream>> {
        let result = self.open_inner(request).await;
        if result.as_ref().err().is_some_and(|error| {
            matches!(error.raw.as_ref(),
            crate::ErrorKind::DownloadH2Error(error) if !error.is_reset())
        }) {
            self.record_stream_failure();
        }
        result
    }

    async fn open_inner(
        &self,
        request: http::Request<()>,
    ) -> crate::Result<http::Response<h2::RecvStream>> {
        let ready_started = std::time::Instant::now();
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        *self
            .last_activity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            std::time::Instant::now();
        let mut sender =
            tokio::time::timeout(STREAM_READY_TIMEOUT, sender.ready())
                .await
                .map_err(|_| crate::ErrorKind::DownloadStreamStalled {
                    context: H2_POOL_ADMISSION.into(),
                })?
                .map_err(crate::ErrorKind::DownloadH2Error)?;
        let ready_wait = ready_started.elapsed();
        if ready_wait >= H2_ADMISSION_LOG_THRESHOLD {
            tracing::debug!(
                authority = %self.authority,
                ready_wait_ms = ready_wait.as_millis(),
                "HTTP/2 stream admission waited for peer or connection capacity"
            );
        }
        let (response, _) = {
            let _admission =
                self.sender.lock().unwrap_or_else(|p| p.into_inner());
            if self.is_retiring() || self.is_dead() {
                return Err(crate::ErrorKind::DownloadH2Error(
                    h2::Reason::REFUSED_STREAM.into(),
                )
                .into());
            }
            sender
                .send_request(request, true)
                .map_err(crate::ErrorKind::DownloadH2Error)?
        };
        let response = tokio::time::timeout(STREAM_READY_TIMEOUT, response)
            .await
            .map_err(|_| crate::ErrorKind::DownloadStreamStalled {
                context: H2_POOL_HEADERS.into(),
            })?
            .map_err(crate::ErrorKind::DownloadH2Error)?;
        if response.status().is_success() {
            self.successful_responses.fetch_add(1, Ordering::AcqRel);
        }
        Ok(response)
    }
}

impl Drop for SharedH2Connection {
    fn drop(&mut self) {
        self.evict.notify_one();
    }
}

type ConnectionSlot = Arc<AsyncMutex<Option<Arc<SharedH2Connection>>>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ConnectionPoolKey {
    budget_scope: String,
    selection: Option<super::endpoint_selection::PoolKey>,
}

fn connection_identity(
    route: &DownloadRoute,
) -> Result<(ConnectionPoolKey, Option<EndpointLease>), H2ConnectError> {
    let authority =
        crate::util::fetch::url_authority(&route.url).ok_or_else(|| {
            H2ConnectError::new(
                H2ConnectFailureKind::Protocol,
                H2_POOL_HTTP_2_ROUTE_HAS_NO_AUTHORITY.into(),
            )
        })?;
    let lease = super::proxy_context::clients().and_then(|clients| {
        if !super::endpoint_selection::EndpointSelector::eligible_network(
            &clients,
            route.proxy,
        ) {
            return None;
        }
        clients
            .endpoints
            .lease(&url::Url::parse(&route.url).ok()?, Protocol::H2)
    });
    Ok((
        ConnectionPoolKey {
            budget_scope: super::proxy_context::authority_key(
                &authority,
                route.proxy,
            ),
            selection: lease.as_ref().map(|lease| lease.pool.clone()),
        },
        lease,
    ))
}

fn connection_matches(
    route: &DownloadRoute,
    lease: Option<&EndpointLease>,
) -> bool {
    connection_identity(route).is_ok_and(|(_, current)| {
        current.as_ref().map(|l| &l.pool) == lease.map(|l| &l.pool)
    })
}

/// Registry of live shared connections, keyed by authority.
static CONNECTIONS: std::sync::LazyLock<
    AsyncMutex<HashMap<ConnectionPoolKey, ConnectionSlot>>,
> = std::sync::LazyLock::new(|| AsyncMutex::new(HashMap::new()));

type ParallelConnectionPool =
    Arc<AsyncMutex<super::authority_scheduler::AuthorityScheduler>>;

static PARALLEL_CONNECTIONS: std::sync::LazyLock<
    AsyncMutex<HashMap<ConnectionPoolKey, ParallelConnectionPool>>,
> = std::sync::LazyLock::new(|| AsyncMutex::new(HashMap::new()));

async fn connection_slot(key: &ConnectionPoolKey) -> ConnectionSlot {
    let mut connections = CONNECTIONS.lock().await;
    connections
        .entry(key.clone())
        .or_insert_with(|| Arc::new(AsyncMutex::new(None)))
        .clone()
}

async fn parallel_connection_pool(
    key: &ConnectionPoolKey,
) -> ParallelConnectionPool {
    let mut connections = PARALLEL_CONNECTIONS.lock().await;
    connections
        .entry(key.clone())
        .or_insert_with(|| {
            Arc::new(AsyncMutex::new(
                super::authority_scheduler::AuthorityScheduler::default(),
            ))
        })
        .clone()
}

fn tls_config() -> Arc<ClientConfig> {
    #[cfg(test)]
    if let Ok(config) = TEST_TLS_CONFIG.try_with(Arc::clone) {
        return config;
    }
    static INSTALL_PROVIDER: Once = Once::new();
    INSTALL_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    static CONFIG: std::sync::LazyLock<Mutex<Option<Arc<ClientConfig>>>> =
        std::sync::LazyLock::new(|| Mutex::new(None));
    let mut guard = CONFIG
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(config) = guard.as_ref() {
        return Arc::clone(config);
    }
    let mut config = ClientConfig::builder()
        .with_root_certificates(platform_root_certs())
        .with_no_client_auth();
    config.enable_early_data = true;
    config.alpn_protocols = vec![b"h2".to_vec()];
    let config = Arc::new(config);
    *guard = Some(Arc::clone(&config));
    config
}

#[cfg(test)]
tokio::task_local! { static TEST_TLS_CONFIG: Arc<ClientConfig>; }

#[cfg(test)]
pub(crate) async fn with_test_tls_config<F: std::future::Future>(
    config: Arc<ClientConfig>,
    future: F,
) -> F::Output {
    TEST_TLS_CONFIG.scope(config, future).await
}

fn platform_root_certs() -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs();
    for cert in certs.certs {
        let _ = store.add(cert);
    }
    // Match reqwest's native + WebPKI trust sources even when the platform
    // store loads successfully but lacks a public root (common on Windows).
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    store
}

async fn connect_addresses(
    host: &str,
    port: u16,
    addresses: &[IpAddr],
    parallel: bool,
) -> std::io::Result<TcpStream> {
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        let mut remaining = addresses.iter().copied();
        let attempt = |address: IpAddr, delay: Duration| async move {
            tokio::time::sleep(delay).await;
            TcpStream::connect((address, port)).await
        };
        let mut pending = FuturesUnordered::new();
        if let Some(first) = remaining.next() {
            pending.push(attempt(first, Duration::ZERO));
        }
        if parallel && let Some(second) = remaining.next() {
            let delay = if second.is_ipv4() {
                HAPPY_EYEBALLS_IPV4_DELAY
            } else {
                HAPPY_EYEBALLS_IPV6_DELAY
            };
            pending.push(attempt(second, delay));
        }
        let mut last_error = None;
        while let Some(result) = pending.next().await {
            match result {
                Ok(stream) => {
                    stream.set_nodelay(true).ok();
                    return Ok(stream);
                }
                Err(error) => {
                    last_error = Some(error);
                    if let Some(next) = remaining.next() {
                        pending.push(attempt(next, Duration::ZERO));
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no addresses available for {host}"),
            )
        }))
    })
    .await
    .map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("connection to {host}:{port} timed out"),
        )
    })?
}

#[cfg(test)]
mod address_tests {
    use super::*;

    #[tokio::test]
    async fn progresses_to_third_address_after_two_connection_failures() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let addresses = [
            "127.0.0.2".parse().unwrap(),
            "127.0.0.3".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ];
        let stream = tokio::time::timeout(
            H2_CONNECTION_PROBE_TIMEOUT,
            connect_addresses(
                "three-addresses.invalid",
                port,
                &addresses,
                true,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(stream.peer_addr().unwrap(), listener.local_addr().unwrap());
    }

    #[tokio::test]
    async fn empty_address_list_returns_not_found() {
        let error = connect_addresses("empty.invalid", 443, &[], true)
            .await
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn saturated_address_fallback_keeps_recovery_slot_and_temporary_race_admission_releases()
     {
        use crate::util::fetch::{
            DownloadClients, DownloadRouteSource, ProxyPolicy,
        };
        use crate::util::proxy::{ProxyConfig, ProxyMode};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut clients = DownloadClients::build(
            &ProxyConfig {
                mode: ProxyMode::None,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        clients.scope = format!("dns-race-budget-{port}");
        clients.endpoints =
            super::super::endpoint_selection::EndpointSelector::with_mode(
                clients.scope.clone(),
                super::super::endpoint_selection::SelectionMode::Auto,
            );
        clients.system_dns.set_test_addresses(
            "cdn.modrinth.com",
            ["127.0.0.2", "127.0.0.3", "127.0.0.1"]
                .into_iter()
                .map(|ip| std::net::SocketAddr::new(ip.parse().unwrap(), 0))
                .collect(),
        );
        let route = DownloadRoute {
            url: "https://cdn.modrinth.com/data/test/file.jar".into(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::System,
        };
        super::super::proxy_context::with_snapshot(clients, async {
            let limit = super::super::constants::MAX_CONNECTIONS_PER_AUTHORITY;
            // One of these normal permits is the caller's first socket.
            let mut permits: Vec<_> = (0..limit - 1)
                .map(|_| {
                    super::super::native_budget::try_acquire(&route).unwrap()
                })
                .collect();
            assert_eq!(super::super::native_budget::available(&route), 1);
            assert!(address_race_budget(&route, true, 3).is_none());
            let stream = tokio::time::timeout(
                H2_DNS_REFRESH_TIMEOUT,
                connect_tcp("cdn.modrinth.com", port, &route, true),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(
                stream.peer_addr().unwrap(),
                listener.local_addr().unwrap()
            );
            assert_eq!(super::super::native_budget::available(&route), 1);
            drop(stream);
            permits.pop();
            let race = address_race_budget(&route, true, 3).unwrap();
            assert_eq!(super::super::native_budget::available(&route), 1);
            drop(race);
            assert_eq!(super::super::native_budget::available(&route), 2);
            assert!(address_race_budget(&route, true, 1).is_none());
            drop(permits);
            assert_eq!(super::super::native_budget::available(&route), limit);
        })
        .await;
    }
}

async fn connect_tcp(
    host: &str,
    port: u16,
    route: &DownloadRoute,
    reserve_native_budget: bool,
) -> std::io::Result<TcpStream> {
    // Prefer the ordered address list from the shared download resolver
    // (IPv4/IPv6 preference and per-IP reliability), falling back to the
    // system resolver when no list is cached yet.
    let resolver = super::proxy_context::resolver(route.proxy);
    let mut addresses = resolver.resolved_addresses(host);
    if addresses.is_empty() {
        // TTL-zero answers are usable for this attempt without entering the
        // cache. Do not discard them by rereading the cache after resolution.
        addresses = resolver.resolve_host(host).await?;
    }
    let mut last_error = None;
    // The caller holds one physical permit. Racing a second TCP socket
    // requires another normal admission; it cannot consume the recovery slot.
    // The loser is closed before this temporary permit is released.
    let _race_budget =
        address_race_budget(route, reserve_native_budget, addresses.len());
    let parallel = _race_budget.is_some();
    if !addresses.is_empty() {
        match connect_addresses(host, port, &addresses, parallel).await {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
        if resolver.record_connection_failure(host) {
            let refreshed = resolver.resolve_host(host).await?;
            if !refreshed.is_empty() && refreshed != addresses {
                match connect_addresses(host, port, &refreshed, parallel).await
                {
                    Ok(stream) => return Ok(stream),
                    Err(error) => last_error = Some(error),
                }
            }
        }
    }
    if !resolver.doh_enabled() {
        let stream = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio::net::TcpStream::connect((host, port)),
        )
        .await
        .map_err(|_| {
            last_error.take().unwrap_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("connection to {host}:{port} timed out"),
                )
            })
        })?
        .map_err(|error| {
            last_error.take().unwrap_or_else(|| {
                std::io::Error::new(
                    error.kind(),
                    format!("connection to {host}:{port} failed: {error}"),
                )
            })
        })?;
        stream.set_nodelay(true).ok();
        return Ok(stream);
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no resolved addresses available for {host}:{port}"),
        )
    }))
}

fn address_race_budget(
    route: &DownloadRoute,
    reserve_native_budget: bool,
    address_count: usize,
) -> Option<super::native_budget::NativeBudgetPermit> {
    (reserve_native_budget && address_count > 1)
        .then(|| super::native_budget::try_acquire(route).ok())
        .flatten()
}

/// Connects a new shared HTTP/2 connection to `authority` (host[:port]).
async fn establish(
    route: &DownloadRoute,
    reserve_native_budget: bool,
    lease: Option<EndpointLease>,
) -> Result<Arc<SharedH2Connection>, H2ConnectError> {
    if let Some(reason) = super::native::h2_ineligible_reason(route) {
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            reason.as_str().to_string(),
        ));
    }
    let authority =
        crate::util::fetch::url_authority(&route.url).ok_or_else(|| {
            H2ConnectError::new(
                H2ConnectFailureKind::Protocol,
                H2_POOL_HTTP_2_ROUTE_HAS_NO_AUTHORITY.to_string(),
            )
        })?;
    let physical_budget = if reserve_native_budget {
        Some(super::native_budget::acquire(route).await.map_err(|error| {
            H2ConnectError::new(
                H2ConnectFailureKind::Tcp,
                format!("failed to reserve HTTP/2 connection capacity: {error}"),
            )
        })?)
    } else {
        None
    };
    let (host, port) = authority
        .rsplit_once(':')
        .map(|(host, port)| (host, port.parse::<u16>().unwrap_or(HTTPS_PORT)))
        .unwrap_or((&authority, HTTPS_PORT));

    let tcp_result = if let Some(lease) = &lease {
        tokio::time::timeout(
            CONNECT_TIMEOUT,
            TcpStream::connect(lease.pool.endpoint),
        )
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                H2_POOL_PINNED_TCP_CONNECTION_TIMED_OUT,
            ))
        })
    } else {
        connect_tcp(host, port, route, reserve_native_budget).await
    };
    let tcp = tcp_result.map_err(|error| {
        H2ConnectError::new(
            H2ConnectFailureKind::Tcp,
            format!(
                "failed to establish shared HTTP/2 connection to {authority}: {error}"
            ),
        )
    })?;

    let remote_addr = tcp.peer_addr().ok();
    let server_name =
        ServerName::try_from(host.to_string()).map_err(|error| {
            H2ConnectError::new(
                H2ConnectFailureKind::Tls,
                format!("invalid server name for {host}: {error}"),
            )
        })?;
    let connector = TlsConnector::from(tls_config());
    let tls = tokio::time::timeout(
        TLS_HANDSHAKE_TIMEOUT,
        connector.connect(server_name, tcp),
    )
    .await
    .map_err(|_| {
        H2ConnectError::new(
            H2ConnectFailureKind::Tls,
            format!("TLS handshake with {authority} timed out"),
        )
    })?
    .map_err(|error| {
        H2ConnectError::new(
            H2ConnectFailureKind::Tls,
            format!("TLS handshake with {authority} failed: {error}"),
        )
    })?;
    if tls.get_ref().1.alpn_protocol() != Some(b"h2") {
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            H2_POOL_ENDPOINT_DID_NOT_NEGOTIATE_H2.into(),
        ));
    }

    let mut builder = h2::client::Builder::new();
    builder
        .initial_window_size(H2_INITIAL_STREAM_WINDOW)
        .initial_connection_window_size(H2_CONNECTION_WINDOW);
    let (sender, mut connection) =
        builder.handshake(Box::pin(tls)).await.map_err(|error| {
            H2ConnectError::new(
                H2ConnectFailureKind::Protocol,
                format!("HTTP/2 handshake with {authority} failed: {error}"),
            )
        })?;

    if let Some(address) = remote_addr {
        super::proxy_context::resolver(route.proxy)
            .record_host_success(host, address.ip());
    }

    // Tune flow-control windows for high-stream multiplexing (e.g. hundreds
    // of concurrent asset downloads over one connection). With the default
    // 64 KiB connection window, concurrent streams would stall waiting for
    // connection-level window updates; a larger per-stream window also lets
    // the peer send a whole small file without round trips.
    // Keep the adaptive connection target aligned with the initial connection
    // window configured before the handshake.
    connection.set_target_window_size(H2_CONNECTION_WINDOW);

    let mut shared =
        SharedH2Connection::new(authority.clone(), sender, physical_budget);
    shared.budget_scope =
        super::proxy_context::authority_key(&authority, route.proxy);
    shared.endpoint_lease = lease;
    shared.remote_addr = remote_addr;
    let shared = Arc::new(shared);

    spawn_connection_driver(&shared, connection, IDLE_EVICTION_TIMEOUT);

    Ok(shared)
}

pub(crate) fn spawn_connection_driver<F>(
    shared: &Arc<SharedH2Connection>,
    connection: F,
    idle_timeout: Duration,
) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future + Send + 'static,
{
    let dead = shared.dead.clone();
    let connection_budget = Arc::downgrade(shared);
    let idle_connection = connection_budget.clone();
    let evict = shared.evict.clone();
    let drained = shared.drained.clone();
    let retiring_connection = connection_budget.clone();
    let authority = shared.authority.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = connection => {}
            _ = evict.notified() => {
                tracing::debug!(authority, "Evicting idle shared HTTP/2 connection");
            }
            _ = wait_for_idle(idle_connection, idle_timeout) => {},
            _ = async {
                loop {
                    let notified = drained.notified();
                    let Some(shared) = retiring_connection.upgrade() else { return; };
                    if shared.is_retiring() && shared.active_streams() == 0 {
                        return;
                    }
                    drop(shared);
                    notified.await;
                }
            } => {},
        }
        dead.store(true, std::sync::atomic::Ordering::Release);
        if let Some(connection_budget) = connection_budget.upgrade() {
            connection_budget.release_physical_budget();
        }
        tracing::debug!(authority, "Shared HTTP/2 connection closed");
    })
}

async fn wait_for_idle(
    connection: std::sync::Weak<SharedH2Connection>,
    timeout: Duration,
) {
    loop {
        let delay = {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            if connection.active_streams() > 0 {
                timeout.min(DOWNLOAD_SCHEDULING_GRACE)
            } else {
                timeout.saturating_sub(
                    connection
                        .last_activity
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .elapsed(),
                )
            }
        };
        if delay.is_zero() {
            return;
        }
        tokio::time::sleep(delay).await;
    }
}

pub(crate) async fn evict_idle_connections(scope: Option<&str>) {
    let slots = CONNECTIONS
        .lock()
        .await
        .iter()
        .filter(|(key, _)| scope.is_none_or(|scope| key.budget_scope == scope))
        .map(|(_, slot)| slot.clone())
        .collect::<Vec<_>>();
    for slot in slots {
        if let Ok(cached) = slot.try_lock()
            && let Some(connection) = cached.as_ref()
            && connection.active_streams() == 0
        {
            connection.evict();
        }
    }
    let pools = PARALLEL_CONNECTIONS
        .lock()
        .await
        .iter()
        .filter(|(key, _)| scope.is_none_or(|scope| key.budget_scope == scope))
        .map(|(_, pool)| pool.clone())
        .collect::<Vec<_>>();
    for pool in pools {
        if let Ok(mut state) = pool.try_lock() {
            for connection in state.connections_for_eviction() {
                if connection.active_streams() == 0 {
                    connection.evict();
                }
            }
            state.prune();
        }
    }
}

/// Advance measured trials even while long-lived streams produce no new
/// connection checkout. Otherwise an unproductive sibling could remain
/// admitted until an unrelated future request happens to sample the pool.
pub(crate) async fn observe_adaptive_connections() {
    let pools: Vec<_> = PARALLEL_CONNECTIONS
        .lock()
        .await
        .values()
        .cloned()
        .collect();
    let now = std::time::Instant::now();
    let pressure =
        super::local_resources::network_pressure().blocks_expansion();
    for pool in pools {
        if let Ok(mut state) = pool.try_lock() {
            state.observe_background(now, pressure);
        }
    }
}

async fn retire_obsolete_connections(current: &ConnectionPoolKey) {
    let slots = CONNECTIONS
        .lock()
        .await
        .iter()
        .filter(|(key, _)| {
            key.budget_scope == current.budget_scope && *key != current
        })
        .map(|(_, slot)| slot.clone())
        .collect::<Vec<_>>();
    for slot in slots {
        // An old handshake may hold the slot; its captured lease is checked
        // before publication and cannot enter the current generation's pool.
        if let Ok(mut cached) = slot.try_lock()
            && let Some(connection) = cached.take()
        {
            connection.retire();
        }
    }
    let pools = PARALLEL_CONNECTIONS
        .lock()
        .await
        .iter()
        .filter(|(key, _)| {
            key.budget_scope == current.budget_scope && *key != current
        })
        .map(|(_, pool)| pool.clone())
        .collect::<Vec<_>>();
    for pool in pools {
        if let Ok(mut state) = pool.try_lock() {
            for connection in state.connections_for_eviction() {
                connection.retire();
            }
            state.prune();
        }
    }
    CONNECTIONS.lock().await.retain(|key, slot| {
        key == current
            || key.budget_scope != current.budget_scope
            || Arc::strong_count(slot) > 1
    });
    PARALLEL_CONNECTIONS.lock().await.retain(|key, pool| {
        key == current
            || key.budget_scope != current.budget_scope
            || Arc::strong_count(pool) > 1
    });
}

/// Returns the live shared connection for `authority`, establishing one on
/// first use or after a previous connection died.
async fn shared_connection_single(
    route: &DownloadRoute,
    reserve_native_budget: bool,
    allow_cold_connection: bool,
) -> Result<Arc<SharedH2Connection>, H2ConnectError> {
    let deadline = tokio::time::Instant::now() + CONNECTION_WAIT_TIMEOUT;
    for attempt in 0..H2_CONNECTION_ATTEMPTS {
        let (_, lease) = connection_identity(route)?;
        let result = tokio::time::timeout_at(
            deadline,
            shared_connection_single_attempt(
                route,
                reserve_native_budget,
                allow_cold_connection,
            ),
        )
        .await
        .map_err(|_| {
            H2ConnectError::new(
                H2ConnectFailureKind::Tcp,
                H2_POOL_H2_CONNECTION_DEADLINE_EXHAUSTED.into(),
            )
        })?;
        match result {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                if let Some(lease) = &lease
                    && let Some(clients) = super::proxy_context::clients()
                {
                    clients.endpoints.record_failure(
                        lease,
                        error.kind == H2ConnectFailureKind::Tls,
                    );
                    if attempt == 0 && !clients.endpoints.is_current(lease) {
                        continue;
                    }
                }
                return Err(error);
            }
        }
    }
    unreachable!("bounded H2 attempts always return")
}

async fn shared_connection_single_attempt(
    route: &DownloadRoute,
    reserve_native_budget: bool,
    allow_cold_connection: bool,
) -> Result<Arc<SharedH2Connection>, H2ConnectError> {
    if let Some(reason) = super::native::h2_ineligible_reason(route) {
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            reason.as_str().to_string(),
        ));
    }
    let authority =
        crate::util::fetch::url_authority(&route.url).ok_or_else(|| {
            H2ConnectError::new(
                H2ConnectFailureKind::Protocol,
                H2_POOL_HTTP_2_ROUTE_HAS_NO_AUTHORITY.to_string(),
            )
        })?;
    let (key, lease) = connection_identity(route)?;
    retire_obsolete_connections(&key).await;
    let slot = connection_slot(&key).await;
    let deadline = tokio::time::Instant::now() + CONNECTION_WAIT_TIMEOUT;
    let mut cached = tokio::time::timeout_at(deadline, slot.lock())
        .await
        .map_err(|_| {
            H2ConnectError::new(
                H2ConnectFailureKind::Protocol,
                format!(
                    "timed out waiting for HTTP/2 connection slot {authority}"
                ),
            )
        })?;
    if !connection_matches(route, lease.as_ref()) {
        if let Some(connection) = cached.as_ref() {
            connection.retire();
        }
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Tcp,
            H2_POOL_ENDPOINT_CHANGED_WHILE_WAITING_FOR_THE_H2_CONNECTION_SLOT
                .into(),
        ));
    }
    if let Some(connection) = cached.as_ref().filter(|connection| {
        !connection.is_dead()
            && !connection.is_retiring()
            && !connection.is_idle_expired()
    }) {
        if !reserve_native_budget || connection.has_physical_budget() {
            tracing::debug!(authority, "Reusing shared HTTP/2 connection");
            return Ok(Arc::clone(connection));
        }
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            H2_POOL_SHARED_HTTP_2_CONNECTION_IS_NOT_COVERED_BY_THE.to_string(),
        ));
    }
    if cached.as_ref().is_some_and(|connection| {
        connection.is_dead()
            || connection.is_retiring()
            || connection.is_idle_expired()
    }) {
        if let Some(connection) = cached.as_ref()
            && !connection.is_retiring()
        {
            connection.evict();
        }
        *cached = None;
    }
    if !allow_cold_connection {
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            H2_POOL_HTTP_2_POLICY_REQUIRES_AN_EXISTING_SHARED_CONNECTION
                .to_string(),
        ));
    }
    tracing::debug!(authority, "Establishing cold shared HTTP/2 connection");
    let connection = tokio::time::timeout_at(
        deadline,
        establish(route, reserve_native_budget, lease),
    )
    .await
    .map_err(|_| {
        H2ConnectError::new(
            H2ConnectFailureKind::Tcp,
            format!("timed out establishing HTTP/2 connection to {authority}"),
        )
    })??;
    if !connection_matches(route, connection.endpoint_lease.as_ref()) {
        connection.retire();
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Tcp,
            H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_THE_H2_HANDSHAKE.into(),
        ));
    }
    *cached = Some(Arc::clone(&connection));
    Ok(connection)
}

/// Returns the least-loaded H2 connection for a parallel content transfer.
/// Every selected connection remains fully multiplexed; expansion only adds a
/// new TCP/TLS connection after the existing pool is carrying sustained load.
pub(crate) async fn shared_connection(
    route: &DownloadRoute,
    reserve_native_budget: bool,
    allow_cold_connection: bool,
    allow_parallel_connections: bool,
) -> Result<Arc<SharedH2Connection>, H2ConnectError> {
    if !allow_parallel_connections || !ADDITIONAL_H2_CONNECTIONS_ENABLED {
        return shared_connection_single(
            route,
            reserve_native_budget,
            allow_cold_connection,
        )
        .await;
    }

    let primary = shared_connection_single(
        route,
        reserve_native_budget,
        allow_cold_connection,
    )
    .await?;
    let (key, lease) = connection_identity(route)?;
    if !connection_matches(route, primary.endpoint_lease.as_ref()) {
        primary.retire();
        return Err(H2ConnectError::new(
            H2ConnectFailureKind::Tcp,
            H2_POOL_ENDPOINT_CHANGED_BEFORE_H2_STREAM_ASSIGNMENT.into(),
        ));
    }
    let pool = parallel_connection_pool(&key).await;
    let route = route.clone();
    let clients = super::proxy_context::clients();
    select_parallel_connection(
        &pool,
        primary,
        allow_cold_connection
            && !super::native_breaker::is_open(&route)
            && !super::local_resources::network_pressure().blocks_expansion(),
        move || async move {
            let establish = async {
                let sibling =
                    establish(&route, reserve_native_budget, lease).await?;
                if !connection_matches(&route, sibling.endpoint_lease.as_ref())
                {
                    sibling.retire();
                    return Err(H2ConnectError::new(
                        H2ConnectFailureKind::Tcp,
                        H2_POOL_ENDPOINT_SELECTION_CHANGED_DURING_H2_EXPANSION
                            .into(),
                    ));
                }
                Ok(sibling)
            };
            match clients {
                Some(clients) => {
                    super::proxy_context::with_snapshot(clients, establish)
                        .await
                }
                None => establish.await,
            }
        },
    )
    .await
}

async fn select_parallel_connection<F, Fut>(
    pool: &ParallelConnectionPool,
    primary: Arc<SharedH2Connection>,
    allow_expansion: bool,
    establish_sibling: F,
) -> Result<Arc<SharedH2Connection>, H2ConnectError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<
            Output = Result<Arc<SharedH2Connection>, H2ConnectError>,
        > + Send
        + 'static,
{
    let mut state = pool.lock().await;
    state.enable_adaptive(std::time::Instant::now());
    state.prune();
    state.add_connection(Arc::clone(&primary));
    if !allow_expansion {
        state.suspend_adaptive(std::time::Instant::now());
    }
    state.observe(std::time::Instant::now());
    let selected = state.select_connection().ok_or_else(|| {
        H2ConnectError::new(
            H2ConnectFailureKind::Protocol,
            H2_POOL_HTTP_2_POOL_HAS_NO_LIVE_CONNECTION.into(),
        )
    })?;
    let connecting =
        if allow_expansion && state.can_expand(std::time::Instant::now()) {
            state.reserve_connector()
        } else {
            None
        };
    let Some(_connecting) = connecting else {
        return Ok(selected);
    };
    drop(state);
    let background_pool = pool.clone();
    tokio::spawn(async move {
        let _connecting = _connecting;
        let sibling = match tokio::time::timeout(
            CONNECTION_WAIT_TIMEOUT,
            establish_sibling(),
        )
        .await
        {
            Ok(Ok(sibling)) => sibling,
            result => {
                let mut state = background_pool.lock().await;
                state.rollback(std::time::Instant::now());
                tracing::debug!(
                    timed_out = result.is_err(),
                    "Optional HTTP/2 pool expansion failed"
                );
                return;
            }
        };
        let mut state = background_pool.lock().await;
        state.prune();
        if state.connection_count() < state.target() {
            state.add_connection(sibling);
        } else {
            sibling.evict_for_scheduler();
        }
    });
    Ok(selected)
}

#[cfg(test)]
mod parallel_pool_tests {
    use super::super::authority_scheduler::AuthorityScheduler;
    use super::*;

    #[test]
    fn scheduler_uses_the_requested_ladder() {
        assert_eq!(AuthorityScheduler::targets(), [4, 8, 16, 32]);
    }

    async fn memory_connection() -> Arc<SharedH2Connection> {
        let (client, _server) = tokio::io::duplex(64 * 1024);
        let (sender, _driver) = h2::client::handshake(client).await.unwrap();
        Arc::new(SharedH2Connection::for_test(sender))
    }

    #[tokio::test]
    async fn reservations_are_counted_before_stream_activation() {
        let first = memory_connection().await;
        let second = memory_connection().await;
        let mut scheduler = AuthorityScheduler::default();
        scheduler.add_connection(first.clone());
        scheduler.add_connection(second.clone());
        first.reserve_stream_for_scheduler();
        first.reserve_stream_for_scheduler();
        assert!(Arc::ptr_eq(
            &scheduler.select_connection().unwrap(),
            &second
        ));
    }

    #[tokio::test]
    async fn idle_connections_are_pruned() {
        let connection = memory_connection().await;
        let mut scheduler = AuthorityScheduler::default();
        scheduler.add_connection(connection.clone());
        connection.evict_for_scheduler();
        scheduler.prune();
        assert_eq!(scheduler.connection_count(), 0);
        assert_eq!(scheduler.target(), 4);
    }

    #[tokio::test]
    async fn non_modrinth_pool_expands_and_failed_expansion_keeps_primary() {
        for succeeds in [true, false] {
            let primary = memory_connection().await;
            let sibling = memory_connection().await;
            let pool = Arc::new(AsyncMutex::new(AuthorityScheduler::default()));
            let first = select_parallel_connection(
                &pool,
                primary.clone(),
                true,
                || async {
                    panic!("one assigned stream must not expand the pool");
                },
            )
            .await
            .unwrap();
            assert!(Arc::ptr_eq(&first, &primary));
            assert_eq!(
                pool.lock().await.target(),
                super::super::constants::ADAPTIVE_CONNECTION_INITIAL_TARGET
            );
            let attempted = Arc::new(Notify::new());
            let notify = attempted.clone();
            let second = select_parallel_connection(
                &pool,
                primary.clone(),
                true,
                move || async move {
                    notify.notify_one();
                    if succeeds {
                        Ok(sibling)
                    } else {
                        Err(H2ConnectError::new(
                            H2ConnectFailureKind::Tcp,
                            "injected optional expansion failure".into(),
                        ))
                    }
                },
            )
            .await
            .unwrap();
            assert!(Arc::ptr_eq(&second, &primary));
            tokio::time::timeout(Duration::from_secs(2), attempted.notified())
                .await
                .unwrap();
            tokio::task::yield_now().await;
            let mut state = pool.lock().await;
            assert_eq!(state.connection_count(), if succeeds { 2 } else { 1 });
            assert!(!primary.is_dead());
            assert!(!primary.is_retiring());
            assert!(state.select_connection().is_some());
            primary.release_stream_reservation();
            primary.release_stream_reservation();
        }
    }
}
/// Selects an asset connection through the same pool used by file downloads.
pub(crate) async fn shared_batch_connection(
    route: &DownloadRoute,
    reserve_native_budget: bool,
) -> Result<Arc<SharedH2Connection>, H2ConnectError> {
    shared_connection(route, reserve_native_budget, true, true).await
}

pub(crate) async fn has_live_connection(route: &DownloadRoute) -> bool {
    let Ok((key, _)) = connection_identity(route) else {
        return false;
    };
    let connections = CONNECTIONS.lock().await;
    let Some(slot) = connections.get(&key).cloned() else {
        return false;
    };
    drop(connections);

    slot.lock().await.as_ref().is_some_and(|connection| {
        !connection.is_dead() && !connection.is_retiring()
    })
}

#[cfg(test)]
pub(crate) async fn hold_slot_for_test(
    route: &DownloadRoute,
) -> tokio::sync::OwnedMutexGuard<Option<Arc<SharedH2Connection>>> {
    let (key, _) = connection_identity(route).unwrap();
    connection_slot(&key).await.lock_owned().await
}
