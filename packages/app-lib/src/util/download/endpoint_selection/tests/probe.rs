use super::*;
use crate::util::download::endpoint_selection::model::CandidateSource;
use crate::util::fetch::{DownloadRouteSource, ProxyPolicy};
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
use rustls_pki_types::PrivatePkcs8KeyDer;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy)]
enum ResponseKind {
    Valid,
    IgnoreRange,
    WrongTotal,
    Truncated,
    Encoded,
    Redirect,
    RateLimited,
    Missing,
    Stall,
    SlowFile,
    File,
    LargeSlowFile,
    LargeFile,
}

struct TlsFixture {
    address: SocketAddr,
    root: reqwest::Certificate,
    requests: Arc<AtomicU64>,
    server: tokio::task::JoinHandle<()>,
    protocol: Protocol,
    rustls_root: rustls_pki_types::CertificateDer<'static>,
}

impl Drop for TlsFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl TlsFixture {
    async fn start(host: &str, kind: ResponseKind) -> Self {
        Self::start_protocol(host, kind, Protocol::Http1).await
    }

    async fn start_protocol(
        host: &str,
        kind: ResponseKind,
        protocol: Protocol,
    ) -> Self {
        Self::start_certificate(host, kind, protocol, false).await
    }

    async fn start_certificate(
        host: &str,
        kind: ResponseKind,
        protocol: Protocol,
        expired: bool,
    ) -> Self {
        Self::start_on(host, kind, protocol, expired, Ipv4Addr::LOCALHOST).await
    }

    async fn start_on(
        host: &str,
        kind: ResponseKind,
        protocol: Protocol,
        expired: bool,
        bind_ip: Ipv4Addr,
    ) -> Self {
        Self::start_on_port(host, kind, protocol, expired, bind_ip, 0).await
    }

