use super::super::model::{Protocol, ThroughputSample};
use super::super::policy::*;
use std::time::{Duration, Instant};

fn sample(
    ip: &str,
    now: Instant,
    speed: u64,
    connection_id: u64,
) -> ThroughputSample {
    ThroughputSample {
        ip: ip.parse().unwrap(),
        protocol: Protocol::H2,
        connection_id,
        bytes: speed,
        elapsed: Duration::from_secs(1),
        observed_at: now,
        complete_object: false,
        contaminated: false,
    }
}

#[test]
fn round_budget_counts_shared_reservations_and_stops_at_deadline() {
    let now = Instant::now();
    let mut budget = ProbeBudget::new(now, u64::MAX);
    assert_eq!(budget.reserve(ROUND_BYTES - 10, now), ROUND_BYTES - 10);
    assert_eq!(budget.reserve(100, now), 10);
    assert_eq!(budget.reserve(1, now), 0);
    assert_eq!(budget.used(), ROUND_BYTES);
    let mut budget = ProbeBudget::new(now, ROUND_BYTES);
    assert_eq!(budget.remaining(now + ROUND_TIMEOUT), 0);
    assert_eq!(budget.reserve(100, now + ROUND_TIMEOUT), 0);
}

#[test]
fn throughput_ranking_uses_two_consistent_samples_without_latency_or_geo_bias()
{
    let now = Instant::now();
    let samples = [
        sample("151.101.1.242", now - Duration::from_secs(2), 100_000, 1),
        sample("151.101.1.242", now, 110_000, 1),
        sample("146.75.113.242", now - Duration::from_secs(2), 5_000_000, 2),
        sample("146.75.113.242", now, 6_000_000, 2),
    ];
    let ranked = rank_candidates(&samples, Protocol::H2, now);
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0], ("146.75.113.242".parse().unwrap(), 5_000_000));
    assert!(should_publish(100_000, ranked[0].1, 64 * 1024 * 1024));
    assert!(!should_publish(100_000, 124_999, 64 * 1024 * 1024));
    assert!(!should_publish(100_000, 5_000_000, 1024));
}

#[test]
fn duplicate_overlapping_tiny_contaminated_and_protocol_mismatch_samples_do_not_promote()
 {
    let now = Instant::now();
    let first =
        sample("151.101.1.242", now - Duration::from_secs(2), 1_000_000, 1);
    for case in 0..9 {
        let mut second = sample("151.101.1.242", now, 1_000_000, 1);
        match case {
            0 => {
                second = first.clone();
            }
            1 => {
                second.observed_at =
                    first.observed_at + Duration::from_millis(500);
            }
            2 => {
                second.elapsed = Duration::ZERO;
            }
            3 => {
                second.bytes = 1;
            }
            4 => {
                second.contaminated = true;
            }
            5 => {
                second.protocol = Protocol::Http1;
            }
            6 => {
                second.observed_at = now + Duration::from_secs(1);
            }
            7 => {
                second.bytes = 10_000_000;
            }
            8 => {
                second.complete_object = true;
            }
            _ => unreachable!(),
        }
        assert!(
            rank_candidates(&[first.clone(), second], Protocol::H2, now)
                .is_empty(),
            "case {case}"
        );
    }
}

#[test]
fn zero_speeds_unknown_remaining_and_expired_samples_cannot_publish() {
    assert!(!should_publish(0, 100_000, 10_000_000));
    assert!(!should_publish(100_000, 0, 10_000_000));
    assert!(!should_publish(100_000, 5_000_000, 0));
    let now = Instant::now();
    let samples = [
        sample("151.101.1.242", now - SELECTION_LIFETIME, 1_000_000, 1),
        sample(
            "151.101.1.242",
            now - SELECTION_LIFETIME - Duration::from_secs(2),
            1_000_000,
            1,
        ),
    ];
    assert!(rank_candidates(&samples, Protocol::H2, now).is_empty());
}

#[test]
fn repeated_small_object_completion_times_are_not_mixed_with_bulk_bandwidth() {
    let now = Instant::now();
    let mut first =
        sample("151.101.1.242", now - Duration::from_secs(2), 128 * 1024, 1);
    first.complete_object = true;
    first.elapsed = Duration::from_millis(100);
    let mut second = first.clone();
    second.observed_at = now;
    second.connection_id = 2;
    assert_eq!(
        rank_candidates(&[first.clone(), second.clone()], Protocol::H2, now)
            .len(),
        1
    );
    second.complete_object = false;
    assert!(rank_candidates(&[first, second], Protocol::H2, now).is_empty());
}
