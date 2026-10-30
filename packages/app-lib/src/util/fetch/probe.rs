use super::client::{
    DIRECT_REQWEST_CLIENT, DownloadClients, HTTP1_DIRECT_REQWEST_CLIENT,
    HTTP1_NO_REDIRECT_REQWEST_CLIENT, NO_REDIRECT_REQWEST_CLIENT,
};
use super::file::parse_content_range;
use super::model::{
    DownloadMeta, DownloadRequest, DownloadRoute, ResourceClass,
};
use super::policy::{
    range_splitting_allowed, record_route_health_failure, record_route_success,
    route_health_key,
};
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::{
    effective_route_authority, order_auto_routes, source_mode_for_resource,
    url_authority,
};
use super::throttle::{
    FetchSemaphore, ROUTE_PROBE_SEMAPHORE, prefer_measured_http1,
};
use crate::util::download::constants::{
    COLD_START_ROUTE_HEALTH_SAMPLE_THRESHOLD, DOWNLOAD_SCHEDULING_GRACE,
    JOB_PROBE_WINDOW, MAX_TASK_PROBE_STATES, MODRINTH_DATA_PREFIX,
    ROUTE_PROBE_BYTES, ROUTE_PROBE_MIN_IMPROVEMENT_PERCENT,
    ROUTE_PROBE_TIMEOUT, ROUTE_PROBE_TIMEOUT_FACTOR,
    SEGMENTED_DOWNLOAD_THRESHOLD, TASK_PROBE_MAX_ROUTES, TASK_PROBE_MAX_WAIT,
    TASK_PROBE_WINDOW,
};
use crate::util::download::modrinth_cdn;
use crate::util::download::native_request::send_path_request_with_clients;
use crate::util::download::route_health::{
    ROUTE_HEALTH, ResourceFamily, TASK_PROBE_STATES, TaskProbeGuard,
    TaskProbeKey,
};
use futures::StreamExt;
use reqwest::StatusCode;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{self, Instant};
use tokio::sync::Notify;
use url::Url;

#[derive(Clone, Debug)]
pub(crate) struct RouteProbeResult {
    pub(crate) route: DownloadRoute,
    pub(crate) bytes_per_second: u64,
    pub(crate) effective_authority: String,
}

pub(crate) type RouteProbeFuture<'a> =
    Pin<Box<dyn Future<Output = Option<RouteProbeResult>> + Send + 'a>>;

pub(crate) fn probe_is_meaningfully_faster(
    candidate_bytes_per_second: u64,
    current_bytes_per_second: u64,
) -> bool {
    u128::from(candidate_bytes_per_second) * 100
        >= u128::from(current_bytes_per_second)
            * u128::from(100 + ROUTE_PROBE_MIN_IMPROVEMENT_PERCENT)
}

pub(crate) fn measured_bytes_per_second(
    bytes: u64,
    elapsed: time::Duration,
) -> u64 {
    let bytes_per_second = u128::from(bytes).saturating_mul(1_000_000_000)
        / elapsed.as_nanos().max(1);
    bytes_per_second.min(u128::from(u64::MAX)) as u64
}

