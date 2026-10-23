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

struct DohFixture {
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
    async fn start(ipv4_delay: Duration, ipv6_delay: Duration) -> Self {
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
                                response_with_chain(&query, 60)
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

fn fixture_resolver(fixtures: &[&DohFixture]) -> DownloadDnsResolver {
    DownloadDnsResolver {
        doh_enabled: true,
        doh_endpoints: Arc::new(
            fixtures.iter().map(|f| f.endpoint.clone()).collect(),
        ),
        doh_clients: Arc::new(
            fixtures
                .iter()
                .map(|_| reqwest::Client::builder().no_proxy().build().unwrap())
                .collect(),
        ),
        ..Default::default()
    }
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
