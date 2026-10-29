//! Functions for fetching information from the Internet
mod client;
mod file;
mod model;
mod native;
mod policy;
mod probe;
mod progress;
mod request;
mod routes;
mod segment;
mod segmented;
mod tail;
mod throttle;
mod transfer;

#[cfg(test)]
mod tests;

pub(crate) use self::client::*;
pub use self::client::{
    INSECURE_REQWEST_CLIENT, build_proxied_client, configured_client,
};
pub(crate) use self::file::*;
pub use self::file::{
    cleanup_stale_partial_downloads, copy, read_json, sha1_async,
    sha1_file_async, sha1_file_cancellable, write, write_cached_icon,
};
pub(crate) use self::model::*;
pub use self::model::{
    ContentValidation, DownloadMeta, DownloadReason, DownloadRequest,
    DownloadResult, DownloadRoute, DownloadRouteSource, FetchProgressFn,
    Integrity, ProxyPolicy, ResourceClass,
};
pub(crate) use self::native::*;
pub(crate) use self::policy::*;
pub(crate) use self::probe::*;
pub(crate) use self::progress::*;
pub(crate) use self::request::*;
pub use self::request::{
    fetch, fetch_advanced, fetch_advanced_with_client, fetch_json,
    fetch_json_nonempty, fetch_official, post_json,
};
pub use self::routes::resolve_download_routes_for;
pub(crate) use self::routes::*;
pub(crate) use self::segment::*;
pub(crate) use self::segmented::*;
pub(crate) use self::tail::*;
pub(crate) use self::throttle::*;
pub use self::throttle::{
    FetchSemaphore, IoSemaphore, clear_download_dns_host_override,
    set_download_dns_host_override,
};
pub use self::transfer::download_to_path;
pub(crate) use self::transfer::*;
pub use super::download::constants::DOWNLOAD_META_HEADER;
use super::download::modrinth_redirect::is_official_redirect as is_official_modrinth_cdn_redirect;
#[cfg(test)]
use super::download::modrinth_redirect::repair_official_redirect as repair_official_cdn_redirect;
use super::download::retired_sources;
use super::download_manager::{DownloadSpeedTracker, SpeedSnapshot};
use super::io;
use crate::ErrorKind;
use crate::util::download::constants::{
    DOWNLOAD_SCHEDULING_GRACE, FILE_TRANSFER_FIRST_BYTE_TIMEOUT,
    H2_FALLBACK_TTL, MAX_SEGMENT_CONCURRENCY, METADATA_HEDGE_DELAY,
    MIN_SEGMENT_SIZE, REASSIGNABLE_FIRST_BYTE_TIMEOUT, RETRY_AFTER_CAP,
    ROUTE_PROBE_BYTES, SEGMENT_EXPANSION_INTERVAL,
    SEGMENT_EXPANSION_SAMPLE_COUNT, SEGMENTED_DOWNLOAD_THRESHOLD,
};
#[cfg(test)]
use crate::util::download::constants::{
    FETCH_127_0_0_1, FETCH_LOCALHOST, HTTP_SCHEME,
};
use crate::util::download::constants::{
    FILE_VALIDATION_CONCURRENCY, ROUTE_PROBE_CONCURRENCY,
};
use bytes::Bytes;
use parking_lot::Mutex;
use reqwest::{Method, StatusCode, header};
use serde::Deserialize;
use std::error::Error;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{self, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::{Notify, Semaphore};
use url::Url;

pub(crate) use super::download::integrity::{
    compute_file_integrity, is_integrity_error, validate_file_content,
    verify_computed_integrity, verify_file,
};
#[cfg(test)]
use super::download::native_request::byte_range_header_value;
#[cfg(test)]
use super::download::route_health::resource_family;
use super::download::route_health::{
    ROUTE_HEALTH, ResourceFamily, RouteHealth,
};

#[cfg(test)]
pub(crate) use self::tests::fixtures::{
    download_benchmark_fixture, download_endpoint_fixture,
};

/// Set while a benchmark runs, so the client and the segment scheduler skip
/// deadline enforcement the measured path would never meet.
#[cfg(test)]
tokio::task_local! { pub(crate) static BENCHMARK_TIMING: (); }
