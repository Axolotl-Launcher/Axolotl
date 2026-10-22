//! HTTP request construction and redirect handling for downloads.

use super::modrinth_redirect::repair_official_redirect as repair_official_cdn_redirect;
use super::retired_sources;
use crate::ErrorKind;
use crate::util::download::constants::{
    API_KEY_HEADER, AUTHORIZATION_HEADER, CURSEFORGE_API_HOST,
    CURSEFORGE_EDGE_HOST, DOWNLOAD_META_HEADER, IDENTITY_ENCODING,
    MAX_DOWNLOAD_REDIRECTS, MAX_REDIRECT_LOCATION_BYTES,
    NATIVE_REQUEST_TIMED_OUT_WAITING_FOR_CDN_RESPONSE_CONNECTION_BUDGET,
    RESOURCE_WAIT_TIMEOUT,
};
use crate::util::fetch::{
    DIRECT_REQWEST_CLIENT, DownloadClients, DownloadMeta, DownloadRoute,
    NO_REDIRECT_REQWEST_CLIENT, ProxyPolicy, authority_uses_http1_fallback_for,
    forget_effective_route_authority, is_allowed_download_redirect,
    is_h2_protocol_failure, is_official_modrinth_download_url,
    is_sensitive_header, record_authority_h2_failure_for,
    record_dns_connection_failure, remember_effective_route_authority,
    same_origin, sanitize_url_for_log, url_authority,
};
use reqwest::{Url, header};
use tokio::sync::Mutex as AsyncMutex;

pub(crate) fn byte_range_header_value(
    range_start: Option<u64>,
    range_end: Option<u64>,
) -> Option<String> {
    range_start.map(|start| {
        range_end.map_or_else(
            || format!("bytes={start}-"),
            |end| format!("bytes={start}-{end}"),
        )
    })
}

/// CDN byte fetches use the same redirect normalization as streamed downloads.
pub(crate) async fn send_cdn_request(
    builder: reqwest::RequestBuilder,
) -> crate::Result<reqwest::Response> {
    send_cdn_request_inner(builder, None, None).await
}

pub(crate) async fn send_cdn_request_with_clients(
    builder: reqwest::RequestBuilder,
    clients: &DownloadClients,
    proxy: ProxyPolicy,
) -> crate::Result<reqwest::Response> {
    send_cdn_request_inner(builder, Some((clients, proxy)), None).await
}

pub(crate) async fn send_cdn_request_with_budget(
    builder: reqwest::RequestBuilder,
    clients: &DownloadClients,
    proxy: ProxyPolicy,
    budget: super::native_budget::NativeBudgetPermit,
) -> crate::Result<reqwest::Response> {
    send_cdn_request_inner(builder, Some((clients, proxy)), Some(budget)).await
}

#[derive(Clone)]
pub(crate) struct ResponseBudget {
    _permit: std::sync::Arc<super::native_budget::NativeBudgetPermit>,
}

