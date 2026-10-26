use super::model::{CandidateSource, EndpointCandidate};
use crate::util::download::constants::ENDPOINT_HINT_LIFETIME;
use crate::util::download::modrinth_cdn;
use crate::util::download_dns::{DnsAnswer, DnsSource};
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::util::download::constants::{
    FASTLY_ATLAS_SEED_PREFIXES as ATLAS_SEED_PREFIXES,
    FASTLY_CATALOG_LIFETIME as CATALOG_LIFETIME,
    FASTLY_CATALOG_VERSION as CATALOG_VERSION,
    FASTLY_KNOWN_CNAME as KNOWN_CNAME,
    FASTLY_POOL_THIRD_OCTETS as POOL_THIRD_OCTETS,
    FASTLY_REVIEWED_UNIX_SECONDS as REVIEWED_UNIX_SECONDS,
    FASTLY_SEED_PREFIXES as SEED_PREFIXES,
};
// Reviewed against the 2026-10-24 study and 2026-10-25 DoH/TLS checks. These
// prefixes remain untrusted seeds; every use requires TLS and Range checks.
pub(super) fn seed_candidates(
    answers: &[DnsAnswer],
    now: Instant,
    wall_time: SystemTime,
) -> Vec<EndpointCandidate> {
    let reviewed = UNIX_EPOCH + Duration::from_secs(REVIEWED_UNIX_SECONDS);
    let expires = reviewed + CATALOG_LIFETIME;
    if wall_time < reviewed || wall_time >= expires {
        return Vec::new();
    }
    let fresh: Vec<_> = answers
        .iter()
        .filter(|answer| answer.addresses.iter().any(|a| a.expires_at > now))
        .collect();
    if fresh.is_empty()
        || fresh.iter().any(|answer| {
            answer.queried_host != modrinth_cdn::HOST
                || !matches!(answer.source, DnsSource::Doh(_))
                || answer.cname_chain.as_slice() != [KNOWN_CNAME]
        })
    {
        return Vec::new();
    }
    let mut suffix = None;
    let mut atlas = None;
    let mut seen = [false; 4];
    let mut valid_until = now + ENDPOINT_HINT_LIFETIME;
    for answer in fresh {
        for address in answer.addresses.iter().filter(|a| a.expires_at > now) {
            valid_until = valid_until.min(address.expires_at);
            let IpAddr::V4(ip) = address.ip else { continue };
            let [a, b, c, d] = ip.octets();
            let pool_index = (a == 151 && b == 101)
                .then(|| POOL_THIRD_OCTETS.iter().position(|&octet| octet == c))
                .flatten();
            let is_atlas = c % 4 == 1
                && (a == 146 && b == 75
                    || a == 151 && b == 101 && pool_index.is_none());
            if (!is_atlas && pool_index.is_none())
                || atlas.is_some_and(|atlas| atlas != is_atlas)
                || suffix.is_some_and(|suffix| suffix != d)
            {
                return Vec::new();
            }
            atlas = Some(is_atlas);
            suffix = Some(d);
            if let Some(index) = pool_index {
                seen[index] = true;
            }
        }
    }
    let Some(suffix) = suffix
        .filter(|_| atlas == Some(true) || seen.into_iter().all(|value| value))
    else {
        return Vec::new();
    };
    valid_until =
        valid_until.min(now + expires.duration_since(wall_time).unwrap());
    let prefixes = if atlas == Some(true) {
        ATLAS_SEED_PREFIXES
    } else {
        SEED_PREFIXES
    };
    prefixes
        .into_iter()
        .map(|[a, b, c]| EndpointCandidate {
            ip: Ipv4Addr::new(a, b, c, suffix).into(),
            source: CandidateSource::FastlyRule(CATALOG_VERSION),
            expires_at: valid_until,
        })
        .collect()
}