    async fn start_on_port(
        host: &str,
        kind: ResponseKind,
        protocol: Protocol,
        expired: bool,
        bind_ip: Ipv4Addr,
        port: u16,
    ) -> Self {
        let mut ca_params = CertificateParams::new(vec![]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let issuer = Issuer::from_params(&ca_params, &ca_key);
        let leaf_key = KeyPair::generate().unwrap();
        let mut params =
            CertificateParams::new(vec![host.to_string()]).unwrap();
        if expired {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let cert = params.signed_by(&leaf_key, &issuer).unwrap();
        let mut config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.der().clone(), ca.der().clone()],
                PrivatePkcs8KeyDer::from(leaf_key.serialize_der()).into(),
            )
            .unwrap();
        if protocol == Protocol::H2 {
            config.alpn_protocols = vec![b"h2".to_vec()];
        }
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind((bind_ip, port))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let root = reqwest::Certificate::from_der(ca.der()).unwrap();
        let requests = Arc::new(AtomicU64::new(0));
        let counted = requests.clone();
        let expected_host = host.to_string();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((socket, _)) = incoming else { break };
                        let acceptor = acceptor.clone();
                        let counted = counted.clone();
                        let expected_host = expected_host.clone();
                        handlers.spawn(async move {
                            let Ok(mut stream) = acceptor.accept(socket).await else { return };
                            assert_eq!(stream.get_ref().1.server_name(), Some(expected_host.as_str()));
                            if protocol == Protocol::H2 {
                                let mut connection = h2::server::handshake(stream).await.unwrap();
                                let mut transfers = tokio::task::JoinSet::new();
                                while let Some(request) = connection.accept().await {
                                    let (request, mut respond) = request.unwrap();
                                    counted.fetch_add(1, Ordering::SeqCst);
                                    assert_eq!(request.uri().host(), Some("cdn.modrinth.com"));
                                    assert!(request.headers().get(header::AUTHORIZATION).is_none());
                                    assert!(request.headers().get(crate::util::fetch::DOWNLOAD_META_HEADER).is_none());
                                    if matches!(kind, ResponseKind::RateLimited) {
                                        let response = http::Response::builder().status(429)
                                            .header(header::RETRY_AFTER, "30").header(header::CONTENT_LENGTH, 0)
                                            .body(()).unwrap();
                                        respond.send_response(response, true).unwrap();
                                        continue;
                                    }
                                    let size = if matches!(kind, ResponseKind::LargeSlowFile | ResponseKind::LargeFile) { 64 * 1024 * 1024 } else { 1_048_576 };
                                    let range = request.headers().get(header::RANGE);
                                    let (start, end) = range.map(|range| {
                                        let (start, end) = range.to_str().unwrap().strip_prefix("bytes=").unwrap().split_once('-').unwrap();
                                        (start.parse::<usize>().unwrap(), if end.is_empty() { size - 1 } else { end.parse::<usize>().unwrap() })
                                    }).unwrap_or((0, size - 1));
                                    let length = end + 1 - start;
                                    let response = http::Response::builder().status(if range.is_some() { 206 } else { 200 });
                                    let response = if range.is_some() {
                                        response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
                                    } else { response };
                                    let response = response
                                        .header(header::CONTENT_LENGTH, length)
                                        .header("x-served-by", "cache-h2-test")
                                        .body(()).unwrap();
                                    let mut stream = respond.send_response(response, false).unwrap();
                                    // The connection keeps polling while the task waits for
                                    // flow-control credit from the client.
                                    transfers.spawn(async move {
                                        let mut remaining = length;
                                        let started = tokio::time::Instant::now();
                                        while remaining > 0 {
                                            let slow = matches!(kind, ResponseKind::SlowFile | ResponseKind::LargeSlowFile);
                                            let wanted = remaining.min(if slow { 1024 } else { 16 * 1024 });
                                            stream.reserve_capacity(wanted);
                                            let Some(Ok(capacity)) = futures::future::poll_fn(|cx| stream.poll_capacity(cx)).await else { return };
                                            let sent = capacity.min(wanted);
                                            remaining -= sent;
                                            if stream.send_data(bytes::Bytes::from(vec![42; sent]), remaining == 0).is_err() { return; }
                                            if slow { tokio::time::sleep(Duration::from_millis(25)).await; }
                                            else if matches!(kind, ResponseKind::File | ResponseKind::LargeFile) {
                                                tokio::time::sleep_until(started + Duration::from_secs_f64((length - remaining) as f64 / (32 * 1024 * 1024) as f64)).await;
                                            }
                                        }
                                    });
                                }
                                return;
                            }
                            let mut request = Vec::new();
                            loop {
                                let mut chunk = [0; 1024];
                                let Ok(read) = stream.read(&mut chunk).await else { return };
                                if read == 0 { return; }
                                request.extend_from_slice(&chunk[..read]);
                                if request.len() > 8192 { return; }
                                if request.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                            }
                            counted.fetch_add(1, Ordering::SeqCst);
                            let request = String::from_utf8(request).unwrap();
                            assert!(request.lines().any(|line| line == format!("host: {expected_host}")
                                || line.starts_with(&format!("host: {expected_host}:"))));
                            if !matches!(kind, ResponseKind::Redirect) {
                                assert!(!request.contains("authorization:"));
                                assert!(!request.contains(crate::util::fetch::DOWNLOAD_META_HEADER));
                            }
                            if matches!(kind, ResponseKind::SlowFile | ResponseKind::File) {
                                let range = request.lines().find_map(|line| line.strip_prefix("range: bytes="));
                                let start = range.map(|range| range.split_once('-').unwrap().0.parse::<usize>().unwrap()).unwrap_or(0);
                                let end = range.and_then(|range| range.split_once('-').unwrap().1.parse::<usize>().ok()).unwrap_or(1_048_575);
                                let length = end + 1 - start;
                                let headers = if range.is_some() {
                                    format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/1048576\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n")
                                } else {
                                    format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n")
                                };
                                if stream.write_all(headers.as_bytes()).await.is_err() { return; }
                                let mut remaining = length;
                                let started = tokio::time::Instant::now();
                                while remaining > 0 {
                                    let chunk = remaining.min(if matches!(kind, ResponseKind::SlowFile) { 1024 } else { 64 * 1024 });
                                    if stream.write_all(&vec![42; chunk]).await.is_err() { return; }
                                    remaining -= chunk;
                                    if matches!(kind, ResponseKind::SlowFile) { tokio::time::sleep(Duration::from_millis(25)).await; }
                                    else {
                                        tokio::time::sleep_until(started + Duration::from_secs_f64((length - remaining) as f64 / (32 * 1024 * 1024) as f64)).await;
                                    }
                                }
                                let _ = stream.shutdown().await;
                                return;
                            }
                            let range = request.lines().find_map(|line| line.strip_prefix("range: bytes=")).unwrap();
                            let (start, end) = range.trim().split_once('-').unwrap();
                            let start = start.parse::<u64>().unwrap();
                            let end = end.parse::<u64>().unwrap();
                            let length = end + 1 - start;
                            let header = match kind {
                                ResponseKind::IgnoreRange => "HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n".to_string(),
                                ResponseKind::Redirect => "HTTP/1.1 302 Found\r\nLocation: https://different.invalid/file\r\nContent-Length: 0\r\n\r\n".to_string(),
                                ResponseKind::RateLimited => "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 30\r\nContent-Length: 0\r\n\r\n".to_string(),
                                ResponseKind::Missing => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string(),
                                _ => format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {length}\r\n{}X-Served-By: cache-test\r\nX-Cache: HIT\r\nConnection: close\r\n\r\n",
                                    if matches!(kind, ResponseKind::WrongTotal) { 999_999 } else { 1_048_576 },
                                    if matches!(kind, ResponseKind::Encoded) { "Content-Encoding: gzip\r\n" } else { "" }),
                            };
                            if stream.write_all(header.as_bytes()).await.is_err() { return; }
                            if matches!(kind, ResponseKind::IgnoreRange | ResponseKind::Stall) {
                                std::future::pending::<()>().await;
                            }
                            let length = if matches!(kind, ResponseKind::Truncated) { 1024 } else { length as usize };
                            let _ = stream.write_all(&vec![42; length]).await;
                            let _ = stream.shutdown().await;
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => { result.unwrap().unwrap(); }
                }
            }
        });
        Self {
            rustls_root: ca.der().clone(),
            address,
            root,
            requests,
            server,
            protocol,
        }
    }

    fn session(&self, trust_fixture: bool) -> ProbeSession {
        let target = ProbeTarget {
            url: url::Url::parse(&format!(
                "https://cdn.modrinth.com:{}/data/project/file.jar",
                self.address.port()
            ))
            .unwrap(),
            size: 1_048_576,
            region: 0..1_048_576,
            protocol: self.protocol,
        };
        let candidate = EndpointCandidate {
            ip: self.address.ip(),
            source: CandidateSource::History,
            expires_at: Instant::now() + Duration::from_secs(60),
        };
        let route = DownloadRoute {
            url: target.url.to_string(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let mut builder = probe_client_builder(&candidate, &target);
        if trust_fixture {
            builder = builder.add_root_certificate(self.root.clone());
        }
        ProbeSession::with_client(
            candidate,
            target,
            builder.build().unwrap(),
            crate::util::download::native_budget::try_acquire(&route).unwrap(),
            Arc::new(AtomicU64::new(0)),
            Arc::default(),
        )
    }
}

fn production_clients(
    fixture: &TlsFixture,
) -> crate::util::fetch::DownloadClients {
    let mut clients = crate::util::fetch::DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: crate::util::proxy::ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    clients.scope = format!("tls-fixture-{}", fixture.address.port());
    clients.endpoints =
        crate::util::download::endpoint_selection::EndpointSelector::with_mode(
            clients.scope.clone(),
            crate::util::download::endpoint_selection::SelectionMode::Auto,
        );
    clients
        .endpoints
        .pinned_clients
        .trust_test_root(fixture.root.clone());
    clients
        .endpoints
        .install_test_selection(fixture.address, fixture.protocol);
    clients
        .system_dns
        .set_test_addresses("cdn.modrinth.com", vec![fixture.address]);
    clients
        .direct_dns
        .set_test_addresses("cdn.modrinth.com", vec![fixture.address]);
    clients
}

fn fixture_route() -> DownloadRoute {
    DownloadRoute {
        url: "https://cdn.modrinth.com/data/project/file.jar".into(),
        source: DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::Direct,
    }
}

#[tokio::test]
async fn pinned_redirect_uses_other_hosts_normal_client_and_removes_authorization()
 {
    let origin =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::Redirect).await;
    let other = TlsFixture::start_on(
        "different.invalid",
        ResponseKind::Valid,
        Protocol::Http1,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let mut clients = production_clients(&origin);
    let fallback = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .http1_only()
        .add_root_certificate(other.root.clone())
        .resolve("different.invalid", other.address)
        .build()
        .unwrap();
    clients.direct = fallback.clone();
    clients.http1_direct = fallback;
    let (response, _) =
        crate::util::download::native_request::send_path_request_with_clients(
            &fixture_route(),
            Some(&("Authorization".into(), "secret-token".into())),
            None,
            None,
            Some(0),
            Some(65535),
            &clients,
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.remote_addr(), Some(other.address));
    assert_eq!(response.url().host_str(), Some("different.invalid"));
    assert_eq!(response.bytes().await.unwrap().len(), 65536);
    assert_eq!(origin.requests.load(Ordering::SeqCst), 1);
    assert_eq!(other.requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn native_h2_tries_verified_backup_after_primary_tcp_accepts_but_tls_fails()
 {
    use crate::util::download::{h2_pool, proxy_context};
    let invalid = TlsFixture::start_protocol(
        "wrong-host.invalid",
        ResponseKind::Valid,
        Protocol::H2,
    )
    .await;
    let valid = TlsFixture::start_on_port(
        "cdn.modrinth.com",
        ResponseKind::Valid,
        Protocol::H2,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
        invalid.address.port(),
    )
    .await;
    let clients = production_clients(&invalid);
    clients.endpoints.install_test_backup(valid.address.ip());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(invalid.rustls_root.clone()).unwrap();
    roots.add(valid.rustls_root.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let mut route = fixture_route();
    route.proxy = ProxyPolicy::System;
    proxy_context::with_snapshot(
        clients,
        h2_pool::with_test_tls_config(Arc::new(config), async {
            let connection = tokio::time::timeout(
                Duration::from_secs(3),
                h2_pool::shared_connection(&route, true, true, false),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(connection.remote_addr, Some(valid.address));
            assert_eq!(
                connection
                    .endpoint_lease
                    .as_ref()
                    .unwrap()
                    .pool
                    .endpoint
                    .ip(),
                valid.address.ip()
            );
            connection.retire();
        }),
    )
    .await;
}

#[tokio::test]
async fn automatic_trial_measures_candidates_with_only_one_recovery_slot_and_resumes_real_https_file()
 {
    use crate::util::download::{native_slow, proxy_context};
    let slow =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::SlowFile).await;
    let fast = TlsFixture::start_on(
        "cdn.modrinth.com",
        ResponseKind::File,
        Protocol::Http1,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let mut clients = production_clients(&slow);
    clients.scope = format!("automatic-trial-{}", slow.address.port());
    clients
        .endpoints
        .pinned_clients
        .trust_test_root(fast.root.clone());
    *clients.endpoints.probe_fixture.lock() =
        Some(super::super::coordinator::ProbeFixture {
            metered: false,
            candidates: vec![
                (slow.address, slow.root.clone()),
                (fast.address, fast.root.clone()),
            ],
        });
    let selector = clients.endpoints.clone();
    let data = vec![42; 1_048_576];
    let request = crate::util::fetch::DownloadRequest::new(
        fixture_route().url,
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1(
            sha1_smol::Sha1::from(&data[..]).hexdigest(),
        )
        .with_size(data.len() as u64),
    )
    .with_segmented_download(false);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("automatic.jar");
    let semaphore =
        crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(4));
    let transfer = proxy_context::with_snapshot(clients, async {
        let mut occupied_route = fixture_route();
        occupied_route.proxy = ProxyPolicy::System;
        let limit =
            crate::util::download::constants::MAX_CONNECTIONS_PER_AUTHORITY;
        let _occupied: Vec<_> = (0..limit - 2)
            .map(|_| {
                crate::util::download::native_budget::try_acquire(
                    &occupied_route,
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            crate::util::download::native_budget::available(&occupied_route),
            2
        );
        for proxy in [ProxyPolicy::System, ProxyPolicy::Direct] {
            crate::util::fetch::record_authority_h2_failure_for(
                "cdn.modrinth.com:443",
                proxy,
            );
        }
        native_slow::with_test_timing(
            Duration::from_millis(75),
            Duration::from_millis(150),
            crate::util::fetch::download_endpoint_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        )
        .await
    });
    let result = tokio::time::timeout(Duration::from_secs(20), transfer)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{error}; {} counters={:?}",
                selector.trial_diagnostic(),
                request.transfer_counters.snapshot()
            )
        })
        .unwrap();
    assert_eq!(result.attempts, 2);
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), data);
    let lease = selector
        .lease(&fixture_route().url.parse().unwrap(), Protocol::Http1)
        .unwrap();
    assert_eq!(lease.pool.endpoint, fast.address);
    assert_eq!(request.endpoint_migrations.load(Ordering::Relaxed), 1);
    assert_eq!(slow.requests.load(Ordering::SeqCst), 1);
    assert!(fast.requests.load(Ordering::SeqCst) >= 4);
    assert_eq!(
        request.transfer_counters.snapshot().received_payload_bytes,
        data.len() as u64
    );
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        data.len() as u64
    );
}

