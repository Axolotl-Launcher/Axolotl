//! Explicit, public-network-free download comparisons. Full slow baselines can
//! take hours; environment sizes below 64 MiB are labelled scaled experiments.

use super::SelectionMode;
use super::fixtures::{self, EndpointFixture, Profile, SocketStats};
use crate::util::download::proxy_context;
use crate::util::fetch::{self, FetchSemaphore};
use futures::StreamExt;
use serde::Serialize;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

async fn endpoint_trial_for_range_recovery() -> super::tuning::TrialLease {
    use super::tuning::{TRIALS, TrialKind};
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(trial) = TRIALS.acquire(
                TrialKind::Endpoint,
                Instant::now(),
                Duration::from_secs(60),
            ) {
                return trial;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the range recovery fixture must acquire its endpoint experiment")
}

#[derive(Clone, Copy, Debug, Serialize)]
enum Scenario {
    Healthy,
    SlowFast,
    AllSlow,
    SlowTail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
enum Mode {
    Off,
    DnsOnly,
    Expanded,
}

#[derive(Serialize)]
struct Run {
    scenario: Scenario,
    mode: Mode,
    iteration: usize,
    warm: bool,
    size_bytes: u64,
    elapsed_ms: u64,
    unique_goodput_bps: u64,
    received_bytes: u64,
    unique_bytes: u64,
    probe_bytes: u64,
    server_payload_bytes: u64,
    server_body_us: u64,
    peak_connections: usize,
    opened_connections: usize,
    migrations: u8,
    trial_rounds: u64,
    attempts: usize,
    client_phases: super::metrics::PhaseSnapshot,
    server_send_us: u64,
    server_pace_sleep_us: u64,
    server_max_pace_late_us: u64,
}

fn size_bytes() -> u64 {
    let mib = std::env::var("AXOLOTL_BENCH_SIZE_MIB")
        .ok()
        .map(|value| value.parse::<u64>().expect("integer MiB size"))
        .unwrap_or(64);
    assert!((1..=128).contains(&mib));
    mib * 1024 * 1024
}

async fn run(
    scenario: Scenario,
    mode: Mode,
    iteration: usize,
    warm: bool,
    size: u64,
) -> Run {
    let stats = Arc::new(SocketStats::default());
    let slow = 45 * 1024;
    let fast = 5 * 1024 * 1024;
    let healthy = 30 * 1024 * 1024;
    let profile = match scenario {
        Scenario::Healthy => Profile::rate(healthy),
        Scenario::SlowFast | Scenario::AllSlow => Profile::rate(slow),
        Scenario::SlowTail => Profile {
            slow_tail: Some((512 * 1024, slow)),
            ..Profile::rate(healthy)
        },
    };
    let first =
        EndpointFixture::start(Ipv4Addr::LOCALHOST, profile, stats.clone())
            .await;
    let second_rate = match scenario {
        Scenario::AllSlow => slow,
        Scenario::Healthy => healthy,
        _ => fast,
    };
    let second = EndpointFixture::start(
        Ipv4Addr::new(127, 0, 0, 2),
        Profile::rate(second_rate),
        stats.clone(),
    )
    .await;
    let clients = fixtures::clients(
        if mode == Mode::Off {
            SelectionMode::Off
        } else {
            SelectionMode::Auto
        },
        &first,
        &second,
        mode == Mode::Expanded,
    );
    let selector = clients.endpoints.clone();
    let directory = tempfile::tempdir().unwrap();
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(4));
    let request = fixtures::request(size, iteration);
    let destination = directory.path().join("download.bin");
    let measured = proxy_context::with_snapshot(clients.clone(), async {
        if warm {
            let response = clients.system.get(&request.url).header(reqwest::header::RANGE, "bytes=0-1023")
                .send().await.unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.bytes().await.unwrap().len(), 1024);
        }
        let received_before = stats.server_payload.load(Ordering::Relaxed);
        let body_before = stats.body_elapsed_us.load(Ordering::Relaxed);
        let opened_before = stats.opened.load(Ordering::Relaxed);
        let send_before = stats.send_us.load(Ordering::Relaxed);
        let sleep_before = stats.pace_sleep_us.load(Ordering::Relaxed);
        let (probe_before, _, rounds_before) = selector.benchmark_counters();
        let started = Instant::now();
        let deadline = Duration::from_secs_f64(size as f64 / slow as f64 * 1.5 + 60.0);
        let mut transfer = Box::pin(fetch::download_benchmark_fixture(request.clone(), &destination, &semaphore));
        let result = tokio::time::timeout(deadline, async {
            loop {
                tokio::select! {
                    result = &mut transfer => break result,
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        println!("benchmark_progress scenario={scenario:?} mode={mode:?} iteration={iteration} warm={warm} elapsed_s={} counters={:?}",
                            started.elapsed().as_secs(), request.transfer_counters.snapshot());
                    }
                }
            }
        }).await.expect("benchmark deadline").expect("verified benchmark download");
        let elapsed = started.elapsed();
        assert_eq!(result.size, size);
        assert_eq!(tokio::fs::metadata(&destination).await.unwrap().len(), size);
        let counters = request.transfer_counters.snapshot();
        assert_eq!(counters.committed_unique_bytes, size);
        let (probe_after, _, rounds_after) = selector.benchmark_counters();
        let migrations = request.endpoint_migrations.load(Ordering::Relaxed);
        assert!(probe_after - probe_before <= 64 * 1024 * 1024);
        assert!(rounds_after - rounds_before <= 2);
        assert!(stats.peak.load(Ordering::Relaxed) <= 32);
        if mode == Mode::Off { assert_eq!(probe_after, probe_before); assert_eq!(migrations, 0); }
        if matches!(scenario, Scenario::Healthy | Scenario::AllSlow) { assert_eq!(migrations, 0); }
        if matches!(scenario, Scenario::SlowTail) && mode == Mode::Expanded {
            assert_eq!(migrations, 1, "tail matrix must prove actual recovery, not merely successful completion");
            assert_eq!(result.attempts, 2);
            assert_eq!(counters.received_payload_bytes, size);
        }
        Run { scenario, mode, iteration, warm, size_bytes: size, elapsed_ms: elapsed.as_millis() as u64,
            unique_goodput_bps: (size as f64 / elapsed.as_secs_f64()) as u64,
            received_bytes: counters.received_payload_bytes, unique_bytes: counters.committed_unique_bytes,
            probe_bytes: probe_after - probe_before, server_payload_bytes: stats.server_payload.load(Ordering::Relaxed) - received_before,
            server_body_us: stats.body_elapsed_us.load(Ordering::Relaxed) - body_before,
            peak_connections: stats.peak.load(Ordering::Relaxed), opened_connections: stats.opened.load(Ordering::Relaxed) - opened_before,
            migrations, trial_rounds: rounds_after - rounds_before, attempts: result.attempts,
            client_phases: request.transfer_counters.phases.snapshot(),
            server_send_us: stats.send_us.load(Ordering::Relaxed) - send_before,
            server_pace_sleep_us: stats.pace_sleep_us.load(Ordering::Relaxed) - sleep_before,
            server_max_pace_late_us: stats.max_pace_late_us.load(Ordering::Relaxed) }
    }).await;
    selector.close();
    drop(clients);
    measured
}

