use super::SegmentedDownloadContext;
use super::SegmentedDownloadOutcome;
use super::client::{
    DownloadClients, HTTP1_DIRECT_REQWEST_CLIENT,
    HTTP1_NO_REDIRECT_REQWEST_CLIENT, is_h2_protocol_failure,
};
#[cfg(test)]
use super::file;
use super::file::{
    IntegrityHashers, any_route_can_resume, create_download_file,
    finalize_download, hash_existing_part_prefix,
    open_download_file_for_append, parse_content_range,
    preserve_or_remove_partial, remove_if_exists, resource_validator_changed,
    response_status_error,
};
#[cfg(test)]
use super::file::{copy, write};
use super::model::{
    DownloadRequest, DownloadResult, DownloadRoute, FetchProgressFn,
};
use super::policy::{
    disable_range_splitting, fetch_retry_delay, range_splitting_allowed,
    record_native_status_failure, record_native_transfer_failure,
    record_route_failure, record_route_success, retry_after,
};
#[cfg(test)]
use super::probe;
use super::probe::{
    RouteProbeFuture, expected_route_speed, probe_recovery_route,
};
use super::record_install_download_progress;
use super::record_install_download_stage;
use super::record_install_download_started;
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::request::fetch;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::{
    cache_busted_download_url, is_official_route, official_fallback_route,
    original_route_authority, route_host, routes_share_effective_authority,
    sanitize_url_for_log, url_authority,
};
use super::segment::{
    acquire_native_connection, allow_low_throughput_route_switch,
    can_recover_file_ranges, native_first_byte_timeout,
    should_downgrade_slow_http2, should_use_segmented_download,
};
#[cfg(test)]
use super::tail;
use super::throttle::{FetchSemaphore, record_authority_h2_failure_for};
#[cfg(test)]
use super::transfer;
use super::try_segmented_download;
use crate::ErrorKind;
use crate::install::DownloadItemStatus;
use crate::util::download::constants::{
    DIAGNOSTICS_NETWORK, DIAGNOSTICS_RANGE,
    DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS, DOWNLOAD_SCHEDULER_INTERVAL,
    FETCH_ABORT, FETCH_CLEAR_PARTIAL_AND_RETRY, FETCH_CLEAR_PARTIAL_AND_SWITCH,
    FETCH_COOLDOWN_AND_SWITCH, FETCH_COOLDOWN_THEN_RETRY,
    FETCH_DISABLE_RANGE_AND_RETRY_SINGLE, FETCH_DISABLE_RANGE_AND_SWITCH,
    FETCH_DROP_ROUTE, FETCH_DROP_ROUTE_AFTER_CLEAN_RETRY,
    FETCH_FALLBACK_OFFICIAL, FETCH_FALLBACK_SINGLE, FETCH_RESUME_OR_SWITCH,
    FETCH_RETRY_NEXT_ROUND, FETCH_SWITCH_ROUTE_OR_RETRY_ROUND,
    FETCH_TIMED_OUT_WAITING_FOR_NATIVE_DOWNLOAD_RESOURCES,
    FILE_RETRY_SINGLE_STREAM_ROUNDS, MIN_PARALLEL_RANGES, MIN_SEGMENT_SIZE,
    RESOURCE_WAIT_TIMEOUT, SEGMENTED_DOWNLOAD_THRESHOLD,
};
use crate::util::download::diagnostics::{
    DownloadAttemptDiagnostic, attach_download_attempt_history,
    push_download_attempt_diagnostic, record_download_attempt_failure,
};
use crate::util::download::integrity::{
    validate_file_content, verify_computed_integrity,
};
use crate::util::download::native_request::{
    send_path_request, send_path_request_with_clients,
};
use crate::util::io::IOError;
use futures::StreamExt;
#[cfg(test)]
use reqwest::header;
use reqwest::{Method, StatusCode};
use std::collections::{HashSet, VecDeque};
#[cfg(test)]
use std::error::Error;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time;
use std::time::Instant;
#[cfg(test)]
use tokio::fs::File;

enum NativeSegmentedAttempt {
    RetryRoute,
    Completed(DownloadResult),
}

