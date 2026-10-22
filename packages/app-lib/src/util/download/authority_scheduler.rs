use super::endpoint_selection::tuning::{
    TRIALS, TrialKind, TrialLease, TrialRegistry,
};
use super::h2_pool::SharedH2Connection;
use crate::util::download::constants::{
    ADAPTIVE_CONNECTION_GAIN_PERCENT, ADAPTIVE_CONNECTION_INITIAL_TARGET,
    ADAPTIVE_CONNECTION_MAX_TARGET, ADAPTIVE_CONNECTION_PERCENT_BASE,
    ADAPTIVE_CONNECTION_RATE_SPREAD, ADAPTIVE_CONNECTION_TRIAL_TIMEOUT,
    ADAPTIVE_CONNECTION_TRIAL_WINDOWS, CONNECTION_FAILURE_COOLDOWN_WINDOWS,
    CONNECTION_GROWTH_FACTOR,
};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

pub(crate) use super::constants::{
    ADAPTIVE_CONNECTION_COOLDOWN as ADAPTIVE_COOLDOWN,
    ADAPTIVE_CONNECTION_WINDOW as ADAPTIVE_WINDOW,
    CONNECTION_STABILITY_WINDOW as STABILITY_WINDOW, INITIAL_CONNECTION_TARGET,
    MAX_CONNECTION_TARGET,
};

fn next_target(current: usize) -> usize {
    (current * CONNECTION_GROWTH_FACTOR).min(MAX_CONNECTION_TARGET)
}

pub(crate) struct AuthorityScheduler {
    adaptive: Option<AdaptiveConnections>,
    byte_samples: HashMap<u64, usize>,
    monotonic_bytes: u64,
    connections: Vec<Arc<SharedH2Connection>>,
    target: usize,
    stable_since: Option<Instant>,
    cooldown_until: Option<Instant>,
    last_byte_sample: usize,
    last_byte_sample_at: Instant,
    connecting: Arc<tokio::sync::Semaphore>,
}

