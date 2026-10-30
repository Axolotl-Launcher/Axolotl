//! Service order for queued units (T4.6, M5): pick the band by aging first,
//! then rotate between instances inside that band, so one busy instance
//! cannot keep another waiting. For L0–L2 the wait is bounded by
//! `queue_max_wait_upper_bound = QUEUE_MAX_DEPTH × T_MAX_SERVICE` (8 s with
//! the defaults, spec §11.6 N12). L3 has no such bound: it keeps queueing when
//! the queue is full instead of being refused (spec §11.2 item 23), and it
//! never ages. An L3 unit's service is long and not preemptible, so letting
//! aged L3 work overtake bounded units would void their bound; L3 is served
//! in its own band, after the bounded bands that have work.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::aging::{T_MAX_SERVICE, effective_band};
use super::tag::Level;

/// spec §11.6 N4.
pub const QUEUE_MAX_DEPTH: usize = 8;
/// spec §11.6 N6 (3 × the 20 s admission timeout).
pub const STALL_TIMEOUT: Duration = Duration::from_secs(60);

pub fn queue_max_wait_upper_bound() -> Duration {
    T_MAX_SERVICE * QUEUE_MAX_DEPTH as u32
}

/// The N12 startup check: `queue_max_depth * T_max_service <= stall_timeout`.
pub fn queue_bound_fits_stall_timeout() -> bool {
    queue_max_wait_upper_bound() <= STALL_TIMEOUT
}

#[derive(Debug, PartialEq, Eq)]
pub struct Unit<T> {
    pub instance: String,
    pub level: Level,
    pub enqueued: Instant,
    pub item: T,
}

/// The queue is full: a bounded refusal the caller may retry.
#[derive(Debug, PartialEq, Eq)]
pub struct QueueFull;

#[derive(Debug)]
pub struct FairQueue<T> {
    units: VecDeque<Unit<T>>,
    last_instance: Option<String>,
}

impl<T> Default for FairQueue<T> {
    fn default() -> Self {
        Self {
            units: VecDeque::new(),
            last_instance: None,
        }
    }
}

