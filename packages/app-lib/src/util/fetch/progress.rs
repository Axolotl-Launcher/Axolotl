#[cfg(test)]
use super::file;
use super::model::{DownloadRequest, DownloadRoute, ProxyPolicy};
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::routes::route;
use super::routes::sanitize_url_for_log;
use crate::install::DownloadItemStatus;
use crate::util::download::constants::{
    DOWNLOAD_DNS_PREWARM_CONCURRENCY, FILE_TCP_KEEPALIVE,
};
use futures::StreamExt;
use std::sync::Arc;
#[cfg(test)]
use std::time;

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
    let resolver = crate::util::download::proxy_context::resolver(proxy);
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
