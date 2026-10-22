//! Native adaptive download support.
//!
//! Downloads use shared HTTP/2 connections when eligible and fall back to
//! HTTP/1.1 range requests. Route health, integrity checks, and recovery are
//! applied uniformly to every download.

pub(crate) mod authority_scheduler;
pub(crate) mod constants;
pub(crate) mod diagnostics;
pub(crate) mod endpoint_selection;
pub mod h2_download;
pub mod h2_pool;
pub(crate) mod h2_range;
pub(crate) mod h2_receive;
pub(crate) mod h2_stream_budget;
pub(crate) mod integrity;
pub mod legacy;
pub(crate) mod local_resources;
pub(crate) mod modrinth_cdn;
pub(crate) mod modrinth_redirect;
pub(crate) mod native;
pub(crate) mod native_breaker;
pub(crate) mod native_budget;
pub(crate) mod native_reputation;
pub(crate) mod native_request;
pub(crate) mod native_slow;
pub(crate) mod proxy_context;
pub(crate) mod range_journal;
pub(crate) mod range_output;
pub(crate) mod retired_sources;
pub(crate) mod route_health;
pub(crate) mod route_policy;
pub(crate) mod stream_output;
pub(crate) mod verified_file;
pub(crate) fn task_concurrency_limit(state: &crate::State) -> Option<usize> {
    Some(state.download_concurrency())
}
