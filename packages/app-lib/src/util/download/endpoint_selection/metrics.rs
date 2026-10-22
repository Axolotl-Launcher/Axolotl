//! Per-file payload and non-overlapping accepted interval accounting.

use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Instant;

#[derive(Clone)]
pub(crate) struct EndpointTransferGuard {
    inner: Arc<TransferLifetime>,
}

struct TransferLifetime {
    selector: Weak<super::EndpointSelector>,
    lease: super::EndpointLease,
}

impl EndpointTransferGuard {
    pub(crate) fn new(
        selector: &Arc<super::EndpointSelector>,
        lease: super::EndpointLease,
    ) -> Self {
        selector.begin_feedback(&lease);
        Self {
            inner: Arc::new(TransferLifetime {
                selector: Arc::downgrade(selector),
                lease,
            }),
        }
    }

    pub(crate) fn record(
        &self,
        bytes: usize,
        pressure: super::super::local_resources::Pressure,
    ) {
        if let Some(selector) = self.inner.selector.upgrade() {
            let contaminated = pressure.blocks_expansion()
                || super::tuning::TRIALS.snapshot(Instant::now()).1.is_some();
            selector.observe_payload(
                &self.inner.lease,
                bytes as u64,
                Instant::now(),
                contaminated,
            );
        }
    }
}

impl Drop for TransferLifetime {
    fn drop(&mut self) {
        if let Some(selector) = self.selector.upgrade() {
            selector.end_feedback(&self.lease);
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct TransferCounters {
    #[cfg(test)]
    pub(crate) phases: BenchmarkPhases,
    received: AtomicU64,
    unique: AtomicU64,
    accepted: Mutex<BTreeMap<u64, u64>>,
}

#[cfg(test)]
#[derive(Default, Debug)]
pub(crate) struct BenchmarkPhases {
    receive_us: AtomicU64,
    output_us: AtomicU64,
    hash_us: AtomicU64,
}

#[cfg(test)]
#[derive(Debug, serde::Serialize)]
pub(crate) struct PhaseSnapshot {
    receive_us: u64,
    output_us: u64,
    hash_us: u64,
}

#[cfg(test)]
pub(crate) struct PhaseTimer<'a> {
    started: Instant,
    destination: &'a AtomicU64,
}

#[cfg(test)]
impl Drop for PhaseTimer<'_> {
    fn drop(&mut self) {
        self.destination.fetch_add(
            self.started.elapsed().as_micros() as u64,
            Ordering::Relaxed,
        );
    }
}

#[cfg(test)]
impl BenchmarkPhases {
    pub(crate) fn receive(&self) -> PhaseTimer<'_> {
        PhaseTimer {
            started: Instant::now(),
            destination: &self.receive_us,
        }
    }
    pub(crate) fn output(&self) -> PhaseTimer<'_> {
        PhaseTimer {
            started: Instant::now(),
            destination: &self.output_us,
        }
    }
    pub(crate) fn hash(&self) -> PhaseTimer<'_> {
        PhaseTimer {
            started: Instant::now(),
            destination: &self.hash_us,
        }
    }
    pub(crate) fn snapshot(&self) -> PhaseSnapshot {
        PhaseSnapshot {
            receive_us: self.receive_us.load(Ordering::Relaxed),
            output_us: self.output_us.load(Ordering::Relaxed),
            hash_us: self.hash_us.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CounterSnapshot {
    pub(crate) received_payload_bytes: u64,
    pub(crate) committed_unique_bytes: u64,
}

impl TransferCounters {
    pub(crate) fn received(&self, bytes: u64) {
        self.received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Count a successfully accepted half-open interval once, even if another
    /// attempt retransmits it. Probe counters live on the selector separately.
    pub(crate) fn commit(&self, start: u64, end: u64) {
        if end <= start {
            return;
        }
        let mut intervals = self.accepted.lock();
        let mut lower = start;
        let mut upper = end;
        let mut replaced = 0u64;
        if let Some((&a, &b)) = intervals.range(..=start).next_back()
            && b >= start
        {
            lower = a;
            upper = upper.max(b);
            replaced += b - a;
            intervals.remove(&a);
        }
        loop {
            let next = intervals.range(lower..).next().map(|(&a, &b)| (a, b));
            let Some((a, b)) = next.filter(|(a, _)| *a <= upper) else {
                break;
            };
            upper = upper.max(b);
            replaced += b - a;
            intervals.remove(&a);
        }
        intervals.insert(lower, upper);
        self.unique.fetch_add(
            (upper - lower).saturating_sub(replaced),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn snapshot(&self) -> CounterSnapshot {
        CounterSnapshot {
            received_payload_bytes: self.received.load(Ordering::Relaxed),
            committed_unique_bytes: self.unique.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_overlap_is_received_twice_but_committed_once() {
        let counters = TransferCounters::default();
        counters.received(100);
        counters.commit(0, 100);
        counters.received(150);
        counters.commit(50, 200);
        counters.commit(75, 80);
        assert_eq!(
            counters.snapshot(),
            CounterSnapshot {
                received_payload_bytes: 250,
                committed_unique_bytes: 200
            }
        );
        assert_eq!(counters.accepted.lock().len(), 1);
    }

    #[test]
    fn out_of_order_ranges_merge_without_counting_holes() {
        let counters = TransferCounters::default();
        for (start, end) in [(100, 200), (300, 400), (0, 50), (25, 350)] {
            counters.commit(start, end);
        }
        assert_eq!(counters.snapshot().committed_unique_bytes, 400);
        assert_eq!(counters.accepted.lock().len(), 1);
        assert_eq!(counters.snapshot().received_payload_bytes, 0);
    }
}
