//! Bounded in-process capture of the telemetry the launcher already records.
//!
//! OTLP export is one consumer of the contract; local diagnostics are another.
//! This capture keeps the most recent events and a small aggregate of every
//! metric in memory so a local client can ask what the launcher just did,
//! without a collector and without touching disk.
//!
//! Install it with [`start`] and remove it with [`stop`]. Capture is off unless
//! something asks for it, so a launcher that never opens the diagnostics MCP
//! pays one atomic load per recording call.

use launcher_observability::{
    Attribute, AttributeValue, Event, Severity, TelemetryError, TelemetrySink,
};
use parking_lot::Mutex;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

/// Events kept by default. Large enough for one debugging session, small
/// enough that the buffer never dominates the process footprint.
pub const DEFAULT_EVENT_CAPACITY: usize = 512;
/// Hard ceiling for the event buffer, whatever a caller asks for.
pub const MAX_EVENT_CAPACITY: usize = 4_096;
/// Distinct metric series kept before new labels are dropped.
const MAX_METRIC_SERIES: usize = 256;

static CAPTURE: OnceLock<Arc<Capture>> = OnceLock::new();
/// Whether the capture is currently retaining records.
static INSTALLED: AtomicBool = AtomicBool::new(false);
/// Whether the sink was appended to the telemetry registry. The registry has no
/// remove API, so this must stay true across a stop/start cycle; re-appending
/// would record every event twice.
static REGISTERED: AtomicBool = AtomicBool::new(false);

/// The capture installed by [`start`], if any.
pub fn active() -> Option<&'static Arc<Capture>> {
    if INSTALLED.load(Ordering::Acquire) {
        CAPTURE.get()
    } else {
        None
    }
}

/// Install the capture sink and make [`crate::observability::is_enabled`]
/// report telemetry as on. Calling this twice returns the same capture.
pub fn start(capacity: usize) -> Arc<Capture> {
    let capture = CAPTURE.get_or_init(|| Arc::new(Capture::new(capacity)));
    capture.recording.store(true, Ordering::Release);
    INSTALLED.store(true, Ordering::Release);
    if !REGISTERED.swap(true, Ordering::AcqRel) {
        crate::observability::telemetry().add_sink(capture.clone());
    }
    capture.clone()
}

/// Stop retaining data and detach the sink. The registry keeps the sink in its
/// list, so this flips a flag instead of removing another consumer's sink;
/// recording becomes a no-op and the buffers are dropped.
pub fn stop() {
    if !INSTALLED.swap(false, Ordering::AcqRel) {
        return;
    }
    if let Some(capture) = CAPTURE.get() {
        capture.stop_recording();
    }
}

/// One aggregate per metric name and attribute set.
#[derive(Clone, Debug)]
pub struct CounterSeries {
    pub name: String,
    pub value: u64,
    pub attributes: BTreeMap<String, AttributeValue>,
}

/// Aggregated histogram observations. The capture never keeps every sample.
#[derive(Clone, Debug)]
pub struct HistogramSeries {
    pub name: String,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub last: f64,
    pub sum: f64,
    pub attributes: BTreeMap<String, AttributeValue>,
}

/// Ordering key for a metric series. `AttributeValue` holds floats and has no
/// total order, so the key renders each value canonically instead.
type SeriesKey = (String, Vec<(String, String)>);

#[derive(Clone, Debug)]
struct CounterEntry {
    attributes: BTreeMap<String, AttributeValue>,
    value: u64,
}

#[derive(Default)]
struct Metrics {
    counters: BTreeMap<SeriesKey, CounterEntry>,
    histograms: BTreeMap<SeriesKey, HistogramEntry>,
}

#[derive(Clone, Debug, Default)]
struct HistogramEntry {
    attributes: BTreeMap<String, AttributeValue>,
    histogram: Histogram,
}

impl Metrics {
    fn counter_mut(
        &mut self,
        name: &str,
        attributes: &[Attribute],
    ) -> Option<&mut CounterEntry> {
        let key = series_key(name, attributes);
        if !self.counters.contains_key(&key)
            && self.counters.len() >= MAX_METRIC_SERIES
        {
            return None;
        }
        Some(self.counters.entry(key).or_insert_with(|| CounterEntry {
            attributes: attribute_map(attributes),
            value: 0,
        }))
    }