#[tokio::test]
async fn production_reqwest_requests_and_retries_follow_new_selection_without_dns_override()
 {
    let first =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::Valid).await;
    let second = TlsFixture::start_on(
        "cdn.modrinth.com",
        ResponseKind::Valid,
        Protocol::Http1,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let clients = production_clients(&first);
    clients
        .endpoints
        .pinned_clients
        .trust_test_root(second.root.clone());
    let route = fixture_route();
    for expected in [first.address, second.address] {
        clients
            .endpoints
            .install_test_selection(expected, Protocol::Http1);
        let (response, _) = crate::util::download::native_request::send_path_request_with_clients(
            &route, None, None, None, Some(0), Some(65535), &clients, None).await.unwrap();
        assert_eq!(response.remote_addr(), Some(expected));
        assert_eq!(response.bytes().await.unwrap().len(), 65536);
    }
    assert_eq!(first.requests.load(Ordering::SeqCst), 1);
    assert_eq!(second.requests.load(Ordering::SeqCst), 1);
    assert!(
        clients
            .direct_dns
            .host_override("cdn.modrinth.com")
            .is_none()
    );
    assert!(
        clients
            .direct_dns
            .resolved_addresses("cdn.modrinth.com")
            .is_empty()
    );
    let response =
        crate::util::download::native_request::send_cdn_request_with_clients(
            clients
                .direct
                .get(&route.url)
                .header(header::RANGE, "bytes=0-65535"),
            &clients,
            route.proxy,
        )
        .await
        .unwrap();
    let limit = crate::util::download::constants::MAX_CONNECTIONS_PER_AUTHORITY;
    assert_eq!(
        crate::util::download::native_budget::available(&route),
        limit - 1
    );
    let budget = response
        .extensions()
        .get::<crate::util::download::native_request::ResponseBudget>()
        .cloned()
        .unwrap();
    assert_eq!(response.remote_addr(), Some(second.address));
    assert_eq!(response.bytes().await.unwrap().len(), 65536);
    assert_eq!(
        crate::util::download::native_budget::available(&route),
        limit - 1
    );
    drop(budget);
    assert_eq!(
        crate::util::download::native_budget::available(&route),
        limit
    );
}

