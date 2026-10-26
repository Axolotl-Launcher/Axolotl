use super::super::model::CandidateSource;
use super::*;

fn selection(ip: &str, now: Instant) -> Selection {
    Selection {
        reconnect_overhead: Duration::from_millis(600),
        protocol: Protocol::H2,
        preferred: EndpointCandidate {
            ip: ip.parse().unwrap(),
            source: CandidateSource::Verified,
            expires_at: now + policy::SELECTION_LIFETIME,
        },
        backups: vec![EndpointCandidate {
            ip: "151.101.65.242".parse().unwrap(),
            source: CandidateSource::Verified,
            expires_at: now + policy::SELECTION_LIFETIME,
        }],
        speed: 5_000_000,
        expires_at: now + policy::SELECTION_LIFETIME,
    }
}

#[test]
fn mode_defaults_to_auto_and_explicit_passive_modes_do_not_probe() {
    assert!(catalog_expansion(None));
    assert!(catalog_expansion(Some("on")));
    assert!(!catalog_expansion(Some("off")));
    assert!(!catalog_expansion(Some("OFF")));
    assert!(!catalog_expansion(Some("151.101.1.242")));
    assert_eq!(SelectionMode::parse(None), SelectionMode::Auto);
    assert_eq!(SelectionMode::parse(Some("off")), SelectionMode::Off);
    assert_eq!(SelectionMode::parse(Some("AUTO")), SelectionMode::Off);
    assert_eq!(
        SelectionMode::parse(Some("observe")),
        SelectionMode::Observe
    );
    for mode in [SelectionMode::Off, SelectionMode::Observe] {
        let selector = EndpointSelector::with_mode("test", mode);
        let _registration = selector.register(CancellationToken::new());
        assert!(selector.begin(Instant::now()).is_none());
        assert!(
            selector
                .lease(
                    &"https://cdn.modrinth.com/data/a".parse().unwrap(),
                    Protocol::H2
                )
                .is_none()
        );
        assert_eq!(selector.probe_bytes.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn disk_pressure_or_cpu_pressure_does_not_start_a_host_trial_or_consume_budget()
{
    let clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    let mut selector =
        EndpointSelector::with_mode("pressure-admission", SelectionMode::Auto);
    Arc::get_mut(&mut selector).unwrap().trials = Arc::default();
    let request = DownloadRequest::new(
        "https://cdn.modrinth.com/data/project/file.jar",
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1("hash").with_size(1_048_576),
    );
    let _registration = selector.register_download(&request).unwrap();
    let route = DownloadRoute {
        url: request.url.clone(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    for pressure in [
        super::super::super::local_resources::Pressure {
            cpu_percent: 0,
            write_latency_ms: 75.0,
        },
        super::super::super::local_resources::Pressure {
            cpu_percent:
                crate::util::download::constants::LOCAL_CPU_PRESSURE_PERCENT,
            write_latency_ms: 0.0,
        },
    ] {
        let revision = selector.trials.snapshot(Instant::now()).0;
        selector.on_slow_region(
            &request,
            &route,
            &clients,
            Protocol::Http1,
            Some("151.101.1.242".parse().unwrap()),
            0..1_048_576,
            Some(45 * 1024),
            pressure,
        );
        let state = selector.state.lock();
        assert!(state.round.is_none());
        assert!(state.hourly_reservations.is_empty());
        assert_eq!(state.next_round, 0);
        assert_eq!(selector.trials.snapshot(Instant::now()).0, revision);
    }
}

#[tokio::test]
async fn metered_network_never_reaches_candidate_dns_or_body_probe() {
    let mut clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    let mut selector =
        EndpointSelector::with_mode("metered-fixture", SelectionMode::Auto);
    Arc::get_mut(&mut selector).unwrap().trials = Arc::default();
    clients.endpoints = selector.clone();
    *selector.probe_fixture.lock() = Some(ProbeFixture {
        metered: true,
        candidates: Vec::new(),
    });
    let request = DownloadRequest::new(
        "https://cdn.modrinth.com/data/project/file.jar",
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1("hash").with_size(1024 * 1024),
    );
    let _registration = selector.register_download(&request).unwrap();
    let route = DownloadRoute {
        url: request.url.clone(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    selector.on_slow(
        &request,
        &route,
        &clients,
        Protocol::Http1,
        Some("151.101.1.242".parse().unwrap()),
        1024 * 1024,
        super::super::super::local_resources::Pressure::default(),
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while selector.is_probing() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(selector.state.lock().last_round_status, RoundStatus::NoGain);
    assert_eq!(selector.state.lock().next_round, 1);
    assert_eq!(selector.probe_bytes.load(Ordering::Relaxed), 0);
    assert!(
        clients
            .system_dns
            .resolved_addresses(modrinth_cdn::HOST)
            .is_empty()
    );
}

#[test]
fn passive_modes_and_cooldown_leave_shared_trial_revision_unchanged() {
    let clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    let request = DownloadRequest::new(
        "https://cdn.modrinth.com/data/project/file.jar",
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1("hash").with_size(1024 * 1024),
    );
    let route = DownloadRoute {
        url: request.url.clone(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    for mode in [
        SelectionMode::Off,
        SelectionMode::Observe,
        SelectionMode::Auto,
    ] {
        let mut selector = EndpointSelector::with_mode("passive-fixture", mode);
        Arc::get_mut(&mut selector).unwrap().trials = Arc::default();
        selector.state.lock().cooldown_until =
            Some(Instant::now() + policy::COOLDOWN);
        let before = selector.trials.snapshot(Instant::now()).0;
        selector.on_slow(
            &request,
            &route,
            &clients,
            Protocol::Http1,
            None,
            1024 * 1024,
            super::super::super::local_resources::Pressure::default(),
        );
        assert_eq!(selector.trials.snapshot(Instant::now()).0, before);
        assert_eq!(selector.state.lock().next_round, 0);
    }
}

#[test]
fn two_valid_bad_production_windows_roll_back_without_accepting_stale_feedback()
{
    let selector = EndpointSelector::with_mode("rollback", SelectionMode::Auto);
    let now = Instant::now();
    selector.install_test_selection(
        "146.75.113.242:443".parse().unwrap(),
        Protocol::H2,
    );
    {
        let mut state = selector.state.lock();
        let mut old = selection("151.101.1.242", now);
        old.speed = 1_000_000;
        state.previous = Some(old);
    }
    let url = "https://cdn.modrinth.com/data/a".parse().unwrap();
    let lease = selector.lease(&url, Protocol::H2).unwrap();
    selector.observe_payload(&lease, 0, now, false);
    selector.observe_payload(
        &lease,
        100_000,
        now + Duration::from_secs(3),
        false,
    );
    assert!(selector.is_current(&lease));
    selector.observe_payload(
        &lease,
        100_000,
        now + Duration::from_secs(6),
        false,
    );
    assert_eq!(
        selector
            .lease(&url, Protocol::H2)
            .unwrap()
            .pool
            .endpoint
            .ip()
            .to_string(),
        "151.101.1.242"
    );
    assert!(!selector.is_current(&lease));
    assert!(selector.state.lock().cooldown_until.is_some());
    let generation = selector.state.lock().generation;
    selector.observe_payload(&lease, 1, now + Duration::from_secs(9), false);
    assert_eq!(selector.state.lock().generation, generation);
}

#[test]
fn pressure_or_another_trial_invalidates_the_bad_window_sequence() {
    let selector =
        EndpointSelector::with_mode("rollback-pressure", SelectionMode::Auto);
    let now = Instant::now();
    selector.install_test_selection(
        "146.75.113.242:443".parse().unwrap(),
        Protocol::H2,
    );
    selector.state.lock().previous = Some(selection("151.101.1.242", now));
    let lease = selector
        .lease(
            &"https://cdn.modrinth.com/data/a".parse().unwrap(),
            Protocol::H2,
        )
        .unwrap();
    selector.observe_payload(&lease, 0, now, false);
    selector.observe_payload(&lease, 10, now + Duration::from_secs(3), false);
    selector.observe_payload(&lease, 10, now + Duration::from_secs(6), true);
    selector.observe_payload(&lease, 10, now + Duration::from_secs(9), false);
    assert!(selector.is_current(&lease));
}

#[test]
fn ending_last_active_transfer_discards_idle_time_from_feedback() {
    use super::super::metrics::EndpointTransferGuard;
    let selector =
        EndpointSelector::with_mode("idle-feedback", SelectionMode::Auto);
    let now = Instant::now();
    selector.install_test_selection(
        "146.75.113.242:443".parse().unwrap(),
        Protocol::H2,
    );
    selector.state.lock().previous = Some(selection("151.101.1.242", now));
    let lease = selector
        .lease(
            &"https://cdn.modrinth.com/data/a".parse().unwrap(),
            Protocol::H2,
        )
        .unwrap();
    let guard = EndpointTransferGuard::new(&selector, lease.clone());
    let clone = guard.clone();
    selector.observe_payload(&lease, 0, now, false);
    selector.observe_payload(&lease, 10, now + Duration::from_secs(3), false);
    drop(guard);
    assert!(selector.state.lock().feedback.is_some());
    drop(clone);
    assert!(selector.state.lock().feedback.is_none());
    assert!(selector.state.lock().active_feedback.is_empty());
}

#[test]
fn twenty_downloads_share_one_round_and_one_cancellation_does_not_cancel_others()
 {
    let selector = EndpointSelector::with_mode("test", SelectionMode::Auto);
    let mut guards: Vec<_> = (0..20)
        .map(|_| selector.register(CancellationToken::new()).unwrap())
        .collect();
    let start = selector.begin(Instant::now()).unwrap();
    let mut status = start.guard.status.subscribe();
    for _ in 0..20 {
        assert!(selector.begin(Instant::now()).is_none());
    }
    guards.pop();
    assert!(!start.cancellation.is_cancelled());
    drop(guards);
    assert!(start.cancellation.is_cancelled());
    drop(start);
    assert_eq!(*status.borrow_and_update(), RoundStatus::Cancelled);
    assert!(selector.state.lock().round.is_none());
}

#[test]
fn cancelled_or_unpolled_worker_retains_terminal_status_and_releases_slot() {
    let selector = EndpointSelector::with_mode("test", SelectionMode::Auto);
    let _registration = selector.register(CancellationToken::new()).unwrap();
    let start = selector.begin(Instant::now()).unwrap();
    let status = start.guard.status.subscribe();
    drop(start);
    assert_eq!(*status.borrow(), RoundStatus::Cancelled);
    assert!(selector.state.lock().round.is_none());
    assert!(selector.state.lock().cooldown_until.is_some());
}

#[test]
fn publication_requires_live_epoch_generation_and_observers() {
    let selector = EndpointSelector::with_mode("test", SelectionMode::Auto);
    let _registration = selector.register(CancellationToken::new()).unwrap();
    let start = selector.begin(Instant::now()).unwrap();
    let now = Instant::now();
    assert_eq!(
        selector.publish(
            selection("146.75.113.242", now),
            start.epoch + 1,
            start.generation
        ),
        RoundStatus::Stale
    );
    assert_eq!(
        selector.publish(
            selection("146.75.113.242", now),
            start.epoch,
            start.generation + 1
        ),
        RoundStatus::Stale
    );
    assert_eq!(
        selector.publish(
            selection("146.75.113.242", now),
            start.epoch,
            start.generation
        ),
        RoundStatus::Selected
    );
    let lease = selector
        .lease(
            &"https://cdn.modrinth.com/data/a".parse().unwrap(),
            Protocol::H2,
        )
        .unwrap();
    assert_eq!(lease.pool.endpoint.ip().to_string(), "146.75.113.242");
    assert!(selector.is_current(&lease));
    assert!(
        selector
            .lease(&"https://api.modrinth.com/".parse().unwrap(), Protocol::H2)
            .is_none()
    );
    assert!(
        selector
            .lease(
                &"https://cdn.modrinth.com/".parse().unwrap(),
                Protocol::Http1
            )
            .is_none()
    );
    selector.close();
    assert!(!selector.is_current(&lease));
    assert!(start.cancellation.is_cancelled());
    assert_eq!(
        selector.publish(
            selection("151.101.1.242", now),
            start.epoch,
            start.generation
        ),
        RoundStatus::Stale
    );
}

#[test]
fn failed_endpoint_moves_to_validated_backup_without_resetting_hourly_budget() {
    let selector = EndpointSelector::with_mode("test", SelectionMode::Auto);
    let _registration = selector.register(CancellationToken::new()).unwrap();
    let start = selector.begin(Instant::now()).unwrap();
    assert_eq!(
        selector.publish(
            selection("146.75.113.242", Instant::now()),
            start.epoch,
            start.generation
        ),
        RoundStatus::Selected
    );
    let url = "https://cdn.modrinth.com/data/a".parse().unwrap();
    let lease = selector.lease(&url, Protocol::H2).unwrap();
    selector.record_failure(&lease, false);
    assert!(selector.is_current(&lease));
    selector.record_failure(&lease, false);
    let backup = selector.lease(&url, Protocol::H2).unwrap();
    assert_eq!(backup.pool.endpoint.ip().to_string(), "151.101.65.242");
    assert!(!selector.is_current(&lease));
    assert_eq!(selector.state.lock().hourly_reservations.len(), 1);
    selector.record_failure(&lease, true);
    assert!(selector.is_current(&backup));
}

#[test]
fn cancelled_trials_keep_hourly_reservations_and_expiry_cannot_republish_old_epoch()
 {
    let selector = EndpointSelector::with_mode("test", SelectionMode::Auto);
    let _registration = selector.register(CancellationToken::new()).unwrap();
    for _ in 0..2 {
        selector.state.lock().cooldown_until = None;
        let round = selector.begin(Instant::now()).unwrap();
        drop(round);
    }
    selector.state.lock().cooldown_until = None;
    assert!(selector.begin(Instant::now()).is_none());
    let mut state = selector.state.lock();
    let epoch = state.epoch;
    state.last_activity = Instant::now() - policy::SELECTION_LIFETIME;
    EndpointSelector::expire(&mut state, Instant::now());
    assert_ne!(state.epoch, epoch);
    assert_eq!(state.hourly_reservations.len(), 2);
}

#[tokio::test]
async fn saturated_production_leaves_exactly_one_recovery_slot_across_generations()
 {
    use crate::util::download::{native_budget, proxy_context};
    let mut clients = DownloadClients::build(
        &crate::util::proxy::ProxyConfig {
            mode: ProxyMode::None,
            ..Default::default()
        },
        false,
        false,
    )
    .unwrap();
    clients.scope = "reserved-slot-test".into();
    clients.endpoints =
        EndpointSelector::with_mode(clients.scope.clone(), SelectionMode::Auto);
    let selector = clients.endpoints.clone();
    let route = DownloadRoute {
        url: "https://cdn.modrinth.com/data/a".into(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    proxy_context::with_snapshot(clients, async {
        let limit =
            crate::util::download::constants::MAX_CONNECTIONS_PER_AUTHORITY;
        let mut regular = Vec::new();
        for _ in 0..limit - 1 {
            regular.push(native_budget::try_acquire(&route).unwrap());
        }
        selector.install_test_selection(
            "151.101.1.242:443".parse().unwrap(),
            Protocol::H2,
        );
        assert!(native_budget::try_acquire(&route).is_err());
        let recovery = native_budget::try_acquire_recovery(&route).unwrap();
        assert_eq!(native_budget::available(&route), 0);
        assert!(native_budget::try_acquire_recovery(&route).is_err());
        drop(recovery);
        drop(regular);
        assert_eq!(native_budget::available(&route), limit);
    })
    .await;
}

#[test]
fn same_url_new_endpoint_recovers_only_with_integrity_and_shared_migration_budget()
 {
    let selector =
        EndpointSelector::with_mode("recovery-test", SelectionMode::Auto);
    selector.install_test_selection(
        "151.101.1.242:443".parse().unwrap(),
        Protocol::H2,
    );
    let request = DownloadRequest::new(
        "https://cdn.modrinth.com/data/project/file.jar",
        crate::util::fetch::ResourceClass::Modrinth,
    )
    .with_integrity(
        crate::util::fetch::Integrity::sha1("test").with_size(1024 * 1024),
    );
    let route = DownloadRoute {
        url: request.url.clone(),
        source: crate::util::fetch::DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: true,
        supports_range: true,
        proxy: ProxyPolicy::Direct,
    };
    let previous = Some("151.101.65.242:443".parse().unwrap());
    let clone = request.clone();
    for (index, request) in [&request, &clone].into_iter().enumerate() {
        if index > 0 {
            selector.install_test_selection(
                "151.101.1.242:443".parse().unwrap(),
                Protocol::H2,
            );
        }
        let lease = selector
            .recovery_lease(
                request,
                &route,
                Protocol::H2,
                previous,
                45 * 1024,
                512 * 1024,
            )
            .unwrap();
        assert_eq!(lease.pool.endpoint.ip().to_string(), "151.101.1.242");
        // Other ranges can follow the same published generation without
        // spending another file-level migration.
        assert!(
            selector
                .recovery_lease(
                    request,
                    &route,
                    Protocol::H2,
                    previous,
                    45 * 1024,
                    512 * 1024
                )
                .is_some()
        );
    }
    selector.install_test_selection(
        "151.101.1.242:443".parse().unwrap(),
        Protocol::H2,
    );
    assert!(
        selector
            .recovery_lease(
                &request,
                &route,
                Protocol::H2,
                previous,
                45 * 1024,
                512 * 1024
            )
            .is_none()
    );
    let fresh = DownloadRequest::new(request.url.clone(), request.resource)
        .with_integrity(request.integrity.clone());
    assert!(
        selector
            .recovery_lease(
                &fresh,
                &route,
                Protocol::H2,
                Some("151.101.1.242:443".parse().unwrap()),
                45 * 1024,
                512 * 1024
            )
            .is_none()
    );
    assert!(
        selector
            .recovery_lease(
                &DownloadRequest::new(request.url, request.resource),
                &route,
                Protocol::H2,
                previous,
                45 * 1024,
                512 * 1024
            )
            .is_none()
    );
}
