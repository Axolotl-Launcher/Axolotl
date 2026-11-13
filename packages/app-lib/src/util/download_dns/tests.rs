use super::*;
use hickory_resolver::proto::rr::{
    DNSClass, Record,
    rdata::{A, AAAA, CNAME},
};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn response_with_chain(query: &Message, ttl: u32) -> Message {
    let mut response = query.clone();
    response.metadata.message_type = MessageType::Response;
    let name = query.queries.first().unwrap().name().clone();
    let alias = DnsName::from_ascii("edge.example.").unwrap();
    response.add_answer(Record::from_rdata(
        name,
        ttl,
        RData::CNAME(CNAME(alias.clone())),
    ));
    response.add_answer(Record::from_rdata(
        alias,
        300,
        RData::A(A(Ipv4Addr::new(151, 101, 1, 242))),
    ));
    response
}

#[test]
fn metadata_preserves_source_chain_and_minimum_path_ttl() {
    let query = doh_query("cdn.example", RecordType::A).unwrap();
    let source = DnsSource::Doh(Arc::from("provider-one"));
    let response = response_with_chain(&query, 7);
    let answer =
        parse_doh_metadata(&response.to_vec().unwrap(), &query, source.clone())
            .unwrap();
    assert_eq!(answer.queried_host, "cdn.example");
    assert_eq!(answer.cname_chain, ["edge.example"]);
    assert_eq!(answer.source, source);
    assert_eq!(answer.addresses.len(), 1);
    assert_eq!(
        answer.addresses[0].expires_at - answer.fetched_at,
        Duration::from_secs(7)
    );
}

#[test]
fn provider_metadata_and_candidate_snapshots_have_finite_host_capacity() {
    let resolver = DownloadDnsResolver::default();
    for index in 0..MAX_METADATA_HOSTS + 20 {
        let host = format!("cdn-{index}.example");
        let query = doh_query(&host, RecordType::A).unwrap();
        let answer = parse_doh_metadata(
            &response_with_chain(&query, 60).to_vec().unwrap(),
            &query,
            DnsSource::System,
        )
        .unwrap();
        resolver.remember_answer(&host, answer.clone());
        resolver.remember_candidates(
            host,
            CandidateSnapshot {
                answers: vec![answer],
                fetched_at: Instant::now(),
            },
        );
    }
    assert_eq!(resolver.dns_answers.lock().len(), MAX_METADATA_HOSTS);
    assert_eq!(
        resolver.candidate_snapshots.lock().len(),
        MAX_METADATA_HOSTS
    );
    assert!(!resolver.dns_answers.lock().contains_key("cdn-0.example"));
    assert!(
        !resolver
            .candidate_snapshots
            .lock()
            .contains_key("cdn-0.example")
    );
    assert!(
        resolver
            .cached_candidates(&format!(
                "cdn-{}.example",
                MAX_METADATA_HOSTS + 19
            ))
            .is_some()
    );
}