async fn try_segmented_native_attempt(
    request: &DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
    progress: &mut Option<&mut FetchProgressFn<'_>>,
    routes: &[DownloadRoute],
    route_index: usize,
    route: &DownloadRoute,
    part_path: &Path,
    credentials: Option<&crate::state::ModrinthCredentials>,
    session: &mut NativeDownloadSession,
    retry_with_single_thread: bool,
    allow_low_throughput_abort: bool,
) -> crate::Result<Option<NativeSegmentedAttempt>> {
    let log_url = sanitize_url_for_log(&route.url);
    let resumable_part_bytes = match (
        request.integrity.supports_resume(),
        request.integrity.size,
        tokio::fs::metadata(part_path).await,
    ) {
        (true, Some(expected), Ok(metadata))
            if metadata.is_file() && metadata.len() < expected =>
        {
            metadata.len()
        }
        _ => 0,
    };
    let explicitly_parallel = request.range_connection_floor.is_some()
        && request.integrity.supports_resume()
        && request.integrity.size.is_some_and(|size| {
            size.saturating_sub(resumable_part_bytes)
                >= MIN_SEGMENT_SIZE * MIN_PARALLEL_RANGES as u64
        });
    let tail_recovery = request.prefer_http1
        && request.integrity.size.is_some_and(|size| {
            can_recover_file_ranges(
                request,
                size.saturating_sub(resumable_part_bytes),
            )
        });
    if (request.allow_http1_segmented_download || tail_recovery)
        && !retry_with_single_thread
        && !session.single_thread_routes.contains(&route.url)
        && route.supports_range
        && range_splitting_allowed(route)
        && request.integrity.size.is_some_and(|size| {
            tail_recovery
                || explicitly_parallel
                || should_use_segmented_download(size, resumable_part_bytes)
        })
    {
        let size = request.integrity.size.unwrap();
        match try_segmented_download(
            SegmentedDownloadContext::new(
                request,
                route,
                &routes[route_index + 1..],
                size,
                part_path,
                semaphore,
                credentials,
                &HTTP1_NO_REDIRECT_REQWEST_CLIENT,
                &HTTP1_DIRECT_REQWEST_CLIENT,
                session.attempts,
                session.file_attempt_budget,
                allow_low_throughput_abort,
            ),
            progress.as_deref_mut(),
        )
        .await
        {
            SegmentedDownloadOutcome::Success(result) => {
                finalize_download(part_path, destination).await?;
                let transferred_bytes =
                    result.size.saturating_sub(resumable_part_bytes);
                if let Some(authority) = original_route_authority(route) {
                    crate::util::download::native_reputation::record_transport_success(
                        &authority,
                        route.proxy,
                        crate::util::download::native_reputation::NativeTransport::Http1MultiRange,
                        transferred_bytes as f64
                            / result
                                .transfer_elapsed
                                .as_secs_f64()
                                .max(0.001),
                    );
                }
                record_route_success(
                    route,
                    request.resource,
                    result.ttfb,
                    transferred_bytes,
                    result.transfer_elapsed,
                    result.remote_addr,
                );
                tracing::debug!(
                    path = %destination.display(),
                    url = %sanitize_url_for_log(&result.final_url),
                    source = route.source.as_str(),
                    bytes = result.size,
                    elapsed_ms = result.transfer_elapsed.as_millis(),
                    remote_addr = ?result.remote_addr,
                    http_version = ?result.http_version,
                    dns_candidates = ?route_host(route).map(|host| {
                        crate::util::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
                    }),
                    attempt = session.attempts,
                    max_attempts = session.file_attempt_budget,
                    "Completed file download"
                );
                if let Some(tracking) = &request.install_tracking
                    && let Err(error) = tracking
                        .reporter
                        .record_download_request_finished(
                            &tracking.item_id,
                            result.size,
                        )
                        .await
                {
                    tracing::warn!(
                        error = %error,
                        "Failed to record completed download request"
                    );
                }
                return Ok(Some(NativeSegmentedAttempt::Completed(
                    DownloadResult {
                        path: destination.to_path_buf(),
                        url: result.final_url,
                        source: route.source,
                        size: result.size,
                        attempts: session.attempts,
                        fallback_count: session.fallback_count,
                        verified_sha1: request.integrity.sha1.clone(),
                        verified_sha512: request.integrity.sha512.clone(),
                        verified_file: None,
                    },
                )));
            }
            SegmentedDownloadOutcome::FallbackSingle {
                disable_range,
                reason,
            } => {
                push_download_attempt_diagnostic(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    DIAGNOSTICS_RANGE,
                    FETCH_FALLBACK_SINGLE,
                    reason,
                    None,
                    None,
                    None,
                );
                tracing::debug!(
                    original_url = %log_url,
                    file_size = size,
                    supports_range = route.supports_range,
                    reason,
                    "Falling back to a single connection"
                );
                if resource_validator_changed(reason) {
                    session.single_thread_routes.insert(route.url.clone());
                } else if disable_range {
                    disable_range_splitting(route);
                }
            }
            SegmentedDownloadOutcome::SourceFailed => {
                record_route_failure(route, request.resource, None);
                record_native_transfer_failure(route, None);
                let error: crate::Error = ErrorKind::OtherError(format!(
                    "File transfer failed from {log_url}"
                ))
                .into();
                push_download_attempt_diagnostic(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    DIAGNOSTICS_NETWORK,
                    FETCH_SWITCH_ROUTE_OR_RETRY_ROUND,
                    error.to_string(),
                    None,
                    None,
                    None,
                );
                session.last_error = Some(error);
                tracing::warn!(
                    path = %destination.display(),
                    url = %log_url,
                    source = route.source.as_str(),
                    attempt = session.attempts,
                    max_attempts = session.file_attempt_budget,
                    "Segmented file download failed; retrying or switching source"
                );
                return Ok(Some(NativeSegmentedAttempt::RetryRoute));
            }
            SegmentedDownloadOutcome::IntegrityFailed(error) => {
                record_route_failure(route, request.resource, None);
                record_download_attempt_failure(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    &error,
                    if is_official_route(route) {
                        FETCH_ABORT
                    } else {
                        FETCH_FALLBACK_OFFICIAL
                    },
                    None,
                    None,
                    None,
                );
                session.last_error = Some(error);
                session.terminal_routes.insert(route.url.clone());
                if !is_official_route(route)
                    && let Some(official) = official_fallback_route(routes)
                {
                    remove_if_exists(part_path).await?;
                    session.official_integrity_retry = true;
                    session.preferred_route = Some(official);
                    return Ok(Some(NativeSegmentedAttempt::RetryRoute));
                }
                if session.official_integrity_retry {
                    return Err(attach_download_attempt_history(
                        session.last_error.take().unwrap(),
                        &session.attempt_history,
                        session.attempts,
                        session.file_attempt_budget,
                    ));
                }
                disable_range_splitting(route);
                session.single_thread_routes.insert(route.url.clone());
                session.terminal_routes.remove(&route.url);
            }
            SegmentedDownloadOutcome::SwitchRoute(probe) => {
                session.preferred_route = Some(probe.route);
                return Ok(Some(NativeSegmentedAttempt::RetryRoute));
            }
            SegmentedDownloadOutcome::Fatal(error) => {
                record_download_attempt_failure(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    &error,
                    FETCH_ABORT,
                    None,
                    None,
                    None,
                );
                return Err(attach_download_attempt_history(
                    error,
                    &session.attempt_history,
                    session.attempts,
                    session.file_attempt_budget,
                ));
            }
        }
    }

    Ok(None)
}

struct NativeDownloadSession {
    official_integrity_retry: bool,
    attempts: usize,
    last_error: Option<crate::Error>,
    attempt_history: VecDeque<DownloadAttemptDiagnostic>,
    fallback_count: usize,
    partial_route_index: Option<usize>,
    terminal_routes: HashSet<String>,
    preferred_route: Option<DownloadRoute>,
    single_thread_routes: HashSet<String>,
    busted_for_route: Option<(usize, String)>,
    file_attempt_budget: usize,
}

