#[cfg(test)]
use super::file;
use super::file::{
    any_route_can_resume, destination_download_lock, part_resume_expected,
    preserve_or_remove_partial, remove_if_exists, suffixed_path,
};
use super::model::{
    DownloadRequest, DownloadResult, DownloadRoute, DownloadRouteSource,
    FetchProgressFn, Integrity, attach_verified_integrity,
};
#[cfg(test)]
use super::policy;
use super::policy::{
    header_requires_official_only, range_splitting_allowed,
    record_native_transfer_failure, record_route_health_failure,
    record_route_transfer_success,
};
use super::prewarm_download_dns;
use super::probe::prepare_native_download_routes;
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::{
    deduplicate_download_routes, first_h2_route, is_official_route,
    official_route, original_route_authority, resolve_download_routes_for,
    route_host, sanitize_url_for_log, source_mode_for_resource, url_authority,
};
use super::run_native_download_attempts;
use super::segment::cleanup_segment_files;
use super::throttle::{FetchSemaphore, record_authority_h2_failure_for};
use crate::ErrorKind;
use crate::util::download::constants::{
    FETCH_CHECKPOINT_RESUME_FAILED_DUE_TO_LOCAL_I_O, FETCH_DOWNLOAD_CANCELED,
    FETCH_HTTP, FETCH_NCAUSED_BY,
    FETCH_TIMED_OUT_WAITING_FOR_DESTINATION_DOWNLOAD_LOCK,
    MAX_SEGMENT_CONCURRENCY, PARTIAL_FILE_SUFFIX, RESOURCE_WAIT_TIMEOUT,
};
use crate::util::download::integrity::verify_file;
use crate::util::download::retired_sources;
use crate::util::io::{self, IOError};
use std::collections::HashSet;
use std::error::Error;
#[cfg(test)]
use std::future::Future;
use std::path::Path;
#[cfg(test)]
use std::time;
use std::time::Instant;

fn error_chain(error: &crate::Error) -> String {
    let mut chain = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        chain.push_str(FETCH_NCAUSED_BY);
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}

/// Streams a download to a sibling `.part` file, verifies it, then atomically
/// moves it into place.

#[tracing::instrument(skip(semaphore, progress, request, destination))]
pub async fn download_to_path(
    mut request: DownloadRequest,
    destination: impl AsRef<Path>,
    semaphore: &FetchSemaphore,
    progress: Option<&mut FetchProgressFn<'_>>,
) -> crate::Result<DownloadResult> {
    let tracking = request.install_tracking.clone();
    let request_url = request.url.clone();
    let request_counters = request.transfer_counters.clone();
    let destination_path = destination.as_ref();
    let integrity = request.integrity.clone();
    let result = async {
        let primary = retired_sources::normalize(&request.url)
            .map(|url| url.into_owned());
        request.candidate_urls = request
            .candidate_urls
            .iter()
            .filter_map(|url| {
                retired_sources::normalize(url)
                    .ok()
                    .map(|url| url.into_owned())
            })
            .collect::<Vec<_>>();
        match primary {
            Ok(url) => request.url = url,
            Err(error) if request.candidate_urls.is_empty() => {
                return Err(error);
            }
            Err(_) => {}
        }
        crate::util::download::proxy_context::with_clients(
            crate::util::single_flight::run(
                destination_path,
                &integrity,
                || {
                    download_to_path_inner(
                        request,
                        destination_path,
                        semaphore,
                        progress,
                    )
                },
            ),
        )
        .await
    }
    .await;
    tracing::debug!(url = %sanitize_url_for_log(&request_url), counters = ?request_counters.snapshot(), "Download payload accounting");
    if let Err(error) = &result {
        if let Some(state) = crate::State::get_if_initialized() {
            state.record_download_error();
        }
        tracing::debug!(
            url = %request_url,
            destination = %destination_path.display(),
            error = %error,
            error_chain = %error_chain(error),
            "Download failed"
        );
    }
    if result.is_err()
        && let Some(tracking) = tracking
        && let Err(error) = tracking
            .reporter
            .record_download_request_failed(&tracking.item_id)
            .await
    {
        tracing::warn!(
            error = %error,
            "Failed to record failed download request"
        );
    }
    result
}

