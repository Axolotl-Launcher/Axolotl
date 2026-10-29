#[cfg(test)]
use super::BENCHMARK_TIMING;
use super::throttle::DOWNLOAD_DNS_RESOLVER;
use crate::util::download::constants::{
    FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID, FETCH_GOAWAY, FETCH_HTTP_2,
    FETCH_HTTP2, FETCH_PROTOCOL_ERROR,
    FETCH_PROXIED_CLIENT_CONFIGURATION_SHOULD_BE_VALID,
    FETCH_PROXY_CONFIGURATION_SHOULD_BE_VALID, FETCH_REFUSED_STREAM,
    FETCH_STREAM_ERROR, FETCH_TOO_MANY_REDIRECTS, FETCH_UNCONFIGURED,
    FILE_H2_KEEPALIVE, FILE_POOL_MAX_IDLE_PER_HOST, FILE_TCP_KEEPALIVE,
    FILE_TRANSFER_CONNECT_TIMEOUT, FILE_TRANSFER_READ_TIMEOUT,
    NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED,
};
use crate::util::download::retired_sources;
use crate::util::download_dns::DownloadDnsResolver;
use std::error::Error;
use std::sync::{Arc, LazyLock};
#[cfg(test)]
use std::time;

pub(crate) fn is_h2_protocol_failure(error: &reqwest::Error) -> bool {
    let mut chain = String::new();
    let mut source = error.source();
    while let Some(next) = source {
        if !chain.is_empty() {
            chain.push(' ');
        }
        chain.push_str(&next.to_string());
        source = next.source();
    }
    let chain = chain.to_ascii_lowercase();
    [
        FETCH_HTTP2,
        FETCH_HTTP_2,
        FETCH_GOAWAY,
        FETCH_STREAM_ERROR,
        FETCH_PROTOCOL_ERROR,
        FETCH_REFUSED_STREAM,
    ]
    .iter()
    .any(|marker| chain.contains(marker))
}

pub(crate) fn reqwest_client_builder() -> reqwest::ClientBuilder {
    #[cfg(test)]
    let read_timeout = if BENCHMARK_TIMING.try_with(|_| ()).is_ok() {
        time::Duration::from_secs(60)
    } else {
        FILE_TRANSFER_READ_TIMEOUT
    };
    #[cfg(not(test))]
    let read_timeout = FILE_TRANSFER_READ_TIMEOUT;
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if retired_sources::is_retired(attempt.url()) {
                attempt.error(NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED)
            } else if attempt.previous().len() >= 10 {
                attempt.error(FETCH_TOO_MANY_REDIRECTS)
            } else {
                attempt.follow()
            }
        }))
        .connect_timeout(FILE_TRANSFER_CONNECT_TIMEOUT)
        .read_timeout(read_timeout)
        .tcp_keepalive(Some(FILE_TCP_KEEPALIVE))
        .tcp_nodelay(true)
        .pool_max_idle_per_host(FILE_POOL_MAX_IDLE_PER_HOST)
        .dns_resolver(Arc::clone(&DOWNLOAD_DNS_RESOLVER))
        .user_agent(crate::launcher_user_agent())
}

pub(crate) fn file_reqwest_client_builder() -> reqwest::ClientBuilder {
    reqwest_client_builder()
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Some(FILE_H2_KEEPALIVE))
}

pub fn build_proxied_client(
    proxy: &crate::util::proxy::ProxyConfig,
) -> reqwest::Client {
    proxy
        .apply(reqwest_client_builder())
        .expect(FETCH_PROXY_CONFIGURATION_SHOULD_BE_VALID)
        .build()
        .expect(FETCH_PROXIED_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
}

/// Authentication and reachability probes share the current proxy, DNS and TLS
/// settings. The fallback is only for callers before application initialization.
pub(crate) fn authentication_client() -> reqwest::Client {
    if let Some(clients) =
        crate::util::download::proxy_context::explicit_clients()
    {
        return clients.metadata;
    }
    if let Some(state) = crate::State::get_if_initialized() {
        return state.configured_http_client();
    }
    static SYSTEM_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        build_proxied_client(&crate::util::proxy::ProxyConfig::default())
    });
    SYSTEM_CLIENT.clone()
}

pub async fn configured_client() -> crate::Result<reqwest::Client> {
    if let Some(clients) =
        crate::util::download::proxy_context::explicit_clients()
    {
        return Ok(clients.metadata);
    }
    Ok(crate::State::get().await?.configured_http_client())
}

#[derive(Clone)]
pub(crate) struct DownloadClients {
    pub(crate) system_proxy_fingerprint: String,
    pub(crate) metadata: reqwest::Client,
    pub(crate) system: reqwest::Client,
    pub(crate) direct: reqwest::Client,
    pub(crate) http1_system: reqwest::Client,
    pub(crate) http1_direct: reqwest::Client,
    pub(crate) proxy: crate::util::proxy::ProxyConfig,
    pub(crate) ignore_ssl_errors: bool,
    pub(crate) scope: String,
    pub(crate) system_dns: Arc<DownloadDnsResolver>,
    pub(crate) direct_dns: Arc<DownloadDnsResolver>,
    pub(crate) doh_enabled: bool,
    pub(crate) force_http1: bool,
    pub(crate) endpoints:
        Arc<crate::util::download::endpoint_selection::EndpointSelector>,
}

impl DownloadClients {
    pub(crate) fn refresh_system_proxy(&self) -> crate::Result<Option<Self>> {
        if self.proxy.mode != crate::util::proxy::ProxyMode::System
            || self.system_proxy_fingerprint
                == crate::util::proxy::system_proxy_fingerprint()
        {
            return Ok(None);
        }
        Self::build(&self.proxy, self.ignore_ssl_errors, self.doh_enabled)
            .map(Some)
    }

