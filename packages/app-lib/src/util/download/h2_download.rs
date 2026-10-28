//! HTTP/2 multiplexed file downloads over shared per-authority connections.
//!
//! General downloads to the same authority reuse one long-lived HTTP/2
//! connection. Minecraft assets use the dedicated batch multiplexer below,
//! which can add one sibling connection only after sustained saturation.

pub(crate) use super::constants::ASSET_BATCH_CONCURRENCY;
use super::constants::{
    ASSET_BATCH_RETRY_PASSES, ASSET_RESOURCE_WAIT_TIMEOUT,
    H2_RANGE_FILE_THRESHOLD, MAX_H2_RANGES_PER_FILE,
};
use super::h2_pool::{H2ConnectFailureKind, SharedH2Connection};
use super::retired_sources;
use crate::util::download::constants::{
    API_KEY_HEADER, CURSEFORGE_API_HOST, DOWNLOAD_FAIR_SHARE_DIVISOR,
    DOWNLOAD_META_HEADER, DOWNLOAD_SCHEDULER_INTERVAL, H2_DOWNLOAD_ASSET,
    H2_DOWNLOAD_AXOLOTL_LAUNCHER, H2_DOWNLOAD_BELOW_EXPECTATION,
    H2_DOWNLOAD_BYTES_0_0, H2_DOWNLOAD_CONTENT_RANGE,
    H2_DOWNLOAD_DOWNLOADED_ASSET_IS_EMPTY,
    H2_DOWNLOAD_DOWNLOADED_FILE_IS_EMPTY, H2_DOWNLOAD_EMPTY, H2_DOWNLOAD_GET,
    H2_DOWNLOAD_HTTP_2, H2_DOWNLOAD_INVALID_JAR, H2_DOWNLOAD_PROBE,
    H2_DOWNLOAD_RANGE_STREAM, H2_DOWNLOAD_SEGMENT,
    H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_DESTINATION_LOCK,
    H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_HTTP_2_STREAM_PERMIT,
    IDENTITY_ENCODING, MAX_DOWNLOAD_REDIRECTS, MAX_REDIRECT_LOCATION_BYTES,
    PARTIAL_FILE_SUFFIX,
};
use crate::util::fetch;
use crate::util::fetch::{
    DownloadRequest, DownloadResult, DownloadRoute, DownloadRouteSource,
    Integrity,
};
use futures::StreamExt;
use http::header::{ACCEPT_ENCODING, LOCATION, RANGE, USER_AGENT};
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use std::path::Path;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use url::Url;

