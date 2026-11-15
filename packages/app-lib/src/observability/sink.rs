//! Bridge from the transport-free telemetry contract to OpenTelemetry.
//!
//! The sink never performs I/O on the caller's thread: log records go into the
//! SDK batch processor and metric instruments are recorded synchronously. When
//! the metrics signal is available instruments are cached so the hot path only
//! takes a short mutex.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use launcher_observability::{
    Attribute, AttributeValue, Event, Severity, TelemetryError, TelemetrySink,
};
use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _};
use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::trace::{Span as _, Tracer as _, TracerProvider as _};
use opentelemetry::{Context, KeyValue, StringValue, Value};
use opentelemetry_sdk::logs::SdkLogger;
use opentelemetry_sdk::trace::SdkTracer;
use parking_lot::Mutex;

/// Target attached to every exported structured event.
const EVENT_TARGET: &str = "axolotl.observability";

pub(crate) struct OtlpSink {
    logger: Option<SdkLogger>,
    tracer: SdkTracer,
    meter: Meter,
    counters: Mutex<HashMap<&'static str, Counter<u64>>>,
    histograms: Mutex<HashMap<&'static str, Histogram<f64>>>,
}

impl OtlpSink {
    pub(crate) fn new(
        logger: Option<SdkLogger>,
        tracer: SdkTracer,
        meter: Meter,
    ) -> Arc<Self> {
        Arc::new(Self {
            logger,
            tracer,
            meter,
            counters: Mutex::new(HashMap::new()),
            histograms: Mutex::new(HashMap::new()),
        })
    }

    fn record_as_span(&self, event: Event) {
        let mut attributes = Vec::with_capacity(event.attributes.len() + 2);
        attributes.push(KeyValue::new("event.id", event.event_id.clone()));
        attributes
            .push(KeyValue::new("event.severity", event.severity.as_str()));
        for (key, value) in event.attributes {
            attributes.push(KeyValue::new(key, otel_value(value)));
        }

        let mut builder = self
            .tracer
            .span_builder(event.name)
            .with_attributes(attributes);
        if let Some(duration_ms) = event.duration_ms {
            let end = UNIX_EPOCH
                + Duration::from_millis(event.occurred_at_unix_ms.max(0) as u64);
            let start = end
                .checked_sub(Duration::from_millis(duration_ms))
                .unwrap_or(end);
            builder = builder.with_start_time(start);
        }
        let mut span =
            self.tracer.build_with_context(builder, &Context::current());
        span.end();
    }
}

impl TelemetrySink for OtlpSink {
    fn record(&self, event: Event) {
        // Structured events prefer the OTLP logs signal. When logs are off,
        // keep the event in the traces signal so disabling logs does not hide
        // frontend/UI events entirely.
        let Some(logger) = &self.logger else {
            self.record_as_span(event);
            return;
        };

        let mut record = logger.create_log_record();
        record.set_target(EVENT_TARGET);
        record.set_timestamp(
            UNIX_EPOCH
                + Duration::from_millis(
                    u64::try_from(event.occurred_at_unix_ms).unwrap_or(0),
                ),
        );
        record.set_observed_timestamp(SystemTime::now());
        record.set_severity_number(severity(event.severity));
        record.set_severity_text(event.severity.as_str());
        record
            .set_body(AnyValue::String(StringValue::from(event.name.clone())));

        record.add_attribute("event.name", event.name);
        record.add_attribute("event.id", event.event_id);
        record.add_attribute(
            "event.schema_version",
            i64::from(event.schema_version),
        );
        if let Some(duration_ms) = event.duration_ms {
            record.add_attribute(
                "event.duration_ms",
                i64::try_from(duration_ms).unwrap_or(i64::MAX),
            );
        }
        for (key, value) in event.attributes {
            if let Some(value) = any_value(&value) {
                record.add_attribute(key, value);
            }
        }

        logger.emit(record);
    }

    fn add_counter(
        &self,
        name: &'static str,
        value: u64,
        attributes: &[Attribute],
    ) {
        let attributes = key_values(attributes);
        let mut counters = self.counters.lock();
        let counter = counters
            .entry(name)
            .or_insert_with(|| self.meter.u64_counter(name).build());
        counter.add(value, &attributes);
    }

    fn record_histogram(
        &self,
        name: &'static str,
        value: f64,
        attributes: &[Attribute],
    ) {
        let attributes = key_values(attributes);
        let mut histograms = self.histograms.lock();
        let histogram = histograms.entry(name).or_insert_with(|| {
            self.meter.f64_histogram(name).with_unit("ms").build()
        });
        histogram.record(value, &attributes);
    }

    fn flush(&self) -> Result<(), TelemetryError> {
        // The owning guard flushes the providers with a caller-provided
        // timeout; doing it here could block a telemetry call indefinitely.
        Ok(())
    }
}

fn severity(severity: Severity) -> opentelemetry::logs::Severity {
    use opentelemetry::logs::Severity as OtelSeverity;
    match severity {
        Severity::Trace => OtelSeverity::Trace,
        Severity::Debug => OtelSeverity::Debug,
        Severity::Info => OtelSeverity::Info,
        Severity::Warn => OtelSeverity::Warn,
        Severity::Error => OtelSeverity::Error,
        _ => OtelSeverity::Info,
    }
}

fn any_value(value: &AttributeValue) -> Option<AnyValue> {
    match value {
        AttributeValue::String(value) => {
            Some(AnyValue::String(StringValue::from(value.clone())))
        }
        AttributeValue::I64(value) => Some(AnyValue::Int(*value)),
        AttributeValue::F64(value) => Some(AnyValue::Double(*value)),
        AttributeValue::Bool(value) => Some(AnyValue::Boolean(*value)),
        _ => None,
    }
}

