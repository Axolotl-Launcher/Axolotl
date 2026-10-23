//! Functions for fetching information from the Internet
pub use super::download::constants::DOWNLOAD_META_HEADER;
#[cfg(test)]
use super::download::constants::MAX_REDIRECT_LOCATION_BYTES;
use super::download::modrinth_cdn;
use super::download::modrinth_redirect::is_official_redirect as is_official_modrinth_cdn_redirect;
#[cfg(test)]
use super::download::modrinth_redirect::repair_official_redirect as repair_official_cdn_redirect;
use super::download::retired_sources;
use super::download::route_policy;
use super::download_dns::DownloadDnsResolver;
use super::download_manager::{DownloadSpeedTracker, SpeedSnapshot};
use super::io::{self, IOError};
use crate::event::LoadingBarId;
use crate::event::emit::emit_loading;
use crate::install::{DownloadItemStatus, InstallProgressReporter};
use crate::util::download::constants::DOWNLOAD_PROGRESS_LOG_INTERVAL;
use crate::util::download::constants::FETCH_RETRY_DELAYS;
use crate::util::download::constants::MODRINTH_CDN_ATTEMPT_TIMEOUT;
use crate::util::download::constants::MODRINTH_CDN_ATTEMPTS;
use crate::util::download::constants::RANGE_SPLITTING_DISABLE_THRESHOLD;
use crate::util::download::constants::STALE_PARTIAL_DOWNLOAD_MAX_AGE;
use crate::util::download::constants::{
    ADAPTIVE_RANGE_BYTES, ALIYUN_MAVEN_BASE_URL, API_KEY_HEADER,
    AUTHORIZATION_HEADER, BMCLAPI_BASE_URL,
    COLD_START_ROUTE_HEALTH_SAMPLE_THRESHOLD, CURSEFORGE_API_HOST,
    CURSEFORGE_EDGE_HOST, DIAGNOSTICS_NETWORK, DIAGNOSTICS_RANGE,
    DOWNLOAD_BUFFER_BYTES, DOWNLOAD_FAIR_SHARE_DIVISOR,
    DOWNLOAD_PROGRESS_INTERVAL, DOWNLOAD_PROGRESS_MIN_BYTES,
    DOWNLOAD_SCHEDULER_INTERVAL, DOWNLOAD_SCHEDULING_GRACE, FETCH_ABORT,
    FETCH_ACTIVE_SEGMENT_BODY, FETCH_ALIYUN, FETCH_ALTERNATE, FETCH_API,
    FETCH_AUTHORIZATION, FETCH_AXOLOTL_RETRY, FETCH_BMCLAPI,
    FETCH_BMCLAPI2_BANGBANG93_COM, FETCH_BYTES, FETCH_CDN,
    FETCH_CHECKPOINT_RESUME_FAILED_DUE_TO_LOCAL_I_O, FETCH_CLEAN_RETRY,
    FETCH_CLEAR_PARTIAL_AND_RETRY, FETCH_CLEAR_PARTIAL_AND_SWITCH,
    FETCH_CLIENT_CONFIGURATION_SHOULD_BE_VALID,
    FETCH_CONFIGURED_CONNECTION_LIMIT_IS_ONE, FETCH_COOKIE,
    FETCH_COOLDOWN_AND_SWITCH, FETCH_COOLDOWN_THEN_RETRY,
    FETCH_CREATING_DOWNLOAD_FILE, FETCH_DISABLE_RANGE_AND_RETRY_SINGLE,
    FETCH_DISABLE_RANGE_AND_SWITCH, FETCH_DOWNLOAD_CANCELED, FETCH_DROP_ROUTE,
    FETCH_DROP_ROUTE_AFTER_CLEAN_RETRY,
    FETCH_EFFECTIVE_CONCURRENCY_CAP_REACHED, FETCH_EO_CACHE_STATUS,
    FETCH_EXPANSION_COOLDOWN_ACTIVE, FETCH_FALLBACK_OFFICIAL,
    FETCH_FALLBACK_SINGLE, FETCH_FETCH_PERMIT_BATCH_HAS_ENOUGH_PERMITS,
    FETCH_FILES_MINECRAFTFORGE_NET, FETCH_FINALIZING_DOWNLOAD,
    FETCH_GITHUB_COM, FETCH_GOAWAY, FETCH_HEDGE_FAILED, FETCH_HTTP,
    FETCH_HTTP_2, FETCH_HTTP2, FETCH_ICONS,
    FETCH_INSUFFICIENT_AGGREGATE_SPEED_SAMPLES, FETCH_INVALID_CONTENT_RANGE,
    FETCH_INVALID_HEDGED_CONTENT_RANGE, FETCH_JAVA_RUNTIME,
    FETCH_LAUNCHER_MOJANG_COM, FETCH_LIBRARIES_MINECRAFT_NET, FETCH_MAVEN,
    FETCH_MAVEN_ALIYUN_COM, FETCH_MAVEN2, FETCH_MCIM,
    FETCH_MEDIAFILEZ_FORGECDN_NET, FETCH_MELOONG_COM, FETCH_META_FABRICMC_NET,
    FETCH_MIRROR, FETCH_MISSING, FETCH_MOD_MCIMIRROR_TOP, FETCH_MOMOT_RS,
    FETCH_MRPACK, FETCH_NCAUSED_BY, FETCH_NO_GLOBAL_PERMIT_AVAILABLE,
    FETCH_OFFICIAL, FETCH_OPENING_DOWNLOAD_FILE, FETCH_OPTIFINE_NET,
    FETCH_PART_RANGES_JSON, FETCH_PART_RANGES_JSON_2,
    FETCH_PISTON_DATA_MOJANG_COM, FETCH_PROTOCOL_ERROR,
    FETCH_PROXIED_CLIENT_CONFIGURATION_SHOULD_BE_VALID,
    FETCH_PROXY_AUTHORIZATION, FETCH_PROXY_CONFIGURATION_SHOULD_BE_VALID,
    FETCH_RANGE_BYTE_COUNT_MISMATCH, FETCH_RANGE_END_OVERFLOW,
    FETCH_RANGE_RESPONSE_ENDED_BEFORE_EXPECTED_BOUNDARY, FETCH_REFUSED_STREAM,
    FETCH_RELEASES, FETCH_REMOVING, FETCH_REPO_MAVEN_APACHE_ORG,
    FETCH_REPO1_MAVEN_ORG, FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES,
    FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE,
    FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET,
    FETCH_RESUMABLE_PARTIAL_USES_A_SINGLE_STREAM, FETCH_RESUME_OR_SWITCH,
    FETCH_RETRY_JITTER_MAX, FETCH_RETRY_JITTER_MIN, FETCH_RETRY_NEXT_ROUND,
    FETCH_RETRY_SAME_ROUTE, FETCH_SEGMENT,
    FETCH_SEGMENTED_CONTENT_VALIDATION_FAILED, FETCH_SELECTED_ENDPOINT_CHANGED,
    FETCH_SERVER_IGNORED_RANGE_AND_RETURNED_200,
    FETCH_SERVER_RETURNED_HTTP_416_FOR_RANGE_REQUEST, FETCH_STOP,
    FETCH_STREAM_ERROR, FETCH_SWITCH_ROUTE, FETCH_SWITCH_ROUTE_OR_RETRY_ROUND,
    FETCH_TIMED_OUT_WAITING_FOR_CDN_CONNECTION_RESOURCES,
    FETCH_TIMED_OUT_WAITING_FOR_DESTINATION_DOWNLOAD_LOCK,
    FETCH_TIMED_OUT_WAITING_FOR_NATIVE_DOWNLOAD_RESOURCES,
    FETCH_TOO_LITTLE_DATA_REMAINS, FETCH_TOO_MANY_REDIRECTS,
    FETCH_UNCONFIGURED, FETCH_UNKNOWN, FETCH_V2_SESSION, FETCH_V2_USER,
    FETCH_V3_FRIEND, FETCH_V3_FRIENDS, FETCH_V3_NOTIFICATION,
    FILE_H2_KEEPALIVE, FILE_TCP_KEEPALIVE, FILE_TRANSFER_CONNECT_TIMEOUT,
    FILE_TRANSFER_FIRST_BYTE_TIMEOUT, FILE_TRANSFER_READ_TIMEOUT,
    H2_FALLBACK_TTL, H2_SLOW_FALLBACK_TTL, HTTPS_SCHEME,
    INITIAL_SEGMENT_CONCURRENCY, JOB_PROBE_WINDOW, MAX_FAILURE_COOLDOWN,
    MAX_GLOBAL_TAIL_HEDGES, MAX_SEGMENT_CONCURRENCY, MAX_TAIL_HEDGES_PER_FILE,
    MAX_TASK_PROBE_STATES, MCIM_BASE_URL, METADATA_ATTEMPT_BUDGET,
    METADATA_HEDGE_DELAY, METADATA_REQUEST_TIMEOUT, MIN_SEGMENT_SIZE,
    MODRINTH_API_HOST, MODRINTH_DATA_PREFIX,
    NATIVE_DOWNLOAD_SOURCE_HAS_BEEN_RETIRED, PARTIAL_FILE_SUFFIX,
    RANGE_IDLE_RECONNECT_TIMEOUT, REASSIGNABLE_FIRST_BYTE_TIMEOUT,
    RESOURCE_WAIT_TIMEOUT, RETIRED_SOURCES_MEDIA_FORGECDN_NET, RETRY_AFTER_CAP,
    ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM, ROUTE_POLICY_MAVEN_FABRICMC_NET,
    ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET, ROUTE_POLICY_MAVEN_NEOFORGED_NET,
    ROUTE_POLICY_PISTON_META_MOJANG_COM, ROUTE_PROBE_BYTES,
    ROUTE_PROBE_MIN_IMPROVEMENT_PERCENT, ROUTE_PROBE_TIMEOUT,
    SEGMENT_EXPANSION_INTERVAL, SEGMENT_EXPANSION_SAMPLE_COUNT,
    SEGMENT_RETRY_ATTEMPTS, SEGMENTED_DOWNLOAD_THRESHOLD,
    TAIL_HEDGE_IDLE_TIMEOUT, TAIL_HEDGE_MIN_REMAINING, TASK_PROBE_MAX_ROUTES,
    TASK_PROBE_MAX_WAIT, TASK_PROBE_WINDOW,
};
use crate::util::download::constants::{
    DOWNLOAD_DNS_PREWARM_CONCURRENCY, DOWNLOAD_FAILURE_DIAGNOSTIC_ATTEMPTS,
    DOWNLOAD_PERCENT_BASE, FILE_POOL_MAX_IDLE_PER_HOST,
    FILE_RETRY_SINGLE_STREAM_ROUNDS, FILE_VALIDATION_CONCURRENCY,
    MIN_PARALLEL_RANGES, RANGE_FULL_FAILURE_LIMIT, RANGE_REDUCED_CONCURRENCY,
    RANGE_REDUCED_FAILURE_LIMIT, ROUTE_PROBE_CONCURRENCY,
    ROUTE_PROBE_TIMEOUT_FACTOR, TAIL_SPLIT_PERCENT,
};
#[cfg(test)]
use crate::util::download::constants::{
    FETCH_127_0_0_1, FETCH_LOCALHOST, HTTP_SCHEME,
};
use crate::{ErrorKind, LabrinthError};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use eyre::{Context, eyre};
use futures::StreamExt;
use parking_lot::Mutex;
use rand::Rng;
use reqwest::{Method, StatusCode, header};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::ffi::OsStr;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{
    Arc, LazyLock, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{self, Instant};
use tokio::sync::{Mutex as AsyncMutex, Notify, Semaphore, SemaphorePermit};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use url::Url;

#[cfg(test)]
fn is_safe_redirect_location(location: &str) -> bool {
    location.len() <= MAX_REDIRECT_LOCATION_BYTES && location.is_ascii()
}

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    Metadata,
    MinecraftAsset,
    MinecraftLibrary,
    Loader,
    Java,
    Modrinth,
    CurseForge,
    Modpack,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadRouteSource {
    Official,
    Bmclapi,
    Mcim,
    Aliyun,
    Alternate,
}

impl DownloadRouteSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Official => FETCH_OFFICIAL,
            Self::Bmclapi => FETCH_BMCLAPI,
            Self::Mcim => FETCH_MCIM,
            Self::Aliyun => FETCH_ALIYUN,
            Self::Alternate => FETCH_ALTERNATE,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProxyPolicy {
    #[default]
    System,
    Direct,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadRoute {
    pub url: String,
    pub source: DownloadRouteSource,
    pub is_mirror: bool,
    pub allow_sensitive_headers: bool,
    pub supports_range: bool,
    pub proxy: ProxyPolicy,
}

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentValidation {
    #[default]
    None,
    Json,
    Jar,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Integrity {
    pub size: Option<u64>,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub sha256: Option<String>,
    pub md5: Option<String>,
    pub content: ContentValidation,
}

impl Integrity {
    pub fn sha1(hash: impl Into<String>) -> Self {
        Self {
            sha1: Some(hash.into()),
            ..Self::default()
        }
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_content_validation(
        mut self,
        content: ContentValidation,
    ) -> Self {
        self.content = content;
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.size.is_none()
            && self.sha1.is_none()
            && self.sha512.is_none()
            && self.sha256.is_none()
            && self.md5.is_none()
            && self.content == ContentValidation::None
    }

    /// Resuming a partial download is only safe when a content hash can
    /// prove the stitched-together file is what the server intended.
    pub(crate) fn supports_resume(&self) -> bool {
        self.size.is_some() && self.has_hash()
    }

    pub(crate) fn has_hash(&self) -> bool {
        self.sha1.is_some()
            || self.sha512.is_some()
            || self.sha256.is_some()
            || self.md5.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct DownloadRequest {
    pub url: String,
    pub resource: ResourceClass,
    pub integrity: Integrity,
    pub download_meta: Option<DownloadMeta>,
    pub header: Option<(String, String)>,
    pub candidate_urls: Vec<String>,
    /// Whether range-segmented (multi-connection) downloading is allowed.
    /// Batch schedulers disable it so many small files share one connection
    /// budget instead of each file multiplying its connections.
    pub allow_segmented_download: bool,
    /// Whether HTTP/1.1 may multiply one file into concurrent Range requests.
    /// Multi-file batch schedulers disable this while retaining shared H2.
    pub(crate) allow_http1_segmented_download: bool,
    /// Explicit shared-connection H2 range stream count for one large file.
    /// This is currently reserved for standalone modpack archive downloads.
    pub(crate) h2_range_concurrency: Option<usize>,
    pub(crate) cancellation: Option<tokio_util::sync::CancellationToken>,
    pub(crate) install_tracking: Option<DownloadInstallTracking>,
    pub(crate) endpoint_migrations: Arc<std::sync::atomic::AtomicU8>,
    pub(crate) endpoint_migration_generations: Arc<Mutex<HashSet<(u64, u64)>>>,
    pub(crate) transfer_counters:
        Arc<super::download::endpoint_selection::metrics::TransferCounters>,
}

#[derive(Clone, Debug)]
pub(crate) struct DownloadInstallTracking {
    pub(crate) reporter: InstallProgressReporter,
    pub(crate) item_id: String,
    pub(crate) item_name: String,
}

impl DownloadRequest {
    pub fn new(url: impl Into<String>, resource: ResourceClass) -> Self {
        Self {
            url: url.into(),
            resource,
            integrity: Integrity::default(),
            download_meta: None,
            header: None,
            candidate_urls: Vec::new(),
            allow_segmented_download: true,
            allow_http1_segmented_download: true,
            h2_range_concurrency: None,
            cancellation: None,
            install_tracking: None,
            endpoint_migrations: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            endpoint_migration_generations: Arc::default(),
            transfer_counters: Arc::default(),
        }
    }

    pub fn with_segmented_download(mut self, allow: bool) -> Self {
        self.allow_segmented_download = allow;
        self.allow_http1_segmented_download = allow;
        self
    }

    pub(crate) fn with_http1_segmented_download(mut self, allow: bool) -> Self {
        self.allow_http1_segmented_download = allow;
        self
    }

    pub(crate) fn with_h2_range_concurrency(
        mut self,
        concurrency: usize,
    ) -> Self {
        self.h2_range_concurrency = Some(concurrency.max(1));
        self
    }

    pub fn with_integrity(mut self, integrity: Integrity) -> Self {
        self.integrity = integrity;
        self
    }

    pub fn with_download_meta(mut self, download_meta: DownloadMeta) -> Self {
        self.download_meta = Some(download_meta);
        self
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.header = Some((name.into(), value.into()));
        self
    }

    pub fn with_candidate_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.candidate_urls.extend(urls.into_iter().map(Into::into));
        self
    }

    pub fn with_install_tracking(
        mut self,
        reporter: InstallProgressReporter,
        item_id: impl Into<String>,
        item_name: impl Into<String>,
    ) -> Self {
        self.cancellation = Some(reporter.cancellation_token());
        self.install_tracking = Some(DownloadInstallTracking {
            reporter,
            item_id: item_id.into(),
            item_name: item_name.into(),
        });
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadResult {
    pub path: PathBuf,
    pub url: String,
    pub source: DownloadRouteSource,
    pub size: u64,
    pub attempts: usize,
    pub fallback_count: usize,
    #[serde(default)]
    pub verified_sha1: Option<String>,
    #[serde(default)]
    pub verified_sha512: Option<String>,
    #[serde(skip)]
    pub(crate) verified_file:
        Option<super::download::verified_file::VerifiedFile>,
}

async fn attach_verified_integrity(
    mut result: DownloadResult,
    integrity: &Integrity,
) -> DownloadResult {
    result.verified_sha1 = integrity.sha1.clone();
    result.verified_sha512 = integrity.sha512.clone();
    result.verified_file = None;
    result
}

static IN_FLIGHT_DOWNLOADS: LazyLock<
    dashmap::DashMap<String, Weak<AsyncMutex<()>>>,
> = LazyLock::new(dashmap::DashMap::new);

use super::download::diagnostics::{
    DownloadAttemptDiagnostic, attach_download_attempt_history,
    push_download_attempt_diagnostic, record_download_attempt_failure,
};
pub(crate) use super::download::integrity::{
    compute_file_integrity, is_integrity_error, validate_file_content,
    verify_computed_integrity, verify_file,
};
#[cfg(test)]
use super::download::native_request::byte_range_header_value;
use super::download::native_request::{
    send_path_request, send_path_request_with_clients,
};
#[cfg(test)]
use super::download::route_health::resource_family;
use super::download::route_health::{
    ROUTE_HEALTH, ResourceFamily, RouteHealth, RouteHealthKey,
    TASK_PROBE_STATES, TaskProbeGuard, TaskProbeKey,
};

pub(crate) fn url_authority(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some(format!(
        "{host}:{}",
        url.port_or_known_default().unwrap_or(0)
    ))
}

fn original_route_authority(route: &DownloadRoute) -> Option<String> {
    url_authority(&route.url)
}

fn effective_route_authority(route: &DownloadRoute) -> Option<String> {
    super::download::route_health::effective_route_authority(route)
}

pub(crate) fn remember_effective_route_authority(
    route: &DownloadRoute,
    final_url: &str,
) {
    super::download::route_health::remember_effective_route_authority(
        route, final_url,
    )
}

pub(crate) fn forget_effective_route_authority(
    route: &DownloadRoute,
    failed_url: &Url,
) {
    super::download::route_health::forget_effective_route_authority(
        route, failed_url,
    )
}

fn deduplicate_download_routes(routes: &mut Vec<DownloadRoute>) {
    let mut seen = HashSet::new();
    routes.retain(|route| seen.insert((route.url.clone(), route.proxy)));
}

fn first_h2_route(routes: &[DownloadRoute]) -> Option<DownloadRoute> {
    routes
        .iter()
        .find(|route| !crate::util::download::native_breaker::is_open(route))
        .or_else(|| routes.first())
        .cloned()
}

fn routes_share_effective_authority(
    left: &DownloadRoute,
    right: &DownloadRoute,
) -> bool {
    left.proxy == right.proxy
        && effective_route_authority(left).is_some_and(|authority| {
            effective_route_authority(right).as_ref() == Some(&authority)
        })
}

fn route_health_key(
    route: &DownloadRoute,
    resource: ResourceClass,
) -> Option<RouteHealthKey> {
    super::download::route_health::route_health_key(route, resource)
}

fn update_ewma(current: &mut Option<f64>, sample: f64) {
    super::download::route_health::update_ewma(current, sample);
}

fn persisted_route_health(
    key: &RouteHealthKey,
    proxy: ProxyPolicy,
) -> RouteHealth {
    super::download::route_health::persisted_route_health(key, proxy)
}

fn modrinth_request_kind(url: &str) -> Option<&'static str> {
    if url.starts_with(env!("MODRINTH_API_URL"))
        || url.starts_with(env!("MODRINTH_API_URL_V3"))
    {
        Some(FETCH_API)
    } else if Url::parse(url)
        .is_ok_and(|url| modrinth_cdn::is_current_url(&url))
    {
        Some(FETCH_CDN)
    } else {
        None
    }
}

fn is_modrinth_api_url(url: &str) -> bool {
    Url::parse(url).ok().is_some_and(|parsed| {
        matches!(parsed.host_str(), Some(MODRINTH_API_HOST))
    })
}

/// Appends a cache-busting query parameter so a retry bypasses a corrupt
/// edge-cache object (a truncated or checksum-mismatched body served for an
/// otherwise valid URL). Existing query pairs are preserved.
fn cache_busted_download_url(url: &str, attempt: usize) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    let original_pairs = parsed
        .query_pairs()
        .filter(|(key, _)| key != FETCH_AXOLOTL_RETRY)
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    parsed.set_query(None);
    {
        let mut pairs = parsed.query_pairs_mut();
        for (key, value) in original_pairs {
            pairs.append_pair(&key, &value);
        }
        pairs.append_pair(FETCH_AXOLOTL_RETRY, &attempt.to_string());
    }
    parsed.into()
}

pub(crate) fn sanitize_url_for_log(url: &str) -> String {
    if let Ok(mut url) = Url::parse(url) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.into();
    }

    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

fn is_mrpack_url(url: &str) -> bool {
    reqwest::Url::parse(url).ok().is_some_and(|url| {
        url.path().to_ascii_lowercase().ends_with(FETCH_MRPACK)
    })
}

fn route(
    url: String,
    source: DownloadRouteSource,
    is_mirror: bool,
    supports_range: bool,
) -> DownloadRoute {
    DownloadRoute {
        url,
        source,
        is_mirror,
        allow_sensitive_headers: !is_mirror,
        supports_range,
        proxy: ProxyPolicy::System,
    }
}

fn official_route(url: &str, resource: ResourceClass) -> DownloadRoute {
    let url = modrinth_cdn::normalize(url).into_owned();
    let parsed_url = Url::parse(&url).ok();
    let source = parsed_url
        .as_ref()
        .and_then(|url| url.host_str().map(str::to_string))
        .map_or(DownloadRouteSource::Official, |host| match host.as_str() {
            FETCH_BMCLAPI2_BANGBANG93_COM => DownloadRouteSource::Bmclapi,
            FETCH_MOD_MCIMIRROR_TOP => DownloadRouteSource::Mcim,
            FETCH_MAVEN_ALIYUN_COM => DownloadRouteSource::Aliyun,
            _ => DownloadRouteSource::Official,
        });
    let is_mirror = matches!(
        source,
        DownloadRouteSource::Bmclapi
            | DownloadRouteSource::Mcim
            | DownloadRouteSource::Aliyun
    );
    let route = route(
        url,
        source,
        is_mirror,
        !matches!(resource, ResourceClass::Metadata),
    );
    #[cfg(test)]
    let route = {
        let mut route = route;
        if parsed_url
            .as_ref()
            .and_then(|url| url.host_str().and_then(|host| host.parse().ok()))
            .is_some_and(|address: std::net::IpAddr| address.is_loopback())
        {
            route.proxy = ProxyPolicy::Direct;
        }
        route
    };
    route
}

fn is_official_route(route: &DownloadRoute) -> bool {
    !route.is_mirror && route.source == DownloadRouteSource::Official
}

fn official_fallback_route(routes: &[DownloadRoute]) -> Option<DownloadRoute> {
    routes
        .iter()
        .find(|candidate| is_official_route(candidate))
        .cloned()
}

fn url_with_base(original: &Url, base: &str, path: &str) -> Option<String> {
    let mut target = Url::parse(base).ok()?;
    target.set_path(path);
    target.set_query(original.query());
    target.set_fragment(None);
    Some(target.into())
}

fn explicit_mirror_routes(
    url: &str,
    resource: ResourceClass,
) -> Vec<DownloadRoute> {
    let Ok(parsed) = Url::parse(url) else {
        return Vec::new();
    };
    if parsed.scheme() != HTTPS_SCHEME {
        return Vec::new();
    }

    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path();
    let supports_range = !matches!(resource, ResourceClass::Metadata);
    let mut routes = Vec::new();
    let push_mirror = |routes: &mut Vec<DownloadRoute>,
                       base: &str,
                       path: String,
                       source: DownloadRouteSource| {
        // Disabled MCIM.
        if source == DownloadRouteSource::Mcim {
            return;
        }
        if let Some(url) = url_with_base(&parsed, base, &path) {
            routes.push(route(url, source, true, supports_range));
        }
    };

    match host {
        FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/assets{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_LIBRARIES_MINECRAFT_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven{path}"),
                DownloadRouteSource::Bmclapi,
            );
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/libraries{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        // Maven Central hosts the bulk of loader-era libraries (Cleanroom,
        // LiteLoader, Babric, legacy Forge processors). Aliyun's public
        // repository mirrors it 1:1 and is reachable without a proxy, which
        // keeps a flaky proxy from stalling a whole install on one authority.
        FETCH_REPO_MAVEN_APACHE_ORG | FETCH_REPO1_MAVEN_ORG
            if path.starts_with(FETCH_MAVEN2) =>
        {
            push_mirror(
                &mut routes,
                ALIYUN_MAVEN_BASE_URL,
                format!(
                    "/repository/public{}",
                    path.trim_start_matches("/maven2")
                ),
                DownloadRouteSource::Aliyun,
            );
        }
        ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET
        | ROUTE_POLICY_MAVEN_FABRICMC_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_FILES_MINECRAFTFORGE_NET if path.starts_with(FETCH_MAVEN) => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                path.to_string(),
                DownloadRouteSource::Bmclapi,
            );
        }
        ROUTE_POLICY_MAVEN_NEOFORGED_NET
            if path.starts_with(FETCH_RELEASES) =>
        {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/maven/{}", path.trim_start_matches("/releases/")),
                DownloadRouteSource::Bmclapi,
            );
        }
        FETCH_META_FABRICMC_NET => {
            push_mirror(
                &mut routes,
                BMCLAPI_BASE_URL,
                format!("/fabric-meta{path}"),
                DownloadRouteSource::Bmclapi,
            );
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
        | ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM
        | FETCH_LAUNCHER_MOJANG_COM
        | FETCH_PISTON_DATA_MOJANG_COM => push_mirror(
            &mut routes,
            BMCLAPI_BASE_URL,
            path.to_string(),
            DownloadRouteSource::Bmclapi,
        ),
        CURSEFORGE_API_HOST => push_mirror(
            &mut routes,
            MCIM_BASE_URL,
            format!("/curseforge{path}"),
            DownloadRouteSource::Mcim,
        ),
        _ => {}
    }

    routes
}

pub(crate) fn route_host(route: &DownloadRoute) -> Option<String> {
    Url::parse(&route.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

pub(crate) fn is_official_modrinth_download_url(url: &str) -> bool {
    route_policy::is_official_modrinth_download_url(url)
}

fn is_official_version_manifest_url(url: &str) -> bool {
    route_policy::is_official_version_manifest_url(url)
}

fn order_auto_routes(routes: &mut [DownloadRoute], resource: ResourceClass) {
    let health = ROUTE_HEALTH.lock().clone();
    routes.sort_by(|left, right| {
        let route_health = |route: &DownloadRoute| {
            let Some(key) = route_health_key(route, resource) else {
                return RouteHealth::default();
            };
            health
                .get(&key)
                .cloned()
                .unwrap_or_else(|| persisted_route_health(&key, route.proxy))
        };
        let left_health = route_health(left);
        let right_health = route_health(right);
        let now = Instant::now();
        let left_cooling =
            left_health.cooldown_until.is_some_and(|until| until > now);
        let right_cooling =
            right_health.cooldown_until.is_some_and(|until| until > now);
        left_cooling
            .cmp(&right_cooling)
            .then_with(|| {
                left_health
                    .consecutive_failures
                    .cmp(&right_health.consecutive_failures)
            })
            .then_with(|| {
                // Automatic content downloads start from the supplied
                // official URL. Health can still move a failing official
                // route behind a mirror, but connectivity alone is not a
                // reason to call a mirror faster.
                (!is_official_route(left)).cmp(&(!is_official_route(right)))
            })
            .then_with(|| {
                (right_health.success_samples > 0)
                    .cmp(&(left_health.success_samples > 0))
            })
            .then_with(|| {
                let ordering = if matches!(resource, ResourceClass::Metadata) {
                    left_health.ttfb_ms.partial_cmp(&right_health.ttfb_ms)
                } else {
                    right_health
                        .throughput_bps
                        .partial_cmp(&left_health.throughput_bps)
                };
                ordering.unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.is_mirror.cmp(&right.is_mirror))
    });
}

pub fn resolve_download_routes_for(
    url: &str,
    resource: ResourceClass,
    mode: crate::state::DownloadSourceMode,
) -> Vec<DownloadRoute> {
    let Ok(url) = retired_sources::normalize(url) else {
        return Vec::new();
    };
    let url = url.into_owned();
    let official = official_route(&url, resource);
    let mut routes = explicit_mirror_routes(&url, resource);
    routes.push(official);
    // Modrinth API calls are authenticated and remain official-only.
    let mode = if is_modrinth_api_url(&url) {
        crate::state::DownloadSourceMode::OfficialOnly
    } else {
        mode
    };
    match mode {
        crate::state::DownloadSourceMode::Auto
            if is_official_version_manifest_url(&url) =>
        {
            routes.sort_by_key(|route| route.is_mirror)
        }
        crate::state::DownloadSourceMode::Auto => {
            order_auto_routes(&mut routes, resource)
        }
        crate::state::DownloadSourceMode::OfficialOnly => {
            routes.retain(|route| !route.is_mirror);
        }
        crate::state::DownloadSourceMode::OfficialPreferred => {
            routes.sort_by_key(|route| route.is_mirror);
        }
        crate::state::DownloadSourceMode::MirrorPreferred => {
            routes.sort_by_key(|route| !route.is_mirror);
        }
    }
    routes
}

fn source_mode_for_resource(
    resource: ResourceClass,
) -> crate::state::DownloadSourceMode {
    let Some(state) = crate::State::get_if_initialized() else {
        return crate::state::DownloadSourceMode::OfficialOnly;
    };

    match resource {
        ResourceClass::Metadata => state.minecraft_metadata_source(),
        ResourceClass::MinecraftAsset
        | ResourceClass::MinecraftLibrary
        | ResourceClass::Loader
        | ResourceClass::Java => state.minecraft_file_source(),
        ResourceClass::Modrinth | ResourceClass::Modpack => {
            state.modrinth_source()
        }
        ResourceClass::CurseForge => state.curseforge_source(),
        ResourceClass::Other => crate::state::DownloadSourceMode::OfficialOnly,
    }
}

fn infer_resource_class(url: &str) -> ResourceClass {
    let Ok(parsed) = Url::parse(url) else {
        return ResourceClass::Other;
    };
    let host = parsed.host_str().unwrap_or_default();
    match host {
        FETCH_RESOURCES_DOWNLOAD_MINECRAFT_NET => ResourceClass::MinecraftAsset,
        FETCH_LIBRARIES_MINECRAFT_NET => ResourceClass::MinecraftLibrary,
        ROUTE_POLICY_MAVEN_MINECRAFTFORGE_NET
        | FETCH_FILES_MINECRAFTFORGE_NET
        | ROUTE_POLICY_MAVEN_FABRICMC_NET
        | ROUTE_POLICY_MAVEN_NEOFORGED_NET
        | FETCH_META_FABRICMC_NET => ResourceClass::Loader,
        FETCH_REPO1_MAVEN_ORG | FETCH_REPO_MAVEN_APACHE_ORG => {
            ResourceClass::MinecraftLibrary
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
            if parsed.path().contains(FETCH_JAVA_RUNTIME) =>
        {
            ResourceClass::Java
        }
        FETCH_PISTON_DATA_MOJANG_COM
            if parsed.path().contains(FETCH_JAVA_RUNTIME) =>
        {
            ResourceClass::Java
        }
        ROUTE_POLICY_PISTON_META_MOJANG_COM
        | ROUTE_POLICY_LAUNCHERMETA_MOJANG_COM => ResourceClass::Metadata,
        FETCH_LAUNCHER_MOJANG_COM | FETCH_PISTON_DATA_MOJANG_COM => {
            ResourceClass::MinecraftLibrary
        }
        MODRINTH_API_HOST | modrinth_cdn::HOST => ResourceClass::Modrinth,
        CURSEFORGE_API_HOST
        | CURSEFORGE_EDGE_HOST
        | RETIRED_SOURCES_MEDIA_FORGECDN_NET
        | FETCH_MEDIAFILEZ_FORGECDN_NET => ResourceClass::CurseForge,
        _ => ResourceClass::Other,
    }
}

#[derive(Debug, derive_more::Display, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[display(rename_all = "snake_case")]
pub enum DownloadReason {
    Standalone,
    Dependency,
    Modpack,
    Update,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadMeta {
    pub reason: DownloadReason,
    pub game_version: String,
    pub loader: String,
    pub dependent_on: Option<String>,
}

impl DownloadMeta {
    pub fn to_header_value(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct IoSemaphore(pub Semaphore);
#[derive(Debug)]
pub struct FetchSemaphore(pub Semaphore);

pub(crate) static DOWNLOAD_DNS_RESOLVER: LazyLock<Arc<DownloadDnsResolver>> =
    LazyLock::new(|| Arc::new(DownloadDnsResolver::default()));

/// Forces download requests to resolve `host` through `resolver_host` while
/// preserving `host` in the URL, HTTP Host header, and TLS SNI.
#[allow(dead_code)]
pub fn set_download_dns_host_override(
    host: &str,
    resolver_host: &str,
) -> crate::Result<()> {
    DOWNLOAD_DNS_RESOLVER
        .set_host_override(host, resolver_host)
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
}

#[allow(dead_code)]
pub fn clear_download_dns_host_override(host: &str) -> crate::Result<()> {
    DOWNLOAD_DNS_RESOLVER
        .clear_host_override(host)
        .map_err(|error| crate::ErrorKind::InputError(error.to_string()).into())
}
static TAIL_HEDGE_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(MAX_GLOBAL_TAIL_HEDGES));
static FILE_VALIDATION_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(FILE_VALIDATION_CONCURRENCY));
static ROUTE_PROBE_SEMAPHORE: LazyLock<FetchSemaphore> =
    LazyLock::new(|| FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)));

pub(crate) async fn acquire_native_validation_permit()
-> crate::Result<Option<SemaphorePermit<'static>>> {
    Ok(Some(FILE_VALIDATION_SEMAPHORE.acquire().await?))
}

static H2_FALLBACK_AUTHORITIES: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static HTTP1_PREFERENCE_AUTHORITIES: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn authority_uses_http1_fallback(authority: &str) -> bool {
    authority_uses_http1_fallback_for(authority, ProxyPolicy::System)
}

pub(crate) fn authority_uses_http1_fallback_for(
    authority: &str,
    proxy: ProxyPolicy,
) -> bool {
    let authority =
        super::download::proxy_context::authority_key(authority, proxy);
    {
        let mut preferences = HTTP1_PREFERENCE_AUTHORITIES.lock();
        preferences.retain(|_, until| *until > Instant::now());
        if preferences.contains_key(&authority) {
            return true;
        }
    }
    let mut fallbacks = H2_FALLBACK_AUTHORITIES.lock();
    match fallbacks.get(&authority) {
        Some(until) if *until > Instant::now() => true,
        Some(_) => {
            fallbacks.remove(&authority);
            false
        }
        None => false,
    }
}

pub(crate) fn record_authority_h2_failure(authority: &str) {
    record_authority_h2_failure_for(authority, ProxyPolicy::System);
}

pub(crate) fn record_authority_h2_failure_for(
    authority: &str,
    proxy: ProxyPolicy,
) {
    let now = Instant::now();
    let mut fallbacks = H2_FALLBACK_AUTHORITIES.lock();
    fallbacks.retain(|_, until| *until > now);
    fallbacks.insert(
        super::download::proxy_context::authority_key(authority, proxy),
        now + H2_FALLBACK_TTL,
    );
}

fn prefer_measured_http1(authority: &str, proxy: ProxyPolicy) {
    let mut preferences = HTTP1_PREFERENCE_AUTHORITIES.lock();
    preferences.retain(|_, until| *until > Instant::now());
    preferences.insert(
        super::download::proxy_context::authority_key(authority, proxy),
        Instant::now() + H2_SLOW_FALLBACK_TTL,
    );
}

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

fn reqwest_client_builder() -> reqwest::ClientBuilder {
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
        .read_timeout(FILE_TRANSFER_READ_TIMEOUT)
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
    if let Some(clients) = super::download::proxy_context::explicit_clients() {
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
    if let Some(clients) = super::download::proxy_context::explicit_clients() {
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
        Arc<super::download::endpoint_selection::EndpointSelector>,
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
            let builder = if super::download::endpoint_selection::SelectionMode::configured()
                == super::download::endpoint_selection::SelectionMode::Auto
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
                super::download::endpoint_selection::EndpointSelector::new(
                    super::download::proxy_context::fingerprint_with_doh(
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
            scope: super::download::proxy_context::fingerprint_with_doh(
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
        if let Some(mut clients) = super::download::proxy_context::clients() {
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
                super::download::endpoint_selection::EndpointSelector::new(
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

fn fetch_retry_delay(attempt: usize) -> time::Duration {
    let base = FETCH_RETRY_DELAYS
        .get(attempt.saturating_sub(1))
        .copied()
        .unwrap_or(*FETCH_RETRY_DELAYS.last().unwrap());
    let jitter = rand::thread_rng()
        .gen_range(FETCH_RETRY_JITTER_MIN..=FETCH_RETRY_JITTER_MAX);
    time::Duration::from_secs_f64(base.as_secs_f64() * jitter)
}

pub(crate) fn retry_after(
    response: &reqwest::Response,
) -> Option<time::Duration> {
    retry_after_headers(response.headers())
}

pub(crate) fn retry_after_headers(
    headers: &header::HeaderMap,
) -> Option<time::Duration> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(clamp_failure_cooldown(time::Duration::from_secs(
            seconds,
        )));
    }

    let retry_at = DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&Utc);
    let seconds = retry_at.signed_duration_since(Utc::now()).num_seconds();
    Some(clamp_failure_cooldown(time::Duration::from_secs(
        seconds.max(0) as u64,
    )))
}

fn clamp_failure_cooldown(cooldown: time::Duration) -> time::Duration {
    cooldown.min(RETRY_AFTER_CAP)
}

pub(crate) fn is_sensitive_header(name: &str) -> bool {
    name.eq_ignore_ascii_case(FETCH_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_PROXY_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_COOKIE)
        || name.eq_ignore_ascii_case(API_KEY_HEADER)
}

fn header_requires_official_only(name: &str) -> bool {
    name.eq_ignore_ascii_case(FETCH_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_PROXY_AUTHORIZATION)
        || name.eq_ignore_ascii_case(FETCH_COOKIE)
}

fn requires_modrinth_auth(
    method: &Method,
    header: Option<(&str, &str)>,
    uri_path: Option<&str>,
) -> bool {
    if method != Method::GET
        || header.is_some_and(|(name, _)| is_sensitive_header(name))
    {
        return true;
    }

    uri_path.is_some_and(|path| {
        matches!(path, FETCH_V2_USER | FETCH_V3_FRIENDS)
            || path.starts_with(FETCH_V2_SESSION)
            || path.starts_with(FETCH_V3_FRIEND)
            || path.starts_with(FETCH_V3_NOTIFICATION)
    })
}

fn record_route_success(
    route: &DownloadRoute,
    resource: ResourceClass,
    ttfb: time::Duration,
    bytes: u64,
    transfer_elapsed: time::Duration,
    remote_addr: Option<std::net::SocketAddr>,
) {
    if let (Some(host), Some(remote_addr)) = (route_host(route), remote_addr) {
        super::download::proxy_context::resolver(route.proxy)
            .record_host_success(&host, remote_addr.ip());
    }
    if let Some(key) = route_health_key(route, resource) {
        let baseline = persisted_route_health(&key, route.proxy);
        let throughput_bps = (!transfer_elapsed.is_zero())
            .then(|| bytes as f64 / transfer_elapsed.as_secs_f64());
        crate::util::download::native_breaker::record_success(route);
        crate::util::download::native_reputation::record_success(
            key.family.as_str(),
            &key.authority,
            route.proxy,
            ttfb.as_secs_f64() * 1000.0,
            throughput_bps,
        );
        let mut health = ROUTE_HEALTH.lock();
        let entry = health.entry(key).or_insert(baseline);
        entry.success_samples = entry.success_samples.saturating_add(1);
        entry.consecutive_failures = 0;
        entry.cooldown_until = None;
        update_ewma(&mut entry.ttfb_ms, ttfb.as_secs_f64() * 1000.0);
        if !transfer_elapsed.is_zero() {
            update_ewma(
                &mut entry.throughput_bps,
                bytes as f64 / transfer_elapsed.as_secs_f64(),
            );
        }
    }
}

pub(crate) fn record_route_transfer_success(
    route: &DownloadRoute,
    resource: ResourceClass,
    bytes: u64,
    transfer_elapsed: time::Duration,
) {
    if transfer_elapsed.is_zero() {
        return;
    }
    let Some(key) = route_health_key(route, resource) else {
        return;
    };
    let baseline = persisted_route_health(&key, route.proxy);
    let throughput_bps = bytes as f64 / transfer_elapsed.as_secs_f64();
    crate::util::download::native_breaker::record_success(route);
    crate::util::download::native_reputation::record_transfer_success(
        key.family.as_str(),
        &key.authority,
        route.proxy,
        throughput_bps,
    );
    let mut health = ROUTE_HEALTH.lock();
    let entry = health.entry(key).or_insert(baseline);
    entry.success_samples = entry.success_samples.saturating_add(1);
    entry.consecutive_failures = 0;
    entry.cooldown_until = None;
    update_ewma(&mut entry.throughput_bps, throughput_bps);
}

fn record_route_failure(
    route: &DownloadRoute,
    resource: ResourceClass,
    cooldown: Option<time::Duration>,
) {
    if cooldown.is_some()
        && let Some(state) = crate::State::get_if_initialized()
    {
        state.record_download_throttle();
    }
    record_route_health_failure(route, resource, cooldown);
}

pub(crate) fn record_dns_connection_failure(
    route: &DownloadRoute,
    error: &reqwest::Error,
) -> Option<String> {
    if !error.is_connect() && !error.is_timeout() {
        return None;
    }
    let host = error
        .url()
        .and_then(Url::host_str)
        .map(str::to_owned)
        .or_else(|| route_host(route))?;
    super::download::proxy_context::resolver(route.proxy)
        .record_connection_failure(&host)
        .then_some(host)
}

fn record_native_transfer_failure(
    route: &DownloadRoute,
    cooldown: Option<time::Duration>,
) {
    crate::util::download::native_breaker::record_failure_with_cooldown(
        route, cooldown,
    );
}

/// Records an HTTP status that asks the client to slow down. `429` and `503`
/// cool down their own bucket, so their pause is not reset by unrelated
/// connection failures on the same route.
fn record_native_status_failure(
    route: &DownloadRoute,
    status: reqwest::StatusCode,
    retry_after: Option<time::Duration>,
) {
    if !crate::util::download::native_breaker::record_response_status_failure(
        route,
        status.as_u16(),
        retry_after,
    ) {
        crate::util::download::native_breaker::record_failure_with_cooldown(
            route,
            retry_after,
        );
    }
}

pub(crate) fn record_route_health_failure(
    route: &DownloadRoute,
    resource: ResourceClass,
    cooldown: Option<time::Duration>,
) {
    if let Some(key) = route_health_key(route, resource) {
        let baseline = persisted_route_health(&key, route.proxy);
        crate::util::download::native_reputation::record_failure(
            key.family.as_str(),
            &key.authority,
            route.proxy,
        );
        let mut health = ROUTE_HEALTH.lock();
        let entry = health.entry(key).or_insert(baseline);
        entry.consecutive_failures =
            entry.consecutive_failures.saturating_add(1);
        if let Some(cooldown) = cooldown {
            entry.cooldown_until =
                Some(Instant::now() + cooldown.min(MAX_FAILURE_COOLDOWN));
        }
    }
}

static RANGE_SPLITTING_PROTOCOL_FAILURES: LazyLock<
    Mutex<HashMap<String, u32>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static RANGE_SPLITTING_SUPPORTED: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

pub(crate) fn range_splitting_authority(
    route: &DownloadRoute,
) -> Option<String> {
    effective_route_authority(route)
}

fn range_splitting_allowed(route: &DownloadRoute) -> bool {
    let host = route_host(route).unwrap_or_default();
    if [
        FETCH_GITHUB_COM,
        FETCH_OPTIFINE_NET,
        FETCH_MOMOT_RS,
        FETCH_MELOONG_COM,
    ]
    .iter()
    .any(|blocked| host.contains(blocked))
    {
        return false;
    }

    range_splitting_authority(route).is_none_or(|authority| {
        let authority = super::download::proxy_context::authority_key(
            &authority,
            route.proxy,
        );
        RANGE_SPLITTING_PROTOCOL_FAILURES
            .lock()
            .get(&authority)
            .copied()
            .unwrap_or(0)
            < RANGE_SPLITTING_DISABLE_THRESHOLD
    })
}

/// Records a range protocol failure for the route's server. A single failure
/// is treated as a transient blip, but once a server repeatedly mishandles
/// range requests, range splitting is disabled for it for this session so
/// later downloads skip the doomed segmented probe.
fn disable_range_splitting(route: &DownloadRoute) {
    let Some(authority) = range_splitting_authority(route) else {
        return;
    };
    let authority =
        super::download::proxy_context::authority_key(&authority, route.proxy);
    let mut failures = RANGE_SPLITTING_PROTOCOL_FAILURES.lock();
    let count = failures.entry(authority.clone()).or_insert(0);
    *count += 1;
    if *count == RANGE_SPLITTING_DISABLE_THRESHOLD {
        RANGE_SPLITTING_SUPPORTED.lock().remove(&authority);
        tracing::info!(
            authority,
            "Disabling range splitting for a server after repeated range protocol failures"
        );
    }
}

fn record_range_splitting_success(route: &DownloadRoute) {
    if let Some(authority) = range_splitting_authority(route) {
        RANGE_SPLITTING_SUPPORTED.lock().insert(
            super::download::proxy_context::authority_key(
                &authority,
                route.proxy,
            ),
        );
    }
}

pub type FetchProgressFn<'a> = dyn FnMut(
        u64,
        u64,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + 'a>>
    + Send
    + 'a;

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

async fn fetch_hedged_metadata(
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
            exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    client: &reqwest::Client,
    source_mode: Option<crate::state::DownloadSourceMode>,
    progress: Option<&mut FetchProgressFn<'_>>,
    response_validator: Option<
        &(dyn Fn(&Bytes) -> crate::Result<()> + Send + Sync),
    >,
    attempt_budget: usize,
) -> crate::Result<Bytes> {
    super::download::proxy_context::with_clients(fetch_advanced_inner(
        method,
        url,
        sha1,
        json_body,
        header,
        download_meta,
        loading_bar,
        uri_path,
        semaphore,
        exec,
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
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        crate::state::ModrinthCredentials::get_active(exec).await?
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
                        super::download::native_budget::acquire(route),
                    )
                    .await
                    .map_err(|_| {
                        ErrorKind::NetworkError(
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
                super::download::native_request::send_cdn_request_with_budget(
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
                        .get::<super::download::native_request::ResponseBudget>(
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
                    let endpoint_feedback = resp.extensions().get::<super::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
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
                                if let Some(feedback) = &endpoint_feedback { feedback.record(chunk.len(), super::download::local_resources::network_pressure()); }
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
                            super::download::local_resources::network_pressure(
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
                                super::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
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

#[derive(Default)]
pub(crate) struct IntegrityHashers {
    sha1: Option<sha1_smol::Sha1>,
    sha512: Option<Sha512>,
    sha256: Option<Sha256>,
    md5: Option<md5::Context>,
}

#[derive(Default)]
pub(crate) struct ComputedIntegrity {
    pub(crate) size: u64,
    pub(crate) sha1: Option<String>,
    pub(crate) sha512: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) md5: Option<String>,
}

impl IntegrityHashers {
    pub(crate) fn new_integrity_hashers(integrity: &Integrity) -> Self {
        Self {
            sha1: integrity.sha1.as_ref().map(|_| sha1_smol::Sha1::new()),
            sha512: integrity.sha512.as_ref().map(|_| Sha512::new()),
            sha256: integrity.sha256.as_ref().map(|_| Sha256::new()),
            md5: integrity.md5.as_ref().map(|_| md5::Context::new()),
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        if let Some(hasher) = &mut self.sha1 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.sha512 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.sha256 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.md5 {
            hasher.consume(bytes);
        }
    }

    pub(crate) fn finish(self, size: u64) -> ComputedIntegrity {
        ComputedIntegrity {
            size,
            sha1: self.sha1.map(|hasher| hasher.digest().to_string()),
            sha512: self
                .sha512
                .map(|hasher| format!("{:x}", hasher.finalize())),
            sha256: self
                .sha256
                .map(|hasher| format!("{:x}", hasher.finalize())),
            md5: self.md5.map(|hasher| format!("{:x}", hasher.finalize())),
        }
    }
}

pub(crate) fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

async fn remove_if_exists(path: &Path) -> crate::Result<()> {
    match io::retry_windows_sharing_violation(path, FETCH_REMOVING, || {
        tokio::fs::remove_file(path)
    })
    .await
    {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::io_error_with_lock_info(error, path).into()),
    }
}

async fn create_download_file(path: &Path) -> Result<File, IOError> {
    io::retry_windows_sharing_violation(
        path,
        FETCH_CREATING_DOWNLOAD_FILE,
        || File::create(path),
    )
    .await
    .map_err(|error| io::io_error_with_lock_info(error, path))
}

async fn open_download_file_for_append(path: &Path) -> Result<File, IOError> {
    io::retry_windows_sharing_violation(
        path,
        FETCH_OPENING_DOWNLOAD_FILE,
        || async {
            tokio::fs::OpenOptions::new().append(true).open(path).await
        },
    )
    .await
    .map_err(|error| io::io_error_with_lock_info(error, path))
}

/// Keeps a partial `.part` file for a later resume when the download can be
/// safely resumed, at least one route could actually serve a resume, and some
/// data has already arrived; removes it otherwise so unusable partial data
/// does not accumulate on disk.
async fn preserve_or_remove_partial(
    part_path: &Path,
    integrity: &Integrity,
    routes_can_resume: bool,
) -> crate::Result<()> {
    let resumable = routes_can_resume
        && integrity.supports_resume()
        && tokio::fs::metadata(part_path)
            .await
            .is_ok_and(|metadata| metadata.len() > 0);
    if !resumable {
        remove_if_exists(part_path).await?;
    }
    Ok(())
}

fn any_route_can_resume(routes: &[DownloadRoute]) -> bool {
    routes
        .iter()
        .any(|route| route.supports_range && range_splitting_allowed(route))
}

/// Whether a `.part` file with resume data exists at `part_path`. The
/// multiplexed path starts from scratch; resume is handled by the legacy
/// path so a partially downloaded file is never lost.
async fn part_resume_expected(part_path: &Path) -> bool {
    tokio::fs::metadata(part_path)
        .await
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false)
}

fn is_partial_download_file_name(name: &str) -> bool {
    name.ends_with(PARTIAL_FILE_SUFFIX)
        || name.ends_with(FETCH_PART_RANGES_JSON)
        || name
            .rsplit_once(FETCH_PART_RANGES_JSON_2)
            .is_some_and(|(_, suffix)| uuid::Uuid::parse_str(suffix).is_ok())
        || name
            .rsplit_once(FETCH_SEGMENT)
            .is_some_and(|(prefix, index)| {
                prefix.ends_with(PARTIAL_FILE_SUFFIX)
                    && !index.is_empty()
                    && index.bytes().all(|byte| byte.is_ascii_digit())
            })
}

/// Removes partial download files under launcher-managed directories that
/// have not been written to for a week. Partial data is preserved between
/// attempts so interrupted downloads can resume, but destinations that are
/// never requested again (for example superseded modpack versions) would
/// otherwise accumulate multi-gigabyte litter forever.
pub fn cleanup_stale_partial_downloads(directories: Vec<PathBuf>) {
    tokio::task::spawn_blocking(move || {
        let Some(cutoff) = std::time::SystemTime::now()
            .checked_sub(STALE_PARTIAL_DOWNLOAD_MAX_AGE)
        else {
            return;
        };
        let mut pending = directories;
        let mut removed = 0_u64;
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                if !file_type.is_file()
                    || !is_partial_download_file_name(
                        &entry.file_name().to_string_lossy(),
                    )
                {
                    continue;
                }
                let stale = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .is_ok_and(|modified| modified < cutoff);
                if stale && std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            tracing::info!(removed, "Removed stale partial download files");
        }
    });
}

/// Feeds an existing partial download into fresh integrity hashers so a
/// resumed transfer can continue hashing where the file left off. Returns
/// `None` when the file cannot be read back or its length changed.
async fn hash_existing_part_prefix(
    path: &Path,
    integrity: &Integrity,
    expected_len: u64,
) -> Option<IntegrityHashers> {
    let mut file = File::open(path).await.ok()?;
    let mut hashers = IntegrityHashers::new_integrity_hashers(integrity);
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; DOWNLOAD_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).await.ok()?;
        if read == 0 {
            break;
        }
        hashers.update(&buffer[..read]);
        size += read as u64;
    }
    (size == expected_len).then_some(hashers)
}

/// Keys the in-flight download lock on the destination path, so concurrent
/// downloads writing the same file (and thus the same sibling `.part` file)
/// serialize even when they expect different content. Uppercasing mirrors
/// NTFS `$UpCase` comparison semantics; unlike lowercasing it has no
/// context-sensitive folds that could split one on-disk file into two keys.
fn download_lock_key(destination: &Path) -> String {
    let path = destination.display().to_string();
    if cfg!(windows) {
        path.to_uppercase()
    } else {
        path
    }
}

fn in_flight_download_lock(key: String) -> Arc<AsyncMutex<()>> {
    use dashmap::mapref::entry::Entry;

    if IN_FLIGHT_DOWNLOADS.len() > 4_096 {
        IN_FLIGHT_DOWNLOADS.retain(|_, lock| lock.strong_count() > 0);
    }
    match IN_FLIGHT_DOWNLOADS.entry(key) {
        Entry::Occupied(mut entry) => {
            if let Some(lock) = entry.get().upgrade() {
                lock
            } else {
                let lock = Arc::new(AsyncMutex::new(()));
                entry.insert(Arc::downgrade(&lock));
                lock
            }
        }
        Entry::Vacant(entry) => {
            let lock = Arc::new(AsyncMutex::new(()));
            entry.insert(Arc::downgrade(&lock));
            lock
        }
    }
}

/// Serializes every writer for one destination, including download engines
/// that do not enter `download_to_path` (such as the H2 asset batch).
pub(crate) fn destination_download_lock(
    destination: &Path,
) -> Arc<AsyncMutex<()>> {
    in_flight_download_lock(download_lock_key(destination))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedContentRange {
    start: u64,
    end: u64,
    /// `None` when the server reports an unknown complete length (`*`),
    /// which RFC 9110 permits on a 206 response.
    total: Option<u64>,
}

fn parse_content_range(
    response: &reqwest::Response,
) -> Option<ParsedContentRange> {
    let value = response
        .headers()
        .get(header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .strip_prefix(FETCH_BYTES)?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some(ParsedContentRange {
        start: start.parse().ok()?,
        end: end.parse().ok()?,
        total: if total == "*" {
            None
        } else {
            Some(total.parse().ok()?)
        },
    })
}

async fn response_status_error(
    response: reqwest::Response,
    method: &Method,
    request_url: &str,
) -> crate::Error {
    let status = response.status();
    if let Ok(mut error) = response.json::<LabrinthError>().await {
        error.status = Some(status.as_u16());
        error.method = Some(method.as_str().to_string());
        error.url = Some(sanitize_url_for_log(request_url));
        ErrorKind::LabrinthError(error).into()
    } else {
        ErrorKind::HttpError {
            status: status.as_u16(),
            method: method.as_str().to_string(),
            url: sanitize_url_for_log(request_url),
        }
        .into()
    }
}

/// Commits a fully downloaded `.part` file to `destination` without ever
/// leaving the destination missing: the part file is flushed first, then
/// replaced in a single step. A destination that already exists is only
/// replaced once the new file is complete, so a failure here keeps the
/// previous contents instead of deleting them.
pub(crate) async fn finalize_download(
    part_path: &Path,
    destination: &Path,
) -> crate::Result<()> {
    io::retry_windows_sharing_violation(
        destination,
        FETCH_FINALIZING_DOWNLOAD,
        || finalize_download_once(part_path, destination),
    )
    .await
    .map_err(|error| {
        io::io_error_with_lock_info_for_paths(
            error,
            destination,
            &[destination, part_path],
        )
    })?;
    Ok(())
}

async fn finalize_download_once(
    part_path: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    // Starting with the flush is what makes the commit meaningful: replacing
    // the destination without it could publish contents that a crash then
    // loses. A flush that fails for any reason therefore aborts the commit and
    // leaves the previous destination in place; the partial file stays for the
    // caller's retry, and a partial file that never existed reports `NotFound`.
    io::fsync_file(part_path).await?;
    io::atomically_replace_file(part_path, destination)
}

pub(crate) fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

pub(crate) fn is_allowed_download_redirect(url: &Url) -> bool {
    if retired_sources::is_retired(url) {
        return false;
    }
    if url.scheme() == HTTPS_SCHEME {
        return true;
    }
    #[cfg(test)]
    if url.scheme() == HTTP_SCHEME
        && url.host_str().is_some_and(|host| {
            host == FETCH_LOCALHOST || host == FETCH_127_0_0_1
        })
    {
        return true;
    }
    false
}

#[derive(Clone)]
struct DownloadRange {
    index: usize,
    start: u64,
    state: Arc<Mutex<DownloadRangeState>>,
}

struct DownloadRangeState {
    end: u64,
    downloaded: u64,
    active: bool,
}

impl DownloadRange {
    fn new(index: usize, start: u64, end: u64) -> Self {
        Self {
            index,
            start,
            state: Arc::new(Mutex::new(DownloadRangeState {
                end,
                downloaded: 0,
                active: true,
            })),
        }
    }

    fn end(&self) -> u64 {
        self.state.lock().end
    }

    fn remaining(&self) -> u64 {
        let state = self.state.lock();
        state
            .end
            .saturating_add(1)
            .saturating_sub(self.start.saturating_add(state.downloaded))
    }

    fn is_active(&self) -> bool {
        self.state.lock().active
    }

    fn split_tail(&self, index: usize) -> Option<Self> {
        let mut state = self.state.lock();
        let remaining = state
            .end
            .saturating_add(1)
            .saturating_sub(self.start.saturating_add(state.downloaded));
        if remaining < DOWNLOAD_PROGRESS_MIN_BYTES {
            return None;
        }
        let split_size = remaining.saturating_mul(TAIL_SPLIT_PERCENT)
            / DOWNLOAD_PERCENT_BASE;
        let split_start =
            state.end.saturating_add(1).saturating_sub(split_size);
        if split_start <= self.start.saturating_add(state.downloaded) {
            return None;
        }
        let split_end = state.end;
        state.end = split_start - 1;
        drop(state);
        Some(Self::new(index, split_start, split_end))
    }

    fn accept_chunk(&self, chunk_size: usize) -> (usize, bool) {
        let mut state = self.state.lock();
        let remaining = state
            .end
            .saturating_add(1)
            .saturating_sub(self.start.saturating_add(state.downloaded));
        let accepted = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(chunk_size);
        state.downloaded += accepted as u64;
        (accepted, state.downloaded == state.end - self.start + 1)
    }

    fn finish(&self) -> bool {
        let mut state = self.state.lock();
        state.active = false;
        state.downloaded == state.end - self.start + 1
    }
}

struct DownloadRangeGuard(Arc<Mutex<DownloadRangeState>>);

impl Drop for DownloadRangeGuard {
    fn drop(&mut self) {
        self.0.lock().active = false;
    }
}

struct SegmentedDownloadSuccess {
    size: u64,
    final_url: String,
    ttfb: time::Duration,
    transfer_elapsed: time::Duration,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: Option<reqwest::Version>,
}

enum SegmentedDownloadOutcome {
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

enum SegmentDownloadError {
    Protocol(&'static str),
    Transport,
    Fatal(crate::Error),
}

#[derive(Clone, Debug)]
struct RouteProbeResult {
    route: DownloadRoute,
    bytes_per_second: u64,
    effective_authority: String,
}

type RouteProbeFuture<'a> =
    Pin<Box<dyn Future<Output = Option<RouteProbeResult>> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResourceValidator {
    etag: Option<String>,
    last_modified: Option<String>,
}

struct SegmentDownloadCompletion {
    final_url: String,
    is_first_range: bool,
    ttfb: time::Duration,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: Option<reqwest::Version>,
}

struct SegmentCleanupGuard {
    part_path: PathBuf,
    armed: bool,
}

impl SegmentCleanupGuard {
    fn new(part_path: &Path) -> Self {
        Self {
            part_path: part_path.to_path_buf(),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for SegmentCleanupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = std::fs::remove_file(&self.part_path);
        for index in 0..MAX_SEGMENT_CONCURRENCY {
            let _ = std::fs::remove_file(segment_path(&self.part_path, index));
            for candidate in 0..MAX_TAIL_HEDGES_PER_FILE {
                let _ = std::fs::remove_file(tail_candidate_path(
                    &self.part_path,
                    index,
                    candidate,
                ));
            }
        }
    }
}

fn response_validator(response: &reqwest::Response) -> ResourceValidator {
    ResourceValidator {
        etag: response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        last_modified: response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    }
}

fn validate_resource_version(
    expected: &Mutex<Option<ResourceValidator>>,
    response: &reqwest::Response,
) -> bool {
    let candidate = response_validator(response);
    let mut expected = expected.lock();
    match expected.as_ref() {
        Some(expected) => {
            (expected.etag.is_none() || expected.etag == candidate.etag)
                && (expected.last_modified.is_none()
                    || expected.last_modified == candidate.last_modified)
        }
        None => {
            *expected = Some(candidate);
            true
        }
    }
}

fn segmented_concurrency_cap(available_permits: usize) -> usize {
    available_permits.min(MAX_SEGMENT_CONCURRENCY)
}

fn route_segmented_concurrency_cap(
    route: &DownloadRoute,
    available_permits: usize,
) -> usize {
    let fair_share = (available_permits / DOWNLOAD_FAIR_SHARE_DIVISOR).max(1);

    segmented_concurrency_cap(available_permits)
        .min(fair_share)
        .min(crate::util::download::native_budget::available(route))
}

pub(crate) fn configured_semaphore_limit(semaphore: &FetchSemaphore) -> usize {
    if let Some(state) = crate::State::get_if_initialized()
        && (std::ptr::eq(
            &raw const state.fetch_semaphore.0,
            &raw const semaphore.0,
        ) || std::ptr::eq(
            &raw const state.download_semaphore.0,
            &raw const semaphore.0,
        ) || std::ptr::eq(
            &raw const state.api_semaphore.0,
            &raw const semaphore.0,
        ))
    {
        return state.download_concurrency();
    }
    semaphore.0.available_permits().max(1)
}

async fn acquire_initial_segment_permits<'a>(
    route: &DownloadRoute,
    semaphore: &'a FetchSemaphore,
    count: usize,
) -> crate::Result<Vec<NativeConnectionPermit<'a>>> {
    let queue_started = Instant::now();
    let native_permits =
        crate::util::download::native_budget::acquire_many(route, count)
            .await?;
    let mut global = semaphore.0.acquire_many(count as u32).await?;
    let mut permits = Vec::with_capacity(count);
    for native in native_permits {
        let global = global
            .split(1)
            .expect(FETCH_FETCH_PERMIT_BATCH_HAS_ENOUGH_PERMITS);
        permits.push(NativeConnectionPermit {
            _global: global,
            _native: native,
        });
    }
    tracing::debug!(
        queue_wait_ms = queue_started.elapsed().as_millis(),
        actual_segments = permits.len(),
        "Acquired initial segmented download permits"
    );
    Ok(permits)
}

pub(crate) struct NativeConnectionPermit<'a> {
    _global: SemaphorePermit<'a>,
    _native: crate::util::download::native_budget::NativeBudgetPermit,
}

pub(crate) async fn acquire_native_connection<'a>(
    route: &DownloadRoute,
    semaphore: &'a FetchSemaphore,
) -> crate::Result<NativeConnectionPermit<'a>> {
    let native = crate::util::download::native_budget::acquire(route).await?;
    let global = semaphore.0.acquire().await?;
    Ok(NativeConnectionPermit {
        _global: global,
        _native: native,
    })
}

fn try_acquire_native_connection<'a>(
    route: &DownloadRoute,
    semaphore: &'a FetchSemaphore,
) -> Option<NativeConnectionPermit<'a>> {
    let native =
        crate::util::download::native_budget::try_acquire(route).ok()?;
    let global = semaphore.0.try_acquire().ok()?;
    Some(NativeConnectionPermit {
        _global: global,
        _native: native,
    })
}

fn initial_segment_count(size: u64, available_permits: usize) -> usize {
    let size_limit = usize::try_from(size / MIN_SEGMENT_SIZE)
        .unwrap_or(usize::MAX)
        .max(1);
    INITIAL_SEGMENT_CONCURRENCY
        .min(segmented_concurrency_cap(available_permits))
        .min(size_limit)
}

fn adaptive_range_cap(
    size: u64,
    route_cap: usize,
    disk_cap: usize,
    consecutive_failures: u32,
) -> usize {
    let size_cap = (size / ADAPTIVE_RANGE_BYTES)
        .max(1)
        .min(MAX_SEGMENT_CONCURRENCY as u64) as usize;
    let failure_cap = match consecutive_failures {
        0..=RANGE_FULL_FAILURE_LIMIT => MAX_SEGMENT_CONCURRENCY,
        2..=RANGE_REDUCED_FAILURE_LIMIT => RANGE_REDUCED_CONCURRENCY,
        _ => 1,
    };
    route_cap.min(size_cap).min(disk_cap).min(failure_cap)
}

fn create_initial_ranges(size: u64, count: usize) -> Vec<DownloadRange> {
    let base_size = size / count as u64;
    let remainder = size % count as u64;
    let mut start = 0_u64;
    (0..count)
        .map(|index| {
            let range_size = base_size + u64::from(index < remainder as usize);
            let end = start + range_size - 1;
            let range = DownloadRange::new(index, start, end);
            start = end + 1;
            range
        })
        .collect()
}

fn expansion_block_reason(
    snapshot: SpeedSnapshot,
    active_ranges: usize,
    concurrency_cap: usize,
    available_permits: usize,
    remaining_bytes: u64,
    elapsed_since_expansion: time::Duration,
) -> Option<&'static str> {
    if active_ranges >= concurrency_cap {
        return Some(FETCH_EFFECTIVE_CONCURRENCY_CAP_REACHED);
    }
    if available_permits == 0 {
        return Some(FETCH_NO_GLOBAL_PERMIT_AVAILABLE);
    }
    if elapsed_since_expansion < SEGMENT_EXPANSION_INTERVAL {
        return Some(FETCH_EXPANSION_COOLDOWN_ACTIVE);
    }
    if snapshot.sample_count < SEGMENT_EXPANSION_SAMPLE_COUNT {
        return Some(FETCH_INSUFFICIENT_AGGREGATE_SPEED_SAMPLES);
    }
    if remaining_bytes
        < MIN_SEGMENT_SIZE.saturating_mul(active_ranges as u64 + 1)
    {
        return Some(FETCH_TOO_LITTLE_DATA_REMAINS);
    }
    None
}

fn allow_low_throughput_route_switch(
    has_alternate_route: bool,
    retry_with_single_thread: bool,
) -> bool {
    has_alternate_route && !retry_with_single_thread
}

fn should_downgrade_slow_http2(
    http_version: reqwest::Version,
    retry_with_single_thread: bool,
) -> bool {
    http_version == reqwest::Version::HTTP_2 && !retry_with_single_thread
}

fn native_first_byte_timeout(
    route: &DownloadRoute,
    has_alternate_route: bool,
) -> time::Duration {
    #[cfg(test)]
    if BENCHMARK_TIMING.try_with(|_| ()).is_ok() {
        return time::Duration::from_secs(
            if route.is_mirror && has_alternate_route {
                5
            } else {
                20
            },
        );
    }
    if route.is_mirror && has_alternate_route {
        REASSIGNABLE_FIRST_BYTE_TIMEOUT
    } else {
        FILE_TRANSFER_FIRST_BYTE_TIMEOUT
    }
}

fn should_use_segmented_download(size: u64, resumable_part_bytes: u64) -> bool {
    size >= SEGMENTED_DOWNLOAD_THRESHOLD && resumable_part_bytes < size / 2
}

fn probe_is_meaningfully_faster(
    candidate_bytes_per_second: u64,
    current_bytes_per_second: u64,
) -> bool {
    u128::from(candidate_bytes_per_second) * 100
        >= u128::from(current_bytes_per_second)
            * u128::from(100 + ROUTE_PROBE_MIN_IMPROVEMENT_PERCENT)
}

fn measured_bytes_per_second(bytes: u64, elapsed: time::Duration) -> u64 {
    let bytes_per_second = u128::from(bytes).saturating_mul(1_000_000_000)
        / elapsed.as_nanos().max(1);
    bytes_per_second.min(u128::from(u64::MAX)) as u64
}

fn expected_route_speed(
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
async fn probe_faster_route(
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
async fn probe_recovery_route(
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
        super::download::native_slow::should_switch(
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
    state: Arc<super::download::route_health::TaskProbeState>,
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
                super::download::proxy_context::authority_key(
                    &authority,
                    route.proxy,
                )
            })
        })
        .collect::<Vec<_>>();
    let scope = super::download::route_health::probe_scope(
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
        use super::download::endpoint_selection::tuning::{TRIALS, TrialKind};
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
    if super::download::proxy_context::clients().is_some_and(|clients| {
        clients.endpoints.mode() == super::download::endpoint_selection::SelectionMode::Auto
            && request.integrity.supports_resume() && request.header.is_none()
            && Url::parse(&request.url).is_ok_and(|url| modrinth_cdn::is_current_url(&url) && url.path().starts_with(MODRINTH_DATA_PREFIX) && url.query().is_none())
            && !routes.is_empty() && routes.iter().all(|route| !route.is_mirror
                && Url::parse(&route.url).is_ok_and(|url| modrinth_cdn::is_current_url(&url))
                && super::download::endpoint_selection::EndpointSelector::eligible_network(&clients, route.proxy))
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
                super::download::proxy_context::with_snapshot(
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

fn segment_path(part_path: &Path, index: usize) -> PathBuf {
    suffixed_path(part_path, &format!(".segment-{index}"))
}

fn tail_candidate_path(
    part_path: &Path,
    range_index: usize,
    candidate_index: usize,
) -> PathBuf {
    suffixed_path(
        part_path,
        &format!(".segment-{range_index}.tail-{candidate_index}"),
    )
}

struct TailCandidateCleanupGuard {
    path: PathBuf,
    armed: bool,
}

impl TailCandidateCleanupGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TailCandidateCleanupGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

struct TailCandidateCompletion {
    path: PathBuf,
    final_url: String,
    remote_addr: Option<std::net::SocketAddr>,
    http_version: reqwest::Version,
}

impl Drop for TailCandidateCompletion {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn cleanup_segment_files(
    part_path: &Path,
    segment_count: usize,
) -> crate::Result<()> {
    for index in 0..segment_count {
        remove_if_exists(&segment_path(part_path, index)).await?;
        for candidate in 0..MAX_TAIL_HEDGES_PER_FILE {
            remove_if_exists(&tail_candidate_path(part_path, index, candidate))
                .await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_tail_candidate(
    route: &DownloadRoute,
    range: &DownloadRange,
    total_size: u64,
    requested_start: u64,
    requested_end: u64,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    part_path: &Path,
    candidate_index: usize,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    validator: &Mutex<Option<ResourceValidator>>,
    redirect_target: Option<&AsyncMutex<Option<Url>>>,
) -> Result<TailCandidateCompletion, SegmentDownloadError> {
    let _activity = (candidate_index > 0)
        .then(crate::State::get_if_initialized)
        .flatten()
        .map(|state| state.begin_download_connection());
    let path = tail_candidate_path(part_path, range.index, candidate_index);
    let mut cleanup = TailCandidateCleanupGuard::new(path.clone());
    let response = tokio::time::timeout(
        FILE_TRANSFER_FIRST_BYTE_TIMEOUT,
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
    .await
    .map_err(|_| SegmentDownloadError::Transport)?
    .map_err(|_| SegmentDownloadError::Transport)?;
    let (response, final_url) = response;
    let parsed_content_range = parse_content_range(&response);
    if response.status() != StatusCode::PARTIAL_CONTENT
        || !parsed_content_range.is_some_and(|range| {
            range.start == requested_start
                && range.end == requested_end
                && range.total.is_none_or(|total| total == total_size)
        })
    {
        return Err(SegmentDownloadError::Protocol(
            FETCH_INVALID_HEDGED_CONTENT_RANGE,
        ));
    }
    if !validate_resource_version(validator, &response) {
        return Err(SegmentDownloadError::Protocol(
            FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE,
        ));
    }
    let remote_addr = response.remote_addr();
    let http_version = response.version();
    let expected = requested_end.saturating_sub(requested_start) + 1;
    let mut received = 0_u64;
    let mut file = create_download_file(&path)
        .await
        .map_err(|error| SegmentDownloadError::Fatal(error.into()))?;
    let mut stream = response.bytes_stream();
    loop {
        let chunk =
            tokio::time::timeout(RANGE_IDLE_RECONNECT_TIMEOUT, stream.next())
                .await
                .map_err(|_| SegmentDownloadError::Transport)?;
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                if is_h2_protocol_failure(&error)
                    && let Some(authority) = url_authority(&final_url)
                {
                    record_authority_h2_failure_for(&authority, route.proxy);
                }
                return Err(SegmentDownloadError::Transport);
            }
        };
        let accepted = usize::try_from(expected.saturating_sub(received))
            .unwrap_or(usize::MAX)
            .min(chunk.len());
        super::download::local_resources::write(
            &path,
            accepted as u64,
            async {
                file.write_all(&chunk[..accepted]).await?;
                file.flush().await
            },
        )
        .await
        .map_err(|error| {
            SegmentDownloadError::Fatal(IOError::with_path(error, &path).into())
        })?;
        received += accepted as u64;
        if let Some(state) = crate::State::get_if_initialized() {
            state.record_download_bytes(accepted as u64);
        }
        if received == expected {
            break;
        }
    }
    if received != expected {
        return Err(SegmentDownloadError::Transport);
    }
    super::download::local_resources::write(&path, 0, file.flush())
        .await
        .map_err(|error| {
            SegmentDownloadError::Fatal(IOError::with_path(error, &path).into())
        })?;
    drop(file);
    cleanup.disarm();
    Ok(TailCandidateCompletion {
        path,
        final_url,
        remote_addr,
        http_version,
    })
}

#[allow(clippy::too_many_arguments)]
async fn race_tail_candidates(
    route: &DownloadRoute,
    range: &DownloadRange,
    total_size: u64,
    requested_start: u64,
    requested_end: u64,
    custom_header: Option<&(String, String)>,
    credentials: Option<&crate::state::ModrinthCredentials>,
    download_meta: Option<&DownloadMeta>,
    part_path: &Path,
    system_client: &reqwest::Client,
    direct_client: &reqwest::Client,
    validator: &Mutex<Option<ResourceValidator>>,
    redirect_target: Option<&AsyncMutex<Option<Url>>>,
) -> Result<(TailCandidateCompletion, usize), SegmentDownloadError> {
    let first = download_tail_candidate(
        route,
        range,
        total_size,
        requested_start,
        requested_end,
        custom_header,
        credentials,
        download_meta,
        part_path,
        0,
        system_client,
        direct_client,
        validator,
        redirect_target,
    );
    let second = download_tail_candidate(
        route,
        range,
        total_size,
        requested_start,
        requested_end,
        custom_header,
        credentials,
        download_meta,
        part_path,
        1,
        system_client,
        direct_client,
        validator,
        redirect_target,
    );
    tokio::pin!(first, second);
    tokio::select! {
        result = &mut first => match result {
            Ok(result) => Ok((result, 0)),
            Err(first_error) => second.await.map(|result| (result, 1)).map_err(|_| first_error),
        },
        result = &mut second => match result {
            Ok(result) => Ok((result, 1)),
            Err(second_error) => first.await.map(|result| (result, 0)).map_err(|_| second_error),
        },
    }
}

#[allow(clippy::too_many_arguments)]
async fn download_segment(
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
    let mut slow = super::download::native_slow::NativeSlowPolicy::new(0, None);
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
            FILE_TRANSFER_FIRST_BYTE_TIMEOUT,
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
        if !validate_resource_version(validator, &response) {
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
        let endpoint_feedback = response.extensions().get::<super::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
        let mut stream = Some(response.bytes_stream());
        let mut stream_end_reason = None;
        loop {
            let tail_threshold = TAIL_HEDGE_MIN_REMAINING.max(total_size / 10);
            let tail_eligible = range.remaining() <= tail_threshold;
            let chunk = match tokio::time::timeout(
                if tail_eligible {
                    TAIL_HEDGE_IDLE_TIMEOUT
                } else {
                    RANGE_IDLE_RECONNECT_TIMEOUT
                },
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
                        TAIL_HEDGE_IDLE_TIMEOUT.as_secs_f64()
                    ));
                    break;
                }
                Err(_) => {
                    stream_end_reason = Some(format!(
                        "no range data for {:.0} seconds",
                        RANGE_IDLE_RECONNECT_TIMEOUT.as_secs_f64()
                    ));
                    tracing::warn!(
                        url = %sanitize_url_for_log(&route.url),
                        range_start = requested_start,
                        range_end = requested_end,
                        remaining_bytes = range.remaining(),
                        idle_seconds = RANGE_IDLE_RECONNECT_TIMEOUT.as_secs_f64(),
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
                    super::download::local_resources::pressure(part_path),
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
                super::download::local_resources::pressure(part_path);
            let downloaded_range = range.state.lock().downloaded;
            if let super::download::native_slow::SlowDecision::Probe {
                bytes_per_second,
            } = slow.observe_with_pressure(
                downloaded_range,
                range.remaining(),
                pressure,
            ) {
                last_slow_speed = Some(bytes_per_second);
                if let Some(clients) = super::download::proxy_context::clients()
                {
                    clients.endpoints.on_slow_region(
                        request,
                        route,
                        &clients,
                        super::download::endpoint_selection::Protocol::Http1,
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
                && let Some(clients) = super::download::proxy_context::clients()
                && clients
                    .endpoints
                    .recovery_lease(
                        request,
                        route,
                        super::download::endpoint_selection::Protocol::Http1,
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

struct SegmentedDownloadContext<'a> {
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
    fn new(
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

async fn try_segmented_download(
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
    let configured_limit = configured_semaphore_limit(semaphore);
    let route_cap = route_segmented_concurrency_cap(route, configured_limit);
    let health =
        route_health_key(route, request.resource)
            .map(|key| {
                ROUTE_HEALTH.lock().get(&key).cloned().unwrap_or_else(|| {
                    persisted_route_health(&key, route.proxy)
                })
            })
            .unwrap_or_default();
    let disk_cap =
        super::download::local_resources::range_limit(part_path).await;
    let concurrency_cap = adaptive_range_cap(
        size,
        route_cap,
        disk_cap,
        health.consecutive_failures,
    );
    let requested_initial_count = initial_segment_count(size, concurrency_cap);
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
    if tokio::fs::metadata(part_path)
        .await
        .is_ok_and(|metadata| metadata.len() > 0)
    {
        return SegmentedDownloadOutcome::FallbackSingle {
            disable_range: false,
            reason: FETCH_RESUMABLE_PARTIAL_USES_A_SINGLE_STREAM,
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
    let mut cleanup_guard = SegmentCleanupGuard::new(part_path);
    let output = match crate::util::download::range_output::RangeOutput::create(
        part_path, size,
    )
    .await
    {
        Ok(output) => output,
        Err(error) => return SegmentedDownloadOutcome::Fatal(error.into()),
    };
    record_install_download_started(request, route, attempt, max_attempts)
        .await;
    let transfer_started = Instant::now();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let speed = DownloadSpeedTracker::default();
    let validator = Mutex::new(None);
    let redirect_target = AsyncMutex::new(None);
    let hedge_count = AtomicUsize::new(0);
    let hedge_active = AtomicBool::new(false);
    let mut downloads = futures::stream::FuturesUnordered::new();
    let mut ranges = create_initial_ranges(size, permits.len());
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
        supports_range = true,
        active_ranges = downloads.len(),
        concurrency_cap,
        available_permits = semaphore.0.available_permits(),
        reason = "initial parallel ranges",
        "Started segmented download"
    );
    let mut next_range_index = ranges.len();
    let mut scheduler = tokio::time::interval(DOWNLOAD_SCHEDULER_INTERVAL);
    scheduler.tick().await;
    let mut last_expansion = Instant::now();
    let mut expansion_baseline = None;
    let mut expansion_trial: Option<
        super::download::endpoint_selection::tuning::TrialLease,
    > = None;
    let mut expansion_exhausted = false;
    let mut last_block_reason = None;
    let mut downloaded = 0_u64;
    let mut slow_policy =
        crate::util::download::native_slow::NativeSlowPolicy::new(
            0,
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
                    super::download::local_resources::pressure(part_path),
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
                let pressure = super::download::local_resources::pressure(part_path);
                if pressure.blocks_expansion() {
                    tracing::debug!(cpu_percent = pressure.cpu_percent, write_latency_ms = pressure.write_latency_ms, "Pausing range expansion under local resource pressure");
                    continue;
                }
                let snapshot = speed.speed_snapshot();
                use super::download::endpoint_selection::tuning::{TRIALS, TrialKind};
                if expansion_trial.as_ref().is_some_and(|lease| !lease.valid(Instant::now())) {
                    expansion_trial = None;
                    expansion_baseline = None;
                }
                if expansion_trial.is_none() && TRIALS.snapshot(Instant::now()).1.is_some() { continue; }
                if expansion_baseline.is_some() && last_expansion.elapsed() >= SEGMENT_EXPANSION_INTERVAL {
                    expansion_trial = None;
                }
                let active_ranges = downloads.len();
                let remaining_bytes = ranges
                    .iter()
                    .filter(|range| range.is_active())
                    .map(DownloadRange::remaining)
                    .sum();
                if expansion_exhausted {
                    continue;
                }
                if last_expansion.elapsed() >= SEGMENT_EXPANSION_INTERVAL
                    && let Some(baseline) = expansion_baseline.take()
                    && u128::from(snapshot.recent_average) * 100
                        < u128::from(baseline) * 115
                    {
                        expansion_exhausted = true;
                        tracing::debug!(
                            original_url = %sanitize_url_for_log(&route.url),
                            baseline_speed = baseline,
                            expanded_speed = snapshot.recent_average,
                            "Stopping range expansion because the last connection added less than 15 percent throughput"
                        );
                        continue;
                    }
                if let Some(reason) = expansion_block_reason(
                    snapshot,
                    active_ranges,
                    concurrency_cap,
                    semaphore.0.available_permits(),
                    remaining_bytes,
                    last_expansion.elapsed(),
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
                let range = ranges
                    .iter()
                    .filter(|range| range.is_active())
                    .max_by_key(|range| range.remaining())
                    .cloned();
                if let Some(range) = range
                    && let Some(trial) = TRIALS.acquire(TrialKind::Ranges, Instant::now(), SEGMENT_EXPANSION_INTERVAL + DOWNLOAD_SCHEDULING_GRACE)
                    && let Some(permit) =
                        try_acquire_native_connection(route, semaphore)
                    && let Some(new_range) =
                        range.split_tail(next_range_index)
                    {
                        tracing::debug!(
                            original_url = %sanitize_url_for_log(&route.url),
                            source = route.source.as_str(),
                            active_ranges = active_ranges + 1,
                            aggregate_speed = snapshot.aggregate_speed,
                            recent_average = snapshot.recent_average,
                            floor = snapshot.speed_floor,
                            range_start = new_range.start,
                            range_end = new_range.end(),
                            reason = "stable aggregate throughput above floor",
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
                        expansion_baseline = Some(snapshot.recent_average);
                        expansion_trial = Some(trial);
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
                    disable_range: true,
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

pub(crate) async fn record_install_download_started(
    request: &DownloadRequest,
    route: &DownloadRoute,
    attempt: usize,
    max_attempts: usize,
) {
    let Some(tracking) = &request.install_tracking else {
        return;
    };
    if let Err(error) = tracking
        .reporter
        .record_download_request(
            &tracking.item_id,
            &tracking.item_name,
            &sanitize_url_for_log(&route.url),
            route.source.as_str(),
            request.integrity.size,
            attempt as u32,
            max_attempts as u32,
        )
        .await
    {
        tracing::warn!(%error, "Failed to record active download request");
    }
}

pub(crate) async fn record_install_download_progress(
    request: &DownloadRequest,
    bytes: u64,
    total: u64,
) {
    let Some(tracking) = &request.install_tracking else {
        return;
    };
    if let Err(error) = tracking
        .reporter
        .record_download_progress(&tracking.item_id, bytes, total)
        .await
    {
        tracing::warn!(%error, "Failed to record download progress");
    }
}

pub(crate) async fn record_install_download_stage(
    request: &DownloadRequest,
    status: DownloadItemStatus,
) {
    let Some(tracking) = &request.install_tracking else {
        return;
    };
    if let Err(error) = tracking
        .reporter
        .record_download_stage(&tracking.item_id, status)
        .await
    {
        tracing::warn!(%error, "Failed to record download stage");
    }
}

/// Resolves hosts ahead of the first request so every file shares one ordered
/// address list instead of racing the same DNS queries.
pub(crate) async fn prewarm_download_dns(hosts: &[&str]) {
    prewarm_download_dns_for(ProxyPolicy::System, hosts).await;
}

pub(crate) async fn prewarm_download_dns_for(
    proxy: ProxyPolicy,
    hosts: &[&str],
) {
    let resolver = super::download::proxy_context::resolver(proxy);
    let hosts = hosts
        .iter()
        .map(|host| (*host).to_string())
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        let requests = hosts.into_iter().map(|host| {
            let resolver = Arc::clone(&resolver);
            async move {
                let _ = tokio::time::timeout(
                    FILE_TCP_KEEPALIVE,
                    resolver.pre_resolve(&host),
                )
                .await;
            }
        });
        futures::stream::iter(requests)
            .buffer_unordered(DOWNLOAD_DNS_PREWARM_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
    });
}

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
#[tracing::instrument(skip(semaphore, _exec, progress, request, destination))]
pub async fn download_to_path(
    mut request: DownloadRequest,
    destination: impl AsRef<Path>,
    semaphore: &FetchSemaphore,
    _exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
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
        super::download::proxy_context::with_clients(
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

fn build_download_routes(
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

async fn select_h2_download_route(
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
    let policy = if request.h2_range_concurrency.is_some() {
        if !super::download::h2_pool::has_live_connection(&h2_route).await {
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
    remove_if_exists(&super::download::range_journal::path(part_path)).await?;
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

enum H2AttemptResult {
    Completed(DownloadResult),
    Fallback { failed_nonofficial: Option<String> },
}

async fn try_h2_download(
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
            Ok(H2AttemptResult::Fallback { failed_nonofficial })
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
            super::download::h2_range::resume_http1(
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

async fn resume_checkpointed_download_with<F, Fut>(
    request: &DownloadRequest,
    routes: &[DownloadRoute],
    part_path: &Path,
    mut attempt: F,
) -> crate::Result<Option<DownloadResult>>
where
    F: FnMut(DownloadRoute) -> Fut,
    Fut: std::future::Future<
            Output = crate::Result<
                Option<super::download::h2_download::H2DownloadOutcome>,
            >,
        >,
{
    let mut had_range_failure = false;
    for route in routes {
        if let Some(outcome) = attempt(route.clone()).await? {
            match outcome {
                super::download::h2_download::H2DownloadOutcome::Completed(
                    result,
                ) => {
                    return Ok(Some(
                        attach_verified_integrity(result, &request.integrity)
                            .await,
                    ));
                }
                super::download::h2_download::H2DownloadOutcome::Canceled => {
                    return Err(ErrorKind::OtherError(
                        FETCH_DOWNLOAD_CANCELED.into(),
                    )
                    .into());
                }
                super::download::h2_download::H2DownloadOutcome::Fallback {
                    failure,
                    preserve_partial,
                } => {
                    if failure
                        == super::download::h2_download::H2DownloadFailure::Io
                    {
                        return Err(ErrorKind::OtherError(
                            FETCH_CHECKPOINT_RESUME_FAILED_DUE_TO_LOCAL_I_O
                                .into(),
                        )
                        .into());
                    }
                    if !preserve_partial {
                        super::download::h2_range::discard(part_path).await?;
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
        super::download::h2_range::discard(part_path).await?;
    }
    Ok(None)
}

async fn download_to_path_inner(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
    progress: Option<&mut FetchProgressFn<'_>>,
) -> crate::Result<DownloadResult> {
    let _endpoint_registration = super::download::proxy_context::clients()
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

    // Prefer one stream on a healthy shared HTTP/2 connection when the file
    // size and transport reputation justify it. Larger or slow H2 transfers
    // fall through to independent HTTP/1.1 range connections.
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
            H2AttemptResult::Fallback { failed_nonofficial } => {
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

#[cfg(test)]
pub(crate) async fn download_endpoint_fixture(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    download_to_path_inner(request, destination, semaphore, None).await
}

#[cfg(test)]
tokio::task_local! { static BENCHMARK_TIMING: (); }

#[cfg(test)]
pub(crate) async fn download_benchmark_fixture(
    request: DownloadRequest,
    destination: &Path,
    semaphore: &FetchSemaphore,
) -> crate::Result<DownloadResult> {
    BENCHMARK_TIMING
        .scope(
            (),
            download_to_path_inner(request, destination, semaphore, None),
        )
        .await
}

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
    if request.allow_http1_segmented_download
        && !retry_with_single_thread
        && !session.single_thread_routes.contains(&route.url)
        && route.supports_range
        && range_splitting_allowed(route)
        && request.integrity.size.is_some_and(|size| {
            should_use_segmented_download(size, resumable_part_bytes)
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
                if let Some(authority) = original_route_authority(route) {
                    crate::util::download::native_reputation::record_transport_success(
                        &authority,
                        route.proxy,
                        crate::util::download::native_reputation::NativeTransport::Http1MultiRange,
                        result.size as f64
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
                    result.size,
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
                        super::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
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
                if disable_range {
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

async fn run_native_download_attempts(
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
#[tracing::instrument(skip_all)]
pub async fn post_json(
    url: &str,
    json_body: serde_json::Value,
    semaphore: &FetchSemaphore,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<()> {
    let _permit = semaphore.0.acquire().await?;

    let mut req = INSECURE_REQWEST_CLIENT.post(url).json(&json_body);

    if let Some(creds) =
        crate::state::ModrinthCredentials::get_active(exec).await?
    {
        req = req.header(AUTHORIZATION_HEADER, &creds.session);
    }

    req.send().await?.error_for_status()?;
    Ok(())
}

pub async fn read_json<T>(
    path: &Path,
    semaphore: &IoSemaphore,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let _permit = semaphore.0.acquire().await?;

    let json = io::read(path).await?;
    let json = serde_json::from_slice::<T>(&json)?;

    Ok(json)
}

#[tracing::instrument(skip(bytes, semaphore))]
pub async fn write(
    path: &Path,
    bytes: &[u8],
    semaphore: &IoSemaphore,
) -> crate::Result<()> {
    let _permit = semaphore.0.acquire().await?;

    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }

    let mut file = create_download_file(path).await?;
    file.write_all(bytes).await.map_err(|e| {
        crate::Error::from(io::io_error_with_lock_info(e, path))
    })?;
    tracing::trace!("Done writing file {}", path.display());
    Ok(())
}

pub async fn copy(
    src: impl AsRef<Path>,
    dest: impl AsRef<Path>,
    semaphore: &IoSemaphore,
) -> crate::Result<()> {
    let src: &Path = src.as_ref();
    let dest = dest.as_ref();

    let _permit = semaphore.0.acquire().await?;

    if let Some(parent) = dest.parent() {
        io::create_dir_all(parent).await?;
    }

    io::copy(src, dest).await?;
    tracing::trace!(
        "Done copying file {} to {}",
        src.display(),
        dest.display()
    );
    Ok(())
}

// Writes a icon to the cache and returns the absolute path of the icon within the cache directory
#[tracing::instrument(skip(bytes, semaphore))]
pub async fn write_cached_icon(
    icon_path: &str,
    cache_dir: &Path,
    bytes: Bytes,
    semaphore: &IoSemaphore,
) -> crate::Result<PathBuf> {
    let extension = Path::new(&icon_path).extension().and_then(OsStr::to_str);
    let hash = sha1_async(bytes.clone()).await?;
    let path = cache_dir
        .join(FETCH_ICONS)
        .join(if let Some(ext) = extension {
            format!("{hash}.{ext}")
        } else {
            hash
        });

    write(&path, &bytes, semaphore).await?;

    let path = io::canonicalize(path)?;
    Ok(path)
}

pub async fn sha1_async(bytes: Bytes) -> crate::Result<String> {
    let hash = tokio::task::spawn_blocking(move || {
        sha1_smol::Sha1::from(bytes).hexdigest()
    })
    .await?;

    Ok(hash)
}

pub async fn sha1_file_async(
    path: impl AsRef<Path>,
) -> crate::Result<(u64, String)> {
    let path = path.as_ref();
    #[cfg(test)]
    super::download::verified_file::record_scan();
    let started = Instant::now();
    let _permit = acquire_native_validation_permit().await?;
    // Local files can be multi-gigabyte .mrpacks, so hash them without materializing bytes.
    let mut file = File::open(path)
        .await
        .map_err(|e| IOError::with_path(e, path))?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; DOWNLOAD_BUFFER_BYTES];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .map_err(|e| IOError::with_path(e, path))?;
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }

    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed SHA-1 file verification");
    Ok((size, hasher.digest().to_string()))
}

pub async fn sha1_file_cancellable(
    path: impl AsRef<Path>,
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<(u64, String)> {
    let path = path.as_ref();
    #[cfg(test)]
    super::download::verified_file::record_scan();
    let started = Instant::now();
    let _permit = tokio::select! { biased; _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("verification canceled while waiting for resources".into()).into()), permit = acquire_native_validation_permit() => permit? };
    let mut file = File::open(path)
        .await
        .map_err(|e| IOError::with_path(e, path))?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; DOWNLOAD_BUFFER_BYTES];
    loop {
        let bytes_read = tokio::select! {
            _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("download canceled during verification".to_string()).into()),
            result = file.read(&mut buffer) => result.map_err(|e| IOError::with_path(e, path))?,
        };
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed cancellable SHA-1 file verification");
    Ok((size, hasher.digest().to_string()))
}
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
    'attempt: {
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
            ErrorKind::NetworkError(
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
            send_path_request(
                &attempt_route,
                request.header.as_ref(),
                credentials,
                request.download_meta.as_ref(),
                (resume_offset > 0).then_some(resume_offset),
                None,
            ),
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
                super::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
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
        let mut file =
            super::download::stream_output::StreamOutput::new(file, part_path);
        let response_length = response.content_length().unwrap_or(0);
        let endpoint_feedback = response.extensions().get::<super::download::endpoint_selection::metrics::EndpointTransferGuard>().cloned();
        let total_size = request
            .integrity
            .size
            .unwrap_or(starting_size.saturating_add(response_length));
        let transfer_started = Instant::now();
        let mut downloaded = starting_size;
        let mut progress_gate =
            super::download::h2_receive::H2ProgressGate::with_starting_bytes(
                total_size,
                starting_size,
            );
        let expected_speed = expected_route_speed(route, request.resource);
        let mut slow_policy = if downgrade_slow_http2 {
            super::download::native_slow::NativeSlowPolicy::for_http2(
                starting_size,
                expected_speed,
            )
        } else {
            super::download::native_slow::NativeSlowPolicy::new(
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
        loop {
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
                    if let Some(feedback) = &endpoint_feedback { feedback.record(chunk.len(), super::download::local_resources::pressure(part_path)); }
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
                    let slow_decision = slow_policy.observe_with_pressure(
                        downloaded,
                        total_size.saturating_sub(downloaded),
                        super::download::local_resources::pressure(part_path),
                    );
                    if let super::download::native_slow::SlowDecision::Probe { bytes_per_second } = slow_decision {
                        last_endpoint_speed = bytes_per_second;
                    }
                    if !super::download::local_resources::pressure(part_path).blocks_expansion()
                        && let Some(clients) = super::download::proxy_context::clients() {
                        let protocol = if http_version == reqwest::Version::HTTP_2 {
                            super::download::endpoint_selection::Protocol::H2
                        } else { super::download::endpoint_selection::Protocol::Http1 };
                        if clients.endpoints.recovery_lease(request, route, protocol, remote_addr,
                            last_endpoint_speed, total_size.saturating_sub(downloaded)).is_some() {
                            switched_endpoint = true;
                            break;
                        }
                    }
                    if matches!(slow_decision, super::download::native_slow::SlowDecision::Probe { .. })
                        && let Some(clients) = super::download::proxy_context::clients()
                    {
                        let protocol = if http_version == reqwest::Version::HTTP_2 {
                            super::download::endpoint_selection::Protocol::H2
                        } else { super::download::endpoint_selection::Protocol::Http1 };
                        clients.endpoints.on_slow_region(request, route, &clients, protocol,
                            remote_addr.map(|address| address.ip()), downloaded..total_size, Some(last_endpoint_speed),
                            super::download::local_resources::pressure(part_path));
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
            let _ = super::download::local_resources::write(
                part_path,
                0,
                file.sync_data(),
            )
            .await;
        }
        drop(file);
        drop(permit);
        drop(activity.take());

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
                super::download::proxy_context::resolver(route.proxy).resolved_addresses(&host)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn post_verification_path_replacement_never_receives_a_trusted_proof()
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("staged");
        tokio::fs::write(&path, b"original").await.unwrap();
        let integrity =
            Integrity::sha1(sha1_smol::Sha1::from(b"original").hexdigest());
        verify_file(&path, &integrity).await.unwrap();
        let replacement = directory.path().join("replacement");
        tokio::fs::write(&replacement, b"modified").await.unwrap();
        finalize_download(&replacement, &path).await.unwrap();
        let result = DownloadResult {
            path: path.clone(),
            url: "https://proof.invalid/file".into(),
            source: DownloadRouteSource::Official,
            size: 8,
            attempts: 1,
            fallback_count: 0,
            verified_sha1: None,
            verified_sha512: None,
            verified_file: None,
        };
        let result = attach_verified_integrity(result, &integrity).await;
        assert!(result.verified_file.is_none());
        assert!(verify_file(&path, &integrity).await.is_err());
    }

    #[tokio::test]
    async fn failed_checkpoint_ranges_restart_full_download_but_io_and_cancel_keep_checkpoints()
     {
        use super::super::download::h2_download::{
            H2DownloadFailure, H2DownloadOutcome,
        };
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("file.part");
        let destination = directory.path().join("file");
        let journal = super::super::download::range_journal::path(&part);
        let request = DownloadRequest::new(
            "https://resume.invalid/file",
            ResourceClass::Other,
        )
        .with_integrity(Integrity::sha1("expected"));
        let route = official_route(&request.url, ResourceClass::Other);
        for failure in [
            H2DownloadFailure::RangeUnsupported,
            H2DownloadFailure::Protocol,
            H2DownloadFailure::Io,
        ] {
            tokio::fs::write(&part, b"sparse-part").await.unwrap();
            tokio::fs::write(&journal, b"checkpoint").await.unwrap();
            let result = resume_checkpointed_download_with(
                &request,
                &[route.clone()],
                &part,
                |_| async {
                    Ok(Some(H2DownloadOutcome::Fallback {
                        failure,
                        preserve_partial: failure
                            != H2DownloadFailure::RangeUnsupported,
                    }))
                },
            )
            .await;
            if failure == H2DownloadFailure::Io {
                assert!(result.is_err());
                assert!(part.exists() && journal.exists());
            } else {
                assert!(result.unwrap().is_none());
                assert!(!part.exists() && !journal.exists());
                tokio::fs::write(&part, b"complete full response")
                    .await
                    .unwrap();
                finalize_download(&part, &destination).await.unwrap();
                assert_eq!(
                    tokio::fs::read(&destination).await.unwrap(),
                    b"complete full response"
                );
            }
        }
        assert!(
            resume_checkpointed_download_with(
                &request,
                &[route],
                &part,
                |_| async { Ok(Some(H2DownloadOutcome::Canceled)) }
            )
            .await
            .is_err()
        );
        assert!(part.exists() && journal.exists());
    }
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    static RANGE_SPLITTING_TEST_LOCK: LazyLock<AsyncMutex<()>> =
        LazyLock::new(|| AsyncMutex::new(()));
    static AUTO_SOURCE_TEST_LOCK: LazyLock<std::sync::Mutex<()>> =
        LazyLock::new(|| std::sync::Mutex::new(()));
    static H2_FALLBACK_TEST_LOCK: LazyLock<std::sync::Mutex<()>> =
        LazyLock::new(|| std::sync::Mutex::new(()));

    #[derive(Clone, Copy, Debug, Default)]
    struct RangeServerBehavior {
        wrong_content_range: bool,
        ignore_range: bool,
        slow_body: bool,
        fail_first_range: bool,
        stall_first_range: bool,
    }

    async fn spawn_range_server(
        data: Arc<Vec<u8>>,
        behavior: RangeServerBehavior,
    ) -> (
        String,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let request_count = requests.clone();
        let normal_requests = Arc::new(AtomicUsize::new(0));
        let normal_request_count = normal_requests.clone();
        let failed_range = Arc::new(AtomicBool::new(false));
        let stalled_range = Arc::new(AtomicBool::new(false));
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let data = data.clone();
                let requests = request_count.clone();
                let normal_requests = normal_request_count.clone();
                let failed_range = Arc::clone(&failed_range);
                let stalled_range = Arc::clone(&stalled_range);
                tokio::spawn(async move {
                    requests.fetch_add(1, Ordering::Relaxed);
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let Ok(read) = stream.read(&mut buffer).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request
                            .windows(4)
                            .any(|window| window == b"\r\n\r\n")
                        {
                            break;
                        }
                    }
                    let request =
                        String::from_utf8_lossy(&request).to_ascii_lowercase();
                    let requested_range = request
                        .lines()
                        .find_map(|line| line.strip_prefix("range: bytes="));
                    let (headers, body) = if let Some(range) =
                        requested_range.filter(|_| !behavior.ignore_range)
                    {
                        let Some((start, end)) = range.split_once('-') else {
                            return;
                        };
                        let Ok(start) = start.parse::<u64>() else {
                            return;
                        };
                        let end = if end.is_empty() {
                            data.len() as u64 - 1
                        } else {
                            let Ok(end) = end.parse::<u64>() else {
                                return;
                            };
                            end
                        };
                        let body = &data[start as usize..=end as usize];
                        let reported_start = if behavior.wrong_content_range {
                            start.saturating_add(1)
                        } else {
                            start
                        };
                        (
                            format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {reported_start}-{end}/{}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
                                body.len(),
                                data.len(),
                            ),
                            body,
                        )
                    } else {
                        if requested_range.is_none() {
                            normal_requests.fetch_add(1, Ordering::Relaxed);
                        }
                        (
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
                                data.len(),
                            ),
                            &data[..],
                        )
                    };
                    if stream.write_all(headers.as_bytes()).await.is_err() {
                        return;
                    }
                    if requested_range.is_some()
                        && behavior.stall_first_range
                        && !stalled_range.swap(true, Ordering::Relaxed)
                    {
                        tokio::time::sleep(DOWNLOAD_SCHEDULING_GRACE).await;
                        return;
                    }
                    if requested_range.is_some()
                        && behavior.fail_first_range
                        && !failed_range.swap(true, Ordering::Relaxed)
                    {
                        let midpoint = body.len() / 2;
                        let _ = stream.write_all(&body[..midpoint]).await;
                        return;
                    }
                    for chunk in body.chunks(64 * 1024) {
                        if stream.write_all(chunk).await.is_err() {
                            return;
                        }
                        if behavior.slow_body {
                            tokio::time::sleep(time::Duration::from_millis(
                                100,
                            ))
                            .await;
                        }
                    }
                });
            }
        });
        (
            format!("http://{address}/file"),
            requests,
            normal_requests,
            handle,
        )
    }

    async fn spawn_stream_server(
        chunks: Vec<(Duration, Vec<u8>)>,
        content_length: usize,
        hold_open: bool,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let chunks = Arc::new(chunks);
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let chunks = Arc::clone(&chunks);
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let Ok(read) = stream.read(&mut buffer).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request
                            .windows(4)
                            .any(|window| window == b"\r\n\r\n")
                        {
                            break;
                        }
                    }
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                    );
                    if stream.write_all(headers.as_bytes()).await.is_err() {
                        return;
                    }
                    for (delay, chunk) in chunks.iter() {
                        tokio::time::sleep(*delay).await;
                        if stream.write_all(chunk).await.is_err() {
                            return;
                        }
                    }
                    if hold_open {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                });
            }
        });
        (format!("http://{address}/file"), handle)
    }

    async fn spawn_json_server(
        body: String,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let request_count = Arc::clone(&requests);
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let requests = Arc::clone(&request_count);
                let body = body.clone();
                tokio::spawn(async move {
                    requests.fetch_add(1, Ordering::Relaxed);
                    let mut buffer = [0_u8; 1024];
                    let _ = stream.read(&mut buffer).await;
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(headers.as_bytes()).await;
                    let _ = stream.write_all(body.as_bytes()).await;
                });
            }
        });
        (
            format!("http://{address}/tag/game_version"),
            requests,
            handle,
        )
    }

    struct HttpFixture {
        status_line: &'static str,
        extra_headers: &'static str,
        body: Vec<u8>,
        response_delay: Duration,
    }

    impl HttpFixture {
        fn new(status_line: &'static str, body: impl Into<Vec<u8>>) -> Self {
            Self {
                status_line,
                extra_headers: "",
                body: body.into(),
                response_delay: Duration::ZERO,
            }
        }

        fn with_headers(mut self, headers: &'static str) -> Self {
            self.extra_headers = headers;
            self
        }

        fn with_delay(mut self, delay: Duration) -> Self {
            self.response_delay = delay;
            self
        }
    }

    async fn spawn_http_fixture(
        fixture: HttpFixture,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let request_count = Arc::clone(&requests);
        let status_line = fixture.status_line.to_string();
        let extra_headers = fixture.extra_headers.to_string();
        let body = Arc::new(fixture.body);
        let response_delay = fixture.response_delay;
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let requests = Arc::clone(&request_count);
                let status_line = status_line.clone();
                let extra_headers = extra_headers.clone();
                let body = Arc::clone(&body);
                tokio::spawn(async move {
                    requests.fetch_add(1, Ordering::Relaxed);
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let Ok(read) = stream.read(&mut buffer).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request
                            .windows(4)
                            .any(|window| window == b"\r\n\r\n")
                        {
                            break;
                        }
                    }
                    tokio::time::sleep(response_delay).await;
                    let response = format!(
                        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n",
                        body.len(),
                    );
                    if stream.write_all(response.as_bytes()).await.is_ok() {
                        let _ = stream.write_all(&body).await;
                    }
                });
            }
        });
        (format!("http://{address}/file"), requests, handle)
    }

    fn direct_test_route(
        url: String,
        source: DownloadRouteSource,
    ) -> DownloadRoute {
        DownloadRoute {
            url,
            source,
            is_mirror: source != DownloadRouteSource::Official,
            allow_sensitive_headers: source == DownloadRouteSource::Official,
            supports_range: false,
            proxy: ProxyPolicy::Direct,
        }
    }

    async fn spawn_redirect_server(
        location: String,
        response_delay: Duration,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let request_count = Arc::clone(&requests);
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let location = location.clone();
                let requests = Arc::clone(&request_count);
                tokio::spawn(async move {
                    requests.fetch_add(1, Ordering::Relaxed);
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let Ok(read) = stream.read(&mut buffer).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request
                            .windows(4)
                            .any(|window| window == b"\r\n\r\n")
                        {
                            break;
                        }
                    }
                    tokio::time::sleep(response_delay).await;
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        (format!("http://{address}/redirect"), requests, handle)
    }

    async fn test_download(
        url: &str,
        destination: &Path,
        expected_size: u64,
    ) -> crate::Result<DownloadResult> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        download_to_path(
            DownloadRequest::new(url, ResourceClass::Other)
                .with_integrity(Integrity::default().with_size(expected_size)),
            destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
    }
    #[test]
    fn modrinth_requests_are_classified_for_logging() {
        assert_eq!(
            modrinth_request_kind("https://api.modrinth.com/v2/project"),
            Some("API")
        );
        assert_eq!(
            modrinth_request_kind(
                "https://cdn.modrinth.com/data/project/version/file.jar"
            ),
            Some("CDN")
        );
        assert_eq!(
            modrinth_request_kind(
                "https://cdn-alt.modrinth.com/data/project/version/file.jar"
            ),
            None
        );
        assert_eq!(modrinth_request_kind("https://example.com/file.jar"), None);
    }

    #[test]
    fn modrinth_cdn_routes_use_the_current_host() {
        for host in ["cdn.modrinth.com", "cdn-alt.modrinth.com"] {
            let url = format!(
                "https://{host}/data/project/version/file.jar?download=1"
            );
            let routes = resolve_download_routes_for(
                &url,
                ResourceClass::Modrinth,
                crate::state::DownloadSourceMode::OfficialOnly,
            );
            assert_eq!(routes.len(), 1);
            assert_eq!(
                routes[0].url,
                "https://cdn.modrinth.com/data/project/version/file.jar?download=1"
            );
        }
    }

    #[test]
    fn modrinth_download_url_recognition_excludes_retired_hosts() {
        assert!(!is_official_modrinth_download_url(
            "https://cdn-alt.modrinth.com/data/project/version/file.jar"
        ));
        assert!(is_official_modrinth_download_url(
            "https://cdn.modrinth.com/data/project/version/file.jar"
        ));
        assert!(is_official_modrinth_download_url(
            "https://api.modrinth.com/v2/project/abc"
        ));
        assert!(!is_official_modrinth_download_url(
            "https://example.com/file.jar"
        ));
    }

    #[test]
    fn cache_busted_urls_preserve_existing_query() {
        assert_eq!(
            cache_busted_download_url(
                "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1",
                2,
            ),
            "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1&axolotl_retry=2"
        );
        assert_eq!(
            cache_busted_download_url(
                "https://cdn-alt.modrinth.com/data/project/version/file.jar",
                1,
            ),
            "https://cdn-alt.modrinth.com/data/project/version/file.jar?axolotl_retry=1"
        );
        // A previous buster is replaced, not stacked.
        assert_eq!(
            cache_busted_download_url(
                "https://cdn-alt.modrinth.com/data/project/version/file.jar?axolotl_retry=1&download=1",
                3,
            ),
            "https://cdn-alt.modrinth.com/data/project/version/file.jar?download=1&axolotl_retry=3"
        );
    }

    #[test]
    fn retired_modrinth_candidates_are_deduplicated_in_every_source_mode() {
        for mode in [
            crate::state::DownloadSourceMode::Auto,
            crate::state::DownloadSourceMode::OfficialOnly,
            crate::state::DownloadSourceMode::OfficialPreferred,
            crate::state::DownloadSourceMode::MirrorPreferred,
        ] {
            let request = DownloadRequest::new(
                "https://cdn-alt.modrinth.com/data/pack.mrpack",
                ResourceClass::Modpack,
            )
            .with_candidate_urls([
                "https://cdn.modrinth.com/data/pack.mrpack",
                "https://cdn-alt.modrinth.com/data/other.jar?x=%2b",
            ]);
            let routes = build_download_routes(&request, mode);
            assert_eq!(routes.len(), 2);
            assert_eq!(
                routes[0].url,
                "https://cdn.modrinth.com/data/pack.mrpack"
            );
            assert_eq!(
                routes[1].url,
                "https://cdn.modrinth.com/data/other.jar?x=%2b"
            );
            assert!(!routes.iter().any(|route| route.url.contains("cdn-alt")));
            assert_eq!(routes[0].source, DownloadRouteSource::Official);
            assert!(is_official_modrinth_download_url(&routes[0].url));
            assert_eq!(modrinth_request_kind(&routes[0].url), Some("CDN"));
            assert_eq!(
                infer_resource_class(&routes[0].url),
                ResourceClass::Modrinth
            );
        }
        assert_eq!(
            modrinth_request_kind(
                "https://cdn.modrinth.com.evil.example/file.jar"
            ),
            None
        );
    }

    #[test]
    fn size_mismatch_is_lenient_when_a_hash_matches() {
        let matching = ComputedIntegrity {
            size: 122_777,
            sha1: Some("abc".to_string()),
            ..ComputedIntegrity::default()
        };
        let hashed = Integrity {
            size: Some(122_778),
            sha1: Some("abc".to_string()),
            ..Integrity::default()
        };
        assert!(
            verify_computed_integrity(&hashed, &matching).is_ok(),
            "a hash match must win over an off-by-a-few-bytes size claim"
        );
        let size_only = Integrity {
            size: Some(122_778),
            ..Integrity::default()
        };
        assert!(
            verify_computed_integrity(&size_only, &matching).is_err(),
            "without a hash the size claim is still enforced"
        );
    }

    #[test]
    fn identifies_hash_integrity_failures_without_treating_size_errors_as_hash_failures()
     {
        let hash_error: crate::Error = ErrorKind::OtherError(
            "Incorrect sha1 hash for download: expected != actual".to_string(),
        )
        .into();
        assert!(is_integrity_error(&hash_error));

        let typed_hash_error: crate::Error =
            ErrorKind::HashError("expected".to_string(), "actual".to_string())
                .into();
        assert!(is_integrity_error(&typed_hash_error));

        let size_error: crate::Error = ErrorKind::OtherError(
            "Incorrect size for download: 10 != 20".to_string(),
        )
        .into();
        assert!(!is_integrity_error(&size_error));
    }

    #[test]
    fn segmented_download_flag_is_buildable_per_request() {
        let request = DownloadRequest::new(
            "https://example.com/a.jar",
            ResourceClass::Modpack,
        );
        assert!(request.allow_segmented_download);
        let request = request.with_segmented_download(false);
        assert!(!request.allow_segmented_download);
    }

    #[test]
    fn batch_requests_can_disable_http1_ranges_without_disabling_h2() {
        let request = DownloadRequest::new(
            "https://example.com/content.jar",
            ResourceClass::Modpack,
        )
        .with_http1_segmented_download(false);

        assert!(request.allow_segmented_download);
        assert!(!request.allow_http1_segmented_download);
    }

    #[test]
    fn h2_range_concurrency_is_explicit_per_request() {
        let request = DownloadRequest::new(
            "https://example.com/pack.mrpack",
            ResourceClass::Modpack,
        );
        assert_eq!(request.h2_range_concurrency, None);
        assert_eq!(
            request.with_h2_range_concurrency(8).h2_range_concurrency,
            Some(8)
        );
    }

    #[tokio::test]
    async fn file_download_times_out_when_a_successful_response_stalls() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let (url, server) = spawn_stream_server(Vec::new(), 4, true).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("stalled.bin");

        let error = test_download(&url, &destination, 4).await.unwrap_err();

        assert!(matches!(
            error.raw.as_ref(),
            ErrorKind::NetworkError(_) | ErrorKind::FetchError(_)
        ));
        assert!(!destination.exists());
        assert!(!suffixed_path(&destination, ".part").exists());
        server.abort();
    }

    #[tokio::test]
    async fn file_download_allows_data_that_keeps_arriving_slowly() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let chunks = (0..4)
            .map(|byte| (Duration::from_millis(100), vec![byte]))
            .collect();
        let (url, server) = spawn_stream_server(chunks, 4, false).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("slow.bin");

        let result = test_download(&url, &destination, 4).await.unwrap();

        assert_eq!(result.size, 4);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), [0, 1, 2, 3]);
        server.abort();
    }

    #[tokio::test]
    async fn file_download_completes_normally() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let (url, server) = spawn_stream_server(
            vec![(Duration::ZERO, b"done".to_vec())],
            4,
            false,
        )
        .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("complete.bin");

        let result = test_download(&url, &destination, 4).await.unwrap();

        assert_eq!(result.size, 4);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"done");
        server.abort();
    }

    #[tokio::test]
    async fn segmentation_disabled_downloads_use_a_single_connection() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let (url, requests, normal_requests, server) =
            spawn_range_server(data, RangeServerBehavior::default()).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("batch.bin");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();

        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_segmented_download(false)
                .with_integrity(Integrity::default().with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, size as u64);
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[tokio::test]
    async fn file_download_drops_a_missing_route_before_later_rounds() {
        let (missing_url, missing_requests, missing_server) =
            spawn_http_fixture(HttpFixture::new("404 Not Found", Vec::new()))
                .await;
        let (fallback_url, fallback_requests, fallback_server) =
            spawn_http_fixture(HttpFixture::new("200 OK", b"done")).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("fallback.bin");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();

        let result = download_to_path(
            DownloadRequest::new(&missing_url, ResourceClass::Other)
                .with_candidate_urls([fallback_url])
                .with_integrity(Integrity::default().with_size(4)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, 4);
        assert_eq!(missing_requests.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_requests.load(Ordering::Relaxed), 1);
        missing_server.abort();
        fallback_server.abort();
    }

    #[tokio::test]
    async fn file_download_bounds_server_error_retries_to_three_rounds() {
        let (url, requests, server) = spawn_http_fixture(HttpFixture::new(
            "503 Service Unavailable",
            Vec::new(),
        ))
        .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("unavailable.bin");
        let request_url = format!("{url}?token=secret#fragment");

        let error = test_download(&request_url, &destination, 4)
            .await
            .unwrap_err();
        let diagnostic = error.to_string();

        assert_eq!(requests.load(Ordering::Relaxed), 3);
        assert!(diagnostic.contains("after 3/3 attempts"));
        assert!(diagnostic.contains("category=http"));
        assert!(diagnostic.contains("status=Some(503)"));
        assert!(!diagnostic.contains("secret"));
        assert!(!diagnostic.contains("fragment"));
        assert!(!destination.exists());
        server.abort();
    }

    #[tokio::test]
    async fn rate_limited_route_cools_down_and_switches_without_waiting() {
        let (limited_url, limited_requests, limited_server) =
            spawn_http_fixture(
                HttpFixture::new("429 Too Many Requests", Vec::new())
                    .with_headers("Retry-After: 60\r\n"),
            )
            .await;
        let (fallback_url, fallback_requests, fallback_server) =
            spawn_http_fixture(HttpFixture::new("200 OK", b"done")).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("rate-limit-fallback.bin");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let started = Instant::now();

        let result = download_to_path(
            DownloadRequest::new(&limited_url, ResourceClass::Other)
                .with_candidate_urls([fallback_url])
                .with_integrity(Integrity::default().with_size(4)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, 4);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(limited_requests.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_requests.load(Ordering::Relaxed), 1);
        limited_server.abort();
        fallback_server.abort();
    }

    #[tokio::test]
    async fn metadata_hedging_returns_the_first_valid_response() {
        let (primary_url, primary_requests, primary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", br#"{"source":"primary"}"#)
                    .with_headers("Content-Type: application/json\r\n")
                    .with_delay(Duration::from_millis(300)),
            )
            .await;
        let (secondary_url, secondary_requests, secondary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", br#"{"source":"secondary"}"#)
                    .with_headers("Content-Type: application/json\r\n"),
            )
            .await;
        let routes = [
            direct_test_route(primary_url, DownloadRouteSource::Official),
            direct_test_route(secondary_url, DownloadRouteSource::Mcim),
        ];
        let validate = |bytes: &Bytes| -> crate::Result<()> {
            serde_json::from_slice::<serde_json::Value>(bytes)?;
            Ok(())
        };

        let bytes = fetch_hedged_metadata(
            &routes,
            ResourceClass::Metadata,
            None,
            None,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &INSECURE_REQWEST_CLIENT,
            &validate,
        )
        .await
        .unwrap();

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            serde_json::json!({ "source": "secondary" })
        );
        assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
        assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
        primary_server.abort();
        secondary_server.abort();
    }

    #[tokio::test]
    async fn metadata_hedging_rejects_a_fast_invalid_primary() {
        let (primary_url, primary_requests, primary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", b"not-json")
                    .with_headers("Content-Type: application/json\r\n"),
            )
            .await;
        let (secondary_url, secondary_requests, secondary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", br#"{"valid":true}"#)
                    .with_headers("Content-Type: application/json\r\n"),
            )
            .await;
        let routes = [
            direct_test_route(primary_url, DownloadRouteSource::Official),
            direct_test_route(secondary_url, DownloadRouteSource::Mcim),
        ];
        let validate = |bytes: &Bytes| -> crate::Result<()> {
            serde_json::from_slice::<serde_json::Value>(bytes)?;
            Ok(())
        };

        let bytes = fetch_hedged_metadata(
            &routes,
            ResourceClass::Metadata,
            None,
            None,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &INSECURE_REQWEST_CLIENT,
            &validate,
        )
        .await
        .unwrap();

        assert_eq!(bytes, Bytes::from_static(br#"{"valid":true}"#));
        assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
        assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
        primary_server.abort();
        secondary_server.abort();
    }

    #[tokio::test]
    async fn metadata_hedging_does_not_start_a_loser_for_a_fast_primary() {
        let (primary_url, primary_requests, primary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", br#"{"valid":true}"#)
                    .with_headers("Content-Type: application/json\r\n"),
            )
            .await;
        let (secondary_url, secondary_requests, secondary_server) =
            spawn_http_fixture(
                HttpFixture::new("200 OK", br#"{"valid":false}"#)
                    .with_headers("Content-Type: application/json\r\n"),
            )
            .await;
        let routes = [
            direct_test_route(primary_url, DownloadRouteSource::Official),
            direct_test_route(secondary_url, DownloadRouteSource::Mcim),
        ];
        let validate = |bytes: &Bytes| -> crate::Result<()> {
            serde_json::from_slice::<serde_json::Value>(bytes)?;
            Ok(())
        };

        let _ = fetch_hedged_metadata(
            &routes,
            ResourceClass::Metadata,
            None,
            None,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &INSECURE_REQWEST_CLIENT,
            &validate,
        )
        .await
        .unwrap();
        tokio::time::sleep(METADATA_HEDGE_DELAY * 2).await;

        assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
        assert_eq!(secondary_requests.load(Ordering::Relaxed), 0);
        primary_server.abort();
        secondary_server.abort();
    }

    #[tokio::test]
    async fn xml_metadata_hedging_bypasses_slow_or_invalid_official_responses()
    {
        #[derive(Deserialize)]
        struct XmlVersion {
            version: String,
        }
        for (body, delay) in [
            (
                b"<metadata><version>old</version></metadata>".as_slice(),
                Duration::from_secs(2),
            ),
            (b"<html>upstream error</html>".as_slice(), Duration::ZERO),
        ] {
            let (primary_url, primary_requests, primary_server) =
                spawn_http_fixture(
                    HttpFixture::new("200 OK", body).with_delay(delay),
                )
                .await;
            let valid = b"<metadata><version>21.1.200</version></metadata>";
            let (secondary_url, secondary_requests, secondary_server) =
                spawn_http_fixture(HttpFixture::new(
                    "200 OK",
                    valid.as_slice(),
                ))
                .await;
            let routes = [
                direct_test_route(primary_url, DownloadRouteSource::Official),
                direct_test_route(secondary_url, DownloadRouteSource::Bmclapi),
            ];
            let validate = |bytes: &Bytes| -> crate::Result<()> {
                quick_xml::de::from_reader::<_, XmlVersion>(bytes.as_ref())
                    .map(|_| ())
                    .map_err(|error| {
                        ErrorKind::OtherError(error.to_string()).as_error()
                    })
            };
            let bytes = tokio::time::timeout(
                Duration::from_secs(1),
                fetch_hedged_metadata(
                    &routes,
                    ResourceClass::Loader,
                    None,
                    None,
                    &FetchSemaphore(Semaphore::new(2)),
                    &INSECURE_REQWEST_CLIENT,
                    &validate,
                ),
            )
            .await
            .unwrap()
            .unwrap();
            let parsed: XmlVersion =
                quick_xml::de::from_reader(bytes.as_ref()).unwrap();
            assert_eq!(parsed.version, "21.1.200");
            assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
            assert_eq!(secondary_requests.load(Ordering::Relaxed), 1);
            primary_server.abort();
            secondary_server.abort();
        }
    }

    #[tokio::test]
    async fn typed_metadata_fetch_validates_xml_without_initializing_application_state()
     {
        #[derive(Deserialize)]
        struct XmlVersion {
            version: String,
        }
        let (url, requests, server) = spawn_http_fixture(HttpFixture::new(
            "200 OK",
            b"<metadata><version>21.1.200</version></metadata>".as_slice(),
        ))
        .await;
        let clients = DownloadClients::build(
            &crate::util::proxy::ProxyConfig {
                mode: crate::util::proxy::ProxyMode::None,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let parsed = super::super::download::proxy_context::with_snapshot(
            clients,
            fetch_metadata(
                &url,
                |bytes| {
                    quick_xml::de::from_reader::<_, XmlVersion>(bytes.as_ref())
                        .map_err(|error| {
                            ErrorKind::OtherError(error.to_string()).as_error()
                        })
                },
                &FetchSemaphore(Semaphore::new(2)),
                &pool,
            ),
        )
        .await
        .unwrap();
        assert_eq!(parsed.version, "21.1.200");
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[tokio::test]
    async fn canceling_a_file_download_drops_the_response_read_promptly() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let (url, server) = spawn_stream_server(Vec::new(), 4, true).await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("canceled.bin");
        let task =
            tokio::spawn(
                async move { test_download(&url, &destination, 4).await },
            );

        tokio::time::sleep(Duration::from_millis(50)).await;
        task.abort();
        let join_error = tokio::time::timeout(Duration::from_millis(100), task)
            .await
            .expect("canceled download should stop promptly")
            .unwrap_err();

        assert!(join_error.is_cancelled());
        server.abort();
    }

    #[test]
    fn log_urls_keep_route_but_remove_credentials_and_fragments() {
        assert_eq!(
            sanitize_url_for_log(
                "https://mod.mcimirror.top/data/file.jar?X-Amz-Credential=secret&X-Amz-Signature=signature#fragment"
            ),
            "https://mod.mcimirror.top/data/file.jar"
        );
        assert_eq!(
            sanitize_url_for_log("not-a-url?token=secret#fragment"),
            "not-a-url"
        );
        assert_eq!(
            sanitize_url_for_log(
                "https://username:password@example.com/file.jar?token=secret"
            ),
            "https://example.com/file.jar"
        );
    }

    #[test]
    fn auto_source_health_is_scoped_by_resource_family() {
        let _guard = AUTO_SOURCE_TEST_LOCK.lock().unwrap();
        let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
        let manifest_url =
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
        let manifest_routes = resolve_download_routes_for(
            manifest_url,
            ResourceClass::Metadata,
            crate::state::DownloadSourceMode::Auto,
        );
        assert_eq!(manifest_routes[0].source, DownloadRouteSource::Official);

        record_route_failure(
            &manifest_routes[0],
            ResourceClass::Metadata,
            None,
        );
        let modrinth_routes = resolve_download_routes_for(
            "https://api.modrinth.com/v2/tag/game_version",
            ResourceClass::Modrinth,
            crate::state::DownloadSourceMode::Auto,
        );
        assert_eq!(modrinth_routes[0].source, DownloadRouteSource::Official);

        *ROUTE_HEALTH.lock() = previous_health;
    }

    #[test]
    fn disabled_tianpao_routes_leave_official_cdn_available() {
        let _guard = AUTO_SOURCE_TEST_LOCK.lock().unwrap();
        let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
        let url = "https://cdn-alt.modrinth.com/data/project/versions/version/file.jar";
        let mut routes = explicit_mirror_routes(url, ResourceClass::Modrinth);
        routes.push(official_route(url, ResourceClass::Modrinth));
        assert!(routes.iter().all(|route| {
            !Url::parse(&route.url)
                .is_ok_and(|url| retired_sources::is_retired(&url))
        }));
        let official = routes
            .iter()
            .find(|route| is_official_route(route))
            .unwrap();
        let key = route_health_key(official, ResourceClass::Modrinth).unwrap();
        ROUTE_HEALTH.lock().insert(
            key,
            RouteHealth {
                success_samples: 3,
                throughput_bps: Some(100_000_000.0),
                ..RouteHealth::default()
            },
        );

        order_auto_routes(&mut routes, ResourceClass::Modrinth);

        assert_eq!(routes[0].source, DownloadRouteSource::Official);
        *ROUTE_HEALTH.lock() = previous_health;
    }

    #[test]
    fn tianpao_routes_restore_official_sources_in_all_modes_and_skip_unknown_candidates()
     {
        for mode in [
            crate::state::DownloadSourceMode::Auto,
            crate::state::DownloadSourceMode::OfficialOnly,
            crate::state::DownloadSourceMode::OfficialPreferred,
            crate::state::DownloadSourceMode::MirrorPreferred,
        ] {
            for (input, official) in [
                (
                    "https://mod.tianpao.top/data/p/file.jar",
                    "https://cdn.modrinth.com/data/p/file.jar",
                ),
                (
                    "https://mod.tianpao.top/files/1/2/file.jar",
                    "https://edge.forgecdn.net/files/1/2/file.jar",
                ),
                (
                    "https://mod.tianpao.top/media/attachments/icon.png",
                    "https://media.forgecdn.net/attachments/icon.png",
                ),
            ] {
                let request =
                    DownloadRequest::new(input, ResourceClass::Modpack)
                        .with_candidate_urls([
                            official,
                            "https://mod.tianpao.top/unknown",
                        ]);
                let routes = build_download_routes(&request, mode);
                assert_eq!(routes.len(), 1);
                assert_eq!(routes[0].url, official);
                assert_eq!(routes[0].source, DownloadRouteSource::Official);
            }
            assert!(
                resolve_download_routes_for(
                    "https://mod.tianpao.top/unknown",
                    ResourceClass::Modpack,
                    mode
                )
                .is_empty()
            );
            let request = DownloadRequest::new(
                "https://mod.tianpao.top/unknown",
                ResourceClass::Modpack,
            )
            .with_candidate_urls(["https://cdn.modrinth.com/data/file.jar"]);
            assert_eq!(
                build_download_routes(&request, mode)[0].url,
                "https://cdn.modrinth.com/data/file.jar"
            );
        }
    }

    #[tokio::test]
    async fn tianpao_unknown_downloads_preserve_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.bin");
        tokio::fs::write(&path, b"existing").await.unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let error = download_to_path(
            DownloadRequest::new(
                "https://mod.tianpao.top/unknown",
                ResourceClass::Modpack,
            ),
            &path,
            &FetchSemaphore(Semaphore::new(1)),
            &pool,
            None,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("retired"));
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"existing");
    }

    #[tokio::test]
    async fn tianpao_metadata_redirects_are_blocked_before_following() {
        let (url, requests, server) = spawn_http_fixture(
            HttpFixture::new("302 Found", Vec::new())
                .with_headers("Location: https://mod.tianpao.top/unknown\r\n"),
        )
        .await;
        let client = reqwest_client_builder().no_proxy().build().unwrap();
        let error = client.get(url).send().await.unwrap_err();
        assert!(error.is_redirect());
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[test]
    fn modrinth_cdn_redirects_only_fall_back_to_official_cdn() {
        assert!(is_official_modrinth_cdn_redirect(Some(
            "https://cdn.modrinth.com/data/project/versions/version/file.jar"
        )));
        assert!(is_official_modrinth_cdn_redirect(Some(
            "https://cdn-alt.modrinth.com/data/project/versions/version/file.jar"
        )));
        assert!(is_official_modrinth_cdn_redirect(Some(
            "https://CDN.MODRINTH.COM/data/project/versions/version/file.jar"
        )));
        assert!(!is_official_modrinth_cdn_redirect(Some(
            "https://cache.mcimirror.top/data/project/versions/version/file.jar"
        )));
        assert!(!is_official_modrinth_cdn_redirect(Some(
            "https://cdn.modrinth.com.evil.example/file.jar"
        )));
        assert!(!is_official_modrinth_cdn_redirect(Some(
            "https://cdn.modrinth.com@evil.example/file.jar"
        )));
        assert!(!is_official_modrinth_cdn_redirect(Some(
            "https://cdn-alt.modrinth.com/\u{4e0b}\u{8f7d}/file.jar"
        )));
        assert!(!is_official_modrinth_cdn_redirect(None));
    }

    #[test]
    fn redirect_locations_are_bounded_ascii_values() {
        assert!(is_safe_redirect_location("/download/file.jar"));
        assert!(!is_safe_redirect_location("/\u{4e0b}\u{8f7d}/file.jar"));
        assert!(!is_safe_redirect_location(
            &"a".repeat(MAX_REDIRECT_LOCATION_BYTES + 1)
        ));
    }

    #[test]
    fn malformed_official_cdn_redirects_reuse_the_original_encoded_path() {
        let original = Url::parse(
			"https://cdn-alt.modrinth.com/data/project/versions/version/%E9%87%91%E5%90%88%E6%AC%A2_1.21%2B.zip",
		)
		.unwrap();
        for host in ["cdn-alt.modrinth.com", "cdn.modrinth.com"] {
            let redirect = Url::parse(&format!(
				"https://{host}/data/project/versions/version/\u{91c8}\u{91c8}.zip",
			))
			.unwrap();

            let repaired = repair_official_cdn_redirect(
				&original,
				&redirect,
				&format!("https://{host}/data/project/versions/version/\u{91c8}\u{91c8}.zip"),
			)
			.unwrap();

            assert_eq!(
                repaired.as_str(),
                format!(
                    "https://cdn.modrinth.com/data/project/versions/version/%E9%87%91%E5%90%88%E6%AC%A2_1.21%2B.zip"
                )
            );
        }
    }

    #[test]
    fn mrpack_urls_are_detected_without_query_string() {
        assert!(is_mrpack_url(
            "https://cdn.modrinth.com/data/project/version/pack.MRPACK?download=1"
        ));
        assert!(!is_mrpack_url(
            "https://cdn.modrinth.com/data/project/version/mod.jar"
        ));
    }

    #[test]
    fn fetch_retries_use_short_jittered_backoff() {
        let cases = [
            (1, Duration::from_millis(85), Duration::from_millis(115)),
            (2, Duration::from_millis(255), Duration::from_millis(345)),
            (3, Duration::from_millis(637), Duration::from_millis(863)),
            (4, Duration::from_millis(637), Duration::from_millis(863)),
        ];
        for (attempt, minimum, maximum) in cases {
            let delay = fetch_retry_delay(attempt);
            assert!(delay >= minimum, "attempt {attempt}: {delay:?}");
            assert!(delay <= maximum, "attempt {attempt}: {delay:?}");
        }
    }

    #[test]
    fn retry_after_is_allowed_up_to_its_own_cap() {
        assert_eq!(
            clamp_failure_cooldown(Duration::from_secs(30)),
            Duration::from_secs(30),
        );
        assert_eq!(
            clamp_failure_cooldown(Duration::from_secs(3600)),
            RETRY_AFTER_CAP
        );
    }

    #[tokio::test]
    async fn finalize_download_moves_a_part_file_into_a_new_destination() {
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("file.part");
        let destination = directory.path().join("file.bin");
        tokio::fs::write(&part, b"downloaded").await.unwrap();

        finalize_download(&part, &destination).await.unwrap();

        assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"downloaded");
        assert!(!part.exists(), "the part file should be consumed");
    }

    #[tokio::test]
    async fn finalize_download_replaces_an_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("file.part");
        let destination = directory.path().join("file.bin");
        tokio::fs::write(&part, b"complete").await.unwrap();
        tokio::fs::write(&destination, b"partial").await.unwrap();

        finalize_download(&part, &destination).await.unwrap();

        assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"complete");
        assert!(!part.exists(), "the part file should be consumed");
    }

    #[tokio::test]
    async fn finalize_download_keeps_the_destination_when_the_part_file_is_missing()
     {
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("missing.part");
        let destination = directory.path().join("file.bin");
        tokio::fs::write(&destination, b"previous").await.unwrap();

        finalize_download(&part, &destination).await.unwrap_err();

        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"previous",
            "a failed commit must not remove the existing destination"
        );
    }

    #[tokio::test]
    async fn finalize_download_aborts_when_the_flush_fails() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("file.bin");
        tokio::fs::write(&destination, b"previous").await.unwrap();
        // A directory cannot be opened for writing, so the flush fails for a
        // reason other than a missing file.
        let part = directory.path().join("unflushable.part");
        tokio::fs::create_dir(&part).await.unwrap();

        let error = finalize_download(&part, &destination).await.unwrap_err();

        assert!(
            !format!("{error}").is_empty(),
            "a failed flush must surface an error"
        );
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"previous",
            "a failed flush must not replace the destination"
        );
    }

    #[test]
    fn h2_fallback_outlives_immediate_retry_cleanup() {
        let _guard = H2_FALLBACK_TEST_LOCK.lock();
        let authority = "h2-cooldown.example:443";
        record_authority_h2_failure(authority);
        assert!(authority_uses_http1_fallback(authority));
        let key = crate::util::download::proxy_context::authority_key(
            authority,
            ProxyPolicy::System,
        );
        let remaining = H2_FALLBACK_AUTHORITIES
            .lock()
            .get(&key)
            .map(|until| until.saturating_duration_since(Instant::now()))
            .unwrap();
        assert!(remaining <= H2_FALLBACK_TTL);
        assert!(remaining >= time::Duration::from_secs(4 * 60));
        H2_FALLBACK_AUTHORITIES.lock().remove(&key);
    }

    #[test]
    fn global_download_dns_override_can_be_registered_and_cleared() {
        set_download_dns_host_override(
            "request-host.test",
            "resolver-target.test",
        )
        .unwrap();
        assert_eq!(
            DOWNLOAD_DNS_RESOLVER
                .host_override("request-host.test")
                .as_deref(),
            Some("resolver-target.test"),
        );
        clear_download_dns_host_override("request-host.test").unwrap();
        assert!(
            DOWNLOAD_DNS_RESOLVER
                .host_override("request-host.test")
                .is_none()
        );
    }

    #[test]
    fn vanilla_libraries_have_both_bmcl_routes() {
        let source = "https://libraries.minecraft.net/com/example/library/1/library-1.jar";
        let routes = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(routes.len(), 3);
        assert_eq!(
            routes[0].url,
            "https://bmclapi2.bangbang93.com/maven/com/example/library/1/library-1.jar"
        );
        assert_eq!(
            routes[1].url,
            "https://bmclapi2.bangbang93.com/libraries/com/example/library/1/library-1.jar"
        );
        assert_eq!(routes[2].url, source);
    }

    #[test]
    fn maven_central_libraries_mirror_to_aliyun_with_official_fallback() {
        let source = "https://repo.maven.apache.org/maven2/org/javassist/javassist/3.30.2-GA/javassist-3.30.2-GA.jar";
        let routes = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(routes.len(), 2);
        assert_eq!(
            routes[0].url,
            "https://maven.aliyun.com/repository/public/org/javassist/javassist/3.30.2-GA/javassist-3.30.2-GA.jar"
        );
        assert!(routes[0].is_mirror);

        assert_eq!(routes[1].url, source);
        assert!(!routes[1].is_mirror);

        let official_only = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::OfficialOnly,
        );
        assert_eq!(official_only.len(), 1);
        assert_eq!(official_only[0].url, source);
    }

    #[test]
    fn loader_libraries_prefer_mirrors_with_an_official_fallback() {
        let source = "https://libraries.minecraft.net/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar";
        let routes = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(routes.len(), 3);
        assert_eq!(
            routes[0].url,
            "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar"
        );
        assert_eq!(
            routes[1].url,
            "https://bmclapi2.bangbang93.com/libraries/net/minecraftforge/forge/1.20.1/forge-1.20.1.jar"
        );
        assert_eq!(routes[2].url, source);
        assert!(!routes[2].is_mirror);

        let official_only = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::OfficialOnly,
        );
        assert_eq!(official_only.len(), 1);
        assert_eq!(official_only[0].url, source);

        let official_preferred = resolve_download_routes_for(
            source,
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::OfficialPreferred,
        );
        assert_eq!(official_preferred.len(), 3);
        assert_eq!(official_preferred[0].url, source);
        assert!(official_preferred[1..].iter().all(|route| route.is_mirror));
        assert!(official_preferred.iter().all(|route| {
            resource_family(route, ResourceClass::MinecraftLibrary)
                == ResourceFamily::Loader
        }));
    }

    #[test]
    fn auto_loader_routes_start_official_and_retain_explicit_mirror_preference()
    {
        let _guard = AUTO_SOURCE_TEST_LOCK.lock().unwrap();
        let previous_health = std::mem::take(&mut *ROUTE_HEALTH.lock());
        for source in [
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
            "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/maven-metadata.xml",
        ] {
            let routes = resolve_download_routes_for(
                source,
                ResourceClass::Loader,
                crate::state::DownloadSourceMode::Auto,
            );
            assert_eq!(routes[0].url, source);
            assert_eq!(routes[0].source, DownloadRouteSource::Official);
            assert!(routes[1..].iter().any(|route| route.is_mirror));
            let mirrors = resolve_download_routes_for(
                source,
                ResourceClass::Loader,
                crate::state::DownloadSourceMode::MirrorPreferred,
            );
            assert!(mirrors[0].is_mirror);
            record_route_failure(&routes[0], ResourceClass::Loader, None);
            let after_failure = resolve_download_routes_for(
                source,
                ResourceClass::Loader,
                crate::state::DownloadSourceMode::Auto,
            );
            assert!(after_failure[0].is_mirror);
        }
        *ROUTE_HEALTH.lock() = previous_health;
    }

    #[test]
    fn maven_central_routes_mirror_to_aliyun_with_official_fallback() {
        for source in [
            "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar?download=1",
            "https://repo.maven.apache.org/maven2/com/example/library/1/library-1.jar?download=1",
        ] {
            let routes = resolve_download_routes_for(
                source,
                ResourceClass::MinecraftLibrary,
                crate::state::DownloadSourceMode::MirrorPreferred,
            );
            assert_eq!(routes.len(), 2);
            assert_eq!(routes[0].source, DownloadRouteSource::Aliyun);
            assert!(routes[0].is_mirror);
            assert!(routes[0]
                .url
                .starts_with("https://maven.aliyun.com/repository/public/com/example/library/1/library-1.jar"));
            assert_eq!(routes[1].url, source);
            assert_eq!(routes[1].source, DownloadRouteSource::Official);
        }

        let unmatched = resolve_download_routes_for(
            "https://repo1.maven.org/repository/com/example/library.jar",
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(unmatched.len(), 1);
        assert_eq!(
            unmatched[0].url,
            "https://repo1.maven.org/repository/com/example/library.jar"
        );

        let prefer_official = resolve_download_routes_for(
            "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar",
            ResourceClass::MinecraftLibrary,
            crate::state::DownloadSourceMode::OfficialOnly,
        );
        assert_eq!(prefer_official.len(), 1);
        assert_eq!(
            prefer_official[0].url,
            "https://repo1.maven.org/maven2/com/example/library/1/library-1.jar"
        );
    }

    #[test]
    fn source_matching_is_origin_safe() {
        assert!(same_origin(
            &Url::parse("https://api.curseforge.com/v1/mods").unwrap(),
            &Url::parse("https://api.curseforge.com/v1/files").unwrap(),
        ));
        assert!(!same_origin(
            &Url::parse("https://api.curseforge.com/v1/mods").unwrap(),
            &Url::parse("https://edge.forgecdn.net/files/1/2/a.jar").unwrap(),
        ));
        assert!(is_sensitive_header("x-api-key"));
        assert!(!header_requires_official_only("x-api-key"));
        assert!(is_sensitive_header("Authorization"));
        assert!(header_requires_official_only("Authorization"));
        assert!(!is_sensitive_header("accept"));
    }

    #[test]
    fn dynamic_ranges_split_the_largest_remaining_tail() {
        let range = DownloadRange::new(0, 0, 10 * 1024 * 1024 - 1);
        let tail = range.split_tail(1).unwrap();
        assert_eq!(range.end(), 6 * 1024 * 1024 - 1);
        assert_eq!(tail.start, 6 * 1024 * 1024);
        assert_eq!(tail.end(), 10 * 1024 * 1024 - 1);
        assert!(tail.remaining() >= 256 * 1024);

        let small = DownloadRange::new(2, 0, 256 * 1024 - 2);
        assert!(small.split_tail(3).is_none());
    }

    #[test]
    fn large_files_start_parallel_ranges_without_consulting_speed_floor() {
        let size = 16 * 1024 * 1024;
        assert_eq!(initial_segment_count(size, 64), 4);
        let ranges = create_initial_ranges(size, 4);
        assert_eq!(ranges.len(), 4);
        assert_eq!(ranges.first().unwrap().start, 0);
        assert_eq!(ranges.last().unwrap().end(), size - 1);
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end() + 1, pair[1].start);
        }
    }

    #[test]
    fn segmented_concurrency_respects_effective_and_global_limits() {
        assert_eq!(segmented_concurrency_cap(64), MAX_SEGMENT_CONCURRENCY);
        assert_eq!(segmented_concurrency_cap(8), 8);
        assert_eq!(segmented_concurrency_cap(4), 4);
        assert_eq!(segmented_concurrency_cap(1), 1);
        assert_eq!(initial_segment_count(16 * 1024 * 1024, 3), 3);
    }

    #[tokio::test]
    async fn segmented_permits_wait_fairly_instead_of_falling_back() {
        let semaphore =
            FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
        let held = semaphore.0.acquire_many(4).await.unwrap();
        let route = direct_test_route(
            "https://example.com/file".to_string(),
            DownloadRouteSource::Official,
        );
        let started = Instant::now();
        let (_, permits) = tokio::join!(
            async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                drop(held);
            },
            acquire_initial_segment_permits(&route, &semaphore, 4,),
        );
        let permits = permits.unwrap();
        assert_eq!(permits.len(), 4);
        assert!(started.elapsed() >= Duration::from_millis(40));
    }

    #[test]
    fn segmented_download_caps_are_provider_neutral() {
        let bmclapi = route(
            "https://bmclapi2.bangbang93.com/assets/file".to_string(),
            DownloadRouteSource::Bmclapi,
            true,
            true,
        );
        let official = route(
            "https://resources.download.minecraft.net/file".to_string(),
            DownloadRouteSource::Official,
            false,
            true,
        );

        let modrinth = route(
            "https://cdn.modrinth.com/data/project/version/file.jar".into(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        assert_eq!(route_segmented_concurrency_cap(&bmclapi, 64), 16);
        assert_eq!(route_segmented_concurrency_cap(&official, 64), 16);
        assert_eq!(route_segmented_concurrency_cap(&modrinth, 64), 16);
    }

    #[test]
    fn adaptive_ranges_can_expand_and_reserve_capacity_for_small_files() {
        let size = 128 * 1024 * 1024;
        let max = adaptive_range_cap(size, 8, 8, 0);
        assert_eq!(max, 8);
        assert!(initial_segment_count(size, max) < max);
        assert_eq!(adaptive_range_cap(8 * 1024 * 1024, 8, 8, 0), 2);
        assert_eq!(adaptive_range_cap(size, 8, 1, 0), 1);
        assert_eq!(adaptive_range_cap(size, 8, 8, 2), 2);
        assert_eq!(adaptive_range_cap(size, 8, 8, 4), 1);
        let official = route(
            "https://range-fairness.invalid/file".into(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        assert_eq!(route_segmented_concurrency_cap(&official, 4), 2);
        assert_eq!(route_segmented_concurrency_cap(&official, 1), 1);
    }

    #[test]
    fn stable_aggregate_throughput_allows_gradual_expansion() {
        let snapshot = SpeedSnapshot {
            aggregate_speed: 2 * 1024 * 1024,
            recent_average: 1900 * 1024,
            speed_floor: 1024 * 1024,
            sample_count: SEGMENT_EXPANSION_SAMPLE_COUNT,
        };
        assert_eq!(
            expansion_block_reason(
                snapshot,
                4,
                8,
                4,
                16 * 1024 * 1024,
                SEGMENT_EXPANSION_INTERVAL,
            ),
            None
        );
        assert_eq!(
            expansion_block_reason(
                snapshot,
                4,
                8,
                0,
                16 * 1024 * 1024,
                SEGMENT_EXPANSION_INTERVAL,
            ),
            Some("no global permit available")
        );
    }

    #[test]
    fn low_aggregate_throughput_still_allows_more_ranges() {
        let snapshot = SpeedSnapshot {
            aggregate_speed: 64 * 1024,
            recent_average: 80 * 1024,
            speed_floor: 1024 * 1024,
            sample_count: SEGMENT_EXPANSION_SAMPLE_COUNT,
        };
        assert_eq!(
            expansion_block_reason(
                snapshot,
                1,
                8,
                7,
                16 * 1024 * 1024,
                SEGMENT_EXPANSION_INTERVAL,
            ),
            None
        );
    }

    #[test]
    fn low_throughput_only_switches_routes_before_fallback_rounds() {
        assert!(allow_low_throughput_route_switch(true, false));
        assert!(!allow_low_throughput_route_switch(true, true));
        assert!(!allow_low_throughput_route_switch(false, false));
    }

    #[test]
    fn slow_http2_downgrades_only_on_the_first_attempt_round() {
        assert!(should_downgrade_slow_http2(reqwest::Version::HTTP_2, false));
        assert!(!should_downgrade_slow_http2(reqwest::Version::HTTP_2, true));
        assert!(!should_downgrade_slow_http2(
            reqwest::Version::HTTP_11,
            false
        ));
    }

    #[test]
    fn reassignable_mirror_uses_shorter_first_byte_timeout() {
        let mirror = route(
            "https://mirror.example/file".to_string(),
            DownloadRouteSource::Bmclapi,
            true,
            true,
        );
        assert_eq!(
            native_first_byte_timeout(&mirror, true),
            REASSIGNABLE_FIRST_BYTE_TIMEOUT
        );
        assert_eq!(
            native_first_byte_timeout(&mirror, false),
            FILE_TRANSFER_FIRST_BYTE_TIMEOUT
        );
    }

    #[test]
    fn official_fallback_excludes_alternate_sources() {
        let alternate = route(
            "https://alternate.example/file".to_string(),
            DownloadRouteSource::Alternate,
            false,
            true,
        );
        let official = route(
            "https://official.example/file".to_string(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        assert_eq!(
            official_fallback_route(&[alternate, official.clone()]),
            Some(official)
        );
    }

    #[test]
    fn segmented_download_starts_at_four_mebibytes() {
        assert!(!should_use_segmented_download(
            SEGMENTED_DOWNLOAD_THRESHOLD - 1,
            0,
        ));
        assert!(should_use_segmented_download(
            SEGMENTED_DOWNLOAD_THRESHOLD,
            0,
        ));
    }

    #[test]
    fn route_probe_requires_a_twenty_five_percent_improvement() {
        assert!(!probe_is_meaningfully_faster(1_249_999, 1_000_000));
        assert!(probe_is_meaningfully_faster(1_250_000, 1_000_000));
        assert!(!probe_is_meaningfully_faster(u64::MAX, u64::MAX));
        assert_eq!(
            measured_bytes_per_second(
                ROUTE_PROBE_BYTES,
                Duration::from_millis(250),
            ),
            1024 * 1024,
        );
    }

    #[test]
    fn effective_authority_drives_health_without_removing_fallback_urls() {
        let alias = route(
            "https://effective-authority-alias.invalid/file.jar".to_string(),
            DownloadRouteSource::Mcim,
            true,
            true,
        );
        let direct = route(
            "https://effective-authority-target.invalid/file.jar".to_string(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        remember_effective_route_authority(&alias, &direct.url);

        assert_eq!(
            route_health_key(&alias, ResourceClass::Other),
            route_health_key(&direct, ResourceClass::Other),
        );
        assert!(routes_share_effective_authority(&alias, &direct));

        let mut direct_without_proxy = direct.clone();
        direct_without_proxy.proxy = ProxyPolicy::Direct;
        let mut routes = vec![alias, direct, direct_without_proxy.clone()];
        deduplicate_download_routes(&mut routes);

        assert_eq!(routes.len(), 3);
        assert_eq!(routes[2], direct_without_proxy);
    }

    #[test]
    fn route_deduplication_preserves_distinct_paths_on_one_authority() {
        let first = route(
            "https://mirror.example/maven/library.jar".to_string(),
            DownloadRouteSource::Bmclapi,
            true,
            true,
        );
        let mut second = first.clone();
        second.url = "https://mirror.example/libraries/library.jar".to_string();
        let mut routes = vec![first.clone(), second.clone(), first];

        deduplicate_download_routes(&mut routes);

        assert_eq!(
            routes,
            vec![
                route(
                    "https://mirror.example/maven/library.jar".to_string(),
                    DownloadRouteSource::Bmclapi,
                    true,
                    true,
                ),
                second,
            ]
        );
    }

    #[test]
    fn effective_authority_memory_is_scoped_to_the_exact_route_url() {
        let redirected = route(
            "https://route-scope.example/cache-miss.jar".to_string(),
            DownloadRouteSource::Bmclapi,
            true,
            true,
        );
        let direct = route(
            "https://route-scope.example/cache-hit.jar".to_string(),
            DownloadRouteSource::Bmclapi,
            true,
            true,
        );
        remember_effective_route_authority(
            &redirected,
            "https://official.example/cache-miss.jar",
        );

        assert_eq!(
            effective_route_authority(&redirected).as_deref(),
            Some("official.example:443"),
        );
        assert_eq!(
            effective_route_authority(&direct).as_deref(),
            Some("route-scope.example:443"),
        );
    }

    #[test]
    fn h2_route_selection_skips_an_open_breaker_when_possible() {
        let blocked = route(
            "https://blocked-h2.example/file".to_string(),
            DownloadRouteSource::Alternate,
            true,
            true,
        );
        let healthy = route(
            "https://healthy-h2.example/file".to_string(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        for _ in 0..3 {
            crate::util::download::native_breaker::record_failure(&blocked);
        }

        assert_eq!(
            first_h2_route(&[blocked.clone(), healthy.clone()]),
            Some(healthy)
        );
        assert_eq!(
            first_h2_route(std::slice::from_ref(&blocked)),
            Some(blocked.clone())
        );
        crate::util::download::native_breaker::record_success(&blocked);
    }

    #[tokio::test]
    async fn route_probe_selects_a_faster_distinct_authority() {
        let data = Arc::new(vec![7_u8; ROUTE_PROBE_BYTES as usize * 2]);
        let (url, requests, _, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let current = route(
            "https://route-probe-current.invalid/file.jar".to_string(),
            DownloadRouteSource::Official,
            false,
            true,
        );
        let mut candidate =
            route(url, DownloadRouteSource::Alternate, false, true);
        candidate.proxy = ProxyPolicy::Direct;
        let candidates = vec![candidate.clone()];
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let semaphore = FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY));

        let probe = probe_faster_route(
            &current,
            &candidates,
            64 * 1024,
            data.len() as u64,
            None,
            None,
            None,
            &semaphore,
            &client,
            &client,
            ResourceClass::Other,
            0,
        )
        .await
        .expect("the local range route should be measurably faster");

        assert_eq!(probe.route, candidate);
        assert!(probe.bytes_per_second >= 80 * 1024);
        assert_eq!(requests.load(Ordering::Relaxed), 1);

        let alias = route(
            "https://route-probe-alias.invalid/file.jar".to_string(),
            DownloadRouteSource::Mcim,
            true,
            true,
        );
        remember_effective_route_authority(&alias, &candidate.url);
        assert!(
            probe_faster_route(
                &alias,
                &candidates,
                1,
                data.len() as u64,
                None,
                None,
                None,
                &semaphore,
                &client,
                &client,
                ResourceClass::Other,
                0,
            )
            .await
            .is_none()
        );
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[derive(Clone, Copy)]
    enum H2FailureFixture {
        Slow,
        Reset,
        ReceiveTimeout,
    }

    async fn check_h2_fallback(
        resume: bool,
        with_alternate: bool,
        failure: H2FailureFixture,
    ) {
        let data = Arc::new(vec![7_u8; 256 * 1024]);
        let (url, requests, normal_requests, http1_server) =
            spawn_range_server(
                data.clone(),
                RangeServerBehavior {
                    fail_first_range: with_alternate,
                    ..Default::default()
                },
            )
            .await;
        let mut primary =
            route(url.clone(), DownloadRouteSource::Official, false, true);
        primary.proxy = ProxyPolicy::Direct;
        let mut routes = vec![primary.clone()];
        let mut alternate_server = None;
        let mut alternate_requests = None;
        if with_alternate {
            let (url, requests, _, server) =
                spawn_range_server(data.clone(), Default::default()).await;
            let mut alternate =
                route(url, DownloadRouteSource::Alternate, false, true);
            alternate.proxy = ProxyPolicy::Direct;
            routes.push(alternate);
            alternate_server = Some(server);
            alternate_requests = Some(requests);
        }
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let h2_data = data.clone();
        let finish_healthy = Arc::new(tokio::sync::Notify::new());
        let finish_server = finish_healthy.clone();
        let h2_server = tokio::spawn(async move {
            let mut server = h2::server::handshake(server_io).await.unwrap();
            let (_, mut healthy) = server.accept().await.unwrap().unwrap();
            let mut healthy_body = healthy
                .send_response(http::Response::new(()), false)
                .unwrap();
            healthy_body
                .send_data(bytes::Bytes::from_static(&[3; 1024]), false)
                .unwrap();
            let healthy_sender = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = finish_server.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {
                            healthy_body.send_data(bytes::Bytes::from_static(&[3; 1024]), false).unwrap();
                        }
                    }
                }
                healthy_body
                    .send_data(bytes::Bytes::from_static(b"done"), true)
                    .unwrap();
            });
            let (_, mut respond) = server.accept().await.unwrap().unwrap();
            let response = http::Response::builder()
                .header("content-length", h2_data.len())
                .body(())
                .unwrap();
            let mut body = respond.send_response(response, false).unwrap();
            let sender = tokio::spawn(async move {
                match failure {
                    H2FailureFixture::Reset => {
                        body.send_reset(h2::Reason::CANCEL);
                        return;
                    }
                    H2FailureFixture::ReceiveTimeout => {
                        std::future::pending::<()>().await;
                        drop(body);
                        return;
                    }
                    H2FailureFixture::Slow => {}
                }
                for chunk in h2_data.chunks(1024) {
                    if body
                        .send_data(bytes::Bytes::copy_from_slice(chunk), false)
                        .is_err()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            });
            while server.accept().await.is_some() {}
            sender.abort();
            healthy_sender.abort();
        });
        let (sender, driver) = h2::client::handshake(client_io).await.unwrap();
        let connection = super::super::download::h2_pool::SharedH2Connection::cache_for_test(&primary, sender).await;
        let driver = super::super::download::h2_pool::spawn_connection_driver(
            &connection,
            driver,
            Duration::from_secs(60),
        );
        let healthy_activity = connection.track_stream();
        let mut scheduler = super::super::download::authority_scheduler::AuthorityScheduler::default();
        scheduler.add_connection(connection.clone());
        let mut healthy_body = connection
            .open(
                http::Request::builder()
                    .uri("https://test.invalid/healthy")
                    .body(())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body();
        let healthy_download = tokio::spawn(async move {
            let _activity = healthy_activity;
            let mut received = Vec::new();
            while let Some(chunk) = healthy_body.data().await {
                let chunk = chunk.unwrap();
                healthy_body
                    .flow_control()
                    .release_capacity(chunk.len())
                    .unwrap();
                received.extend_from_slice(&chunk);
            }
            received
        });
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("fallback.bin");
        let part = suffixed_path(&destination, ".part");
        let integrity = if resume {
            Integrity::sha1(sha1_smol::Sha1::from(&data[..]).hexdigest())
                .with_size(data.len() as u64)
        } else {
            Integrity::default().with_size(data.len() as u64)
        };
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(integrity);
        let semaphore =
            FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
        let mut policy = super::super::download::native::h2_policy(
            &primary,
            request.integrity.size,
        )
        .await
        .unwrap();
        if !matches!(failure, H2FailureFixture::Slow) {
            policy.abort_if_slow = false;
        }
        assert!(matches!(
            try_h2_download(
                &request,
                primary.clone(),
                policy,
                &destination,
                &part,
                &semaphore
            )
            .await
            .unwrap(),
            H2AttemptResult::Fallback { .. }
        ));
        assert!(!connection.is_retiring());
        assert!(
            !connection.is_dead(),
            "healthy stream must keep its driver alive"
        );
        assert!(
            super::super::download::h2_pool::has_live_connection(&primary)
                .await
        );
        assert!(scheduler.select_connection().is_some());
        connection.release_stream_reservation();
        scheduler.prune();
        assert!(scheduler.select_connection().is_some());
        connection.release_stream_reservation();
        finish_healthy.notify_one();
        let healthy_bytes =
            tokio::time::timeout(Duration::from_secs(3), healthy_download)
                .await
                .unwrap()
                .unwrap();
        assert!(healthy_bytes.len() > 4);
        assert!(
            healthy_bytes[..healthy_bytes.len() - 4]
                .iter()
                .all(|byte| *byte == 3)
        );
        assert!(healthy_bytes.ends_with(b"done"));
        connection.retire();
        tokio::time::timeout(Duration::from_secs(3), driver)
            .await
            .unwrap()
            .unwrap();
        assert!(connection.is_dead());
        assert!(!authority_uses_http1_fallback_for(
            &url_authority(&url).unwrap(),
            primary.proxy
        ));
        if resume {
            let len = tokio::fs::metadata(&part).await.unwrap().len();
            assert!(len > 0 && len < data.len() as u64);
            assert_eq!(
                tokio::fs::read(&part).await.unwrap(),
                data[..len as usize]
            );
        } else {
            assert!(!part.exists());
        }
        let result = run_native_download_attempts(
            request,
            &destination,
            &semaphore,
            None,
            routes,
            part,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.size, data.len() as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        assert!(requests.load(Ordering::Relaxed) > 0);
        if resume && !with_alternate {
            assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
        }
        if let Some(requests) = alternate_requests {
            assert!(requests.load(Ordering::Relaxed) > 0);
        }
        if let Some(server) = alternate_server {
            server.abort();
        }
        h2_server.abort();
        http1_server.abort();
    }

    #[tokio::test]
    async fn slow_h2_fallback_restarts_single_source_over_http1() {
        check_h2_fallback(false, false, H2FailureFixture::Slow).await;
    }

    #[tokio::test]
    async fn slow_h2_fallback_resumes_and_verifies_partial_over_http1() {
        check_h2_fallback(true, false, H2FailureFixture::Slow).await;
    }

    #[tokio::test]
    async fn slow_h2_fallback_can_recover_using_alternate_cdn() {
        check_h2_fallback(true, true, H2FailureFixture::Slow).await;
    }

    #[tokio::test]
    async fn reset_h2_stream_does_not_interrupt_healthy_sibling() {
        check_h2_fallback(false, false, H2FailureFixture::Reset).await;
    }

    #[tokio::test]
    async fn timed_out_h2_stream_does_not_interrupt_healthy_sibling() {
        check_h2_fallback(false, false, H2FailureFixture::ReceiveTimeout).await;
    }

    #[tokio::test]
    async fn recovery_probe_prefers_measured_http1_without_recording_protocol_failure()
     {
        let data = Arc::new(vec![9_u8; 512 * 1024]);
        let (target_url, target_requests, _, target_server) =
            spawn_range_server(data.clone(), Default::default()).await;
        let (url, redirect_requests, redirect_server) =
            spawn_redirect_server(target_url.clone(), Duration::ZERO).await;
        let mut primary =
            route(url.clone(), DownloadRouteSource::Official, false, true);
        primary.proxy = ProxyPolicy::Direct;
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::default().with_size(data.len() as u64));
        let semaphore =
            FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
        let selected = probe_recovery_route(
            &request,
            &primary,
            &[],
            1024,
            data.len() as u64,
            0,
            true,
            None,
            &semaphore,
        )
        .await
        .unwrap();
        let original = url_authority(&url).unwrap();
        let target = url_authority(&target_url).unwrap();
        assert_ne!(original, target);
        assert_eq!(selected.effective_authority, target);
        assert!(authority_uses_http1_fallback_for(&original, primary.proxy));
        assert!(authority_uses_http1_fallback_for(&target, primary.proxy));
        for authority in [&original, &target] {
            assert!(!H2_FALLBACK_AUTHORITIES.lock().contains_key(
                &super::super::download::proxy_context::authority_key(
                    authority,
                    primary.proxy
                )
            ));
        }
        assert_eq!(redirect_requests.load(Ordering::Relaxed), 1);
        assert_eq!(target_requests.load(Ordering::Relaxed), 1);
        redirect_server.abort();
        target_server.abort();
    }

    #[tokio::test]
    async fn recovery_probe_compares_http1_and_alternate_cdn() {
        for http1_is_faster in [false, true] {
            let data = Arc::new(vec![9_u8; 512 * 1024]);
            let (url, primary_requests, _, primary_server) =
                spawn_range_server(
                    data.clone(),
                    RangeServerBehavior {
                        slow_body: !http1_is_faster,
                        ..Default::default()
                    },
                )
                .await;
            let (alternate_url, alternate_requests, _, alternate_server) =
                spawn_range_server(
                    data.clone(),
                    RangeServerBehavior {
                        slow_body: http1_is_faster,
                        ..Default::default()
                    },
                )
                .await;
            let mut primary =
                route(url.clone(), DownloadRouteSource::Official, false, true);
            primary.proxy = ProxyPolicy::Direct;
            let mut alternate = route(
                alternate_url,
                DownloadRouteSource::Alternate,
                false,
                true,
            );
            alternate.proxy = ProxyPolicy::Direct;
            let request = DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(
                    Integrity::sha1(
                        sha1_smol::Sha1::from(&data[..]).hexdigest(),
                    )
                    .with_size(data.len() as u64),
                );
            let semaphore =
                FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
            let selected = probe_recovery_route(
                &request,
                &primary,
                std::slice::from_ref(&alternate),
                1024,
                data.len() as u64,
                0,
                true,
                None,
                &semaphore,
            )
            .await
            .unwrap();
            assert_eq!(
                selected.route,
                if http1_is_faster {
                    primary.clone()
                } else {
                    alternate
                }
            );
            assert_eq!(primary_requests.load(Ordering::Relaxed), 1);
            assert_eq!(alternate_requests.load(Ordering::Relaxed), 1);
            assert_eq!(
                authority_uses_http1_fallback_for(
                    &url_authority(&url).unwrap(),
                    primary.proxy
                ),
                http1_is_faster
            );
            primary_server.abort();
            alternate_server.abort();
        }
    }

    #[test]
    fn redirect_hops_rebuild_the_same_range_header() {
        let original = byte_range_header_value(Some(1024), Some(2047));
        let redirected = byte_range_header_value(Some(1024), Some(2047));
        assert_eq!(original.as_deref(), Some("bytes=1024-2047"));
        assert_eq!(redirected, original);
    }

    #[tokio::test]
    async fn verifies_streaming_integrity_algorithms() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"axolotl download").unwrap();
        let integrity = Integrity {
			size: Some(16),
			sha1: Some("90e438ead880c77ea2d7e726b5aa74e6d21a805f".to_string()),
			sha512: Some("2dcd3e0a9f198e9ef892a28ed6534dd154be2bd13531c961c0852aa6f1e24f633d2cd8288d3cf13c6a482a87c822b74a4901a2aa64292f4a371c5ebfea392c1b".to_string()),
			sha256: Some("120561bc60d59ebe2a08fc229ff2b1eb06b20c4211d21a17c15dd80790f48672".to_string()),
			md5: Some("30018bb52add8c6dbc5d4149c1325df0".to_string()),
			content: ContentValidation::None,
		};
        assert_eq!(verify_file(file.path(), &integrity).await.unwrap(), 16);
    }

    #[tokio::test]
    async fn segmented_download_uses_parallel_validated_ranges() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        RANGE_SPLITTING_SUPPORTED.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2 + 1024 * 1024) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, normal_requests, server) = spawn_range_server(
            data.clone(),
            RangeServerBehavior {
                slow_body: true,
                ..Default::default()
            },
        )
        .await;
        let route = DownloadRoute {
            url: url.clone(),
            source: DownloadRouteSource::Alternate,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64));
        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("fixture.part");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let semaphore = FetchSemaphore(Semaphore::new(8));
        let progress_samples = Arc::new(Mutex::new(Vec::new()));
        let samples = Arc::clone(&progress_samples);
        let mut progress: Box<FetchProgressFn<'_>> =
            Box::new(move |bytes, _| {
                samples.lock().push(bytes);
                Box::pin(async { Ok(()) })
            });
        let outcome = try_segmented_download(
            SegmentedDownloadContext::new(
                &request,
                &route,
                &[],
                size as u64,
                &part_path,
                &semaphore,
                None,
                &client,
                &client,
                1,
                1,
                false,
            ),
            Some(&mut progress),
        )
        .await;
        match outcome {
            SegmentedDownloadOutcome::Success(result) => {
                assert_eq!(result.size, size as u64);
            }
            _ => panic!("segmented fixture download did not succeed"),
        }
        let samples = progress_samples.lock();
        assert!(samples.first().is_some_and(|bytes| *bytes <= 512 * 1024));
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(samples.iter().all(|bytes| *bytes <= size as u64));
        drop(samples);
        assert!(requests.load(Ordering::Relaxed) >= 2);
        assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
        assert!(RANGE_SPLITTING_SUPPORTED.lock().contains(
            &super::super::download::proxy_context::authority_key(
                &range_splitting_authority(&route).unwrap(),
                route.proxy
            )
        ));
        assert_eq!(
            verify_file(&part_path, &request.integrity).await.unwrap(),
            size as u64
        );
        for index in 0..MAX_SEGMENT_CONCURRENCY {
            assert!(
                !segment_path(&part_path, index).exists(),
                "direct range output must not create segment files"
            );
        }
        server.abort();
    }

    #[tokio::test]
    async fn stalled_tail_range_is_hedged_and_temp_files_are_cleaned() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, _, server) = spawn_range_server(
            data,
            RangeServerBehavior {
                stall_first_range: true,
                ..Default::default()
            },
        )
        .await;
        let route = DownloadRoute {
            url: url.clone(),
            source: DownloadRouteSource::Alternate,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64));
        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("hedged.part");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let semaphore = FetchSemaphore(Semaphore::new(8));
        let started = Instant::now();
        let outcome = try_segmented_download(
            SegmentedDownloadContext::new(
                &request,
                &route,
                &[],
                size as u64,
                &part_path,
                &semaphore,
                None,
                &client,
                &client,
                1,
                1,
                false,
            ),
            None,
        )
        .await;
        assert!(matches!(outcome, SegmentedDownloadOutcome::Success(_)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(requests.load(Ordering::Relaxed) > 2);
        assert_eq!(
            verify_file(&part_path, &request.integrity).await.unwrap(),
            size as u64
        );
        for range_index in 0..MAX_SEGMENT_CONCURRENCY {
            for candidate_index in 0..2 {
                assert!(
                    !tail_candidate_path(
                        &part_path,
                        range_index,
                        candidate_index,
                    )
                    .exists()
                );
            }
        }
        server.abort();
    }

    #[tokio::test]
    async fn mirror_ranges_start_concurrently_and_retries_reuse_redirect() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (target_url, range_requests, normal_requests, range_server) =
            spawn_range_server(
                data,
                RangeServerBehavior {
                    fail_first_range: true,
                    ..Default::default()
                },
            )
            .await;
        let (redirect_url, redirect_requests, redirect_server) =
            spawn_redirect_server(target_url, Duration::from_millis(50)).await;
        let route = DownloadRoute {
            url: redirect_url.clone(),
            source: DownloadRouteSource::Mcim,
            is_mirror: true,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let request =
            DownloadRequest::new(&redirect_url, ResourceClass::Modrinth)
                .with_integrity(Integrity::sha1(hash).with_size(size as u64));
        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("redirect.part");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let outcome = try_segmented_download(
            SegmentedDownloadContext::new(
                &request,
                &route,
                &[],
                size as u64,
                &part_path,
                &FetchSemaphore(Semaphore::new(8)),
                None,
                &client,
                &client,
                1,
                1,
                false,
            ),
            None,
        )
        .await;

        assert!(matches!(outcome, SegmentedDownloadOutcome::Success(_)));
        assert_eq!(redirect_requests.load(Ordering::Relaxed), 2);
        assert!(range_requests.load(Ordering::Relaxed) > 2);
        assert_eq!(normal_requests.load(Ordering::Relaxed), 0);
        redirect_server.abort();
        range_server.abort();
    }

    #[tokio::test]
    async fn temporary_range_failure_resumes_without_disabling_segments() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, _, server) = spawn_range_server(
            data,
            RangeServerBehavior {
                fail_first_range: true,
                ..Default::default()
            },
        )
        .await;
        let route = DownloadRoute {
            url: url.clone(),
            source: DownloadRouteSource::Alternate,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let request = DownloadRequest::new(&url, ResourceClass::Other)
            .with_integrity(Integrity::sha1(hash).with_size(size as u64));
        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("retry.part");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let outcome = try_segmented_download(
            SegmentedDownloadContext::new(
                &request,
                &route,
                &[],
                size as u64,
                &part_path,
                &FetchSemaphore(Semaphore::new(8)),
                None,
                &client,
                &client,
                1,
                1,
                false,
            ),
            None,
        )
        .await;

        assert!(matches!(outcome, SegmentedDownloadOutcome::Success(_)));
        assert!(requests.load(Ordering::Relaxed) > 2);
        assert_eq!(
            verify_file(&part_path, &request.integrity).await.unwrap(),
            size as u64
        );
        server.abort();
    }

    #[tokio::test]
    async fn invalid_content_range_disables_range_splitting_after_repeats() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let data = Arc::new(vec![7_u8; 1024 * 1024]);
        let (url, _, _, server) = spawn_range_server(
            data.clone(),
            RangeServerBehavior {
                wrong_content_range: true,
                ..Default::default()
            },
        )
        .await;
        let route = DownloadRoute {
            url: url.clone(),
            source: DownloadRouteSource::Alternate,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let directory = tempfile::tempdir().unwrap();
        let part_path = directory.path().join("fixture.part");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let semaphore =
            FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
        let (progress, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let permit =
            acquire_native_connection(&route, &semaphore).await.unwrap();
        let speed = DownloadSpeedTracker::default();
        let validator = Mutex::new(None);
        let output = crate::util::download::range_output::RangeOutput::create(
            &part_path,
            data.len() as u64,
        )
        .await
        .unwrap();
        let result = download_segment(
            &route,
            DownloadRange::new(0, 0, data.len() as u64 - 1),
            data.len() as u64,
            None,
            None,
            None,
            &part_path,
            &output,
            permit,
            &client,
            &client,
            progress,
            &speed,
            &validator,
            None,
            &semaphore,
            &AtomicUsize::new(0),
            &AtomicBool::new(false),
            &DownloadRequest::new(&route.url, ResourceClass::Other),
        )
        .await;
        assert!(matches!(
            result,
            Err(SegmentDownloadError::Protocol("invalid Content-Range"))
        ));
        disable_range_splitting(&route);
        assert!(
            range_splitting_allowed(&route),
            "a single protocol failure should not disable range splitting"
        );
        disable_range_splitting(&route);
        assert!(
            !range_splitting_allowed(&route),
            "repeated protocol failures should disable range splitting"
        );
        server.abort();
    }

    #[tokio::test]
    async fn ignored_ranges_fall_back_to_one_full_file_write() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let (url, requests, normal_requests, server) = spawn_range_server(
            data.clone(),
            RangeServerBehavior {
                ignore_range: true,
                ..Default::default()
            },
        )
        .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("ignored-range.bin");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(Integrity::default().with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(8)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, size as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        assert!(requests.load(Ordering::Relaxed) >= 2);
        assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
        for index in 0..MAX_SEGMENT_CONCURRENCY {
            assert!(
                !segment_path(&suffixed_path(&destination, ".part"), index)
                    .exists()
            );
        }
        server.abort();
    }

    #[tokio::test]
    async fn canceling_segmented_download_releases_permits_and_temp_files() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 4) as usize;
        let data = Arc::new(vec![13_u8; size]);
        let (url, _, _, server) = spawn_range_server(
            data,
            RangeServerBehavior {
                slow_body: true,
                ..Default::default()
            },
        )
        .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("canceled-segments.bin");
        let destination_for_task = destination.clone();
        let semaphore = Arc::new(FetchSemaphore(Semaphore::new(8)));
        let semaphore_for_task = Arc::clone(&semaphore);
        let task = tokio::spawn(async move {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .connect_lazy("sqlite::memory:")
                .unwrap();
            download_to_path(
                DownloadRequest::new(url, ResourceClass::Other).with_integrity(
                    Integrity::default().with_size(size as u64),
                ),
                destination_for_task,
                &semaphore_for_task,
                &pool,
                None,
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(150)).await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(Duration::from_millis(25)).await;

        assert_eq!(semaphore.0.available_permits(), 8);
        let part_path = suffixed_path(&destination, ".part");
        assert!(!part_path.exists());
        for index in 0..MAX_SEGMENT_CONCURRENCY {
            assert!(!segment_path(&part_path, index).exists());
        }
        server.abort();
    }

    #[tokio::test]
    async fn valid_existing_destination_is_reused_without_network_request() {
        let data = Arc::new(b"already complete".to_vec());
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, _, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing.bin");
        tokio::fs::write(&destination, &data[..]).await.unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();

        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other).with_integrity(
                Integrity::sha1(hash).with_size(data.len() as u64),
            ),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.attempts, 0);
        assert_eq!(requests.load(Ordering::Relaxed), 0);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        server.abort();
    }

    #[tokio::test]
    async fn invalid_existing_destination_is_redownloaded() {
        let data = Arc::new(b"correct content".to_vec());
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, _, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("stale.bin");
        tokio::fs::write(&destination, b"wrong content")
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();

        download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other).with_integrity(
                Integrity::sha1(hash).with_size(data.len() as u64),
            ),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(requests.load(Ordering::Relaxed), 1);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        server.abort();
    }

    #[tokio::test]
    async fn unknown_size_and_small_files_use_one_connection() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        for (expected_size, integrity) in [
            (None, Integrity::default()),
            (Some(1024_u64), Integrity::default().with_size(1024)),
        ] {
            let data = Arc::new(vec![11_u8; 1024]);
            let (url, requests, normal_requests, server) = spawn_range_server(
                data.clone(),
                RangeServerBehavior::default(),
            )
            .await;
            let directory = tempfile::tempdir().unwrap();
            let destination = directory.path().join("single.bin");
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .connect_lazy("sqlite::memory:")
                .unwrap();
            let result = download_to_path(
                DownloadRequest::new(&url, ResourceClass::Other)
                    .with_integrity(integrity),
                &destination,
                &FetchSemaphore(Semaphore::new(8)),
                &pool,
                None,
            )
            .await
            .unwrap();
            assert_eq!(result.size, expected_size.unwrap_or(1024));
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            assert_eq!(normal_requests.load(Ordering::Relaxed), 1);
            server.abort();
        }
    }

    #[tokio::test]
    async fn single_connection_downloads_resume_from_partial_files() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = 512 * 1024_usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, normal_requests, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("resumed.bin");
        let part_path = suffixed_path(&destination, ".part");
        tokio::fs::write(&part_path, &data[..size / 2])
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, size as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        assert_eq!(
            requests.load(Ordering::Relaxed),
            1,
            "the resumed download should finish with one range request"
        );
        assert_eq!(
            normal_requests.load(Ordering::Relaxed),
            0,
            "the resumed download should not restart from the beginning"
        );
        assert!(!part_path.exists());
        server.abort();
    }

    #[tokio::test]
    async fn stale_partial_files_with_wrong_content_self_heal() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = 512 * 1024_usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, _, _, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("stale.bin");
        let part_path = suffixed_path(&destination, ".part");
        tokio::fs::write(&part_path, vec![0xAB_u8; size / 2])
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, size as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        server.abort();
    }

    #[tokio::test]
    async fn switching_sources_does_not_resume_another_sources_partial_file() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = 512 * 1024_usize;
        let expected = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let stale = Arc::new(vec![0xAB_u8; size - 4096]);
        let hash = sha1_smol::Sha1::from(&expected[..]).hexdigest();
        let (stale_url, stale_requests, stale_normal_requests, stale_server) =
            spawn_range_server(stale, RangeServerBehavior::default()).await;
        let (
            official_url,
            official_requests,
            official_normal_requests,
            official_server,
        ) = spawn_range_server(
            expected.clone(),
            RangeServerBehavior::default(),
        )
        .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("source-isolated.bin");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();

        let result = download_to_path(
            DownloadRequest::new(&stale_url, ResourceClass::Other)
                .with_candidate_urls([official_url])
                .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(ROUTE_PROBE_CONCURRENCY)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.source, DownloadRouteSource::Alternate);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *expected);
        assert_eq!(stale_requests.load(Ordering::Relaxed), 1);
        assert_eq!(stale_normal_requests.load(Ordering::Relaxed), 1);
        assert_eq!(official_requests.load(Ordering::Relaxed), 1);
        assert_eq!(
            official_normal_requests.load(Ordering::Relaxed),
            1,
            "the fallback source must restart instead of resuming mirror bytes",
        );
        stale_server.abort();
        official_server.abort();
    }

    #[tokio::test]
    async fn segmented_fallback_keeps_partial_files_for_resume() {
        let _guard = RANGE_SPLITTING_TEST_LOCK.lock().await;
        RANGE_SPLITTING_PROTOCOL_FAILURES.lock().clear();
        let size = (SEGMENTED_DOWNLOAD_THRESHOLD * 2) as usize;
        let data = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let hash = sha1_smol::Sha1::from(&data[..]).hexdigest();
        let (url, requests, normal_requests, server) =
            spawn_range_server(data.clone(), RangeServerBehavior::default())
                .await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("fallback-resume.bin");
        let part_path = suffixed_path(&destination, ".part");
        tokio::fs::write(&part_path, &data[..size / 4])
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        // A single global permit forces the segmented attempt to fall back
        // before transferring anything; the preserved partial data must
        // survive that fallback and drive a resumed single connection.
        let result = download_to_path(
            DownloadRequest::new(&url, ResourceClass::Other)
                .with_integrity(Integrity::sha1(hash).with_size(size as u64)),
            &destination,
            &FetchSemaphore(Semaphore::new(1)),
            &pool,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.size, size as u64);
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), *data);
        assert_eq!(
            requests.load(Ordering::Relaxed),
            1,
            "the fallback should finish with one resumed range request"
        );
        assert_eq!(
            normal_requests.load(Ordering::Relaxed),
            0,
            "the preserved partial data should not be discarded"
        );
        server.abort();
    }

    #[tokio::test]
    async fn nonempty_json_fetch_rejects_empty_collections() {
        let clients = DownloadClients::build(
            &crate::util::proxy::ProxyConfig {
                mode: crate::util::proxy::ProxyMode::None,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        super::super::download::proxy_context::with_snapshot(clients, async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .connect_lazy("sqlite::memory:")
                .unwrap();
            let semaphore =
                FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));

            let (url, requests, server) =
                spawn_json_server("[]".to_string()).await;
            let result = fetch_json_nonempty::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
                &pool,
            )
            .await;
            assert!(
                result.is_err(),
                "an empty JSON array must be rejected as invalid data"
            );
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();

            let (url, requests, server) =
                spawn_json_server("[1,2,3]".to_string()).await;
            let result = fetch_json_nonempty::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
                &pool,
            )
            .await;
            let values =
                result.expect("a non-empty JSON array should be accepted");
            assert_eq!(values.len(), 3);
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();
        })
        .await;
    }

    #[tokio::test]
    async fn regular_json_fetch_still_accepts_empty_collections() {
        let clients = DownloadClients::build(
            &crate::util::proxy::ProxyConfig {
                mode: crate::util::proxy::ProxyMode::None,
                ..Default::default()
            },
            false,
            false,
        )
        .unwrap();
        super::super::download::proxy_context::with_snapshot(clients, async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .connect_lazy("sqlite::memory:")
                .unwrap();
            let semaphore =
                FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));

            let (url, requests, server) =
                spawn_json_server("[]".to_string()).await;
            let result = fetch_json::<Vec<serde_json::Value>>(
                Method::GET,
                &url,
                None,
                None,
                None,
                &semaphore,
                &pool,
            )
            .await;
            let values = result
                .expect("regular JSON fetches keep accepting empty arrays");
            assert!(values.is_empty());
            assert_eq!(requests.load(Ordering::Relaxed), 1);
            server.abort();
        })
        .await;
    }

    #[test]
    fn h2_fallback_authority_marking() {
        let _guard = H2_FALLBACK_TEST_LOCK.lock();
        record_authority_h2_failure("example.com:443");
        assert!(authority_uses_http1_fallback("example.com:443"));
        assert!(!authority_uses_http1_fallback("other.com:443"));
        H2_FALLBACK_AUTHORITIES.lock().clear();
    }
}