/// Outcome of attempting a multiplexed download.
pub(crate) enum H2DownloadOutcome {
    /// The download completed through the multiplexed path.
    Completed(DownloadResult),
    /// The install job canceled this transfer; do not enter fallback.
    Canceled,
    /// The multiplexed path cannot be used; the caller should fall back to
    /// the legacy path.
    Fallback {
        failure: H2DownloadFailure,
        preserve_partial: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum H2DownloadFailure {
    Ineligible(&'static str),
    Connect,
    Tls,
    /// H2 transfer failure with unknown scope; never proves connection failure.
    Protocol,
    StreamReset,
    Http,
    RangeUnsupported,
    IndependentRanges(u64),
    Integrity,
    Content,
    Io,
    Slow,
    EndpointChanged,
}

impl H2DownloadFailure {
    pub(crate) const fn should_cooldown_authority(self) -> bool {
        matches!(self, Self::Protocol)
    }

    pub(crate) const fn is_transfer_failure(self) -> bool {
        matches!(
            self,
            Self::Connect | Self::Tls | Self::Protocol | Self::StreamReset
        )
    }

    pub(crate) const fn integrity_failure(self) -> bool {
        matches!(self, Self::Integrity | Self::Content)
    }
}

/// Attempts to download `request` as one stream on a shared HTTP/2 connection.
pub(crate) async fn try_download_via_h2(
    request: &DownloadRequest,
    route: &DownloadRoute,
    destination: &Path,
    part_path: &Path,
    policy: super::native::NativeH2Policy,
    semaphore: &fetch::FetchSemaphore,
) -> H2DownloadOutcome {
    if request
        .cancellation
        .as_ref()
        .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
    {
        return H2DownloadOutcome::Canceled;
    }
    if let Some(reason) = super::native::h2_ineligible_reason(route) {
        return H2DownloadOutcome::Fallback {
            failure: H2DownloadFailure::Ineligible(reason.as_str()),
            preserve_partial: false,
        };
    }
    let connection = match connect_authority(
        route,
        request.resource,
        true,
        policy.allow_cold_connection,
    )
    .await
    {
        Ok(connection) => connection,
        Err(failure) => {
            return H2DownloadOutcome::Fallback {
                failure,
                preserve_partial: false,
            };
        }
    };
    let connection_activity = connection.track_stream();
    let Ok(uri) = route.url.parse::<Uri>() else {
        return H2DownloadOutcome::Fallback {
            failure: H2DownloadFailure::Http,
            preserve_partial: false,
        };
    };

    let integrity = request.integrity.clone();
    let expected_size = integrity.size;

    fetch::record_install_download_started(request, route, 1, 1).await;

    // When the size is known (Modrinth metadata provides it) skip the probe
    // entirely: small files fetch the body directly, large files split into
    // range streams right away. The probe is only used when the size must be
    // discovered from the server.
    let total_size = if let Some(size) = expected_size {
        size
    } else {
        let _probe_stream_permit = match tokio::time::timeout(
            ASSET_RESOURCE_WAIT_TIMEOUT,
            super::h2_stream_budget::acquire_download(
                route,
                Some(semaphore),
                false,
            ),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) | Err(_) => {
                return H2DownloadOutcome::Fallback {
                    failure: H2DownloadFailure::Connect,
                    preserve_partial: false,
                };
            }
        };
        let mut probe_headers = request_headers(request, route);
        probe_headers
            .insert(RANGE, HeaderValue::from_static(H2_DOWNLOAD_BYTES_0_0));
        probe_headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static(IDENTITY_ENCODING),
        );

        let (response, mut probe_body) =
            match open_stream(&connection, &uri, probe_headers).await {
                Ok(pair) => pair,
                Err(error) => {
                    tracing::debug!(
                        url = %fetch::sanitize_url_for_log(&request.url),
                        error = %error,
                        "HTTP/2 probe failed; falling back to legacy download"
                    );
                    return H2DownloadOutcome::Fallback {
                        failure: classify_download_error(&error),
                        preserve_partial: false,
                    };
                }
            };

        let status = response.status();
        let headers = response.headers();
        super::native_breaker::record_response_status_failure(
            route,
            status.as_u16(),
            fetch::retry_after_headers(headers),
        );
        let total_size = parse_content_range_total(headers)
            .or_else(|| parse_content_length(headers));
        if status != StatusCode::PARTIAL_CONTENT {
            // A server ignoring the probe may send the whole Java archive.
            // Cancel that body before the fallback downloads it again.
            return H2DownloadOutcome::Fallback {
                failure: H2DownloadFailure::Http,
                preserve_partial: false,
            };
        }
        // Drain the probe body so the stream slot is released.
        drain_body(&mut probe_body).await;
        drop(probe_body);

        let Some(total_size) = total_size else {
            return H2DownloadOutcome::Fallback {
                failure: H2DownloadFailure::Http,
                preserve_partial: false,
            };
        };
        if total_size == 0 {
            return H2DownloadOutcome::Fallback {
                failure: H2DownloadFailure::Content,
                preserve_partial: false,
            };
        }
        total_size
    };

    if request.prefers_independent_ranges(total_size)
        && route.supports_range
        && fetch::range_splitting_allowed(route)
    {
        tracing::debug!(
            url = %fetch::sanitize_url_for_log(&route.url),
            total_size,
            "Moving verified large file from HTTP/2 probe to independent ranges"
        );
        return H2DownloadOutcome::Fallback {
            failure: H2DownloadFailure::IndependentRanges(total_size),
            preserve_partial: false,
        };
    }
    if let Some(concurrency) = request.h2_range_concurrency {
        drop(connection_activity);
        let configured = fetch::configured_semaphore_limit(semaphore);
        let disk_limit = super::local_resources::range_limit(part_path).await;
        let concurrency =
            h2_range_limit(total_size, concurrency, configured, disk_limit);
        return super::h2_range::download(
            &connection,
            &uri,
            request,
            route,
            destination,
            part_path,
            total_size,
            concurrency,
            Some(semaphore),
        )
        .await;
    }
    record_install_stage(
        request,
        crate::install::DownloadItemStatus::WaitingForResource,
    )
    .await;
    let stream_wait = tokio::time::timeout(
        ASSET_RESOURCE_WAIT_TIMEOUT,
        super::h2_stream_budget::acquire_download(
            route,
            Some(semaphore),
            false,
        ),
    );
    let stream_wait_started = Instant::now();
    let stream_result = if let Some(cancellation) =
        request.cancellation.as_ref()
    {
        tokio::select! {
            _ = cancellation.cancelled() => return H2DownloadOutcome::Canceled,
            result = stream_wait => result,
        }
    } else {
        stream_wait.await
    };
    let _stream_permit = match stream_result {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) | Err(_) => {
            return H2DownloadOutcome::Fallback {
                failure: H2DownloadFailure::Connect,
                preserve_partial: false,
            };
        }
    };
    tracing::debug!(
        route = %fetch::sanitize_url_for_log(&route.url),
        resource = "h2_stream",
        wait_ms = stream_wait_started.elapsed().as_millis(),
        "Acquired native H2 stream resource"
    );
    record_install_stage(
        request,
        crate::install::DownloadItemStatus::Downloading,
    )
    .await;
    let result = single_stream(
        &connection,
        &uri,
        request,
        route,
        destination,
        part_path,
        &integrity,
        total_size,
        policy,
        _stream_permit,
        connection_activity,
        semaphore,
    )
    .await;
    match result {
        Ok(result) => H2DownloadOutcome::Completed(result),
        Err(error) => {
            if request
                .cancellation
                .as_ref()
                .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
            {
                return H2DownloadOutcome::Canceled;
            }
            let failure = classify_download_error(&error);
            if matches!(
                failure,
                H2DownloadFailure::Protocol
                    | H2DownloadFailure::Connect
                    | H2DownloadFailure::Tls
            ) {
                connection.record_stream_failure();
            }
            if failure == H2DownloadFailure::Protocol {
                connection.retire();
            }
            tracing::debug!(
                url = %fetch::sanitize_url_for_log(&request.url),
                error = %error,
                "Multiplexed download failed; falling back to legacy download"
            );
            H2DownloadOutcome::Fallback {
                failure,
                preserve_partial: integrity.supports_resume()
                    && matches!(
                        failure,
                        H2DownloadFailure::Protocol
                            | H2DownloadFailure::Connect
                            | H2DownloadFailure::StreamReset
                            | H2DownloadFailure::Slow
                            | H2DownloadFailure::EndpointChanged
                    ),
            }
        }
    }
}

fn h2_range_limit(
    size: u64,
    requested: usize,
    global_limit: usize,
    disk_limit: usize,
) -> usize {
    let size_limit = if size >= H2_RANGE_FILE_THRESHOLD {
        MAX_H2_RANGES_PER_FILE
    } else {
        1
    };
    requested
        .max(1)
        .min(size_limit)
        .min((global_limit / DOWNLOAD_FAIR_SHARE_DIVISOR).max(1))
        .min(disk_limit.max(1))
}

#[cfg(test)]
mod range_policy_tests {
    use super::*;

    #[test]
    fn large_ranges_reserve_global_capacity_and_respect_disk_limit() {
        assert_eq!(h2_range_limit(128 * 1024 * 1024, 16, 64, 8), 8);
        assert_eq!(h2_range_limit(128 * 1024 * 1024, 16, 4, 8), 2);
        assert_eq!(h2_range_limit(128 * 1024 * 1024, 16, 64, 1), 1);
        assert_eq!(h2_range_limit(1024 * 1024, 16, 64, 8), 1);
    }
}

