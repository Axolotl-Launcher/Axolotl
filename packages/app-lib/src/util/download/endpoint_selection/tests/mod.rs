mod policy;

use super::candidates::{collect_candidates, eligible_target, is_public_ip};
use super::fastly_rules::seed_candidates;
use super::model::{CandidateSource, EndpointCandidate};
use crate::util::download::modrinth_cdn;
use crate::util::download_dns::{DnsAddress, DnsAnswer, DnsSource};
use crate::util::fetch::Integrity;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use url::Url;

fn reviewed_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_792_540_800)
}

fn pool_answer(now: Instant, suffix: u8) -> DnsAnswer {
    DnsAnswer {
        queried_host: modrinth_cdn::HOST.to_string(),
        cname_chain: vec!["72b69f.atlas.fastly.net".to_string()],
        addresses: [1, 65, 129, 193]
            .into_iter()
            .map(|c| DnsAddress {
                ip: std::net::Ipv4Addr::new(151, 101, c, suffix).into(),
                expires_at: now + Duration::from_secs(60),
            })
            .collect(),
        fetched_at: now,
        source: DnsSource::Doh(Arc::from("provider-one")),
    }
}

#[test]
fn only_public_official_https_files_with_size_and_hash_are_probe_targets() {
    let integrity = Integrity::sha1("test-hash").with_size(1024);
    let good = "https://cdn.modrinth.com/data/project/version/file.jar";
    assert!(eligible_target(&Url::parse(good).unwrap(), &integrity));
    for url in [
        "http://cdn.modrinth.com/data/project/file.jar",
        "https://api.modrinth.com/data/project/file.jar",
        "https://cdn-raw.modrinth.com/data/project/file.jar",
        "https://cdn.modrinth.com.evil.example/data/project/file.jar",
        "https://cdn.modrinth.com:8443/data/project/file.jar",
        "https://user:secret@cdn.modrinth.com/data/project/file.jar",
        "https://cdn.modrinth.com/",
        "https://cdn.modrinth.com/data/project/file.jar?token=secret",
    ] {
        assert!(
            !eligible_target(&Url::parse(url).unwrap(), &integrity),
            "{url}"
        );
    }
    for integrity in [
        Integrity::default(),
        Integrity::sha1("hash"),
        Integrity::default().with_size(1024),
        Integrity::sha1("hash").with_size(0),
    ] {
        assert!(!eligible_target(&Url::parse(good).unwrap(), &integrity));
    }
}

#[test]
fn seed_rule_uses_pool_suffix_without_assuming_242_or_fixed_pop() {
    let now = Instant::now();
    let seeds = seed_candidates(&[pool_answer(now, 212)], now, reviewed_time());
    assert_eq!(
        seeds
            .iter()
            .map(|seed| seed.ip.to_string())
            .collect::<Vec<_>>(),
        ["146.75.113.212", "151.101.41.212", "146.75.93.212",]
    );
    assert!(
        seeds
            .iter()
            .all(|seed| seed.expires_at == now + Duration::from_secs(60)
                && seed.source == CandidateSource::FastlyRule(1))
    );
}

