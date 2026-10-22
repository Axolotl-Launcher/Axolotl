use super::model::{Protocol, ThroughputSample};
use crate::util::download::constants::{
    ENDPOINT_MAX_RATE_SPREAD, ENDPOINT_MIN_SAMPLE_TIME,
    ENDPOINT_MIN_SUCCESS_SAMPLES,
};
use crate::util::download::native_slow;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Instant;

pub(super) use crate::util::download::constants::{
    ENDPOINT_COOLDOWN as COOLDOWN, ENDPOINT_HOURLY_BYTES as HOURLY_BYTES,
    ENDPOINT_MAX_CANDIDATES as MAX_CANDIDATES,
    ENDPOINT_MAX_MEASURED_CANDIDATES as MAX_MEASURED_CANDIDATES,
    ENDPOINT_ROUND_BYTES as ROUND_BYTES,
    ENDPOINT_ROUND_TIMEOUT as ROUND_TIMEOUT,
    ENDPOINT_SAMPLE_BYTES as SAMPLE_BYTES,
    ENDPOINT_SAMPLE_TIMEOUT as SAMPLE_TIMEOUT,
    ENDPOINT_SELECTION_LIFETIME as SELECTION_LIFETIME,
    ENDPOINT_VALIDATION_BYTES as VALIDATION_BYTES,
    ENDPOINT_VALIDATION_CONCURRENCY as VALIDATION_CONCURRENCY,
    ENDPOINT_VALIDATION_TIMEOUT as VALIDATION_TIMEOUT,
};

pub(super) struct ProbeBudget {
    started_at: Instant,
    used: u64,
    limit: u64,
}

impl ProbeBudget {
    pub(super) fn new(now: Instant, limit: u64) -> Self {
        Self {
            started_at: now,
            used: 0,
            limit: limit.min(ROUND_BYTES),
        }
    }

    pub(super) fn remaining(&self, now: Instant) -> u64 {
        if now.saturating_duration_since(self.started_at) >= ROUND_TIMEOUT {
            0
        } else {
            self.limit.saturating_sub(self.used)
        }
    }

    /// Call under the round's short state lock before polling more payload.
    /// Wire overhead and bounded library preread are measured separately.
    pub(super) fn reserve(&mut self, requested: u64, now: Instant) -> u64 {
        let reserved = requested.min(self.remaining(now));
        self.used += reserved;
        reserved
    }

    pub(super) fn used(&self) -> u64 {
        self.used
    }
}

/// Two non-overlapping observations are needed to rank an endpoint's bulk
/// goodput. Very short complete objects do not prove sustainable bandwidth.
pub(super) fn rank_candidates(
    samples: &[ThroughputSample],
    protocol: Protocol,
    now: Instant,
) -> Vec<(IpAddr, u64)> {
    let mut grouped: HashMap<(IpAddr, bool, u64), Vec<&ThroughputSample>> =
        HashMap::new();
    let mut seen = HashSet::new();
    for sample in samples {
        if sample.protocol != protocol
            || sample.contaminated
            || sample.bytes < VALIDATION_BYTES
            || sample.elapsed.is_zero()
            || !sample.complete_object
                && sample.elapsed < ENDPOINT_MIN_SAMPLE_TIME
                && sample.bytes < SAMPLE_BYTES
            || sample.observed_at > now
            || now.saturating_duration_since(sample.observed_at)
                >= SELECTION_LIFETIME
            || !seen.insert((
                sample.ip,
                sample.connection_id,
                sample.observed_at,
            ))
        {
            continue;
        }
        grouped
            .entry((
                sample.ip,
                sample.complete_object,
                if sample.complete_object {
                    sample.bytes
                } else {
                    0
                },
            ))
            .or_default()
            .push(sample);
    }
    let mut ranked = Vec::new();
    for ((ip, _, _), mut samples) in grouped {
        samples.sort_by_key(|sample| sample.observed_at);
        let mut rates: Vec<u64> = Vec::new();
        let mut previous_end = None;
        for sample in samples {
            let Some(start) = sample.observed_at.checked_sub(sample.elapsed)
            else {
                continue;
            };
            if previous_end.is_some_and(|end| start < end) {
                continue;
            }
            previous_end = Some(sample.observed_at);
            let rate = sample.bytes as f64 / sample.elapsed.as_secs_f64();
            if rate.is_finite() && rate >= 1.0 {
                rates.push(rate.min(u64::MAX as f64) as u64);
            }
        }
        if rates.len() < ENDPOINT_MIN_SUCCESS_SAMPLES {
            continue;
        }
        rates.sort_unstable();
        let slowest = rates[0];
        // Large disagreement means the trial is uncertain rather than fast.
        if rates[rates.len() - 1]
            > slowest.saturating_mul(ENDPOINT_MAX_RATE_SPREAD)
        {
            continue;
        }
        ranked.push((ip, slowest));
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

pub(super) fn should_publish(
    current_speed: u64,
    candidate_speed: u64,
    remaining: u64,
) -> bool {
    native_slow::should_switch(
        current_speed,
        candidate_speed,
        remaining,
        remaining,
    )
}