async fn connect_authority(
    route: &DownloadRoute,
    resource: fetch::ResourceClass,
    reserve_native_budget: bool,
    allow_cold_connection: bool,
) -> Result<Arc<SharedH2Connection>, H2DownloadFailure> {
    let authority =
        fetch::url_authority(&route.url).ok_or(H2DownloadFailure::Http)?;
    match super::h2_pool::shared_connection(
        route,
        reserve_native_budget,
        allow_cold_connection,
        resource != fetch::ResourceClass::Metadata,
    )
    .await
    {
        Ok(connection) => Ok(connection),
        Err(error) => {
            tracing::debug!(
                authority,
                error = %error,
                "Failed to establish shared HTTP/2 connection"
            );
            Err(match error.kind {
                H2ConnectFailureKind::Tcp => H2DownloadFailure::Connect,
                H2ConnectFailureKind::Tls => H2DownloadFailure::Tls,
                H2ConnectFailureKind::Protocol => H2DownloadFailure::Protocol,
            })
        }
    }
}

pub(crate) fn classify_download_error(
    error: &crate::Error,
) -> H2DownloadFailure {
    if fetch::is_integrity_error(error) {
        return H2DownloadFailure::Integrity;
    }
    match error.raw.as_ref() {
        crate::ErrorKind::FetchError(source) => {
            if source.status().is_some() {
                H2DownloadFailure::Http
            } else if source.is_timeout() && !source.is_connect() {
                H2DownloadFailure::Slow
            } else if source.is_connect()
                && super::endpoint_selection::invalid_certificate(source)
            {
                H2DownloadFailure::Tls
            } else {
                H2DownloadFailure::Connect
            }
        }
        crate::ErrorKind::DownloadH2Error(error) => {
            if error.is_reset() {
                H2DownloadFailure::StreamReset
            } else if error.is_io() {
                H2DownloadFailure::Connect
            } else {
                H2DownloadFailure::Protocol
            }
        }
        crate::ErrorKind::DownloadEndpointChanged => {
            H2DownloadFailure::EndpointChanged
        }
        crate::ErrorKind::DownloadTransferSlow
        | crate::ErrorKind::DownloadStreamStalled { .. } => {
            H2DownloadFailure::Slow
        }
        crate::ErrorKind::HttpError { .. }
        | crate::ErrorKind::LabrinthError(_) => H2DownloadFailure::Http,
        crate::ErrorKind::IOError(_) | crate::ErrorKind::StdIOError(_) => {
            H2DownloadFailure::Io
        }
        crate::ErrorKind::JSONError(_) => H2DownloadFailure::Content,
        crate::ErrorKind::NetworkError(message)
            if message.contains(H2_DOWNLOAD_BELOW_EXPECTATION) =>
        {
            H2DownloadFailure::Slow
        }
        crate::ErrorKind::NetworkError(message)
            if message.contains(H2_DOWNLOAD_HTTP_2)
                || message.contains(H2_DOWNLOAD_RANGE_STREAM) =>
        {
            H2DownloadFailure::Protocol
        }
        crate::ErrorKind::OtherError(message)
            if message.contains(H2_DOWNLOAD_HTTP_2)
                || message.contains(H2_DOWNLOAD_CONTENT_RANGE)
                || message.contains(H2_DOWNLOAD_SEGMENT) =>
        {
            H2DownloadFailure::Protocol
        }
        crate::ErrorKind::OtherError(message)
            if message.contains(H2_DOWNLOAD_EMPTY)
                || message.contains(H2_DOWNLOAD_INVALID_JAR) =>
        {
            H2DownloadFailure::Content
        }
        _ => H2DownloadFailure::Http,
    }
}

pub(crate) fn request_headers(
    request: &DownloadRequest,
    route: &DownloadRoute,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&crate::launcher_user_agent()).unwrap_or_else(
            |_| HeaderValue::from_static(H2_DOWNLOAD_AXOLOTL_LAUNCHER),
        ),
    );
    let route_host = Url::parse(&route.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string));
    if let Some((name, value)) = &request.header
        && (route.allow_sensitive_headers || !fetch::is_sensitive_header(name))
        && (!name.eq_ignore_ascii_case(API_KEY_HEADER)
            || route_host.as_deref() == Some(CURSEFORGE_API_HOST))
        && let Ok(name) = http::header::HeaderName::from_str(name)
        && let Ok(value) = HeaderValue::from_str(value)
    {
        headers.insert(name, value);
    }
    if route.source == DownloadRouteSource::Official
        && fetch::is_official_modrinth_download_url(&request.url)
        && let Some(download_meta) = &request.download_meta
        && let Ok(value) =
            HeaderValue::from_str(&download_meta.to_header_value())
    {
        headers.insert(DOWNLOAD_META_HEADER, value);
    }
    headers
}

fn parse_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

fn parse_content_range_total(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(http::header::CONTENT_RANGE)?.to_str().ok()?;
    let (_, total) = value.split_once('/')?;
    if total == "*" {
        return None;
    }
    total.parse().ok()
}

type StreamPair = (http::Response<()>, h2::RecvStream);

pub(crate) async fn open_stream(
    connection: &SharedH2Connection,
    uri: &Uri,
    headers: HeaderMap,
) -> crate::Result<StreamPair> {
    let mut request = http::Request::builder()
        .method(Method::GET)
        .uri(uri.clone())
        .version(http::Version::HTTP_2)
        .body(())
        .unwrap();
    *request.headers_mut() = headers;
    let response = connection.open(request).await?;
    let (parts, body) = response.into_parts();
    let response = http::Response::from_parts(parts, ());
    Ok((response, body))
}

async fn drain_body(stream: &mut h2::RecvStream) {
    loop {
        let chunk =
            match super::h2_receive::receive_chunk(stream, H2_DOWNLOAD_PROBE)
                .await
            {
                Ok(Some(chunk)) => chunk,
                Ok(None) | Err(_) => break,
            };
        if super::h2_receive::release_capacity(stream, chunk.len()).is_err() {
            break;
        }
    }
}