#[test]
fn seed_rule_disables_on_incomplete_mismatched_or_conflicting_evidence() {
    let now = Instant::now();
    let good = pool_answer(now, 242);
    for case in 0..7 {
        let mut changed = good.clone();
        match case {
            0 => {
                changed.cname_chain = vec!["other.atlas.fastly.net".into()];
            }
            1 => {
                changed.addresses[0].ip = "151.101.1.42".parse().unwrap();
            }
            2 => {
                changed.addresses.pop();
            }
            3 => {
                changed.queried_host = "api.modrinth.com".into();
            }
            4 => {
                changed.source = DnsSource::System;
            }
            5 => {
                changed.addresses[0].ip = "146.75.113.242".parse().unwrap();
            }
            6 => {
                for a in &mut changed.addresses {
                    a.expires_at = now;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            seed_candidates(&[changed], now, reviewed_time()).is_empty(),
            "case {case}"
        );
    }
    let mut conflicting = good.clone();
    conflicting.source = DnsSource::Doh(Arc::from("provider-two"));
    conflicting.cname_chain = vec!["other.atlas.fastly.net".into()];
    assert!(
        seed_candidates(
            &[good.clone(), conflicting.clone()],
            now,
            reviewed_time()
        )
        .is_empty()
    );
    // Conflict only disables expansion: normal DNS endpoints are still usable.
    let candidates = collect_candidates(
        &[good, conflicting],
        &[],
        &[],
        true,
        now,
        reviewed_time(),
    );
    assert_eq!(candidates.len(), 4);
    assert!(
        candidates
            .iter()
            .all(|c| matches!(c.source, CandidateSource::Dns(_)))
    );
}

#[test]
fn catalog_expiry_or_wall_clock_before_review_disables_only_expansion() {
    let now = Instant::now();
    let answer = pool_answer(now, 242);
    for wall in [
        reviewed_time() - Duration::from_secs(1),
        reviewed_time() + Duration::from_secs(90 * 86400),
    ] {
        assert!(seed_candidates(&[answer.clone()], now, wall).is_empty());
        assert_eq!(
            collect_candidates(&[answer.clone()], &[], &[], true, now, wall)
                .len(),
            4
        );
    }
}

#[test]
fn candidates_are_deduplicated_bounded_and_preserve_address_families() {
    let now = Instant::now();
    let mut answer = pool_answer(now, 242);
    answer.addresses = (1..=30)
        .map(|d| DnsAddress {
            ip: std::net::Ipv4Addr::new(151, 101, 1, d).into(),
            expires_at: now + Duration::from_secs(60),
        })
        .collect();
    answer.addresses.push(DnsAddress {
        ip: "2a04:4e42::498".parse().unwrap(),
        expires_at: now + Duration::from_secs(60),
    });
    let candidates = collect_candidates(
        &[answer.clone(), answer],
        &[],
        &[],
        false,
        now,
        reviewed_time(),
    );
    assert_eq!(candidates.len(), 16);
    assert!(candidates.iter().any(|candidate| candidate.ip.is_ipv6()));
    let unique: std::collections::HashSet<_> =
        candidates.iter().map(|c| c.ip).collect();
    assert_eq!(unique.len(), candidates.len());
}

#[test]
fn dns_only_catalog_mode_does_not_reintroduce_seed_ips_from_history() {
    let now = Instant::now();
    let history = EndpointCandidate {
        ip: "146.75.113.242".parse().unwrap(),
        source: CandidateSource::History,
        expires_at: now + Duration::from_secs(60),
    };
    let candidates = collect_candidates(
        &[pool_answer(now, 242)],
        &[],
        &[history],
        false,
        now,
        reviewed_time(),
    );
    assert_eq!(candidates.len(), 4);
    assert!(
        candidates.iter().all(|candidate| matches!(
            candidate.source,
            CandidateSource::Dns(_)
        ))
    );
}

#[test]
fn expired_nonpublic_and_mapped_addresses_are_rejected() {
    for ip in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.1.1",
        "172.16.0.1",
        "192.168.0.1",
        "192.0.2.1",
        "198.18.1.2",
        "198.19.255.255",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "fe80::1",
        "fc00::1",
        "ff02::1",
        "::ffff:127.0.0.1",
        "::ffff:198.18.1.2",
        "2001:db8::1",
        "2001:2::1",
        "3fff::1",
        "2002:a00:1::1",
    ] {
        assert!(!is_public_ip(ip.parse().unwrap()), "{ip}");
    }
    for ip in ["151.101.1.242", "146.75.113.242", "2a04:4e42::498"] {
        assert!(is_public_ip(ip.parse().unwrap()), "{ip}");
    }
    let now = Instant::now();
    let history = EndpointCandidate {
        ip: "146.75.113.242".parse().unwrap(),
        source: CandidateSource::History,
        expires_at: now,
    };
    assert!(
        collect_candidates(&[], &[], &[history], true, now, reviewed_time())
            .is_empty()
    );
}

#[test]
fn fake_system_dns_suspends_pinning_even_with_public_doh_evidence() {
    let now = Instant::now();
    let good = pool_answer(now, 242);
    let mut fake = good.clone();
    fake.source = DnsSource::System;
    fake.cname_chain.clear();
    fake.addresses[0].ip = "198.18.1.236".parse().unwrap();
    assert!(
        collect_candidates(&[good, fake], &[], &[], true, now, reviewed_time())
            .is_empty()
    );
}