async fn matrix(scenario: Scenario) {
    let size = size_bytes();
    let modes = match std::env::var("AXOLOTL_BENCH_MODE").ok().as_deref() {
        None => vec![Mode::Off, Mode::DnsOnly, Mode::Expanded],
        Some("off") => vec![Mode::Off],
        Some("dns") => vec![Mode::DnsOnly],
        Some("expanded") => vec![Mode::Expanded],
        Some(_) => panic!("benchmark mode must be off, dns or expanded"),
    };
    let shard = std::env::var("AXOLOTL_BENCH_SHARD")
        .ok()
        .map(|value| value.parse::<usize>().expect("integer shard index"));
    assert!(shard.is_none_or(|shard| shard < 10));
    let partial = modes.len() != 3 || shard.is_some();
    if partial {
        println!(
            "benchmark_partial_mode true; mode or shard diagnostics require complete cross-run aggregation for comparative acceptance"
        );
    }
    println!(
        "benchmark_schedule shard={shard:?}; each shard retains sequential interleaved modes; separate processes have independent admission but share CPU and disk"
    );
    let mut results = Vec::new();
    for iteration in 0..5 {
        for warm in [false, true] {
            if shard
                .is_some_and(|shard| shard != iteration * 2 + usize::from(warm))
            {
                continue;
            }
            // Interleaving rotates order so one mode never always runs first.
            for offset in 0..modes.len() {
                let mode = modes[(iteration + offset) % modes.len()];
                let result = run(scenario, mode, iteration, warm, size).await;
                println!(
                    "benchmark_run {}",
                    serde_json::to_string(&result).unwrap()
                );
                results.push(result);
            }
        }
    }
    for warm in [false, true] {
        let mut medians = Vec::new();
        for &mode in &modes {
            let group: Vec<_> = results
                .iter()
                .filter(|run| run.mode == mode && run.warm == warm)
                .collect();
            if group.is_empty() {
                continue;
            }
            let mut times: Vec<_> =
                group.iter().map(|run| run.elapsed_ms).collect();
            times.sort_unstable();
            let median = times[times.len() / 2];
            let p95 = times[(times.len() * 95).div_ceil(100).saturating_sub(1)];
            medians.push(median);
            println!(
                "benchmark_summary scenario={scenario:?} mode={mode:?} warm={warm} size_bytes={size} scaled={} median_ms={median} p95_ms={p95} runs={} probe_bytes={} received_bytes={} unique_bytes={} peak_connections={} migrations={}",
                size != 64 * 1024 * 1024,
                group.len(),
                group.iter().map(|run| run.probe_bytes).sum::<u64>(),
                group.iter().map(|run| run.received_bytes).sum::<u64>(),
                group.iter().map(|run| run.unique_bytes).sum::<u64>(),
                group.iter().map(|run| run.peak_connections).max().unwrap(),
                group.iter().map(|run| run.migrations as u64).sum::<u64>()
            );
        }
        if !partial
            && matches!(scenario, Scenario::SlowFast)
            && size >= 64 * 1024 * 1024
        {
            assert!(
                medians[2] * 5 <= medians[0],
                "full weak-network gain should exceed 5x"
            );
        }
        if !partial && matches!(scenario, Scenario::AllSlow) {
            for &median in &medians[1..] {
                assert!(
                    median <= medians[0] * 110 / 100,
                    "all-slow probing must not regress median completion by more than 10 percent"
                );
            }
        }
        if !partial
            && matches!(scenario, Scenario::Healthy)
            && medians[1..]
                .iter()
                .any(|median| *median > medians[0] * 105 / 100)
        {
            println!(
                "benchmark_noise healthy regression above 5 percent; inspect timer and disk variance before acceptance"
            );
            assert!(
                size < 64 * 1024 * 1024,
                "full-size healthy-network regression exceeds the 5 percent acceptance gate"
            );
        }
    }
}

