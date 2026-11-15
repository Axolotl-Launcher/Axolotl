//! Construction and lifetime of the OpenTelemetry providers.
//!
//! All three signals use the SDK's thread-based batch processors and periodic
//! reader together with the blocking OTLP/HTTP client. Export therefore works
//! from any thread and never needs a Tokio runtime, which keeps logger startup
//! independent of the async runtime the shell creates later.

use std::collections::HashMap;
use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry::logs::LoggerProvider as _;
use opentelemetry::metrics::{Meter, MeterProvider as _};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{
    BatchLogProcessor, SdkLogger, SdkLoggerProvider,
};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{
    BatchConfigBuilder, BatchSpanProcessor, Sampler, SdkTracer,
    SdkTracerProvider,
};

use super::config::{ObservabilityConfig, SignalConfig};

/// Instrumentation scope shared by every launcher signal.
pub(crate) const SCOPE_NAME: &str = "axolotl-launcher";

/// Owns the providers for as long as export should stay active.
///
/// Cloning is cheap and shares the same providers; shutting one clone down
/// shuts the shared pipeline down, which is only done at process exit.
#[derive(Clone)]
pub struct ObservabilityGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: Option<SdkLoggerProvider>,
    tracer: SdkTracer,
    meter: Meter,
    logger: Option<SdkLogger>,
    raw_logs: bool,
}

impl ObservabilityGuard {
    pub(crate) fn tracer(&self) -> SdkTracer {
        self.tracer.clone()
    }

    pub(crate) fn meter(&self) -> Meter {
        self.meter.clone()
    }

    /// Logger handle used by the telemetry sink, present only when the logs
    /// signal was explicitly enabled.
    pub(crate) fn logger(&self) -> Option<SdkLogger> {
        self.logger.clone()
    }

    /// Provider handle used by the raw-log bridge. Present only when the logs
    /// signal was explicitly enabled.
    pub(crate) fn logger_provider(&self) -> Option<&SdkLoggerProvider> {
        self.logger_provider.as_ref()
    }

    pub(crate) fn logs_enabled(&self) -> bool {
        self.logger.is_some()
    }

    /// Whether raw tracing events should be bridged as OTLP log records.
    pub(crate) fn raw_logs(&self) -> bool {
        self.raw_logs
    }

    /// Flush every enabled signal, returning whether the whole flush finished
    /// within `timeout`. The exporters run on a helper thread so a slow or
    /// unreachable collector can never block application shutdown past the
    /// caller's budget.
    pub fn flush(&self, timeout: Duration) -> bool {
        let tracer_provider = self.tracer_provider.clone();
        let meter_provider = self.meter_provider.clone();
        let logger_provider = self.logger_provider.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("launcher-otel-flush".to_owned())
            .spawn(move || {
                let mut ok = tracer_provider.force_flush().is_ok();
                ok &= meter_provider.force_flush().is_ok();
                if let Some(provider) = &logger_provider {
                    ok &= provider.force_flush().is_ok();
                }
                let _ = sender.send(ok);
            })
            .ok();
        receiver.recv_timeout(timeout).unwrap_or(false)
    }

    /// Flush and stop the providers off-thread. Safe to call more than once.
    pub(crate) fn shutdown(&self) {
        let tracer_provider = self.tracer_provider.clone();
        let meter_provider = self.meter_provider.clone();
        let logger_provider = self.logger_provider.clone();
        std::thread::Builder::new()
            .name("launcher-otel-shutdown".to_owned())
            .spawn(move || {
                let _ = tracer_provider.shutdown();
                let _ = meter_provider.shutdown();
                if let Some(provider) = &logger_provider {
                    let _ = provider.shutdown();
                }
            })
            .ok();
    }
}

fn resource(
    config: &ObservabilityConfig,
    app_identifier: &str,
    session_id: &str,
) -> Resource {
    let mut builder = Resource::builder();
    for (key, value) in &config.resource_attributes {
        builder =
            builder.with_attribute(KeyValue::new(key.clone(), value.clone()));
    }
    builder = builder.with_attribute(KeyValue::new(
        "app.identifier",
        app_identifier.to_owned(),
    ));
    builder = builder
        .with_attribute(KeyValue::new("session.id", session_id.to_owned()));
    builder
        .with_service_name(config.service_name.clone())
        .build()
}