impl NativeDownloadSession {
    fn new(
        route_count: usize,
        h2_failed_nonofficial: Option<String>,
    ) -> (Self, Option<String>) {
        let official_integrity_retry = h2_failed_nonofficial.is_some();
        (
            Self {
                official_integrity_retry,
                attempts: 0,
                last_error: None,
                attempt_history: VecDeque::new(),
                fallback_count: 0,
                partial_route_index: None,
                terminal_routes: HashSet::new(),
                preferred_route: None,
                single_thread_routes: HashSet::new(),
                busted_for_route: None,
                file_attempt_budget: route_count
                    .saturating_mul(FILE_RETRY_SINGLE_STREAM_ROUNDS.len())
                    .max(1),
            },
            h2_failed_nonofficial,
        )
    }

    fn initialize_preferred_route(&mut self, routes: &[DownloadRoute]) {
        self.preferred_route = self
            .official_integrity_retry
            .then(|| official_fallback_route(routes))
            .flatten();
    }

    fn take_final_error(&mut self, request: &DownloadRequest) -> crate::Error {
        self.last_error.take().unwrap_or_else(|| {
            ErrorKind::OtherError(format!(
                "Unable to download {} from any source",
                sanitize_url_for_log(&request.url)
            ))
            .into()
        })
    }
}

pub(crate) async fn run_native_download_attempts(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
    mut progress: Option<&mut FetchProgressFn<'_>>,
    mut routes: Vec<DownloadRoute>,
    part_path: PathBuf,
    h2_failed_nonofficial: Option<String>,
) -> crate::Result<DownloadResult> {
    let credentials: Option<crate::state::ModrinthCredentials> = None;
    let (mut session, mut h2_failed_nonofficial) =
        NativeDownloadSession::new(routes.len(), h2_failed_nonofficial);
    if let Some(failed_route) = h2_failed_nonofficial.take() {
        routes.retain(|route| route.url != failed_route);
    }
    session.initialize_preferred_route(&routes);
    for (round, retry_with_single_thread) in
        FILE_RETRY_SINGLE_STREAM_ROUNDS.into_iter().enumerate()
    {
        let mut attempted_routes = Vec::new();
        for (route_index, route) in routes.iter().enumerate() {
            if session.terminal_routes.contains(&route.url) {
                continue;
            }
            let has_breaker_alternate = routes.iter().enumerate().any(
                |(candidate_index, candidate)| {
                    candidate_index != route_index
                        && !session.terminal_routes.contains(&candidate.url)
                        && !crate::util::download::native_breaker::is_open(
                            candidate,
                        )
                },
            );
            let recovery_route = session.preferred_route.as_ref()
                == Some(route)
                || session.single_thread_routes.contains(&route.url);
            if !recovery_route
                && crate::util::download::native_breaker::should_skip(
                    route,
                    has_breaker_alternate,
                )
            {
                continue;
            }
            if session
                .preferred_route
                .as_ref()
                .is_some_and(|preferred| preferred != route)
            {
                continue;
            }
            if session.preferred_route.as_ref() == Some(route) {
                session.preferred_route = None;
            }
            if attempted_routes.iter().any(|attempted: &&DownloadRoute| {
                routes_share_effective_authority(attempted, route)
            }) {
                continue;
            }
            attempted_routes.push(route);
            if route_index > 0 {
                session.fallback_count += 1;
            }
            if session
                .partial_route_index
                .is_some_and(|index| index != route_index)
            {
                remove_if_exists(&part_path).await?;
            }
            session.partial_route_index = Some(route_index);
            match run_native_route_attempts(
                &request,
                destination,
                semaphore,
                &mut progress,
                &routes,
                route_index,
                route,
                &part_path,
                credentials.as_ref(),
                &mut session,
                retry_with_single_thread,
            )
            .await?
            {
                NativeRouteAttempt::Completed(result) => return Ok(result),
                NativeRouteAttempt::Continue => {}
            }
        }
        if round + 1 < FILE_RETRY_SINGLE_STREAM_ROUNDS.len()
            && routes
                .iter()
                .any(|route| !session.terminal_routes.contains(&route.url))
        {
            tokio::time::sleep(fetch_retry_delay(round + 1)).await;
        }
    }

    preserve_or_remove_partial(
        &part_path,
        &request.integrity,
        any_route_can_resume(&routes),
    )
    .await?;
    let error = session.take_final_error(&request);
    Err(attach_download_attempt_history(
        error,
        &session.attempt_history,
        session.attempts,
        session.file_attempt_budget,
    ))
}
/// Posts a JSON to a URL

enum NativeRouteAttempt {
    Continue,
    Completed(DownloadResult),
}