pub(crate) fn build_download_routes(
    request: &DownloadRequest,
    mode: crate::state::DownloadSourceMode,
) -> Vec<DownloadRoute> {
    let mut urls = Vec::with_capacity(request.candidate_urls.len() + 1);
    urls.push(request.url.clone());
    urls.extend(request.candidate_urls.iter().cloned());
    let mut routes = Vec::new();
    for (index, url) in urls.into_iter().enumerate() {
        let mut candidates =
            resolve_download_routes_for(&url, request.resource, mode);
        if index > 0 {
            for candidate in &mut candidates {
                if !candidate.is_mirror {
                    candidate.source = DownloadRouteSource::Alternate;
                    candidate.allow_sensitive_headers = false;
                }
            }
        }
        for candidate in candidates {
            if !routes.iter().any(|existing: &DownloadRoute| {
                existing.url == candidate.url
                    && existing.proxy == candidate.proxy
            }) {
                routes.push(candidate);
            }
        }
    }
    if request
        .header
        .as_ref()
        .is_some_and(|(name, _)| header_requires_official_only(name))
    {
        routes.retain(|route| !route.is_mirror);
    }
    deduplicate_download_routes(&mut routes);
    if routes.is_empty()
        && let Ok(url) = retired_sources::normalize(&request.url)
    {
        routes.push(official_route(&url, request.resource));
    }
    routes
}

pub(crate) async fn select_h2_download_route(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    part_path: &Path,
) -> Option<(DownloadRoute, crate::util::download::native::NativeH2Policy)> {
    if !request.allow_segmented_download
        || part_resume_expected(part_path).await
        || request.url.starts_with(FETCH_HTTP)
    {
        return None;
    }
    let h2_route = first_h2_route(routes)?;
    // A warm H2 socket must not bypass the large-file connection scheduler.
    // More streams on that socket cannot escape a per-connection speed limit.
    if request
        .integrity
        .size
        .is_some_and(|size| request.prefers_independent_ranges(size))
        && h2_route.supports_range
        && range_splitting_allowed(&h2_route)
    {
        tracing::debug!(
            url = %sanitize_url_for_log(&h2_route.url),
            file_size = request.integrity.size,
            requested_h2_streams = request.h2_range_concurrency,
            "Selecting independent range connections instead of shared HTTP/2"
        );
        return None;
    }
    let policy = if request.h2_range_concurrency.is_some() {
        if !crate::util::download::h2_pool::has_live_connection(&h2_route).await
        {
            return None;
        }
        crate::util::download::native::explicit_h2_policy(&h2_route)
    } else {
        crate::util::download::native::h2_policy(
            &h2_route,
            request.integrity.size,
        )
        .await
    }?;
    Some((h2_route, policy))
}

async fn reuse_existing_download(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    destination: &Path,
    part_path: &Path,
) -> crate::Result<Option<DownloadResult>> {
    if request.integrity.is_empty()
        || !tokio::fs::try_exists(destination)
            .await
            .map_err(|error| IOError::with_path(error, destination))?
    {
        return Ok(None);
    }
    let Ok(size) = verify_file(destination, &request.integrity).await else {
        return Ok(None);
    };
    let route = routes
        .first()
        .cloned()
        .unwrap_or_else(|| official_route(&request.url, request.resource));
    remove_if_exists(part_path).await?;
    remove_if_exists(&crate::util::download::range_journal::path(part_path))
        .await?;
    Ok(Some(DownloadResult {
        path: destination.to_path_buf(),
        url: route.url,
        source: route.source,
        size,
        attempts: 0,
        fallback_count: 0,
        verified_sha1: request.integrity.sha1.clone(),
        verified_sha512: request.integrity.sha512.clone(),
        verified_file: None,
    }))
}

async fn prepare_partial_download(
    routes: &[DownloadRoute],
    part_path: &Path,
    integrity: &Integrity,
) -> crate::Result<()> {
    let dns_hosts =
        routes.iter().filter_map(route_host).collect::<HashSet<_>>();
    let dns_hosts = dns_hosts.iter().map(String::as_str).collect::<Vec<_>>();
    prewarm_download_dns(&dns_hosts).await;
    preserve_or_remove_partial(
        part_path,
        integrity,
        any_route_can_resume(routes),
    )
    .await
}

