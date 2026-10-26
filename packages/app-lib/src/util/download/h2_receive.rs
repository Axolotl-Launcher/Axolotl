//! Shared receive-side accounting for native HTTP/2 streams.

use crate::util::download::constants::{
    ADAPTIVE_CONNECTION_WINDOW, NATIVE_SLOW_COLD_SPEED_FLOOR,
};
use bytes::Bytes;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

use super::constants::{
    MAX_PROGRESS_WAIT, MIN_PROGRESS_BYTES, STREAM_RECV_TIMEOUT,
};

/// Recent aggregate socket rate, independent of one multiplexed stream.
/// Reset every window so a fast prefix cannot hide a later slow tail.
pub(crate) struct ConnectionRateWindow {
    started_at: Instant,
    start_bytes: usize,
    rate: Option<u64>,
}

impl ConnectionRateWindow {
    pub(crate) fn new(bytes: usize, now: Instant) -> Self {
        Self {
            started_at: now,
            start_bytes: bytes,
            rate: None,
        }
    }

    pub(crate) fn observe(
        &mut self,
        bytes: usize,
        now: Instant,
    ) -> Option<u64> {
        if now < self.started_at || bytes < self.start_bytes {
            *self = Self::new(bytes, now);
            return None;
        }
        let elapsed = now.duration_since(self.started_at);
        if elapsed >= ADAPTIVE_CONNECTION_WINDOW {
            self.rate = Some(
                ((bytes - self.start_bytes) as f64 / elapsed.as_secs_f64())
                    as u64,
            );
            self.started_at = now;
            self.start_bytes = bytes;
        }
        self.rate
    }
}

pub(crate) fn endpoint_speed(
    stream_speed: u64,
    active_streams: usize,
    aggregate: Option<u64>,
) -> Option<u64> {
    if active_streams <= 1 {
        return Some(stream_speed);
    }
    aggregate.filter(|speed| *speed < NATIVE_SLOW_COLD_SPEED_FLOOR)
}

/// A file can leave a congested shared socket without declaring its siblings
/// or the authority unhealthy. Only count windows with spare physical slots.
pub(crate) struct BatchTailWindow {
    started_at: Instant,
    start_bytes: u64,
}

impl BatchTailWindow {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            started_at: now,
            start_bytes: 0,
        }
    }

    pub(crate) fn observe(
        &mut self,
        downloaded: u64,
        can_recover: bool,
        now: Instant,
    ) -> bool {
        if !can_recover
            || now < self.started_at
            || downloaded < self.start_bytes
        {
            self.started_at = now;
            self.start_bytes = downloaded;
            return false;
        }
        let elapsed = now.duration_since(self.started_at);
        if elapsed < super::constants::BATCH_TAIL_RECOVERY_WINDOW {
            return false;
        }
        let speed = ((downloaded - self.start_bytes) as f64
            / elapsed.as_secs_f64()) as u64;
        self.started_at = now;
        self.start_bytes = downloaded;
        speed < NATIVE_SLOW_COLD_SPEED_FLOOR
    }
}

/// Tracks one active H2 stream in the native automatic-concurrency metrics.
pub(crate) struct H2TransferActivity {
    _activity: Option<crate::state::DownloadConnectionActivity>,
}

impl H2TransferActivity {
    pub(crate) fn begin() -> Self {
        Self {
            _activity: crate::State::get_if_initialized()
                .map(|state| state.begin_download_connection()),
        }
    }

    pub(crate) fn record_bytes(&self, bytes: usize) {
        if let Some(state) = crate::State::get_if_initialized() {
            state.record_download_bytes(bytes as u64);
        }
    }
}

/// Limits install-progress work before it reaches the shared reporter lock.
pub(crate) struct H2ProgressGate {
    last_reported: u64,
    last_reported_at: Instant,
    threshold: u64,
}

impl H2ProgressGate {
    pub(crate) fn new(total_size: u64) -> Self {
        Self::with_starting_bytes(total_size, 0)
    }

    pub(crate) fn with_starting_bytes(
        total_size: u64,
        starting_bytes: u64,
    ) -> Self {
        Self {
            last_reported: starting_bytes,
            last_reported_at: Instant::now(),
            threshold: MIN_PROGRESS_BYTES.max(total_size / 200),
        }
    }

    pub(crate) fn should_report(
        &mut self,
        downloaded: u64,
        total_size: u64,
    ) -> bool {
        if downloaded < total_size
            && downloaded.saturating_sub(self.last_reported) < self.threshold
            && self.last_reported_at.elapsed() < MAX_PROGRESS_WAIT
        {
            return false;
        }
        if downloaded <= self.last_reported {
            return false;
        }
        self.last_reported = downloaded;
        self.last_reported_at = Instant::now();
        true
    }
}

pub(crate) async fn receive_chunk(
    stream: &mut h2::RecvStream,
    context: &str,
) -> crate::Result<Option<Bytes>> {
    tokio::time::timeout(STREAM_RECV_TIMEOUT, stream.data())
        .await
        .map_err(|_| crate::ErrorKind::DownloadStreamStalled {
            context: context.to_string(),
        })?
        .transpose()
        .map_err(|error| crate::ErrorKind::DownloadH2Error(error).into())
}