pub(crate) fn expected_route_speed(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> Option<u64> {
    let key = route_health_key(route, resource)?;
    ROUTE_HEALTH
        .lock()
        .get(&key)
        .and_then(|health| health.throughput_bps)
        .or_else(|| {
            crate::util::download::native_reputation::get(
                key.family.as_str(),
                &key.authority,
                route.proxy,
            )
            .and_then(|health| health.throughput_bps)
        })
        .filter(|speed| speed.is_finite() && *speed > 0.0)
        .map(|speed| speed.min(u64::MAX as f64) as u64)
}

#[allow(clippy::too_many_arguments)]
async fn probe_route_throughput(
    route: &DownloadRoute,
    current_effective_authority: Option<&str>,
    total_size: u64,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    _semaphore: &FetchSemaphore,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    resource: ResourceClass,
) -> Option<RouteProbeResult> {
    let known_authority = effective_route_authority(route)?;
    if current_effective_authority == Some(known_authority.as_str()) {
        return None;
    }
    let probe_bytes = ROUTE_PROBE_BYTES.min(total_size);
    let probe_end = probe_bytes.checked_sub(1)?;
    let _permit = ROUTE_PROBE_SEMAPHORE.0.acquire().await.ok()?;
    let started = Instant::now();
    let response = tokio::time::timeout(
        ROUTE_PROBE_TIMEOUT,
        send_path_request_with_clients(
            route,
            custom_header,
            credentials,
            download_meta,
            Some(0),
            Some(probe_end),
            &DownloadClients::for_request(system_client, direct_client),
            None,
        ),
    )
    .await;
    let (mut response, final_url) = match response {
        Ok(Ok(response)) => response,
        Ok(Err(_)) | Err(_) => {
            record_route_health_failure(route, resource, None);
            return None;
        }
    };
    let ttfb = started.elapsed();
    let final_authority = url_authority(&final_url)?;
    if current_effective_authority == Some(final_authority.as_str()) {
        return None;
    }
    let content_range_matches =
        parse_content_range(&response).is_some_and(|range| {
            range.start == 0
                && range.end == probe_end
                && range.total.is_none_or(|total| total == total_size)
        });
    if response.status() != StatusCode::PARTIAL_CONTENT
        || !content_range_matches
    {
        record_route_health_failure(route, resource, None);
        return None;
    }
    let remote_addr = response.remote_addr();
    let transfer_started = Instant::now();
    let mut received = 0_u64;
    while received < probe_bytes {
        let remaining_time =
            ROUTE_PROBE_TIMEOUT.saturating_sub(started.elapsed());
        if remaining_time.is_zero() {
            record_route_health_failure(route, resource, None);
            return None;
        }
        let chunk = match tokio::time::timeout(remaining_time, response.chunk())
            .await
        {
            Ok(Ok(Some(chunk))) => chunk,
            _ => {
                record_route_health_failure(route, resource, None);
                return None;
            }
        };
        received = received.saturating_add(chunk.len() as u64);
    }
    let transfer_elapsed = transfer_started.elapsed();
    let bytes_per_second =
        measured_bytes_per_second(received, started.elapsed());
    record_route_success(
        route,
        resource,
        ttfb,
        received,
        transfer_elapsed,
        remote_addr,
    );
    Some(RouteProbeResult {
        route: route.clone(),
        bytes_per_second,
        effective_authority: final_authority,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn probe_faster_route(
    current_route: &DownloadRoute,
    candidate_routes: &[DownloadRoute],
    current_bytes_per_second: u64,
    total_size: u64,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    semaphore: &FetchSemaphore,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    resource: ResourceClass,
    downloaded_bytes: u64,
) -> Option<RouteProbeResult> {
    let current_authority = effective_route_authority(current_route);
    let mut seen = HashSet::new();
    let mut probes = futures::stream::FuturesUnordered::new();
    for route in candidate_routes {
        if !route.supports_range || !range_splitting_allowed(route) {
            continue;
        }
        let Some(authority) = effective_route_authority(route) else {
            continue;
        };
        if current_authority.as_ref() == Some(&authority)
            || !seen.insert((authority, route.proxy))
        {
            continue;
        }
        probes.push(probe_route_throughput(
            route,
            current_authority.as_deref(),
            total_size,
            custom_header,
            credentials,
            download_meta,
            semaphore,
            system_client,
            direct_client,
            resource,
        ));
    }

    let mut best: Option<RouteProbeResult> = None;
    while let Some(probe) = probes.next().await {
        let Some(probe) = probe else { continue };
        if probe_is_meaningfully_faster(
            probe.bytes_per_second,
            current_bytes_per_second,
        ) && crate::util::download::native_slow::should_switch(
            current_bytes_per_second,
            probe.bytes_per_second,
            total_size.saturating_sub(downloaded_bytes),
            total_size,
        ) && best
            .as_ref()
            .is_none_or(|best| probe.bytes_per_second > best.bytes_per_second)
        {
            best = Some(probe);
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn probe_recovery_route(
    request: &DownloadRequest,
    current: &DownloadRoute,
    candidates: &[DownloadRoute],
    speed: u64,
    total: u64,
    downloaded: u64,
    probe_http1: bool,
    credentials: Option<&crate::state::ModrinthCredentials>,
    semaphore: &FetchSemaphore,
) -> Option<RouteProbeResult> {
    let http1 = async {
        if !probe_http1 || !current.supports_range {
            return None;
        }
        probe_route_throughput(
            current,
            None,
            total,
            request.header.as_ref(),
            credentials,
            request.download_meta.as_ref(),
            semaphore,
            &HTTP1_NO_REDIRECT_REQWEST_CLIENT,
            &HTTP1_DIRECT_REQWEST_CLIENT,
            request.resource,
        )
        .await
    };
    let alternate = probe_faster_route(
        current,
        candidates,
        speed,
        total,
        request.header.as_ref(),
        credentials,
        request.download_meta.as_ref(),
        semaphore,
        &NO_REDIRECT_REQWEST_CLIENT,
        &DIRECT_REQWEST_CLIENT,
        request.resource,
        downloaded,
    );
    let (http1, alternate) = tokio::join!(
        tokio::time::timeout(
            ROUTE_PROBE_TIMEOUT * ROUTE_PROBE_TIMEOUT_FACTOR,
            http1
        ),
        tokio::time::timeout(
            ROUTE_PROBE_TIMEOUT * ROUTE_PROBE_TIMEOUT_FACTOR,
            alternate
        ),
    );
    let http1 = http1.ok().flatten();
    let alternate = alternate.ok().flatten();
    let remaining = total.saturating_sub(downloaded);
    let restart = if request.integrity.supports_resume() {
        remaining
    } else {
        total
    };
    let http1 = http1.filter(|probe| {
        crate::util::download::native_slow::should_switch(
            speed,
            probe.bytes_per_second,
            remaining,
            restart,
        )
    });
    let selected = match (http1, alternate) {
        (Some(http1), Some(alternate)) => {
            if restart as f64 / http1.bytes_per_second.max(1) as f64
                <= total as f64 / alternate.bytes_per_second.max(1) as f64
            {
                Some(http1)
            } else {
                Some(alternate)
            }
        }
        (http1, alternate) => http1.or(alternate),
    };
    if let Some(probe) =
        selected.as_ref().filter(|probe| probe.route == *current)
    {
        prefer_measured_http1(&probe.effective_authority, current.proxy);
        if let Some(authority) = url_authority(&current.url)
            && authority != probe.effective_authority
        {
            prefer_measured_http1(&authority, current.proxy);
        }
    }
    selected
}

fn route_health_is_cold(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> bool {
    route_health_key(route, resource).is_none_or(|key| {
        let in_memory_samples = ROUTE_HEALTH
            .lock()
            .get(&key)
            .map(|entry| entry.success_samples)
            .unwrap_or(0);
        let persisted_samples = crate::util::download::native_reputation::get(
            key.family.as_str(),
            &key.authority,
            route.proxy,
        )
        .map(|entry| entry.success_samples)
        .unwrap_or(0);
        in_memory_samples.max(persisted_samples)
            < COLD_START_ROUTE_HEALTH_SAMPLE_THRESHOLD
    })
}

/// Schedules one shared throughput probe while real downloads proceed.
enum TaskProbeDecision {
    Run(Arc<Notify>),
    Done,
}

struct TaskProbePlan {
    family: ResourceFamily,
    size: u64,
    candidates: Vec<DownloadRoute>,
    task_key: TaskProbeKey,
    state: Arc<crate::util::download::route_health::TaskProbeState>,
}

fn build_task_probe_plan(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    _semaphore: &FetchSemaphore,
) -> Option<TaskProbePlan> {
    if !matches!(
        source_mode_for_resource(request.resource),
        crate::state::DownloadSourceMode::Auto
    ) {
        return None;
    }
    let size = request
        .integrity
        .size
        .filter(|size| *size >= SEGMENTED_DOWNLOAD_THRESHOLD)?;
    let first_route = routes.first()?;
    let family = route_health_key(first_route, request.resource)?.family;
    if !route_health_is_cold(first_route, request.resource) {
        return None;
    }
    let mut candidate_keys = HashSet::new();
    let candidates = routes
        .iter()
        .filter(|route| route.supports_range && range_splitting_allowed(route))
        .filter(|route| {
            effective_route_authority(route).is_none_or(|authority| {
                candidate_keys.insert((authority, route.proxy))
            })
        })
        .take(TASK_PROBE_MAX_ROUTES)
        .cloned()
        .collect::<Vec<_>>();
    if candidates.len() < 2 {
        return None;
    }
    let mut probe_authorities = candidates
        .iter()
        .filter_map(|route| {
            effective_route_authority(route).map(|authority| {
                crate::util::download::proxy_context::authority_key(
                    &authority,
                    route.proxy,
                )
            })
        })
        .collect::<Vec<_>>();
    let scope = crate::util::download::route_health::probe_scope(
        family.as_str(),
        &mut probe_authorities,
    );
    let task_key = request
        .install_tracking
        .as_ref()
        .map(|tracking| TaskProbeKey::Job(tracking.reporter.job_id(), scope))
        .unwrap_or(TaskProbeKey::Anonymous(scope));
    let state = {
        let mut tasks = TASK_PROBE_STATES.lock();
        if tasks.len() >= MAX_TASK_PROBE_STATES {
            tasks.retain(|_, state| state.has_in_flight());
        }
        tasks.entry(task_key).or_default().clone()
    };
    Some(TaskProbePlan {
        family,
        size,
        candidates,
        task_key,
        state,
    })
}

fn claim_task_probe(plan: &TaskProbePlan) -> TaskProbeDecision {
    let mut families = plan.state.families.lock();
    let entry = families.entry(plan.family).or_default();
    let recently_probed = entry.last_probed.is_some_and(|probed| {
        probed.elapsed()
            < if matches!(plan.task_key, TaskProbeKey::Job(_, _)) {
                JOB_PROBE_WINDOW
            } else {
                TASK_PROBE_WINDOW
            }
    });
    if recently_probed {
        TaskProbeDecision::Done
    } else if entry.in_flight.is_some() {
        TaskProbeDecision::Done
    } else {
        let notify = Arc::new(Notify::new());
        entry.in_flight = Some(notify.clone());
        TaskProbeDecision::Run(notify)
    }
}

async fn run_task_probe(
    request: &DownloadRequest,
    semaphore: &FetchSemaphore,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    plan: &TaskProbePlan,
    notify: Arc<Notify>,
) {
    let probe = async {
        let mut guard = TaskProbeGuard {
            state: plan.state.clone(),
            family: plan.family,
            notify: notify.clone(),
            armed: true,
        };
        use crate::util::download::endpoint_selection::tuning::{
            TRIALS, TrialKind,
        };
        let Some(_trial) = TRIALS.acquire(
            TrialKind::Endpoint,
            Instant::now(),
            TASK_PROBE_MAX_WAIT + DOWNLOAD_SCHEDULING_GRACE,
        ) else {
            return false;
        };
        let mut probes = futures::stream::FuturesUnordered::new();
        for route in &plan.candidates {
            probes.push(probe_route_throughput(
                route,
                None,
                plan.size,
                request.header.as_ref(),
                None,
                request.download_meta.as_ref(),
                semaphore,
                system_client,
                direct_client,
                request.resource,
            ));
        }
        while probes.next().await.is_some() {}
        guard.disarm();
        true
    };
    let completed = tokio::time::timeout(TASK_PROBE_MAX_WAIT, probe)
        .await
        .unwrap_or(false);
    {
        let mut families = plan.state.families.lock();
        if let Some(entry) = families.get_mut(&plan.family) {
            if completed {
                entry.last_probed = Some(Instant::now());
            }
            entry.in_flight = None;
        }
    }
    notify.notify_waiters();
    tracing::debug!(
        family = ?plan.family,
        completed,
        "Task download route probe finished"
    );
}

async fn ensure_task_routes_probed(
    request: &DownloadRequest,
    routes: &mut Vec<DownloadRoute>,
    semaphore: &FetchSemaphore,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
) {
    if crate::util::download::proxy_context::clients().is_some_and(|clients| {
        clients.endpoints.mode() == crate::util::download::endpoint_selection::SelectionMode::Auto
            && request.integrity.supports_resume() && request.header.is_none()
            && Url::parse(&request.url).is_ok_and(|url| modrinth_cdn::is_current_url(&url) && url.path().starts_with(MODRINTH_DATA_PREFIX) && url.query().is_none())
            && !routes.is_empty() && routes.iter().all(|route| !route.is_mirror
                && Url::parse(&route.url).is_ok_and(|url| modrinth_cdn::is_current_url(&url))
                && crate::util::download::endpoint_selection::EndpointSelector::eligible_network(&clients, route.proxy))
    }) {
        // One official authority has no alternate URL to rank. Its endpoint
        // selector owns any needed throughput trial and shared body budget.
        return;
    }
    let Some(plan) = build_task_probe_plan(request, routes, semaphore) else {
        return;
    };
    match claim_task_probe(&plan) {
        TaskProbeDecision::Done => {}
        TaskProbeDecision::Run(notify) => {
            let request = request.clone();
            let system_client = system_client.clone();
            let direct_client = direct_client.clone();
            let clients =
                DownloadClients::for_request(&system_client, &direct_client);
            tokio::spawn(async move {
                crate::util::download::proxy_context::with_snapshot(
                    clients,
                    run_task_probe(
                        &request,
                        &ROUTE_PROBE_SEMAPHORE,
                        &system_client,
                        &direct_client,
                        &plan,
                        notify,
                    ),
                )
                .await;
            });
        }
    }
    order_auto_routes(routes, request.resource);
}

pub(crate) async fn prepare_native_download_routes(
    request: &DownloadRequest,
    routes: &mut Vec<DownloadRoute>,
    semaphore: &FetchSemaphore,
) {
    crate::util::download::native_reputation::load_if_needed().await;
    ensure_task_routes_probed(
        request,
        routes,
        semaphore,
        &NO_REDIRECT_REQWEST_CLIENT,
        &DIRECT_REQWEST_CLIENT,
    )
    .await;
    if source_mode_for_resource(request.resource)
        == crate::state::DownloadSourceMode::Auto
    {
        order_auto_routes(routes, request.resource);
    }
}