pub(crate) enum H2AttemptResult {
    Completed(DownloadResult),
    Fallback {
        failed_nonofficial: Option<String>,
        prefer_http1: bool,
        discovered_size: Option<u64>,
    },
}

pub(crate) async fn try_h2_download(
    request: &DownloadRequest,
    route: DownloadRoute,
    policy: crate::util::download::native::NativeH2Policy,
    destination: &Path,
    part_path: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<H2AttemptResult> {
    let started = Instant::now();
    match crate::util::download::h2_download::try_download_via_h2(
        request,
        &route,
        destination,
        part_path,
        policy,
        semaphore,
    )
    .await
    {
        crate::util::download::h2_download::H2DownloadOutcome::Completed(
            result,
        ) => {
            record_route_transfer_success(
                &route,
                request.resource,
                result.size,
                started.elapsed(),
            );
            if request.h2_range_concurrency.is_none()
                && let Some(authority) = original_route_authority(&route)
            {
                crate::util::download::native_reputation::record_transport_success(
                    &authority,
                    route.proxy,
                    if request.h2_range_concurrency.is_some() {
                        crate::util::download::native_reputation::NativeTransport::H2MultiRange
                    } else {
                        crate::util::download::native_reputation::NativeTransport::H2Single
                    },
                    result.size as f64 / started.elapsed().as_secs_f64().max(0.001),
                );
            }
            if let Some(tracking) = &request.install_tracking
                && let Err(error) = tracking
                    .reporter
                    .record_download_request_finished(
                        &tracking.item_id,
                        result.size,
                    )
                    .await
            {
                tracing::warn!(error = %error, "Failed to record completed download request");
            }
            Ok(H2AttemptResult::Completed(result))
        }
        crate::util::download::h2_download::H2DownloadOutcome::Canceled => {
            Err(ErrorKind::OtherError(FETCH_DOWNLOAD_CANCELED.to_string())
                .into())
        }
        crate::util::download::h2_download::H2DownloadOutcome::Fallback {
            failure,
            preserve_partial,
        } => {
            let mut failed_nonofficial = None;
            if failure.integrity_failure() {
                if !is_official_route(&route) {
                    failed_nonofficial = Some(route.url.clone());
                    tracing::warn!(url = %sanitize_url_for_log(&route.url), source = route.source.as_str(), "Mirror hash validation failed; falling back to the official source");
                }
            } else if failure.should_cooldown_authority()
                && let Some(authority) = url_authority(&route.url)
            {
                record_authority_h2_failure_for(&authority, route.proxy);
            }
            if failure.is_transfer_failure() {
                record_native_transfer_failure(&route, None);
                record_route_health_failure(&route, request.resource, None);
            }
            if !preserve_partial {
                remove_if_exists(part_path).await?;
            }
            cleanup_segment_files(part_path, MAX_SEGMENT_CONCURRENCY).await?;
            Ok(H2AttemptResult::Fallback {
                failed_nonofficial,
                prefer_http1: matches!(failure,
                    crate::util::download::h2_download::H2DownloadFailure::Slow
                    | crate::util::download::h2_download::H2DownloadFailure::IndependentRanges(_)),
                discovered_size: match failure {
                    crate::util::download::h2_download::H2DownloadFailure::IndependentRanges(size) => Some(size),
                    _ => None,
                },
            })
        }
    }
}

async fn resume_checkpointed_download(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    destination: &Path,
    part_path: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<Option<DownloadResult>> {
    resume_checkpointed_download_with(
        request,
        routes,
        part_path,
        |route| async move {
            crate::util::download::h2_range::resume_http1(
                request,
                &route,
                destination,
                part_path,
                semaphore,
            )
            .await
        },
    )
    .await
}

pub(crate) async fn resume_checkpointed_download_with<F, Fut>(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    part_path: &Path,
    mut attempt: F,
) -> crate::Result<Option<DownloadResult>>
where
    F: FnMut(DownloadRoute) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<
                Option<crate::util::download::h2_download::H2DownloadOutcome>,
            >,
        >,
{
    let mut had_range_failure = false;
    for route in routes {
        if let Some(outcome) = attempt(route.clone()).await? {
            match outcome {
                crate::util::download::h2_download::H2DownloadOutcome::Completed(
                    result,
                ) => {
                    return Ok(Some(
                        attach_verified_integrity(result, &request.integrity)
                            .await,
                    ));
                }
                crate::util::download::h2_download::H2DownloadOutcome::Canceled => {
                    return Err(ErrorKind::OtherError(
                        FETCH_DOWNLOAD_CANCELED.into(),
                    )
                    .into());
                }
                crate::util::download::h2_download::H2DownloadOutcome::Fallback {
                    failure,
                    preserve_partial,
                } => {
                    if failure
                        == crate::util::download::h2_download::H2DownloadFailure::Io
                    {
                        return Err(ErrorKind::OtherError(
                            FETCH_CHECKPOINT_RESUME_FAILED_DUE_TO_LOCAL_I_O
                                .into(),
                        )
                        .into());
                    }
                    if !preserve_partial {
                        crate::util::download::h2_range::discard(part_path).await?;
                        had_range_failure = false;
                        break;
                    }
                    record_route_health_failure(route, request.resource, None);
                    had_range_failure = true;
                }
            }
        } else {
            break;
        }
    }
    if had_range_failure {
        crate::util::download::h2_range::discard(part_path).await?;
    }
    Ok(None)
}

pub(crate) async fn download_to_path_inner(
    mut request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
    progress: Option<&mut FetchProgressFn<'_>>,
) -> crate::Result<DownloadResult> {
    let _endpoint_registration =
        crate::util::download::proxy_context::clients()
            .and_then(|clients| clients.endpoints.register_download(&request));
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    let download_lock = destination_download_lock(destination);
    let lock_started = Instant::now();
    let lock_wait =
        tokio::time::timeout(RESOURCE_WAIT_TIMEOUT, download_lock.lock());
    let _download_guard = if let Some(cancellation) = request.cancellation.as_ref() {
        tokio::select! {
            _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("download canceled while waiting for destination lock".to_string()).into()),
            result = lock_wait => result,
        }
    } else {
        lock_wait.await
    }
    .map_err(|_| ErrorKind::NetworkError(
        FETCH_TIMED_OUT_WAITING_FOR_DESTINATION_DOWNLOAD_LOCK.to_string(),
    ))?;
    tracing::debug!(
        destination = %destination.display(),
        wait_ms = lock_started.elapsed().as_millis(),
        "Acquired destination download lock"
    );
    let mode = source_mode_for_resource(request.resource);
    let mut routes = build_download_routes(&request, mode);
    let part_path = suffixed_path(destination, PARTIAL_FILE_SUFFIX);

    if let Some(result) =
        reuse_existing_download(&request, &routes, destination, &part_path)
            .await?
    {
        return Ok(attach_verified_integrity(result, &request.integrity).await);
    }
    if let Some(result) = resume_checkpointed_download(
        &request,
        &routes,
        destination,
        &part_path,
        semaphore,
    )
    .await?
    {
        return Ok(result);
    }
    prepare_partial_download(&routes, &part_path, &request.integrity).await?;
    prepare_native_download_routes(&request, &mut routes, semaphore).await;

    // Small files can share H2. Large, hash-verified range downloads enter
    // the independent-connection scheduler even when an H2 socket is warm.
    let h2_failed_nonofficial = if let Some((h2_route, h2_policy)) =
        select_h2_download_route(&request, &routes, &part_path).await
    {
        match try_h2_download(
            &request,
            h2_route,
            h2_policy,
            destination,
            &part_path,
            semaphore,
        )
        .await?
        {
            H2AttemptResult::Completed(result) => {
                return Ok(attach_verified_integrity(
                    result,
                    &request.integrity,
                )
                .await);
            }
            H2AttemptResult::Fallback {
                failed_nonofficial,
                prefer_http1,
                discovered_size,
            } => {
                request.prefer_http1 = prefer_http1;
                if request.integrity.size.is_none() {
                    request.integrity.size = discovered_size;
                }
                failed_nonofficial
            }
        }
    } else {
        None
    };

    if let Some(result) = resume_checkpointed_download(
        &request,
        &routes,
        destination,
        &part_path,
        semaphore,
    )
    .await?
    {
        return Ok(result);
    }
    let request_integrity = request.integrity.clone();
    let result = run_native_download_attempts(
        request,
        destination,
        semaphore,
        progress,
        routes,
        part_path,
        h2_failed_nonofficial,
    )
    .await?;
    Ok(attach_verified_integrity(result, &request_integrity).await)
}