/// Returns receive-window capacity after the caller has consumed the frame.
pub(crate) fn release_capacity(
    stream: &mut h2::RecvStream,
    bytes: usize,
) -> crate::Result<()> {
    stream
        .flow_control()
        .release_capacity(bytes)
        .map_err(|error| {
            crate::ErrorKind::NetworkError(format!(
                "HTTP/2 flow-control release failed: {error}"
            ))
            .into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::poll_fn;
    use http::{Request, Response};

    #[test]
    fn tail_floor_waits_for_a_sustained_slow_window_and_resets_under_load() {
        let now = Instant::now();
        let mut window = BatchTailWindow::new(now);
        assert!(!window.observe(100_000, true, now + Duration::from_secs(1)));
        assert!(window.observe(150_000, true, now + Duration::from_secs(3)));
        assert!(!window.observe(160_000, false, now + Duration::from_secs(4)));
        assert!(!window.observe(170_000, true, now + Duration::from_secs(6)));
        assert!(window.observe(180_000, true, now + Duration::from_secs(7)));
        assert!(!window.observe(
            4_000_000,
            true,
            now + Duration::from_secs(10)
        ));
    }

    #[test]
    fn aggregate_windows_expire_fast_prefix_and_protect_healthy_siblings() {
        let now = Instant::now();
        let mut window = ConnectionRateWindow::new(0, now);
        assert_eq!(
            window.observe(30 * 1024 * 1024, now + ADAPTIVE_CONNECTION_WINDOW),
            Some(10 * 1024 * 1024)
        );
        assert_eq!(endpoint_speed(45 * 1024, 2, window.rate), None);
        assert_eq!(endpoint_speed(45 * 1024, 2, None), None);
        assert_eq!(endpoint_speed(45 * 1024, 1, window.rate), Some(45 * 1024));
        let slow = window.observe(
            30 * 1024 * 1024 + 3 * 90 * 1024,
            now + Duration::from_secs(6),
        );
        assert_eq!(slow, Some(90 * 1024));
        assert_eq!(endpoint_speed(45 * 1024, 2, slow), slow);
        assert_eq!(window.observe(1, now + Duration::from_secs(7)), None);
    }

    #[test]
    fn progress_gate_throttles_intermediate_updates_and_reports_completion() {
        let total = 2 * 1024 * 1024;
        let mut gate = H2ProgressGate::new(total);

        assert!(!gate.should_report(64 * 1024, total));
        assert!(gate.should_report(256 * 1024, total));
        assert!(!gate.should_report(320 * 1024, total));
        assert!(gate.should_report(total, total));
        assert!(!gate.should_report(total, total));
    }

    #[test]
    fn resumed_progress_waits_for_new_bytes_and_reports_on_time() {
        let total = 2 * 1024 * 1024;
        let starting = 1024 * 1024;
        let mut gate = H2ProgressGate::with_starting_bytes(total, starting);
        assert!(!gate.should_report(starting + 4096, total));
        gate.last_reported_at = Instant::now() - MAX_PROGRESS_WAIT;
        assert!(!gate.should_report(starting, total));
        assert!(gate.should_report(starting + 4096, total));
    }

    #[test]
    fn progress_gate_reports_slow_transfers_on_time() {
        let total = 100 * 1024 * 1024;
        let mut gate = H2ProgressGate::new(total);

        assert!(!gate.should_report(100, total));
        gate.last_reported_at = Instant::now() - MAX_PROGRESS_WAIT;
        assert!(gate.should_report(100, total));
        assert!(!gate.should_report(200, total));
        gate.last_reported_at = Instant::now() - MAX_PROGRESS_WAIT;
        assert!(!gate.should_report(100, total));
        assert!(gate.should_report(200, total));
        assert!(gate.should_report(total, total));
    }

    #[tokio::test]
    async fn receive_capacity_allows_body_larger_than_initial_window() {
        use crate::util::download::constants::TEST_BODY_SIZE as BODY_SIZE;

        use crate::util::download::constants::TEST_FRAME_SIZE as FRAME_SIZE;

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move {
            let mut connection =
                h2::server::handshake(server_io).await.unwrap();
            let (_, mut respond) = connection.accept().await.unwrap().unwrap();
            // Keep polling the connection while the response body waits for
            // client WINDOW_UPDATE frames.
            tokio::spawn(async move {
                while connection.accept().await.is_some() {}
            });
            let mut stream =
                respond.send_response(Response::new(()), false).unwrap();
            let mut remaining = BODY_SIZE;
            while remaining > 0 {
                stream.reserve_capacity(remaining.min(FRAME_SIZE));
                let capacity = poll_fn(|cx| stream.poll_capacity(cx))
                    .await
                    .unwrap()
                    .unwrap();
                let length = capacity.min(remaining).min(FRAME_SIZE);
                remaining -= length;
                stream
                    .send_data(Bytes::from(vec![0x5a; length]), remaining == 0)
                    .unwrap();
            }
        });

        let mut builder = h2::client::Builder::new();
        builder
            .initial_window_size(1024 * 1024)
            .initial_connection_window_size(64 * 1024 * 1024);
        let (mut sender, mut connection) =
            builder.handshake::<_, Bytes>(client_io).await.unwrap();
        connection.set_target_window_size(64 * 1024 * 1024);
        let client_driver = tokio::spawn(connection);

        let request =
            Request::get("https://h2-flow.test/file").body(()).unwrap();
        sender = sender.ready().await.unwrap();
        let (response, _) = sender.send_request(request, true).unwrap();
        let mut body = response.await.unwrap().into_body();
        let mut received = 0;
        while let Some(chunk) = receive_chunk(&mut body, "test").await.unwrap()
        {
            assert!(chunk.iter().all(|byte| *byte == 0x5a));
            received += chunk.len();
            release_capacity(&mut body, chunk.len()).unwrap();
        }

        assert_eq!(received, BODY_SIZE);
        server.await.unwrap();
        client_driver.abort();
    }
}
