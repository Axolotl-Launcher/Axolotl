//! The differential test the plan asks for: the bus must report the same
//! delivery outcomes as the L0 queue for the same sequence of posts.
//!
//! The bus is meant to replace ad-hoc channels without changing what
//! "dropped", "overflowed" and "closed" mean. Rather than assert that in
//! prose, this runs both implementations over one scripted sequence and
//! compares the outcomes element by element. If they ever diverge, the bus
//! has become a second delivery vocabulary and the migration would change
//! behaviour silently.

use std::sync::Arc;

use launcher_observability::{
    Attribute, Event, Telemetry, TelemetryError, TelemetrySink,
};

use crate::scheduler::bus::{Bus, ChannelSpec, Delivery, Posted, Subscriber};
use crate::scheduler::l0::queue::{L0Queue, L0QueueReceiver};

#[derive(Default)]
struct Silent;

impl TelemetrySink for Silent {
    fn record(&self, _event: Event) {}
    fn add_counter(&self, _: &'static str, _: u64, _: &[Attribute]) {}
    fn record_histogram(&self, _: &'static str, _: f64, _: &[Attribute]) {}
    fn flush(&self) -> Result<(), TelemetryError> {
        Ok(())
    }
}

fn isolated_bus() -> Bus {
    let telemetry = Telemetry::new();
    telemetry.install(Arc::new(Silent), "differential");
    Bus::new(Arc::new(telemetry))
}

/// One step of the shared script.
#[derive(Clone, Copy, Debug)]
enum Step {
    Lossy(u32),
    Reserved(u32),
    Recv,
    DrainLossy,
    DropSubscriber,
}

/// Run a script through the L0 queue, recording every observable outcome.
async fn run_l0(
    lossy: usize,
    reserved: usize,
    steps: &[Step],
) -> (Vec<Posted>, Vec<Option<u32>>, Vec<Vec<u32>>) {
    let (queue, receiver): (L0Queue<u32>, L0QueueReceiver<u32>) =
        L0Queue::channel(lossy, reserved);
    let mut receiver = Some(receiver);
    let mut posted = Vec::new();
    let mut received = Vec::new();
    let mut drained = Vec::new();
    for step in steps {
        match *step {
            Step::Lossy(value) => {
                posted.push(
                    queue.post(
                        value,
                        crate::scheduler::l0::queue::Delivery::Lossy,
                    ),
                );
            }
            Step::Reserved(value) => {
                posted.push(queue.post(
                    value,
                    crate::scheduler::l0::queue::Delivery::Reserved,
                ));
            }
            Step::Recv => {
                if let Some(receiver) = receiver.as_mut() {
                    received.push(receiver.recv().await);
                }
            }
            Step::DrainLossy => {
                if let Some(receiver) = receiver.as_mut() {
                    drained.push(receiver.drain_lossy());
                }
            }
            Step::DropSubscriber => {
                receiver = None;
            }
        }
    }
    (posted, received, drained)
}

/// The same script through the bus.
async fn run_bus(
    lossy: usize,
    reserved: usize,
    steps: &[Step],
) -> (Vec<Posted>, Vec<Option<u32>>, Vec<Vec<u32>>) {
    let bus = isolated_bus();
    let (publisher, subscriber): (
        crate::scheduler::bus::Publisher<u32>,
        Subscriber<u32>,
    ) = bus.channel(ChannelSpec::split("differential", lossy, reserved));
    let mut subscriber = Some(subscriber);
    let mut posted = Vec::new();
    let mut received = Vec::new();
    let mut drained = Vec::new();
    for step in steps {
        match *step {
            Step::Lossy(value) => {
                posted.push(publisher.try_publish(value, Delivery::Lossy));
            }
            Step::Reserved(value) => {
                posted.push(publisher.try_publish(value, Delivery::Reserved));
            }
            Step::Recv => {
                if let Some(subscriber) = subscriber.as_mut() {
                    received.push(subscriber.recv().await);
                }
            }
            Step::DrainLossy => {
                if let Some(subscriber) = subscriber.as_mut() {
                    drained.push(subscriber.drain_lossy());
                }
            }
            Step::DropSubscriber => {
                subscriber = None;
            }
        }
    }
    (posted, received, drained)
}

/// The script matrix: fill, overflow, drain, close.
const SCRIPTS: &[&[Step]] = &[
    // Fill the lossy segment, then overflow it.
    &[
        Step::Lossy(1),
        Step::Lossy(2),
        Step::Lossy(3),
        Step::Recv,
        Step::Recv,
    ],
    // A flood of lossy posts must not crowd out a reserved one. The script
    // ends on the drain: a further recv would wait, since both senders are
    // still alive and both segments are empty.
    &[
        Step::Lossy(1),
        Step::Lossy(2),
        Step::Lossy(3),
        Step::Reserved(99),
        Step::Recv,
        Step::DrainLossy,
    ],
    // Reserved overflow is its own outcome.
    &[
        Step::Reserved(1),
        Step::Reserved(2),
        Step::Reserved(3),
        Step::Recv,
    ],
    // A dropped subscriber closes the channel.
    &[
        Step::Lossy(1),
        Step::DropSubscriber,
        Step::Lossy(2),
        Step::Reserved(3),
    ],
    // Draining an empty lossy segment is a no-op, not an error.
    &[Step::DrainLossy, Step::Lossy(1), Step::DrainLossy],
];

fn label(posted: Posted) -> &'static str {
    posted.label()
}

#[tokio::test]
async fn the_bus_reports_the_same_outcomes_as_the_l0_queue() {
    // Capacities chosen so each script exercises both a full and an empty
    // segment; the scripts themselves stay independent of the numbers.
    let (lossy, reserved) = (2, 1);
    for (index, steps) in SCRIPTS.iter().enumerate() {
        let (l0_posted, l0_received, l0_drained) =
            run_l0(lossy, reserved, steps).await;
        let (bus_posted, bus_received, bus_drained) =
            run_bus(lossy, reserved, steps).await;

        let l0_labels: Vec<&str> =
            l0_posted.iter().copied().map(label).collect();
        let bus_labels: Vec<&str> =
            bus_posted.iter().copied().map(label).collect();
        assert_eq!(
            l0_labels, bus_labels,
            "script {index}: delivery outcomes diverged from the L0 queue"
        );
        assert_eq!(
            l0_received, bus_received,
            "script {index}: receive order diverged from the L0 queue"
        );
        assert_eq!(
            l0_drained, bus_drained,
            "script {index}: lossy drain diverged from the L0 queue"
        );
    }
}

/// The counters the bus reports for a run must agree with the outcomes it
/// returned, so a dashboard reading the counters sees the same story.
#[tokio::test]
async fn bus_counters_agree_with_the_returned_outcomes() {
    let bus = isolated_bus();
    let (publisher, subscriber) =
        bus.channel::<u32>(ChannelSpec::split("differential.counters", 2, 1));
    let mut outcomes = Vec::new();
    for value in 0..4 {
        outcomes.push(publisher.try_publish(value, Delivery::Lossy));
    }
    for value in 0..3 {
        outcomes.push(publisher.try_publish(value, Delivery::Reserved));
    }
    drop(subscriber);
    outcomes.push(publisher.try_publish(9, Delivery::Lossy));

    let count = |posted: Posted| {
        outcomes
            .iter()
            .copied()
            .filter(|outcome| *outcome == posted)
            .count() as u64
    };
    assert_eq!(publisher.dropped(), count(Posted::Dropped));
    assert_eq!(publisher.overflowed(), count(Posted::Overflowed));
    assert_eq!(publisher.closed(), count(Posted::Closed));
}

#[allow(dead_code)]
fn _the_silent_sink_is_shared_state_free() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Silent>();
}