#[tokio::test]
async fn candidate_host_admission_does_not_grow_past_active_query_capacity() {
    let resolver = DownloadDnsResolver::default();
    let held: Vec<_> = (0..MAX_METADATA_HOSTS)
        .map(|index| {
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            resolver
                .candidate_hosts
                .lock()
                .insert(format!("busy-{index}.example"), lock.clone());
            lock
        })
        .collect();
    let token = CancellationToken::new();
    let error = resolver
        .candidate_snapshot(
            "new.example",
            &token,
            Instant::now() + Duration::from_secs(1),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(resolver.candidate_hosts.lock().len(), MAX_METADATA_HOSTS);
    drop(held);
}

#[test]
fn address_ttl_can_be_shorter_than_cname_and_duplicate_uses_shortest() {
    let query = doh_query("cdn.example", RecordType::A).unwrap();
    let mut response = response_with_chain(&query, 100);
    response.add_answer(Record::from_rdata(
        DnsName::from_ascii("edge.example.").unwrap(),
        2,
        RData::A(A(Ipv4Addr::new(151, 101, 1, 242))),
    ));
    let answer = parse_doh_metadata(
        &response.to_vec().unwrap(),
        &query,
        DnsSource::System,
    )
    .unwrap();
    assert_eq!(answer.addresses.len(), 1);
    assert_eq!(
        answer.addresses[0].expires_at - answer.fetched_at,
        Duration::from_secs(2)
    );
}

#[test]
fn ttl_zero_can_return_once_but_cannot_warm_either_cache() {
    let query = doh_query("cdn.example", RecordType::A).unwrap();
    let response = response_with_chain(&query, 0);
    let answer = parse_doh_metadata(
        &response.to_vec().unwrap(),
        &query,
        DnsSource::System,
    )
    .unwrap();
    assert_eq!(answer.addresses[0].expires_at, answer.fetched_at);
    let resolver = DownloadDnsResolver {
        doh_enabled: true,
        ..Default::default()
    };
    let sockets = answer.socket_addresses();
    resolver.remember_answer("cdn.example", answer.clone());
    let ttl = resolver.doh_cache_ttl("cdn.example", &sockets);
    resolver.cache_addresses("cdn.example".to_string(), sockets, ttl);
    resolver.candidate_snapshots.lock().insert(
        "cdn.example".to_string(),
        CandidateSnapshot {
            answers: vec![answer],
            fetched_at: Instant::now(),
        },
    );
    assert!(resolver.resolved_addresses("cdn.example").is_empty());
    assert!(resolver.cached_candidates("cdn.example").is_none());
}

#[test]
fn rejects_wrong_class_cname_cycles_conflicting_targets_and_unrelated_addresses()
 {
    let query = doh_query("cdn.example", RecordType::A).unwrap();
    let mut wrong_class = response_with_chain(&query, 60);
    wrong_class.answers[0].dns_class = DNSClass::CH;
    let mut cycle = response_with_chain(&query, 60);
    cycle.answers.pop();
    cycle.add_answer(Record::from_rdata(
        DnsName::from_ascii("edge.example.").unwrap(),
        60,
        RData::CNAME(CNAME(query.queries.first().unwrap().name().clone())),
    ));
    let mut conflict = response_with_chain(&query, 60);
    conflict.add_answer(Record::from_rdata(
        query.queries.first().unwrap().name().clone(),
        60,
        RData::CNAME(CNAME(DnsName::from_ascii("other.example.").unwrap())),
    ));
    let mut unrelated = response_with_chain(&query, 60);
    unrelated.answers.remove(0);
    for response in [wrong_class, cycle, conflict, unrelated] {
        assert!(
            parse_doh_metadata(
                &response.to_vec().unwrap(),
                &query,
                DnsSource::System
            )
            .is_err()
        );
    }
}

#[test]
fn rejects_bad_header_question_class_type_truncation_and_oversize() {
    let query = doh_query("cdn.example", RecordType::A).unwrap();
    for invalid in 0..6 {
        let mut response = response_with_chain(&query, 60);
        match invalid {
            0 => {
                response.metadata.id = query.metadata.id.wrapping_add(1);
            }
            1 => {
                response.metadata.truncation = true;
            }
            2 => {
                response.queries[0].set_query_class(DNSClass::CH);
            }
            3 => {
                response.queries[0].set_query_type(RecordType::AAAA);
            }
            4 => {
                response.metadata.response_code = ResponseCode::ServFail;
            }
            5 => {
                response.queries[0]
                    .set_name(DnsName::from_ascii("other.example.").unwrap());
            }
            _ => unreachable!(),
        }
        assert!(
            parse_doh_metadata(
                &response.to_vec().unwrap(),
                &query,
                DnsSource::System
            )
            .is_err()
        );
    }
    assert!(
        parse_doh_metadata(
            &vec![0; MAX_DNS_MESSAGE_BYTES + 1],
            &query,
            DnsSource::System
        )
        .is_err()
    );
}

pub(crate) struct DohFixture {
    endpoint: Arc<str>,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DohFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DohFixture {
    pub(crate) async fn start(
        ipv4_delay: Duration,
        ipv6_delay: Duration,
    ) -> Self {
        Self::start_with_ipv4(
            ipv4_delay,
            ipv6_delay,
            Ipv4Addr::new(151, 101, 1, 242),
        )
        .await
    }

    pub(crate) async fn start_with_ipv4(
        ipv4_delay: Duration,
        ipv6_delay: Duration,
        ipv4_address: Ipv4Addr,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let endpoint = Arc::from(format!(
            "http://{}/dns-query",
            listener.local_addr().unwrap()
        ));
        let requests = Arc::new(AtomicUsize::new(0));
        let counted = requests.clone();
        let task = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut stream, _)) = accepted else { break };
                        let counted = counted.clone();
                        handlers.spawn(async move {
                            let mut bytes = Vec::new();
                            let body_start = loop {
                                let mut chunk = [0; 1024];
                                let read = stream.read(&mut chunk).await.unwrap();
                                if read == 0 { return; }
                                bytes.extend_from_slice(&chunk[..read]);
                                if bytes.len() > MAX_DNS_MESSAGE_BYTES + 4096 { return; }
                                if let Some(start) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                                    break start + 4;
                                }
                            };
                            let headers = String::from_utf8_lossy(&bytes[..body_start]);
                            let length = headers.lines().find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            }).unwrap();
                            while bytes.len() < body_start + length {
                                let mut chunk = [0; 1024];
                                let read = stream.read(&mut chunk).await.unwrap();
                                if read == 0 { return; }
                                bytes.extend_from_slice(&chunk[..read]);
                            }
                            let query = Message::from_vec(&bytes[body_start..body_start + length]).unwrap();
                            counted.fetch_add(1, Ordering::SeqCst);
                            let ipv4 = query.queries.first().unwrap().query_type() == RecordType::A;
                            tokio::time::sleep(if ipv4 { ipv4_delay } else { ipv6_delay }).await;
                            let mut response = if ipv4 {
                                let mut response = query.clone();
                                response.metadata.message_type = MessageType::Response;
                                response.add_answer(Record::from_rdata(
                                    query.queries.first().unwrap().name().clone(), 60,
                                    RData::A(A(ipv4_address)),
                                ));
                                // Preserve metadata coverage for the ordinary fixture.
                                if ipv4_address == Ipv4Addr::new(151, 101, 1, 242) {
                                    response = response_with_chain(&query, 60);
                                }
                                response
                            } else {
                                let mut response = query.clone();
                                response.metadata.message_type = MessageType::Response;
                                response.add_answer(Record::from_rdata(
                                    query.queries.first().unwrap().name().clone(), 60,
                                    RData::AAAA(AAAA(Ipv6Addr::new(0x2a04, 0x4e42, 0, 0, 0, 0, 0, 0x498))),
                                ));
                                response
                            };
                            response.metadata.id = query.metadata.id;
                            let body = response.to_vec().unwrap();
                            let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                            if stream.write_all(headers.as_bytes()).await.is_ok() {
                                let _ = stream.write_all(&body).await;
                            }
                        });
                    }
                    result = handlers.join_next(), if !handlers.is_empty() => {
                        result.unwrap().unwrap();
                    }
                }
            }
        });
        Self {
            endpoint,
            requests,
            task,
        }
    }
}