    fn histogram_mut(
        &mut self,
        name: &str,
        attributes: &[Attribute],
    ) -> Option<&mut HistogramEntry> {
        let key = series_key(name, attributes);
        if !self.histograms.contains_key(&key)
            && self.histograms.len() >= MAX_METRIC_SERIES
        {
            return None;
        }
        Some(
            self.histograms
                .entry(key)
                .or_insert_with(|| HistogramEntry {
                    attributes: attribute_map(attributes),
                    histogram: Histogram::default(),
                }),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Histogram {
    count: u64,
    min: f64,
    max: f64,
    last: f64,
    sum: f64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            last: 0.0,
            sum: 0.0,
        }
    }
}

impl Histogram {
    fn record(&mut self, value: f64) {
        self.count = self.count.saturating_add(1);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.last = value;
        self.sum += value;
    }
}

#[derive(Default)]
struct Inner {
    events: VecDeque<Event>,
    dropped_events: u64,
    metrics: Metrics,
}

/// The sink itself. Cloning is not supported on purpose: the registry holds
/// exactly one handle and every reader goes through [`active`].
pub struct Capture {
    capacity: usize,
    recording: AtomicBool,
    inner: Mutex<Inner>,
    installed_at_unix_ms: i64,
}

impl Capture {
    /// Create a capture that keeps at most `capacity` events.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, MAX_EVENT_CAPACITY),
            recording: AtomicBool::new(true),
            inner: Mutex::new(Inner::default()),
            installed_at_unix_ms: unix_millis(),
        }
    }

    fn stop_recording(&self) {
        self.recording.store(false, Ordering::Release);
        self.clear();
    }

    /// Drop everything captured so far. Recording continues.
    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.events.clear();
        inner.dropped_events = 0;
        inner.metrics = Metrics::default();
    }

    /// Events newest first, filtered by severity and event-name prefix.
    pub fn events(
        &self,
        limit: usize,
        severity: Option<Severity>,
        name_prefix: Option<&str>,
    ) -> Vec<Event> {
        let inner = self.inner.lock();
        inner
            .events
            .iter()
            .rev()
            .filter(|event| {
                severity.is_none_or(|severity| event.severity == severity)
            })
            .filter(|event| {
                name_prefix.is_none_or(|prefix| event.name.starts_with(prefix))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Counters sorted by name, then attributes.
    pub fn counters(&self) -> Vec<CounterSeries> {
        let inner = self.inner.lock();
        inner
            .metrics
            .counters
            .iter()
            .map(|((name, _), counter)| CounterSeries {
                name: name.clone(),
                value: counter.value,
                attributes: counter.attributes.clone(),
            })
            .collect()
    }

    /// Histograms sorted by name, then attributes.
    pub fn histograms(&self) -> Vec<HistogramSeries> {
        let inner = self.inner.lock();
        inner
            .metrics
            .histograms
            .iter()
            .map(|((name, _), entry)| HistogramSeries {
                name: name.clone(),
                count: entry.histogram.count,
                min: entry.histogram.min,
                max: entry.histogram.max,
                last: entry.histogram.last,
                sum: entry.histogram.sum,
                attributes: entry.attributes.clone(),
            })
            .collect()
    }

    /// Events dropped because the buffer was full.
    pub fn dropped_events(&self) -> u64 {
        self.inner.lock().dropped_events
    }

    /// When capture was installed, as Unix milliseconds.
    pub fn installed_at_unix_ms(&self) -> i64 {
        self.installed_at_unix_ms
    }

    /// Whether records are still retained.
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Acquire)
    }

    /// Capacity in events.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of retained events.
    pub fn len(&self) -> usize {
        self.inner.lock().events.len()
    }

    /// Whether nothing is retained.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TelemetrySink for Capture {
    fn record(&self, event: Event) {
        if !self.recording.load(Ordering::Acquire) {
            return;
        }
        let mut inner = self.inner.lock();
        if inner.events.len() >= self.capacity {
            inner.events.pop_front();
            inner.dropped_events = inner.dropped_events.saturating_add(1);
        }
        inner.events.push_back(event);
    }

    fn add_counter(
        &self,
        name: &'static str,
        value: u64,
        attributes: &[Attribute],
    ) {
        if !self.recording.load(Ordering::Acquire) {
            return;
        }
        let mut inner = self.inner.lock();
        if let Some(counter) = inner.metrics.counter_mut(name, attributes) {
            counter.value = counter.value.saturating_add(value);
        }
    }

    fn record_histogram(
        &self,
        name: &'static str,
        value: f64,
        attributes: &[Attribute],
    ) {
        if !self.recording.load(Ordering::Acquire) {
            return;
        }
        let mut inner = self.inner.lock();
        if let Some(entry) = inner.metrics.histogram_mut(name, attributes) {
            entry.histogram.record(value);
        }
    }

    fn flush(&self) -> Result<(), TelemetryError> {
        // Capture is already durable in memory; there is nothing to push out.
        Ok(())
    }
}

