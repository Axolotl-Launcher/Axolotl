use super::coordinator::ProbeFixture;
use super::{EndpointSelector, SelectionMode};
use crate::util::fetch::DownloadClients;
use parking_lot::Mutex;
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
use rustls_pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
pub(super) struct Profile {
    pub(super) bytes_per_second: u64,
    pub(super) ttfb: Duration,
    pub(super) slow_tail: Option<(u64, u64)>,
    pub(super) interrupt_after: Option<u64>,
    pub(super) wrong_range: bool,
}

impl Profile {
    pub(super) fn rate(bytes_per_second: u64) -> Self {
        Self {
            bytes_per_second,
            ttfb: Duration::ZERO,
            slow_tail: None,
            interrupt_after: None,
            wrong_range: false,
        }
    }
}

#[derive(Default)]
pub(super) struct SocketStats {
    pub(super) active: AtomicUsize,
    pub(super) peak: AtomicUsize,
    pub(super) opened: AtomicUsize,
    pub(super) requests: AtomicU64,
    pub(super) requested_ranges: Mutex<Vec<(u64, u64)>>,
    pub(super) server_payload: AtomicU64,
    pub(super) body_elapsed_us: AtomicU64,
    pub(super) send_us: AtomicU64,
    pub(super) pace_sleep_us: AtomicU64,
    pub(super) max_pace_late_us: AtomicU64,
}

struct ActiveSocket(Arc<SocketStats>);
impl ActiveSocket {
    fn new(stats: Arc<SocketStats>) -> Self {
        let active = stats.active.fetch_add(1, Ordering::SeqCst) + 1;
        stats.peak.fetch_max(active, Ordering::SeqCst);
        stats.opened.fetch_add(1, Ordering::SeqCst);
        Self(stats)
    }
}
impl Drop for ActiveSocket {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(super) struct EndpointFixture {
    pub(super) address: SocketAddr,
    pub(super) root: reqwest::Certificate,
    pub(super) rustls_root: CertificateDer<'static>,
    pub(super) stats: Arc<SocketStats>,
    cancellation: CancellationToken,
    task: Option<std::thread::JoinHandle<()>>,
}

impl Drop for EndpointFixture {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            let result = task.join();
            if !std::thread::panicking() {
                result.expect("benchmark server thread");
            }
        }
    }
}