/// Downloads a single-stream body to `part_path`, hashing as it streams,
/// then verifies and finalises.
async fn single_stream(
    connection: &SharedH2Connection,
    uri: &Uri,
    request: &DownloadRequest,
    route: &DownloadRoute,
    destination: &Path,
    part_path: &Path,
    integrity: &Integrity,
    total_size: u64,
    policy: super::native::NativeH2Policy,
    permit: super::h2_stream_budget::H2DownloadPermit<'_>,
    connection_activity: super::h2_pool::H2StreamActivity,
    semaphore: &fetch::FetchSemaphore,
) -> crate::Result<DownloadResult> {
    let mut headers = request_headers(request, route);
    headers
        .insert(ACCEPT_ENCODING, HeaderValue::from_static(IDENTITY_ENCODING));

    let (response, mut stream) = open_stream(connection, uri, headers).await?;
    super::native_breaker::record_response_status_failure(
        route,
        response.status().as_u16(),
        fetch::retry_after_headers(response.headers()),
    );
    if !response.status().is_success() {
        return Err(crate::ErrorKind::HttpError {
            status: response.status().as_u16(),
            method: H2_DOWNLOAD_GET.to_string(),
            url: fetch::sanitize_url_for_log(uri.to_string().as_str()),
        }
        .into());
    }

    let mut hashers = fetch::IntegrityHashers::new_integrity_hashers(integrity);
    let mut file = super::stream_output::StreamOutput::new(
        tokio::fs::File::create(part_path).await?,
        part_path,
    );
    let mut downloaded = 0_u64;
    let activity = super::h2_receive::H2TransferActivity::begin();
    let mut progress_gate = super::h2_receive::H2ProgressGate::new(total_size);
    let mut slow_policy = super::native_slow::NativeSlowPolicy::for_http2(
        0,
        policy.expected_speed,
    );
    let transfer_started = Instant::now();
    let mut aggregate_window = super::h2_receive::ConnectionRateWindow::new(
        connection.bytes_transferred_for_scheduler(),
        transfer_started,
    );
    let mut timer = tokio::time::interval(DOWNLOAD_SCHEDULER_INTERVAL);
    let mut last_endpoint_speed = 0;
    let mut batch_tail =
        super::h2_receive::BatchTailWindow::new(transfer_started);
    timer.tick().await;
    loop {
        let chunk = tokio::select! {
            chunk = super::h2_receive::receive_chunk(&mut stream, "file") => chunk?,
            _ = async {
                match &request.cancellation {
                    Some(token) => token.cancelled().await,
                    None => std::future::pending().await,
                }
            } => return Err(crate::ErrorKind::OtherError("download canceled".into()).into()),
            _ = timer.tick(), if policy.abort_if_slow => {
                let persisted = downloaded.saturating_sub(file.pending_bytes());
                if progress_gate.should_report(persisted, total_size) {
                    record_install_progress(request, persisted, total_size).await;
                }
                let aggregate = aggregate_window.observe(connection.bytes_transferred_for_scheduler(), Instant::now());
                let pressure = super::local_resources::pressure(part_path);
                let can_recover_batch = request.supports_range_recovery()
                    && !pressure.blocks_expansion()
                    && semaphore.0.available_permits() >= super::constants::BATCH_DOWNLOAD_CONNECTION_FLOOR
                    && super::native_budget::available(route) >= super::constants::BATCH_DOWNLOAD_CONNECTION_FLOOR;
                if batch_tail.observe(downloaded, can_recover_batch, Instant::now())
                    && downloaded < total_size
                {
                    tracing::debug!(
                        url = %fetch::sanitize_url_for_log(&route.url),
                        physical_connections = super::native_budget::in_use(route),
                        active_streams = connection.active_streams(),
                        remaining_bytes = total_size - downloaded,
                        "Moving slow batch tail from shared HTTP/2 to an independent connection"
                    );
                    drop(stream);
                    file.flush().await?;
                    request.transfer_counters.commit(0, downloaded);
                    return Err(crate::ErrorKind::DownloadTransferSlow.into());
                }
                let mut decision = slow_policy.observe_with_pressure(
                    downloaded,
                    total_size.saturating_sub(downloaded),
                    pressure,
                );
                if connection.active_streams() > 1 {
                    if let Some(speed) = super::h2_receive::endpoint_speed(last_endpoint_speed,
                        connection.active_streams(), aggregate) {
                        if let super::native_slow::SlowDecision::Probe { .. } = decision {
                            decision = super::native_slow::SlowDecision::Probe { bytes_per_second: speed };
                        }
                    } else {
                        last_endpoint_speed = 0;
                        // One slow object is not evidence to change the host
                        // while the same socket serves healthy sibling streams.
                        if !matches!(decision, super::native_slow::SlowDecision::Idle { .. }) {
                            decision = super::native_slow::SlowDecision::Continue;
                        }
                    }
                }
                if let super::native_slow::SlowDecision::Probe { bytes_per_second } = decision {
                    last_endpoint_speed = bytes_per_second;
                }
                if !pressure.blocks_expansion()
                    && let Some(clients) = super::proxy_context::clients()
                    && clients.endpoints.recovery_lease(request, route, super::endpoint_selection::Protocol::H2,
                        connection.remote_addr, last_endpoint_speed, total_size.saturating_sub(downloaded)).is_some()
                {
                    drop(stream);
                    file.flush().await?;
                    request.transfer_counters.commit(0, downloaded);
                    return Err(crate::ErrorKind::DownloadEndpointChanged.into());
                }
                if matches!(decision, super::native_slow::SlowDecision::Probe { .. })
                    && let Some(clients) = super::proxy_context::clients()
                {
                    clients.endpoints.on_slow_region(request, route, &clients,
                        super::endpoint_selection::Protocol::H2,
                        connection.remote_addr.map(|address| address.ip()),
                        downloaded..total_size, Some(last_endpoint_speed), super::local_resources::pressure(part_path));
                    if clients.endpoints.is_probing() { continue; }
                }
                if matches!(decision, super::native_slow::SlowDecision::Probe { .. }
                    | super::native_slow::SlowDecision::Idle { .. }) {
                    tracing::warn!(
                        url = %fetch::sanitize_url_for_log(&route.url),
                        http_version = "HTTP/2",
                        remote_addr = ?connection.remote_addr,
                        downloaded_bytes = downloaded,
                        elapsed_ms = transfer_started.elapsed().as_millis(),
                        reason = ?decision,
                        "Native HTTP/2 download stayed slow; falling back to HTTP/1.1"
                    );
                    drop(stream);
                    file.flush().await?;
                    request.transfer_counters.commit(0, downloaded);
                    return Err(crate::ErrorKind::DownloadTransferSlow.into());
                }
                continue;
            }
        };
        let Some(chunk) = chunk else {
            break;
        };
        request.transfer_counters.received(chunk.len() as u64);
        file.write_chunk(&chunk).await?;
        hashers.update(&chunk);
        downloaded += chunk.len() as u64;
        request
            .transfer_counters
            .commit(0, downloaded.saturating_sub(file.pending_bytes()));
        connection.record_bytes(chunk.len());
        activity.record_bytes(chunk.len());
        super::h2_receive::release_capacity(&mut stream, chunk.len())?;
        let persisted = downloaded.saturating_sub(file.pending_bytes());
        if progress_gate.should_report(persisted, total_size) {
            record_install_progress(request, persisted, total_size).await;
        }
    }
    file.flush().await?;
    request.transfer_counters.commit(0, downloaded);
    record_install_progress(request, downloaded, total_size).await;
    drop(file);
    drop(permit);
    drop(connection_activity);
    let computed = hashers.finish(downloaded);
    record_install_stage(
        request,
        crate::install::DownloadItemStatus::Verifying,
    )
    .await;

    verify_and_finalize(
        part_path,
        destination,
        integrity,
        computed,
        downloaded,
        total_size,
    )
    .await?;

    Ok(DownloadResult {
        path: destination.to_path_buf(),
        url: uri.to_string(),
        source: route.source,
        size: downloaded,
        attempts: 1,
        fallback_count: 0,
        verified_sha1: request.integrity.sha1.clone(),
        verified_sha512: request.integrity.sha512.clone(),
        verified_file: None,
    })
}

