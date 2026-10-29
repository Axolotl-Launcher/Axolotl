use super::SegmentDownloadError;
use super::client::{DownloadClients, is_h2_protocol_failure};
#[cfg(test)]
use super::file;
#[cfg(test)]
use super::file::write;
use super::file::{
    ResourceValidator, create_download_file, parse_content_range,
    suffixed_path, validate_resource_version,
};
use super::model::{DownloadMeta, DownloadRoute, Integrity};
#[cfg(test)]
use super::routes::route;
use super::routes::url_authority;
use super::segment::{
    DownloadRange, native_first_byte_timeout, range_idle_timeout,
};
use super::throttle::record_authority_h2_failure_for;
use crate::util::download::constants::{
    FETCH_INVALID_HEDGED_CONTENT_RANGE,
    FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE,
};
use crate::util::download::native_request::send_path_request_with_clients;
use crate::util::io::IOError;
use futures::StreamExt;
use parking_lot::Mutex;
use reqwest::StatusCode;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

pub(crate) fn tail_candidate_path(
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

pub(crate) struct TailCandidateCompletion {
    pub(crate) path: PathBuf,
    pub(crate) final_url: String,
    pub(crate) remote_addr: Option<std::net::SocketAddr>,
    pub(crate) http_version: reqwest::Version,
}

impl Drop for TailCandidateCompletion {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
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
    integrity: &Integrity,
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
    if !validate_resource_version(validator, &response, integrity) {
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
            tokio::time::timeout(range_idle_timeout(false), stream.next())
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
        crate::util::download::local_resources::write(
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
    crate::util::download::local_resources::write(&path, 0, file.flush())
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
pub(crate) async fn race_tail_candidates(
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
    integrity: &Integrity,
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
        integrity,
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
        integrity,
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
