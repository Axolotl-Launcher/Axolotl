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
    fetch, fetch_advanced, fetch_json, fetch_json_nonempty, fetch_official,
    post_json,
};
pub use self::routes::resolve_download_routes_for;
pub(crate) use self::routes::*;
pub(crate) use self::segment::*;
pub(crate) use self::segmented::*;
pub(crate) use self::tail::*;
pub(crate) use self::throttle::*;
pub use self::throttle::{FetchSemaphore, IoSemaphore};
pub use self::transfer::download_to_path;
#[cfg(test)]
use super::download::modrinth_redirect::repair_official_redirect as repair_official_cdn_redirect;
#[cfg(test)]
use crate::util::download::constants::{
    FETCH_127_0_0_1, FETCH_LOCALHOST, HTTP_SCHEME,
};

pub(crate) use super::download::integrity::{
    compute_file_integrity, is_integrity_error, validate_file_content,
    verify_computed_integrity, verify_file,
};
#[cfg(test)]
use super::download::native_request::byte_range_header_value;
#[cfg(test)]
use super::download::route_health::resource_family;

#[cfg(test)]
pub(crate) use self::tests::fixtures::{
    download_benchmark_fixture, download_endpoint_fixture,
};

/// Set while a benchmark runs, so the client and the segment scheduler skip
/// deadline enforcement the measured path would never meet.
#[cfg(test)]
tokio::task_local! { pub(crate) static BENCHMARK_TIMING: (); }