pub(crate) fn fixture_resolver(
    fixtures: &[&DohFixture],
) -> DownloadDnsResolver {
    let resolver = DownloadDnsResolver {
        doh_enabled: true,
        doh_endpoints: Arc::new(
            fixtures.iter().map(|f| f.endpoint.clone()).collect(),
        ),
        doh_clients: Arc::new(
            fixtures
                .iter()
                .map(|_| {
                    crate::util::http::client_builder()
                        .no_proxy()
                        .build()
                        .unwrap()
                })
                .collect(),
        ),
        ..Default::default()
    };
    resolver.set_test_system_answer("cdn.example", Duration::ZERO, Vec::new());
    resolver
}

#[tokio::test]
async fn slow_system_fallback_has_the_full_caller_budget() {
    let stalled =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    let resolver = fixture_resolver(&[&stalled]);
    resolver.test_system_answers.lock().insert(
        "slow-system.example".into(),
        (Duration::from_secs(4), vec!["127.0.0.1:0".parse().unwrap()]),
    );
    let addresses = tokio::time::timeout(
        Duration::from_secs(5),
        resolver.resolve_host("slow-system.example"),
    )
    .await
    .expect("system DNS must not lose three seconds waiting for DoH")
    .unwrap();
    assert_eq!(addresses, vec!["127.0.0.1".parse::<IpAddr>().unwrap()]);
    assert!(stalled.requests.load(Ordering::SeqCst) > 0);
    let cache = resolver.last_resolved.lock();
    assert_eq!(cache.get("slow-system.example").unwrap().ttl, CACHE_TTL);
}

