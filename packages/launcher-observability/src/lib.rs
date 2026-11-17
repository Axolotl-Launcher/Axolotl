//! Stable, transport-free telemetry contract for Axolotl extensions.
//!
//! The launcher installs one or more [`TelemetrySink`] implementations at
//! runtime. When no sink is installed every operation is a cheap no-op, so
//! extension code can instrument itself without knowing whether the user has
//! enabled OTLP export.

#![forbid(unsafe_code)]
#![warn(unused_import_braces)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use thiserror::Error;
use tracing::Span;

pub mod attrs;
pub mod events;
pub mod redact;

/// Version of the event envelope and attribute vocabulary.
pub const SCHEMA_VERSION: u32 = 1;

/// Maximum number of attributes accepted on one event, counter or histogram.
pub const MAX_ATTRIBUTES: usize = 32;

/// Severity of a structured telemetry event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Severity {
    /// Stable string used by the OTLP log bridge.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// A bounded telemetry attribute value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum AttributeValue {
    String(String),
    I64(i64),
    F64(f64),
    Bool(bool),
}

impl From<&str> for AttributeValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for AttributeValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for AttributeValue {
    fn from(value: u64) -> Self {
        Self::I64(value.min(i64::MAX as u64) as i64)
    }
}

impl From<i32> for AttributeValue {
    fn from(value: i32) -> Self {
        Self::I64(i64::from(value))
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// One key/value pair attached to an event, counter or histogram.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attribute {
    pub key: String,
    pub value: AttributeValue,
}

impl Attribute {
    /// Create an attribute. Validation happens when the event is emitted or
    /// the metric is recorded.
    pub fn new(
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Serialized telemetry event. The launcher maps this to an OTLP log record;
/// future local analysis sinks can consume the same envelope.
#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub schema_version: u32,
    pub event_id: String,
    pub occurred_at_unix_ms: i64,
    pub name: String,
    pub severity: Severity,
    pub duration_ms: Option<u64>,
    pub attributes: BTreeMap<String, AttributeValue>,
}

/// A sink receives already-bounded, already-redacted telemetry.
///
/// Implementations must not block the caller. A sink that needs I/O should
/// enqueue into its own bounded buffer and return immediately.
pub trait TelemetrySink: Send + Sync + 'static {
    fn record(&self, event: Event);
    fn add_counter(
        &self,
        name: &'static str,
        value: u64,
        attributes: &[Attribute],
    );
    fn record_histogram(
        &self,
        name: &'static str,
        value: f64,
        attributes: &[Attribute],
    );
    fn flush(&self) -> Result<(), TelemetryError>;
}

/// Errors returned by the contract; they never contain user data.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TelemetryError {
    #[error("telemetry is not installed")]
    Unavailable,
    #[error("invalid telemetry event: {0}")]
    InvalidEvent(String),
    #[error("invalid telemetry metric: {0}")]
    InvalidMetric(String),
    #[error("invalid telemetry attribute: {0}")]
    InvalidAttribute(String),
    #[error("telemetry sink failed: {0}")]
    Sink(String),
}

/// Builder returned by [`Telemetry::event`].
pub struct EventBuilder<'a> {
    telemetry: &'a Telemetry,
    event: Event,
    invalid: Option<TelemetryError>,
}

impl EventBuilder<'_> {
    /// Attach an attribute. More than [`MAX_ATTRIBUTES`] attributes makes
    /// [`Self::emit`] fail instead of silently dropping data.
    #[must_use]
    pub fn attr(
        mut self,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Self {
        let key = key.into();
        if self.event.attributes.len() >= MAX_ATTRIBUTES {
            self.invalid = Some(TelemetryError::InvalidAttribute(format!(
                "more than {MAX_ATTRIBUTES} attributes"
            )));
            return self;
        }
        self.event.attributes.insert(key, value.into());
        self
    }

    /// Attach a duration measured by the caller.
    #[must_use]
    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.event.duration_ms = Some(duration_ms);
        self
    }

    /// Override the default `Info` severity.
    #[must_use]
    pub fn severity(mut self, severity: Severity) -> Self {
        self.event.severity = severity;
        self
    }

    /// Validate and send the event. It is a no-op when no sink is installed.
    pub fn emit(self) -> Result<(), TelemetryError> {
        if let Some(error) = self.invalid {
            return Err(error);
        }
        let mut event = self.event;
        redact::sanitize_event(&mut event)?;
        self.telemetry.record(event);
        Ok(())
    }
}

/// A counter facade. It is cheap to create and no-ops when telemetry is off.
pub struct Counter<'a> {
    telemetry: &'a Telemetry,
    name: &'static str,
}

impl Counter<'_> {
    /// Increment the counter. Invalid names or attributes are ignored so a
    /// bug in extension instrumentation cannot affect the host application.
    pub fn add(&self, value: u64, attributes: &[Attribute]) {
        if !self.telemetry.is_enabled() {
            return;
        }
        if redact::validate_metric_name(self.name).is_err() {
            return;
        }
        let Ok(attributes) = redact::sanitize_attributes(attributes) else {
            return;
        };
        self.telemetry.for_each_sink(|sink| {
            sink.add_counter(self.name, value, &attributes);
        });
    }
}