#[tokio::test]
async fn native_h2_new_selection_retires_old_pool_and_preserves_authority_budget()
 {
    use crate::util::download::{h2_pool, native_budget, proxy_context};
    let first = TlsFixture::start_protocol(
        "cdn.modrinth.com",
        ResponseKind::Valid,
        Protocol::H2,
    )
    .await;
    let second = TlsFixture::start_on(
        "cdn.modrinth.com",
        ResponseKind::Valid,
        Protocol::H2,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let clients = production_clients(&first);
    let selector = clients.endpoints.clone();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(first.rustls_root.clone()).unwrap();
    roots.add(second.rustls_root.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let mut route = fixture_route();
    route.proxy = ProxyPolicy::System;
    proxy_context::with_snapshot(
        clients,
        h2_pool::with_test_tls_config(Arc::new(config), async {
            let initial_budget = native_budget::available(&route);
            let initial_key = proxy_context::authority_key(
                "cdn.modrinth.com:443",
                route.proxy,
            );
            let old = h2_pool::shared_connection(&route, true, true, false)
                .await
                .unwrap();
            assert_eq!(old.remote_addr, Some(first.address));
            let active = old.track_stream();
            let held = h2_pool::hold_slot_for_test(&route).await;
            let waiting = h2_pool::shared_connection(&route, true, true, false);
            tokio::pin!(waiting);
            assert!(futures::poll!(waiting.as_mut()).is_pending());
            selector.install_test_selection(second.address, Protocol::H2);
            assert!(!h2_pool::has_live_connection(&route).await);
            drop(held);
            let new = waiting.await.unwrap();
            assert_eq!(new.remote_addr, Some(second.address));
            assert_ne!(old.connection_id, new.connection_id);
            assert!(old.is_retiring());
            assert!(!old.is_dead());
            assert_eq!(
                initial_key,
                proxy_context::authority_key(
                    "cdn.modrinth.com:443",
                    route.proxy
                )
            );
            assert_eq!(native_budget::available(&route), initial_budget - 2);
            drop(active);
            new.retire();
            tokio::time::timeout(Duration::from_secs(2), async {
                while !old.is_dead() || !new.is_dead() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(native_budget::available(&route), initial_budget);
        }),
    )
    .await;
}

#[tokio::test]
async fn legacy_memory_cdn_fetch_holds_physical_budget_until_body_or_cancellation()
 {
    use crate::util::download::{native_budget, proxy_context};
    let fixture =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::SlowFile).await;
    let clients = production_clients(&fixture);
    let audit_clients = clients.clone();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let semaphore =
        crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(4));
    proxy_context::with_snapshot(clients, async {
        let route = fixture_route();
        let mut route = route;
        route.proxy = ProxyPolicy::System;
        let initial = native_budget::available(&route);
        let fetch = crate::util::fetch::fetch_official(&route.url, None, None, None, &semaphore, &pool);
        tokio::pin!(fetch);
        tokio::select! {
            result = &mut fetch => panic!("slow body completed before cancellation: {result:?}"),
            _ = async {
                tokio::time::timeout(Duration::from_secs(2), async {
                    while fixture.requests.load(Ordering::Relaxed) == 0 {
                        tokio::task::yield_now().await;
                    }
                }).await.unwrap();
                tokio::time::sleep(Duration::from_millis(150)).await;
                assert_eq!(native_budget::available(&route), initial - 1);
                assert_eq!(semaphore.0.available_permits(), 3);
            } => {}
        }
    }).await;
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 1);
    assert_eq!(semaphore.0.available_permits(), 4);
    proxy_context::with_snapshot(audit_clients, async {
        let mut route = fixture_route();
        route.proxy = ProxyPolicy::System;
        assert_eq!(
            native_budget::available(&route),
            crate::util::download::constants::MAX_CONNECTIONS_PER_AUTHORITY
        );
    })
    .await;
}

#[tokio::test]
async fn same_url_http1_download_resumes_on_new_ip_and_verifies_final_hash() {
    use crate::util::download::{native_slow, proxy_context};
    let slow =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::SlowFile).await;
    let fast = TlsFixture::start_on(
        "cdn.modrinth.com",
        ResponseKind::File,
        Protocol::Http1,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let mut clients = production_clients(&slow);
    clients.scope = format!("http1-recovery-{}", slow.address.port());
    clients
        .endpoints
        .pinned_clients
        .trust_test_root(fast.root.clone());
    let selector = clients.endpoints.clone();
    let bytes = vec![42; 1_048_576];
    let request = crate::util::fetch::DownloadRequest::new(
        fixture_route().url,
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1(
            sha1_smol::Sha1::from(&bytes[..]).hexdigest(),
        )
        .with_size(bytes.len() as u64),
    )
    .with_segmented_download(false);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("verified.jar");
    let semaphore =
        crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(4));
    let publish = async {
        tokio::time::sleep(Duration::from_millis(450)).await;
        selector.install_test_selection(fast.address, Protocol::Http1);
    };
    let transfer = proxy_context::with_snapshot(clients, async {
        // Select the HTTP/1 fixture path before the production entry
        // evaluates native H2, so this test cannot access public DNS.
        for proxy in [ProxyPolicy::System, ProxyPolicy::Direct] {
            crate::util::fetch::record_authority_h2_failure_for(
                "cdn.modrinth.com:443",
                proxy,
            );
        }
        native_slow::with_test_timing(
            Duration::from_millis(75),
            Duration::from_millis(150),
            crate::util::fetch::download_endpoint_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        )
        .await
    });
    let (result, _) = tokio::time::timeout(Duration::from_secs(8), async {
        tokio::join!(transfer, publish)
    })
    .await
    .unwrap();
    let result = result.unwrap();
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), bytes);
    assert_eq!(result.attempts, 2);
    assert_eq!(slow.requests.load(Ordering::SeqCst), 1);
    assert_eq!(fast.requests.load(Ordering::SeqCst), 1);
    assert_eq!(request.endpoint_migrations.load(Ordering::Relaxed), 1);
    let counters = request.transfer_counters.snapshot();
    assert_eq!(counters.committed_unique_bytes, bytes.len() as u64);
    assert_eq!(counters.received_payload_bytes, bytes.len() as u64);
}