#[tokio::test]
#[ignore = "explicit full or scaled local performance matrix"]
async fn modrinth_download_benchmark_healthy() {
    matrix(Scenario::Healthy).await;
}

#[tokio::test]
#[ignore = "64 MiB at 45 KiB/s baseline takes about 24 minutes per run"]
async fn modrinth_download_benchmark_slow_fast() {
    matrix(Scenario::SlowFast).await;
}

#[tokio::test]
#[ignore = "all-slow comparison intentionally retains long baseline transfers"]
async fn modrinth_download_benchmark_all_slow() {
    matrix(Scenario::AllSlow).await;
}

#[tokio::test]
#[ignore = "explicit tail completion experiment"]
async fn modrinth_download_benchmark_slow_tail() {
    matrix(Scenario::SlowTail).await;
}

#[tokio::test]
async fn benchmark_fixture_smoke_is_local_and_verifies_one_complete_file() {
    let result = run(Scenario::Healthy, Mode::Off, 0, false, 256 * 1024).await;
    assert_eq!(result.probe_bytes, 0);
    assert_eq!(result.migrations, 0);
}

#[tokio::test]
async fn real_slow_tail_is_probed_at_unfinished_offset_and_resumed_without_duplicate_payload()
 {
    let result = run(
        Scenario::SlowTail,
        Mode::Expanded,
        0,
        false,
        64 * 1024 * 1024,
    )
    .await;
    println!("tail_recovery {}", serde_json::to_string(&result).unwrap());
    assert_eq!(
        result.migrations, 1,
        "tail recovery must actually change endpoint"
    );
    assert_eq!(result.attempts, 2);
    assert_eq!(result.received_bytes, result.unique_bytes);
    assert!(result.probe_bytes > 0 && result.probe_bytes <= 2 * 1024 * 1024);
}