impl EndpointFixture {
    pub(super) async fn start(
        bind_ip: Ipv4Addr,
        profile: Profile,
        stats: Arc<SocketStats>,
    ) -> Self {
        let mut ca_params = CertificateParams::new(vec![]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let issuer = Issuer::from_params(&ca_params, &ca_key);
        let leaf_key = KeyPair::generate().unwrap();
        let cert = CertificateParams::new(vec!["cdn.modrinth.com".into()])
            .unwrap()
            .signed_by(&leaf_key, &issuer)
            .unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone(), ca.der().clone()],
                PrivatePkcs8KeyDer::from(leaf_key.serialize_der()).into(),
            )
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = std::net::TcpListener::bind((bind_ip, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let stop = cancellation.clone();
        let server_stats = stats.clone();
        let cached_objects = Arc::new(Mutex::new(HashSet::new()));
        // Keep the emulated remote server off the client's single-thread
        // test executor. Client hashing or disk polling must not delay the
        // server's pace clock and masquerade as a slower network endpoint.
        let task = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    connection = listener.accept() => {
                        let Ok((socket, _)) = connection else { break };
                        socket.set_nodelay(true).unwrap();
                        let acceptor = acceptor.clone();
                        let stats = server_stats.clone();
                        let cached_objects = cached_objects.clone();
                        let activity = ActiveSocket::new(stats.clone());
                        handlers.spawn(async move {
                            let _activity = activity;
                            let Ok(mut socket) = acceptor.accept(socket).await else { return };
                            let mut pending = Vec::new();
                            loop {
                                let header_end = loop {
                                    if let Some(index) = pending.windows(4).position(|b| b == b"\r\n\r\n") { break index + 4; }
                                    let mut chunk = [0; 4096];
                                    let Ok(read) = socket.read(&mut chunk).await else { return };
                                    if read == 0 || pending.len() + read > 16384 { return; }
                                    pending.extend_from_slice(&chunk[..read]);
                                };
                                let headers = String::from_utf8_lossy(&pending[..header_end]).to_string();
                                pending.drain(..header_end);
                                let path = headers.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                                let size = path.split('/').nth(3).unwrap().parse::<u64>().unwrap();
                                let range = headers.lines().find_map(|line| {
                                    let (key, value) = line.split_once(':')?;
                                    key.eq_ignore_ascii_case("range").then_some(value.trim())
                                });
                                let (start, end) = range.map(|range| {
                                    let (start, end) = range.strip_prefix("bytes=").unwrap().split_once('-').unwrap();
                                    (start.parse::<u64>().unwrap(), if end.is_empty() { size - 1 } else { end.parse().unwrap() })
                                }).unwrap_or((0, size - 1));
                                assert!(start <= end && end < size);
                                stats.requests.fetch_add(1, Ordering::Relaxed);
                                stats.requested_ranges.lock().push((start, end));
                                let length = end + 1 - start;
                                let range_header = if range.is_some() {
                                    format!("Content-Range: bytes {}-{end}/{size}\r\n", if profile.wrong_range { start + 1 } else { start })
                                } else { String::new() };
                                let status = if range.is_some() { "206 Partial Content" } else { "200 OK" };
                                let cache = if cached_objects.lock().insert(path.to_string()) { "MISS" } else { "HIT" };
                                let response = format!("HTTP/1.1 {status}\r\n{range_header}Content-Length: {length}\r\nX-Served-By: local-benchmark\r\nX-Cache: {cache}\r\nConnection: keep-alive\r\n\r\n");
                                tokio::time::sleep(profile.ttfb).await;
                                if socket.write_all(response.as_bytes()).await.is_err() { return; }
                                if socket.flush().await.is_err() { return; }
                                let mut offset = start;
                                let body_started = Instant::now();
                                let mut clock = body_started;
                                let mut sent = 0u64;
                                let mut last_rate = profile.bytes_per_second;
                                while offset <= end {
                                    if profile.interrupt_after.is_some_and(|limit| sent >= limit) { return; }
                                    let rate = profile.slow_tail.filter(|(tail, _)| offset >= size.saturating_sub(*tail)).map_or(profile.bytes_per_second, |(_, rate)| rate);
                                    if rate != last_rate { sent = 0; clock = Instant::now(); last_rate = rate; }
                                    let count = (end + 1 - offset).min((rate / 50).clamp(1024, 256 * 1024));
                                    // Application pacing is not TCP packet-loss emulation.
                                    let due = clock + Duration::from_secs_f64((sent + count) as f64 / rate as f64);
                                    let sleep_started = Instant::now();
                                    tokio::time::sleep_until(due.into()).await;
                                    stats.pace_sleep_us.fetch_add(sleep_started.elapsed().as_micros() as u64, Ordering::Relaxed);
                                    stats.max_pace_late_us.fetch_max(Instant::now().saturating_duration_since(due).as_micros() as u64, Ordering::Relaxed);
                                    let send_started = Instant::now();
                                    if socket.write_all(&vec![42; count as usize]).await.is_err() { return; }
                                    // TLS accepts plaintext into its own buffer;
                                    // an HTTP/1 fixture must flush each paced
                                    // frame rather than wait for the next request.
                                    if socket.flush().await.is_err() { return; }
                                    stats.send_us.fetch_add(send_started.elapsed().as_micros() as u64, Ordering::Relaxed);
                                    stats.server_payload.fetch_add(count, Ordering::Relaxed);
                                    sent += count; offset += count;
                                }
                                stats.body_elapsed_us.fetch_add(body_started.elapsed().as_micros() as u64, Ordering::Relaxed);
                            }
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => { result.unwrap().unwrap(); }
                }
            }
            handlers.shutdown().await;
            });
        });
        Self {
            address,
            root: reqwest::Certificate::from_der(ca.der()).unwrap(),
            rustls_root: ca.der().clone(),
            stats,
            cancellation,
            task: Some(task),
        }
    }
}

pub(super) fn clients(
    mode: SelectionMode,
    first: &EndpointFixture,
    second: &EndpointFixture,
    expanded: bool,
) -> DownloadClients {
    let mut clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: crate::util::proxy::ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    clients.scope = format!("benchmark-{}-{:?}", first.address.port(), mode);
    clients.endpoints =
        EndpointSelector::with_mode(clients.scope.clone(), mode);
    let base = crate::util::fetch::file_reqwest_client_builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .http1_only()
        .read_timeout(Duration::from_secs(60))
        .add_root_certificate(first.root.clone())
        .resolve("cdn.modrinth.com", first.address);
    let base = if mode == SelectionMode::Auto {
        base.pool_max_idle_per_host(0)
    } else {
        base
    }
    .build()
    .unwrap();
    clients.system = base.clone();
    clients.direct = base.clone();
    clients.http1_system = base.clone();
    clients.http1_direct = base;
    clients.force_http1 = true;
    for fixture in [first, second] {
        clients
            .endpoints
            .pinned_clients
            .trust_test_root(fixture.root.clone());
    }
    clients
        .system_dns
        .set_test_addresses("cdn.modrinth.com", vec![first.address]);
    clients
        .direct_dns
        .set_test_addresses("cdn.modrinth.com", vec![first.address]);
    *clients.endpoints.probe_fixture.lock() = Some(ProbeFixture {
        metered: false,
        candidates: if expanded {
            vec![
                (first.address, first.root.clone()),
                (second.address, second.root.clone()),
            ]
        } else {
            vec![(first.address, first.root.clone())]
        },
    });
    clients
}

pub(super) fn request(
    size: u64,
    id: usize,
) -> crate::util::fetch::DownloadRequest {
    let mut hash = sha1_smol::Sha1::new();
    let mut remaining = size;
    while remaining > 0 {
        let count = remaining.min(256 * 1024);
        hash.update(&vec![42; count as usize]);
        remaining -= count;
    }
    crate::util::fetch::DownloadRequest::new(
        format!("https://cdn.modrinth.com/data/benchmark/{size}/{id}.bin"),
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1(hash.hexdigest()).with_size(size),
    )
    .with_segmented_download(false)
}