#[tokio::test]
async fn system_failure_keeps_fast_doh_usable() {
    let fast = DohFixture::start(Duration::ZERO, Duration::ZERO).await;
    let resolver = fixture_resolver(&[&fast]);
    resolver
        .test_system_answers
        .lock()
        .insert("cdn.example".into(), (Duration::ZERO, Vec::new()));
    let addresses = tokio::time::timeout(
        Duration::from_secs(2),
        resolver.resolve_host("cdn.example"),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(addresses.contains(&"151.101.1.242".parse().unwrap()));
    assert!(
        resolver
            .last_resolved
            .lock()
            .get("cdn.example")
            .unwrap()
            .ttl
            <= Duration::from_secs(60)
    );
}

#[tokio::test]
async fn system_recovery_keeps_parallel_doh_result_when_system_stalls() {
    let fast =
        DohFixture::start(Duration::from_secs(2), Duration::from_secs(2)).await;
    let resolver = fixture_resolver(&[&fast]);
    resolver.cache_addresses(
        "cdn.example".into(),
        vec!["127.0.0.2:0".parse().unwrap()],
        CACHE_TTL,
    );
    resolver.request_system_recovery("cdn.example");
    resolver.set_test_system_answer(
        "cdn.example",
        Duration::from_secs(10),
        vec!["127.0.0.1:0".parse().unwrap()],
    );
    let addresses = tokio::time::timeout(
        Duration::from_secs(4),
        resolver.resolve_host("cdn.example"),
    )
    .await
    .expect("DoH should already be ready when the system recovery budget ends")
    .unwrap();
    assert!(addresses.contains(&"151.101.1.242".parse().unwrap()));
}

#[tokio::test]
async fn body_failure_after_successful_headers_recovers_only_redirect_target() {
    use crate::util::fetch::{
        DownloadClients, DownloadRoute, DownloadRouteSource, ProxyPolicy,
        request_dns_system_recovery,
    };
    let clients =
        DownloadClients::build(&Default::default(), false, true).unwrap();
    let resolver = clients.system_dns.clone();
    for host in ["origin.example", "target.example"] {
        resolver.cache_addresses(
            host.into(),
            vec!["127.0.0.2:0".parse().unwrap()],
            CACHE_TTL,
        );
    }
    let route = DownloadRoute {
        url: "https://origin.example/file".into(),
        source: DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: false,
        supports_range: true,
        proxy: ProxyPolicy::System,
    };
    crate::util::download::proxy_context::with_snapshot(clients, async {
        // Headers succeeding on each attempt used to reset the two-failure
        // counter and keep the bad edge cached forever.
        for _ in 0..2 {
            resolver.record_host_success(
                "target.example",
                "127.0.0.2".parse().unwrap(),
            );
            request_dns_system_recovery(&route, "https://target.example/file");
        }
    })
    .await;
    assert!(resolver.resolved_addresses("target.example").is_empty());
    assert!(!resolver.resolved_addresses("origin.example").is_empty());
    assert!(
        resolver
            .last_resolved
            .lock()
            .get("target.example")
            .unwrap()
            .prefer_system
    );
}

#[test]
fn mixed_system_answers_replace_each_other_without_losing_provenance() {
    let resolver = DownloadDnsResolver::default();
    let now = Instant::now();
    let answer = |source, ips: &[&str]| DnsAnswer {
        queried_host: "cdn.example".into(),
        cname_chain: Vec::new(),
        addresses: ips
            .iter()
            .map(|ip| DnsAddress {
                ip: ip.parse().unwrap(),
                expires_at: now + CACHE_TTL,
            })
            .collect(),
        fetched_at: now,
        source,
    };
    resolver.remember_answer(
        "cdn.example",
        answer(DnsSource::System, &["198.18.0.1", "::1"]),
    );
    resolver.remember_answer(
        "cdn.example",
        answer(DnsSource::System, &["::1", "198.18.0.1"]),
    );
    for provider in 0..BUILTIN_DOH_SERVERS.len() {
        let source = DnsSource::Doh(Arc::from(format!("provider-{provider}")));
        resolver.remember_answer(
            "cdn.example",
            answer(source.clone(), &["151.101.1.242"]),
        );
        resolver.remember_answer(
            "cdn.example",
            answer(source, &["2a04:4e42::498"]),
        );
    }
    let cache = resolver.dns_answers.lock();
    let answers = cache.get("cdn.example").unwrap();
    assert_eq!(answers.len(), BUILTIN_DOH_SERVERS.len() * 2 + 1);
    assert_eq!(
        answers
            .iter()
            .filter(|answer| answer.source == DnsSource::System)
            .count(),
        1
    );
}

#[tokio::test]
async fn native_truncated_body_retries_redirect_target_through_system_dns() {
    use crate::util::fetch::{
        DownloadClients, DownloadRequest, DownloadRoute, DownloadRouteSource,
        FetchSemaphore, Integrity, ProxyPolicy, ResourceClass,
        run_native_download_attempts,
    };
    use crate::util::proxy::{ProxyConfig, ProxyMode};
    let bad_listener = tokio::net::TcpListener::bind(("127.0.0.2", 0))
        .await
        .unwrap();
    let port = bad_listener.local_addr().unwrap().port();
    let good_listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let target = format!("http://localhost:{port}/file");
    let bad_target = target.clone();
    let bad = tokio::spawn(async move {
        loop {
            let (mut stream, _) = bad_listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let read = stream.read(&mut buffer).await.unwrap();
            if String::from_utf8_lossy(&buffer[..read]).contains("/redirect") {
                stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {bad_target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            } else {
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nx").await.unwrap();
            }
        }
    });
    let good = tokio::spawn(async move {
        let (mut stream, _) = good_listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        stream.read(&mut buffer).await.unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
    });
    let mut clients = DownloadClients::build(
        &ProxyConfig {
            mode: ProxyMode::None,
            ..Default::default()
        },
        false,
        true,
    )
    .unwrap();
    clients.scope = format!("dns-body-{port}");
    let resolver = clients.system_dns.clone();
    for host in ["origin.example", "localhost"] {
        resolver.cache_addresses(
            host.into(),
            vec!["127.0.0.2:0".parse().unwrap()],
            CACHE_TTL,
        );
    }
    resolver.set_test_system_answer(
        "localhost",
        Duration::ZERO,
        vec!["127.0.0.1:0".parse().unwrap()],
    );
    let origin = format!("http://origin.example:{port}/redirect");
    let route = DownloadRoute {
        url: origin.clone(),
        source: DownloadRouteSource::Official,
        is_mirror: false,
        allow_sensitive_headers: false,
        supports_range: false,
        proxy: ProxyPolicy::System,
    };
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("file.bin");
    let part = directory.path().join("file.bin.part");
    let result = crate::util::download::proxy_context::with_snapshot(
        clients,
        tokio::time::timeout(
            Duration::from_secs(20),
            run_native_download_attempts(
                DownloadRequest::new(&origin, ResourceClass::Other)
                    .with_integrity(Integrity::default().with_size(2)),
                &destination,
                &FetchSemaphore(tokio::sync::Semaphore::new(8)),
                None,
                vec![route],
                part,
                None,
            ),
        ),
    )
    .await;
    bad.abort();
    good.abort();
    result.unwrap().unwrap();
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"ok");
    assert_eq!(
        resolver.resolved_addresses("localhost"),
        vec!["127.0.0.1".parse::<IpAddr>().unwrap()]
    );
    assert_eq!(
        resolver.resolved_addresses("origin.example"),
        vec!["127.0.0.2".parse::<IpAddr>().unwrap()]
    );
}

#[tokio::test]
async fn failed_valid_doh_address_recovers_through_system_dns() {
    let fast = DohFixture::start(Duration::ZERO, Duration::ZERO).await;
    let resolver = fixture_resolver(&[&fast]);
    resolver.test_system_answers.lock().insert(
        "cdn.example".into(),
        (
            Duration::from_millis(200),
            vec!["127.0.0.1:0".parse().unwrap()],
        ),
    );
    // The real DoH fixture returns a valid remote address before system DNS.
    let initial = resolver.resolve_host("cdn.example").await.unwrap();
    assert!(initial.contains(&"151.101.1.242".parse().unwrap()));
    assert!(!resolver.record_connection_failure("cdn.example"));
    assert!(resolver.record_connection_failure("cdn.example"));
    resolver.record_host_success("cdn.example", initial[0]);
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 2048];
        stream.read(&mut buffer).await.unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
    });
    let client = crate::util::http::client_builder()
        .no_proxy()
        .dns_resolver(Arc::new(resolver.clone()))
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let body = client
        .get(format!("http://cdn.example:{port}/file"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "ok");
    server.await.unwrap();
    assert_eq!(
        resolver.resolved_addresses("cdn.example"),
        vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
    );
}

