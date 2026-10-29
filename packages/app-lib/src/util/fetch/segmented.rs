use super::client::{DownloadClients, is_h2_protocol_failure};
use super::file::{
    ResourceValidator, parse_content_range, remove_if_exists,
    resource_validator_changed, validate_resource_version,
};
use super::model::{
    DownloadMeta, DownloadRequest, DownloadRoute, DownloadRouteSource,
    FetchProgressFn,
};
use super::policy::{
    fetch_retry_delay, persisted_route_health, record_range_splitting_success,
    route_health_key,
};
#[cfg(test)]
use super::probe;
use super::probe::{
    RouteProbeFuture, RouteProbeResult, expected_route_speed,
    probe_faster_route,
};
use super::record_install_download_progress;
use super::record_install_download_stage;
use super::record_install_download_started;
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::{sanitize_url_for_log, url_authority};
use super::segment::{
    DownloadRange, DownloadRangeGuard, NativeConnectionPermit,
    RangeExpansionBaseline, SegmentCleanupGuard, SegmentDownloadCompletion,
    acquire_initial_segment_permits, adaptive_range_cap,
    create_remaining_ranges, expansion_block_reason, initial_segment_count,
    native_first_byte_timeout, range_connection_floor, range_growth_can_retry,
    range_growth_target, range_idle_timeout, requested_range_floor,
    route_segmented_concurrency_cap, try_acquire_native_connection,
};
#[cfg(test)]
use super::tail;
use super::tail::race_tail_candidates;
use super::throttle::{
    FetchSemaphore, TAIL_HEDGE_SEMAPHORE, configured_semaphore_limit,
    record_authority_h2_failure_for,
};
#[cfg(test)]
use super::transfer;
#[cfg(test)]
use crate::ErrorKind;
use crate::install::DownloadItemStatus;
use crate::util::download::constants::{
    DOWNLOAD_BUFFER_BYTES, DOWNLOAD_PROGRESS_INTERVAL,
    DOWNLOAD_PROGRESS_MIN_BYTES, DOWNLOAD_RANGE_RECOVERY_INTERVAL,
    DOWNLOAD_RANGE_TARGET_TIME, DOWNLOAD_SCHEDULER_INTERVAL,
    DOWNLOAD_SCHEDULING_GRACE, FETCH_ACTIVE_SEGMENT_BODY,
    FETCH_CONFIGURED_CONNECTION_LIMIT_IS_ONE, FETCH_INVALID_CONTENT_RANGE,
    FETCH_RANGE_BYTE_COUNT_MISMATCH, FETCH_RANGE_END_OVERFLOW,
    FETCH_RANGE_RESPONSE_ENDED_BEFORE_EXPECTED_BOUNDARY,
    FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES,
    FETCH_RESUMABLE_PARTIAL_USES_A_SINGLE_STREAM,
    FETCH_SEGMENTED_CONTENT_VALIDATION_FAILED, FETCH_SELECTED_ENDPOINT_CHANGED,
    FETCH_SERVER_IGNORED_RANGE_AND_RETURNED_200,
    FETCH_SERVER_RETURNED_HTTP_416_FOR_RANGE_REQUEST, MAX_TAIL_HEDGES_PER_FILE,
    MIN_PARALLEL_RANGES, SEGMENT_EXPANSION_INTERVAL, SEGMENT_RETRY_ATTEMPTS,
    TAIL_HEDGE_MIN_REMAINING,
};
use crate::util::download::integrity::{
    compute_file_integrity, validate_file_content, verify_computed_integrity,
};
use crate::util::download::native_request::send_path_request_with_clients;
use crate::util::download::route_health::ROUTE_HEALTH;
use crate::util::download_manager::DownloadSpeedTracker;
#[cfg(test)]
use crate::util::io;
use crate::util::io::IOError;
use futures::StreamExt;
use parking_lot::Mutex;
use reqwest::StatusCode;
#[cfg(test)]
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{self, Instant};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

pub(crate) struct SegmentedDownloadSuccess {
    pub(crate) size: u64,
    pub(crate) final_url: String,
    pub(crate) ttfb: time::Duration,
    pub(crate) transfer_elapsed: time::Duration,
    pub(crate) remote_addr: Option<std::net::SocketAddr>,
    pub(crate) http_version: Option<reqwest::Version>,
}

pub(crate) enum SegmentedDownloadOutcome {
    Success(SegmentedDownloadSuccess),
    FallbackSingle {
        disable_range: bool,
        reason: &'static str,
    },
    SwitchRoute(RouteProbeResult),
    SourceFailed,
    IntegrityFailed(crate::Error),
    Fatal(crate::Error),
}