async fn automatic_native_h2_recovery(range_count: Option<usize>) {
    use crate::util::download::{h2_pool, native_slow, proxy_context};
    let slow = TlsFixture::start_protocol(
        "cdn.modrinth.com",
        if range_count.is_some() {
            ResponseKind::LargeSlowFile
        } else {
            ResponseKind::SlowFile
        },
        Protocol::H2,
    )
    .await;
    let fast = TlsFixture::start_on(
        "cdn.modrinth.com",
        if range_count.is_some() {
            ResponseKind::LargeFile
        } else {
            ResponseKind::File
        },
        Protocol::H2,
        false,
        Ipv4Addr::new(127, 0, 0, 2),
    )
    .await;
    let clients = production_clients(&slow);
    clients
        .endpoints
        .pinned_clients
        .trust_test_root(fast.root.clone());
    *clients.endpoints.probe_fixture.lock() =
        Some(super::super::coordinator::ProbeFixture {
            metered: false,
            candidates: vec![
                (slow.address, slow.root.clone()),
                (fast.address, fast.root.clone()),
            ],
        });
    let selector = clients.endpoints.clone();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(slow.rustls_root.clone()).unwrap();
    roots.add(fast.rustls_root.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let data = vec![
        42;
        if range_count.is_some() {
            64 * 1024 * 1024
        } else {
            1_048_576
        }
    ];
    let mut request = crate::util::fetch::DownloadRequest::new(
        fixture_route().url,
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1(
            sha1_smol::Sha1::from(&data[..]).hexdigest(),
        )
        .with_size(data.len() as u64),
    )
    .with_http1_segmented_download(false);
    request.h2_range_concurrency = range_count;
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("native-h2.jar");
    let semaphore =
        crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(8));
    let transfer = proxy_context::with_snapshot(
        clients,
        h2_pool::with_test_tls_config(Arc::new(config), async {
            let mut route = fixture_route();
            route.proxy = ProxyPolicy::System;
            let initial = h2_pool::shared_connection(&route, true, true, false)
                .await
                .unwrap();
            assert_eq!(initial.remote_addr, Some(slow.address));
            let result = native_slow::with_test_timing(
                Duration::from_millis(75),
                Duration::from_millis(150),
                crate::util::fetch::download_endpoint_fixture(
                    request.clone(),
                    &destination,
                    &semaphore,
                ),
            )
            .await;
            assert!(
                initial.bytes_transferred_for_scheduler() > 0,
                "production must consume the native H2 pool rather than only reqwest H2"
            );
            if range_count.is_some() {
                let selected = h2_pool::shared_batch_connection(&route, true)
                    .await
                    .unwrap();
                assert_eq!(selected.remote_addr, Some(fast.address));
                assert!(
                    selected.bytes_transferred_for_scheduler() > 0,
                    "unfinished range must consume native H2 on the new peer"
                );
            }
            result
        }),
    );
    let result = tokio::time::timeout(Duration::from_secs(15), transfer)
        .await
        .unwrap_or_else(|error| {
            panic!("{error}; {}", selector.trial_diagnostic())
        })
        .unwrap();
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), data);
    assert_eq!(result.size, data.len() as u64);
    assert_eq!(request.endpoint_migrations.load(Ordering::Relaxed), 1);
    let counters = request.transfer_counters.snapshot();
    assert_eq!(counters.received_payload_bytes, data.len() as u64);
    assert_eq!(counters.committed_unique_bytes, data.len() as u64);
    let selected = selector
        .lease(&fixture_route().url.parse().unwrap(), Protocol::H2)
        .unwrap();
    assert_eq!(selected.pool.endpoint, fast.address);
    assert!(fast.requests.load(Ordering::Relaxed) >= 4);
    selector.close();
}