#[tokio::test]
async fn stalled_doh_candidates_include_system_provenance() {
    let stalled =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    let resolver = fixture_resolver(&[&stalled]);
    resolver.test_system_answers.lock().insert(
        "cdn.example".into(),
        (
            Duration::from_millis(20),
            vec!["198.18.0.1:0".parse().unwrap()],
        ),
    );
    let answers = resolver
        .candidate_snapshot(
            "cdn.example",
            &CancellationToken::new(),
            Instant::now() + Duration::from_millis(100),
        )
        .await
        .unwrap();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].source, DnsSource::System);
    assert_eq!(
        answers[0].addresses[0].ip,
        "198.18.0.1".parse::<IpAddr>().unwrap()
    );
}

#[tokio::test]
async fn sustained_slow_address_expires_without_aborting_current_connection() {
    let fast = DohFixture::start(Duration::ZERO, Duration::ZERO).await;
    let resolver = fixture_resolver(&[&fast]);
    resolver.test_system_answers.lock().insert(
        "cdn.example".into(),
        (
            Duration::from_millis(200),
            vec!["127.0.0.1:0".parse().unwrap()],
        ),
    );
    let addresses = resolver.resolve_host("cdn.example").await.unwrap();
    resolver.request_system_recovery("cdn.example");
    resolver.record_host_success("cdn.example", addresses[0]);
    assert!(resolver.resolved_addresses("cdn.example").is_empty());
    assert_eq!(
        resolver.resolve_host("cdn.example").await.unwrap(),
        vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
    );
}