/// A histogram facade.
pub struct Histogram<'a> {
    telemetry: &'a Telemetry,
    name: &'static str,
}

impl Histogram<'_> {
    /// Record one observation. Non-finite values are ignored.
    pub fn record(&self, value: f64, attributes: &[Attribute]) {
        if !self.telemetry.is_enabled() || !value.is_finite() {
            return;
        }
        if redact::validate_metric_name(self.name).is_err() {
            return;
        }
        let Ok(attributes) = redact::sanitize_attributes(attributes) else {
            return;
        };
        self.telemetry.for_each_sink(|sink| {
            sink.record_histogram(self.name, value, &attributes);
        });
    }
}

/// Process-wide telemetry registry.
///
/// The launcher installs the OTLP sink; tests and future local analysis
/// features can use [`Telemetry::new`] for an isolated registry.
pub struct Telemetry {
    sinks: RwLock<Vec<Arc<dyn TelemetrySink>>>,
    session_id: OnceLock<String>,
    enabled: AtomicBool,
}

impl Telemetry {
    /// Create an isolated registry, primarily for tests and embedded users.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sinks: RwLock::new(Vec::new()),
            session_id: OnceLock::new(),
            enabled: AtomicBool::new(false),
        }
    }

    /// The process-wide registry used by the launcher and extensions.
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL: Telemetry = Telemetry::new();
        &GLOBAL
    }

    /// Install a sink and session identifier. The first installed sink turns
    /// the registry on; later sinks are appended.
    pub fn install(
        &self,
        sink: Arc<dyn TelemetrySink>,
        session_id: impl Into<String>,
    ) {
        let _ = self.session_id.set(session_id.into());
        self.add_sink(sink);
    }

    /// Append a sink. This is the extension point for a future user-configured
    /// local analysis feature.
    pub fn add_sink(&self, sink: Arc<dyn TelemetrySink>) {
        self.write_sinks().push(sink);
        self.enabled.store(true, Ordering::Release);
    }

    /// Remove all sinks and disable the registry. Intended for shutdown and
    /// tests; installing a new sink re-enables it.
    pub fn clear_sinks(&self) {
        self.write_sinks().clear();
        self.enabled.store(false, Ordering::Release);
    }

    /// Whether at least one sink is installed.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Per-process random identifier supplied by the host. Never persisted and
    /// never an installation/account identifier.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.get().map(String::as_str)
    }

    /// Start an extension span. The stable name is `extension.span`; the
    /// caller's logical name is carried in the `extension.name` attribute.
    #[must_use]
    pub fn span(&self, name: &'static str) -> Span {
        tracing::info_span!("extension.span", "extension.name" = name)
    }

    /// Build a structured event.
    #[must_use]
    pub fn event(&self, name: &'static str) -> EventBuilder<'_> {
        EventBuilder {
            telemetry: self,
            event: Event {
                schema_version: SCHEMA_VERSION,
                event_id: uuid::Uuid::new_v4().to_string(),
                occurred_at_unix_ms: unix_millis(),
                name: name.to_owned(),
                severity: Severity::Info,
                duration_ms: None,
                attributes: BTreeMap::new(),
            },
            invalid: None,
        }
    }

    /// Create a counter facade.
    #[must_use]
    pub fn counter(&self, name: &'static str) -> Counter<'_> {
        Counter {
            telemetry: self,
            name,
        }
    }

    /// Create a histogram facade.
    #[must_use]
    pub fn histogram(&self, name: &'static str) -> Histogram<'_> {
        Histogram {
            telemetry: self,
            name,
        }
    }

    /// Flush every sink. The first failure is returned after all sinks have
    /// been given a chance to flush.
    pub fn flush(&self) -> Result<(), TelemetryError> {
        let mut first_error = None;
        self.for_each_sink(|sink| {
            if let Err(error) = sink.flush()
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        });
        first_error.map_or(Ok(()), Err)
    }

    fn record(&self, event: Event) {
        if !self.is_enabled() {
            return;
        }
        self.for_each_sink(|sink| sink.record(event.clone()));
    }

    fn for_each_sink(&self, mut visit: impl FnMut(&Arc<dyn TelemetrySink>)) {
        let sinks = self.read_sinks().clone();
        for sink in &sinks {
            visit(sink);
        }
    }

    fn read_sinks(&self) -> RwLockReadGuard<'_, Vec<Arc<dyn TelemetrySink>>> {
        self.sinks.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write_sinks(&self) -> RwLockWriteGuard<'_, Vec<Arc<dyn TelemetrySink>>> {
        self.sinks.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Telemetry")
            .field("enabled", &self.is_enabled())
            .field("session_id", &self.session_id())
            .finish_non_exhaustive()
    }
}

fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<Event>>,
        counters: Mutex<Vec<(&'static str, u64)>>,
        histograms: Mutex<Vec<(&'static str, f64)>>,
        flushes: Mutex<usize>,
    }

    impl TelemetrySink for RecordingSink {
        fn record(&self, event: Event) {
            self.events.lock().expect("events").push(event);
        }

        fn add_counter(
            &self,
            name: &'static str,
            value: u64,
            _attributes: &[Attribute],
        ) {
            self.counters.lock().expect("counters").push((name, value));
        }

        fn record_histogram(
            &self,
            name: &'static str,
            value: f64,
            _attributes: &[Attribute],
        ) {
            self.histograms
                .lock()
                .expect("histograms")
                .push((name, value));
        }

        fn flush(&self) -> Result<(), TelemetryError> {
            *self.flushes.lock().expect("flushes") += 1;
            Ok(())
        }
    }

    #[test]
    fn a_registry_without_a_sink_is_a_noop() {
        let telemetry = Telemetry::new();
        assert!(!telemetry.is_enabled());
        assert!(
            telemetry
                .event("ext.test.noop")
                .attr(attrs::OUTCOME, "ok")
                .emit()
                .is_ok()
        );
        telemetry.counter("ext.test.counter").add(1, &[]);
        telemetry.histogram("ext.test.histogram").record(1.0, &[]);
        assert!(telemetry.flush().is_ok());
    }

    #[test]
    fn events_are_delivered_to_every_installed_sink() {
        let telemetry = Telemetry::new();
        let first = Arc::new(RecordingSink::default());
        let second = Arc::new(RecordingSink::default());
        telemetry.install(first.clone(), "session-1");
        telemetry.add_sink(second.clone());

        telemetry
            .event(events::EXTENSION_STARTED)
            .attr(attrs::OUTCOME, "ok")
            .duration_ms(7)
            .emit()
            .expect("event");

        for sink in [&first, &second] {
            let events = sink.events.lock().expect("events");
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].name, events::EXTENSION_STARTED);
            assert_eq!(events[0].duration_ms, Some(7));
            assert_eq!(
                events[0].attributes.get(attrs::OUTCOME),
                Some(&AttributeValue::String("ok".to_owned()))
            );
        }
        assert_eq!(telemetry.session_id(), Some("session-1"));
    }

    #[test]
    fn counters_and_histograms_are_delivered() {
        let telemetry = Telemetry::new();
        let sink = Arc::new(RecordingSink::default());
        telemetry.install(sink.clone(), "session-1");

        telemetry
            .counter("ext.test.items")
            .add(3, &[Attribute::new(attrs::OUTCOME, "ok")]);
        telemetry
            .histogram("ext.test.duration")
            .record(12.5, &[Attribute::new(attrs::OUTCOME, "ok")]);

        assert_eq!(
            *sink.counters.lock().expect("counters"),
            vec![("ext.test.items", 3)]
        );
        assert_eq!(
            *sink.histograms.lock().expect("histograms"),
            vec![("ext.test.duration", 12.5)]
        );
        telemetry.flush().expect("flush");
        assert_eq!(*sink.flushes.lock().expect("flushes"), 1);
    }

    #[test]
    fn sensitive_attribute_keys_are_rejected() {
        let telemetry = Telemetry::new();
        telemetry.install(Arc::new(RecordingSink::default()), "session-1");

        let error = telemetry
            .event(events::EXTENSION_STARTED)
            .attr("access_token", "secret")
            .emit()
            .expect_err("sensitive key must fail");
        assert!(matches!(error, TelemetryError::InvalidAttribute(_)));
    }

    #[test]
    fn strings_are_bounded_and_control_characters_are_removed() {
        let telemetry = Telemetry::new();
        let sink = Arc::new(RecordingSink::default());
        telemetry.install(sink.clone(), "session-1");

        let noisy = format!("{}\n\t", "x".repeat(400));
        telemetry
            .event(events::EXTENSION_STARTED)
            .attr(attrs::OUTCOME, noisy)
            .emit()
            .expect("event");
        let events = sink.events.lock().expect("events");
        let value = match events[0].attributes.get(attrs::OUTCOME) {
            Some(AttributeValue::String(value)) => value,
            other => panic!("expected a string attribute, got {other:?}"),
        };
        assert!(value.len() <= redact::MAX_STRING_LEN);
        assert!(!value.contains('\n'));
        assert!(!value.contains('\t'));
    }

    #[test]
    fn event_names_are_validated() {
        let telemetry = Telemetry::new();
        telemetry.install(Arc::new(RecordingSink::default()), "session-1");
        let error = telemetry
            .event("not a valid name")
            .emit()
            .expect_err("invalid name must fail");
        assert!(matches!(error, TelemetryError::InvalidEvent(_)));
    }

    #[test]
    fn clearing_sinks_disables_the_registry() {
        let telemetry = Telemetry::new();
        telemetry.install(Arc::new(RecordingSink::default()), "session-1");
        assert!(telemetry.is_enabled());
        telemetry.clear_sinks();
        assert!(!telemetry.is_enabled());
    }
}
