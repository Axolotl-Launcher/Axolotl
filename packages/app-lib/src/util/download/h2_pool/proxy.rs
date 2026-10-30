//! Proxy selection and tunneling for the shared HTTP/2 pool.

use super::{CONNECT_TIMEOUT, TLS_HANDSHAKE_TIMEOUT, connect_tcp, tls_config};
use crate::util::fetch::{DownloadRoute, ProxyPolicy};
use fast_socks5::client::{Config as SocksConfig, Socks5Stream};
use fast_socks5::util::target_addr::TargetAddr;
use fast_socks5::{AuthenticationMethod, Socks5Command};
use http::Uri;
use hyper_util::client::legacy::connect::proxy::{SocksV4, Tunnel};
use hyper_util::client::proxy::matcher::Intercept;
use hyper_util::rt::TokioIo;
use rustls_pki_types::ServerName;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;
use tower_service::Service;

pub(super) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
pub(super) type BoxIo = Box<dyn Io>;

pub(super) fn intercept(route: &DownloadRoute) -> Option<Intercept> {
    if route.proxy == ProxyPolicy::Direct {
        return None;
    }
    let matcher = super::super::proxy_context::clients()
        .map(|clients| clients.native_proxy)
        .or_else(|| {
            crate::util::proxy::ProxyConfig::default()
                .native_matcher()
                .ok()
        })?;
    matcher.intercept(&route.url.parse::<Uri>().ok()?)
}

struct ProxyConnector {
    route: DownloadRoute,
    reserve_native_budget: bool,
    tls: Arc<rustls::ClientConfig>,
}

impl Service<Uri> for ProxyConnector {
    type Response = TokioIo<BoxIo>;
    type Error = io::Error;
    type Future =
        Pin<Box<dyn Future<Output = io::Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, destination: Uri) -> Self::Future {
        let route = self.route.clone();
        let reserve_native_budget = self.reserve_native_budget;
        let tls_config = Arc::clone(&self.tls);
        Box::pin(async move {
            let host = destination
                .host()
                .ok_or_else(|| io::Error::other("proxy has no host"))?
                .trim_matches(['[', ']']);
            let secure = destination.scheme_str() == Some("https");
            let port = destination.port_u16().unwrap_or(if secure {
                443
            } else if destination.scheme_str() == Some("http") {
                80
            } else {
                1080
            });
            let tcp = connect_tcp(host, port, &route, reserve_native_budget)
                .await
                .map_err(|error| {
                    io::Error::new(error.kind(), "proxy TCP connection failed")
                })?;
            let stream: BoxIo = if secure {
                // The outer proxy connection carries HTTP/1.1 CONNECT. The
                // destination negotiates HTTP/2 inside the resulting tunnel.
                let mut config = (*tls_config).clone();
                config.alpn_protocols = vec![b"http/1.1".to_vec()];
                config.enable_early_data = false;
                let name =
                    ServerName::try_from(host.to_string()).map_err(|_| {
                        io::Error::other("invalid TLS proxy server name")
                    })?;
                let stream = tokio::time::timeout(
                    TLS_HANDSHAKE_TIMEOUT,
                    TlsConnector::from(Arc::new(config)).connect(name, tcp),
                )
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "proxy TLS handshake timed out",
                    )
                })?
                .map_err(|_| io::Error::other("proxy TLS handshake failed"))?;
                Box::new(stream)
            } else {
                Box::new(tcp)
            };
            Ok(TokioIo::new(stream))
        })
    }
}