impl Default for AuthorityScheduler {
    fn default() -> Self {
        Self {
            adaptive: None,
            byte_samples: HashMap::new(),
            monotonic_bytes: 0,
            connections: Vec::new(),
            target: INITIAL_CONNECTION_TARGET,
            stable_since: None,
            cooldown_until: None,
            last_byte_sample: 0,
            last_byte_sample_at: Instant::now(),
            connecting: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}

impl AuthorityScheduler {
    pub(crate) fn observe_background(&mut self, now: Instant, pressure: bool) {
        if self.adaptive.is_none() {
            return;
        }
        self.prune();
        if pressure {
            self.suspend_adaptive(now);
        }
        self.observe(now);
    }

    pub(crate) fn prune(&mut self) {
        self.sample_connection_bytes();
        self.connections.retain(|connection| {
            if connection.is_retiring() {
                return false;
            }
            if connection.is_dead()
                || connection.is_idle_expired_for_scheduler()
            {
                connection.evict_for_scheduler();
                false
            } else {
                true
            }
        });
        if self.connections.is_empty() {
            self.target = if self.adaptive.is_some() {
                ADAPTIVE_CONNECTION_INITIAL_TARGET
            } else {
                INITIAL_CONNECTION_TARGET
            };
            self.stable_since = None;
        }
    }

    pub(crate) fn add_connection(
        &mut self,
        connection: Arc<SharedH2Connection>,
    ) {
        if !connection.is_dead()
            && !connection.is_retiring()
            && !self
                .connections
                .iter()
                .any(|existing| Arc::ptr_eq(existing, &connection))
        {
            self.byte_samples.insert(
                connection.connection_id,
                connection.bytes_transferred_for_scheduler(),
            );
            self.connections.push(connection);
        }
    }

    pub(crate) fn least_loaded(&self) -> Option<Arc<SharedH2Connection>> {
        self.connections
            .iter()
            .filter(|connection| {
                !connection.is_dead() && !connection.is_retiring()
            })
            .min_by_key(|connection| {
                connection.assigned_streams_for_scheduler()
            })
            .cloned()
    }

    pub(crate) fn select_connection(
        &mut self,
    ) -> Option<Arc<SharedH2Connection>> {
        let connection = self.least_loaded()?;
        connection.reserve_stream_for_scheduler();
        Some(connection)
    }

    pub(crate) fn observe(&mut self, now: Instant) {
        self.sample_connection_bytes();
        if let Some(adaptive) = &mut self.adaptive {
            let busy = self.connections.iter().all(|connection| {
                connection.healthy_for_scheduler()
                    && connection.assigned_streams_for_scheduler() > 0
            }) && self
                .connections
                .iter()
                .map(|c| c.assigned_streams_for_scheduler())
                .sum::<usize>()
                > self.connections.len();
            let healthy = self
                .connections
                .iter()
                .all(|connection| connection.healthy_for_scheduler());
            let retire = adaptive.observe(
                now,
                self.monotonic_bytes,
                self.connections.len(),
                busy,
                healthy,
            );
            self.target = adaptive.target;
            if retire && let Some(connection) = self.connections.pop() {
                connection.retire();
            }
            return;
        }
        let total_bytes = self
            .connections
            .iter()
            .map(|connection| connection.bytes_transferred_for_scheduler())
            .sum();
        if now.duration_since(self.last_byte_sample_at) >= STABILITY_WINDOW {
            if total_bytes == self.last_byte_sample {
                self.stable_since = None;
                self.last_byte_sample_at = now;
                return;
            }
            self.last_byte_sample = total_bytes;
            self.last_byte_sample_at = now;
        }
        let Some(since) = self.stable_since else {
            self.stable_since = Some(now);
            return;
        };
        if self.connections.len() >= self.target
            && self.connections.iter().all(|connection| {
                connection.healthy_for_scheduler()
                    && connection.assigned_streams_for_scheduler() > 0
            })
            && now.duration_since(since) >= STABILITY_WINDOW
        {
            self.target = next_target(self.target);
            self.stable_since = None;
        }
    }

    pub(crate) fn target(&self) -> usize {
        self.target
    }

    pub(crate) fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub(crate) fn connections_for_eviction(
        &self,
    ) -> &[Arc<SharedH2Connection>] {
        &self.connections
    }

    pub(crate) fn can_expand(&self, now: Instant) -> bool {
        if let Some(adaptive) = &self.adaptive {
            let owns_trial = adaptive
                .trial
                .as_ref()
                .is_some_and(|trial| trial.lease.valid(now));
            if !owns_trial && TRIALS.snapshot(now).1.is_some() {
                return false;
            }
            if self
                .connections
                .iter()
                .map(|c| c.assigned_streams_for_scheduler())
                .sum::<usize>()
                <= self.connections.len()
            {
                return false;
            }
        }
        self.target <= MAX_CONNECTION_TARGET
            && self.connection_count() < self.target
            && self.cooldown_until.is_none_or(|until| now >= until)
    }

    pub(crate) fn reserve_connector(
        &self,
    ) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.connecting.clone().try_acquire_owned().ok()
    }

    pub(crate) fn rollback(&mut self, now: Instant) {
        if let Some(adaptive) = &mut self.adaptive {
            adaptive.rollback(now);
            self.target = adaptive.target;
            return;
        }
        self.target = (self.target / CONNECTION_GROWTH_FACTOR)
            .max(INITIAL_CONNECTION_TARGET);
        self.stable_since = None;
        self.cooldown_until =
            Some(now + STABILITY_WINDOW * CONNECTION_FAILURE_COOLDOWN_WINDOWS);
    }

    #[cfg(test)]
    pub(crate) fn targets() -> [usize; 4] {
        [4, 8, 16, 32]
    }

    pub(crate) fn enable_adaptive(&mut self, now: Instant) {
        if self.adaptive.is_none() {
            self.adaptive =
                Some(AdaptiveConnections::new(now, self.monotonic_bytes));
            self.target = ADAPTIVE_CONNECTION_INITIAL_TARGET;
        }
    }

    pub(crate) fn suspend_adaptive(&mut self, now: Instant) {
        if let Some(adaptive) = &mut self.adaptive {
            adaptive.rollback(now);
            self.target = adaptive.target;
            while self.connections.len() > self.target {
                self.connections.pop().unwrap().retire();
            }
        }
    }

    fn sample_connection_bytes(&mut self) {
        for connection in &self.connections {
            let current = connection.bytes_transferred_for_scheduler();
            let previous = self
                .byte_samples
                .entry(connection.connection_id)
                .or_insert(current);
            self.monotonic_bytes = self
                .monotonic_bytes
                .saturating_add(current.saturating_sub(*previous) as u64);
            *previous = current;
        }
        self.byte_samples.retain(|id, _| {
            self.connections
                .iter()
                .any(|connection| connection.connection_id == *id)
        });
    }
}

struct ConnectionTrial {
    lease: TrialLease,
    previous: usize,
    baseline_bps: u64,
    windows: u8,
    rate_sum: u64,
    attached: bool,
}

struct AdaptiveConnections {
    registry: Arc<TrialRegistry>,
    target: usize,
    sampled_at: Instant,
    sampled_bytes: u64,
    baseline_bps: Option<u64>,
    cooldown_until: Option<Instant>,
    trial: Option<ConnectionTrial>,
    revision: u64,
}

impl AdaptiveConnections {
    fn new(now: Instant, bytes: u64) -> Self {
        Self::with_registry(now, bytes, TRIALS.clone())
    }

