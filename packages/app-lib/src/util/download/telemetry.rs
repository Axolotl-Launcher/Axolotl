//! Optional bounded download observations. These never participate in scheduling.

use crate::util::fetch::{DownloadRequest, DownloadRoute, ProxyPolicy};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Weak};
use std::time::{Duration, Instant};

const MAX_FILES: usize = 2048;
const MAX_TRANSFERS: usize = 8192;
const MAX_FILE_TRANSFERS: usize = 128;
const MAX_SAMPLES: usize = 60;
static ENABLED: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static STORE: LazyLock<Mutex<Store>> = LazyLock::new(Mutex::default);
#[cfg(test)]
pub(crate) static TEST_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

#[derive(Default)]
struct Store {
    files: VecDeque<Arc<FileCapture>>,
    transfers: VecDeque<(Weak<FileCapture>, Weak<Transfer>)>,
    dropped_files: u64,
    dropped_transfers: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadDiagnosticsSnapshot {
    pub enabled: bool,
    /// Remaining launcher permits, rather than a promise of OS socket capacity.
    pub pool_available: Option<usize>,
    pub files: Vec<DownloadFileSummary>,
    pub selected_file: Option<DownloadFileDetails>,
    pub dropped_files: u64,
    pub dropped_transfers: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadDiagnosticsExport {
    pub schema_version: u32,
    pub enabled: bool,
    pub pool_available: Option<usize>,
    pub files: Vec<DownloadFileDetails>,
    pub dropped_files: u64,
    pub dropped_transfers: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadFileSummary {
    pub id: String,
    pub job_id: Option<String>,
    pub item_id: Option<String>,
    pub name: String,
    pub status: &'static str,
    pub elapsed_ms: u64,
    pub network_elapsed_ms: Option<u64>,
    pub received_bytes: u64,
    pub configured_limit: Option<usize>,
    pub effective_cap: Option<usize>,
    pub target_connections: Option<usize>,
    pub held_permits: Option<usize>,
    pub active_requests: usize,
    pub effective_requests: usize,
    /// Unknown when reqwest does not expose physical socket identity.
    pub physical_connections: Option<usize>,
    pub effective_physical_connections: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadFileDetails {
    pub summary: DownloadFileSummary,
    pub transfers: Vec<DownloadTransferSnapshot>,
    pub samples: Vec<DownloadFileSample>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadTransferSnapshot {
    pub id: String,
    pub identity_kind: &'static str,
    pub physical_id: Option<u64>,
    pub protocol: String,
    /// The observed remote peer; it can be a proxy rather than the origin.
    pub remote_addr: Option<String>,
    pub proxied: bool,
    pub state: &'static str,
    pub received_bytes: u64,
    pub speed_bytes_per_second: Option<u64>,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadFileSample {
    pub elapsed_ms: u64,
    pub received_bytes: u64,
    pub active_requests: usize,
    pub effective_requests: usize,
    pub physical_connections: Option<usize>,
    pub effective_physical_connections: Option<usize>,
    pub effective_cap: Option<usize>,
    pub target_connections: Option<usize>,
}

#[derive(Debug, Default)]
struct FileState {
    configured_limit: Option<usize>,
    effective_cap: Option<usize>,
    target_connections: Option<usize>,
    held_permits: Option<usize>,
    transfers: VecDeque<Arc<Transfer>>,
    samples: VecDeque<DownloadFileSample>,
    last_sample_ms: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct FileCapture {
    id: u64,
    generation: u64,
    job_id: Option<String>,
    item_id: Option<String>,
    name: String,
    started: Instant,
    status: AtomicU8,
    ended_ms: AtomicU64,
    network_started_ms: AtomicU64,
    network_ended_ms: AtomicU64,
    received: AtomicU64,
    state: Mutex<FileState>,
}

#[derive(Debug)]
struct Transfer {
    id: u64,
    started: Instant,
    state: AtomicU8,
    ended_ms: AtomicU64,
    received: AtomicU64,
    last_byte_ms: AtomicU64,
    proxied: bool,
    metadata: Mutex<TransferMetadata>,
    rate: Mutex<RateSample>,
}

#[derive(Debug)]
struct TransferMetadata {
    physical_id: Option<u64>,
    protocol: String,
    remote_addr: Option<SocketAddr>,
}

#[derive(Debug, Default)]
struct RateSample {
    at_ms: u64,
    bytes: u64,
    speed: Option<u64>,
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(value: bool) {
    let mut store = STORE.lock();
    if ENABLED.swap(value, Ordering::AcqRel) == value {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    *store = Store::default();
    drop(store);
    if value && let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            loop {
                timer.tick().await;
                if !enabled()
                    || GENERATION.load(Ordering::Acquire) != generation
                {
                    break;
                }
                let files =
                    STORE.lock().files.iter().cloned().collect::<Vec<_>>();
                for file in files {
                    if file.status.load(Ordering::Relaxed) == 0 {
                        file.details();
                    }
                }
            }
        });
    }
}

fn safe_item(value: &str) -> String {
    let value = value.split(['?', '#']).next().unwrap_or_default();
    let absolute = value.starts_with(['/', '\\'])
        || value.as_bytes().get(1) == Some(&b':')
        || value.contains("://")
        || value.split(['/', '\\']).any(|part| part == "..");
    let value = if absolute {
        value.rsplit(['/', '\\']).next().unwrap_or_default()
    } else {
        value
    };
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect()
}

pub(crate) struct FileGuard(Arc<FileCapture>);

impl FileGuard {
    pub(crate) fn finish(&self, success: bool) {
        self.set_status(if success { 1 } else { 2 });
    }

    pub(crate) fn cancel(&self) {
        self.set_status(3);
    }

    fn set_status(&self, status: u8) {
        self.0.ended_ms.store(self.0.elapsed(), Ordering::Relaxed);
        self.0.status.store(status, Ordering::Release);
        self.0.state.lock().held_permits = Some(0);
        self.0.details();
    }
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        if self
            .0
            .status
            .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            self.0.ended_ms.store(self.0.elapsed(), Ordering::Relaxed);
        }
    }
}

pub(crate) fn begin_file(
    request: &mut DownloadRequest,
    destination: &Path,
) -> Option<FileGuard> {
    request.diagnostics = None;
    if !enabled() {
        return None;
    }
    let mut store = STORE.lock();
    if !enabled() {
        return None;
    }
    let tracking = request.install_tracking.as_ref();
    let file = Arc::new(FileCapture {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        generation: GENERATION.load(Ordering::Acquire),
        job_id: tracking.map(|t| t.reporter.job_id().to_string()),
        item_id: tracking.map(|t| safe_item(&t.item_id)),
        name: safe_item(
            &destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
        ),
        started: Instant::now(),
        status: AtomicU8::new(0),
        ended_ms: AtomicU64::new(0),
        network_started_ms: AtomicU64::new(u64::MAX),
        network_ended_ms: AtomicU64::new(0),
        received: AtomicU64::new(0),
        state: Mutex::new(FileState::default()),
    });
    if store.files.len() == MAX_FILES {
        store.dropped_files += 1;
        if let Some(index) = store
            .files
            .iter()
            .position(|file| file.status.load(Ordering::Acquire) != 0)
        {
            store.files.remove(index);
        } else {
            return None;
        }
    }
    store.files.push_back(Arc::clone(&file));
    request.diagnostics = Some(Arc::clone(&file));
    Some(FileGuard(file))
}

impl FileCapture {
    fn current(&self) -> bool {
        enabled() && self.generation == GENERATION.load(Ordering::Acquire)
    }

    fn elapsed(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }

    fn begin(
        self: &Arc<Self>,
        physical_id: Option<u64>,
        protocol: &str,
        remote_addr: Option<SocketAddr>,
        proxied: bool,
    ) -> Option<TransferGuard> {
        if !self.current() {
            return None;
        }
        let transfer = Arc::new(Transfer {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            state: AtomicU8::new(if protocol == "pending" { 0 } else { 1 }),
            ended_ms: AtomicU64::new(0),
            received: AtomicU64::new(0),
            last_byte_ms: AtomicU64::new(u64::MAX),
            proxied,
            metadata: Mutex::new(TransferMetadata {
                physical_id,
                protocol: protocol.to_owned(),
                remote_addr,
            }),
            rate: Mutex::new(RateSample::default()),
        });
        let mut store = STORE.lock();
        if !self.current() {
            return None;
        }
        while store.transfers.len() >= MAX_TRANSFERS {
            let Some(index) =
                store.transfers.iter().position(|(_, transfer)| {
                    transfer.upgrade().is_none_or(|transfer| {
                        transfer.state.load(Ordering::Acquire) >= 2
                    })
                })
            else {
                store.dropped_transfers += 1;
                return None;
            };
            if let Some((owner, transfer)) = store.transfers.remove(index)
                && let Some(owner) = owner.upgrade()
                && let Some(transfer) = transfer.upgrade()
            {
                let mut state = owner.state.lock();
                let before = state.transfers.len();
                state.transfers.retain(|t| t.id != transfer.id);
                store.dropped_transfers +=
                    (before - state.transfers.len()) as u64;
            }
        }
        let mut state = self.state.lock();
        if state.transfers.len() >= MAX_FILE_TRANSFERS {
            store.dropped_transfers += 1;
            if let Some(index) = state.transfers.iter().position(|transfer| {
                transfer.state.load(Ordering::Acquire) >= 2
            }) {
                state.transfers.remove(index);
            } else {
                return None;
            }
        }
        state.transfers.push_back(Arc::clone(&transfer));
        store
            .transfers
            .push_back((Arc::downgrade(self), Arc::downgrade(&transfer)));
        self.network_started_ms
            .fetch_min(self.elapsed(), Ordering::Relaxed);
        Some(TransferGuard {
            file: Arc::clone(self),
            transfer,
        })
    }

    fn details(&self) -> DownloadFileDetails {
        let now = self.elapsed();
        let status = self.status.load(Ordering::Acquire);
        let elapsed = if status == 0 {
            now
        } else {
            self.ended_ms.load(Ordering::Relaxed)
        };
        let mut state = self.state.lock();
        let mut active = 0;
        let mut connecting = 0;
        let mut effective = 0;
        let mut physical = HashSet::new();
        let mut effective_physical = HashSet::new();
        let mut unknown = false;
        let transfers = state
            .transfers
            .iter()
            .map(|t| {
                let transfer_state = t.state.load(Ordering::Acquire);
                connecting += usize::from(transfer_state == 0);
                let bytes = t.received.load(Ordering::Relaxed);
                let age = t.started.elapsed().as_millis().min(u64::MAX as u128)
                    as u64;
                let duration = if transfer_state < 2 {
                    age
                } else {
                    t.ended_ms.load(Ordering::Relaxed)
                };
                let metadata = t.metadata.lock();
                let mut rate = t.rate.lock();
                let interval = age.saturating_sub(rate.at_ms);
                if transfer_state >= 2 {
                    // Completed records retain their transfer-wide average.
                    rate.speed = (duration > 0)
                        .then(|| bytes.saturating_mul(1000) / duration);
                } else if interval >= 250 {
                    rate.speed = Some(
                        bytes.saturating_sub(rate.bytes).saturating_mul(1000)
                            / interval,
                    );
                    rate.at_ms = age;
                    rate.bytes = bytes;
                }
                if transfer_state == 1 {
                    active += 1;
                    let last_byte = t.last_byte_ms.load(Ordering::Relaxed);
                    let productive = last_byte != u64::MAX
                        && age.saturating_sub(last_byte) < 2000;
                    effective += usize::from(productive);
                    if let Some(id) = metadata.physical_id {
                        physical.insert(id);
                        if productive {
                            effective_physical.insert(id);
                        }
                    } else {
                        unknown = true;
                    }
                }
                DownloadTransferSnapshot {
                    id: t.id.to_string(),
                    identity_kind: if metadata.physical_id.is_some() {
                        "physical_h2"
                    } else {
                        "http_request"
                    },
                    physical_id: metadata.physical_id,
                    protocol: metadata.protocol.clone(),
                    remote_addr: metadata.remote_addr.map(|a| a.to_string()),
                    proxied: t.proxied,
                    state: match transfer_state {
                        0 => "connecting",
                        1 => "active",
                        2 => "completed",
                        _ => "closed",
                    },
                    received_bytes: bytes,
                    speed_bytes_per_second: rate.speed,
                    elapsed_ms: duration,
                }
            })
            .collect();
        let summary = DownloadFileSummary {
            id: self.id.to_string(),
            job_id: self.job_id.clone(),
            item_id: self.item_id.clone(),
            name: self.name.clone(),
            status: match status {
                0 => "active",
                1 => "completed",
                2 => "failed",
                _ => "canceled",
            },
            elapsed_ms: elapsed,
            network_elapsed_ms: {
                let start = self.network_started_ms.load(Ordering::Relaxed);
                (start != u64::MAX).then(|| {
                    let end = if active + connecting > 0 {
                        now
                    } else {
                        self.network_ended_ms.load(Ordering::Relaxed)
                    };
                    end.saturating_sub(start)
                })
            },
            received_bytes: self.received.load(Ordering::Relaxed),
            configured_limit: state.configured_limit,
            effective_cap: state.effective_cap,
            target_connections: state.target_connections,
            held_permits: state.held_permits,
            active_requests: active,
            effective_requests: effective,
            physical_connections: (!unknown).then_some(physical.len()),
            effective_physical_connections: (!unknown)
                .then_some(effective_physical.len()),
        };
        if state
            .last_sample_ms
            .is_none_or(|at| now.saturating_sub(at) >= 1000)
        {
            if state.samples.len() == MAX_SAMPLES {
                state.samples.pop_front();
            }
            let sample = DownloadFileSample {
                elapsed_ms: elapsed,
                received_bytes: summary.received_bytes,
                active_requests: active,
                effective_requests: effective,
                physical_connections: summary.physical_connections,
                effective_physical_connections: summary
                    .effective_physical_connections,
                effective_cap: state.effective_cap,
                target_connections: state.target_connections,
            };
            state.samples.push_back(sample);
            state.last_sample_ms = Some(now);
        }
        DownloadFileDetails {
            summary,
            transfers,
            samples: state.samples.iter().cloned().collect(),
        }
    }
}

pub(crate) struct TransferGuard {
    file: Arc<FileCapture>,
    transfer: Arc<Transfer>,
}

impl TransferGuard {
    pub(crate) fn connected(
        &self,
        version: reqwest::Version,
        peer: Option<SocketAddr>,
    ) {
        let mut metadata = self.transfer.metadata.lock();
        metadata.protocol = format!("{version:?}");
        metadata.remote_addr = peer;
        self.transfer.state.store(1, Ordering::Release);
    }

    pub(crate) fn received(&self, bytes: usize) {
        if self.file.current() {
            self.transfer
                .received
                .fetch_add(bytes as u64, Ordering::Relaxed);
            self.file
                .received
                .fetch_add(bytes as u64, Ordering::Relaxed);
            self.transfer.last_byte_ms.store(
                self.transfer.started.elapsed().as_millis() as u64,
                Ordering::Relaxed,
            );
        }
    }

    pub(crate) fn finish(&self) {
        self.close(2);
    }

    fn close(&self, status: u8) {
        if self.transfer.state.load(Ordering::Acquire) < 2 {
            self.transfer.ended_ms.store(
                self.transfer.started.elapsed().as_millis() as u64,
                Ordering::Relaxed,
            );
            self.transfer.state.store(status, Ordering::Release);
            self.file
                .network_ended_ms
                .fetch_max(self.file.elapsed(), Ordering::Relaxed);
        }
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        self.close(3);
    }
}

pub(crate) fn begin_request(
    request: &DownloadRequest,
    route: &DownloadRoute,
) -> Option<TransferGuard> {
    request.diagnostics.as_ref()?.begin_http_request(route)
}

impl FileCapture {
    pub(crate) fn begin_http_request(
        self: &Arc<Self>,
        route: &DownloadRoute,
    ) -> Option<TransferGuard> {
        self.begin(None, "pending", None, route_is_proxied(route))
    }
}

pub(crate) fn begin_h2(
    request: &DownloadRequest,
    route: &DownloadRoute,
    connection: &super::h2_pool::SharedH2Connection,
) -> Option<TransferGuard> {
    record_single_budget(request);
    request.diagnostics.as_ref()?.begin(
        Some(connection.connection_id),
        "HTTP/2.0",
        connection.remote_addr,
        route_is_proxied(route),
    )
}

pub(crate) fn route_is_proxied(route: &DownloadRoute) -> bool {
    if route.proxy == ProxyPolicy::Direct {
        return false;
    }
    let Some(matcher) = super::proxy_context::clients()
        .map(|clients| clients.native_proxy)
        .or_else(|| {
            crate::util::proxy::ProxyConfig::default()
                .native_matcher()
                .ok()
        })
    else {
        return false;
    };
    route
        .url
        .parse::<http::Uri>()
        .ok()
        .is_some_and(|uri| matcher.intercept(&uri).is_some())
}

pub(crate) fn record_single_budget(request: &DownloadRequest) {
    if let Some(file) = &request.diagnostics
        && file.current()
    {
        let mut state = file.state.lock();
        state.configured_limit = crate::State::get_if_initialized()
            .map(|state| state.download_concurrency());
        state.effective_cap = Some(1);
        state.target_connections = Some(1);
        state.held_permits = Some(1);
    }
}

pub(crate) fn begin_range(
    request: &DownloadRequest,
    physical_id: Option<u64>,
    peer: Option<SocketAddr>,
    h2: bool,
    proxied: bool,
) -> Option<TransferGuard> {
    request.diagnostics.as_ref()?.begin(
        physical_id,
        if h2 { "HTTP/2.0" } else { "HTTP/1.1" },
        peer,
        proxied,
    )
}

pub(crate) fn record_budget(
    request: &DownloadRequest,
    configured: usize,
    cap: usize,
    target: usize,
    held: usize,
) {
    if let Some(file) = &request.diagnostics
        && file.current()
    {
        let mut state = file.state.lock();
        state.configured_limit = Some(configured);
        state.effective_cap = Some(cap);
        state.target_connections = Some(target);
        state.held_permits = Some(held);
    }
}

fn capture_files(job_id: Option<&str>) -> (Vec<Arc<FileCapture>>, u64, u64) {
    let store = STORE.lock();
    let files = if enabled() {
        store
            .files
            .iter()
            .rev()
            .filter(|file| {
                job_id.is_none_or(|id| file.job_id.as_deref() == Some(id))
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    (files, store.dropped_files, store.dropped_transfers)
}

fn pool_available() -> Option<usize> {
    crate::State::get_if_initialized()
        .map(|state| state.download_semaphore.0.available_permits())
}

pub fn snapshot(
    job_id: Option<&str>,
    file_id: Option<&str>,
) -> DownloadDiagnosticsSnapshot {
    let (files, dropped_files, dropped_transfers) = capture_files(job_id);
    let mut selected_file = None;
    let summaries = files
        .iter()
        .map(|file| {
            let details = file.details();
            let summary = details.summary.clone();
            if file_id == Some(summary.id.as_str()) {
                selected_file = Some(details);
            }
            summary
        })
        .collect();
    DownloadDiagnosticsSnapshot {
        enabled: enabled(),
        pool_available: pool_available(),
        files: summaries,
        selected_file,
        dropped_files,
        dropped_transfers,
    }
}

pub fn export_snapshot(job_id: Option<&str>) -> DownloadDiagnosticsExport {
    let (files, dropped_files, dropped_transfers) = capture_files(job_id);
    DownloadDiagnosticsExport {
        schema_version: 1,
        enabled: enabled(),
        pool_available: pool_available(),
        files: files.iter().map(|file| file.details()).collect(),
        dropped_files,
        dropped_transfers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capture_is_optional_bounded_and_never_calls_requests_physical_connections()
     {
        let _lock = TEST_LOCK.lock().await;
        set_enabled(false);
        let mut request = DownloadRequest::new(
            "https://example.invalid/file?token=secret",
            crate::util::fetch::ResourceClass::Modrinth,
        );
        assert!(
            begin_file(&mut request, Path::new("private/file.jar")).is_none()
        );
        set_enabled(true);
        let file =
            begin_file(&mut request, Path::new("private/file.jar")).unwrap();
        let route = crate::util::fetch::DownloadRoute {
            url: request.url.clone(),
            source: crate::util::fetch::DownloadRouteSource::Official,
            is_mirror: false,
            allow_sensitive_headers: false,
            supports_range: true,
            proxy: ProxyPolicy::Direct,
        };
        let transfer = begin_request(&request, &route).unwrap();
        transfer.connected(
            reqwest::Version::HTTP_11,
            Some("127.0.0.1:443".parse().unwrap()),
        );
        transfer.received(1024);
        let view = snapshot(None, None);
        let id = view.files[0].id.clone();
        assert_eq!(view.files[0].active_requests, 1);
        assert_eq!(view.files[0].physical_connections, None);
        assert!(view.selected_file.is_none());
        assert_eq!(
            snapshot(None, Some(&id)).selected_file.unwrap().transfers[0]
                .speed_bytes_per_second,
            None
        );
        for _ in 0..MAX_FILE_TRANSFERS + 2 {
            drop(begin_request(&request, &route));
        }
        let details = snapshot(None, Some(&id)).selected_file.unwrap();
        assert_eq!(details.transfers.len(), MAX_FILE_TRANSFERS);
        assert!(
            details
                .transfers
                .iter()
                .any(|item| item.id == transfer.transfer.id.to_string())
        );
        assert_eq!(
            details.summary.active_requests, 1,
            "an old slow active request must survive history eviction"
        );
        assert_eq!(details.summary.received_bytes, 1024);
        drop(transfer);
        assert_eq!(snapshot(None, None).files[0].active_requests, 0);
        let busy = (0..MAX_FILE_TRANSFERS)
            .map(|_| begin_request(&request, &route).unwrap())
            .collect::<Vec<_>>();
        assert!(
            begin_request(&request, &route).is_none(),
            "full active history must reject new observations, never evict a live request"
        );
        assert!(snapshot(None, None).dropped_transfers > 0);
        drop(busy);
        let capture = request.diagnostics.as_ref().unwrap();
        let first = capture.begin(Some(17), "HTTP/2.0", None, false).unwrap();
        let second = capture.begin(Some(17), "HTTP/2.0", None, false).unwrap();
        first.received(10);
        second.received(20);
        assert_eq!(snapshot(None, None).files[0].physical_connections, Some(1));
        assert_eq!(
            snapshot(None, None).files[0].effective_physical_connections,
            Some(1)
        );
        first.finish();
        second.finish();
        file.finish(true);
        let serialized = serde_json::to_string(&export_snapshot(None)).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("private"));
        assert_eq!(
            safe_item("C:\\Users\\private\\pack.jar?token=secret"),
            "pack.jar"
        );
        set_enabled(false);
        assert!(snapshot(None, None).files.is_empty());
        set_enabled(true);
        first.received(100);
        assert!(snapshot(None, None).files.is_empty());
        set_enabled(false);
    }
}
