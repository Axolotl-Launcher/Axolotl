//! A typed in-process bus for messages between subsystems.
//!
//! A channel is a named, bounded pair of publisher and subscriber handles.
//! Delivery is at-most-once: a full channel refuses the message and counts it,
//! and the publisher decides whether that loss is acceptable. This is the same
//! contract as [`crate::scheduler::l0::queue`], and it deliberately reuses that
//! module's [`Posted`] vocabulary rather than introducing a second one.
//!
//! What this is not: a broker. There is no persistence, no replay and no
//! cross-process delivery. A message that must survive a restart belongs in
//! the durable record (the install job row, the operation journal), with the
//! bus carrying only the wake-up. Keeping those two roles apart is what lets
//! the bus stay allocation-light and synchronous on the publish path.
//!
//! Telemetry is injected rather than reached through a global, so a test can
//! record into its own sink and assert delivery counts without touching
//! process-wide state.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use launcher_observability::attrs;
use launcher_observability::{Attribute, Telemetry};
use tokio::sync::mpsc::{self, error::TrySendError};

pub use crate::scheduler::l0::queue::{Delivery, Posted};

/// A message a channel carries. `Send + 'static` is all the bus needs: it
/// never serializes, so a message can borrow nothing and is moved, not copied.
pub trait Message: Send + 'static {}

impl<T: Send + 'static> Message for T {}

/// How a channel behaves when it is full.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Overflow {
    /// Refuse the new message and count it. The default for detection paths.
    DropNewest,
}

/// The shape of one channel, declared once at construction.
#[derive(Clone, Copy, Debug)]
pub struct ChannelSpec {
    /// Stable name used in telemetry. Must be `'static` so a counter label can
    /// borrow it without allocating.
    pub name: &'static str,
    /// Capacity of the lossy segment.
    pub capacity: usize,
    /// Capacity of the reserved segment, which a flood of lossy messages
    /// cannot crowd out.
    pub reserved: usize,
    pub overflow: Overflow,
}

impl ChannelSpec {
    /// A channel whose messages are all equally droppable.
    pub const fn lossy(name: &'static str, capacity: usize) -> Self {
        Self {
            name,
            capacity,
            reserved: 0,
            overflow: Overflow::DropNewest,
        }
    }

    /// A channel with a reserved segment for messages that must not be lost
    /// to a flood of droppable ones.
    pub const fn split(
        name: &'static str,
        capacity: usize,
        reserved: usize,
    ) -> Self {
        Self {
            name,
            capacity,
            reserved,
            overflow: Overflow::DropNewest,
        }
    }
}

/// The publishing half of a channel.
pub struct Publisher<M> {
    name: &'static str,
    lossy: mpsc::Sender<M>,
    reserved: Option<mpsc::Sender<M>>,
    dropped: Arc<AtomicU64>,
    overflowed: Arc<AtomicU64>,
    closed: Arc<AtomicU64>,
    telemetry: Arc<Telemetry>,
}

/// The receiving half of a channel.
pub struct Subscriber<M> {
    lossy: mpsc::Receiver<M>,
    reserved: Option<mpsc::Receiver<M>>,
}

impl<M: Message> Publisher<M> {
    /// Post without waiting. Returns what happened so the caller can decide
    /// whether a refusal matters (a dropped progress tick is carried into the
    /// next one; a dropped exit event is logged and reconciled at startup).
    pub fn try_publish(&self, message: M, delivery: Delivery) -> Posted {
        let sender = match delivery {
            Delivery::Lossy => &self.lossy,
            Delivery::Reserved => match &self.reserved {
                Some(reserved) => reserved,
                None => {
                    // A reserved post on a channel with no reserved segment
                    // is a declaration error, not a runtime condition.
                    debug_assert!(
                        false,
                        "channel {} declares no reserved segment",
                        self.name
                    );
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                    self.record(delivery, "dropped");
                    return Posted::Dropped;
                }
            },
        };
        let posted = match sender.try_send(message) {
            Ok(()) => Posted::Queued,
            Err(TrySendError::Closed(_)) => {
                self.closed.fetch_add(1, Ordering::Relaxed);
                Posted::Closed
            }
            Err(TrySendError::Full(_)) => match delivery {
                Delivery::Lossy => {
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                    Posted::Dropped
                }
                Delivery::Reserved => {
                    self.overflowed.fetch_add(1, Ordering::Relaxed);
                    Posted::Overflowed
                }
            },
        };
        self.record(delivery, posted.label());
        posted
    }