pub(super) async fn connect(
    route: &DownloadRoute,
    proxy: Intercept,
    reserve_native_budget: bool,
) -> io::Result<BoxIo> {
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        let mut destination = route
            .url
            .parse::<Uri>()
            .map_err(|_| io::Error::other("invalid tunnel destination"))?;
        let mut connector = ProxyConnector {
            route: route.clone(),
            reserve_native_budget,
            tls: tls_config(),
        };
        match proxy.uri().scheme_str() {
            Some("http" | "https") => {
                let mut tunnel = Tunnel::new(proxy.uri().clone(), connector);
                if let Some(auth) = proxy.basic_auth() {
                    tunnel = tunnel.with_auth(auth.clone());
                }
                tunnel
                    .call(destination)
                    .await
                    .map(TokioIo::into_inner)
                    .map_err(|error| {
                        io::Error::other(format!(
                            "HTTP CONNECT failed: {error}"
                        ))
                    })
            }
            Some(scheme @ ("socks4" | "socks4a" | "socks5" | "socks5h")) => {
                if matches!(scheme, "socks4" | "socks5") {
                    destination = locally_resolved_destination(
                        route,
                        &destination,
                        scheme == "socks4",
                    )
                    .await?;
                }
                if matches!(scheme, "socks4" | "socks4a") {
                    SocksV4::new(proxy.uri().clone(), connector)
                        .call(destination)
                        .await
                        .map(TokioIo::into_inner)
                        .map_err(|error| {
                            io::Error::other(format!(
                                "SOCKS4 tunnel failed: {error}"
                            ))
                        })
                } else {
                    let auth = socks5_auth(proxy.raw_auth())?;
                    let target = socks5_target(&destination)?;
                    let socket =
                        connector.call(proxy.uri().clone()).await?.into_inner();
                    let mut stream = Socks5Stream::use_stream(
                        socket,
                        auth,
                        SocksConfig::default(),
                    )
                    .await
                    .map_err(|_| {
                        io::Error::other("SOCKS5 authentication failed")
                    })?;
                    stream
                        .request(Socks5Command::TCPConnect, target)
                        .await
                        .map_err(|_| {
                            io::Error::other("SOCKS5 tunnel failed")
                        })?;
                    Ok(Box::new(stream) as BoxIo)
                }
            }
            _ => Err(io::Error::other("unsupported native proxy scheme")),
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(io::ErrorKind::TimedOut, "proxy connection timed out")
    })?
}

fn socks5_auth(
    credentials: Option<(&str, &str)>,
) -> io::Result<Option<AuthenticationMethod>> {
    credentials
        .map(|(username, password)| {
            // SOCKS5 encodes credential lengths in one byte. Empty credentials are
            // accepted by existing launcher proxy configurations and some proxies.
            if username.len() > 255 || password.len() > 255 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "SOCKS5 credentials exceed 255 bytes",
                ));
            }
            Ok(AuthenticationMethod::Password {
                username: username.into(),
                password: password.into(),
            })
        })
        .transpose()
}

fn socks5_target(destination: &Uri) -> io::Result<TargetAddr> {
    let host = destination
        .host()
        .ok_or_else(|| io::Error::other("SOCKS5 destination has no host"))?
        .trim_matches(['[', ']']);
    let port = destination.port_u16().unwrap_or(443);
    match host.parse::<std::net::IpAddr>() {
        Ok(address) => {
            Ok(TargetAddr::Ip(std::net::SocketAddr::new(address, port)))
        }
        Err(_) if host.len() <= 255 => {
            Ok(TargetAddr::Domain(host.into(), port))
        }
        Err(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOCKS5 destination exceeds 255 bytes",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks5_targets_keep_ipv6_binary_and_hostnames_remote() {
        assert_eq!(
            socks5_target(&"https://[::1]:8443/file".parse().unwrap()).unwrap(),
            TargetAddr::Ip("[::1]:8443".parse().unwrap())
        );
        assert_eq!(
            socks5_target(&"https://assets.invalid/file".parse().unwrap())
                .unwrap(),
            TargetAddr::Domain("assets.invalid".into(), 443)
        );
    }

    #[test]
    fn socks5_credentials_allow_empty_password_and_reject_truncation() {
        assert!(
            matches!(socks5_auth(Some(("user", ""))).unwrap(), Some(AuthenticationMethod::Password { password, .. }) if password.is_empty())
        );
        assert!(socks5_auth(Some((&"u".repeat(256), "password"))).is_err());
        assert!(socks5_auth(Some(("user", &"密".repeat(86)))).is_err());
        assert!(socks5_auth(None).unwrap().is_none());
    }
}

async fn locally_resolved_destination(
    route: &DownloadRoute,
    destination: &Uri,
    ipv4_only: bool,
) -> io::Result<Uri> {
    let host = destination
        .host()
        .ok_or_else(|| io::Error::other("destination has no host"))?
        .trim_matches(['[', ']']);
    let addresses = match host.parse::<std::net::IpAddr>() {
        Ok(address) => vec![address],
        Err(_) => {
            super::super::proxy_context::resolver(route.proxy)
                .resolve_host(host)
                .await?
        }
    };
    let address = addresses
        .into_iter()
        .find(|ip| !ipv4_only || ip.is_ipv4())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "SOCKS destination has no usable address",
            )
        })?;
    let authority = std::net::SocketAddr::new(
        address,
        destination.port_u16().unwrap_or(443),
    );
    format!("https://{authority}")
        .parse()
        .map_err(|_| io::Error::other("invalid resolved SOCKS destination"))
}