async fn run_native_route_attempts(
    request: &DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
    progress: &mut Option<&mut FetchProgressFn<'_>>,
    routes: &[DownloadRoute],
    route_index: usize,
    route: &DownloadRoute,
    part_path: &Path,
    credentials: Option<&crate::state::ModrinthCredentials>,
    session: &mut NativeDownloadSession,
    retry_with_single_thread: bool,
) -> crate::Result<NativeRouteAttempt> {
    let log_url = sanitize_url_for_log(&route.url);
    let can_switch_route =
        routes
            .iter()
            .enumerate()
            .any(|(candidate_index, candidate)| {
                candidate_index != route_index
                    && !session.terminal_routes.contains(&candidate.url)
            });
    let allow_low_throughput_abort = allow_low_throughput_route_switch(
        can_switch_route,
        retry_with_single_thread,
    );
    let mut batch_ranges_attempted = false;
    'attempt: loop {
        if session.attempts >= session.file_attempt_budget {
            break 'attempt;
        }
        session.attempts += 1;
        tracing::debug!(
            path = %destination.display(),
            temporary_path = %part_path.display(),
            url = %log_url,
            source = route.source.as_str(),
            expected_bytes = request.integrity.size,
            proxy = ?route.proxy,
            attempt = session.attempts,
            max_attempts = session.file_attempt_budget,
            "Starting file download attempt"
        );
        if let Some(decision) = try_segmented_native_attempt(
            request,
            destination,
            semaphore,
            progress,
            routes,
            route_index,
            route,
            part_path,
            credentials,
            session,
            retry_with_single_thread,
            allow_low_throughput_abort,
        )
        .await?
        {
            match decision {
                NativeSegmentedAttempt::Completed(result) => {
                    return Ok(NativeRouteAttempt::Completed(result));
                }
                NativeSegmentedAttempt::RetryRoute => break 'attempt,
            }
        }

        let expected_size = request.integrity.size;
        let mut resume_offset = if route.supports_range
            && range_splitting_allowed(route)
            && request.integrity.supports_resume()
        {
            match (expected_size, tokio::fs::metadata(&part_path).await) {
                (Some(expected), Ok(metadata))
                    if metadata.is_file()
                        && metadata.len() > 0
                        && metadata.len() < expected =>
                {
                    metadata.len()
                }
                _ => 0,
            }
        } else {
            0
        };
        let mut activity = crate::State::get_if_initialized()
            .map(|state| state.begin_download_connection());
        record_install_download_started(
            request,
            route,
            session.attempts,
            session.file_attempt_budget,
        )
        .await;
        record_install_download_stage(
            request,
            DownloadItemStatus::WaitingForResource,
        )
        .await;
        let permit_wait = tokio::time::timeout(
            RESOURCE_WAIT_TIMEOUT,
            acquire_native_connection(route, semaphore),
        );
        let resource_wait_started = Instant::now();
        let permit = if let Some(cancellation) = request.cancellation.as_ref() {
            tokio::select! {
                _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("download canceled while waiting for native resources".to_string()).into()),
                result = permit_wait => result,
            }
        } else {
            permit_wait.await
        }
        .map_err(|_| {
            ErrorKind::DownloadResourceWaitTimeout(
                FETCH_TIMED_OUT_WAITING_FOR_NATIVE_DOWNLOAD_RESOURCES
                    .to_string(),
            )
        })??;
        tracing::debug!(
            route = %sanitize_url_for_log(&route.url),
            resource = "native_connection_and_fetch",
            wait_ms = resource_wait_started.elapsed().as_millis(),
            "Acquired native download resources"
        );
        record_install_download_stage(request, DownloadItemStatus::Downloading)
            .await;
        let request_started = Instant::now();
        let first_byte_timeout =
            native_first_byte_timeout(route, can_switch_route);
        // A truncated or invalid body may be a corrupt edge-cache
        // object; the retry then uses a cache-busted URL that forces a
        // fresh origin fetch instead of the same broken copy.
        let attempt_route = session
            .busted_for_route
            .as_ref()
            .filter(|(index, _)| *index == route_index)
            .map(|(_, url)| {
                let mut busted = route.clone();
                busted.url = url.clone();
                busted
            })
            .unwrap_or_else(|| route.clone());
        let (response, final_url) = match tokio::time::timeout(
            first_byte_timeout,
            async {
                if request.prefer_http1 || batch_ranges_attempted {
                    let clients = DownloadClients::for_request(
                        &HTTP1_NO_REDIRECT_REQWEST_CLIENT,
                        &HTTP1_DIRECT_REQWEST_CLIENT,
                    );
                    send_path_request_with_clients(
                        &attempt_route,
                        request.header.as_ref(),
                        credentials,
                        request.download_meta.as_ref(),
                        (resume_offset > 0).then_some(resume_offset),
                        None,
                        &clients,
                        None,
                    )
                    .await
                } else {
                    send_path_request(
                        &attempt_route,
                        request.header.as_ref(),
                        credentials,
                        request.download_meta.as_ref(),
                        (resume_offset > 0).then_some(resume_offset),
                        None,
                    )
                    .await
                }
            },
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                drop(permit);
                drop(activity.take());
                record_route_failure(route, request.resource, None);
                record_native_transfer_failure(route, None);
                record_download_attempt_failure(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    &error,
                    FETCH_SWITCH_ROUTE_OR_RETRY_ROUND,
                    None,
                    None,
                    None,
                );
                tracing::warn!(
                    path = %destination.display(),
                    url = %log_url,
                    source = route.source.as_str(),
                    attempt = session.attempts,
                    max_attempts = session.file_attempt_budget,
                    error = %error,
                    "File download request failed; trying the next source or retry"
                );
                session.last_error = Some(error);
                break 'attempt;
            }
            Err(_) => {
                drop(permit);
                drop(activity.take());
                record_route_failure(route, request.resource, None);
                record_native_transfer_failure(route, None);
                let error = ErrorKind::NetworkError(format!(
                    "no response received for {:.0} seconds while downloading {log_url} to {}",
                    first_byte_timeout.as_secs_f64(),
                    destination.display(),
                ))
                .into();
                record_download_attempt_failure(
                    &mut session.attempt_history,
                    route,
                    session.attempts,
                    &error,
                    FETCH_SWITCH_ROUTE_OR_RETRY_ROUND,
                    None,
                    None,
                    None,
                );
                tracing::warn!(
                    path = %destination.display(),
                    url = %log_url,
                    source = route.source.as_str(),
                    no_data_seconds = first_byte_timeout.as_secs_f64(),
                    downloaded_bytes = 0,
                    attempt = session.attempts,
                    max_attempts = session.file_attempt_budget,
                    "File download stalled before receiving a response"
                );
                session.last_error = Some(error);
                break 'attempt;
            }
        };
        let ttfb = request_started.elapsed();
        let status = response.status();
        let remote_addr = response.remote_addr();
        let http_version = response.version();
        let downgrade_slow_http2 =
            should_downgrade_slow_http2(http_version, retry_with_single_thread);
        let response_retry_after = retry_after(&response);
        tracing::debug!(
            path = %destination.display(),
            url = %log_url,
            source = route.source.as_str(),
            status = status.as_u16(),
            content_length = response.content_length(),
            ttfb_ms = ttfb.as_millis(),
            remote_addr = ?remote_addr,
            http_version = ?http_version,
            dns_candidates = ?route_host(route).map(|host| {
                crate::util::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
            }),
            "Received file download response"
        );
        if status.is_client_error() || status.is_server_error() {
            if status != StatusCode::RANGE_NOT_SATISFIABLE {
                record_route_failure(
                    route,
                    request.resource,
                    (status == StatusCode::TOO_MANY_REQUESTS).then_some(
                        response_retry_after.unwrap_or_else(|| {
                            fetch_retry_delay(session.attempts)
                        }),
                    ),
                );
                if status == StatusCode::TOO_MANY_REQUESTS
                    || status.is_server_error()
                {
                    // A server that sends Retry-After is asking for that pause
                    // whether it answered 429 or 503, so keep it for both.
                    record_native_status_failure(
                        route,
                        status,
                        response_retry_after,
                    );
                }
            }
            let error =
                response_status_error(response, &Method::GET, &route.url).await;
            drop(permit);
            drop(activity.take());
            if status == StatusCode::RANGE_NOT_SATISFIABLE {
                remove_if_exists(part_path).await?;
                disable_range_splitting(route);
                session.single_thread_routes.insert(route.url.clone());
                session.preferred_route = Some(route.clone());
            }
            let terminal_status = matches!(
                status,
                StatusCode::UNAUTHORIZED
                    | StatusCode::FORBIDDEN
                    | StatusCode::NOT_FOUND
                    | StatusCode::GONE
            );
            let cooldown_and_switch =
                status == StatusCode::TOO_MANY_REQUESTS && routes.len() > 1;
            if terminal_status || cooldown_and_switch {
                session.terminal_routes.insert(route.url.clone());
            } else if status == StatusCode::TOO_MANY_REQUESTS
                && routes.len() == 1
                && session.attempts < session.file_attempt_budget
            {
                tokio::time::sleep(
                    response_retry_after
                        .unwrap_or_else(|| fetch_retry_delay(session.attempts)),
                )
                .await;
            }
            let decision = if status == StatusCode::RANGE_NOT_SATISFIABLE {
                FETCH_DISABLE_RANGE_AND_RETRY_SINGLE
            } else if terminal_status {
                FETCH_DROP_ROUTE
            } else if cooldown_and_switch {
                FETCH_COOLDOWN_AND_SWITCH
            } else if status == StatusCode::TOO_MANY_REQUESTS {
                FETCH_COOLDOWN_THEN_RETRY
            } else {
                FETCH_RETRY_NEXT_ROUND
            };
            record_download_attempt_failure(
                &mut session.attempt_history,
                route,
                session.attempts,
                &error,
                decision,
                Some(status),
                remote_addr,
                Some(http_version),
            );
            session.last_error = Some(error);
            break 'attempt;
        }

        let mut hashers =
            IntegrityHashers::new_integrity_hashers(&request.integrity);
        if resume_offset > 0 {
            if status == StatusCode::PARTIAL_CONTENT {
                let content_range = parse_content_range(&response);
                let content_range_valid = content_range.is_some_and(|range| {
                    let total = range.total.or(expected_size);
                    range.start == resume_offset
                        && total == expected_size
                        && Some(range.end.saturating_add(1)) == total
                });
                if !content_range_valid {
                    drop(permit);
                    drop(activity.take());
                    record_route_failure(route, request.resource, None);
                    disable_range_splitting(route);
                    preserve_or_remove_partial(
                        part_path,
                        &request.integrity,
                        any_route_can_resume(routes),
                    )
                    .await?;
                    let error: crate::Error =
                        ErrorKind::OtherError(format!(
                            "Invalid Content-Range while resuming download from {log_url}"
                        ))
                        .into();
                    record_download_attempt_failure(
                        &mut session.attempt_history,
                        route,
                        session.attempts,
                        &error,
                        FETCH_DISABLE_RANGE_AND_SWITCH,
                        Some(status),
                        remote_addr,
                        Some(http_version),
                    );
                    session.last_error = Some(error);
                    break 'attempt;
                }
                // Hashing the existing prefix is deferred until the
                // resume response is validated, so routes that fail
                // before sending data never pay a full re-read of a
                // potentially huge partial file.
                match hash_existing_part_prefix(
                    part_path,
                    &request.integrity,
                    resume_offset,
                )
                .await
                {
                    Some(prefix_hashers) => {
                        tracing::debug!(
                            path = %destination.display(),
                            url = %log_url,
                            resume_offset,
                            "Resuming file download from existing partial data"
                        );
                        hashers = prefix_hashers;
                    }
                    None => {
                        drop(permit);
                        drop(activity.take());
                        remove_if_exists(part_path).await?;
                        let error: crate::Error =
                            ErrorKind::OtherError(format!(
                                "Partial download changed on disk while resuming {log_url}"
                            ))
                            .into();
                        record_download_attempt_failure(
                            &mut session.attempt_history,
                            route,
                            session.attempts,
                            &error,
                            FETCH_CLEAR_PARTIAL_AND_RETRY,
                            Some(status),
                            remote_addr,
                            Some(http_version),
                        );
                        session.last_error = Some(error);
                        break 'attempt;
                    }
                }
            } else {
                // The server ignored the Range header and replied with
                // the full file; restart the transfer from scratch.
                disable_range_splitting(route);
                resume_offset = 0;
            }
        }

        let starting_size = resume_offset;
        let file = if starting_size > 0 {
            match open_download_file_for_append(part_path).await {
                Ok(file) => file,
                Err(error) => {
                    drop(permit);
                    drop(activity.take());
                    remove_if_exists(part_path).await?;
                    let error: crate::Error = error.into();
                    record_download_attempt_failure(
                        &mut session.attempt_history,
                        route,
                        session.attempts,
                        &error,
                        FETCH_CLEAR_PARTIAL_AND_RETRY,
                        Some(status),
                        remote_addr,
                        Some(http_version),
                    );
                    session.last_error = Some(error);
                    break 'attempt;
                }
            }
        } else {
            create_download_file(part_path).await?
        };
        let mut file = crate::util::download::stream_output::StreamOutput::new(
            file, part_path,
        );
        let response_length = response.content_length().unwrap_or(0);
        let endpoint_feedback = response.extensions().get::<crate::util::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
        let total_size = request
            .integrity
            .size
            .unwrap_or(starting_size.saturating_add(response_length));
        let transfer_started = Instant::now();
        let mut downloaded = starting_size;
        let mut batch_tail =
            crate::util::download::h2_receive::BatchTailWindow::new(
                transfer_started,
            );
        let mut recover_file_ranges = !batch_ranges_attempted
            && !retry_with_single_thread
            && !session.single_thread_routes.contains(&route.url)
            && request.integrity.size.is_none()
            && request.prefers_independent_ranges(total_size)
            && route.supports_range
            && range_splitting_allowed(route);
        let mut progress_gate =
            crate::util::download::h2_receive::H2ProgressGate::with_starting_bytes(
                total_size,
                starting_size,
            );
        let expected_speed = expected_route_speed(route, request.resource);
        let mut slow_policy = if downgrade_slow_http2 {
            crate::util::download::native_slow::NativeSlowPolicy::for_http2(
                starting_size,
                expected_speed,
            )
        } else {
            crate::util::download::native_slow::NativeSlowPolicy::new(
                starting_size,
                expected_speed,
            )
        };
        let mut throughput_timer =
            tokio::time::interval(DOWNLOAD_SCHEDULER_INTERVAL);
        throughput_timer.tick().await;
        let mut stream = response.bytes_stream();
        let mut alternate_probe: Option<RouteProbeFuture<'_>> = None;
        let mut alternate_probe_finished = false;
        let mut confirmed_switch = None;
        let mut transfer_error: Option<crate::Error> = None;
        let mut switched_endpoint = false;
        let mut last_endpoint_speed = 0;
        while !recover_file_ranges {
            tokio::select! {
                item = async {
                    #[cfg(test)]
                    let _phase = request.transfer_counters.phases.receive();
                    stream.next().await
                } => {
                    let Some(item) = item else {
                        break;
                    };
                    let chunk = match item {
                        Ok(chunk) => chunk,
                        Err(error) => {
                            if is_h2_protocol_failure(&error)
                                && let Some(authority) =
                                    url_authority(&final_url)
                            {
                                record_authority_h2_failure_for(&authority, route.proxy);
                            }
                            transfer_error = Some(error.into());
                            break;
                        }
                    };
                    request.transfer_counters.received(chunk.len() as u64);
                    if let Some(feedback) = &endpoint_feedback { feedback.record(chunk.len(), crate::util::download::local_resources::pressure(part_path)); }
                    {
                    #[cfg(test)]
                    let _phase = request.transfer_counters.phases.output();
                    file.write_chunk(&chunk).await.map_err(|error| {
                        IOError::with_path(error, part_path)
                    })?;
                    }
                    {
                    #[cfg(test)]
                    let _phase = request.transfer_counters.phases.hash();
                    hashers.update(&chunk);
                    }
                    downloaded += chunk.len() as u64;
                    request.transfer_counters.commit(starting_size, downloaded.saturating_sub(file.pending_bytes()));
                    if let Some(state) = crate::State::get_if_initialized() {
                        state.record_download_bytes(chunk.len() as u64);
                    }
                    let persisted = downloaded.saturating_sub(file.pending_bytes());
                    if progress_gate.should_report(persisted, total_size) {
                        record_install_download_progress(
                            request, persisted, total_size,
                        )
                        .await;
                    }
                    if let Some(progress) = progress.as_mut()
                        && let Err(error) = progress(persisted, total_size).await
                    {
                        tracing::warn!(%error, "Download progress callback failed");
                    }
                }
                _ = throughput_timer.tick() => {
                    let persisted = downloaded.saturating_sub(file.pending_bytes());
                    if progress_gate.should_report(persisted, total_size) {
                        record_install_download_progress(request, persisted, total_size).await;
                    }
                    let can_recover = !batch_ranges_attempted
                        && !retry_with_single_thread
                        && !session.single_thread_routes.contains(&route.url)
                        && route.supports_range
                        && range_splitting_allowed(route)
                        && can_recover_file_ranges(request, total_size.saturating_sub(downloaded))
                        && !crate::util::download::local_resources::pressure(part_path).blocks_expansion()
                        && semaphore.0.available_permits() >= crate::util::download::constants::BATCH_DOWNLOAD_CONNECTION_FLOOR
                        && crate::util::download::native_budget::available(route) >= crate::util::download::constants::BATCH_DOWNLOAD_CONNECTION_FLOOR;
                    if batch_tail.observe(downloaded.saturating_sub(starting_size), can_recover, Instant::now()) {
                        recover_file_ranges = true;
                        break;
                    }
                    let slow_decision = slow_policy.observe_with_pressure(
                        downloaded,
                        total_size.saturating_sub(downloaded),
                        crate::util::download::local_resources::pressure(part_path),
                    );
                    if let crate::util::download::native_slow::SlowDecision::Probe { bytes_per_second } = slow_decision {
                        last_endpoint_speed = bytes_per_second;
                    }
                    if !crate::util::download::local_resources::pressure(part_path).blocks_expansion()
                        && let Some(clients) = crate::util::download::proxy_context::clients() {
                        let protocol = if http_version == reqwest::Version::HTTP_2 {
                            crate::util::download::endpoint_selection::Protocol::H2
                        } else { crate::util::download::endpoint_selection::Protocol::Http1 };
                        if clients.endpoints.recovery_lease(request, route, protocol, remote_addr,
                            last_endpoint_speed, total_size.saturating_sub(downloaded)).is_some() {
                            switched_endpoint = true;
                            break;
                        }
                    }
                    if matches!(slow_decision, crate::util::download::native_slow::SlowDecision::Probe { .. })
                        && let Some(clients) = crate::util::download::proxy_context::clients()
                    {
                        let protocol = if http_version == reqwest::Version::HTTP_2 {
                            crate::util::download::endpoint_selection::Protocol::H2
                        } else { crate::util::download::endpoint_selection::Protocol::Http1 };
                        clients.endpoints.on_slow_region(request, route, &clients, protocol,
                            remote_addr.map(|address| address.ip()), downloaded..total_size, Some(last_endpoint_speed),
                            crate::util::download::local_resources::pressure(part_path));
                        if clients.endpoints.is_probing() { continue; }
                    }
                    if downgrade_slow_http2 && alternate_probe_finished
                        && let crate::util::download::native_slow::SlowDecision::Probe {
                            bytes_per_second,
                        } = slow_decision
                    {
                        tracing::debug!(
                            path = %destination.display(),
                            url = %log_url,
                            source = route.source.as_str(),
                            http_version = ?http_version,
                            remote_addr = ?remote_addr,
                            downloaded_bytes = downloaded,
                            elapsed_ms = transfer_started.elapsed().as_millis(),
                            reason = "sustained_slow_after_route_probes",
                            bytes_per_second,
                            remaining_bytes = total_size.saturating_sub(downloaded),
                            "HTTP/2 transfer remains slow without a proven faster route"
                        );
                    }
                    if (downgrade_slow_http2 || (allow_low_throughput_abort
                        && total_size >= SEGMENTED_DOWNLOAD_THRESHOLD))
                        && !alternate_probe_finished
                        && alternate_probe.is_none()
                        && let crate::util::download::native_slow::SlowDecision::Probe {
                            bytes_per_second,
                        } = slow_decision
                    {
                        tracing::warn!(
                            path = %destination.display(),
                            url = %log_url,
                            source = route.source.as_str(),
                            bytes_per_second,
                            remaining_bytes = total_size.saturating_sub(downloaded),
                            "Sustained low throughput; probing alternate routes"
                        );
                        alternate_probe = Some(Box::pin(probe_recovery_route(
                            request, route, &routes[route_index + 1..],
                            bytes_per_second, total_size, downloaded,
                            downgrade_slow_http2, credentials, semaphore,
                        )));
                    }
                    if (allow_low_throughput_abort || downgrade_slow_http2)
                        && let crate::util::download::native_slow::SlowDecision::Idle {
                            elapsed,
                        } = slow_decision
                    {
                        tracing::warn!(
                            path = %destination.display(),
                            url = %log_url,
                            source = route.source.as_str(),
                            http_version = ?http_version,
                            remote_addr = ?remote_addr,
                            downloaded_bytes = downloaded,
                            elapsed_ms = transfer_started.elapsed().as_millis(),
                            reason = "body_idle_timeout",
                            idle_ms = elapsed.as_millis(),
                            "Download body idle deadline exceeded"
                        );
                        transfer_error = Some(crate::ErrorKind::NetworkError(
                            format!("download body idle for {}", elapsed.as_secs()),
                        ).into());
                        break;
                    }
                    if matches!(
                        slow_decision,
                        crate::util::download::native_slow::SlowDecision::Commit
                    ) {
                        alternate_probe = None;
                        alternate_probe_finished = true;
                        slow_policy.commit();
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
                            original_url = %log_url,
                            source = route.source.as_str(),
                            http_version = ?http_version,
                            remote_addr = ?remote_addr,
                            downloaded_bytes = downloaded,
                            elapsed_ms = transfer_started.elapsed().as_millis(),
                            reason = "faster_route_confirmed",
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
            }
        }
        drop(alternate_probe);
        drop(stream);
        file.flush()
            .await
            .map_err(|error| IOError::with_path(error, part_path))?;
        request.transfer_counters.commit(starting_size, downloaded);
        record_install_download_progress(request, downloaded, total_size).await;
        if transfer_error.is_some() {
            // Best-effort durability for data a later resume builds
            // on; a power loss could otherwise leave a zero-filled
            // tail that wastes the resumed transfer.
            let _ = crate::util::download::local_resources::write(
                part_path,
                0,
                file.sync_data(),
            )
            .await;
        }
        drop(file);
        drop(permit);
        drop(activity.take());

        if recover_file_ranges {
            batch_ranges_attempted = true;
            let mut recovery_request = request.clone();
            recovery_request.prefer_http1 = true;
            recovery_request.integrity.size.get_or_insert(total_size);
            tracing::debug!(
                url = %log_url,
                downloaded_bytes = downloaded,
                remaining_bytes = total_size.saturating_sub(downloaded),
                "Promoting a file connection to adaptive parallel ranges"
            );
            match try_segmented_native_attempt(
                &recovery_request,
                destination,
                semaphore,
                progress,
                routes,
                route_index,
                route,
                part_path,
                credentials,
                session,
                retry_with_single_thread,
                allow_low_throughput_abort,
            )
            .await?
            {
                Some(NativeSegmentedAttempt::Completed(result)) => {
                    return Ok(NativeRouteAttempt::Completed(result));
                }
                Some(NativeSegmentedAttempt::RetryRoute) => break 'attempt,
                None => {
                    // A rejected Range response resumes the preserved prefix
                    // on one connection without spending a failure retry.
                    session.attempts -= 1;
                    continue 'attempt;
                }
            }
        }

        if switched_endpoint {
            session.preferred_route = Some(route.clone());
            break 'attempt;
        }

        if let Some(probe) = confirmed_switch {
            session.preferred_route = Some(probe.route);
            break 'attempt;
        }

        if let Some(error) = transfer_error {
            record_route_failure(route, request.resource, None);
            record_native_transfer_failure(route, None);
            preserve_or_remove_partial(
                part_path,
                &request.integrity,
                any_route_can_resume(routes),
            )
            .await?;
            record_download_attempt_failure(
                &mut session.attempt_history,
                route,
                session.attempts,
                &error,
                FETCH_RESUME_OR_SWITCH,
                Some(status),
                remote_addr,
                Some(http_version),
            );
            tracing::warn!(
                path = %destination.display(),
                url = %log_url,
                source = route.source.as_str(),
                attempt = session.attempts,
                max_attempts = session.file_attempt_budget,
                error = %error,
                "File download attempt failed; trying the next source or retry"
            );
            session.last_error = Some(error);
            break 'attempt;
        }

        if let Some(expected) = expected_size
            && downloaded < expected
            && !request.integrity.has_hash()
        {
            // No hash to fall back on: a close-delimited body that
            // ends short is a transfer failure, keeping the valid
            // data so far available for a resume. With a hash present
            // the short body is verified below instead, because a
            // broken CDN or manifest can under-report the size while
            // the received content is actually complete and correct.
            record_route_failure(route, request.resource, None);
            preserve_or_remove_partial(
                part_path,
                &request.integrity,
                any_route_can_resume(routes),
            )
            .await?;
            let error: crate::Error = ErrorKind::OtherError(format!(
                "Truncated response from {log_url}: received {downloaded} of {expected} bytes"
            ))
            .into();
            record_download_attempt_failure(
                &mut session.attempt_history,
                route,
                session.attempts,
                &error,
                FETCH_RESUME_OR_SWITCH,
                Some(status),
                remote_addr,
                Some(http_version),
            );
            if session.attempts < session.file_attempt_budget {
                session.busted_for_route = Some((
                    route_index,
                    cache_busted_download_url(&route.url, session.attempts),
                ));
            }
            session.last_error = Some(error);
            break 'attempt;
        }
        record_install_download_stage(request, DownloadItemStatus::Verifying)
            .await;
        let computed = hashers.finish(downloaded);
        if let Err(error) =
            verify_computed_integrity(&request.integrity, &computed)
        {
            record_route_failure(route, request.resource, None);
            // A short body is kept as a resumable partial; a body that
            // arrived in full is discarded so the retry restarts.
            if downloaded < expected_size.unwrap_or(0) {
                preserve_or_remove_partial(
                    part_path,
                    &request.integrity,
                    any_route_can_resume(routes),
                )
                .await?;
            } else {
                remove_if_exists(part_path).await?;
            }
            let official = (!is_official_route(route))
                .then(|| official_fallback_route(routes))
                .flatten();
            let decision = if official.is_some() {
                FETCH_FALLBACK_OFFICIAL
            } else if session.attempts >= DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS
                || (is_official_route(route)
                    && session.official_integrity_retry)
            {
                FETCH_DROP_ROUTE_AFTER_CLEAN_RETRY
            } else {
                FETCH_CLEAR_PARTIAL_AND_RETRY
            };
            record_download_attempt_failure(
                &mut session.attempt_history,
                route,
                session.attempts,
                &error,
                decision,
                Some(status),
                remote_addr,
                Some(http_version),
            );
            if session.attempts < session.file_attempt_budget {
                session.busted_for_route = Some((
                    route_index,
                    cache_busted_download_url(&route.url, session.attempts),
                ));
            }
            session.last_error = Some(error);
            if let Some(official) = official {
                session.terminal_routes.insert(route.url.clone());
                session.official_integrity_retry = true;
                session.preferred_route = Some(official);
            } else if (is_official_route(route)
                && session.official_integrity_retry)
                || session.attempts >= DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS
            {
                session.terminal_routes.insert(route.url.clone());
                if is_official_route(route) && session.official_integrity_retry
                {
                    return Err(attach_download_attempt_history(
                        session.last_error.take().unwrap(),
                        &session.attempt_history,
                        session.attempts,
                        session.file_attempt_budget,
                    ));
                }
            }
            break 'attempt;
        }
        if let Err(error) =
            validate_file_content(part_path, request.integrity.content).await
        {
            record_route_failure(route, request.resource, None);
            if downloaded < expected_size.unwrap_or(0) {
                preserve_or_remove_partial(
                    part_path,
                    &request.integrity,
                    any_route_can_resume(routes),
                )
                .await?;
            } else {
                remove_if_exists(part_path).await?;
            }
            let decision = if routes.len() > 1 {
                FETCH_CLEAR_PARTIAL_AND_SWITCH
            } else if session.attempts >= DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS {
                FETCH_DROP_ROUTE_AFTER_CLEAN_RETRY
            } else {
                FETCH_CLEAR_PARTIAL_AND_RETRY
            };
            record_download_attempt_failure(
                &mut session.attempt_history,
                route,
                session.attempts,
                &error,
                decision,
                Some(status),
                remote_addr,
                Some(http_version),
            );
            if session.attempts < session.file_attempt_budget {
                session.busted_for_route = Some((
                    route_index,
                    cache_busted_download_url(&route.url, session.attempts),
                ));
            }
            if routes.len() > 1
                || session.attempts >= DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS
            {
                session.terminal_routes.insert(route.url.clone());
            }
            session.last_error = Some(error);
            break 'attempt;
        }

        finalize_download(part_path, destination).await?;
        record_route_success(
            route,
            request.resource,
            ttfb,
            downloaded.saturating_sub(starting_size),
            transfer_started.elapsed(),
            remote_addr,
        );
        let log_final_url = sanitize_url_for_log(&final_url);
        tracing::debug!(
            path = %destination.display(),
            url = %log_final_url,
            source = route.source.as_str(),
            bytes = downloaded.saturating_sub(starting_size),
            elapsed_ms = transfer_started.elapsed().as_millis(),
            remote_addr = ?remote_addr,
            http_version = ?http_version,
            dns_candidates = ?route_host(route).map(|host| {
                crate::util::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
            }),
            "Completed file download"
        );
        if let Some(tracking) = &request.install_tracking
            && let Err(error) = tracking
                .reporter
                .record_download_request_finished(&tracking.item_id, downloaded)
                .await
        {
            tracing::warn!(
                error = %error,
                "Failed to record completed download request"
            );
        }
        return Ok(NativeRouteAttempt::Completed(DownloadResult {
            path: destination.to_path_buf(),
            url: final_url,
            source: route.source,
            size: downloaded,
            attempts: session.attempts,
            fallback_count: session.fallback_count,
            verified_sha1: request.integrity.sha1.clone(),
            verified_sha512: request.integrity.sha512.clone(),
            verified_file: None,
        }));
    }

    Ok(NativeRouteAttempt::Continue)
}
