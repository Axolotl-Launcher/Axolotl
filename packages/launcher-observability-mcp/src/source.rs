//! What the MCP can read, independent of where the launcher keeps it.
//!
//! Every method returns already-bounded, already-redacted values: the capture
//! that fills them rejects credential-shaped keys and long strings before they
//! are stored, so nothing here needs to redact again.

use serde::Serialize;
use serde_json::{Map, Value};

/// One recorded telemetry event.
#[derive(Clone, Debug, Serialize)]
pub struct EventRecord {
    pub name: String,
    pub severity: String,
    pub occurred_at_unix_ms: i64,
    pub duration_ms: Option<u64>,
    pub attributes: Map<String, Value>,
}

/// One aggregated counter.
#[derive(Clone, Debug, Serialize)]
pub struct CounterSeries {
    pub name: String,
    pub value: u64,
    pub attributes: Map<String, Value>,
}

/// One aggregated histogram. Samples themselves are not kept.
#[derive(Clone, Debug, Serialize)]
pub struct HistogramSeries {
    pub name: String,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub last: f64,
    pub sum: f64,
    pub attributes: Map<String, Value>,
}

/// One closed domain span.
#[derive(Clone, Debug, Serialize)]
pub struct SpanRecord {
    pub name: String,
    pub target: String,
    pub started_at_unix_ms: i64,
    pub duration_ms: f64,
}

/// Live capture statistics.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CaptureStats {
    pub recording: bool,
    pub event_capacity: usize,
    pub event_count: usize,
    pub dropped_events: u64,
    pub span_count: usize,
    pub installed_at_unix_ms: i64,
}

/// The telemetry a launcher keeps for local diagnostics.
pub trait Source: Send + Sync + 'static {
    /// Whether capture is currently recording.
    fn stats(&self) -> CaptureStats;

    /// Events newest first.
    fn events(
        &self,
        limit: usize,
        severity: Option<&str>,
        name_prefix: Option<&str>,
    ) -> Vec<EventRecord>;

    /// Aggregated counters.
    fn counters(&self) -> Vec<CounterSeries>;

    /// Aggregated histograms.
    fn histograms(&self) -> Vec<HistogramSeries>;

    /// Closed spans newest first, at least `min_duration_ms` long.
    fn spans(&self, limit: usize, min_duration_ms: f64) -> Vec<SpanRecord>;

    /// Flush any consumer that buffers. Local capture has nothing to push, so
    /// this reports whether a flush was attempted, not whether data was sent.
    fn flush(&self) -> bool;
}