pub(crate) async fn record_install_stage(
    request: &DownloadRequest,
    status: crate::install::DownloadItemStatus,
) {
    if let Some(tracking) = &request.install_tracking {
        let reporter = tracking.reporter.clone();
        let item_id = tracking.item_id.clone();
        let _ = reporter.record_download_stage(item_id, status).await;
    }
}

pub(crate) async fn record_install_progress(
    request: &DownloadRequest,
    downloaded: u64,
    total_size: u64,
) {
    if let Some(tracking) = &request.install_tracking {
        let reporter = tracking.reporter.clone();
        let item_id = tracking.item_id.clone();
        let _ = reporter
            .record_download_progress(item_id, downloaded, total_size)
            .await;
    }
}

async fn verify_and_finalize(
    part_path: &Path,
    destination: &Path,
    integrity: &Integrity,
    hashers: fetch::ComputedIntegrity,
    downloaded: u64,
    _expected_size: u64,
) -> crate::Result<()> {
    // The size check lives inside `verify_computed_integrity`: the hash is
    // authoritative whenever one is available, mirroring the legacy path.
    fetch::verify_computed_integrity(integrity, &hashers)?;
    fetch::validate_file_content(part_path, integrity.content).await?;
    if downloaded == 0 {
        return Err(crate::ErrorKind::OtherError(
            H2_DOWNLOAD_DOWNLOADED_FILE_IS_EMPTY.to_string(),
        )
        .into());
    }
    fetch::finalize_download(part_path, destination).await?;
    Ok(())
}

/// A single small file (a Minecraft asset object) to download over a shared
/// HTTP/2 connection. All items in one batch must share the same authority.
pub(crate) struct H2BatchAsset {
    /// Route-resolved URL for the asset.
    pub url: String,
    /// Destination for the object (`assets/objects/<hh>/<hash>`).
    pub destination: std::path::PathBuf,
    /// Legacy `resources/` copies to create after the object is committed.
    /// Several logical asset names may point to this one physical object.
    pub legacy_destinations: Vec<std::path::PathBuf>,
    /// Expected SHA-1 hash of the asset (also its file name).
    pub sha1: String,
    /// Expected size in bytes.
    pub size: u64,
    /// Number of logical index entries represented by this physical object.
    /// Progress remains index-based even when duplicate objects are coalesced.
    pub logical_items: u32,
}

/// Completion state for one physical asset object. A legacy-resource copy is
/// deliberately separate from fetching and committing the content-addressed
/// object: a local path error must not cause another GET for an object that is
/// already valid on disk.
enum AssetBatchItemOutcome {
    Completed {
        downloaded: bool,
    },
    LocalCopyFailed {
        downloaded: bool,
        error: crate::Error,
    },
    LocalObjectFailed {
        error: crate::Error,
    },
}

impl Clone for H2BatchAsset {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            destination: self.destination.clone(),
            legacy_destinations: self.legacy_destinations.clone(),
            sha1: self.sha1.clone(),
            size: self.size,
            logical_items: self.logical_items,
        }
    }
}

/// Selects every asset stream through the shared authority scheduler.
struct AssetBatchConnectionGroup {
    primary: Arc<SharedH2Connection>,
    route: DownloadRoute,
    reserve_native_budget: bool,
}

impl AssetBatchConnectionGroup {
    fn new(
        primary: Arc<SharedH2Connection>,
        route: &DownloadRoute,
        reserve_native_budget: bool,
    ) -> Self {
        Self {
            primary,
            route: route.clone(),
            reserve_native_budget,
        }
    }

    async fn connection_for(
        &self,
        route: &DownloadRoute,
        rescue: bool,
    ) -> Arc<SharedH2Connection> {
        match super::h2_pool::shared_batch_connection(
            route,
            self.reserve_native_budget,
        )
        .await
        {
            Ok(connection) => connection,
            Err(error) => {
                tracing::debug!(
                    rescue,
                    error = %error,
                    "Asset scheduler fell back to its primary H2 connection"
                );
                Arc::clone(&self.primary)
            }
        }
    }