pub(crate) enum SegmentDownloadError {
    Protocol(&'static str),
    Transport,
    Fatal(crate::Error),
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn download_segment(
    route: &DownloadRoute,
    range: DownloadRange,
    total_size: u64,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    part_path: &Path,
    output: &Arc<crate::util::download::range_output::RangeOutput>,
    _permit: NativeConnectionPermit<'_>,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    progress: tokio::sync::mpsc::UnboundedSender<u64>,
    speed: &DownloadSpeedTracker,
    validator: &Mutex<Option<ResourceValidator>>,
    redirect_target: Option<&AsyncMutex<Option<Url>>>,
    semaphore: &FetchSemaphore,
    hedge_count: &AtomicUsize,
    hedge_active: &AtomicBool,
    request: &DownloadRequest,
) -> Result<SegmentDownloadCompletion, SegmentDownloadError> {
    let counters = &request.transfer_counters;
    let _activity = crate::State::get_if_initialized()
        .map(|state| state.begin_download_connection());
    let _range_guard = DownloadRangeGuard(Arc::clone(&range.state));
    let request_started = Instant::now();
    let mut pending_progress = 0_u64;
    let mut last_progress_at = time::Instant::now();
    let mut final_url = route.url.clone();
    let mut remote_addr = None;
    let mut http_version = None;
    let mut slow =
        crate::util::download::native_slow::NativeSlowPolicy::new(0, None);
    let mut last_slow_speed = None;
    for attempt in 1..=SEGMENT_RETRY_ATTEMPTS {
        let requested_start = range.start + {
            let state = range.state.lock();
            state.downloaded
        };
        let downloaded_before_attempt = requested_start - range.start;
        let requested_end = range.end();
        let mut writer = output
            .open_range(
                requested_start,
                requested_end.checked_add(1).ok_or_else(|| {
                    SegmentDownloadError::Protocol(FETCH_RANGE_END_OVERFLOW)
                })?,
            )
            .await
            .map_err(|error| SegmentDownloadError::Fatal(error.into()))?;
        let response = tokio::time::timeout(
            native_first_byte_timeout(route, false),
            send_path_request_with_clients(
                route,
                custom_header,
                credentials,
                download_meta,
                Some(requested_start),
                Some(requested_end),
                &DownloadClients::for_request(system_client, direct_client),
                redirect_target,
            ),
        )
        .await;
        let (response, response_url) = match response {
            Ok(Ok(response)) => response,
            Ok(Err(_)) | Err(_) if attempt < SEGMENT_RETRY_ATTEMPTS => {
                tracing::debug!(
                    url = %sanitize_url_for_log(&route.url),
                    range_start = requested_start,
                    range_end = requested_end,
                    attempt,
                    "Range request failed temporarily; retrying"
                );
                tokio::time::sleep(fetch_retry_delay(attempt)).await;
                continue;
            }
            Ok(Err(_)) | Err(_) => return Err(SegmentDownloadError::Transport),
        };
        final_url = response_url;
        let parsed_content_range = parse_content_range(&response);
        tracing::debug!(
            path = %part_path.display(),
            original_url = %sanitize_url_for_log(&route.url),
            final_host = Url::parse(&final_url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_default(),
            source = route.source.as_str(),
            status = response.status().as_u16(),
            content_range = ?parsed_content_range,
            range_start = requested_start,
            range_end = requested_end,
            "Received download range response"
        );
        if response.status() == StatusCode::OK {
            return Err(SegmentDownloadError::Protocol(
                FETCH_SERVER_IGNORED_RANGE_AND_RETURNED_200,
            ));
        }
        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
            return Err(SegmentDownloadError::Protocol(
                FETCH_SERVER_RETURNED_HTTP_416_FOR_RANGE_REQUEST,
            ));
        }
        if response.status() != StatusCode::PARTIAL_CONTENT {
            if attempt < SEGMENT_RETRY_ATTEMPTS {
                tokio::time::sleep(fetch_retry_delay(attempt)).await;
                continue;
            }
            return Err(SegmentDownloadError::Transport);
        }
        let content_range_matches = parsed_content_range.is_some_and(|range| {
            range.start == requested_start
                && range.end == requested_end
                && range.total.is_none_or(|total| total == total_size)
        });
        if !content_range_matches {
            return Err(SegmentDownloadError::Protocol(
                FETCH_INVALID_CONTENT_RANGE,
            ));
        }
        if !validate_resource_version(validator, &response, &request.integrity)
        {
            return Err(SegmentDownloadError::Protocol(
                FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES,
            ));
        }
        remote_addr = response.remote_addr();
        http_version = Some(response.version());
        if range.index == 0 && requested_start == range.start {
            tracing::debug!(
                original_url = %sanitize_url_for_log(&route.url),
                final_host = Url::parse(&final_url)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .unwrap_or_default(),
                file_size = total_size,
                supports_range = true,
                "Confirmed byte-range download support"
            );
        }
        let endpoint_feedback = response.extensions().get::<crate::util::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
        let mut stream = Some(response.bytes_stream());
        let mut stream_end_reason = None;
        loop {
            let tail_threshold = TAIL_HEDGE_MIN_REMAINING.max(total_size / 10);
            let tail_eligible = range.remaining() <= tail_threshold;
            let chunk = match tokio::time::timeout(
                range_idle_timeout(tail_eligible),
                async {
                    stream
                        .as_mut()
                        .expect(FETCH_ACTIVE_SEGMENT_BODY)
                        .next()
                        .await
                },
            )
            .await
            {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) if tail_eligible => {
                    let hedge_start =
                        range.start + range.state.lock().downloaded;
                    let hedge_end = range.end();
                    let hedge_reserved = !hedge_active
                        .swap(true, Ordering::AcqRel)
                        && hedge_count.load(Ordering::Acquire)
                            < MAX_TAIL_HEDGES_PER_FILE;
                    if hedge_reserved {
                        let global_permit = TAIL_HEDGE_SEMAPHORE.try_acquire();
                        let connection_permit =
                            try_acquire_native_connection(route, semaphore);
                        if let (Ok(_global_permit), Some(_connection_permit)) =
                            (global_permit, connection_permit)
                        {
                            hedge_count.fetch_add(1, Ordering::AcqRel);
                            drop(stream.take());
                            let raced = race_tail_candidates(
                                route,
                                &range,
                                total_size,
                                hedge_start,
                                hedge_end,
                                custom_header,
                                credentials,
                                download_meta,
                                part_path,
                                system_client,
                                direct_client,
                                &request.integrity,
                                validator,
                                redirect_target,
                            )
                            .await;
                            hedge_active.store(false, Ordering::Release);
                            match raced {
                                Ok((winner, winner_index)) => {
                                    let mut winner_file =
                                        File::open(&winner.path)
                                            .await
                                            .map_err(|error| {
                                                SegmentDownloadError::Fatal(
                                                    IOError::with_path(
                                                        error,
                                                        &winner.path,
                                                    )
                                                    .into(),
                                                )
                                            })?;
                                    let mut buffer =
                                        vec![0_u8; DOWNLOAD_BUFFER_BYTES];
                                    loop {
                                        let read = winner_file
                                            .read(&mut buffer)
                                            .await
                                            .map_err(|error| {
                                                SegmentDownloadError::Fatal(
                                                    IOError::with_path(
                                                        error,
                                                        &winner.path,
                                                    )
                                                    .into(),
                                                )
                                            })?;
                                        if read == 0 {
                                            break;
                                        }
                                        let (accepted, _) =
                                            range.accept_chunk(read);
                                        writer
                                            .write_next(&buffer[..accepted])
                                            .await
                                            .map_err(|error| {
                                                SegmentDownloadError::Fatal(
                                                    error.into(),
                                                )
                                            })?;
                                        pending_progress += accepted as u64;
                                        speed.record_bytes(accepted as u64);
                                        if let Some(state) =
                                            crate::State::get_if_initialized()
                                        {
                                            state.record_download_bytes(
                                                accepted as u64,
                                            );
                                        }
                                    }
                                    remove_if_exists(&winner.path)
                                        .await
                                        .map_err(SegmentDownloadError::Fatal)?;
                                    final_url = winner.final_url.clone();
                                    remote_addr = winner.remote_addr;
                                    http_version = Some(winner.http_version);
                                    tracing::info!(
                                        url = %sanitize_url_for_log(&route.url),
                                        range_start = hedge_start,
                                        range_end = hedge_end,
                                        winner = winner_index,
                                        "Tail Range hedge won"
                                    );
                                    break;
                                }
                                Err(SegmentDownloadError::Fatal(error)) => {
                                    return Err(SegmentDownloadError::Fatal(
                                        error,
                                    ));
                                }
                                Err(SegmentDownloadError::Protocol(reason)) => {
                                    return Err(
                                        SegmentDownloadError::Protocol(reason),
                                    );
                                }
                                Err(SegmentDownloadError::Transport) => {}
                            }
                        } else {
                            hedge_active.store(false, Ordering::Release);
                        }
                    }
                    stream_end_reason = Some(format!(
                        "tail range made no progress for {:.0} seconds",
                        range_idle_timeout(true).as_secs_f64()
                    ));
                    break;
                }
                Err(_) => {
                    stream_end_reason = Some(format!(
                        "no range data for {:.0} seconds",
                        range_idle_timeout(false).as_secs_f64()
                    ));
                    tracing::warn!(
                        url = %sanitize_url_for_log(&route.url),
                        range_start = requested_start,
                        range_end = requested_end,
                        remaining_bytes = range.remaining(),
                        idle_seconds = range_idle_timeout(false).as_secs_f64(),
                        "Range stream stalled; reconnecting from confirmed offset"
                    );
                    break;
                }
            };
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    if is_h2_protocol_failure(&error)
                        && let Some(authority) = url_authority(&final_url)
                    {
                        record_authority_h2_failure_for(
                            &authority,
                            route.proxy,
                        );
                    }
                    stream_end_reason = Some(error.to_string());
                    break;
                }
            };
            counters.received(chunk.len() as u64);
            if let Some(feedback) = &endpoint_feedback {
                feedback.record(
                    chunk.len(),
                    crate::util::download::local_resources::pressure(part_path),
                );
            }
            let (accepted, completed) = range.accept_chunk(chunk.len());
            writer
                .write_next(&chunk[..accepted])
                .await
                .map_err(|error| SegmentDownloadError::Fatal(error.into()))?;
            pending_progress += accepted as u64;
            speed.record_bytes(accepted as u64);
            if pending_progress >= DOWNLOAD_PROGRESS_MIN_BYTES
                || last_progress_at.elapsed() >= DOWNLOAD_PROGRESS_INTERVAL
            {
                let _ = progress.send(std::mem::take(&mut pending_progress));
                last_progress_at = time::Instant::now();
            }
            if completed {
                break;
            }
            let pressure =
                crate::util::download::local_resources::pressure(part_path);
            let downloaded_range = range.state.lock().downloaded;
            if let crate::util::download::native_slow::SlowDecision::Probe {
                bytes_per_second,
            } = slow.observe_with_pressure(
                downloaded_range,
                range.remaining(),
                pressure,
            ) {
                last_slow_speed = Some(bytes_per_second);
                if let Some(clients) =
                    crate::util::download::proxy_context::clients()
                {
                    clients.endpoints.on_slow_region(
                        request,
                        route,
                        &clients,
                        crate::util::download::endpoint_selection::Protocol::Http1,
                        remote_addr.map(|a| a.ip()),
                        range.start + downloaded_range
                            ..range.end().saturating_add(1),
                        Some(bytes_per_second),
                        pressure,
                    );
                }
            }
            if let Some(current_speed) = last_slow_speed
                && !pressure.blocks_expansion()
                && let Some(clients) = crate::util::download::proxy_context::clients()
                && clients
                    .endpoints
                    .recovery_lease(
                        request,
                        route,
                        crate::util::download::endpoint_selection::Protocol::Http1,
                        remote_addr,
                        current_speed,
                        range.remaining(),
                    )
                    .is_some()
            {
                stream_end_reason =
                    Some(FETCH_SELECTED_ENDPOINT_CHANGED.into());
                break;
            }
        }
        drop(stream);
        if range.remaining() == 0 {
            writer
                .flush()
                .await
                .map_err(|error| SegmentDownloadError::Fatal(error.into()))?;
            counters.commit(
                requested_start,
                range.start + range.state.lock().downloaded,
            );
            break;
        }
        if attempt < SEGMENT_RETRY_ATTEMPTS {
            writer
                .flush()
                .await
                .map_err(|error| SegmentDownloadError::Fatal(error.into()))?;
            counters.commit(
                requested_start,
                range.start + range.state.lock().downloaded,
            );
            let downloaded_after_attempt = {
                let state = range.state.lock();
                state.downloaded
            };
            tracing::warn!(
                url = %sanitize_url_for_log(&route.url),
                range_start = requested_start,
                range_end = range.end(),
                attempt,
                received_bytes = downloaded_after_attempt
                    .saturating_sub(downloaded_before_attempt),
                remaining_bytes = range.remaining(),
                reason = stream_end_reason
                    .as_deref()
                    .unwrap_or("response ended before Content-Range boundary"),
                "Range stream ended early; resuming remaining bytes"
            );
            tokio::time::sleep(fetch_retry_delay(attempt)).await;
            continue;
        }
        return Err(SegmentDownloadError::Transport);
    }
    if pending_progress > 0 {
        let _ = progress.send(pending_progress);
    }
    if !range.finish() {
        return Err(SegmentDownloadError::Protocol(
            FETCH_RANGE_RESPONSE_ENDED_BEFORE_EXPECTED_BOUNDARY,
        ));
    }
    Ok(SegmentDownloadCompletion {
        final_url,
        is_first_range: range.index == 0,
        ttfb: request_started.elapsed(),
        remote_addr,
        http_version,
    })
}

