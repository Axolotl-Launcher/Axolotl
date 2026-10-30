#[cfg(test)]
use super::BENCHMARK_TIMING;
#[cfg(test)]
use super::file;
use super::file::{remove_if_exists, suffixed_path};
use super::model::{DownloadRequest, DownloadRoute};
#[cfg(test)]
use super::request;
#[cfg(test)]
use super::routes::route;
use super::tail_candidate_path;
use super::throttle::FetchSemaphore;
use crate::util::download::constants::{
    ADAPTIVE_RANGE_BYTES, BULK_ADAPTIVE_RANGE_BYTES,
    BULK_DOWNLOAD_RESERVED_PERMITS, BULK_SEGMENTED_DOWNLOAD_THRESHOLD,
    DOWNLOAD_FAIR_SHARE_DIVISOR, DOWNLOAD_PERCENT_BASE,
    DOWNLOAD_PROGRESS_MIN_BYTES, DOWNLOAD_RANGE_CONNECTION_FLOOR,
    DOWNLOAD_RANGE_TARGET_TIME, FETCH_EFFECTIVE_CONCURRENCY_CAP_REACHED,
    FETCH_EXPANSION_COOLDOWN_ACTIVE,
    FETCH_FETCH_PERMIT_BATCH_HAS_ENOUGH_PERMITS,
    FETCH_INSUFFICIENT_AGGREGATE_SPEED_SAMPLES,
    FETCH_NO_GLOBAL_PERMIT_AVAILABLE, FETCH_TOO_LITTLE_DATA_REMAINS,
    FILE_TRANSFER_FIRST_BYTE_TIMEOUT, INITIAL_BULK_SEGMENT_CONCURRENCY,
    INITIAL_SEGMENT_CONCURRENCY, MAX_SEGMENT_CONCURRENCY,
    MAX_TAIL_HEDGES_PER_FILE, MIN_PARALLEL_RANGES, MIN_SEGMENT_SIZE,
    RANGE_FULL_FAILURE_LIMIT, RANGE_IDLE_RECONNECT_TIMEOUT,
    RANGE_REDUCED_CONCURRENCY, RANGE_REDUCED_FAILURE_LIMIT,
    REASSIGNABLE_FIRST_BYTE_TIMEOUT, SEGMENT_EXPANSION_GAIN_PERCENT,
    SEGMENT_EXPANSION_RETRY_SPEED_PERCENT, SEGMENT_EXPANSION_SAMPLE_COUNT,
    SEGMENTED_DOWNLOAD_THRESHOLD, TAIL_HEDGE_IDLE_TIMEOUT, TAIL_SPLIT_PERCENT,
};
use crate::util::download_manager::SpeedSnapshot;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{self, Instant};
use tokio::sync::SemaphorePermit;

#[derive(Clone)]
pub(crate) struct DownloadRange {
    pub(crate) index: usize,
    pub(crate) start: u64,
    pub(crate) state: Arc<Mutex<DownloadRangeState>>,
}

pub(crate) struct DownloadRangeState {
    pub(crate) end: u64,
    pub(crate) downloaded: u64,
    pub(crate) active: bool,
}

