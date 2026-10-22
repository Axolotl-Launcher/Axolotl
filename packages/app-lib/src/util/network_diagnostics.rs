//! Bounded, read-only probes for the launcher's public service endpoints.

use crate::util::download_dns::DownloadDnsResolver;
use crate::util::proxy::{ProxyConfig, ProxyMode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOWNLOAD_SAMPLE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkTestTarget {
    CurseforgeApi,
    ModrinthApi,
    ModrinthCdn,
    MojangApi,
    MinecraftServices,
}

impl NetworkTestTarget {
    fn endpoint(self) -> (&'static str, &'static str) {
        match self {
            Self::CurseforgeApi => (
                "api.curseforge.com",
                "https://api.curseforge.com/v1/games/432",
            ),
            Self::ModrinthApi => (
                "api.modrinth.com",
                "https://api.modrinth.com/v2/tag/game_version",
            ),
            Self::ModrinthCdn => (
                "cdn.modrinth.com",
                "https://cdn.modrinth.com/data/uCdwusMi/versions/13xaVT4W/DistantHorizons-2.1.0-a-1.20.6-noForge.jar",
            ),
            Self::MojangApi => (
                "api.mojang.com",
                "https://api.mojang.com/users/profiles/minecraft/Notch",
            ),
            Self::MinecraftServices => (
                "api.minecraftservices.com",
                "https://api.minecraftservices.com/publickeys",
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkTestMode {
    Launcher,
    Direct,
}

#[derive(Debug, Serialize)]
pub struct NetworkTestResult {
    pub target: NetworkTestTarget,
    pub mode: NetworkTestMode,
    pub tested_at: String,
    pub launcher_version: String,
    pub platform: String,
    pub url: String,
    pub proxy_mode: String,
    pub dns_mode: String,
    pub tls_verification: bool,
    pub resolved_ips: Vec<String>,
    pub dns_ms: u64,
    pub dns_error: Option<String>,
    /// The transport peer can be a proxy, rather than the destination CDN.
    pub peer_ip: Option<String>,
    pub status: Option<u16>,
    pub response_ms: Option<u64>,
    pub total_ms: u64,
    pub http_version: Option<String>,
    pub server: Option<String>,
    pub served_by: Option<String>,
    pub cache: Option<String>,
    pub bytes_received: u64,
    pub sample_ms: Option<u64>,
    pub sample_mib_per_second: Option<f64>,
    pub error: Option<String>,
}

fn millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

// Do not serialize arbitrary transport errors: custom proxy URLs may contain credentials.
fn transport_error(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection"
    } else if error.is_body() || error.is_decode() {
        "body"
    } else {
        "request"
    }
}

pub async fn test_network_connection(
    target: NetworkTestTarget,
    mode: NetworkTestMode,
) -> crate::Result<NetworkTestResult> {
    let state = crate::State::get().await?;
    let configured = state.download_clients();
    let proxy = match mode {
        NetworkTestMode::Launcher => configured.proxy,
        NetworkTestMode::Direct => ProxyConfig {
            mode: ProxyMode::None,
            ..ProxyConfig::default()
        },
    };
    probe_connection(
        target,
        mode,
        &proxy,
        configured.doh_enabled,
        configured.ignore_ssl_errors,
        target.endpoint(),
    )
    .await
}

async fn probe_connection(
    target: NetworkTestTarget,
    mode: NetworkTestMode,
    proxy: &ProxyConfig,
    doh_enabled: bool,
    ignore_ssl_errors: bool,
    (host, url): (&str, &str),
) -> crate::Result<NetworkTestResult> {
    let resolver = Arc::new(DownloadDnsResolver::with_doh_and_proxy(
        doh_enabled,
        Some(proxy),
    )?);
    // A fresh client avoids reporting a warm pool's timings as a new connection.
    let builder = reqwest::Client::builder()
        .dns_resolver(Arc::clone(&resolver))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(6))
        .danger_accept_invalid_certs(ignore_ssl_errors)
        .user_agent(crate::launcher_user_agent());
    let client = proxy.apply(builder)?.build()?;
    let started = Instant::now();
    let dns_started = Instant::now();
    let addresses = tokio::time::timeout(
        Duration::from_secs(6),
        resolver.resolve_host(host),
    )
    .await;
    let (resolved_ips, dns_error) = match addresses {
        Ok(Ok(addresses)) => {
            (addresses.iter().map(ToString::to_string).collect(), None)
        }
        Ok(Err(_)) => (Vec::new(), Some("dns".to_string())),
        Err(_) => (Vec::new(), Some("timeout".to_string())),
    };
    let mut result = NetworkTestResult {
        target,
        mode,
        tested_at: chrono::Utc::now().to_rfc3339(),
        launcher_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: format!(
            "{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        url: url.to_string(),
        proxy_mode: proxy.mode.as_str().to_string(),
        dns_mode: if doh_enabled { "doh" } else { "system" }.to_string(),
        tls_verification: !ignore_ssl_errors,
        resolved_ips,
        dns_ms: millis(dns_started),
        dns_error,
        peer_ip: None,
        status: None,
        response_ms: None,
        total_ms: 0,
        http_version: None,
        server: None,
        served_by: None,
        cache: None,
        bytes_received: 0,
        sample_ms: None,
        sample_mib_per_second: None,
        error: None,
    };
    let cdn = matches!(target, NetworkTestTarget::ModrinthCdn);
    let mut request = client.get(url);
    if cdn {
        request = request
            .header(
                reqwest::header::RANGE,
                format!("bytes=0-{}", DOWNLOAD_SAMPLE_BYTES - 1),
            )
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
    }
    if matches!(target, NetworkTestTarget::CurseforgeApi)
        && let Some(key) = crate::api::curseforge::api_key()
    {
        request = request.header("x-api-key", key);
    }
    let request_started = Instant::now();
    match request.send().await {
        Ok(mut response) => {
            result.response_ms = Some(millis(request_started));
            result.peer_ip = response
                .remote_addr()
                .map(|address| address.ip().to_string());
            result.status = Some(response.status().as_u16());
            result.http_version = Some(format!("{:?}", response.version()));
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| value.chars().take(512).collect())
            };
            result.server = header("server");
            result.served_by = header("x-served-by");
            result.cache = header("x-cache");
            if cdn && response.status().is_success() {
                let sample_started = Instant::now();
                let deadline =
                    tokio::time::Instant::now() + Duration::from_secs(3);
                // Stop at 8 MiB even when the server ignores Range; never save the body.
                loop {
                    match tokio::time::timeout_at(deadline, response.chunk())
                        .await
                    {
                        Ok(Ok(Some(chunk))) => {
                            result.bytes_received += (chunk.len() as u64).min(
                                DOWNLOAD_SAMPLE_BYTES - result.bytes_received,
                            );
                            if result.bytes_received >= DOWNLOAD_SAMPLE_BYTES {
                                break;
                            }
                        }
                        Ok(Ok(None)) => break,
                        Ok(Err(error)) => {
                            result.error =
                                Some(transport_error(&error).to_string());
                            break;
                        }
                        Err(_) => break,
                    }
                }
                result.sample_ms = Some(millis(sample_started));
                if result.bytes_received > 0
                    && sample_started.elapsed().as_secs_f64() > 0.0
                {
                    result.sample_mib_per_second = Some(
                        result.bytes_received as f64
                            / 1048576.0
                            / sample_started.elapsed().as_secs_f64(),
                    );
                } else if result.error.is_none() {
                    result.error = Some("empty_body".to_string());
                }
            }
        }
        Err(error) => result.error = Some(transport_error(&error).to_string()),
    }
    result.total_ms = millis(started);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn local_probe(response: Vec<u8>) -> NetworkTestResult {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let read = stream.read(&mut request).await.unwrap();
            let request =
                String::from_utf8_lossy(&request[..read]).to_lowercase();
            assert!(request.contains("range: bytes=0-8388607"));
            assert!(!request.contains("x-api-key"));
            let _ = stream.write_all(&response).await;
        });
        let result = probe_connection(
            NetworkTestTarget::ModrinthCdn,
            NetworkTestMode::Direct,
            &ProxyConfig {
                mode: ProxyMode::None,
                ..ProxyConfig::default()
            },
            false,
            false,
            ("127.0.0.1", &format!("http://{address}/file.jar")),
        )
        .await
        .unwrap();
        server.await.unwrap();
        result
    }

    #[tokio::test]
    async fn ignored_range_is_bounded_and_reports_transport_peer() {
        let mut response = b"HTTP/1.1 200 OK\r\nContent-Length: 9437184\r\nX-Served-By: cache-test-NRT\r\nX-Cache: HIT\r\nConnection: close\r\n\r\n".to_vec();
        response.resize(response.len() + 9 * 1024 * 1024, b'x');
        let result = local_probe(response).await;
        assert_eq!(result.status, Some(200));
        assert_eq!(result.bytes_received, DOWNLOAD_SAMPLE_BYTES);
        assert_eq!(result.peer_ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(result.served_by.as_deref(), Some("cache-test-NRT"));
        assert_eq!(result.cache.as_deref(), Some("HIT"));
        assert!(
            result
                .sample_mib_per_second
                .is_some_and(|speed| speed > 0.0)
        );
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn redirects_are_reported_without_following_the_destination() {
        let result = local_probe(b"HTTP/1.1 302 Found\r\nLocation: http://untrusted.invalid/file.jar\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()).await;
        assert_eq!(result.status, Some(302));
        assert_eq!(result.bytes_received, 0);
        assert!(result.sample_mib_per_second.is_none());
    }

    #[tokio::test]
    async fn empty_downloads_are_not_reported_as_successful_samples() {
        let result = local_probe(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()).await;
        assert_eq!(result.error.as_deref(), Some("empty_body"));
        assert!(result.sample_mib_per_second.is_none());
    }

    #[test]
    fn targets_are_fixed_https_endpoints() {
        for target in [
            NetworkTestTarget::CurseforgeApi,
            NetworkTestTarget::ModrinthApi,
            NetworkTestTarget::ModrinthCdn,
            NetworkTestTarget::MojangApi,
            NetworkTestTarget::MinecraftServices,
        ] {
            let (host, endpoint) = target.endpoint();
            let url = url::Url::parse(endpoint).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some(host));
            assert!(url.username().is_empty());
            assert!(url.password().is_none());
        }
        assert!(
            serde_json::from_str::<NetworkTestTarget>("\"https://localhost\"")
                .is_err()
        );
    }
}