#[tokio::test]
async fn native_h2_single_stream_measures_new_peer_and_resumes_verified_file() {
    automatic_native_h2_recovery(None).await;
}

#[tokio::test]
async fn native_h2_ranges_measure_new_peer_and_resume_verified_file() {
    automatic_native_h2_recovery(Some(2)).await;
}

#[tokio::test]
async fn native_h2_status_preserves_authority_retry_after_before_endpoint_recovery()
 {
    use crate::util::download::{
        h2_download, h2_pool, native, native_breaker, proxy_context,
    };
    let fixture = TlsFixture::start_protocol(
        "cdn.modrinth.com",
        ResponseKind::RateLimited,
        Protocol::H2,
    )
    .await;
    let clients = production_clients(&fixture);
    let selector = clients.endpoints.clone();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.rustls_root.clone()).unwrap();
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("rate-limited.jar");
    let part = directory.path().join("rate-limited.part");
    let semaphore =
        crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(4));
    let mut route = fixture_route();
    route.proxy = ProxyPolicy::System;
    let request = crate::util::fetch::DownloadRequest::new(
        route.url.clone(),
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1("hash").with_size(1_048_576),
    );
    proxy_context::with_snapshot(
        clients,
        h2_pool::with_test_tls_config(Arc::new(config), async {
            for _ in 0..3 {
                let outcome = h2_download::try_download_via_h2(
                    &request,
                    &route,
                    &destination,
                    &part,
                    native::NativeH2Policy {
                        allow_cold_connection: true,
                        abort_if_slow: true,
                        expected_speed: None,
                    },
                    &semaphore,
                )
                .await;
                assert!(matches!(
                    outcome,
                    h2_download::H2DownloadOutcome::Fallback {
                        failure: h2_download::H2DownloadFailure::Http,
                        ..
                    }
                ));
            }
            assert!(native_breaker::is_open(&route));
            assert!(
                selector
                    .recovery_lease(
                        &request,
                        &route,
                        Protocol::H2,
                        Some("127.0.0.2:443".parse().unwrap()),
                        1,
                        1_048_576
                    )
                    .is_none()
            );
            let connection =
                h2_pool::shared_connection(&route, true, true, false)
                    .await
                    .unwrap();
            assert!(!connection.is_retiring());
            connection.retire();
        }),
    )
    .await;
    assert_eq!(fixture.requests.load(Ordering::Relaxed), 3);
    assert!(!destination.exists());
    selector.close();
}