    async fn connection(&self, rescue: bool) -> Arc<SharedH2Connection> {
        self.connection_for(&self.route, rescue).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_transfer_recovery_is_not_a_protocol_failure() {
        for error in [
            crate::ErrorKind::DownloadTransferSlow,
            crate::ErrorKind::DownloadStreamStalled {
                context: "range".into(),
            },
        ] {
            assert_eq!(
                classify_download_error(&error.into()),
                H2DownloadFailure::Slow
            );
        }
        assert_eq!(
            classify_download_error(
                &crate::ErrorKind::DownloadEndpointChanged.into()
            ),
            H2DownloadFailure::EndpointChanged
        );
        assert_eq!(
            classify_download_error(
                &crate::ErrorKind::DownloadH2Error(
                    h2::Reason::PROTOCOL_ERROR.into()
                )
                .into()
            ),
            H2DownloadFailure::Protocol
        );
    }

    #[test]
    fn slow_h2_transfers_do_not_poison_protocol_health() {
        assert!(!H2DownloadFailure::Slow.should_cooldown_authority());
        assert!(
            !H2DownloadFailure::EndpointChanged.should_cooldown_authority()
        );
        assert!(H2DownloadFailure::Protocol.should_cooldown_authority());
        assert!(!H2DownloadFailure::Http.should_cooldown_authority());
    }

    #[tokio::test]
    async fn reqwest_range_body_timeout_does_not_become_an_h2_protocol_failure()
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _bytes_read = stream.read(&mut request).await.unwrap();
            stream.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 65536\r\nContent-Range: bytes 0-65535/65536\r\n\r\nx").await.unwrap();
            std::future::pending::<()>().await;
        });
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            let client = reqwest::Client::builder()
                .no_proxy()
                .read_timeout(Duration::from_millis(100))
                .build()
                .unwrap();
            let mut response = client
                .get(format!("http://{address}/file"))
                .send()
                .await
                .unwrap();
            let error = loop {
                match response.chunk().await {
                    Err(error) => break error,
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        panic!("truncated test body unexpectedly completed")
                    }
                }
            };
            let failure = classify_download_error(&error.into());
            assert_eq!(failure, H2DownloadFailure::Slow);
            assert!(!failure.should_cooldown_authority());
        })
        .await;
        server.abort();
        result.unwrap();
    }

    #[tokio::test]
    async fn committed_asset_recovers_a_legacy_copy_without_redownloading() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("object");
        tokio::fs::write(&destination, b"already committed")
            .await
            .unwrap();
        let blocked_parent = temp.path().join("legacy-parent");
        tokio::fs::write(&blocked_parent, b"not a directory")
            .await
            .unwrap();
        let legacy = blocked_parent.join("resource");
        let item = H2BatchAsset {
            url: "https://resources.download.minecraft.net/aa/object".into(),
            destination: destination.clone(),
            legacy_destinations: vec![legacy.clone()],
            sha1: "unused-by-copy-test".into(),
            size: 17,
            logical_items: 1,
        };

        assert!(copy_asset_legacy_destinations(&item).await.is_err());
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"already committed"
        );

        tokio::fs::remove_file(&blocked_parent).await.unwrap();
        tokio::fs::create_dir(&blocked_parent).await.unwrap();
        copy_asset_legacy_destinations(&item).await.unwrap();

        assert_eq!(
            tokio::fs::read(&legacy).await.unwrap(),
            b"already committed"
        );
    }

    #[tokio::test]
    async fn blocked_object_parent_is_detected_before_asset_get() {
        let temp = tempfile::tempdir().unwrap();
        let blocked_parent = temp.path().join("object-parent");
        tokio::fs::write(&blocked_parent, b"not a directory")
            .await
            .unwrap();
        let destination = blocked_parent.join("object");
        let part_path = fetch::suffixed_path(&destination, ".part");

        assert!(prepare_asset_part_path(&destination).await.is_err());
        assert!(tokio::fs::metadata(&part_path).await.is_err());
    }
}

/// Downloads a batch of small files over a shared HTTP/2 connection group,
/// multiplexing up to `concurrency` logical workers. Physical H2 streams are
/// governed by the shared authority stream budget. The group begins with one
/// connection and expands the shared authority pool under sustained load.
/// Items that cannot be downloaded
/// after internal retries fail the batch instead of switching to per-file
/// network downloads.
pub(crate) async fn download_asset_batch_via_h2<F>(
    route: &DownloadRoute,
    items: Vec<H2BatchAsset>,
    concurrency: usize,
    apply_native_policy: bool,
    on_completed: F,
) -> crate::Result<()>
where
    F: Fn(H2BatchAsset) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync
        + 'static,
{
    super::proxy_context::with_clients(download_asset_batch_inner(
        route,
        items,
        concurrency,
        apply_native_policy,
        on_completed,
    ))
    .await
}