    fn with_registry(
        now: Instant,
        bytes: u64,
        registry: Arc<TrialRegistry>,
    ) -> Self {
        let revision = registry.snapshot(now).0;
        Self {
            registry,
            target: ADAPTIVE_CONNECTION_INITIAL_TARGET,
            sampled_at: now,
            sampled_bytes: bytes,
            baseline_bps: None,
            cooldown_until: None,
            trial: None,
            revision,
        }
    }

    fn rollback(&mut self, now: Instant) {
        if let Some(trial) = self.trial.take() {
            self.target = trial.previous;
        }
        self.baseline_bps = None;
        self.cooldown_until = Some(now + ADAPTIVE_COOLDOWN);
    }

    /// Returns whether the added connection should retire. `bytes` is a
    /// monotonic delta sum, so pruning a connection cannot fabricate a rate.
    fn observe(
        &mut self,
        now: Instant,
        bytes: u64,
        count: usize,
        busy: bool,
        healthy: bool,
    ) -> bool {
        if self
            .trial
            .as_ref()
            .is_some_and(|trial| !trial.lease.valid(now))
        {
            self.rollback(now);
            self.sampled_at = now;
            self.sampled_bytes = bytes;
            return count > self.target;
        }
        let (revision, owner) = self.registry.snapshot(now);
        if self.trial.is_none()
            && (owner.is_some() || revision != self.revision)
        {
            self.revision = revision;
            self.baseline_bps = None;
            self.sampled_at = now;
            self.sampled_bytes = bytes;
            return false;
        }
        if let Some(trial) = &mut self.trial
            && !trial.attached
            && count >= self.target
        {
            trial.attached = true;
            self.sampled_at = now;
            self.sampled_bytes = bytes;
            return false;
        }
        let elapsed = now.saturating_duration_since(self.sampled_at);
        if elapsed < ADAPTIVE_WINDOW {
            return false;
        }
        let rate = (bytes.saturating_sub(self.sampled_bytes) as f64
            / elapsed.as_secs_f64()) as u64;
        self.sampled_at = now;
        self.sampled_bytes = bytes;
        if !busy || !healthy || rate == 0 {
            let added = self.trial.is_some();
            if added {
                self.rollback(now);
            } else {
                self.baseline_bps = None;
            }
            return added && count > self.target;
        }
        if let Some(trial) = &mut self.trial {
            if !trial.attached {
                return false;
            }
            trial.windows += 1;
            trial.rate_sum = trial.rate_sum.saturating_add(rate);
            if trial.windows < ADAPTIVE_CONNECTION_TRIAL_WINDOWS {
                return false;
            }
            let gain = u128::from(
                trial.rate_sum / u64::from(ADAPTIVE_CONNECTION_TRIAL_WINDOWS),
            ) * ADAPTIVE_CONNECTION_PERCENT_BASE
                >= u128::from(trial.baseline_bps)
                    * ADAPTIVE_CONNECTION_GAIN_PERCENT;
            if !gain {
                self.rollback(now);
                return count > self.target;
            }
            self.trial = None;
            self.baseline_bps = None;
            self.revision = self.registry.snapshot(now).0;
            return false;
        }
        if count < self.target
            || self.target >= ADAPTIVE_CONNECTION_MAX_TARGET
            || self.cooldown_until.is_some_and(|until| until > now)
        {
            return false;
        }
        let Some(baseline) = self.baseline_bps.replace(rate) else {
            return false;
        };
        if rate < baseline / ADAPTIVE_CONNECTION_RATE_SPREAD
            || rate > baseline.saturating_mul(ADAPTIVE_CONNECTION_RATE_SPREAD)
        {
            return false;
        }
        let Some(lease) = self.registry.acquire(
            TrialKind::Connections,
            now,
            ADAPTIVE_CONNECTION_TRIAL_TIMEOUT,
        ) else {
            return false;
        };
        self.trial = Some(ConnectionTrial {
            lease,
            previous: self.target,
            baseline_bps: baseline.min(rate),
            windows: 0,
            rate_sum: 0,
            attached: false,
        });
        self.target += 1;
        false
    }
}

#[cfg(test)]
mod adaptive_tests {
    use super::*;