pub(crate) struct SegmentedDownloadContext<'a> {
    request: &'a DownloadRequest,
    route: &'a DownloadRoute,
    candidate_routes: &'a [DownloadRoute],
    size: u64,
    part_path: &'a Path,
    semaphore: &'a FetchSemaphore,
    credentials: Option<&'a crate::state::ModrinthCredentials>,
    system_client: &'a reqwest::Client,
    direct_client: &'a reqwest::Client,
    attempt: usize,
    max_attempts: usize,
    allow_low_throughput_abort: bool,
}

async fn finalize_segmented_output(
    request: &DownloadRequest,
    route: &DownloadRoute,
    size: u64,
    part_path: &Path,
    mut progress: Option<&mut FetchProgressFn<'_>>,
    output: std::sync::Arc<crate::util::download::range_output::RangeOutput>,
    cleanup_guard: &mut SegmentCleanupGuard,
    transfer_started: Instant,
    downloaded: u64,
    final_url: Option<String>,
    initial_ttfb: Option<time::Duration>,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: Option<reqwest::Version>,
) -> SegmentedDownloadOutcome {
    record_install_download_stage(request, DownloadItemStatus::Writing).await;
    if downloaded != size {
        let _ = remove_if_exists(part_path).await;
        return SegmentedDownloadOutcome::FallbackSingle {
            disable_range: true,
            reason: FETCH_RANGE_BYTE_COUNT_MISMATCH,
        };
    }
    drop(output);
    let computed =
        match compute_file_integrity(part_path, &request.integrity).await {
            Ok(computed) => computed,
            Err(error) => return SegmentedDownloadOutcome::Fatal(error),
        };
    record_install_download_stage(request, DownloadItemStatus::Verifying).await;
    if let Err(error) = verify_computed_integrity(&request.integrity, &computed)
    {
        let _ = remove_if_exists(part_path).await;
        return SegmentedDownloadOutcome::IntegrityFailed(error);
    }
    if validate_file_content(part_path, request.integrity.content)
        .await
        .is_err()
    {
        let _ = remove_if_exists(part_path).await;
        return SegmentedDownloadOutcome::FallbackSingle {
            disable_range: true,
            reason: FETCH_SEGMENTED_CONTENT_VALIDATION_FAILED,
        };
    }
    if downloaded < size
        && let Some(progress) = progress.as_mut()
        && let Err(error) = progress(size, size).await
    {
        tracing::warn!(%error, "Download progress callback failed");
    }
    record_range_splitting_success(route);
    cleanup_guard.disarm();
    SegmentedDownloadOutcome::Success(SegmentedDownloadSuccess {
        size,
        final_url: final_url.unwrap_or_else(|| route.url.clone()),
        ttfb: initial_ttfb.unwrap_or_default(),
        transfer_elapsed: transfer_started.elapsed(),
        remote_addr,
        http_version,
    })
}

