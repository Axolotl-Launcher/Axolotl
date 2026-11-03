//! L0's outbound queue. L0 may never wait on delivery, so it only posts with
//! `try_send` into bounded channels; there is no unbounded or `.send().await`
//! path. Events that may be lost go to the lossy segment; events that must not
//! be lost go to a separately bounded reserved segment, so a flood of lossy
//! events cannot crowd them out.
//!
//! Overflow cost (declared): when a lossy event finds its segment full it is
//! dropped and counted (a dropped playtime tick is carried into the next one);
//! a reserved event that finds its segment full or the receiver gone is
//! counted too. Losing a process-exit event leaves that instance shown as
//! running for the rest of the session; the record is in memory only, so a
//! restart does not carry it over.

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::{self, error::TrySendError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// May be dropped when the queue is full.
    Lossy,
    /// Must arrive; uses its own reserved capacity.
    Reserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posted {
    Queued,
    /// A lossy event found the queue full.
    Dropped,
    /// A reserved event found the reserved segment full too.
    Overflowed,
    /// The receiving side is gone.
    Closed,
}

pub struct L0Queue<T> {
    lossy: mpsc::Sender<T>,
    reserved: mpsc::Sender<T>,
    dropped: AtomicU64,
    overflowed: AtomicU64,
    closed: AtomicU64,
}

pub struct L0QueueReceiver<T> {
    lossy: mpsc::Receiver<T>,
    reserved: mpsc::Receiver<T>,
}

impl<T> L0Queue<T> {
    pub fn channel(
        lossy_capacity: usize,
        reserved_capacity: usize,
    ) -> (Self, L0QueueReceiver<T>) {
        let (lossy, lossy_rx) = mpsc::channel(lossy_capacity);
        let (reserved, reserved_rx) = mpsc::channel(reserved_capacity);
        (
            Self {
                lossy,
                reserved,
                dropped: AtomicU64::new(0),
                overflowed: AtomicU64::new(0),
                closed: AtomicU64::new(0),
            },
            L0QueueReceiver {
                lossy: lossy_rx,
                reserved: reserved_rx,
            },
        )
    }

    /// Never waits.
    pub fn post(&self, event: T, delivery: Delivery) -> Posted {
        let sender = match delivery {
            Delivery::Lossy => &self.lossy,
            Delivery::Reserved => &self.reserved,
        };
        match sender.try_send(event) {
            Ok(()) => Posted::Queued,
            Err(TrySendError::Closed(_)) => {
                self.closed.fetch_add(1, Ordering::Relaxed);
                if delivery == Delivery::Reserved {
                    tracing::warn!(
                        "L0 reserved queue closed; the reaction task is gone"
                    );
                }
                Posted::Closed
            }
            Err(TrySendError::Full(_)) => match delivery {
                Delivery::Lossy => {
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                    Posted::Dropped
                }
                Delivery::Reserved => {
                    self.overflowed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        "L0 reserved queue overflowed; the event is reconciled at the next start"
                    );
                    Posted::Overflowed
                }
            },
        }
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn overflowed(&self) -> u64 {
        self.overflowed.load(Ordering::Relaxed)
    }

    pub fn closed(&self) -> u64 {
        self.closed.load(Ordering::Relaxed)
    }
}

impl<T> L0QueueReceiver<T> {
    /// The next event, reserved ones first; `None` once both sides closed.
    pub async fn recv(&mut self) -> Option<T> {
        tokio::select! {
            biased;
            Some(event) = self.reserved.recv() => Some(event),
            Some(event) = self.lossy.recv() => Some(event),
            else => None,
        }
    }

    /// Everything already queued in the lossy segment, without waiting. A
    /// reserved event is served first, so the reaction task calls this when
    /// it receives one: the lossy events were posted earlier and must not be
    /// dropped with the receiver.
    pub fn drain_lossy(&mut self) -> Vec<T> {
        let mut events = Vec::new();
        while let Ok(event) = self.lossy.try_recv() {
            events.push(event);
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_full_lossy_segment_drops_and_counts() {
        let (queue, _rx) = L0Queue::channel(1, 1);
        assert_eq!(queue.post(1, Delivery::Lossy), Posted::Queued);
        assert_eq!(queue.post(2, Delivery::Lossy), Posted::Dropped);
        assert_eq!(queue.dropped(), 1);
    }

    #[tokio::test]
    async fn reserved_events_are_not_crowded_out_by_lossy_ones() {
        let (queue, mut rx) = L0Queue::channel(2, 1);
        for event in 0..5 {
            queue.post(event, Delivery::Lossy);
        }
        assert_eq!(queue.post(99, Delivery::Reserved), Posted::Queued);
        assert_eq!(rx.recv().await, Some(99), "reserved events come first");
    }

    #[tokio::test]
    async fn draining_lossy_after_a_reserved_event_keeps_queued_ticks() {
        let (queue, mut rx) = L0Queue::channel(8, 1);
        for event in 0..3 {
            queue.post(event, Delivery::Lossy);
        }
        queue.post(99, Delivery::Reserved);
        assert_eq!(rx.recv().await, Some(99), "the reserved event is first");
        assert_eq!(
            rx.drain_lossy(),
            vec![0, 1, 2],
            "the lossy events posted before it are not lost"
        );
    }

    #[tokio::test]
    async fn a_full_reserved_segment_overflows_and_counts() {
        let (queue, _rx) = L0Queue::channel(1, 1);
        assert_eq!(queue.post(1, Delivery::Reserved), Posted::Queued);
        assert_eq!(queue.post(2, Delivery::Reserved), Posted::Overflowed);
        assert_eq!(queue.overflowed(), 1);
    }

    #[tokio::test]
    async fn posting_never_waits_even_when_nobody_receives() {
        let (queue, rx) = L0Queue::channel(1, 1);
        drop(rx);
        assert_eq!(queue.post(1, Delivery::Reserved), Posted::Closed);
        assert_eq!(queue.post(2, Delivery::Lossy), Posted::Closed);
        assert_eq!(queue.closed(), 2, "a closed post is counted");
    }
}