fn attribute_map(attributes: &[Attribute]) -> BTreeMap<String, AttributeValue> {
    attributes
        .iter()
        .map(|attribute| (attribute.key.clone(), attribute.value.clone()))
        .collect()
}

fn series_key(name: &str, attributes: &[Attribute]) -> SeriesKey {
    let rendered = attributes
        .iter()
        .map(|attribute| {
            let value = match &attribute.value {
                AttributeValue::String(value) => value.clone(),
                AttributeValue::I64(value) => value.to_string(),
                AttributeValue::F64(value) => value.to_string(),
                AttributeValue::Bool(value) => value.to_string(),
                _ => String::new(),
            };
            (attribute.key.clone(), value)
        })
        .collect();
    (name.to_owned(), rendered)
}

/// Number of capture sinks the registry holds, including the capture's own
/// flag. Used by tests to prove [`start`] and [`stop`] are paired.
#[cfg(test)]
pub(crate) fn reset_for_test() {
    INSTALLED.store(false, Ordering::Release);
    if let Some(capture) = CAPTURE.get() {
        capture.clear();
        capture.recording.store(true, Ordering::Release);
    }
}

/// Whether the sink has been appended to the telemetry registry. A second
/// registration would double every record, so tests assert on it.
#[cfg(test)]
pub(crate) fn is_registered() -> bool {
    REGISTERED.load(Ordering::Acquire)
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str) -> Event {
        Event {
            schema_version: launcher_observability::SCHEMA_VERSION,
            event_id: name.to_owned(),
            occurred_at_unix_ms: 0,
            name: name.to_owned(),
            severity: Severity::Info,
            duration_ms: None,
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn keeps_the_newest_events_within_capacity() {
        let capture = Capture::new(2);
        for name in ["a", "b", "c"] {
            capture.record(event(name));
        }
        let names: Vec<String> = capture
            .events(10, None, None)
            .into_iter()
            .map(|event| event.name)
            .collect();
        assert_eq!(names, ["c", "b"]);
        assert_eq!(capture.dropped_events(), 1);
    }

    #[test]
    fn filters_by_severity_and_name_prefix() {
        let capture = Capture::new(8);
        let mut warning = event("install.job");
        warning.severity = Severity::Warn;
        capture.record(event("app.started"));
        capture.record(warning);

        let warnings = capture.events(10, Some(Severity::Warn), None);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].name, "install.job");

        let install = capture.events(10, None, Some("install."));
        assert_eq!(install.len(), 1);
        assert_eq!(install[0].name, "install.job");
    }

    #[test]
    fn aggregates_metrics_instead_of_keeping_samples() {
        let capture = Capture::new(8);
        let attributes = [Attribute::new("command", "instance_list")];
        capture.add_counter("axolotl.ipc.duration", 2, &attributes);
        capture.add_counter("axolotl.ipc.duration", 3, &attributes);
        capture.record_histogram("axolotl.ipc.duration", 10.0, &attributes);
        capture.record_histogram("axolotl.ipc.duration", 4.0, &attributes);

        let counters = capture.counters();
        assert_eq!(counters.len(), 1);
        assert_eq!(counters[0].value, 5);

        let histograms = capture.histograms();
        assert_eq!(histograms.len(), 1);
        let histogram = &histograms[0];
        assert_eq!(histogram.count, 2);
        assert_eq!(histogram.min, 4.0);
        assert_eq!(histogram.max, 10.0);
        assert_eq!(histogram.last, 4.0);
        assert_eq!(histogram.sum, 14.0);
    }

    #[test]
    fn stops_recording_and_clears_when_stopped() {
        let capture = Capture::new(4);
        capture.record(event("app.started"));
        capture.stop_recording();
        assert!(capture.is_empty());
        capture.record(event("app.shutdown"));
        assert!(capture.is_empty());
    }

    #[test]
    fn clamps_capacity() {
        assert_eq!(Capture::new(0).capacity(), 1);
        assert_eq!(Capture::new(usize::MAX).capacity(), MAX_EVENT_CAPACITY);
    }
}