    struct PacedH2 {
        connection: Arc<SharedH2Connection>,
        tasks: Vec<tokio::task::JoinHandle<()>>,
    }

    impl Drop for PacedH2 {
        fn drop(&mut self) {
            for task in &self.tasks {
                task.abort();
            }
        }
    }

    async fn paced_h2(
        clock: Arc<tokio::sync::Mutex<tokio::time::Instant>>,
    ) -> PacedH2 {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            socket.set_nodelay(true).unwrap();
            let mut connection = h2::server::handshake(socket).await.unwrap();
            let mut transfers = tokio::task::JoinSet::new();
            while let Some(request) = connection.accept().await {
                let (_, mut respond) = request.unwrap();
                let mut stream = respond
                    .send_response(http::Response::new(()), false)
                    .unwrap();
                let clock = clock.clone();
                transfers.spawn(async move {
                    loop {
                        stream.reserve_capacity(2048);
                        let Some(Ok(capacity)) =
                            futures::future::poll_fn(|cx| {
                                stream.poll_capacity(cx)
                            })
                            .await
                        else {
                            return;
                        };
                        let count = capacity.min(2048);
                        if count == 0 {
                            continue;
                        }
                        let due = {
                            let mut next = clock.lock().await;
                            *next = (*next).max(tokio::time::Instant::now())
                                + Duration::from_secs_f64(
                                    count as f64 / (128 * 1024) as f64,
                                );
                            *next
                        };
                        tokio::time::sleep_until(due).await;
                        if stream
                            .send_data(
                                bytes::Bytes::from(vec![42; count]),
                                false,
                            )
                            .is_err()
                        {
                            return;
                        }
                    }
                });
            }
        });
        let socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.set_nodelay(true).unwrap();
        let (sender, driver) = h2::client::handshake(socket).await.unwrap();
        let driver = tokio::spawn(async move {
            let _ = driver.await;
        });
        let connection = Arc::new(SharedH2Connection::for_test(sender));
        let mut tasks = vec![server, driver];
        for id in 0..2 {
            let response = connection
                .open(
                    http::Request::get(format!("http://{address}/{id}"))
                        .body(())
                        .unwrap(),
                )
                .await
                .unwrap();
            let connection = connection.clone();
            let active = connection.track_stream();
            tasks.push(tokio::spawn(async move {
                let _active = active;
                let mut body = response.into_body();
                while let Some(Ok(chunk)) = body.data().await {
                    connection.record_bytes(chunk.len());
                    body.flow_control().release_capacity(chunk.len()).unwrap();
                }
            }));
        }
        PacedH2 { connection, tasks }
    }

    async fn measured_socket_trial(shared_bottleneck: bool) {
        let shared_clock =
            Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now()));
        let mut fixtures = Vec::new();
        for _ in 0..2 {
            let clock = if shared_bottleneck {
                shared_clock.clone()
            } else {
                Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now()))
            };
            fixtures.push(paced_h2(clock).await);
        }
        let now = Instant::now();
        let mut scheduler = AuthorityScheduler::default();
        scheduler.adaptive =
            Some(AdaptiveConnections::with_registry(now, 0, Arc::default()));
        scheduler.target = 2;
        for fixture in &fixtures {
            scheduler.add_connection(fixture.connection.clone());
        }
        let mut timer = tokio::time::interval(Duration::from_millis(100));
        loop {
            timer.tick().await;
            scheduler.observe_background(Instant::now(), false);
            if scheduler.target == 3 {
                break;
            }
        }
        let baseline = scheduler
            .adaptive
            .as_ref()
            .unwrap()
            .trial
            .as_ref()
            .unwrap()
            .baseline_bps;
        let clock = if shared_bottleneck {
            shared_clock
        } else {
            Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now()))
        };
        fixtures.push(paced_h2(clock).await);
        scheduler.add_connection(fixtures.last().unwrap().connection.clone());
        scheduler.observe_background(Instant::now(), false);
        loop {
            timer.tick().await;
            scheduler.observe_background(Instant::now(), false);
            if scheduler.adaptive.as_ref().unwrap().trial.is_none() {
                break;
            }
        }
        println!(
            "h2_socket_trial shared_bottleneck={shared_bottleneck} baseline_bps={baseline} accepted_target={} elapsed_ms={} received_bytes={}",
            scheduler.target,
            now.elapsed().as_millis(),
            scheduler.monotonic_bytes
        );
        assert_eq!(scheduler.target, if shared_bottleneck { 2 } else { 3 });
        assert_eq!(
            fixtures.last().unwrap().connection.is_retiring(),
            shared_bottleneck
        );
    }

    #[tokio::test]
    async fn real_h2_socket_measurements_accept_per_connection_gain_and_reject_common_bottleneck()
     {
        tokio::time::timeout(Duration::from_secs(40), async {
            measured_socket_trial(false).await;
            measured_socket_trial(true).await;
        })
        .await
        .expect("bounded socket expansion experiment");
    }

    #[tokio::test]
    async fn background_samples_roll_back_unproductive_connection_without_new_checkout()
     {
        async fn connection() -> Arc<SharedH2Connection> {
            let (io, _server) = tokio::io::duplex(65536);
            let (sender, _driver) = h2::client::handshake(io).await.unwrap();
            let connection = Arc::new(SharedH2Connection::for_test(sender));
            connection.mark_success_for_test();
            connection
        }
        let first = connection().await;
        let second = connection().await;
        let sibling = connection().await;
        let _active = [
            first.track_stream(),
            first.track_stream(),
            second.track_stream(),
            sibling.track_stream(),
        ];
        let now = Instant::now();
        let mut scheduler = AuthorityScheduler::default();
        scheduler.adaptive =
            Some(AdaptiveConnections::with_registry(now, 0, Arc::default()));
        scheduler.target = 2;
        scheduler.add_connection(first.clone());
        scheduler.add_connection(second.clone());
        first.record_bytes(3000);
        scheduler.observe_background(now + ADAPTIVE_WINDOW, false);
        first.record_bytes(3000);
        scheduler.observe_background(now + ADAPTIVE_WINDOW * 2, false);
        assert_eq!(scheduler.target, 3);
        scheduler.add_connection(sibling.clone());
        scheduler.observe_background(now + ADAPTIVE_WINDOW * 2, false);
        for step in 3..=4 {
            first.record_bytes(3000);
            scheduler.observe_background(now + ADAPTIVE_WINDOW * step, false);
        }
        assert_eq!(scheduler.target, 2);
        assert!(sibling.is_retiring());
        assert_eq!(scheduler.connection_count(), 2);
    }

    #[test]
    fn adaptive_accepts_gain_rolls_back_no_gain_and_ignores_empty_queue() {
        // Isolated registries keep policy tests independent of concurrent
        // end-to-end endpoint trials in the same test process.
        for (rate, accepted) in [(1200, true), (1000, false)] {
            let start = Instant::now();
            let mut policy =
                AdaptiveConnections::with_registry(start, 0, Arc::default());
            policy.observe(start + ADAPTIVE_WINDOW, 3000, 2, true, true);
            policy.observe(start + ADAPTIVE_WINDOW * 2, 6000, 2, true, true);
            assert_eq!(policy.target, 3);
            policy.observe(start + ADAPTIVE_WINDOW * 2, 6000, 3, true, true);
            policy.observe(
                start + ADAPTIVE_WINDOW * 3,
                6000 + rate * 3,
                3,
                true,
                true,
            );
            let retire = policy.observe(
                start + ADAPTIVE_WINDOW * 4,
                6000 + rate * 6,
                3,
                true,
                true,
            );
            assert_eq!(policy.target, if accepted { 3 } else { 2 });
            assert_eq!(retire, !accepted);
            assert!(policy.trial.is_none());
        }
        let now = Instant::now();
        let mut policy =
            AdaptiveConnections::with_registry(now, 0, Arc::default());
        for n in 1..5 {
            policy.observe(now + ADAPTIVE_WINDOW * n, 0, 2, false, true);
        }
        assert_eq!(policy.target, 2);
        assert!(policy.trial.is_none());
    }

    #[tokio::test]
    async fn pruning_connections_never_subtracts_from_authority_payload() {
        async fn connection() -> Arc<SharedH2Connection> {
            let (client, _server) = tokio::io::duplex(65536);
            let (sender, _driver) =
                h2::client::handshake(client).await.unwrap();
            Arc::new(SharedH2Connection::for_test(sender))
        }
        let first = connection().await;
        let second = connection().await;
        let mut scheduler = AuthorityScheduler::default();
        scheduler.add_connection(first.clone());
        scheduler.add_connection(second.clone());
        first.record_bytes(1000);
        second.record_bytes(2000);
        scheduler.sample_connection_bytes();
        assert_eq!(scheduler.monotonic_bytes, 3000);
        second.evict_for_scheduler();
        scheduler.prune();
        first.record_bytes(500);
        scheduler.sample_connection_bytes();
        assert_eq!(scheduler.monotonic_bytes, 3500);
    }
}