async fn download_asset_batch_inner<F>(
    route: &DownloadRoute,
    items: Vec<H2BatchAsset>,
    concurrency: usize,
    apply_native_policy: bool,
    on_completed: F,
) -> crate::Result<()>
where
    F: Fn(H2BatchAsset) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync
        + 'static,
{
    let concurrency = concurrency.max(1);
    if apply_native_policy
        && let Some(reason) = super::native::h2_ineligible_reason(route)
    {
        return Err(crate::ErrorKind::NetworkError(format!(
            "Minecraft asset route cannot use HTTP/2 batching: {reason:?}"
        ))
        .into());
    }
    // Local object I/O failures are deterministic per destination; remember the
    // first one and keep draining the batch so siblings already in flight or
    // still queued are not abandoned, then surface the error to the caller.
    let mut local_object_error: Option<crate::Error> = None;
    let connection = match connect_authority(
        route,
        fetch::ResourceClass::MinecraftAsset,
        apply_native_policy,
        true,
    )
    .await
    {
        Ok(connection) => connection,
        Err(failure) => {
            if apply_native_policy
                && failure.should_cooldown_authority()
                && let Some(authority) = fetch::url_authority(&route.url)
            {
                fetch::record_authority_h2_failure_for(&authority, route.proxy);
            }
            if apply_native_policy && failure.is_transfer_failure() {
                super::native_breaker::record_failure(route);
                fetch::record_route_health_failure(
                    route,
                    fetch::ResourceClass::MinecraftAsset,
                    None,
                );
            }
            return Err(crate::ErrorKind::NetworkError(format!(
                "Could not establish an HTTP/2 asset batch connection: {failure:?}"
            ))
            .into());
        }
    };
    let connections = Arc::new(AssetBatchConnectionGroup::new(
        connection,
        route,
        apply_native_policy,
    ));
    let route_authority = fetch::url_authority(&route.url);

    // Items whose URL targets a different authority cannot be multiplexed on
    // this connection; hand them straight back without wasting batch passes.
    let (mut items, mut failed): (Vec<_>, Vec<_>) = items
        .into_iter()
        .partition(|item| fetch::url_authority(&item.url) == route_authority);
    if !failed.is_empty() {
        tracing::warn!(
            items = failed.len(),
            route = %fetch::sanitize_url_for_log(&route.url),
            "Skipping {} assets whose resolved URL does not match the batch authority",
            failed.len(),
        );
    }

    let batch_started = std::time::Instant::now();
    let mut completed_bytes = 0_u64;
    let mut network_failures = 0_u32;
    let callback = Arc::new(on_completed);
    for pass in 0..ASSET_BATCH_RETRY_PASSES {
        if items.is_empty() {
            break;
        }
        let results = futures::stream::iter(items)
            .map(|item| {
                let connections = Arc::clone(&connections);
                let callback = callback.clone();
                async move {
                    let Ok(uri) = item.url.parse::<Uri>() else {
                        let error =
                            crate::Error::from(crate::ErrorKind::InputError(
                                format!("invalid asset URL: {}", item.url),
                            ));
                        return (item, Err(error));
                    };
                    let result = download_asset_item(
                        &connections,
                        &uri,
                        &item,
                        route,
                        apply_native_policy,
                        pass > 0,
                    )
                    .await;
                    if matches!(
                        &result,
                        Ok(AssetBatchItemOutcome::Completed { .. })
                    ) {
                        callback(item.clone()).await;
                    }
                    (item, result)
                }
            })
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await;
        items = Vec::new();
        for (item, result) in results {
            match result {
                Ok(AssetBatchItemOutcome::Completed { downloaded }) => {
                    if downloaded {
                        completed_bytes =
                            completed_bytes.saturating_add(item.size);
                    }
                }
                Ok(AssetBatchItemOutcome::LocalCopyFailed {
                    downloaded,
                    error,
                }) => {
                    if downloaded {
                        completed_bytes =
                            completed_bytes.saturating_add(item.size);
                    }
                    tracing::warn!(
                        url = %fetch::sanitize_url_for_log(&item.url),
                        destination = %item.destination.display(),
                        error = %error,
                        "Asset object is committed, but copying its legacy resource failed; retrying locally without another download"
                    );
                    // The ordinary fallback path sees the valid object and
                    // performs only the outstanding local copy. Do not spend
                    // another network retry pass on a local filesystem error.
                    failed.push(item);
                }
                Ok(AssetBatchItemOutcome::LocalObjectFailed { error }) => {
                    tracing::warn!(
                        url = %fetch::sanitize_url_for_log(&item.url),
                        destination = %item.destination.display(),
                        error = %error,
                        "Asset object failed local I/O; continuing to drain the batch"
                    );
                    if local_object_error.is_none() {
                        local_object_error = Some(error);
                    }
                }
                Err(error) => {
                    network_failures = network_failures.saturating_add(1);
                    tracing::debug!(
                        url = %fetch::sanitize_url_for_log(&item.url),
                        pass = pass + 1,
                        error = %error,
                        "Batch asset download failed"
                    );
                    if pass + 1 < ASSET_BATCH_RETRY_PASSES {
                        items.push(item);
                    } else {
                        failed.push(item);
                    }
                }
            }
        }
    }
    if apply_native_policy {
        if completed_bytes > 0 {
            fetch::record_route_transfer_success(
                route,
                fetch::ResourceClass::MinecraftAsset,
                completed_bytes,
                batch_started.elapsed(),
            );
        } else if network_failures > 0 {
            super::native_breaker::record_failure(route);
            fetch::record_route_health_failure(
                route,
                fetch::ResourceClass::MinecraftAsset,
                None,
            );
        }
    }
    if let Some(error) = local_object_error {
        return Err(error);
    }
    if !failed.is_empty() {
        tracing::warn!(
            items = failed.len(),
            retry_passes = ASSET_BATCH_RETRY_PASSES,
            source = route.source.as_str(),
            "{} assets exhausted all {ASSET_BATCH_RETRY_PASSES} batch retry passes on route {}",
            failed.len(),
            route.source.as_str(),
        );
        return Err(crate::ErrorKind::NetworkError(format!(
            "{} Minecraft assets failed in the HTTP/2 batch on route {}",
            failed.len(),
            route.source.as_str(),
        ))
        .into());
    }
    Ok(())
}

async fn prepare_asset_part_path(
    destination: &Path,
) -> crate::Result<std::path::PathBuf> {
    let part_path = fetch::suffixed_path(destination, PARTIAL_FILE_SUFFIX);
    if let Some(parent) = part_path.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    Ok(part_path)
}

