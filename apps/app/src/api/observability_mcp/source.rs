//! Adapter from the launcher's in-process capture to the MCP's query surface.

use launcher_observability::{AttributeValue, Severity};
use launcher_observability_mcp::{
    CaptureStats, CounterSeries, EventRecord, HistogramSeries, Source,
    SpanRecord,
};
use serde_json::{Map, Value, json};

/// Reads the process-wide capture. Every call is cheap when capture is off.
pub struct LauncherCapture;

impl LauncherCapture {
    fn capture()
    -> Option<&'static std::sync::Arc<theseus::observability::capture::Capture>>
    {
        theseus::observability::capture::active()
    }
}

impl Source for LauncherCapture {
    fn stats(&self) -> CaptureStats {
        match Self::capture() {
            Some(capture) => CaptureStats {
                recording: capture.is_recording(),
                event_capacity: capture.capacity(),
                event_count: capture.len(),
                dropped_events: capture.dropped_events(),
                span_count: theseus::observability::spans::len(),
                installed_at_unix_ms: capture.installed_at_unix_ms(),
            },
            None => CaptureStats::default(),
        }
    }

    fn events(
        &self,
        limit: usize,
        severity: Option<&str>,
        name_prefix: Option<&str>,
    ) -> Vec<EventRecord> {
        let Some(capture) = Self::capture() else {
            return Vec::new();
        };
        capture
            .events(limit, severity.and_then(parse_severity), name_prefix)
            .into_iter()
            .map(|event| EventRecord {
                name: event.name,
                severity: event.severity.as_str().to_owned(),
                occurred_at_unix_ms: event.occurred_at_unix_ms,
                duration_ms: event.duration_ms,
                attributes: render_attributes(&event.attributes),
            })
            .collect()
    }

    fn counters(&self) -> Vec<CounterSeries> {
        let Some(capture) = Self::capture() else {
            return Vec::new();
        };
        capture
            .counters()
            .into_iter()
            .map(|counter| CounterSeries {
                name: counter.name,
                value: counter.value,
                attributes: render_attributes(&counter.attributes),
            })
            .collect()
    }

    fn histograms(&self) -> Vec<HistogramSeries> {
        let Some(capture) = Self::capture() else {
            return Vec::new();
        };
        capture
            .histograms()
            .into_iter()
            .map(|histogram| HistogramSeries {
                name: histogram.name,
                count: histogram.count,
                min: histogram.min,
                max: histogram.max,
                last: histogram.last,
                sum: histogram.sum,
                attributes: render_attributes(&histogram.attributes),
            })
            .collect()
    }

    fn spans(&self, limit: usize, min_duration_ms: f64) -> Vec<SpanRecord> {
        theseus::observability::spans::recent(limit, min_duration_ms)
            .into_iter()
            .map(|span| SpanRecord {
                name: span.name,
                target: span.target,
                started_at_unix_ms: span.started_at_unix_ms,
                duration_ms: span.duration_ms,
            })
            .collect()
    }

    fn flush(&self) -> bool {
        theseus::observability::flush(std::time::Duration::from_secs(3))
    }
}

fn parse_severity(severity: &str) -> Option<Severity> {
    match severity {
        "trace" => Some(Severity::Trace),
        "debug" => Some(Severity::Debug),
        "info" => Some(Severity::Info),
        "warn" => Some(Severity::Warn),
        "error" => Some(Severity::Error),
        _ => None,
    }
}

fn render_attributes(
    attributes: &std::collections::BTreeMap<String, AttributeValue>,
) -> Map<String, Value> {
    attributes
        .iter()
        .map(|(key, value)| {
            let value = match value {
                AttributeValue::String(value) => json!(value),
                AttributeValue::I64(value) => json!(value),
                AttributeValue::F64(value) => json!(value),
                AttributeValue::Bool(value) => json!(value),
                _ => Value::Null,
            };
            (key.clone(), value)
        })
        .collect()
}
