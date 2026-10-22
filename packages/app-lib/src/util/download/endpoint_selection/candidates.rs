use super::fastly_rules;
use super::model::{CandidateSource, EndpointCandidate};
use super::policy::MAX_CANDIDATES;
use crate::util::download::constants::MODRINTH_DATA_PREFIX;
use crate::util::download::modrinth_cdn;
use crate::util::download_dns::DnsAnswer;
use crate::util::fetch::Integrity;
use std::collections::HashSet;
use std::net::IpAddr;
use std::time::{Instant, SystemTime};
use url::Url;

pub(super) fn eligible_target(url: &Url, integrity: &Integrity) -> bool {
    modrinth_cdn::is_current_url(url)
        && url.path().starts_with(MODRINTH_DATA_PREFIX)
        && url.query().is_none()
        && integrity.supports_resume()
        && integrity.size.is_some_and(|size| size > 0)
}

/// Returns no candidates on synthetic system DNS answers. The ordinary
/// transport keeps following the user's network instead of bypassing a TUN.
pub(super) fn collect_candidates(
    answers: &[DnsAnswer],
    verified: &[EndpointCandidate],
    history: &[EndpointCandidate],
    expand_catalog: bool,
    now: Instant,
    wall_time: SystemTime,
) -> Vec<EndpointCandidate> {
    let history = if expand_catalog { history } else { &[] };
    if answers.iter().any(|answer| {
        matches!(answer.source, crate::util::download_dns::DnsSource::System)
            && answer
                .addresses
                .iter()
                .any(|address| is_fake_ip(address.ip))
    }) {
        return Vec::new();
    }
    let dns: Vec<_> = answers
        .iter()
        .filter(|answer| answer.queried_host == modrinth_cdn::HOST)
        .flat_map(|answer| {
            answer.addresses.iter().map(|address| EndpointCandidate {
                ip: address.ip,
                source: CandidateSource::Dns(answer.source.clone()),
                expires_at: address.expires_at,
            })
        })
        .collect();
    let seeds = if expand_catalog {
        fastly_rules::seed_candidates(answers, now, wall_time)
    } else {
        Vec::new()
    };
    merge_candidates(&dns, verified, history, &seeds, now)
}

fn merge_candidates(
    dns: &[EndpointCandidate],
    verified: &[EndpointCandidate],
    history: &[EndpointCandidate],
    seeds: &[EndpointCandidate],
    now: Instant,
) -> Vec<EndpointCandidate> {
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    // Interleave each source's families before imposing the shared cap. A
    // long IPv4 list must not eliminate all IPv6 candidates (or vice versa).
    for source in [dns, verified, history, seeds] {
        let usable: Vec<_> = source
            .iter()
            .filter(|candidate| {
                candidate.expires_at > now && is_public_ip(candidate.ip)
            })
            .collect();
        let mut ipv4 = usable
            .iter()
            .copied()
            .filter(|candidate| candidate.ip.is_ipv4());
        let mut ipv6 = usable
            .iter()
            .copied()
            .filter(|candidate| candidate.ip.is_ipv6());
        loop {
            let pair = [ipv4.next(), ipv6.next()];
            if pair.iter().all(Option::is_none) {
                break;
            }
            for candidate in pair.into_iter().flatten() {
                if seen.insert(candidate.ip) {
                    merged.push(candidate.clone());
                    if merged.len() == MAX_CANDIDATES {
                        return merged;
                    }
                }
            }
        }
    }
    merged
}

fn is_fake_ip(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(ip) if ip.octets()[0] == 198 && ip.octets()[1] & 0xfe == 18)
}

pub(super) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || a == 100 && (64..=127).contains(&b)
                || a == 169 && b == 254
                || a == 172 && (16..=31).contains(&b)
                || a == 192 && b == 168
                || a == 192 && b == 0 && (c == 0 || c == 2)
                || a == 192 && b == 88 && c == 99
                || a == 198 && (b == 18 || b == 19)
                || a == 198 && b == 51 && c == 100
                || a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            let [a, b, ..] = ip.segments();
            // Conservative native global-unicast scope; exclude mapped,
            // translation, transition, benchmark and documentation ranges.
            a & 0xe000 == 0x2000
                && a != 0x2002
                && !(a == 0x2001 && (b < 0x0200 || b == 0x0db8))
                && !(a == 0x3fff && b & 0xf000 == 0)
        }
    }
}