#[tokio::test]
async fn default_selection_recovers_a_fast_prefix_then_500_kib_tail() {
    let size = 32 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let first = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile {
            slow_tail: Some((16 * 1024 * 1024, 500 * 1024)),
            ..Profile::rate(5 * 1024 * 1024)
        },
        stats.clone(),
    )
    .await;
    let second = EndpointFixture::start(
        Ipv4Addr::new(127, 0, 0, 2),
        Profile::rate(5 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::parse(None), &first, &second, true);
    let selector = clients.endpoints.clone();
    let request = fixtures::request(size, 0);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("throttled-tail.bin");
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(4));
    let result = tokio::time::timeout(
        Duration::from_secs(40),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.size, size);
    assert_eq!(request.endpoint_migrations.load(Ordering::Relaxed), 1);
    let counters = request.transfer_counters.snapshot();
    assert_eq!(counters.received_payload_bytes, size);
    assert_eq!(counters.committed_unique_bytes, size);
    assert!(selector.benchmark_counters().0 > 0);
    selector.close();
}

#[tokio::test]
async fn segmented_tail_reuses_initial_slots_after_an_unproductive_expansion() {
    let size = 32 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let first = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile {
            slow_tail: Some((16 * 1024 * 1024, 500 * 1024)),
            ..Profile::rate(5 * 1024 * 1024)
        },
        stats.clone(),
    )
    .await;
    let clients = fixtures::clients(SelectionMode::Off, &first, &first, false);
    let selector = clients.endpoints.clone();
    let request = fixtures::request(size, 0).with_segmented_download(true);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("segmented-tail.bin");
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(8));
    let result = tokio::time::timeout(
        Duration::from_secs(40),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.size, size);
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        size
    );
    assert!(
        stats.requests.load(Ordering::Relaxed) >= 6,
        "initial four ranges and the first growth trial must be followed by a replacement for completed ranges"
    );
    assert!(stats.peak.load(Ordering::Relaxed) <= 8);
    assert_eq!(selector.benchmark_counters().0, 0);
    selector.close();
}

#[tokio::test]
async fn tail_floor_moves_small_batch_files_off_a_slow_shared_h2_socket() {
    let size = 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let fast = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(100 * 1024),
        stats.clone(),
    )
    .await;
    let mut clients =
        fixtures::clients(SelectionMode::Off, &fast, &fast, false);
    clients.force_http1 = false;
    let request = fixtures::request(size, 0).with_batch_download_policy();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("small-batch-tail.bin");
    let part = destination.with_extension("bin.part");
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(16));
    proxy_context::with_snapshot(clients, async {
        let route = crate::util::fetch::DownloadRoute {
            url: request.url.clone(), source: crate::util::fetch::DownloadRouteSource::Official,
            is_mirror: false, allow_sensitive_headers: true, supports_range: true,
            proxy: crate::util::fetch::ProxyPolicy::System,
        };
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let server = tokio::spawn(async move {
            let mut server = h2::server::handshake(server_io).await.unwrap();
            let (_, mut response) = server.accept().await.unwrap().unwrap();
            let mut body = response.send_response(http::Response::builder().header("content-length", size).body(()).unwrap(), false).unwrap();
            let sender = tokio::spawn(async move {
                for offset in (0..size).step_by(4096) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if body.send_data(bytes::Bytes::from(vec![42; 4096]), offset + 4096 == size).is_err() { break; }
                }
            });
            while server.accept().await.is_some() {}
            sender.abort();
        });
        let (sender, driver) = h2::client::handshake(client_io).await.unwrap();
        let connection = crate::util::download::h2_pool::SharedH2Connection::cache_for_test(&route, sender).await;
        let driver = crate::util::download::h2_pool::spawn_connection_driver(&connection, driver, Duration::from_secs(60));
        let started = Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(12), fetch::download_benchmark_fixture(request.clone(), &destination, &semaphore)).await
            .expect("small batch recovery must not wait for the 35-second target or an endpoint trial").unwrap();
        println!("tail_floor small_batch elapsed_ms={}", started.elapsed().as_millis());
        assert_eq!(result.size, size);
        assert!(stats.requests.load(Ordering::Relaxed) > 0, "the small file must resume on the independent HTTP/1 server");
          assert!(stats.peak.load(Ordering::Relaxed) >= 2, "leaving H2 must not strand the small file on another slow single connection");
        assert!(stats.requested_ranges.lock().iter().all(|(start, _)| *start > 0));
        assert_eq!(request.transfer_counters.snapshot().committed_unique_bytes, size);
        assert!(!connection.is_retiring());
        driver.abort();
        server.abort();
    }).await;
    assert_eq!(semaphore.0.available_permits(), 16);
}