    pub(crate) fn build(
        proxy: &crate::util::proxy::ProxyConfig,
        ignore_ssl_errors: bool,
        doh_enabled: bool,
    ) -> crate::Result<Self> {
        let system_proxy_fingerprint =
            crate::util::proxy::system_proxy_fingerprint();
        let system_dns = Arc::new(DownloadDnsResolver::with_doh_and_proxy(
            doh_enabled,
            Some(proxy),
        )?);
        let direct_dns = Arc::new(DownloadDnsResolver::with_doh_and_proxy(
            doh_enabled,
            None,
        )?);
        let build = |direct: bool, http1: bool| {
            let builder = file_reqwest_client_builder()
                .redirect(reqwest::redirect::Policy::none())
                .danger_accept_invalid_certs(ignore_ssl_errors);
            // Reqwest does not expose a socket-lifetime admission hook. In
            // automatic mode its idle sockets would outlive request permits
            // and accumulate alongside probes and new endpoint generations.
            // Native H2 retains its separately budgeted multiplexed pool.
            let builder = if crate::util::download::endpoint_selection::SelectionMode::configured()
                == crate::util::download::endpoint_selection::SelectionMode::Auto
            {
                builder.pool_max_idle_per_host(0)
            } else {
                builder
            };
            let builder = builder.dns_resolver(if direct {
                Arc::clone(&direct_dns)
            } else {
                Arc::clone(&system_dns)
            });
            let builder = if http1
                || !direct
                    && proxy.mode == crate::util::proxy::ProxyMode::Custom
            {
                builder.http1_only()
            } else {
                builder
            };
            let builder = if direct {
                builder.no_proxy()
            } else {
                proxy.apply(builder)?
            };
            #[cfg(not(test))]
            let builder = builder.https_only(true);
            builder.build().map_err(crate::Error::from)
        };
        Ok(Self {
            system_proxy_fingerprint,
            endpoints:
                crate::util::download::endpoint_selection::EndpointSelector::new(
                    crate::util::download::proxy_context::fingerprint_with_doh(
                        proxy,
                        ignore_ssl_errors,
                        doh_enabled,
                    ),
                ),
            metadata: proxy
                .apply(
                    reqwest_client_builder()
                        .dns_resolver(Arc::clone(&system_dns))
                        .danger_accept_invalid_certs(ignore_ssl_errors),
                )?
                .build()?,
            system: build(false, false)?,
            direct: build(true, false)?,
            http1_system: build(false, true)?,
            http1_direct: build(true, true)?,
            proxy: proxy.clone(),
            ignore_ssl_errors,
            scope: crate::util::download::proxy_context::fingerprint_with_doh(
                proxy,
                ignore_ssl_errors,
                doh_enabled,
            ),
            system_dns,
            direct_dns,
            doh_enabled,
            force_http1: false,
        })
    }

    pub(crate) fn for_request(
        system: &reqwest::Client,
        direct: &reqwest::Client,
    ) -> Self {
        if let Some(mut clients) =
            crate::util::download::proxy_context::clients()
        {
            if std::ptr::eq(
                system,
                &raw const *HTTP1_NO_REDIRECT_REQWEST_CLIENT,
            ) {
                clients.system = clients.http1_system.clone();
                clients.direct = clients.http1_direct.clone();
                clients.force_http1 = true;
            }
            return clients;
        }
        Self {
            system_proxy_fingerprint:
                crate::util::proxy::system_proxy_fingerprint(),
            metadata: system.clone(),
            system: system.clone(),
            direct: direct.clone(),
            http1_system: HTTP1_NO_REDIRECT_REQWEST_CLIENT.clone(),
            http1_direct: HTTP1_DIRECT_REQWEST_CLIENT.clone(),
            proxy: Default::default(),
            ignore_ssl_errors: false,
            scope: FETCH_UNCONFIGURED.into(),
            system_dns: Arc::clone(&DOWNLOAD_DNS_RESOLVER),
            direct_dns: Arc::clone(&DOWNLOAD_DNS_RESOLVER),
            doh_enabled: false,
            force_http1: std::ptr::eq(
                system,
                &raw const *HTTP1_NO_REDIRECT_REQWEST_CLIENT,
            ),
            endpoints:
                crate::util::download::endpoint_selection::EndpointSelector::new(
                    FETCH_UNCONFIGURED,
                ),
        }
    }
}

fn http1_file_reqwest_client_builder() -> reqwest::ClientBuilder {
    reqwest_client_builder().http1_only()
}

pub static INSECURE_REQWEST_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| {
        reqwest_client_builder()
            .no_proxy()
            .build()
            .expect(FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
    });

pub(crate) static NO_REDIRECT_REQWEST_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| {
        let builder = file_reqwest_client_builder()
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(not(test))]
        let builder = builder.https_only(true);
        builder
            .build()
            .expect(FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
    });

pub(crate) static DIRECT_REQWEST_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| {
        let builder = file_reqwest_client_builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(not(test))]
        let builder = builder.https_only(true);
        builder
            .build()
            .expect(FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
    });

pub(crate) static HTTP1_NO_REDIRECT_REQWEST_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| {
        let builder = http1_file_reqwest_client_builder()
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(not(test))]
        let builder = builder.https_only(true);
        builder
            .build()
            .expect(FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
    });

pub(crate) static HTTP1_DIRECT_REQWEST_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| {
        let builder = http1_file_reqwest_client_builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(not(test))]
        let builder = builder.https_only(true);
        builder
            .build()
            .expect(FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID)
    });