#[tokio::test]
async fn first_usable_provider_does_not_wait_for_stalled_provider() {
    let fast = DohFixture::start(Duration::ZERO, Duration::ZERO).await;
    let slow =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    // The third configured provider is still usable when the preferred two
    // stall; the bounded pool must not silently discard it.
    let other_slow =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    let resolver = fixture_resolver(&[&slow, &other_slow, &fast]);
    let addresses = tokio::time::timeout(
        Duration::from_secs(2),
        resolver.lookup_doh_family("cdn.example", RecordType::A),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(addresses.len(), 1);
    assert_eq!(
        addresses[0].ip(),
        IpAddr::V4(Ipv4Addr::new(151, 101, 1, 242))
    );
}

#[tokio::test]
async fn collection_keeps_partial_answers_and_sources_at_deadline() {
    let fast = DohFixture::start(Duration::ZERO, Duration::from_secs(10)).await;
    let slow =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    let resolver = fixture_resolver(&[&fast, &slow]);
    let answers = tokio::time::timeout(
        Duration::from_secs(2),
        resolver.candidate_snapshot(
            "cdn.example",
            &CancellationToken::new(),
            Instant::now() + Duration::from_millis(300),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].source, DnsSource::Doh(fast.endpoint.clone()));
    assert_eq!(answers[0].cname_chain, ["edge.example"]);
}

#[tokio::test]
async fn concurrent_candidate_readers_share_snapshot_and_keep_families() {
    let fixture =
        DohFixture::start(Duration::from_millis(40), Duration::from_millis(40))
            .await;
    let resolver = fixture_resolver(&[&fixture]);
    let mut jobs = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let resolver = resolver.clone();
        jobs.spawn(async move {
            resolver
                .candidate_snapshot(
                    "cdn.example",
                    &CancellationToken::new(),
                    Instant::now() + Duration::from_secs(2),
                )
                .await
                .unwrap()
        });
    }
    while let Some(result) = jobs.join_next().await {
        let answers = result.unwrap();
        assert_eq!(answers.len(), 2);
        assert_eq!(
            answers.iter().flat_map(|answer| &answer.addresses).count(),
            2
        );
    }
    assert_eq!(fixture.requests.load(Ordering::SeqCst), 2);
    // Candidate metadata must not replace the ordinary resolver's truth.
    assert!(resolver.resolved_addresses("cdn.example").is_empty());
}