#[tokio::test]
async fn tail_floor_promotes_a_slow_small_http1_batch_file_to_multiple_connections()
 {
    let size = 6 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(128 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let request = fixtures::request(size, 0).with_batch_download_policy();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("small-http1-tail.bin");
    let part = destination.with_extension("bin.part");
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(32));
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .expect("a slow small HTTP/1 file must borrow idle connections")
    .unwrap();
    assert_eq!(result.size, size);
    let ranges = stats.requested_ranges.lock();
    assert_eq!(ranges[0].0, 0);
    assert!(ranges.iter().skip(1).all(|(start, _)| *start > 0));
    assert!(stats.peak.load(Ordering::Relaxed) >= 4);
    assert_eq!(
        result.attempts, 1,
        "transport recovery is not a failed download attempt"
    );
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        size
    );
    assert_eq!(semaphore.0.available_permits(), 32);
}

#[tokio::test]
async fn range_recovery_replenishes_the_last_quarter_during_an_endpoint_trial()
{
    let size = 64 * 1024 * 1024;
    let tail = size / 4;
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile {
            slow_tail: Some((tail, 100 * 1024)),
            ..Profile::rate(10 * 1024 * 1024)
        },
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let request = fixtures::request(size, 0)
        .with_segmented_download(true)
        .with_range_connection_floor(32);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("last-quarter.bin");
    let part = destination.with_extension("bin.part");
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(64));
    let trial = endpoint_trial_for_range_recovery().await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .expect("tail recovery must not wait for a throughput experiment")
    .unwrap();
    let tail_requests = stats
        .requested_ranges
        .lock()
        .iter()
        .filter(|(start, _)| *start >= size - tail)
        .count();
    println!(
        "range_recovery elapsed_ms={} tail_requests={tail_requests}",
        started.elapsed().as_millis()
    );
    assert_eq!(result.size, size);
    assert!(
        tail_requests > 8,
        "the last eight original ranges must borrow new connections"
    );
    assert!(!trial.valid(Instant::now()));
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        size
    );
    assert_eq!(semaphore.0.available_permits(), 64);
}

#[tokio::test]
async fn connection_limited_bulk_download_grows_past_eight_and_verifies_hash() {
    let size = 64 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(256 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let selector = clients.endpoints.clone();
    let request = fixtures::request(size, 0).with_segmented_download(true);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("bulk-ranges.bin");
    let part = destination.with_extension("bin.part");
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(64));
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(40),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .expect("parallel ranges must finish within the connection-limited budget")
    .unwrap();
    let peak = stats.peak.load(Ordering::Relaxed);
    println!(
        "bulk_ranges size={size} elapsed_ms={} peak_connections={peak}",
        started.elapsed().as_millis()
    );
    assert_eq!(result.size, size);
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        size
    );
    assert!(
        peak > 8,
        "bulk traffic must escape the former eight-socket cap"
    );
    assert!(
        peak <= 56,
        "one bulk file must leave eight global permits available"
    );
    assert_eq!(semaphore.0.available_permits(), 64);
    assert_eq!(selector.benchmark_counters().0, 0);
    selector.close();
}

