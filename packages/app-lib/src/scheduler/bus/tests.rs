use std::sync::{Arc, Mutex};

use launcher_observability::{
    Attribute, Event, Telemetry, TelemetryError, TelemetrySink,
};

use super::*;

#[derive(Default)]
struct Counters {
    entries: Mutex<Vec<(&'static str, u64, Vec<Attribute>)>>,
}

impl TelemetrySink for Counters {
    fn record(&self, _event: Event) {}

    fn add_counter(
        &self,
        name: &'static str,
        value: u64,
        attributes: &[Attribute],
    ) {
        self.entries.lock().expect("counters").push((
            name,
            value,
            attributes.to_vec(),
        ));
    }

    fn record_histogram(&self, _: &'static str, _: f64, _: &[Attribute]) {}

    fn flush(&self) -> Result<(), TelemetryError> {
        Ok(())
    }
}

fn bus() -> (Bus, Arc<Counters>) {
    let telemetry = Telemetry::new();
    let sink = Arc::new(Counters::default());
    telemetry.install(sink.clone(), "test");
    (Bus::new(Arc::new(telemetry)), sink)
}

fn depth(sink: &Counters) -> usize {
    sink.entries.lock().expect("counters").len()
}

#[tokio::test]
async fn a_message_posted_on_an_open_channel_is_delivered() {
    let (bus, _sink) = bus();
    let (publisher, mut subscriber) =
        bus.channel::<u32>(ChannelSpec::lossy("test.lossy", 4));
    assert_eq!(publisher.try_publish(7, Delivery::Lossy), Posted::Queued);
    assert_eq!(subscriber.recv().await, Some(7));
}

#[tokio::test]
async fn a_full_lossy_channel_refuses_and_counts() {
    let (bus, sink) = bus();
    let (publisher, _subscriber) =
        bus.channel::<u32>(ChannelSpec::lossy("test.full", 1));
    assert_eq!(publisher.try_publish(1, Delivery::Lossy), Posted::Queued);
    assert_eq!(publisher.try_publish(2, Delivery::Lossy), Posted::Dropped);
    assert_eq!(publisher.dropped(), 1);
    // One queued, one dropped: both were recorded.
    assert_eq!(depth(&sink), 2);
}

#[tokio::test]
async fn the_reserved_segment_is_not_crowded_out_by_lossy_traffic() {
    let (bus, _sink) = bus();
    let (publisher, mut subscriber) =
        bus.channel::<&'static str>(ChannelSpec::split("test.split", 1, 1));
    assert_eq!(
        publisher.try_publish("tick-1", Delivery::Lossy),
        Posted::Queued
    );
    assert_eq!(
        publisher.try_publish("tick-2", Delivery::Lossy),
        Posted::Dropped
    );
    // The lossy segment is full, but the reserved one still accepts.
    assert_eq!(
        publisher.try_publish("exit", Delivery::Reserved),
        Posted::Queued
    );
    // Reserved is served first, then the queue drains the older lossy tick.
    assert_eq!(subscriber.recv().await, Some("exit"));
    assert_eq!(subscriber.drain_lossy(), vec!["tick-1"]);
    assert_eq!(publisher.overflowed(), 0);
}

#[tokio::test]
async fn a_reserved_overflow_is_counted_separately_from_a_lossy_drop() {
    let (bus, _sink) = bus();
    let (publisher, _subscriber) =
        bus.channel::<u32>(ChannelSpec::split("test.reserved-full", 1, 1));
    assert_eq!(publisher.try_publish(1, Delivery::Reserved), Posted::Queued);
    assert_eq!(
        publisher.try_publish(2, Delivery::Reserved),
        Posted::Overflowed
    );
    assert_eq!(publisher.overflowed(), 1);
    assert_eq!(publisher.dropped(), 0);
}

#[tokio::test]
async fn a_dropped_subscriber_is_reported_as_closed() {
    let (bus, _sink) = bus();
    let (publisher, subscriber) =
        bus.channel::<u32>(ChannelSpec::lossy("test.closed", 1));
    drop(subscriber);
    assert_eq!(publisher.try_publish(1, Delivery::Lossy), Posted::Closed);
    assert_eq!(publisher.closed(), 1);
}

#[tokio::test]
async fn waiting_publish_is_bounded_by_the_subscriber() {
    let (bus, _sink) = bus();
    let (publisher, mut subscriber) =
        bus.channel::<u32>(ChannelSpec::lossy("test.await", 1));
    assert_eq!(publisher.try_publish(1, Delivery::Lossy), Posted::Queued);
    let waiting =
        tokio::spawn(
            async move { publisher.publish(2, Delivery::Lossy).await },
        );
    // The sender is waiting for capacity; taking the first message frees it.
    assert_eq!(subscriber.recv().await, Some(1));
    assert_eq!(waiting.await.expect("join"), Ok(()));
    assert_eq!(subscriber.recv().await, Some(2));
}

#[tokio::test]
async fn delivery_outcomes_are_recorded_with_their_channel_and_segment() {
    let (bus, sink) = bus();
    let (publisher, subscriber) =
        bus.channel::<u32>(ChannelSpec::lossy("test.labels", 1));
    publisher.try_publish(1, Delivery::Lossy);
    publisher.try_publish(2, Delivery::Lossy);
    drop(subscriber);
    publisher.try_publish(3, Delivery::Lossy);

    let entries = sink.entries.lock().expect("counters");
    assert!(
        entries
            .iter()
            .all(|(name, _, _)| *name == "axolotl.sched.bus.delivery")
    );
    let outcomes: Vec<String> = entries
        .iter()
        .map(|(_, _, attributes)| {
            attributes
                .iter()
                .find(|attribute| attribute.key == "outcome")
                .map(|attribute| format!("{:?}", attribute.value))
                .unwrap_or_default()
        })
        .collect();
    assert!(outcomes.iter().any(|outcome| outcome.contains("queued")));
    assert!(outcomes.iter().any(|outcome| outcome.contains("dropped")));
    assert!(outcomes.iter().any(|outcome| outcome.contains("closed")));
    let channels: Vec<&str> = entries
        .iter()
        .flat_map(|(_, _, attributes)| attributes.iter())
        .filter(|attribute| attribute.key == "sched.channel")
        .map(|attribute| attribute.key.as_str())
        .collect();
    assert!(!channels.is_empty());
}
