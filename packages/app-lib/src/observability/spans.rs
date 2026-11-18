//! Span retention for the in-process capture.
//!
//! The capture sink only sees events and metrics, because spans never reach the
//! contract as values. This layer records the name and duration of domain spans
//! so the diagnostics MCP can show which operations ran, without recording any
//! span field: `skip_all` instrumentation already keeps fields out, and the
//! layer never reads them.

use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tracing::Subscriber;
use tracing::span::{Attributes, Id};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Domain span targets bridged by OTLP today; capture watches the same set.
const CAPTURED_TARGETS: &[&str] = &["theseus", "theseus_gui"];

/// Spans kept by default.
pub const DEFAULT_SPAN_CAPACITY: usize = 256;
/// Hard ceiling for retained spans.
pub const MAX_SPAN_CAPACITY: usize = 2_048;

static SPANS: OnceLock<Mutex<VecDeque<SpanRecord>>> = OnceLock::new();
static TRACKING: AtomicBool = AtomicBool::new(false);

/// One closed domain span.
#[derive(Clone, Debug)]
pub struct SpanRecord {
    pub name: String,
    pub target: String,
    pub started_at_unix_ms: i64,
    pub duration_ms: f64,
}

fn buffer() -> &'static Mutex<VecDeque<SpanRecord>> {
    SPANS.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Begin retaining spans.
pub fn start() {
    TRACKING.store(true, Ordering::Release);
}

/// Stop retaining spans and drop what was kept.
pub fn stop() {
    TRACKING.store(false, Ordering::Release);
    clear();
}

/// Drop retained spans, keep tracking.
pub fn clear() {
    buffer().lock().clear();
}

/// Retained spans, newest first, at least `min_duration_ms` long.
pub fn recent(limit: usize, min_duration_ms: f64) -> Vec<SpanRecord> {
    buffer()
        .lock()
        .iter()
        .rev()
        .filter(|span| span.duration_ms >= min_duration_ms)
        .take(limit)
        .cloned()
        .collect()
}

/// Number of retained spans.
pub fn len() -> usize {
    buffer().lock().len()
}

fn capacity() -> usize {
    DEFAULT_SPAN_CAPACITY.min(MAX_SPAN_CAPACITY)
}

/// Layer that records closed domain spans into the capture buffer.
#[derive(Clone, Copy, Debug, Default)]
pub struct CaptureSpanLayer;

impl CaptureSpanLayer {
    /// Attach the layer to a subscriber.
    pub fn new() -> Self {
        Self
    }

    /// Whether `target` is a domain span this layer records.
    pub fn tracks(target: &str) -> bool {
        CAPTURED_TARGETS.iter().any(|captured| {
            target == *captured
                || target
                    .strip_prefix(captured)
                    .is_some_and(|rest| rest.starts_with("::"))
        })
    }

    fn push(record: SpanRecord) {
        if !TRACKING.load(Ordering::Acquire) {
            return;
        }
        let mut buffer = buffer().lock();
        if buffer.len() >= capacity() {
            buffer.pop_front();
        }
        buffer.push_back(record);
    }
}

struct SpanStart {
    started: Instant,
    started_at_unix_ms: i64,
}

impl<S> Layer<S> for CaptureSpanLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attributes: &Attributes<'_>,
        id: &Id,
        context: Context<'_, S>,
    ) {
        if !Self::tracks(attributes.metadata().target()) {
            return;
        }
        if let Some(span) = context.span(id) {
            span.extensions_mut().insert(SpanStart {
                started: Instant::now(),
                started_at_unix_ms: unix_millis(),
            });
        }
    }

    fn on_close(&self, id: Id, context: Context<'_, S>) {
        let Some(span) = context.span(&id) else {
            return;
        };
        let target = span.metadata().target();
        if !Self::tracks(target) {
            return;
        }
        let Some(start) = span
            .extensions()
            .get::<SpanStart>()
            .map(|start| (start.started, start.started_at_unix_ms))
        else {
            return;
        };
        Self::push(SpanRecord {
            name: span.name().to_owned(),
            target: target.to_owned(),
            started_at_unix_ms: start.1,
            duration_ms: start.0.elapsed().as_secs_f64() * 1_000.0,
        });
    }
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
    use std::sync::Mutex;
    use tracing_subscriber::prelude::*;

    /// Tracing and capture are process-wide, so tests that flip them must not
    /// run against each other in parallel.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[test]
    fn tracks_only_domain_targets() {
        assert!(CaptureSpanLayer::tracks("theseus"));
        assert!(CaptureSpanLayer::tracks("theseus::api::instance"));
        assert!(CaptureSpanLayer::tracks("theseus_gui"));
        assert!(!CaptureSpanLayer::tracks("theseus_gui_extra"));
        assert!(!CaptureSpanLayer::tracks("rmcp"));
    }

    #[test]
    fn records_closed_domain_spans() {
        let _guard = lock();
        clear();
        start();
        let subscriber =
            tracing_subscriber::registry().with(CaptureSpanLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                target: "theseus::api::instance",
                "instance_list"
            );
            drop(span);
            let ignored = tracing::info_span!(target: "rmcp", "ignored");
            drop(ignored);
        });

        let spans = recent(10, 0.0);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "instance_list");
        assert_eq!(spans[0].target, "theseus::api::instance");
        stop();
    }

    #[test]
    fn stops_when_stopped() {
        let _guard = lock();
        clear();
        stop();
        CaptureSpanLayer::push(SpanRecord {
            name: "ignored".into(),
            target: "theseus".into(),
            started_at_unix_ms: 0,
            duration_ms: 0.0,
        });
        assert_eq!(len(), 0);
    }
}