async fn download_asset_item(
    connections: &AssetBatchConnectionGroup,
    uri: &Uri,
    item: &H2BatchAsset,
    route: &DownloadRoute,
    apply_native_policy: bool,
    rescue: bool,
) -> crate::Result<AssetBatchItemOutcome> {
    let integrity = Integrity {
        size: Some(item.size),
        sha1: Some(item.sha1.clone()),
        ..Integrity::default()
    };
    let destination_lock = fetch::destination_download_lock(&item.destination);
    let _destination_guard = tokio::time::timeout(
        ASSET_RESOURCE_WAIT_TIMEOUT,
        destination_lock.lock(),
    )
    .await
    .map_err(|_| {
        crate::ErrorKind::NetworkError(
            H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_DESTINATION_LOCK
                .to_string(),
        )
    })?;
    // A different downloader may have committed the object while this item
    // waited for the destination lock. Reuse it instead of opening another
    // stream, which also prevents cross-engine `.part`/rename races.
    // Existence is checked before full hash verification: `verify_file`
    // acquires the global validation budget before it opens the file, so a
    // batch full of missing objects must not queue behind unrelated hashing
    // work before it can even reach the network stage. Existence alone is
    // never treated as correctness; an existing object is still verified.
    match tokio::fs::try_exists(&item.destination).await {
        Ok(true) => {
            if fetch::verify_file(&item.destination, &integrity)
                .await
                .is_ok()
            {
                return Ok(match copy_asset_legacy_destinations(item).await {
                    Ok(()) => {
                        AssetBatchItemOutcome::Completed { downloaded: false }
                    }
                    Err(error) => AssetBatchItemOutcome::LocalCopyFailed {
                        downloaded: false,
                        error,
                    },
                });
            }
        }
        Ok(false) => {}
        Err(error) => {
            return Ok(AssetBatchItemOutcome::LocalObjectFailed {
                error: error.into(),
            });
        }
    }
    let part_path = match prepare_asset_part_path(&item.destination).await {
        Ok(part_path) => part_path,
        Err(error) => {
            return Ok(AssetBatchItemOutcome::LocalObjectFailed { error });
        }
    };
    let mut current_uri = uri.clone();
    let mut current_route = route.clone();
    let mut redirect_count = 0;
    let (connection, _stream_permit, _connection_stream, response, mut stream) = loop {
        let _stream_permit = if apply_native_policy {
            Some(
                tokio::time::timeout(
                    ASSET_RESOURCE_WAIT_TIMEOUT,
                    super::h2_stream_budget::acquire_download(
                        &current_route,
                        None,
                        true,
                    ),
                )
                .await
                .map_err(|_| {
                    crate::ErrorKind::NetworkError(
                        H2_DOWNLOAD_TIMED_OUT_WAITING_FOR_ASSET_HTTP_2_STREAM_PERMIT
                            .to_string(),
                    )
                })??,
            )
        } else {
            None
        };
        let connection =
            connections.connection_for(&current_route, rescue).await;
        let connection_stream = connection.track_stream();
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&crate::launcher_user_agent())
                .unwrap_or_else(|_| {
                    HeaderValue::from_static(H2_DOWNLOAD_AXOLOTL_LAUNCHER)
                }),
        );
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static(IDENTITY_ENCODING),
        );

        let (response, mut stream) =
            open_stream(&connection, &current_uri, headers).await?;
        super::native_breaker::record_response_status_failure(
            &current_route,
            response.status().as_u16(),
            fetch::retry_after_headers(response.headers()),
        );
        if !response.status().is_redirection() {
            break (
                connection,
                _stream_permit,
                connection_stream,
                response,
                stream,
            );
        }
        if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return Err(crate::ErrorKind::OtherError(format!(
                "HTTP/2 GET failed with status {}",
                response.status()
            ))
            .into());
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                crate::ErrorKind::OtherError(format!(
                    "Redirect from {current_uri} did not include a valid Location header"
                ))
            })?;
        if location.len() > MAX_REDIRECT_LOCATION_BYTES
            || location.chars().any(char::is_control)
        {
            return Err(crate::ErrorKind::OtherError(format!(
                "Redirect from {current_uri} included an unsafe Location header"
            ))
            .into());
        }
        let current_url = Url::parse(&current_uri.to_string())?;
        let next_url =
            retired_sources::normalize_parsed(current_url.join(location)?)?;
        if !fetch::is_allowed_download_redirect(&next_url) {
            return Err(crate::ErrorKind::OtherError(format!(
                "Refusing insecure redirect from {current_url} to {next_url}"
            ))
            .into());
        }
        if redirect_count >= MAX_DOWNLOAD_REDIRECTS {
            return Err(crate::ErrorKind::OtherError(format!(
                "Too many redirects while downloading {}",
                route.url
            ))
            .into());
        }
        drain_body(&mut stream).await;
        current_uri = next_url.as_str().parse().map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Invalid redirect target {next_url}: {error}"
            ))
        })?;
        current_route.url = next_url.into();
        redirect_count += 1;
    };
    super::native_breaker::record_response_status_failure(
        &current_route,
        response.status().as_u16(),
        fetch::retry_after_headers(response.headers()),
    );
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "HTTP/2 GET failed with status {}",
            response.status()
        ))
        .into());
    }

    let mut hashers =
        fetch::IntegrityHashers::new_integrity_hashers(&integrity);
    // Any failure below leaves a partial file behind; clean it up so retries
    // start from a clean slate and no orphaned `.part` files accumulate.
    let result: crate::Result<AssetBatchItemOutcome> = async {
        let mut file = match tokio::fs::File::create(&part_path).await {
            Ok(file) => file,
            Err(error) => {
                return Ok(AssetBatchItemOutcome::LocalObjectFailed {
                    error: error.into(),
                });
            }
        };
        let mut downloaded = 0_u64;
        let activity = super::h2_receive::H2TransferActivity::begin();
        loop {
            let chunk = super::h2_receive::receive_chunk(
                &mut stream,
                H2_DOWNLOAD_ASSET,
            )
            .await?;
            let Some(chunk) = chunk else {
                break;
            };
            if let Err(error) = super::local_resources::write(
                &part_path,
                chunk.len() as u64,
                async {
                    file.write_all(&chunk).await?;
                    file.flush().await
                },
            )
            .await
            {
                return Ok(AssetBatchItemOutcome::LocalObjectFailed {
                    error: error.into(),
                });
            }
            hashers.update(&chunk);
            downloaded += chunk.len() as u64;
            connection.record_bytes(chunk.len());
            activity.record_bytes(chunk.len());
            super::h2_receive::release_capacity(&mut stream, chunk.len())?;
        }
        if let Err(error) =
            super::local_resources::write(&part_path, 0, file.flush()).await
        {
            return Ok(AssetBatchItemOutcome::LocalObjectFailed {
                error: error.into(),
            });
        }
        drop(file);
        drop(_stream_permit);
        if downloaded == 0 {
            return Err(crate::ErrorKind::OtherError(
                H2_DOWNLOAD_DOWNLOADED_ASSET_IS_EMPTY.to_string(),
            )
            .into());
        }
        let computed = hashers.finish(downloaded);
        fetch::verify_computed_integrity(&integrity, &computed)?;
        if let Err(error) =
            fetch::finalize_download(&part_path, &item.destination).await
        {
            return Ok(AssetBatchItemOutcome::LocalObjectFailed { error });
        }

        Ok(match copy_asset_legacy_destinations(item).await {
            Ok(()) => AssetBatchItemOutcome::Completed { downloaded: true },
            Err(error) => AssetBatchItemOutcome::LocalCopyFailed {
                downloaded: true,
                error,
            },
        })
    }
    .await;
    if result.is_err()
        || matches!(
            &result,
            Ok(AssetBatchItemOutcome::LocalObjectFailed { .. })
        )
    {
        let _ = tokio::fs::remove_file(&part_path).await;
    }
    result
}

async fn copy_asset_legacy_destinations(
    item: &H2BatchAsset,
) -> crate::Result<()> {
    for legacy in &item.legacy_destinations {
        if let Some(state) = crate::State::get_if_initialized() {
            fetch::copy(&item.destination, legacy, &state.io_semaphore).await?;
        } else {
            if let Some(parent) = legacy.parent() {
                crate::util::io::create_dir_all(parent).await?;
            }
            tokio::fs::copy(&item.destination, legacy).await?;
        }
    }
    Ok(())
}