#[tokio::test]
async fn batch_large_item_resumes_after_halfway_with_multiple_ranges() {
    let size = 24 * 1024 * 1024;
    let prefix = 16 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(1024 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let request = fixtures::request(size, 0).with_batch_download_policy();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("resumed-batch.jar");
    let part = destination.with_extension("jar.part");
    tokio::fs::write(&part, vec![42; prefix as usize])
        .await
        .unwrap();
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(64));
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .expect("the remaining ranges should finish in parallel")
    .unwrap();
    assert_eq!(result.size, size);
    assert!(stats.peak.load(Ordering::Relaxed) >= 2);
    assert!(
        stats
            .requested_ranges
            .lock()
            .iter()
            .all(|(start, end)| *start >= prefix && *end < size)
    );
    let counters = request.transfer_counters.snapshot();
    assert_eq!(counters.received_payload_bytes, size - prefix);
    assert_eq!(counters.committed_unique_bytes, size);
    assert_eq!(semaphore.0.available_permits(), 64);
}

#[tokio::test]
async fn batch_cancelled_ranges_restore_the_original_contiguous_prefix() {
    let size = 24 * 1024 * 1024;
    let prefix = vec![42; 16 * 1024 * 1024];
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(45 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let request = fixtures::request(size, 0).with_batch_download_policy();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("cancelled-batch.jar");
    let part = destination.with_extension("jar.part");
    tokio::fs::write(&part, &prefix).await.unwrap();
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(64));
    {
        let transfer = proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request,
                &destination,
                &semaphore,
            ),
        );
        tokio::pin!(transfer);
        tokio::select! {
            result = &mut transfer => panic!("download finished before cancellation: {result:?}"),
            ready = tokio::time::timeout(Duration::from_secs(5), async {
                while stats.requests.load(Ordering::Relaxed) < 2 || stats.server_payload.load(Ordering::Relaxed) == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }) => ready.expect("parallel requests must start before cancellation"),
        }
    }
    assert_eq!(tokio::fs::read(&part).await.unwrap(), prefix);
    assert!(!destination.exists());
    assert_eq!(semaphore.0.available_permits(), 64);
}

#[tokio::test]
async fn range_recovery_borrows_connections_after_other_files_finish() {
    let size = 12 * 1024 * 1024;
    let stats = Arc::new(SocketStats::default());
    let server = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(100 * 1024),
        stats.clone(),
    )
    .await;
    let clients =
        fixtures::clients(SelectionMode::Off, &server, &server, false);
    let request = fixtures::request(size, 0).with_batch_download_policy();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("growing-batch.jar");
    let part = destination.with_extension("jar.part");
    let _disk = crate::util::download::local_resources::set_test_disk_kind(
        &part,
        sysinfo::DiskKind::SSD,
    );
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(64));
    let started = Instant::now();
    let trial = endpoint_trial_for_range_recovery().await;
    proxy_context::with_snapshot(clients, async {
        let route = crate::util::fetch::DownloadRoute {
            url: request.url.clone(), source: crate::util::fetch::DownloadRouteSource::Official,
            is_mirror: false, allow_sensitive_headers: true, supports_range: true,
            proxy: crate::util::fetch::ProxyPolicy::System,
        };
        let held: Vec<_> = (0..crate::util::download::constants::MAX_CONNECTIONS_PER_AUTHORITY - 1)
            .map(|_| crate::util::download::native_budget::try_acquire(&route).unwrap()).collect();
        let finish_other_files = async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while stats.requests.load(Ordering::Relaxed) == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
            assert_eq!(stats.requests.load(Ordering::Relaxed), 1);
            tokio::time::sleep(Duration::from_millis(250)).await;
            drop(held);
        };
        let (result, _) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(30), fetch::download_benchmark_fixture(
                request.clone(), &destination, &semaphore,
            )),
            finish_other_files,
        );
        assert_eq!(result.expect("a slow batch item must grow beyond its initial single connection").unwrap().size, size);
    }).await;
    let peak = stats.peak.load(Ordering::Relaxed);
    println!(
        "batch_growth size={size} elapsed_ms={} peak_connections={peak}",
        started.elapsed().as_millis()
    );
    assert!(peak >= 3);
    assert!(!trial.valid(Instant::now()));
    assert_eq!(
        request.transfer_counters.snapshot().committed_unique_bytes,
        size
    );
    assert_eq!(semaphore.0.available_permits(), 64);
}