impl<T> FairQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Queues a unit. L0–L2 and L4 are refused once `QUEUE_MAX_DEPTH`
    /// bounded units wait; L3 is always queued.
    pub fn push(
        &mut self,
        instance: &str,
        level: Level,
        now: Instant,
        item: T,
    ) -> Result<(), QueueFull> {
        let bounded = self
            .units
            .iter()
            .filter(|unit| unit.level != Level::L3)
            .count();
        if level != Level::L3 && bounded >= QUEUE_MAX_DEPTH {
            return Err(QueueFull);
        }
        self.units.push_back(Unit {
            instance: instance.to_owned(),
            level,
            enqueued: now,
            item,
        });
        Ok(())
    }

    /// The next unit to serve: the most privileged effective band, then the
    /// instance after the one served last, then that instance's oldest unit.
    pub fn next(&mut self, now: Instant) -> Option<Unit<T>> {
        let band_of = |unit: &Unit<T>| match unit.level {
            Level::L3 => Level::L3 as u32,
            level => effective_band(
                level,
                now.saturating_duration_since(unit.enqueued),
            ),
        };
        let band = self.units.iter().map(band_of).min()?;
        let mut instances: Vec<&str> = self
            .units
            .iter()
            .filter(|unit| band_of(unit) == band)
            .map(|unit| unit.instance.as_str())
            .collect();
        instances.sort_unstable();
        instances.dedup();
        let instance = match self.last_instance.as_deref() {
            Some(last) => instances
                .iter()
                .find(|instance| **instance > last)
                .unwrap_or(&instances[0]),
            None => &instances[0],
        }
        .to_string();
        let index = self.units.iter().position(|unit| {
            unit.instance == instance && band_of(unit) == band
        })?;
        self.last_instance = Some(instance);
        self.units.remove(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_bound_fits_the_stall_timeout() {
        assert!(queue_bound_fits_stall_timeout());
        assert_eq!(queue_max_wait_upper_bound(), Duration::from_secs(8));
    }

    #[test]
    fn a_full_queue_refuses_bounded_levels_but_keeps_l3() {
        let mut queue = FairQueue::new();
        let now = Instant::now();
        for i in 0..QUEUE_MAX_DEPTH {
            queue.push("a", Level::L2, now, i).unwrap();
        }
        assert_eq!(queue.push("a", Level::L2, now, 99), Err(QueueFull));
        assert_eq!(queue.push("a", Level::L4, now, 99), Err(QueueFull));
        assert_eq!(queue.push("a", Level::L3, now, 99), Ok(()));
    }

    /// Two instances keep three units each queued; a third instance's one
    /// unit must be served within `queue_max_wait_upper_bound` (≤ 8 s).
    #[test]
    fn a_third_instance_is_served_within_the_bound_while_two_stay_busy() {
        let start = Instant::now();
        let mut queue = FairQueue::new();
        for _ in 0..3 {
            queue.push("a", Level::L2, start, "a").unwrap();
            queue.push("b", Level::L2, start, "b").unwrap();
        }
        queue
            .push("c", Level::L2, start + Duration::from_millis(1), "c")
            .unwrap();
        let mut now = start + Duration::from_millis(1);
        let served_at = loop {
            let unit = queue.next(now).expect("the queue never drains");
            if unit.item == "c" {
                break now;
            }
            queue
                .push(&unit.instance, Level::L2, now, unit.item)
                .unwrap();
            now += T_MAX_SERVICE;
            assert!(now - start <= Duration::from_secs(60), "c was starved");
        };
        let waited = served_at - (start + Duration::from_millis(1));
        assert!(
            waited <= queue_max_wait_upper_bound(),
            "c waited {waited:?}, bound {:?}",
            queue_max_wait_upper_bound()
        );
    }

    /// Band numbers grow as privilege falls: a steady stream of L1 requests
    /// must not starve an L2 unit, which aging lifts past them within the
    /// aging bound.
    #[test]
    fn a_high_band_is_not_starved_by_sustained_low_band_requests() {
        let start = Instant::now();
        let mut queue = FairQueue::new();
        queue.push("busy", Level::L1, start, "l1").unwrap();
        queue.push("other", Level::L2, start, "l2").unwrap();
        let mut now = start;
        let served_at = loop {
            let unit = queue.next(now).unwrap();
            if unit.item == "l2" {
                break now;
            }
            now += T_MAX_SERVICE;
            queue.push("busy", Level::L1, now, "l1").unwrap();
            assert!(now - start <= Duration::from_secs(60), "L2 was starved");
        };
        assert!(served_at - start <= super::super::aging::max_wait_bound());
    }

    /// Aged L3 work never overtakes a bounded unit, or the bound would not
    /// hold: L3 service is long and cannot be preempted.
    #[test]
    fn aged_l3_work_does_not_overtake_bounded_units() {
        let start = Instant::now();
        let mut queue = FairQueue::new();
        for i in 0..20 {
            queue.push("install", Level::L3, start, i).unwrap();
        }
        let now = start + Duration::from_secs(600);
        // An L4 unit that has aged to the top band, and a fresh L2 unit.
        queue
            .push("bulk", Level::L4, now - Duration::from_secs(1), 100)
            .unwrap();
        queue.push("view", Level::L2, now, 200).unwrap();
        let first: Vec<_> =
            (0..2).map(|_| queue.next(now).unwrap().item).collect();
        assert!(first.contains(&100) && first.contains(&200), "{first:?}");
        assert!(queue.next(now).is_some_and(|unit| unit.level == Level::L3));
    }

    /// The other reading of the same guarantee: the most privileged band is
    /// served first however many less privileged units wait.
    #[test]
    fn a_privileged_unit_is_not_held_behind_less_privileged_ones() {
        let now = Instant::now();
        let mut queue = FairQueue::new();
        for i in 0..7 {
            queue.push("bulk", Level::L4, now, i).unwrap();
        }
        queue.push("urgent", Level::L1, now, 100).unwrap();
        assert_eq!(queue.next(now).unwrap().item, 100);
    }
}