async fn send_cdn_request_inner(
    builder: reqwest::RequestBuilder,
    selection: Option<(&DownloadClients, ProxyPolicy)>,
    mut admitted: Option<super::native_budget::NativeBudgetPermit>,
) -> crate::Result<reqwest::Response> {
    let (client, request) = builder.build_split();
    let mut request = request?;
    *request.url_mut() =
        retired_sources::normalize_parsed(request.url().clone())?;
    let original = request.url().clone();
    for hop in 0..=MAX_DOWNLOAD_REDIRECTS {
        let physical_budget = if let Some(permit) = admitted.take() {
            Some(std::sync::Arc::new(permit))
        } else if let Some((_, proxy)) = selection {
            let route = DownloadRoute {
                url: request.url().to_string(),
                source: crate::util::fetch::DownloadRouteSource::Official,
                is_mirror: false,
                allow_sensitive_headers: true,
                supports_range: true,
                proxy,
            };
            Some(std::sync::Arc::new(
                tokio::time::timeout(
                    RESOURCE_WAIT_TIMEOUT,
                    super::native_budget::acquire(&route),
                )
                .await
                .map_err(|_| {
                    ErrorKind::NetworkError(
                        NATIVE_REQUEST_TIMED_OUT_WAITING_FOR_CDN_RESPONSE_CONNECTION_BUDGET
                            .into(),
                    )
                })??,
            ))
        } else {
            None
        };
        let next_request = request.try_clone();
        let pinned = selection.and_then(|(clients, proxy)| {
            let fallback = clients.force_http1
                || url_authority(request.url().as_str()).is_some_and(
                    |authority| {
                        authority_uses_http1_fallback_for(&authority, proxy)
                    },
                );
            clients.endpoints.client_for(
                request.url(),
                proxy,
                clients,
                fallback,
            )
        });
        let active_client =
            pinned.as_ref().map_or(&client, |(client, _)| client);
        let mut response = match active_client.execute(request).await {
            Ok(response) => response,
            Err(error) => {
                if let Some((_, lease)) = &pinned
                    && let Some((clients, _)) = selection
                {
                    clients.endpoints.record_failure(
                        lease,
                        super::endpoint_selection::invalid_certificate(&error),
                    );
                }
                return Err(error.into());
            }
        };
        if let Some(permit) = physical_budget {
            response
                .extensions_mut()
                .insert(ResponseBudget { _permit: permit });
        }
        if let Some((_, lease)) = pinned
            && let Some((clients, _)) = selection
        {
            response.extensions_mut().insert(
                super::endpoint_selection::metrics::EndpointTransferGuard::new(
                    &clients.endpoints,
                    lease,
                ),
            );
        }
        if !response.status().is_redirection() || hop == MAX_DOWNLOAD_REDIRECTS
        {
            return Ok(response);
        }
        let Some(location) =
            response.headers().get(header::LOCATION).map(|value| {
                String::from_utf8_lossy(value.as_bytes()).into_owned()
            })
        else {
            return Ok(response);
        };
        if location.len() > MAX_REDIRECT_LOCATION_BYTES
            || location.chars().any(char::is_control)
        {
            return Ok(response);
        }
        let Ok(next) = response.url().join(&location) else {
            return Ok(response);
        };
        let next = retired_sources::normalize_parsed(next)?;
        if !is_allowed_download_redirect(&next) {
            return Ok(response);
        }
        let Some(mut cloned) = next_request else {
            return Ok(response);
        };
        let next = repair_official_cdn_redirect(&original, &next, &location)
            .unwrap_or(next);
        if !same_origin(cloned.url(), &next) {
            let names = cloned
                .headers()
                .keys()
                .filter(|name| {
                    is_sensitive_header(name.as_str())
                        || name.as_str() == DOWNLOAD_META_HEADER
                })
                .cloned()
                .collect::<Vec<_>>();
            for name in names {
                cloned.headers_mut().remove(name);
            }
        }
        *cloned.url_mut() = next;
        // A redirect must release its old-hop response and physical permit
        // before admission for the next authority is awaited.
        drop(response);
        request = cloned;
    }
    unreachable!()
}

