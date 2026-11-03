#[cfg(test)]
use super::client;
use super::client::{
    DIRECT_REQWEST_CLIENT, DownloadClients, INSECURE_REQWEST_CLIENT,
    NO_REDIRECT_REQWEST_CLIENT, configured_client,
};
#[cfg(test)]
use super::file;
#[cfg(test)]
use super::file::response_validator;
use super::file::{response_status_error, sha1_async};
use super::model::{
    DownloadMeta, DownloadRoute, FetchProgressFn, ProxyPolicy, ResourceClass,
};
use super::policy::{
    fetch_retry_delay, header_requires_official_only, is_sensitive_header,
    record_dns_connection_failure, record_route_failure, record_route_success,
    requires_modrinth_auth, retry_after,
};
use super::prewarm_download_dns_for;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::{
    infer_resource_class, is_mrpack_url, is_official_modrinth_download_url,
    modrinth_request_kind, official_route, resolve_download_routes_for,
    route_host, sanitize_url_for_log, source_mode_for_resource,
};
use super::throttle::FetchSemaphore;
use crate::event::LoadingBarId;
use crate::event::emit::emit_loading;
use crate::util::download::constants::DOWNLOAD_META_HEADER;
use crate::util::download::constants::{
    AUTHORIZATION_HEADER, DOWNLOAD_PROGRESS_LOG_INTERVAL, FETCH_CDN,
    FETCH_CLEAN_RETRY, FETCH_COOLDOWN_THEN_RETRY, FETCH_EO_CACHE_STATUS,
    FETCH_HEDGE_FAILED, FETCH_MIRROR, FETCH_MISSING, FETCH_OFFICIAL,
    FETCH_RETRY_SAME_ROUTE, FETCH_STOP, FETCH_SWITCH_ROUTE,
    FETCH_TIMED_OUT_WAITING_FOR_CDN_CONNECTION_RESOURCES, FETCH_UNKNOWN,
    METADATA_ATTEMPT_BUDGET, METADATA_HEDGE_DELAY, METADATA_REQUEST_TIMEOUT,
    MODRINTH_CDN_ATTEMPT_TIMEOUT, MODRINTH_CDN_ATTEMPTS, RESOURCE_WAIT_TIMEOUT,
};
use crate::util::download::diagnostics::{
    attach_download_attempt_history, record_download_attempt_failure,
};
use crate::util::download::modrinth_redirect::is_official_redirect as is_official_modrinth_cdn_redirect;
use crate::util::download::retired_sources;
use crate::{ErrorKind, LabrinthError};
use bytes::Bytes;
use eyre::{Context, eyre};
use futures::StreamExt;
#[cfg(test)]
use reqwest::header;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use std::collections::VecDeque;
#[cfg(test)]
use std::error::Error;
use std::time::{self, Instant};

async fn fetch_validated_metadata_route(
    route: &DownloadRoute,
    resource: ResourceClass,
    sha1: Option<&str>,
    header: Option<(&str, &str)>,
    semaphore: &FetchSemaphore,
    client: &reqwest::Client,
    response_validator: &(dyn Fn(&Bytes) -> crate::Result<()> + Send + Sync),
) -> crate::Result<Bytes> {
    let download_clients = DownloadClients::for_request(
        &NO_REDIRECT_REQWEST_CLIENT,
        &DIRECT_REQWEST_CLIENT,
    );
    let route_client = match route.proxy {
        ProxyPolicy::System => client,
        ProxyPolicy::Direct => &download_clients.direct,
    };
    let mut request = route_client.get(&route.url);
    if matches!(resource, ResourceClass::Metadata | ResourceClass::Loader) {
        request = request.timeout(METADATA_REQUEST_TIMEOUT);
    }
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    let _permit = semaphore.0.acquire().await?;
    let request_started = Instant::now();
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            if let Some(host) = record_dns_connection_failure(route, &error) {
                prewarm_download_dns_for(route.proxy, &[&host]).await;
            }
            return Err(error.into());
        }
    };
    let ttfb = request_started.elapsed();
    let status = response.status();
    let remote_addr = response.remote_addr();
    let http_version = response.version();
    let response_retry_after = retry_after(&response);
    if !status.is_success() {
        record_route_failure(
            route,
            resource,
            (status == StatusCode::TOO_MANY_REQUESTS).then_some(
                response_retry_after.unwrap_or_else(|| fetch_retry_delay(1)),
            ),
        );
        return Err(
            response_status_error(response, &Method::GET, &route.url).await
        );
    }

    let transfer_started = Instant::now();
    let bytes = response.bytes().await?;
    if let Some(expected) = sha1 {
        let actual = sha1_async(bytes.clone()).await?;
        if actual.as_str() != expected {
            record_route_failure(route, resource, None);
            return Err(
                ErrorKind::HashError(expected.to_string(), actual).into()
            );
        }
    }
    if let Err(error) = response_validator(&bytes) {
        record_route_failure(route, resource, None);
        return Err(error);
    }

    record_route_success(
        route,
        resource,
        ttfb,
        bytes.len() as u64,
        transfer_started.elapsed(),
        remote_addr,
    );
    tracing::debug!(
        source = route.source.as_str(),
        url = %sanitize_url_for_log(&route.url),
        remote_addr = ?remote_addr,
        http_version = ?http_version,
        ttfb_ms = ttfb.as_millis(),
        "Completed hedged metadata route"
    );
    Ok(bytes)
}