#[tokio::test]
async fn validates_real_tls_host_sni_pinned_peer_and_exact_range() {
    let fixture =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::Valid).await;
    let session = fixture.session(true);
    let result = session
        .validate_endpoint(
            &Mutex::new(ProbeBudget::new(Instant::now(), policy::ROUND_BYTES)),
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert_eq!(result.remote_addr, fixture.address);
    assert_eq!(result.sample.bytes, policy::VALIDATION_BYTES);
    assert_eq!(result.x_served_by, "cache-test");
    assert_eq!(result.cache_status, "HIT");
    assert!(result.ttfb < Duration::from_secs(2));
    assert_eq!(fixture.requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn validates_real_h2_tls_alpn_peer_and_bounded_payload() {
    let fixture = TlsFixture::start_protocol(
        "cdn.modrinth.com",
        ResponseKind::Valid,
        Protocol::H2,
    )
    .await;
    let session = fixture.session(true);
    let result = session
        .validate_endpoint(
            &Mutex::new(ProbeBudget::new(Instant::now(), policy::ROUND_BYTES)),
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert_eq!(result.remote_addr, fixture.address);
    assert_eq!(result.sample.protocol, Protocol::H2);
    assert_eq!(result.sample.bytes, policy::VALIDATION_BYTES);
    assert_eq!(result.x_served_by, "cache-h2-test");
    assert_eq!(
        session.bytes_received.load(Ordering::Relaxed),
        policy::VALIDATION_BYTES
    );
}

#[tokio::test]
async fn probes_unfinished_region_with_exact_nonzero_bounds_over_http1_and_h2()
{
    for protocol in [Protocol::Http1, Protocol::H2] {
        let fixture = TlsFixture::start_protocol(
            "cdn.modrinth.com",
            ResponseKind::Valid,
            protocol,
        )
        .await;
        let mut session = fixture.session(true);
        session.target.region = 983_040..1_048_576;
        let result = session
            .validate_endpoint(
                &Mutex::new(ProbeBudget::new(
                    Instant::now(),
                    policy::ROUND_BYTES,
                )),
                &CancellationToken::new(),
                Instant::now() + Duration::from_secs(2),
            )
            .await
            .unwrap();
        assert_eq!(result.sample.bytes, 65_536);
        assert!(result.sample.complete_object);
        assert_eq!(result.remote_addr, fixture.address);
    }
}

#[test]
fn nonzero_range_validation_rejects_prefix_wrong_end_and_overflow() {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONTENT_RANGE,
        "bytes 983040-1048575/1048576".parse().unwrap(),
    );
    headers.insert(header::CONTENT_LENGTH, "65536".parse().unwrap());
    assert_eq!(
        validate_headers(&headers, 983_040, 65_536, 1_048_576),
        Ok(())
    );
    assert_eq!(
        validate_headers(&headers, 0, 65_536, 1_048_576),
        Err(ProbeFailure::InvalidRange)
    );
    assert_eq!(
        validate_headers(&headers, 983_040, 65_535, 1_048_576),
        Err(ProbeFailure::InvalidRange)
    );
    assert_eq!(
        validate_headers(&headers, u64::MAX, 2, u64::MAX),
        Err(ProbeFailure::InvalidRange)
    );
}

#[tokio::test]
async fn wrong_hostname_and_untrusted_ca_fail_before_any_http_request() {
    for (host, trust, expired) in [
        ("different.invalid", true, false),
        ("cdn.modrinth.com", false, false),
        ("cdn.modrinth.com", true, true),
    ] {
        let fixture = TlsFixture::start_certificate(
            host,
            ResponseKind::Valid,
            Protocol::Http1,
            expired,
        )
        .await;
        let session = fixture.session(trust);
        let result = session
            .validate_endpoint(
                &Mutex::new(ProbeBudget::new(
                    Instant::now(),
                    policy::ROUND_BYTES,
                )),
                &CancellationToken::new(),
                Instant::now() + Duration::from_secs(2),
            )
            .await;
        assert_eq!(result.unwrap_err(), ProbeFailure::InvalidCertificate);
        assert_eq!(fixture.requests.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn invalid_ranges_encoding_redirect_status_and_ignored_range_never_validate()
 {
    for (kind, expected) in [
        (ResponseKind::WrongTotal, ProbeFailure::InvalidRange),
        (ResponseKind::Encoded, ProbeFailure::InvalidEncoding),
        (ResponseKind::IgnoreRange, ProbeFailure::RangeUnsupported),
        (ResponseKind::Redirect, ProbeFailure::Redirect),
        (
            ResponseKind::RateLimited,
            ProbeFailure::ServerCooldown {
                status: 429,
                retry_after: Duration::from_secs(30),
            },
        ),
        (ResponseKind::Missing, ProbeFailure::TargetMissing),
        (ResponseKind::Truncated, ProbeFailure::Truncated),
    ] {
        let fixture = TlsFixture::start("cdn.modrinth.com", kind).await;
        let session = fixture.session(true);
        let result = session
            .validate_endpoint(
                &Mutex::new(ProbeBudget::new(
                    Instant::now(),
                    policy::ROUND_BYTES,
                )),
                &CancellationToken::new(),
                Instant::now() + Duration::from_secs(2),
            )
            .await;
        assert_eq!(result.unwrap_err(), expected);
        assert!(
            session.bytes_received.load(Ordering::Relaxed)
                <= policy::VALIDATION_BYTES
        );
        assert_eq!(fixture.requests.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn stalled_body_and_cancelled_probe_have_bounded_lifetimes() {
    let fixture =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::Stall).await;
    let session = fixture.session(true);
    let budget =
        Mutex::new(ProbeBudget::new(Instant::now(), policy::ROUND_BYTES));
    let result = session
        .validate_endpoint(
            &budget,
            &CancellationToken::new(),
            Instant::now() + Duration::from_millis(100),
        )
        .await;
    assert_eq!(result.unwrap_err(), ProbeFailure::Timeout);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let result = session
        .validate_endpoint(
            &budget,
            &cancellation,
            Instant::now() + Duration::from_secs(1),
        )
        .await;
    assert_eq!(result.unwrap_err(), ProbeFailure::Cancelled);
}

#[tokio::test]
async fn throughput_sample_crossing_another_tuning_trial_is_contaminated() {
    let fixture =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::SlowFile).await;
    let session = fixture.session(true);
    let budget =
        Mutex::new(ProbeBudget::new(Instant::now(), policy::ROUND_BYTES));
    let cancellation = CancellationToken::new();
    let sample = session.sample_endpoint(
        &budget,
        &cancellation,
        Instant::now() + Duration::from_secs(1),
    );
    let concurrent_tuning = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let lease = session.trials.acquire(
            crate::util::download::endpoint_selection::tuning::TrialKind::Files,
            Instant::now(), Duration::from_secs(2),
        ).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(lease);
    };
    let (result, _) = tokio::join!(sample, concurrent_tuning);
    let sample = result.unwrap().sample;
    assert!(sample.bytes > 0);
    assert!(
        sample.contaminated,
        "trial revision changes invalidate the complete measurement window"
    );
}

#[test]
fn header_checks_reject_wrong_bounds_unknown_total_short_or_missing_length() {
    for range in [
        "bytes 1-65536/1048576",
        "bytes 0-65534/1048576",
        "bytes 0-65535/*",
        "garbage",
    ] {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_RANGE, range.parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "65536".parse().unwrap());
        assert_eq!(
            validate_headers(&headers, 0, 65536, 1048576),
            Err(ProbeFailure::InvalidRange)
        );
    }
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONTENT_RANGE,
        "bytes 0-65535/1048576".parse().unwrap(),
    );
    for length in [None, Some("1"), Some("invalid")] {
        headers.remove(header::CONTENT_LENGTH);
        if let Some(length) = length {
            headers.insert(header::CONTENT_LENGTH, length.parse().unwrap());
        }
        assert_eq!(
            validate_headers(&headers, 0, 65536, 1048576),
            Err(ProbeFailure::InvalidRange)
        );
    }
}

#[tokio::test]
async fn empty_probe_budget_sends_no_request_and_counts_no_payload() {
    let fixture =
        TlsFixture::start("cdn.modrinth.com", ResponseKind::Valid).await;
    let session = fixture.session(true);
    let result = session
        .validate_endpoint(
            &Mutex::new(ProbeBudget::new(Instant::now(), 0)),
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(2),
        )
        .await;
    assert_eq!(result.unwrap_err(), ProbeFailure::BudgetExhausted);
    assert_eq!(fixture.requests.load(Ordering::SeqCst), 0);
    assert_eq!(session.bytes_received.load(Ordering::Relaxed), 0);
}

#[test]
fn production_probe_rejects_custom_proxy_insecure_tls_nonpublic_and_expired_candidates()
 {
    use crate::util::proxy::{ProxyConfig, ProxyMode};
    let config = ProxyConfig {
        mode: ProxyMode::Custom,
        url: "http://127.0.0.1:7890".into(),
        ..Default::default()
    };
    let clients =
        crate::util::fetch::DownloadClients::build(&config, false, false)
            .unwrap();
    let target = ProbeTarget {
        url: "https://cdn.modrinth.com/data/project/file.jar"
            .parse()
            .unwrap(),
        size: 1_048_576,
        region: 0..1_048_576,
        protocol: Protocol::H2,
    };
    let candidate = EndpointCandidate {
        ip: "151.101.1.242".parse().unwrap(),
        source: CandidateSource::Verified,
        expires_at: Instant::now() + Duration::from_secs(60),
    };
    let route = DownloadRoute {
        url: target.url.to_string(),
        source: DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    for case in 0..4 {
        let mut clients = clients.clone();
        let mut candidate = candidate.clone();
        match case {
            0 => {}
            1 => {
                clients.proxy.mode = ProxyMode::None;
                clients.ignore_ssl_errors = true;
            }
            2 => {
                clients.proxy.mode = ProxyMode::None;
                candidate.ip = "127.0.0.1".parse().unwrap();
            }
            3 => {
                clients.proxy.mode = ProxyMode::None;
                candidate.expires_at = Instant::now();
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            ProbeSession::open(
                candidate,
                target.clone(),
                &route,
                &clients,
                Arc::new(AtomicU64::new(0))
            ),
            Err(ProbeFailure::Ineligible)
        ));
    }
}