pub(crate) async fn send_path_request_with_clients(
    route: &DownloadRoute,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    range_start: Option<u64>,
    range_end: Option<u64>,
    clients: &DownloadClients,
    redirect_target: Option<&AsyncMutex<Option<Url>>>,
) -> crate::Result<(reqwest::Response, String)> {
    let original = Url::parse(&retired_sources::normalize(&route.url)?)?;
    let mut current = match redirect_target {
        Some(target) => target
            .lock()
            .await
            .as_ref()
            .cloned()
            .unwrap_or_else(|| original.clone()),
        None => original.clone(),
    };
    current = retired_sources::normalize_parsed(current)?;
    let mut reused_redirect_target = current != original;
    for redirect_count in 0..=MAX_DOWNLOAD_REDIRECTS {
        let fallback_to_http1 = clients.force_http1
            || url_authority(current.as_str()).is_some_and(|authority| {
                authority_uses_http1_fallback_for(&authority, route.proxy)
            });
        let (system_client_for_hop, direct_client_for_hop): (
            &reqwest::Client,
            &reqwest::Client,
        ) = if fallback_to_http1 {
            (&clients.http1_system, &clients.http1_direct)
        } else {
            (&clients.system, &clients.direct)
        };
        let client = if route.proxy == ProxyPolicy::Direct {
            direct_client_for_hop
        } else {
            system_client_for_hop
        };
        let pinned = clients.endpoints.client_for(
            &current,
            route.proxy,
            clients,
            fallback_to_http1,
        );
        let client = pinned.as_ref().map_or(client, |(client, _)| client);
        let same_as_original = same_origin(&original, &current);
        let allow_sensitive = route.allow_sensitive_headers && same_as_original;
        let mut request = client.get(current.clone());
        if let Some((name, value)) = custom_header
            && (allow_sensitive || !is_sensitive_header(name))
            && (!name.eq_ignore_ascii_case(API_KEY_HEADER)
                || original.host_str() == Some(CURSEFORGE_API_HOST))
        {
            request = request.header(name, value);
        }
        if allow_sensitive && let Some(credentials) = credentials {
            request =
                request.header(AUTHORIZATION_HEADER, &credentials.session);
        }
        if !route.is_mirror
            && same_as_original
            && is_official_modrinth_download_url(original.as_str())
            && let Some(download_meta) = download_meta
        {
            request = request
                .header(DOWNLOAD_META_HEADER, download_meta.to_header_value());
        }
        let redirect_probe = (range_start.is_some()
            && current.host_str() == Some(CURSEFORGE_EDGE_HOST))
        .then(|| request.try_clone())
        .flatten();
        if let Some(range) = byte_range_header_value(range_start, range_end) {
            request = request
                .header(header::RANGE, range)
                .header(header::ACCEPT_ENCODING, IDENTITY_ENCODING);
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                if let Some((_, lease)) = &pinned {
                    clients.endpoints.record_failure(
                        lease,
                        super::endpoint_selection::invalid_certificate(&error),
                    );
                }
                tracing::warn!(
                    url = %sanitize_url_for_log(current.as_str()),
                    http1_fallback = fallback_to_http1,
                    cause = ?std::error::Error::source(&error),
                    "File download transport failed"
                );
                if let Some(host) = record_dns_connection_failure(route, &error)
                {
                    crate::util::fetch::prewarm_download_dns_for(
                        route.proxy,
                        &[&host],
                    )
                    .await;
                }
                if !fallback_to_http1
                    && redirect_count < MAX_DOWNLOAD_REDIRECTS
                    && is_h2_protocol_failure(&error)
                    && let Some(authority) = url_authority(current.as_str())
                {
                    tracing::warn!(
                        authority,
                        error = %error.without_url(),
                        "HTTP/2 download request failed; retrying over HTTP/1.1"
                    );
                    record_authority_h2_failure_for(&authority, route.proxy);
                    continue;
                }
                return Err(error.into());
            }
        };
        if let Some((_, lease)) = pinned {
            response.extensions_mut().insert(
                super::endpoint_selection::metrics::EndpointTransferGuard::new(
                    &clients.endpoints,
                    lease,
                ),
            );
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND
            && let Some(probe) = redirect_probe
            && let Ok(redirect) = probe.send().await
            && redirect.status().is_redirection()
        {
            tracing::debug!(
                host = current.host_str().unwrap_or_default(),
                "Recovered ForgeCDN redirect without Range header"
            );
            response = redirect;
        }
        if !response.status().is_redirection() {
            if reused_redirect_target
                && (response.status().is_client_error()
                    || response.status().is_server_error())
            {
                forget_effective_route_authority(route, &current);
                if let Some(target) = redirect_target {
                    let mut cached = target.lock().await;
                    if cached.as_ref() == Some(&current) {
                        *cached = None;
                    }
                }
                current = original.clone();
                reused_redirect_target = false;
                continue;
            }
            remember_effective_route_authority(route, current.as_str());
            if response.status().is_success()
                && current != original
                && let Some(target) = redirect_target
            {
                let mut cached = target.lock().await;
                if cached.is_none() {
                    *cached = Some(current.clone());
                }
            }
            tracing::debug!(
                original_url = %sanitize_url_for_log(&route.url),
                final_host = current.host_str().unwrap_or_default(),
                reused_redirect_target,
                http1_fallback = fallback_to_http1,
                "Resolved file download route"
            );
            return Ok((response, current.into()));
        }
        if redirect_count == MAX_DOWNLOAD_REDIRECTS {
            return Err(ErrorKind::OtherError(format!(
                "Too many redirects while downloading {}",
                route.url
            ))
            .into());
        }
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
            .ok_or_else(|| {
                ErrorKind::OtherError(format!(
                    "Redirect from {current} did not include a valid Location header"
                ))
            })?;
        if location.len() > MAX_REDIRECT_LOCATION_BYTES
            || location.chars().any(char::is_control)
        {
            return Err(ErrorKind::OtherError(format!(
                "Redirect from {current} included an unsafe Location header"
            ))
            .into());
        }
        let next = retired_sources::normalize_parsed(current.join(&location)?)?;
        if !is_allowed_download_redirect(&next) {
            return Err(ErrorKind::OtherError(format!(
                "Refusing insecure redirect from {current} to {next}"
            ))
            .into());
        }
        current = repair_official_cdn_redirect(&original, &next, &location)
            .unwrap_or(next);
    }
    unreachable!()
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_path_request(
    route: &DownloadRoute,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    range_start: Option<u64>,
    range_end: Option<u64>,
) -> crate::Result<(reqwest::Response, String)> {
    let clients = DownloadClients::for_request(
        &NO_REDIRECT_REQWEST_CLIENT,
        &DIRECT_REQWEST_CLIENT,
    );
    send_path_request_with_clients(
        route,
        custom_header,
        credentials,
        download_meta,
        range_start,
        range_end,
        &clients,
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fetch::{DownloadRouteSource, ResourceClass};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_headers(stream: &mut tokio::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let n = stream.read(&mut buffer).await.unwrap();
            assert_ne!(n, 0);
            request.extend_from_slice(&buffer[..n]);
        }
        String::from_utf8(request).unwrap()
    }

    async fn proxy_client(
        redirect: bool,
    ) -> (reqwest::Client, tokio::task::JoinHandle<String>) {
        proxy_client_with_location(
            redirect
                .then_some("https://cdn-alt.modrinth.com/data/file.jar?x=%2b"),
        )
        .await
    }

    async fn proxy_client_with_location(
        location: Option<&'static str>,
    ) -> (reqwest::Client, tokio::task::JoinHandle<String>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            if let Some(location) = location {
                let (mut first, _) = listener.accept().await.unwrap();
                let headers = read_headers(&mut first).await;
                assert!(headers.starts_with("GET http://origin.invalid/"));
                first.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            }
            let (mut stream, _) = listener.accept().await.unwrap();
            let headers = read_headers(&mut stream).await;
            stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            headers
        });
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        (client, task)
    }

    #[tokio::test]
    async fn forgecdn_range_404_recovers_redirect_and_caches_target() {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let target = format!("http://{address}/files/pack%20name%2B.zip");
        let expected_target = target.clone();
        let server = tokio::spawn(async move {
            for step in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let headers = read_headers(&mut stream).await;
                let lower = headers.to_ascii_lowercase();
                let response = match step {
                    0 => {
                        assert!(headers.starts_with("GET http://edge.forgecdn.net/files/pack%20name%2B.zip "));
                        assert!(lower.contains("range: bytes=2-5\r\n"));
                        assert!(lower.contains("authorization: secret\r\n"));
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
                    }
                    1 => {
                        assert!(
                            headers
                                .starts_with("GET http://edge.forgecdn.net/")
                        );
                        assert!(!lower.contains("\r\nrange:"));
                        format!(
                            "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                    }
                    _ => {
                        assert!(headers.starts_with(&format!("GET {target} ")));
                        assert!(lower.contains(if step == 2 {
                            "range: bytes=2-5\r\n"
                        } else {
                            "range: bytes=6-9\r\n"
                        }));
                        assert!(
                            lower.contains("accept-encoding: identity\r\n")
                        );
                        assert!(!lower.contains("authorization:"));
                        let range = if step == 2 { "2-5" } else { "6-9" };
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {range}/10\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata"
                        )
                    }
                };
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let mut clients =
            DownloadClients::build(&Default::default(), false, false).unwrap();
        clients.system = client.clone();
        clients.http1_system = client;
        let route = DownloadRoute {
            url: "http://edge.forgecdn.net/files/pack%20name%2B.zip".into(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::System,
        };
        let cached = AsyncMutex::new(None);
        let header = ("Authorization".into(), "secret".into());
        for start in [2, 6] {
            let (response, final_url) = send_path_request_with_clients(
                &route,
                Some(&header),
                None,
                None,
                Some(start),
                Some(start + 3),
                &clients,
                Some(&cached),
            )
            .await
            .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
            assert_eq!(final_url, expected_target);
            assert_eq!(response.bytes().await.unwrap(), "data");
        }
        assert_eq!(
            cached.lock().await.as_ref().unwrap().as_str(),
            expected_target
        );
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn forgecdn_range_404_keeps_original_error_without_redirect() {
        for probe_status in ["404 Not Found", "200 OK"] {
            let listener =
                tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                for step in 0..2 {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let headers =
                        read_headers(&mut stream).await.to_ascii_lowercase();
                    assert_eq!(headers.contains("\r\nrange:"), step == 0);
                    let status = if step == 0 {
                        "404 Not Found"
                    } else {
                        probe_status
                    };
                    stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
                }
            });
            let client = reqwest::Client::builder()
                .proxy(
                    reqwest::Proxy::all(format!("http://{address}")).unwrap(),
                )
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            let mut clients =
                DownloadClients::build(&Default::default(), false, false)
                    .unwrap();
            clients.system = client.clone();
            clients.http1_system = client;
            let route = DownloadRoute {
                url: "http://edge.forgecdn.net/files/missing.zip".into(),
                source: DownloadRouteSource::Official,
                is_mirror: false,
                allow_sensitive_headers: false,
                supports_range: true,
                proxy: ProxyPolicy::System,
            };
            let (response, final_url) = send_path_request_with_clients(
                &route,
                None,
                None,
                None,
                Some(0),
                Some(3),
                &clients,
                None,
            )
            .await
            .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
            assert_eq!(final_url, route.url);
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn retired_modrinth_streamed_requests_never_connect_to_the_retired_host()
     {
        let (client, proxy) = proxy_client(false).await;
        let route = DownloadRoute {
            url: "https://cdn-alt.modrinth.com/data/file.jar".into(),
            source: DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: true,
            supports_range: true,
            proxy: ProxyPolicy::System,
        };
        let clients = DownloadClients::for_request(&client, &client);
        assert!(
            send_path_request_with_clients(
                &route, None, None, None, None, None, &clients, None
            )
            .await
            .is_err()
        );
        let headers = tokio::time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
        assert!(
            headers.starts_with("CONNECT cdn.modrinth.com:443 "),
            "{headers}"
        );
        assert!(!headers.contains("cdn-alt"));
    }

    #[tokio::test]
    async fn retired_modrinth_redirects_never_connect_to_the_retired_host() {
        for streamed in [true, false] {
            let (client, proxy) = proxy_client(true).await;
            if streamed {
                let route = DownloadRoute {
                    url: "http://origin.invalid/".into(),
                    source: DownloadRouteSource::Official,
                    is_mirror: false,
                    allow_sensitive_headers: false,
                    supports_range: true,
                    proxy: ProxyPolicy::System,
                };
                let clients = DownloadClients::for_request(&client, &client);
                assert!(
                    send_path_request_with_clients(
                        &route, None, None, None, None, None, &clients, None
                    )
                    .await
                    .is_err()
                );
            } else {
                assert!(
                    send_cdn_request(client.get("http://origin.invalid/"))
                        .await
                        .is_err()
                );
            }
            let headers = tokio::time::timeout(Duration::from_secs(5), proxy)
                .await
                .unwrap()
                .unwrap();
            assert!(
                headers.starts_with("CONNECT cdn.modrinth.com:443 "),
                "{headers}"
            );
            assert!(!headers.contains("cdn-alt"));
        }
    }

    #[tokio::test]
    async fn retired_modrinth_byte_requests_never_connect_to_the_retired_host()
    {
        let (client, proxy) = proxy_client(false).await;
        assert!(
            send_cdn_request(
                client.get("https://cdn-alt.modrinth.com/data/file.jar")
            )
            .await
            .is_err()
        );
        let headers = tokio::time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
        assert!(
            headers.starts_with("CONNECT cdn.modrinth.com:443 "),
            "{headers}"
        );
        assert!(!headers.contains("cdn-alt"));
    }

    #[tokio::test]
    async fn modrinth_byte_redirects_strip_protected_headers_across_origins() {
        let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}/", first.local_addr().unwrap());
        let target = format!(
            "http://{}/file.jar?download=1",
            second.local_addr().unwrap()
        );
        let first_task = tokio::spawn(async move {
            let (mut stream, _) = first.accept().await.unwrap();
            let headers = read_headers(&mut stream).await.to_ascii_lowercase();
            assert!(headers.contains("authorization: secret"));
            stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        });
        let second_task = tokio::spawn(async move {
            let (mut stream, _) = second.accept().await.unwrap();
            let headers = read_headers(&mut stream).await.to_ascii_lowercase();
            assert!(!headers.contains("authorization:"));
            assert!(!headers.contains("cookie:"));
            assert!(!headers.contains("modrinth-download-meta:"));
            assert!(headers.contains("range: bytes=0-3"));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata").await.unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let response = send_cdn_request(
            client
                .get(origin)
                .header("Authorization", "secret")
                .header("Cookie", "secret=1")
                .header(DOWNLOAD_META_HEADER, "statistics")
                .header(header::RANGE, "bytes=0-3"),
        )
        .await
        .unwrap();
        assert_eq!(response.bytes().await.unwrap(), "data");
        first_task.await.unwrap();
        second_task.await.unwrap();
    }

    #[test]
    fn retired_modrinth_routes_keep_download_metadata_for_both_transports() {
        let routes = crate::util::fetch::resolve_download_routes_for(
            "https://cdn-alt.modrinth.com/data/file.jar",
            ResourceClass::Modrinth,
            crate::state::DownloadSourceMode::OfficialOnly,
        );
        let request = crate::util::fetch::DownloadRequest::new(
            &routes[0].url,
            ResourceClass::Modrinth,
        )
        .with_download_meta(DownloadMeta {
            reason: crate::util::fetch::DownloadReason::Standalone,
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            dependent_on: None,
        });
        let headers =
            super::super::h2_download::request_headers(&request, &routes[0]);
        assert!(headers.contains_key(DOWNLOAD_META_HEADER));
        assert!(is_official_modrinth_download_url(&routes[0].url));
    }

    #[tokio::test]
    async fn tianpao_downloads_and_redirects_connect_only_to_recovered_origins()
    {
        for (input, host) in [
            ("https://mod.tianpao.top/data/file.jar", "cdn.modrinth.com"),
            (
                "https://mod.tianpao.top/files/1/2/file.jar",
                "edge.forgecdn.net",
            ),
            (
                "https://mod.tianpao.top/media/attachments/icon.png",
                "media.forgecdn.net",
            ),
        ] {
            for redirect in [false, true] {
                for streamed in [false, true] {
                    let (client, proxy) =
                        proxy_client_with_location(redirect.then_some(input))
                            .await;
                    let url = if redirect {
                        "http://origin.invalid/"
                    } else {
                        input
                    };
                    if streamed {
                        let route = DownloadRoute {
                            url: url.into(),
                            source: DownloadRouteSource::Official,
                            is_mirror: false,
                            allow_sensitive_headers: false,
                            supports_range: true,
                            proxy: ProxyPolicy::System,
                        };
                        let clients =
                            DownloadClients::for_request(&client, &client);
                        assert!(
                            send_path_request_with_clients(
                                &route, None, None, None, None, None, &clients,
                                None
                            )
                            .await
                            .is_err()
                        );
                    } else {
                        assert!(
                            send_cdn_request(client.get(url)).await.is_err()
                        );
                    }
                    let headers =
                        tokio::time::timeout(Duration::from_secs(5), proxy)
                            .await
                            .unwrap()
                            .unwrap();
                    assert!(
                        headers.starts_with(&format!("CONNECT {host}:443 ")),
                        "{headers}"
                    );
                    assert!(!headers.contains("tianpao"));
                }
            }
        }
    }

    #[tokio::test]
    async fn tianpao_unknown_urls_fail_before_connecting() {
        for url in [
            "https://mod.tianpao.top/unknown",
            "https://mod.tianpao.top:8443/data/file",
        ] {
            let listener =
                tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let client = reqwest::Client::builder()
                .proxy(
                    reqwest::Proxy::all(format!(
                        "http://{}",
                        listener.local_addr().unwrap()
                    ))
                    .unwrap(),
                )
                .build()
                .unwrap();
            let error = send_cdn_request(client.get(url)).await.unwrap_err();
            assert!(error.to_string().contains("retired"));
            let route = DownloadRoute {
                url: url.into(),
                source: DownloadRouteSource::Official,
                is_mirror: false,
                allow_sensitive_headers: true,
                supports_range: true,
                proxy: ProxyPolicy::System,
            };
            let clients = DownloadClients::for_request(&client, &client);
            assert!(
                send_path_request_with_clients(
                    &route, None, None, None, None, None, &clients, None
                )
                .await
                .is_err()
            );
            assert_eq!(super::super::native::h2_ineligible_reason(&route), Some(super::super::native::NativeH2IneligibleReason::RetiredSource));
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(100),
                    listener.accept()
                )
                .await
                .is_err()
            );
        }
    }

    #[tokio::test]
    async fn tianpao_unknown_primary_does_not_grant_candidates_sensitive_headers()
     {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let headers = read_headers(&mut stream).await.to_ascii_lowercase();
            assert!(!headers.contains("authorization:"));
            assert!(!headers.contains("modrinth-download-meta:"));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata").await.unwrap();
        });
        let root = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let request = crate::util::fetch::DownloadRequest::new(
            "https://mod.tianpao.top/unknown",
            ResourceClass::Modpack,
        )
        .with_candidate_urls([url])
        .with_header("Authorization", "original-secret")
        .with_segmented_download(false)
        .with_integrity(crate::util::fetch::Integrity::sha1(
            sha1_smol::Sha1::from(b"data").hexdigest(),
        ));
        let result = crate::util::fetch::download_to_path(
            request,
            root.path().join("file"),
            &crate::util::fetch::FetchSemaphore(tokio::sync::Semaphore::new(1)),
            &pool,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.source, DownloadRouteSource::Alternate);
        assert_eq!(tokio::fs::read(result.path).await.unwrap(), b"data");
        server.await.unwrap();
    }
}
