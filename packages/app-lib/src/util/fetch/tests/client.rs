//! Fetch client tests.

use super::super::*;
use super::fixtures::*;
use crate::util::download::{h2_pool, native, proxy_context};
use crate::util::proxy::{ProxyConfig, ProxyMode};
use rustls_pki_types::PrivatePkcs8KeyDer;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct FileClientTlsFixture {
    address: SocketAddr,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for FileClientTlsFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl FileClientTlsFixture {
    async fn start() -> Self {
        let config =
            file_client_tls_config(vec![b"h2".to_vec(), b"http/1.1".to_vec()]);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((socket, _)) = incoming else { break };
                        let acceptor = acceptor.clone();
                        handlers.spawn(async move {
                            let Ok(mut stream) = acceptor.accept(socket).await else {
                                return;
                            };
                            if stream.get_ref().1.alpn_protocol() == Some(b"h2") {
                                let mut connection = h2::server::handshake(stream).await.unwrap();
                                while let Some(request) = connection.accept().await {
                                    let (request, mut respond) = request.unwrap();
                                    assert_eq!(request.version(), http::Version::HTTP_2);
                                    assert!(request.headers().get(http::header::PROXY_AUTHORIZATION).is_none());
                                    let response = http::Response::builder()
                                        .header(http::header::CONTENT_LENGTH, 2)
                                        .body(())
                                        .unwrap();
                                    respond.send_response(response, false).unwrap()
                                        .send_data(bytes::Bytes::from_static(b"ok"), true).unwrap();
                                }
                            } else {
                                let request = read_file_client_headers(&mut stream).await;
                                assert!(request.starts_with("GET /file HTTP/1.1\r\n"));
                                assert!(!request.to_ascii_lowercase().contains("proxy-authorization:"));
                                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                                    .await.unwrap();
                            }
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        Self { address, server }
    }

    fn url(&self) -> String {
        format!("https://{}/file", self.address)
    }
}

fn file_client_tls_config(
    alpn_protocols: Vec<Vec<u8>>,
) -> rustls::ServerConfig {
    let key = rcgen::KeyPair::generate().unwrap();
    let certificate = rcgen::CertificateParams::new(vec!["127.0.0.1".into()])
        .unwrap()
        .self_signed(&key)
        .unwrap();
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.der().clone()],
            PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
        .unwrap();
    config.alpn_protocols = alpn_protocols;
    config
}

trait FileClientFixtureIo:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>
    FileClientFixtureIo for T
{
}

async fn read_file_client_headers(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> String {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        assert!(headers.len() < 16 * 1024);
        headers.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(headers).unwrap()
}

struct FileClientProxyFixture {
    url: String,
    connections: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for FileClientProxyFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl FileClientProxyFixture {
    async fn start_socks5h(target: SocketAddr) -> Self {
        Self::start_socks5(
            target,
            "socks5h",
            "native-h2-origin.invalid",
            "pass",
        )
        .await
    }

    async fn start_socks5(
        target: SocketAddr,
        scheme: &str,
        host: &str,
        password: &str,
    ) -> Self {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("{scheme}://{}", listener.local_addr().unwrap());
        let expected_host = host.to_string();
        let expected_password = password.to_string();
        let connections = Arc::new(AtomicUsize::new(0));
        let counted = connections.clone();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((mut socket, _)) = incoming else { break };
                        let counted = counted.clone();
                        let expected_host = expected_host.clone();
                        let expected_password = expected_password.clone();
                        handlers.spawn(async move {
                            assert_eq!(socket.read_u8().await.unwrap(), 5);
                            let mut methods = vec![0; socket.read_u8().await.unwrap() as usize];
                            socket.read_exact(&mut methods).await.unwrap();
                            assert!(methods.contains(&2));
                            socket.write_all(&[5, 2]).await.unwrap();
                            assert_eq!(socket.read_u8().await.unwrap(), 1);
                            let mut username = vec![0; socket.read_u8().await.unwrap() as usize];
                            socket.read_exact(&mut username).await.unwrap();
                            let mut password = vec![0; socket.read_u8().await.unwrap() as usize];
                            socket.read_exact(&mut password).await.unwrap();
                            assert_eq!(username, b"user");
                            assert_eq!(password, expected_password.as_bytes());
                            socket.write_all(&[1, 0]).await.unwrap();
                            let mut request = [0; 4];
                            socket.read_exact(&mut request).await.unwrap();
                            assert_eq!(&request[..3], &[5, 1, 0]);
                            match expected_host.parse::<std::net::IpAddr>() {
                                Ok(std::net::IpAddr::V4(expected)) => {
                                    assert_eq!(request[3], 1, "IPv4 target requires a SOCKS IPv4 address");
                                    let mut address = [0; 4];
                                    socket.read_exact(&mut address).await.unwrap();
                                    assert_eq!(address, expected.octets());
                                }
                                Ok(std::net::IpAddr::V6(expected)) => {
                                    assert_eq!(request[3], 4, "IPv6 target requires a SOCKS IPv6 address");
                                    let mut address = [0; 16];
                                    socket.read_exact(&mut address).await.unwrap();
                                    assert_eq!(address, expected.octets());
                                }
                                Err(_) => {
                                    assert_eq!(request[3], 3, "SOCKS5h must send the unresolved hostname");
                                    let mut host = vec![0; socket.read_u8().await.unwrap() as usize];
                                    socket.read_exact(&mut host).await.unwrap();
                                    assert_eq!(host, expected_host.as_bytes());
                                }
                            }
                            assert_eq!(socket.read_u16().await.unwrap(), target.port());
                            counted.fetch_add(1, Ordering::SeqCst);
                            let mut remote = tokio::net::TcpStream::connect(target).await.unwrap();
                            socket.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await.unwrap();
                            let _ = tokio::io::copy_bidirectional(&mut socket, &mut remote).await;
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        Self {
            url,
            connections,
            server,
        }
    }

    async fn start(target: SocketAddr) -> Self {
        Self::start_connect(target, false).await
    }

    async fn start_https(target: SocketAddr) -> Self {
        Self::start_connect(target, true).await
    }

    async fn start_connect(target: SocketAddr, secure: bool) -> Self {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let scheme = if secure { "https" } else { "http" };
        let url = format!("{scheme}://{}", listener.local_addr().unwrap());
        let acceptor = secure.then(|| {
            tokio_rustls::TlsAcceptor::from(Arc::new(file_client_tls_config(
                vec![b"http/1.1".to_vec()],
            )))
        });
        let connections = Arc::new(AtomicUsize::new(0));
        let counted = connections.clone();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let Ok((socket, _)) = incoming else { break };
                        let counted = counted.clone();
                        let acceptor = acceptor.clone();
                        handlers.spawn(async move {
                            let mut socket: Box<dyn FileClientFixtureIo> = if let Some(acceptor) = acceptor {
                                let Ok(stream) = acceptor.accept(socket).await else { return };
                                assert!(stream.get_ref().1.alpn_protocol().is_none_or(|protocol| protocol == b"http/1.1"));
                                Box::new(stream)
                            } else { Box::new(socket) };
                            let headers = read_file_client_headers(&mut socket).await;
                            assert!(headers.starts_with(&format!("CONNECT {target} HTTP/1.1\r\n")));
                            assert!(headers.to_ascii_lowercase().lines()
                                .any(|line| line == "proxy-authorization: basic dxnlcjpwyxnz"));
                            counted.fetch_add(1, Ordering::SeqCst);
                            let mut remote = tokio::net::TcpStream::connect(target).await.unwrap();
                            socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.unwrap();
                            let _ = tokio::io::copy_bidirectional(&mut socket, &mut remote).await;
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        Self {
            url,
            connections,
            server,
        }
    }

    fn config(&self) -> ProxyConfig {
        ProxyConfig {
            mode: ProxyMode::Custom,
            url: self.url.clone(),
            username: "user".into(),
            password: "pass".into(),
        }
    }
}

async fn assert_file_client_protocol(
    client: &reqwest::Client,
    url: &str,
    expected: http::Version,
) {
    let response =
        tokio::time::timeout(Duration::from_secs(5), client.get(url).send())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(response.version(), expected);
    assert_eq!(response.bytes().await.unwrap(), b"ok"[..]);
}

fn native_file_route(url: String, proxy: ProxyPolicy) -> DownloadRoute {
    DownloadRoute {
        url,
        source: DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: false,
        proxy,
    }
}

async fn assert_native_file_protocol(
    route: &DownloadRoute,
) -> Arc<h2_pool::SharedH2Connection> {
    assert_eq!(native::h2_ineligible_reason(route), None);
    let connection = tokio::time::timeout(
        Duration::from_secs(5),
        h2_pool::shared_connection(route, true, true, false),
    )
    .await
    .unwrap()
    .unwrap();
    assert_native_connection_response(&connection, &route.url).await;
    connection
}

async fn assert_native_connection_response(
    connection: &h2_pool::SharedH2Connection,
    url: &str,
) {
    let _activity = connection.track_stream();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        connection.open(http::Request::get(url).body(()).unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.version(), http::Version::HTTP_2);
    let mut body = response.into_body();
    let mut received = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(chunk) = body.data().await {
            let chunk = chunk.unwrap();
            body.flow_control().release_capacity(chunk.len()).unwrap();
            received.extend_from_slice(&chunk);
        }
    })
    .await
    .unwrap();
    assert_eq!(received, b"ok");
}

#[tokio::test]
async fn ignored_ssl_errors_enable_native_http2_without_reusing_insecure_tls_in_strict_mode()
 {
    let fixture = FileClientTlsFixture::start().await;
    let proxy = ProxyConfig {
        mode: ProxyMode::None,
        ..Default::default()
    };
    let route = native_file_route(fixture.url(), ProxyPolicy::Direct);
    let insecure = DownloadClients::build(&proxy, true, false).unwrap();
    let connection = proxy_context::with_snapshot(
        insecure,
        assert_native_file_protocol(&route),
    )
    .await;
    assert_eq!(connection.remote_addr, Some(fixture.address));
    let strict = DownloadClients::build(&proxy, false, false).unwrap();
    let failure = proxy_context::with_snapshot(strict, async {
        tokio::time::timeout(
            Duration::from_secs(5),
            h2_pool::shared_batch_connection(&route, true),
        )
        .await
        .unwrap()
        .err()
        .expect("strict TLS must reject the self-signed origin")
    })
    .await;
    assert_eq!(failure.kind, h2_pool::H2ConnectFailureKind::Tls);
    assert!(!fixture.server.is_finished());
}

#[tokio::test]
async fn custom_connect_proxy_multiplexes_native_asset_batch_and_preserves_direct_bypass()
 {
    use crate::util::download::h2_download::{
        H2BatchAsset, download_asset_batch_via_h2,
    };

    let fixture = FileClientTlsFixture::start().await;
    let proxy = FileClientProxyFixture::start(fixture.address).await;
    let clients = DownloadClients::build(&proxy.config(), true, false).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut items = Vec::new();
    for index in 0..2 {
        items.push(H2BatchAsset {
            url: format!("{}?asset={index}", fixture.url()),
            destination: directory.path().join(format!("object-{index}")),
            legacy_destinations: vec![
                directory.path().join(format!("legacy/resource-{index}")),
            ],
            sha1: sha1_smol::Sha1::from(b"ok").hexdigest(),
            size: 2,
            logical_items: index + 1,
        });
    }
    let expected = items.clone();
    let completed = Arc::new(AtomicUsize::new(0));
    let logical = Arc::new(AtomicUsize::new(0));
    proxy_context::with_snapshot(clients, async {
        let route = native_file_route(fixture.url(), ProxyPolicy::System);
        assert_eq!(native::h2_ineligible_reason(&route), None);
        let connection = tokio::time::timeout(
            Duration::from_secs(5),
            h2_pool::shared_connection(&route, true, true, false),
        )
        .await
        .unwrap()
        .unwrap();
        let ((), ()) = tokio::join!(
            assert_native_connection_response(&connection, &route.url),
            assert_native_connection_response(&connection, &route.url),
        );
        assert_eq!(proxy.connections.load(Ordering::SeqCst), 1);
        assert_eq!(connection.remote_addr, None);
        assert!(connection.endpoint_lease.is_none());
        let direct = native_file_route(fixture.url(), ProxyPolicy::Direct);
        let direct_connection = assert_native_file_protocol(&direct).await;
        assert_eq!(direct_connection.remote_addr, Some(fixture.address));
        assert_eq!(proxy.connections.load(Ordering::SeqCst), 1);
        let callback_completed = completed.clone();
        let callback_logical = logical.clone();
        let unresolved = tokio::time::timeout(
            Duration::from_secs(10),
            download_asset_batch_via_h2(&route, items, 2, true, move |item| {
                let completed = callback_completed.clone();
                let logical = callback_logical.clone();
                Box::pin(async move {
                    completed.fetch_add(1, Ordering::SeqCst);
                    logical.fetch_add(
                        item.logical_items as usize,
                        Ordering::SeqCst,
                    );
                })
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(unresolved.is_empty());
        assert!(
            (1..=2).contains(&proxy.connections.load(Ordering::SeqCst)),
            "asset scheduler may expand to its initial two-connection target"
        );
        let connection = assert_native_file_protocol(&route).await;
        assert!(connection.endpoint_lease.is_none());
        assert_eq!(
            connection.remote_addr, None,
            "proxy address is not an origin endpoint"
        );
        assert!((1..=2).contains(&proxy.connections.load(Ordering::SeqCst)));
    })
    .await;
    assert_eq!(completed.load(Ordering::SeqCst), 2);
    assert_eq!(logical.load(Ordering::SeqCst), 3);
    for item in expected {
        assert_eq!(tokio::fs::read(item.destination).await.unwrap(), b"ok");
        for legacy in item.legacy_destinations {
            assert_eq!(tokio::fs::read(legacy).await.unwrap(), b"ok");
        }
    }
    assert!(!fixture.server.is_finished());
    assert!(!proxy.server.is_finished());
}

#[tokio::test]
async fn detected_system_proxy_preserves_reqwest_and_native_http2_and_no_proxy_bypass()
 {
    const CHILD_URL: &str = "AXOLOTL_H2_SYSTEM_TEST_URL";
    const CHILD_BYPASS: &str = "AXOLOTL_H2_SYSTEM_TEST_BYPASS";
    if let Ok(url) = std::env::var(CHILD_URL) {
        let bypass = std::env::var(CHILD_BYPASS).unwrap() == "true";
        let clients =
            DownloadClients::build(&ProxyConfig::default(), true, false)
                .unwrap();
        assert!(native::system_proxy_configured());
        assert_file_client_protocol(
            &clients.system,
            &url,
            http::Version::HTTP_2,
        )
        .await;
        proxy_context::with_snapshot(clients, async {
            let route = native_file_route(url, ProxyPolicy::System);
            let connection = assert_native_file_protocol(&route).await;
            assert_eq!(connection.remote_addr.is_some(), bypass);
        })
        .await;
        return;
    }
    let fixture = FileClientTlsFixture::start().await;
    let proxy = FileClientProxyFixture::start(fixture.address).await;
    let authenticated_proxy =
        proxy.url.replacen("http://", "http://user:pass@", 1);
    for bypass in [false, true] {
        let mut command =
            tokio::process::Command::new(std::env::current_exe().unwrap());
        command.arg("--exact")
            .arg("util::fetch::tests::client::detected_system_proxy_preserves_reqwest_and_native_http2_and_no_proxy_bypass")
            .arg("--nocapture")
            .env(CHILD_URL, fixture.url())
            .env(CHILD_BYPASS, bypass.to_string())
            .env_remove("REQUEST_METHOD")
            .kill_on_drop(true);
        for name in [
            "HTTP_PROXY",
            "http_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
        ] {
            command.env(name, &authenticated_proxy);
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command.env(
                name,
                if bypass {
                    "127.0.0.1"
                } else {
                    "axolotl-do-not-bypass.invalid"
                },
            );
        }
        let output =
            tokio::time::timeout(Duration::from_secs(25), command.output())
                .await
                .unwrap()
                .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(proxy.connections.load(Ordering::SeqCst), 2);
    }
    assert!(!fixture.server.is_finished());
    assert!(!proxy.server.is_finished());
}

#[tokio::test]
async fn ignored_ssl_errors_preserve_file_client_http2_negotiation() {
    let fixture = FileClientTlsFixture::start().await;
    let proxy = ProxyConfig {
        mode: ProxyMode::None,
        ..Default::default()
    };
    let clients = DownloadClients::build(&proxy, true, false).unwrap();
    for client in [&clients.system, &clients.direct] {
        assert_file_client_protocol(
            client,
            &fixture.url(),
            http::Version::HTTP_2,
        )
        .await;
    }
    let strict = DownloadClients::build(&proxy, false, false).unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(5),
            strict.system.get(fixture.url()).send()
        )
        .await
        .unwrap()
        .is_err()
    );
    assert!(!fixture.server.is_finished());
}

#[tokio::test]
async fn authenticated_socks5h_preserves_native_and_reqwest_http2_with_proxy_dns()
 {
    let fixture = FileClientTlsFixture::start().await;
    let proxy = FileClientProxyFixture::start_socks5h(fixture.address).await;
    let clients = DownloadClients::build(&proxy.config(), true, true).unwrap();
    let url = format!(
        "https://native-h2-origin.invalid:{}/file",
        fixture.address.port()
    );
    assert_file_client_protocol(&clients.system, &url, http::Version::HTTP_2)
        .await;
    proxy_context::with_snapshot(clients, async {
        let route = native_file_route(url, ProxyPolicy::System);
        let connection = assert_native_file_protocol(&route).await;
        assert_eq!(connection.remote_addr, None);
    })
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 2);
    assert!(!fixture.server.is_finished());
    assert!(!proxy.server.is_finished());
}

#[tokio::test]
async fn native_socks5_preserves_ipv4_ipv6_targets_and_empty_passwords() {
    let fixture = FileClientTlsFixture::start().await;
    for (scheme, host, proxy_host, password) in [
        ("socks5", "127.0.0.1", "127.0.0.1", "pass"),
        ("socks5h", "::1", "::1", ""),
        ("socks5", "native-local.invalid", "::1", "pass"),
    ] {
        let proxy = FileClientProxyFixture::start_socks5(
            fixture.address,
            scheme,
            proxy_host,
            password,
        )
        .await;
        let mut config = proxy.config();
        config.password = password.to_string();
        let clients = DownloadClients::build(&config, true, true).unwrap();
        if host == "native-local.invalid" {
            clients.system_dns.set_test_addresses(
                host,
                vec![SocketAddr::new("::1".parse().unwrap(), 0)],
            );
        }
        let authority_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let url =
            format!("https://{authority_host}:{}/file", fixture.address.port());
        proxy_context::with_snapshot(clients, async {
            let route = native_file_route(url, ProxyPolicy::System);
            let connection = assert_native_file_protocol(&route).await;
            assert_eq!(connection.remote_addr, None);
            assert!(connection.endpoint_lease.is_none());
        })
        .await;
        assert_eq!(proxy.connections.load(Ordering::SeqCst), 1);
        assert!(!proxy.server.is_finished());
    }
    assert!(!fixture.server.is_finished());
}

#[tokio::test]
async fn custom_connect_proxy_preserves_file_client_http2_and_http1_fallback() {
    let fixture = FileClientTlsFixture::start().await;
    let proxy = FileClientProxyFixture::start(fixture.address).await;
    let clients = DownloadClients::build(&proxy.config(), true, false).unwrap();
    assert_file_client_protocol(
        &clients.system,
        &fixture.url(),
        http::Version::HTTP_2,
    )
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 1);
    assert_file_client_protocol(
        &clients.direct,
        &fixture.url(),
        http::Version::HTTP_2,
    )
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 1);
    assert_file_client_protocol(
        &clients.http1_system,
        &fixture.url(),
        http::Version::HTTP_11,
    )
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 2);
    assert_file_client_protocol(
        &clients.http1_direct,
        &fixture.url(),
        http::Version::HTTP_11,
    )
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 2);
    let strict = DownloadClients::build(&proxy.config(), false, false).unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(5),
            strict.system.get(fixture.url()).send()
        )
        .await
        .unwrap()
        .is_err()
    );
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 3);
    assert!(!fixture.server.is_finished());
    assert!(!proxy.server.is_finished());
}