pub(crate) async fn fetch_hedged_metadata(
    routes: &[DownloadRoute],
    resource: ResourceClass,
    sha1: Option<&str>,
    header: Option<(&str, &str)>,
    semaphore: &FetchSemaphore,
    client: &reqwest::Client,
    response_validator: &(dyn Fn(&Bytes) -> crate::Result<()> + Send + Sync),
) -> Result<Bytes, Vec<(usize, crate::Error)>> {
    let primary = async {
        fetch_validated_metadata_route(
            &routes[0],
            resource,
            sha1,
            header,
            semaphore,
            client,
            response_validator,
        )
        .await
        .map_err(|error| (0, error))
    };
    tokio::pin!(primary);
    let delay = tokio::time::sleep(METADATA_HEDGE_DELAY);
    tokio::pin!(delay);

    let first_error = tokio::select! {
        result = &mut primary => match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) => error,
        },
        _ = &mut delay => {
            let secondary = async {
                fetch_validated_metadata_route(
                    &routes[1],
                    resource,
                    sha1,
                    header,
                    semaphore,
                    client,
                    response_validator,
                )
                .await
                .map_err(|error| (1, error))
            };
            tokio::pin!(secondary);
            return tokio::select! {
                result = &mut primary => match result {
                    Ok(bytes) => Ok(bytes),
                    Err(primary_error) => secondary.await
                        .map_err(|secondary_error| {
                            vec![primary_error, secondary_error]
                        }),
                },
                result = &mut secondary => match result {
                    Ok(bytes) => Ok(bytes),
                    Err(secondary_error) => primary.await
                        .map_err(|primary_error| {
                            vec![secondary_error, primary_error]
                        }),
                },
            };
        },
    };

    fetch_validated_metadata_route(
        &routes[1],
        resource,
        sha1,
        header,
        semaphore,
        client,
        response_validator,
    )
    .await
    .map_err(|secondary_error| vec![first_error, (1, secondary_error)])
}

#[tracing::instrument(skip_all)]
pub async fn fetch(
    url: &str,
    sha1: Option<&str>,
    download_meta: Option<&DownloadMeta>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
    fetch_advanced(
        Method::GET,
        url,
        sha1,
        None,
        None,
        download_meta,
        None,
        uri_path,
        semaphore,
    )
    .await
}

/// Downloads a file from its official source without applying mirror routes.

#[tracing::instrument(skip_all)]
pub async fn fetch_official(
    url: &str,
    sha1: Option<&str>,
    download_meta: Option<&DownloadMeta>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
    let client = configured_client().await?;
    fetch_advanced_with_client_and_progress(
        Method::GET,
        url,
        sha1,
        None,
        None,
        download_meta,
        None,
        uri_path,
        semaphore,
        &client,
        Some(crate::state::DownloadSourceMode::OfficialOnly),
        None,
        None,
        METADATA_ATTEMPT_BUDGET,
    )
    .await
}

#[tracing::instrument(skip_all)]
pub(crate) async fn fetch_metadata<T>(
    url: &str,
    parse: impl Fn(&Bytes) -> crate::Result<T> + Send + Sync,
    semaphore: &FetchSemaphore,
) -> crate::Result<T> {
    let validate = |bytes: &Bytes| parse(bytes).map(|_| ());
    let client = configured_client().await?;
    let bytes = tokio::time::timeout(
        METADATA_REQUEST_TIMEOUT,
        fetch_advanced_with_client_and_progress(
            Method::GET,
            url,
            None,
            None,
            None,
            None,
            None,
            None,
            semaphore,
            &client,
            None,
            None,
            Some(&validate),
            METADATA_ATTEMPT_BUDGET,
        ),
    )
    .await
    .map_err(|_| {
        ErrorKind::OtherError(format!(
            "Metadata request timed out: {}",
            sanitize_url_for_log(url)
        ))
        .as_error()
    })??;
    parse(&bytes)
}