    /// Post, waiting for capacity. Only for work a user is waiting on: the
    /// detection path must use [`Self::try_publish`] so L0 never blocks.
    pub async fn publish(
        &self,
        message: M,
        delivery: Delivery,
    ) -> Result<(), Closed> {
        let sender = match delivery {
            Delivery::Lossy => &self.lossy,
            Delivery::Reserved => self.reserved.as_ref().ok_or(Closed)?,
        };
        match sender.send(message).await {
            Ok(()) => {
                self.record(delivery, "queued");
                Ok(())
            }
            Err(_) => {
                self.closed.fetch_add(1, Ordering::Relaxed);
                self.record(delivery, "closed");
                Err(Closed)
            }
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
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

    fn record(&self, delivery: Delivery, result: &'static str) {
        let segment = match delivery {
            Delivery::Lossy => "lossy",
            Delivery::Reserved => "reserved",
        };
        self.telemetry.counter("axolotl.sched.bus.delivery").add(
            1,
            &[
                Attribute::new("sched.channel", self.name),
                Attribute::new("sched.segment", segment),
                Attribute::new(attrs::OUTCOME, result),
            ],
        );
    }
}

/// The receiving side is gone; the publisher's work has nobody to reach.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Closed;

impl<M: Message> Subscriber<M> {
    /// The next message, reserved ones first. `None` once both segments are
    /// closed and empty.
    pub async fn recv(&mut self) -> Option<M> {
        match &mut self.reserved {
            Some(reserved) => {
                tokio::select! {
                    biased;
                    Some(message) = reserved.recv() => Some(message),
                    Some(message) = self.lossy.recv() => Some(message),
                    else => None,
                }
            }
            None => self.lossy.recv().await,
        }
    }

    /// Everything already queued in the lossy segment, without waiting.
    ///
    /// A consumer that has just received a reserved message calls this first:
    /// the lossy messages were posted earlier and must not be dropped with the
    /// receiver.
    pub fn drain_lossy(&mut self) -> Vec<M> {
        let mut messages = Vec::new();
        while let Ok(message) = self.lossy.try_recv() {
            messages.push(message);
        }
        messages
    }

    /// Whether both segments are empty and every publisher has been dropped.
    pub fn is_closed(&self) -> bool {
        self.lossy.is_closed()
            && self.reserved.as_ref().is_none_or(|r| r.is_closed())
    }
}

/// The bus: a factory for channels, plus the telemetry they share.
#[derive(Clone)]
pub struct Bus {
    telemetry: Arc<Telemetry>,
}

impl Bus {
    pub fn new(telemetry: Arc<Telemetry>) -> Self {
        Self { telemetry }
    }

    pub fn channel<M: Message>(
        &self,
        spec: ChannelSpec,
    ) -> (Publisher<M>, Subscriber<M>) {
        let (lossy, lossy_rx) = mpsc::channel(spec.capacity.max(1));
        let (reserved, reserved_rx) = if spec.reserved > 0 {
            let (tx, rx) = mpsc::channel(spec.reserved);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        (
            Publisher {
                name: spec.name,
                lossy,
                reserved,
                dropped: Arc::new(AtomicU64::new(0)),
                overflowed: Arc::new(AtomicU64::new(0)),
                closed: Arc::new(AtomicU64::new(0)),
                telemetry: Arc::clone(&self.telemetry),
            },
            Subscriber {
                lossy: lossy_rx,
                reserved: reserved_rx,
            },
        )
    }
}

impl Posted {
    /// The telemetry label for this outcome.
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Dropped => "dropped",
            Self::Overflowed => "overflowed",
            Self::Closed => "closed",
        }
    }
}

/// A short, bounded observation helper for callers that want to time a
/// delivery without holding a timer around unrelated work.
pub struct DeliveryTimer {
    started: Instant,
}

impl DeliveryTimer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

#[cfg(test)]
mod differential_tests;
#[cfg(test)]
mod tests;