#[tokio::test]
async fn disabled_doh_candidates_use_system_path_without_contacting_providers()
{
    let fixture = DohFixture::start(Duration::ZERO, Duration::ZERO).await;
    let mut resolver = fixture_resolver(&[&fixture]);
    resolver.doh_enabled = false;
    resolver.set_test_addresses(
        "cdn.example",
        vec!["151.101.1.242:0".parse().unwrap()],
    );
    let answer = resolver
        .candidate_snapshot(
            "cdn.example",
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap()
        .remove(0);
    assert_eq!(answer.source, DnsSource::System);
    assert!(answer.cname_chain.is_empty());
    assert!(answer.addresses[0].expires_at - answer.fetched_at <= CACHE_TTL);
    assert_eq!(fixture.requests.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancellation_and_host_lock_wait_are_bounded() {
    let fixture =
        DohFixture::start(Duration::from_secs(10), Duration::from_secs(10))
            .await;
    let resolver = fixture_resolver(&[&fixture]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = resolver
        .candidate_snapshot(
            "cdn.example",
            &cancel,
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(fixture.requests.load(Ordering::SeqCst), 0);

    let lock = Arc::new(tokio::sync::Mutex::new(()));
    resolver
        .candidate_hosts
        .lock()
        .insert("cdn.example".to_string(), lock.clone());
    let _guard = lock.lock().await;
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        resolver.candidate_snapshot(
            "cdn.example",
            &CancellationToken::new(),
            Instant::now() + Duration::from_millis(50),
        ),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}