impl<'a> SegmentedDownloadContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        request: &'a DownloadRequest,
        route: &'a DownloadRoute,
        candidate_routes: &'a [DownloadRoute],
        size: u64,
        part_path: &'a Path,
        semaphore: &'a FetchSemaphore,
        credentials: Option<&'a crate::state::ModrinthCredentials>,
        system_client: &'a reqwest::Client,
        direct_client: &'a reqwest::Client,
        attempt: usize,
        max_attempts: usize,
        allow_low_throughput_abort: bool,
    ) -> Self {
        Self {
            request,
            route,
            candidate_routes,
            size,
            part_path,
            semaphore,
            credentials,
            system_client,
            direct_client,
            attempt,
            max_attempts,
            allow_low_throughput_abort,
        }
    }
}

pub(crate) async fn try_segmented_download(
    context: SegmentedDownloadContext<'_>,
    mut progress: Option<&mut FetchProgressFn<'_>>,
) -> SegmentedDownloadOutcome {
    let SegmentedDownloadContext {
        request,
        route,
        candidate_routes,
        size,
        part_path,
        semaphore,
        credentials,
        system_client,
        direct_client,
        attempt,
        max_attempts,
        allow_low_throughput_abort,
    } = context;
    let resume_prefix = match tokio::fs::metadata(part_path).await {
        Ok(metadata) if metadata.len() > 0 => {
            if !metadata.is_file()
                || metadata.len() >= size
                || !request.integrity.supports_resume()
            {
                return SegmentedDownloadOutcome::FallbackSingle {
                    disable_range: false,
                    reason: FETCH_RESUMABLE_PARTIAL_USES_A_SINGLE_STREAM,
                };
            }
            metadata.len()
        }
        Ok(_) => 0,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return SegmentedDownloadOutcome::Fatal(
                IOError::with_path(error, part_path).into(),
            );
        }
    };
    let remaining_size = size - resume_prefix;
    let configured_limit = configured_semaphore_limit(semaphore);
    let route_cap = route_segmented_concurrency_cap(
        route,
        configured_limit,
        remaining_size,
    );
    let health =
        route_health_key(route, request.resource)
            .map(|key| {
                ROUTE_HEALTH.lock().get(&key).cloned().unwrap_or_else(|| {
                    persisted_route_health(&key, route.proxy)
                })
            })
            .unwrap_or_default();
    let disk_cap =
        crate::util::download::local_resources::http1_range_limit(part_path)
            .await;
    let range_recovery = (request.batch_download && request.prefer_http1)
        || request.minimum_range_connections(size) > 1;
    let mut concurrency_cap = adaptive_range_cap(
        remaining_size,
        route_cap,
        disk_cap,
        health.consecutive_failures,
        range_recovery,
    );
    let requested_initial_count =
        initial_segment_count(remaining_size, concurrency_cap).max(
            requested_range_floor(
                request,
                size,
                remaining_size,
                concurrency_cap,
            ),
        );
    if requested_initial_count < MIN_PARALLEL_RANGES {
        tracing::debug!(
            original_url = %sanitize_url_for_log(&route.url),
            file_size = size,
            supports_range = true,
            configured_limit,
            "Using a single connection because the configured limit is one"
        );
        return SegmentedDownloadOutcome::FallbackSingle {
            disable_range: false,
            reason: FETCH_CONFIGURED_CONNECTION_LIMIT_IS_ONE,
        };
    }
    let permits = match acquire_initial_segment_permits(
        route,
        semaphore,
        requested_initial_count,
    )
    .await
    {
        Ok(permits) => permits,
        Err(error) => return SegmentedDownloadOutcome::Fatal(error),
    };
    let mut cleanup_guard =
        SegmentCleanupGuard::new(part_path).preserve_prefix(resume_prefix);
    let output = if resume_prefix > 0 {
        crate::util::download::range_output::RangeOutput::resume_prefix(
            part_path,
            size,
            resume_prefix,
        )
        .await
    } else {
        crate::util::download::range_output::RangeOutput::create(
            part_path, size,
        )
        .await
    };
    let output = match output {
        Ok(output) => output,
        Err(error) => return SegmentedDownloadOutcome::Fatal(error.into()),
    };
    request.transfer_counters.commit(0, resume_prefix);
    record_install_download_started(request, route, attempt, max_attempts)
        .await;
    let transfer_started = Instant::now();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    if request.batch_download && request.prefer_http1 {
        crate::util::download::endpoint_selection::tuning::TRIALS
            .invalidate_for_range_recovery();
    }
    let speed = DownloadSpeedTracker::default();
    let validator = Mutex::new(None);
    let redirect_target = AsyncMutex::new(None);
    let hedge_count = AtomicUsize::new(0);
    let hedge_active = AtomicBool::new(false);
    let mut downloads = futures::stream::FuturesUnordered::new();
    let mut ranges =
        create_remaining_ranges(size, resume_prefix, permits.len());
    for (range, permit) in ranges.iter().cloned().zip(permits) {
        downloads.push(download_segment(
            route,
            range,
            size,
            request.header.as_ref(),
            credentials,
            request.download_meta.as_ref(),
            part_path,
            &output,
            permit,
            system_client,
            direct_client,
            progress_tx.clone(),
            &speed,
            &validator,
            (route.source == DownloadRouteSource::Mcim)
                .then_some(&redirect_target),
            semaphore,
            &hedge_count,
            &hedge_active,
            request,
        ));
    }
    tracing::debug!(
        original_url = %sanitize_url_for_log(&route.url),
        file_size = size,
        resume_offset = resume_prefix,
        supports_range = true,
        active_ranges = downloads.len(),
        concurrency_cap,
        configured_limit,
        route_cap,
        disk_cap,
        requested_range_floor = request.minimum_range_connections(size),
        available_permits = semaphore.0.available_permits(),
        reason = "initial parallel ranges",
        "Started segmented download"
    );
    let mut next_range_index = ranges.len();
    let mut retained_ranges = ranges.len();
    let mut completed_ranges = 0;
    let mut scheduler = tokio::time::interval(DOWNLOAD_SCHEDULER_INTERVAL);
    scheduler.tick().await;
    let mut last_expansion = Instant::now();
    let mut expansion_baseline: Option<RangeExpansionBaseline> = None;
    let mut expansion_trial: Option<
        crate::util::download::endpoint_selection::tuning::TrialLease,
    > = None;
    let mut expansion_exhausted: Option<u64> = None;
    let mut last_block_reason = None;
    let mut downloaded = resume_prefix;
    let mut slow_policy =
        crate::util::download::native_slow::NativeSlowPolicy::new(
            resume_prefix,
            expected_route_speed(route, request.resource),
        );
    let mut alternate_probe: Option<RouteProbeFuture<'_>> = None;
    let mut alternate_probe_finished = false;
    let mut confirmed_switch = None;
    let mut segment_error = None;
    let mut final_url = None;
    let mut initial_ttfb = None;
    let mut remote_addr = None;
    let mut http_version = None;
    while !downloads.is_empty() {
        tokio::select! {
            Some(delta) = progress_rx.recv() => {
                downloaded = downloaded.saturating_add(delta);
                record_install_download_progress(request, downloaded, size).await;
                if let Some(progress) = progress.as_mut()
                    && let Err(error) = progress(downloaded, size).await
                {
                    tracing::warn!(%error, "Download progress callback failed");
                }
            }
            result = downloads.next() => {
                if let Some(result) = result {
                    completed_ranges += 1;
                    match result {
                        Ok(completion) if completion.is_first_range => {
                            final_url = Some(completion.final_url);
                            initial_ttfb = Some(completion.ttfb);
                            remote_addr = completion.remote_addr;
                            http_version = completion.http_version;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            segment_error = Some(error);
                            break;
                        }
                    }
                }
            }
            probe = async {
                alternate_probe
                    .as_mut()
                    .expect("route probe is guarded by the select condition")
                    .await
            }, if alternate_probe.is_some() => {
                alternate_probe = None;
                alternate_probe_finished = true;
                if let Some(probe) = probe {
                    tracing::warn!(
                        original_url = %sanitize_url_for_log(&route.url),
                        source = route.source.as_str(),
                        alternate_url = %sanitize_url_for_log(&probe.route.url),
                        alternate_source = probe.route.source.as_str(),
                        alternate_authority = probe.effective_authority,
                        alternate_bytes_per_second = probe.bytes_per_second,
                        "Confirmed a faster download route; switching source"
                    );
                    confirmed_switch = Some(probe);
                    break;
                }
            }
            _ = scheduler.tick() => {
                let slow_decision = slow_policy.observe_with_pressure(
                    downloaded,
                    size.saturating_sub(downloaded),
                    crate::util::download::local_resources::pressure(part_path),
                );
                if allow_low_throughput_abort
                    && !alternate_probe_finished
                    && alternate_probe.is_none()
                    && let crate::util::download::native_slow::SlowDecision::Probe {
                        bytes_per_second,
                    } = slow_decision
                {
                    tracing::warn!(
                        original_url = %sanitize_url_for_log(&route.url),
                        source = route.source.as_str(),
                        bytes_per_second,
                        remaining_bytes = size.saturating_sub(downloaded),
                        "Segmented download stayed slow; probing alternate routes"
                    );
                    alternate_probe = Some(Box::pin(probe_faster_route(
                        route,
                        candidate_routes,
                        bytes_per_second,
                        size,
                        request.header.as_ref(),
                        credentials,
                        request.download_meta.as_ref(),
                        semaphore,
                        system_client,
                        direct_client,
                        request.resource,
                        downloaded,
                    )));
                }
                if matches!(
                    slow_decision,
                    crate::util::download::native_slow::SlowDecision::Commit
                ) {
                    alternate_probe = None;
                    alternate_probe_finished = true;
                    slow_policy.commit();
                }
                if hedge_active.load(Ordering::Acquire) {
                    continue;
                }
                let pressure = crate::util::download::local_resources::pressure(part_path);
                if pressure.blocks_expansion() {
                    tracing::debug!(cpu_percent = pressure.cpu_percent, write_latency_ms = pressure.write_latency_ms, "Pausing range expansion under local resource pressure");
                    continue;
                }
                // A busy disk or CPU at admission must not fix the ceiling
                // for the entire transfer after local pressure has cleared.
                concurrency_cap = adaptive_range_cap(
                    remaining_size,
                    route_cap,
                    crate::util::download::local_resources::http1_range_limit(part_path).await,
                    health.consecutive_failures,
                    range_recovery,
                );
                let snapshot = speed.speed_snapshot();
                use crate::util::download::endpoint_selection::tuning::{TRIALS, TrialKind};
                if expansion_trial.as_ref().is_some_and(|lease| !lease.valid(Instant::now())) {
                    expansion_trial = None;
                    expansion_baseline = None;
                }
                if expansion_baseline.is_some() && last_expansion.elapsed() >= SEGMENT_EXPANSION_INTERVAL {
                    expansion_trial = None;
                }
                let active_ranges = downloads.len();
                let remaining_bytes = ranges
                    .iter()
                    .filter(|range| range.is_active())
                    .map(DownloadRange::remaining)
                    .sum();
                if last_expansion.elapsed() >= SEGMENT_EXPANSION_INTERVAL
                    && let Some(baseline) = expansion_baseline.take()
                    && baseline.gained_throughput(snapshot.recent_average, completed_ranges) == Some(false)
                {
                    retained_ranges = baseline.previous_target;
                    expansion_exhausted = Some(snapshot.recent_average);
                    tracing::debug!(
                        original_url = %sanitize_url_for_log(&route.url),
                        baseline_speed = baseline.speed,
                        expanded_speed = snapshot.recent_average,
                        retained_ranges,
                        "Stopping range growth because the last batch added less than 15 percent throughput"
                    );
                }
                // A burst can hide per-connection throttling. A substantially
                // slower production window warrants another bounded trial.
                if expansion_exhausted.is_some_and(|previous| {
                    range_growth_can_retry(previous, snapshot.recent_average)
                }) {
                    expansion_exhausted = None;
                }
                // Reuse the last productive connection count as ranges finish.
                let remaining_time = crate::util::download::native_slow::estimated_duration(
                    remaining_bytes,
                    snapshot.recent_average.min(snapshot.aggregate_speed),
                );
                let floor_target = range_connection_floor(size, remaining_bytes, concurrency_cap,
                    request.batch_download && request.prefer_http1)
                    .max(requested_range_floor(request, size, remaining_bytes, concurrency_cap));
                let below_floor = active_ranges < floor_target;
                if !below_floor && ((expansion_exhausted.is_some() && active_ranges >= retained_ranges)
                    || remaining_time <= crate::util::download::constants::NATIVE_SLOW_RECONNECT_OVERHEAD
                        + crate::util::download::constants::NATIVE_SLOW_MIN_SAVINGS
                ) {
                    continue;
                }
                let restoring_ranges = active_ranges < retained_ranges;
                let deadline_recovery = expansion_exhausted.is_none()
                    && remaining_time > DOWNLOAD_RANGE_TARGET_TIME;
                let recovering_ranges = below_floor || restoring_ranges || deadline_recovery;
                if !recovering_ranges && expansion_trial.is_none()
                    && TRIALS.snapshot(Instant::now()).1.is_some() { continue; }
                if let Some(reason) = expansion_block_reason(
                    snapshot,
                    active_ranges,
                    concurrency_cap,
                    semaphore.0.available_permits(),
                    remaining_bytes,
                    last_expansion.elapsed(),
                    if below_floor { DOWNLOAD_RANGE_RECOVERY_INTERVAL } else { SEGMENT_EXPANSION_INTERVAL },
                ) {
                    if last_block_reason != Some(reason) {
                        tracing::debug!(
                            original_url = %sanitize_url_for_log(&route.url),
                            active_ranges,
                            aggregate_speed = snapshot.aggregate_speed,
                            recent_average = snapshot.recent_average,
                            floor = snapshot.speed_floor,
                            remaining_bytes,
                            available_permits = semaphore.0.available_permits(),
                            reason,
                            "Segmented download did not increase concurrency"
                        );
                        last_block_reason = Some(reason);
                    }
                    continue;
                }
                let trial = if recovering_ranges {
                    None
                } else {
                    let Some(trial) = TRIALS.acquire(
                        TrialKind::Ranges,
                        Instant::now(),
                        SEGMENT_EXPANSION_INTERVAL + DOWNLOAD_SCHEDULING_GRACE,
                    ) else { continue; };
                    Some(trial)
                };
                let previous_target = retained_ranges;
                let growth_target = range_growth_target(
                    active_ranges,
                    concurrency_cap,
                    retained_ranges,
                    expansion_exhausted.is_some(),
                    remaining_bytes,
                    snapshot.recent_average,
                );
                let target = if below_floor {
                    floor_target.max(if restoring_ranges { retained_ranges } else { active_ranges }).min(concurrency_cap)
                } else if restoring_ranges {
                    retained_ranges.max(if deadline_recovery { growth_target } else { active_ranges }).min(concurrency_cap)
                } else { growth_target };
                for _ in active_ranges..target {
                    let Some(range) = ranges
                        .iter()
                        .filter(|range| range.is_active())
                        .max_by_key(|range| range.remaining())
                        .cloned() else { break; };
                    let Some(permit) = try_acquire_native_connection(route, semaphore) else { break; };
                    let Some(new_range) = range.split_tail(next_range_index) else { break; };
                    tracing::debug!(
                        original_url = %sanitize_url_for_log(&route.url),
                        source = route.source.as_str(),
                        active_ranges = downloads.len() + 1,
                        target_ranges = target,
                        aggregate_speed = snapshot.aggregate_speed,
                        recent_average = snapshot.recent_average,
                        floor = snapshot.speed_floor,
                        range_start = new_range.start,
                        range_end = new_range.end(),
                        reason = if below_floor {
                            "restoring the minimum live range connections"
                        } else if restoring_ranges {
                            "replacing completed range connections"
                        } else if deadline_recovery {
                            "recovering the remaining download time target"
                        } else {
                            "testing a larger connection batch"
                        },
                        "Starting an additional download range"
                    );
                    next_range_index += 1;
                    downloads.push(download_segment(
                        route,
                        new_range.clone(),
                        size,
                        request.header.as_ref(),
                        credentials,
                        None,
                        part_path,
                        &output,
                        permit,
                        system_client,
                        direct_client,
                        progress_tx.clone(),
                        &speed,
                        &validator,
                        (route.source == DownloadRouteSource::Mcim)
                            .then_some(&redirect_target),
                        semaphore,
                        &hedge_count,
                        &hedge_active,
                        request,
                    ));
                    ranges.push(new_range);
                }
                if downloads.len() > active_ranges {
                    retained_ranges = retained_ranges.max(downloads.len());
                    if recovering_ranges {
                        expansion_baseline = None;
                        expansion_trial = None;
                        TRIALS.invalidate_for_range_recovery();
                    } else {
                        expansion_baseline = Some(RangeExpansionBaseline {
                            speed: snapshot.recent_average,
                            completed_ranges,
                            previous_target,
                        });
                        expansion_trial = trial;
                    }
                    last_expansion = Instant::now();
                    last_block_reason = None;
                }
            }
        }
    }
    drop(progress_tx);
    drop(downloads);
    while let Ok(delta) = progress_rx.try_recv() {
        downloaded = downloaded.saturating_add(delta);
    }
    record_install_download_progress(request, downloaded, size).await;
    if let Some(probe) = confirmed_switch {
        return SegmentedDownloadOutcome::SwitchRoute(probe);
    }
    if let Some(error) = segment_error {
        return match error {
            SegmentDownloadError::Protocol(reason) => {
                SegmentedDownloadOutcome::FallbackSingle {
                    // Version metadata belongs to one object. A mismatch is
                    // not evidence that its server cannot serve byte ranges.
                    disable_range: !resource_validator_changed(reason),
                    reason,
                }
            }
            SegmentDownloadError::Transport => {
                SegmentedDownloadOutcome::SourceFailed
            }
            SegmentDownloadError::Fatal(error) => {
                SegmentedDownloadOutcome::Fatal(error)
            }
        };
    }

    finalize_segmented_output(
        request,
        route,
        size,
        part_path,
        progress,
        output,
        &mut cleanup_guard,
        transfer_started,
        downloaded,
        final_url,
        initial_ttfb,
        remote_addr,
        http_version,
    )
    .await
}