#[tokio::test]
async fn automatic_official_download_does_not_launch_a_duplicate_url_route_probe()
 {
    let stats = Arc::new(SocketStats::default());
    let first = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(30 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let second = EndpointFixture::start(
        Ipv4Addr::new(127, 0, 0, 2),
        Profile::rate(30 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let clients = fixtures::clients(SelectionMode::Auto, &first, &second, true);
    let request = fixtures::request(4 * 1024 * 1024, 0);
    let route = crate::util::fetch::DownloadRoute {
        url: request.url.clone(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: crate::util::fetch::ProxyPolicy::System,
    };
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(4));
    let mut routes = vec![route.clone()];
    proxy_context::with_snapshot(clients.clone(), async {
        fetch::prepare_native_download_routes(&request, &mut routes, &semaphore).await;
        assert_eq!(crate::util::download::native::h2_ineligible_reason(&route), Some(crate::util::download::native::NativeH2IneligibleReason::Http1Fallback));
        tokio::task::yield_now().await;
        assert_eq!(stats.requests.load(Ordering::Relaxed), 0);
        assert_eq!(clients.endpoints.benchmark_counters().0, 0);
    }).await;
}

#[tokio::test]
async fn http1_multi_range_measures_new_peer_and_resumes_each_unfinished_range()
{
    let stats = Arc::new(SocketStats::default());
    let slow = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(45 * 1024),
        stats.clone(),
    )
    .await;
    let fast = EndpointFixture::start(
        Ipv4Addr::new(127, 0, 0, 2),
        Profile::rate(5 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let clients = fixtures::clients(SelectionMode::Auto, &slow, &fast, true);
    let selector = clients.endpoints.clone();
    let request =
        fixtures::request(8 * 1024 * 1024, 0).with_segmented_download(true);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("http1-ranges.bin");
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(8));
    let result = tokio::time::timeout(
        Duration::from_secs(40),
        proxy_context::with_snapshot(
            clients,
            fetch::download_benchmark_fixture(
                request.clone(),
                &destination,
                &semaphore,
            ),
        ),
    )
    .await
    .unwrap_or_else(|error| panic!("{error}; {}", selector.trial_diagnostic()))
    .unwrap();
    assert_eq!(result.size, 8 * 1024 * 1024);
    assert_eq!(request.endpoint_migrations.load(Ordering::Relaxed), 1);
    let counters = request.transfer_counters.snapshot();
    assert_eq!(counters.received_payload_bytes, result.size);
    assert_eq!(counters.committed_unique_bytes, result.size);
    assert_eq!(selector.benchmark_counters().2, 1);
    assert!(
        stats.requests.load(Ordering::Relaxed) >= 5,
        "production splits and resumes ranges in addition to probes"
    );
    assert!(stats.peak.load(Ordering::Relaxed) <= 8);
    assert_eq!(
        tokio::fs::metadata(&destination).await.unwrap().len(),
        result.size
    );
    selector.close();
}

#[tokio::test]
async fn automatic_http1_generations_do_not_retain_unbudgeted_idle_sockets() {
    let stats = Arc::new(SocketStats::default());
    let first = EndpointFixture::start(
        Ipv4Addr::LOCALHOST,
        Profile::rate(30 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let second = EndpointFixture::start(
        Ipv4Addr::new(127, 0, 0, 2),
        Profile::rate(30 * 1024 * 1024),
        stats.clone(),
    )
    .await;
    let clients = fixtures::clients(SelectionMode::Auto, &first, &second, true);
    let selector = clients.endpoints.clone();
    let directory = tempfile::tempdir().unwrap();
    let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(32));
    for wave in 0..2 {
        if wave == 1 {
            selector
                .install_test_selection(second.address, super::Protocol::Http1);
        }
        let transfers = futures::stream::iter(0..32)
            .map(|id| {
                let request = fixtures::request(128 * 1024, id + wave * 32);
                let destination =
                    directory.path().join(format!("{wave}-{id}.bin"));
                let clients = clients.clone();
                let semaphore = &semaphore;
                async move {
                    proxy_context::with_snapshot(
                        clients,
                        fetch::download_benchmark_fixture(
                            request,
                            &destination,
                            semaphore,
                        ),
                    )
                    .await
                    .unwrap();
                }
            })
            .buffer_unordered(32);
        tokio::time::timeout(
            Duration::from_secs(10),
            transfers.collect::<Vec<_>>(),
        )
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while stats.active.load(Ordering::SeqCst) != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("completed HTTP/1 requests must close pooled sockets before next generation");
    }
    assert!(stats.peak.load(Ordering::SeqCst) <= 32);
    assert_eq!(first.stats.requests.load(Ordering::Relaxed), 64);
    selector.close();
}

#[tokio::test]
#[ignore = "explicit 200-file shared-selection comparison, five interleaved rounds"]
async fn modrinth_download_benchmark_many_files() {
    for iteration in 0..5 {
        let modes = [Mode::Off, Mode::DnsOnly, Mode::Expanded];
        for offset in 0..3 {
            let mode = modes[(iteration + offset) % 3];
            let stats = Arc::new(SocketStats::default());
            let first = EndpointFixture::start(
                Ipv4Addr::LOCALHOST,
                Profile::rate(45 * 1024),
                stats.clone(),
            )
            .await;
            let second = EndpointFixture::start(
                Ipv4Addr::new(127, 0, 0, 2),
                Profile::rate(5 * 1024 * 1024),
                stats.clone(),
            )
            .await;
            let clients = fixtures::clients(
                if mode == Mode::Off {
                    SelectionMode::Off
                } else {
                    SelectionMode::Auto
                },
                &first,
                &second,
                mode == Mode::Expanded,
            );
            let selector = clients.endpoints.clone();
            let directory = tempfile::tempdir().unwrap();
            let semaphore = FetchSemaphore(tokio::sync::Semaphore::new(32));
            let requests: Vec<_> = (0..200)
                .map(|index| {
                    fixtures::request(
                        (128 + (index % 8) * 128) as u64 * 1024,
                        index,
                    )
                })
                .collect();
            let unique: u64 = requests
                .iter()
                .map(|request| request.integrity.size.unwrap())
                .sum();
            let started = Instant::now();
            proxy_context::with_snapshot(clients.clone(), async {
                let downloads = futures::stream::iter(requests.iter().enumerate()).map(|(index, request)| {
                    let destination = directory.path().join(format!("{index}.bin"));
                    let clients = clients.clone();
                    let semaphore = &semaphore;
                    async move {
                        let start = Instant::now();
                        proxy_context::with_snapshot(clients, fetch::download_benchmark_fixture(request.clone(), &destination, semaphore)).await.unwrap();
                        start.elapsed().as_millis() as u64
                    }
                }).buffer_unordered(32);
                let times = tokio::time::timeout(Duration::from_secs(180), downloads.collect::<Vec<_>>()).await.unwrap();
                assert_eq!(times.len(), 200);
                let received: u64 = requests.iter().map(|request| request.transfer_counters.snapshot().received_payload_bytes).sum();
                assert_eq!(requests.iter().map(|request| request.transfer_counters.snapshot().committed_unique_bytes).sum::<u64>(), unique);
                let mut sorted = times;
                sorted.sort_unstable();
                let (probe_bytes, _, trials) = selector.benchmark_counters();
                let migrations: u64 = requests.iter().map(|request| request.endpoint_migrations.load(Ordering::Relaxed) as u64).sum();
                println!("benchmark_batch mode={mode:?} iteration={iteration} files=200 median_file_ms={} p95_file_ms={} total_ms={} unique_bytes={unique} received_bytes={received} probe_bytes={probe_bytes} peak_connections={} migrations={migrations} trial_rounds={trials}",
                    sorted[100], sorted[189], started.elapsed().as_millis(), stats.peak.load(Ordering::Relaxed));
                assert!(trials <= 2, "batch shares its hourly probe budget");
                assert!(stats.peak.load(Ordering::Relaxed) <= 32);
            }).await;
            selector.close();
        }
    }
}