fn headers(signal: &SignalConfig) -> HashMap<String, String> {
    signal
        .headers
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn span_exporter(
    signal: &SignalConfig,
) -> Result<opentelemetry_otlp::SpanExporter, String> {
    opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(signal.endpoint.clone())
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(signal.timeout)
        .with_headers(headers(signal))
        .build()
        .map_err(|error| {
            format!("failed to build the OTLP span exporter: {error}")
        })
}

fn metric_exporter(
    signal: &SignalConfig,
) -> Result<opentelemetry_otlp::MetricExporter, String> {
    opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint(signal.endpoint.clone())
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(signal.timeout)
        .with_headers(headers(signal))
        .build()
        .map_err(|error| {
            format!("failed to build the OTLP metric exporter: {error}")
        })
}

fn log_exporter(
    signal: &SignalConfig,
) -> Result<opentelemetry_otlp::LogExporter, String> {
    opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_endpoint(signal.endpoint.clone())
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(signal.timeout)
        .with_headers(headers(signal))
        .build()
        .map_err(|error| {
            format!("failed to build the OTLP log exporter: {error}")
        })
}

/// Build every provider required by `config`.
pub(crate) fn build(
    config: &ObservabilityConfig,
    app_identifier: &str,
    session_id: &str,
) -> Result<ObservabilityGuard, String> {
    let resource = resource(config, app_identifier, session_id);

    let batch_config = BatchConfigBuilder::default()
        .with_max_queue_size(config.batch.max_queue_size)
        .with_max_export_batch_size(config.batch.max_export_batch_size)
        .with_scheduled_delay(config.batch.scheduled_delay)
        .build();
    let batch = BatchSpanProcessor::builder(span_exporter(&config.traces)?)
        .with_batch_config(batch_config)
        .build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_sampler(Sampler::ParentBased(Box::new(
            Sampler::TraceIdRatioBased(config.sample_ratio),
        )))
        .with_resource(resource.clone())
        .with_span_processor(batch)
        .build();

    let reader = PeriodicReader::builder(metric_exporter(&config.metrics)?)
        .with_interval(config.metric_interval)
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource.clone())
        .build();

    let (logger_provider, logger) = match &config.logs {
        Some(signal) => {
            let processor =
                BatchLogProcessor::builder(log_exporter(signal)?).build();
            let provider = SdkLoggerProvider::builder()
                .with_resource(resource)
                .with_log_processor(processor)
                .build();
            let logger = provider.logger(SCOPE_NAME);
            (Some(provider), Some(logger))
        }
        None => (None, None),
    };

    let tracer = tracer_provider.tracer(SCOPE_NAME);
    let meter = meter_provider.meter(SCOPE_NAME);

    Ok(ObservabilityGuard {
        tracer_provider,
        meter_provider,
        logger_provider,
        tracer,
        meter,
        logger,
        raw_logs: config.raw_logs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_is_bounded_when_disabled_signals_are_absent() {
        // Building an exporter must not contact the network; with a loopback
        // endpoint the provider builds and flushes without a live collector.
        let env = std::collections::HashMap::from([
            ("AXOLOTL_OBSERVABILITY".to_owned(), "1".to_owned()),
            (
                "OTEL_EXPORTER_OTLP_ENDPOINT".to_owned(),
                "http://127.0.0.1:4318".to_owned(),
            ),
        ]);
        let config =
            super::super::config::ObservabilityConfig::from_env_with(|key| {
                env.get(key).cloned()
            })
            .unwrap()
            .unwrap();
        let guard = build(&config, "test.app", "session").unwrap();
        assert!(!guard.logs_enabled());
        // Export fails (nothing listens), which a flush reports as false, but
        // it must return without panicking.
        let _ = guard.flush(Duration::from_millis(200));
        guard.shutdown();
    }
}