fn value(value: &AttributeValue) -> Option<Value> {
    match value {
        AttributeValue::String(value) => {
            Some(Value::String(StringValue::from(value.clone())))
        }
        AttributeValue::I64(value) => Some(Value::I64(*value)),
        AttributeValue::F64(value) => Some(Value::F64(*value)),
        AttributeValue::Bool(value) => Some(Value::Bool(*value)),
        _ => None,
    }
}

fn key_values(attributes: &[Attribute]) -> Vec<KeyValue> {
    attributes
        .iter()
        .filter_map(|attribute| {
            value(&attribute.value)
                .map(|value| KeyValue::new(attribute.key.clone(), value))
        })
        .collect()
}

fn otel_value(value: AttributeValue) -> Value {
    match value {
        AttributeValue::String(value) => {
            Value::String(StringValue::from(value))
        }
        AttributeValue::I64(value) => Value::I64(value),
        AttributeValue::F64(value) => Value::F64(value),
        AttributeValue::Bool(value) => Value::Bool(value),
        _ => Value::String(StringValue::from("")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_observability::attrs;
    use opentelemetry::logs::{LoggerProvider as _, Severity as OtelSeverity};
    use opentelemetry::metrics::MeterProvider as _;
    use opentelemetry_sdk::logs::{
        InMemoryLogExporter, InMemoryLogExporterBuilder, SdkLogRecord,
        SdkLoggerProvider,
    };
    use opentelemetry_sdk::metrics::{
        InMemoryMetricExporter, PeriodicReader, SdkMeterProvider,
    };

    fn log_exporter() -> InMemoryLogExporter {
        InMemoryLogExporterBuilder::default().build()
    }

    fn test_tracer() -> SdkTracer {
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .build()
            .tracer("test")
    }

    fn describe(
        record: &SdkLogRecord,
    ) -> (Option<String>, Option<String>, Option<String>) {
        (
            record.target().map(|target| target.to_string()),
            record.body().map(|body| format!("{body:?}")),
            record
                .attributes_iter()
                .find(|(key, _)| key.as_str() == attrs::OUTCOME)
                .map(|(_, value)| format!("{value:?}")),
        )
    }

    fn sample_event() -> Event {
        let mut attributes = std::collections::BTreeMap::new();
        attributes
            .insert(attrs::OUTCOME.to_owned(), AttributeValue::from("ok"));
        attributes.insert(
            attrs::HTTP_STATUS.to_owned(),
            AttributeValue::from(204_i64),
        );
        Event {
            schema_version: launcher_observability::SCHEMA_VERSION,
            event_id: "event-1".to_owned(),
            occurred_at_unix_ms: 1_700_000_000_000,
            name: "instance.launch".to_owned(),
            severity: Severity::Info,
            duration_ms: Some(42),
            attributes,
        }
    }

    #[test]
    fn does_not_emit_logs_without_a_logger() {
        let provider = SdkMeterProvider::builder().build();
        let sink = OtlpSink::new(None, test_tracer(), provider.meter("test"));
        // Nothing to assert beyond "does not panic": no logger is installed.
        sink.record(sample_event());
        assert!(sink.flush().is_ok());
        provider.shutdown().unwrap();
    }

    #[test]
    fn maps_events_to_log_records() {
        let exporter = log_exporter();
        let provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let logger = provider.logger("test");
        let meter_provider = SdkMeterProvider::builder().build();
        let sink = OtlpSink::new(
            Some(logger),
            test_tracer(),
            meter_provider.meter("test"),
        );

        sink.record(sample_event());
        assert!(sink.flush().is_ok());

        let records = exporter.get_emitted_logs().unwrap();
        assert_eq!(records.len(), 1);
        let (target, body, outcome) = describe(&records[0].record);
        assert_eq!(target.as_deref(), Some(EVENT_TARGET));
        assert!(
            body.as_deref()
                .unwrap_or_default()
                .contains("instance.launch")
        );
        assert!(outcome.as_deref().unwrap_or_default().contains("ok"));

        provider.shutdown().unwrap();
        meter_provider.shutdown().unwrap();
    }

    #[test]
    fn maps_counters_and_histograms_to_metrics() {
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(Duration::from_secs(3600))
            .build();
        let provider = SdkMeterProvider::builder().with_reader(reader).build();
        let sink = OtlpSink::new(None, test_tracer(), provider.meter("test"));

        sink.add_counter(
            "axolotl.errors",
            1,
            &[Attribute::new(attrs::OUTCOME, "error")],
        );
        sink.record_histogram(
            "axolotl.ipc.duration",
            12.5,
            &[Attribute::new("command", "list_instances")],
        );
        provider.force_flush().unwrap();

        let captured = exporter.get_finished_metrics().unwrap();
        let names: Vec<_> = captured
            .iter()
            .flat_map(|resource| resource.scope_metrics())
            .flat_map(|scope| scope.metrics())
            .map(|metric| metric.name().to_owned())
            .collect();
        assert!(names.iter().any(|name| name == "axolotl.errors"));
        assert!(names.iter().any(|name| name == "axolotl.ipc.duration"));

        provider.shutdown().unwrap();
    }

    #[test]
    fn severity_mapping_covers_the_contract() {
        assert_eq!(severity(Severity::Trace), OtelSeverity::Trace);
        assert_eq!(severity(Severity::Debug), OtelSeverity::Debug);
        assert_eq!(severity(Severity::Info), OtelSeverity::Info);
        assert_eq!(severity(Severity::Warn), OtelSeverity::Warn);
        assert_eq!(severity(Severity::Error), OtelSeverity::Error);
    }
}