#[tracing::instrument(skip_all)]
pub async fn fetch_json<T>(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let validate_json = |bytes: &Bytes| -> crate::Result<()> {
        serde_json::from_slice::<T>(bytes)
            .map(|_| ())
            .map_err(Into::into)
    };
    let client = configured_client().await?;
    let result = fetch_advanced_with_client_and_progress(
        method,
        url,
        sha1,
        json_body,
        None,
        None,
        None,
        uri_path,
        semaphore,
        &client,
        None,
        None,
        Some(&validate_json),
        METADATA_ATTEMPT_BUDGET,
    )
    .await?;
    Ok(serde_json::from_slice(&result)?)
}

/// Like [`fetch_json`], but rejects responses that are empty JSON arrays.
///
/// Mirrors can serve an empty array for collection endpoints they have not
/// synced (e.g. `tag/game_version`). Treating that as a valid response would
/// poison the cache with an empty collection, so collection fetches validate
/// that the response actually contains data and fall back to the next source.

#[tracing::instrument(skip_all)]
pub async fn fetch_json_nonempty<T>(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let validate_json = |bytes: &Bytes| -> crate::Result<()> {
        let parsed: serde_json::Value = serde_json::from_slice(bytes)?;
        if parsed.as_array().is_some_and(|array| array.is_empty()) {
            return Err(ErrorKind::OtherError(format!(
                "Expected a non-empty JSON collection from {url}, got an empty array"
            ))
            .into());
        }
        serde_json::from_slice::<T>(bytes)
            .map(|_| ())
            .map_err(Into::into)
    };
    let client = configured_client().await?;
    let result = fetch_advanced_with_client_and_progress(
        method,
        url,
        sha1,
        json_body,
        None,
        None,
        None,
        uri_path,
        semaphore,
        &client,
        None,
        None,
        Some(&validate_json),
        METADATA_ATTEMPT_BUDGET,
    )
    .await?;
    Ok(serde_json::from_slice(&result)?)
}

/// Downloads a file with retry and checksum functionality, and a specific
/// [`reqwest::Client`].

#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub async fn fetch_advanced(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    header: Option<(&str, &str)>,
    download_meta: Option<&DownloadMeta>,
    loading_bar: Option<(&LoadingBarId, f64)>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
) -> crate::Result<Bytes> {
    fetch_advanced_with_client(
        method,
        url,
        sha1,
        json_body,
        header,
        download_meta,
        loading_bar,
        uri_path,
        semaphore,
        &INSECURE_REQWEST_CLIENT,
    )
    .await
}

/// Downloads a file with retry and checksum functionality

#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub async fn fetch_advanced_with_client(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    header: Option<(&str, &str)>,
    download_meta: Option<&DownloadMeta>,
    loading_bar: Option<(&LoadingBarId, f64)>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
    client: &reqwest::Client,
) -> crate::Result<Bytes> {
    fetch_advanced_with_client_and_progress(
        method,
        url,
        sha1,
        json_body,
        header,
        download_meta,
        loading_bar,
        uri_path,
        semaphore,
        client,
        None,
        None,
        None,
        METADATA_ATTEMPT_BUDGET,
    )
    .await
}

#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
async fn fetch_advanced_with_client_and_progress(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    header: Option<(&str, &str)>,
    download_meta: Option<&DownloadMeta>,
    loading_bar: Option<(&LoadingBarId, f64)>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
    client: &reqwest::Client,
    source_mode: Option<crate::state::DownloadSourceMode>,
    progress: Option<&mut FetchProgressFn<'_>>,
    response_validator: Option<
        &(dyn Fn(&Bytes) -> crate::Result<()> + Send + Sync),
    >,
    attempt_budget: usize,
) -> crate::Result<Bytes> {
    crate::util::download::proxy_context::with_clients(fetch_advanced_inner(
        method,
        url,
        sha1,
        json_body,
        header,
        download_meta,
        loading_bar,
        uri_path,
        semaphore,
        client,
        source_mode,
        progress,
        response_validator,
        attempt_budget,
    ))
    .await
}