#[tokio::test]
async fn https_connect_proxy_preserves_reqwest_and_native_http2_and_enforces_tls_policy()
 {
    let fixture = FileClientTlsFixture::start().await;
    let proxy = FileClientProxyFixture::start_https(fixture.address).await;
    let clients = DownloadClients::build(&proxy.config(), true, true).unwrap();
    assert_file_client_protocol(
        &clients.system,
        &fixture.url(),
        http::Version::HTTP_2,
    )
    .await;
    let route = native_file_route(fixture.url(), ProxyPolicy::System);
    proxy_context::with_snapshot(clients, async {
        let connection = assert_native_file_protocol(&route).await;
        assert_eq!(connection.remote_addr, None);
        assert!(connection.endpoint_lease.is_none());
    })
    .await;
    assert_eq!(proxy.connections.load(Ordering::SeqCst), 2);
    let strict = DownloadClients::build(&proxy.config(), false, true).unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(5),
            strict.system.get(fixture.url()).send()
        )
        .await
        .unwrap()
        .is_err()
    );
    proxy_context::with_snapshot(strict, async {
        assert!(
            tokio::time::timeout(
                Duration::from_secs(5),
                h2_pool::shared_batch_connection(&route, true)
            )
            .await
            .unwrap()
            .is_err()
        );
    })
    .await;
    assert_eq!(
        proxy.connections.load(Ordering::SeqCst),
        2,
        "strict TLS rejects the proxy before CONNECT"
    );
    assert!(!fixture.server.is_finished());
    assert!(!proxy.server.is_finished());
}

#[tokio::test]
async fn tianpao_metadata_redirects_are_blocked_before_following() {
    let (url, requests, server) = spawn_http_fixture(
        HttpFixture::new("302 Found", Vec::new())
            .with_headers("Location: https://mod.tianpao.top/unknown\r\n"),
    )
    .await;
    let client = reqwest_client_builder().no_proxy().build().unwrap();
    let error = client.get(url).send().await.unwrap_err();
    assert!(error.is_redirect());
    assert_eq!(requests.load(Ordering::Relaxed), 1);
    server.abort();
}