impl DownloadRange {
    pub(crate) fn new(index: usize, start: u64, end: u64) -> Self {
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

    pub(crate) fn end(&self) -> u64 {
        self.state.lock().end
    }

    pub(crate) fn remaining(&self) -> u64 {
        let state = self.state.lock();
        state
            .end
            .saturating_add(1)
            .saturating_sub(self.start.saturating_add(state.downloaded))
    }

    pub(crate) fn is_active(&self) -> bool {
        self.state.lock().active
    }

    pub(crate) fn split_tail(&self, index: usize) -> Option<Self> {
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

    pub(crate) fn accept_chunk(&self, chunk_size: usize) -> (usize, bool) {
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

    pub(crate) fn finish(&self) -> bool {
        let mut state = self.state.lock();
        state.active = false;
        state.downloaded == state.end - self.start + 1
    }
}

pub(crate) struct DownloadRangeGuard(pub(crate) Arc<Mutex<DownloadRangeState>>);

impl Drop for DownloadRangeGuard {
    fn drop(&mut self) {
        self.0.lock().active = false;
    }
}

pub(crate) struct SegmentDownloadCompletion {
    pub(crate) final_url: String,
    pub(crate) is_first_range: bool,
    pub(crate) ttfb: time::Duration,
    pub(crate) remote_addr: Option<std::net::SocketAddr>,
    pub(crate) http_version: Option<reqwest::Version>,
}

pub(crate) struct SegmentCleanupGuard {
    part_path: PathBuf,
    armed: bool,
    preserved_prefix: u64,
}

impl SegmentCleanupGuard {
    pub(crate) fn new(part_path: &Path) -> Self {
        Self {
            part_path: part_path.to_path_buf(),
            armed: true,
            preserved_prefix: 0,
        }
    }

    pub(crate) fn preserve_prefix(mut self, bytes: u64) -> Self {
        self.preserved_prefix = bytes;
        self
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for SegmentCleanupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if self.preserved_prefix > 0 {
            // Concurrent ranges may leave holes. Only the original contiguous
            // prefix remains resumable after cancellation or a failed trial.
            if let Ok(file) = std::fs::OpenOptions::new()
                .write(true)
                .open(&self.part_path)
            {
                let _ = file.set_len(self.preserved_prefix);
            }
        } else {
            let _ = std::fs::remove_file(&self.part_path);
        }
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

pub(crate) fn segmented_concurrency_cap(available_permits: usize) -> usize {
    available_permits.min(MAX_SEGMENT_CONCURRENCY)
}

pub(crate) fn route_segmented_concurrency_cap(
    route: &DownloadRoute,
    available_permits: usize,
    size: u64,
) -> usize {
    let fair_share = if size >= BULK_SEGMENTED_DOWNLOAD_THRESHOLD {
        let reserved = BULK_DOWNLOAD_RESERVED_PERMITS
            .min(available_permits / DOWNLOAD_FAIR_SHARE_DIVISOR);
        available_permits.saturating_sub(reserved).max(1)
    } else {
        (available_permits / DOWNLOAD_FAIR_SHARE_DIVISOR).max(1)
    };

    segmented_concurrency_cap(available_permits)
        .min(fair_share)
        .min(crate::util::download::native_budget::capacity(route))
}

pub(crate) async fn acquire_initial_segment_permits<'a>(
    route: &DownloadRoute,
    semaphore: &'a FetchSemaphore,
    count: usize,
) -> crate::Result<Vec<NativeConnectionPermit<'a>>> {
    let queue_started = Instant::now();
    // Start with the available share instead of waiting for an entire batch.
    // The scheduler retains its configured ceiling and grows as slots return.
    let count = count
        .min(crate::util::download::native_budget::available(route).max(1))
        .min(semaphore.0.available_permits().max(1));
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

pub(crate) fn try_acquire_native_connection<'a>(
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

pub(crate) fn initial_segment_count(
    size: u64,
    available_permits: usize,
) -> usize {
    let size_limit = usize::try_from(size / MIN_SEGMENT_SIZE)
        .unwrap_or(usize::MAX)
        .max(1);
    let initial = if size >= BULK_SEGMENTED_DOWNLOAD_THRESHOLD {
        INITIAL_BULK_SEGMENT_CONCURRENCY
    } else {
        INITIAL_SEGMENT_CONCURRENCY
    };
    initial
        .min(segmented_concurrency_cap(available_permits))
        .min(size_limit)
}

pub(crate) fn adaptive_range_cap(
    size: u64,
    route_cap: usize,
    disk_cap: usize,
    consecutive_failures: u32,
    tail_recovery: bool,
) -> usize {
    let range_bytes = if tail_recovery {
        MIN_SEGMENT_SIZE
    } else if size >= BULK_SEGMENTED_DOWNLOAD_THRESHOLD {
        BULK_ADAPTIVE_RANGE_BYTES
    } else {
        ADAPTIVE_RANGE_BYTES
    };
    let size_cap = (size / range_bytes)
        .max(1)
        .min(MAX_SEGMENT_CONCURRENCY as u64) as usize;
    let failure_cap = match consecutive_failures {
        0..=RANGE_FULL_FAILURE_LIMIT => MAX_SEGMENT_CONCURRENCY,
        2..=RANGE_REDUCED_FAILURE_LIMIT => RANGE_REDUCED_CONCURRENCY,
        _ => 1,
    };
    route_cap.min(size_cap).min(disk_cap).min(failure_cap)
}

pub(crate) fn create_remaining_ranges(
    size: u64,
    prefix: u64,
    count: usize,
) -> Vec<DownloadRange> {
    let remaining = size - prefix;
    let base_size = remaining / count as u64;
    let remainder = remaining % count as u64;
    let mut start = prefix;
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

pub(crate) fn range_growth_target(
    active: usize,
    cap: usize,
    retained: usize,
    exhausted: bool,
    remaining: u64,
    speed: u64,
) -> usize {
    if exhausted {
        retained
    } else {
        let deadline_target = if speed == 0 {
            active
        } else {
            let budget = u128::from(speed)
                * u128::from(DOWNLOAD_RANGE_TARGET_TIME.as_secs());
            let needed =
                (u128::from(remaining) * active as u128).div_ceil(budget);
            usize::try_from(needed).unwrap_or(MAX_SEGMENT_CONCURRENCY)
        };
        active.saturating_mul(2).max(deadline_target)
    }
    .min(cap)
}

pub(crate) fn range_growth_can_retry(previous: u64, current: u64) -> bool {
    previous > 0
        && u128::from(current) * u128::from(DOWNLOAD_PERCENT_BASE)
            < u128::from(previous) * SEGMENT_EXPANSION_RETRY_SPEED_PERCENT
}

pub(crate) fn requested_range_floor(
    request: &DownloadRequest,
    size: u64,
    remaining: u64,
    cap: usize,
) -> usize {
    request.minimum_range_connections(size).min(cap).min(
        usize::try_from(remaining / MIN_SEGMENT_SIZE)
            .unwrap_or(usize::MAX)
            .max(1),
    )
}

pub(crate) fn range_connection_floor(
    size: u64,
    remaining: u64,
    cap: usize,
    tail_recovery: bool,
) -> usize {
    if !tail_recovery && size < BULK_SEGMENTED_DOWNLOAD_THRESHOLD {
        return 1;
    }
    DOWNLOAD_RANGE_CONNECTION_FLOOR.min(cap).min(
        usize::try_from(remaining / MIN_SEGMENT_SIZE)
            .unwrap_or(usize::MAX)
            .max(1),
    )
}

pub(crate) struct RangeExpansionBaseline {
    pub(crate) speed: u64,
    pub(crate) completed_ranges: usize,
    pub(crate) previous_target: usize,
}

impl RangeExpansionBaseline {
    pub(crate) fn gained_throughput(
        &self,
        speed: u64,
        completed_ranges: usize,
    ) -> Option<bool> {
        // A smaller active set naturally has less aggregate throughput. It
        // cannot establish that the new connections made the transfer slower.
        (completed_ranges == self.completed_ranges).then(|| {
            u128::from(speed) * u128::from(DOWNLOAD_PERCENT_BASE)
                >= u128::from(self.speed) * SEGMENT_EXPANSION_GAIN_PERCENT
        })
    }
}

pub(crate) fn expansion_block_reason(
    snapshot: SpeedSnapshot,
    active_ranges: usize,
    concurrency_cap: usize,
    available_permits: usize,
    remaining_bytes: u64,
    elapsed_since_expansion: time::Duration,
    minimum_interval: time::Duration,
) -> Option<&'static str> {
    if active_ranges >= concurrency_cap {
        return Some(FETCH_EFFECTIVE_CONCURRENCY_CAP_REACHED);
    }
    if available_permits == 0 {
        return Some(FETCH_NO_GLOBAL_PERMIT_AVAILABLE);
    }
    if elapsed_since_expansion < minimum_interval {
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

pub(crate) fn allow_low_throughput_route_switch(
    has_alternate_route: bool,
    retry_with_single_thread: bool,
) -> bool {
    has_alternate_route && !retry_with_single_thread
}

pub(crate) fn should_downgrade_slow_http2(
    http_version: reqwest::Version,
    retry_with_single_thread: bool,
) -> bool {
    http_version == reqwest::Version::HTTP_2 && !retry_with_single_thread
}

pub(crate) fn native_first_byte_timeout(
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

pub(crate) fn should_use_segmented_download(
    size: u64,
    resumable_part_bytes: u64,
) -> bool {
    size.saturating_sub(resumable_part_bytes) >= SEGMENTED_DOWNLOAD_THRESHOLD
}

pub(crate) fn range_idle_timeout(tail: bool) -> time::Duration {
    #[cfg(test)]
    if BENCHMARK_TIMING.try_with(|_| ()).is_ok() {
        return time::Duration::from_secs(if tail { 5 } else { 8 });
    }
    if tail {
        TAIL_HEDGE_IDLE_TIMEOUT
    } else {
        RANGE_IDLE_RECONNECT_TIMEOUT
    }
}

pub(crate) fn can_recover_file_ranges(
    request: &DownloadRequest,
    remaining: u64,
) -> bool {
    request.supports_range_recovery()
        && remaining >= MIN_SEGMENT_SIZE * MIN_PARALLEL_RANGES as u64
}

pub(crate) fn segment_path(part_path: &Path, index: usize) -> PathBuf {
    suffixed_path(part_path, &format!(".segment-{index}"))
}

pub(crate) async fn cleanup_segment_files(
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