#[allow(clippy::too_many_arguments)]
async fn fetch_advanced_inner(
    method: Method,
    url: &str,
    sha1: Option<&str>,
    json_body: Option<serde_json::Value>,
    header: Option<(&str, &str)>,
    download_meta: Option<&DownloadMeta>,
    loading_bar: Option<(&LoadingBarId, f64)>,
    uri_path: Option<&'static str>,
    semaphore: &FetchSemaphore,
    client: &reqwest::Client,
    source_mode: Option<crate::state::DownloadSourceMode>,
    mut progress: Option<&mut FetchProgressFn<'_>>,
    response_validator: Option<
        &(dyn Fn(&Bytes) -> crate::Result<()> + Send + Sync),
    >,
    attempt_budget: usize,
) -> crate::Result<Bytes> {
    let normalized_url = retired_sources::normalize(url)?;
    let url = normalized_url.as_ref();
    let resource = infer_resource_class(url);
    let mode =
        source_mode.unwrap_or_else(|| source_mode_for_resource(resource));
    let mut request_routes = resolve_download_routes_for(url, resource, mode);
    let modrinth_request_kind = modrinth_request_kind(url);
    let is_mrpack_download =
        modrinth_request_kind == Some(FETCH_CDN) && is_mrpack_url(url);
    let is_api_url = url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"));
    let requires_auth =
        is_api_url && requires_modrinth_auth(&method, header, uri_path);
    let creds = if requires_auth
        && header.as_ref().is_none_or(|x| !is_sensitive_header(x.0))
    {
        // The session is a stored record, so it is read through the store and
        // not through the connection the caller handed on.
        let state = crate::State::get().await?;
        crate::state::ModrinthCredentials::get_active(&state.data).await?
    } else {
        None
    };
    if method != Method::GET
        || header
            .as_ref()
            .is_some_and(|header| header_requires_official_only(header.0))
        || requires_auth
    {
        request_routes.retain(|route| !route.is_mirror);
    }
    if request_routes.is_empty() {
        request_routes.push(official_route(url, resource));
    }

    let mut total_attempts = 0;
    let mut last_error = None;
    let mut attempt_history = VecDeque::new();
    let hedge_is_safe = method == Method::GET
        && json_body.is_none()
        && download_meta.is_none()
        && progress.is_none()
        && creds.is_none()
        && header.is_none_or(|(name, _)| !is_sensitive_header(name))
        && response_validator.is_some();
    if hedge_is_safe
        && request_routes.len() > 1
        && attempt_budget >= 2
        && let Some(validate_response) = response_validator
    {
        match fetch_hedged_metadata(
            &request_routes,
            resource,
            sha1,
            header,
            semaphore,
            client,
            validate_response,
        )
        .await
        {
            Ok(bytes) => return Ok(bytes),
            Err(errors) => {
                total_attempts = errors.len();
                for (route_index, error) in errors {
                    let route = &request_routes[route_index];
                    record_download_attempt_failure(
                        &mut attempt_history,
                        route,
                        route_index + 1,
                        &error,
                        FETCH_HEDGE_FAILED,
                        None,
                        None,
                        None,
                    );
                    last_error = Some(error);
                }
            }
        }
    }

    for (route_index, route) in request_routes.iter().enumerate() {
        let request_url = &route.url;
        let log_request_url = sanitize_url_for_log(request_url);
        let is_mirror = route.is_mirror;
        let route_source = route.source;
        let request_target = if is_mirror {
            FETCH_MIRROR
        } else {
            FETCH_OFFICIAL
        };
        let has_next_route = route_index + 1 < request_routes.len();
        let download_meta_header = (!is_mirror
            && is_official_modrinth_download_url(request_url))
        .then(|| {
            download_meta.map(|m| {
                (DOWNLOAD_META_HEADER.to_string(), m.to_header_value())
            })
        })
        .flatten();

        let max_attempts = if modrinth_request_kind == Some(FETCH_CDN) {
            if is_mirror { 1 } else { MODRINTH_CDN_ATTEMPTS }
        } else {
            attempt_budget
        };
        let mut retried_server_error = false;
        let mut route_attempts = 0;
        while total_attempts < attempt_budget {
            let remaining_routes = request_routes.len() - route_index - 1;
            route_attempts += 1;
            let attempt = route_attempts;
            let has_more_attempts = attempt < max_attempts
                && total_attempts + remaining_routes < attempt_budget;
            total_attempts += 1;

            let started = time::Instant::now();
            tracing::debug!(
                method = %method,
                url = %log_request_url,
                source = route_source.as_str(),
                proxy = ?route.proxy,
                attempt = total_attempts,
                max_attempts = attempt_budget,
                "Starting metadata or API request attempt"
            );

            let protected_headers = creds.is_some()
                || download_meta_header.is_some()
                || header.is_some_and(|header| is_sensitive_header(header.0));
            let download_clients = DownloadClients::for_request(
                &NO_REDIRECT_REQWEST_CLIENT,
                &DIRECT_REQWEST_CLIENT,
            );
            let route_client = match (route.proxy, protected_headers) {
                (ProxyPolicy::System, _)
                    if modrinth_request_kind == Some(FETCH_CDN) =>
                {
                    &download_clients.system
                }
                (ProxyPolicy::System, false)
                    if is_mirror && modrinth_request_kind.is_some() =>
                {
                    &download_clients.system
                }
                (ProxyPolicy::System, false) => client,
                (ProxyPolicy::System, true) => &download_clients.system,
                (ProxyPolicy::Direct, false) => &download_clients.direct,
                (ProxyPolicy::Direct, true) => &download_clients.direct,
            };
            let mut req = route_client.request(method.clone(), request_url);
            if response_validator.is_some()
                && matches!(
                    resource,
                    ResourceClass::Metadata | ResourceClass::Loader
                )
            {
                req = req.timeout(METADATA_REQUEST_TIMEOUT);
            }
            if modrinth_request_kind == Some(FETCH_CDN) && !is_mrpack_download {
                req = req.timeout(MODRINTH_CDN_ATTEMPT_TIMEOUT);
            }

            if let Some(body) = json_body.clone() {
                req = req.json(&body);
            }

            if let Some(header) = header
                && (route.allow_sensitive_headers
                    || !is_sensitive_header(header.0))
            {
                req = req.header(header.0, header.1);
            }

            if route.allow_sensitive_headers
                && let Some(ref creds) = creds
            {
                req = req.header(AUTHORIZATION_HEADER, &creds.session);
            }

            if let Some((name, value)) = &download_meta_header {
                tracing::debug!("Sending download analytics: {value}");
                req = req.header(name.as_str(), value.as_str());
            }

            let native_budget = if modrinth_request_kind == Some(FETCH_CDN)
                && method == Method::GET
            {
                Some(
                    tokio::time::timeout(
                        RESOURCE_WAIT_TIMEOUT,
                        crate::util::download::native_budget::acquire(route),
                    )
                    .await
                    .map_err(|_| {
                        ErrorKind::DownloadResourceWaitTimeout(
                            FETCH_TIMED_OUT_WAITING_FOR_CDN_CONNECTION_RESOURCES
                                .into(),
                        )
                    })??,
                )
            } else {
                None
            };
            let permit = semaphore.0.acquire().await?;
            let request_started = Instant::now();
            let result = if let Some(native_budget) = native_budget {
                crate::util::download::native_request::send_cdn_request_with_budget(
                    req,
                    &download_clients,
                    route.proxy,
                    native_budget,
                )
                .await
            } else {
                req.send().await.map_err(crate::Error::from)
            };
            let ttfb = request_started.elapsed();
            match result {
                Ok(resp) => {
                    // Consuming reqwest helpers drop response extensions
                    // before polling their body. Retain admission through
                    // bytes()/bytes_stream()/JSON consumption and cancellation.
                    let _physical_budget = resp
                        .extensions()
                        .get::<crate::util::download::native_request::ResponseBudget>(
                        )
                        .cloned();
                    let status = resp.status();
                    let remote_addr = resp.remote_addr();
                    let http_version = resp.version();
                    let retry_after = retry_after(&resp);
                    if status.is_redirection() {
                        if is_mirror
                            && has_next_route
                            && modrinth_request_kind.is_some()
                        {
                            let status = resp.status();
                            let redirect_url = resp
                                .headers()
                                .get(reqwest::header::LOCATION)
                                .and_then(|value| value.to_str().ok())
                                .map(str::to_string);
                            let cache_status = resp
                                .headers()
                                .get(FETCH_EO_CACHE_STATUS)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or(FETCH_UNKNOWN);
                            let redirects_to_official =
                                is_official_modrinth_cdn_redirect(
                                    redirect_url.as_deref(),
                                );
                            let log_redirect_url = redirect_url
                                .as_deref()
                                .map(sanitize_url_for_log)
                                .unwrap_or_else(|| FETCH_MISSING.to_string());
                            if redirects_to_official {
                                tracing::warn!(
                                    mirror_status = "cache_miss",
                                    request_target,
                                    source = route_source.as_str(),
                                    mirror_url = %log_request_url,
                                    redirect_url = %log_redirect_url,
                                    cache_status,
                                    attempt,
                                    max_attempts,
                                    status = status.as_u16(),
                                    elapsed_ms = started.elapsed().as_millis(),
                                    "Modrinth mirror redirected to official CDN; falling back to official source"
                                );
                            } else {
                                tracing::warn!(
                                    mirror_status = "redirect_unresolved",
                                    request_target,
                                    source = route_source.as_str(),
                                    mirror_url = %log_request_url,
                                    redirect_url = %log_redirect_url,
                                    cache_status,
                                    attempt,
                                    max_attempts,
                                    status = status.as_u16(),
                                    elapsed_ms = started.elapsed().as_millis(),
                                    "Modrinth mirror returned an unresolved redirect; falling back to official source"
                                );
                            }
                        }
                        drop(permit);
                        record_route_failure(route, resource, None);
                        let error: crate::Error = ErrorKind::OtherError(
                            format!(
                                "Refusing to automatically forward protected headers while redirecting {log_request_url}"
                            ),
                        )
                        .into();
                        record_download_attempt_failure(
                            &mut attempt_history,
                            route,
                            total_attempts,
                            &error,
                            FETCH_SWITCH_ROUTE,
                            Some(status),
                            remote_addr,
                            Some(http_version),
                        );
                        last_error = Some(error);
                        break;
                    }
                    if status.is_client_error() || status.is_server_error() {
                        record_route_failure(
                            route,
                            resource,
                            (status == StatusCode::TOO_MANY_REQUESTS)
                                .then_some(retry_after.unwrap_or_else(|| {
                                    fetch_retry_delay(total_attempts)
                                })),
                        );
                        let route_error: crate::Error = if let Ok(mut error) =
                            resp.json::<LabrinthError>().await
                        {
                            error.status = Some(status.as_u16());
                            error.method = Some(method.as_str().to_string());
                            error.url = Some(log_request_url.clone());
                            error.route = uri_path.map(str::to_string);
                            ErrorKind::LabrinthError(error).into()
                        } else {
                            ErrorKind::HttpError {
                                status: status.as_u16(),
                                method: method.as_str().to_string(),
                                url: log_request_url.clone(),
                            }
                            .into()
                        };
                        let route_error_message = route_error.to_string();
                        drop(permit);
                        let retry_rate_limited = status
                            == StatusCode::TOO_MANY_REQUESTS
                            && !has_next_route
                            && has_more_attempts;
                        let retry_server_error = status.is_server_error()
                            && !retried_server_error
                            && has_more_attempts
                            && total_attempts + remaining_routes
                                < attempt_budget;
                        let decision = if retry_rate_limited {
                            FETCH_COOLDOWN_THEN_RETRY
                        } else if retry_server_error {
                            FETCH_RETRY_SAME_ROUTE
                        } else if has_next_route {
                            FETCH_SWITCH_ROUTE
                        } else {
                            FETCH_STOP
                        };
                        record_download_attempt_failure(
                            &mut attempt_history,
                            route,
                            total_attempts,
                            &route_error,
                            decision,
                            Some(status),
                            remote_addr,
                            Some(http_version),
                        );
                        last_error = Some(route_error);

                        if retry_rate_limited {
                            tokio::time::sleep(retry_after.unwrap_or_else(
                                || fetch_retry_delay(total_attempts),
                            ))
                            .await;
                            continue;
                        }

                        if retry_server_error {
                            retried_server_error = true;
                            tokio::time::sleep(fetch_retry_delay(
                                total_attempts,
                            ))
                            .await;
                            continue;
                        }

                        if has_next_route {
                            if modrinth_request_kind.is_some() {
                                tracing::warn!(
                                    request_target,
                                    source = route_source.as_str(),
                                    url = %log_request_url,
                                    attempt,
                                    max_attempts,
                                    status = status.as_u16(),
                                    elapsed_ms = started.elapsed().as_millis(),
                                    error = %route_error_message,
                                    "Modrinth mirror failed; falling back to official source"
                                );
                            } else {
                                tracing::warn!(
                                    url = %log_request_url,
                                    status = status.as_u16(),
                                    error = %route_error_message,
                                    "Mirror request failed; falling back to official source"
                                );
                            }
                            break;
                        }
                        if modrinth_request_kind.is_some() {
                            tracing::warn!(
                                request_target,
                                source = route_source.as_str(),
                                url = %log_request_url,
                                attempt,
                                max_attempts,
                                status = status.as_u16(),
                                elapsed_ms = started.elapsed().as_millis(),
                                error = %route_error_message,
                                "Modrinth official request failed"
                            );
                        }
                        break;
                    }

                    let response_url = resp.url().to_string();
                    let log_response_url = sanitize_url_for_log(&response_url);
                    if is_mirror && modrinth_request_kind == Some(FETCH_CDN) {
                        let cache_status = resp
                            .headers()
                            .get(FETCH_EO_CACHE_STATUS)
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or(FETCH_UNKNOWN);
                        tracing::info!(
                            mirror_status = "cache_hit",
                            request_target,
                            source = route_source.as_str(),
                            mirror_url = %log_request_url,
                            final_url = %log_response_url,
                            cache_status,
                            attempt,
                            max_attempts,
                            status = resp.status().as_u16(),
                            elapsed_ms = started.elapsed().as_millis(),
                            "Modrinth mirror resolved cached file"
                        );
                    }
                    let transfer_started = Instant::now();
                    let endpoint_feedback = resp.extensions().get::<crate::util::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
                    let bytes: eyre::Result<Bytes> = if loading_bar.is_some()
                        || progress.is_some()
                    {
                        let total_size = resp.content_length().unwrap_or(0);
                        let mut stream = resp.bytes_stream();

                        async {
                            let mut bytes = Vec::new();
                            let mut downloaded = 0_u64;
                            let mut next_progress_log =
                                DOWNLOAD_PROGRESS_LOG_INTERVAL;

                            while let Some(item) = stream.next().await {
                                let chunk = item.wrap_err_with(|| {
                                    eyre!(
                                        "failed to read response body from {log_request_url}"
                                    )
                                })?;

                                downloaded += chunk.len() as u64;
                                if let Some(feedback) = &endpoint_feedback { feedback.record(chunk.len(), crate::util::download::local_resources::network_pressure()); }
                                bytes.extend_from_slice(&chunk);

                                if modrinth_request_kind == Some(FETCH_CDN)
                                    && downloaded >= next_progress_log
                                {
                                    tracing::info!(
                                        request_target,
                                        source = route_source.as_str(),
                                        attempt,
                                        max_attempts,
                                        url = %log_request_url,
                                        final_url = %log_response_url,
                                        downloaded_bytes = downloaded,
                                        expected_bytes = total_size,
                                        "Modrinth CDN download progress"
                                    );
                                    while next_progress_log <= downloaded {
                                        next_progress_log = next_progress_log
                                            .saturating_add(
                                                DOWNLOAD_PROGRESS_LOG_INTERVAL,
                                            );
                                    }
                                }

                                if total_size > 0
                                    && let Some((bar, total)) = &loading_bar
                                {
                                    emit_loading(
                                        bar,
                                        (chunk.len() as f64
                                            / total_size as f64)
                                            * total,
                                        None,
                                    )?;
                                }

                                if let Some(progress) = progress.as_mut()
                                    && let Err(error) =
                                        progress(downloaded, total_size).await
                                    {
                                        tracing::warn!(%error, "Download progress callback failed");
                                    }
                            }

                            Ok(Bytes::from(bytes))
                        }
                        .await
                    } else {
                        resp.bytes().await.wrap_err_with(|| {
                            eyre!(
                                "failed to read response body from {log_request_url}"
                            )
                        })
                    };
                    if loading_bar.is_none()
                        && progress.is_none()
                        && let Ok(bytes) = &bytes
                        && let Some(feedback) = &endpoint_feedback
                    {
                        feedback.record(
                            bytes.len(),
                            crate::util::download::local_resources::network_pressure(
                            ),
                        );
                    }
                    drop(permit);

                    if let Ok(bytes) = bytes {
                        if let Some(sha1) = sha1 {
                            let hash = sha1_async(bytes.clone()).await?;
                            if &*hash != sha1 {
                                record_route_failure(route, resource, None);
                                let route_error: crate::Error =
                                    ErrorKind::HashError(
                                        sha1.to_string(),
                                        hash,
                                    )
                                    .into();
                                let decision =
                                    if !has_next_route && has_more_attempts {
                                        FETCH_CLEAN_RETRY
                                    } else if has_next_route {
                                        FETCH_SWITCH_ROUTE
                                    } else {
                                        FETCH_STOP
                                    };
                                record_download_attempt_failure(
                                    &mut attempt_history,
                                    route,
                                    total_attempts,
                                    &route_error,
                                    decision,
                                    Some(status),
                                    remote_addr,
                                    Some(http_version),
                                );
                                last_error = Some(route_error);
                                if !has_next_route && has_more_attempts {
                                    if modrinth_request_kind.is_some() {
                                        tracing::warn!(
                                            request_target,
                                            source = route_source.as_str(),
                                            url = %log_request_url,
                                            attempt,
                                            max_attempts,
                                            elapsed_ms = started.elapsed().as_millis(),
                                            "Modrinth checksum validation failed; retrying"
                                        );
                                    }
                                    tokio::time::sleep(fetch_retry_delay(
                                        total_attempts,
                                    ))
                                    .await;
                                    continue;
                                }
                                break;
                            }
                        }

                        if let Some(validate_response) = response_validator
                            && let Err(error) = validate_response(&bytes)
                        {
                            record_route_failure(route, resource, None);
                            let decision = if has_next_route {
                                FETCH_SWITCH_ROUTE
                            } else {
                                FETCH_STOP
                            };
                            record_download_attempt_failure(
                                &mut attempt_history,
                                route,
                                total_attempts,
                                &error,
                                decision,
                                Some(status),
                                remote_addr,
                                Some(http_version),
                            );
                            if has_next_route {
                                tracing::warn!(
                                    url = %log_request_url,
                                    error = %error,
                                    "Download route returned incompatible data; trying the next source"
                                );
                                last_error = Some(error);
                                break;
                            }
                            return Err(attach_download_attempt_history(
                                error,
                                &attempt_history,
                                total_attempts,
                                attempt_budget,
                            ));
                        }

                        tracing::trace!(
                            "Done downloading URL {log_request_url}"
                        );

                        record_route_success(
                            route,
                            resource,
                            ttfb,
                            bytes.len() as u64,
                            transfer_started.elapsed(),
                            remote_addr,
                        );
                        tracing::debug!(
                            source = route.source.as_str(),
                            remote_addr = ?remote_addr,
                            http_version = ?http_version,
                            dns_candidates = ?route_host(route).map(|host| {
                                crate::util::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
                            }),
                            ttfb_ms = ttfb.as_millis(),
                            "Recorded download route connection details"
                        );

                        return Ok(bytes);
                    } else if let Err(err) = bytes {
                        record_route_failure(route, resource, None);
                        let error_message = err.to_string();
                        let error: crate::Error = err.into();
                        let decision = if has_next_route {
                            FETCH_SWITCH_ROUTE
                        } else if has_more_attempts {
                            FETCH_RETRY_SAME_ROUTE
                        } else {
                            FETCH_STOP
                        };
                        record_download_attempt_failure(
                            &mut attempt_history,
                            route,
                            total_attempts,
                            &error,
                            decision,
                            Some(status),
                            remote_addr,
                            Some(http_version),
                        );
                        last_error = Some(error);
                        if has_next_route {
                            if modrinth_request_kind.is_some() {
                                tracing::warn!(
                                    request_target,
                                    source = route_source.as_str(),
                                    url = %log_request_url,
                                    attempt,
                                    max_attempts,
                                    elapsed_ms = started.elapsed().as_millis(),
                                    error = %error_message,
                                    "Modrinth mirror response failed; falling back to official source"
                                );
                            } else {
                                tracing::warn!(
                                    url = %log_request_url,
                                    error = %error_message,
                                    "Mirror response failed; falling back to official source"
                                );
                            }
                            break;
                        }
                        if has_more_attempts {
                            if modrinth_request_kind.is_some() {
                                tracing::warn!(
                                    request_target,
                                    source = route_source.as_str(),
                                    url = %log_request_url,
                                    attempt,
                                    max_attempts,
                                    elapsed_ms = started.elapsed().as_millis(),
                                    error = %error_message,
                                    "Modrinth response body failed; retrying"
                                );
                            }
                            tokio::time::sleep(fetch_retry_delay(
                                total_attempts,
                            ))
                            .await;
                            continue;
                        }
                        break;
                    }
                }
                Err(err) => {
                    drop(permit);
                    if let ErrorKind::FetchError(source) = err.raw.as_ref()
                        && let Some(host) =
                            record_dns_connection_failure(route, source)
                    {
                        prewarm_download_dns_for(route.proxy, &[&host]).await;
                    }
                    record_route_failure(route, resource, None);
                    let error_message = err.to_string();
                    let error: crate::Error = err;
                    let decision = if has_next_route {
                        FETCH_SWITCH_ROUTE
                    } else if has_more_attempts {
                        FETCH_RETRY_SAME_ROUTE
                    } else {
                        FETCH_STOP
                    };
                    record_download_attempt_failure(
                        &mut attempt_history,
                        route,
                        total_attempts,
                        &error,
                        decision,
                        None,
                        None,
                        None,
                    );
                    last_error = Some(error);
                    if has_next_route {
                        if modrinth_request_kind.is_some() {
                            tracing::warn!(
                                request_target,
                                source = route_source.as_str(),
                                url = %log_request_url,
                                attempt,
                                max_attempts,
                                elapsed_ms = started.elapsed().as_millis(),
                                error = %error_message,
                                "Modrinth mirror connection failed; falling back to official source"
                            );
                        } else {
                            tracing::warn!(
                                url = %log_request_url,
                                error = %error_message,
                                "Mirror connection failed; falling back to official source"
                            );
                        }
                        break;
                    }
                    if has_more_attempts {
                        if modrinth_request_kind.is_some() {
                            tracing::warn!(
                                request_target,
                                source = route_source.as_str(),
                                url = %log_request_url,
                                attempt,
                                max_attempts,
                                elapsed_ms = started.elapsed().as_millis(),
                                error = %error_message,
                                "Modrinth connection failed; retrying"
                            );
                        } else {
                            tracing::debug!(
                                attempt,
                                url = %log_request_url,
                                error = %error_message,
                                "Fetch failed; retrying"
                            );
                        }
                        tokio::time::sleep(fetch_retry_delay(total_attempts))
                            .await;
                        continue;
                    }
                    break;
                }
            }
        }
    }

    let error = last_error.unwrap_or_else(|| {
        ErrorKind::OtherError(format!(
            "Unable to download {url} from any source"
        ))
        .into()
    });
    Err(attach_download_attempt_history(
        error,
        &attempt_history,
        total_attempts,
        attempt_budget,
    ))
}

#[tracing::instrument(skip_all)]
pub async fn post_json(
    url: &str,
    json_body: serde_json::Value,
    semaphore: &FetchSemaphore,
) -> crate::Result<()> {
    let _permit = semaphore.0.acquire().await?;

    let mut req = INSECURE_REQWEST_CLIENT.post(url).json(&json_body);

    let state = crate::State::get().await?;
    if let Some(creds) =
        crate::state::ModrinthCredentials::get_active(&state.data).await?
    {
        req = req.header(AUTHORIZATION_HEADER, &creds.session);
    }

    req.send().await?.error_for_status()?;
    Ok(())
}
