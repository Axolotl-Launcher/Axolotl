//! Process-wide observability wiring for the launcher runtime.
//!
//! The contract crate is transport-free; this module reads the OTLP
//! configuration, owns the SDK providers and installs the bridge sink. Every
//! public helper is cheap and safe to call when export is disabled.

pub mod capture;
mod command_metrics;
mod config;
mod runtime;
mod sink;
pub mod spans;

pub use launcher_observability::{
    Attribute, AttributeValue, Telemetry, attrs, events,
};
pub use runtime::ObservabilityGuard;

use std::sync::OnceLock;
use std::time::Duration;

static GUARD: OnceLock<Option<ObservabilityGuard>> = OnceLock::new();
static SESSION_ID: OnceLock<String> = OnceLock::new();
static SIGNALS: OnceLock<&'static [&'static str]> = OnceLock::new();

/// The process-wide telemetry registry.
pub fn telemetry() -> &'static Telemetry {
    Telemetry::global()
}

/// Whether telemetry is being recorded at all: either OTLP export started or a
/// local sink such as [`capture`] is installed. Callers use this to skip
/// building expensive attributes when nothing consumes them.
pub fn is_enabled() -> bool {
    otlp_enabled() || capture::active().is_some()
}

/// Whether the OTLP pipeline itself is running.
///
/// The log filter uses this rather than [`is_enabled`]: widening the file log to
/// INFO domain spans is a cost the operator opted into with
/// `AXOLOTL_OBSERVABILITY`, not something a local diagnostics toggle should
/// change behind their back.
pub fn otlp_enabled() -> bool {
    GUARD.get().is_some_and(Option::is_some)
}

/// Start local capture without OTLP export.
///
/// This is the switch the diagnostics MCP uses: the launcher keeps recent
/// events, metric aggregates and domain span durations in memory so a client on
/// this computer can query them. Nothing leaves the process.
pub fn start_capture() -> std::sync::Arc<capture::Capture> {
    spans::start();
    capture::start(capture::DEFAULT_EVENT_CAPACITY)
}

/// Stop local capture and drop everything it retained.
pub fn stop_capture() {
    spans::stop();
    capture::stop();
}

/// The random identifier shared by every record of this process, when export
/// is enabled. It is never persisted and never identifies the installation.
pub fn session_id() -> Option<&'static str> {
    SESSION_ID.get().map(String::as_str)
}

/// Signals enabled for this process. Logs are only listed when the operator
/// explicitly opted into the logs signal.
pub fn signals() -> &'static [&'static str] {
    SIGNALS.get().copied().unwrap_or(&["metrics", "traces"])
}

/// Record one failed operation in the `axolotl.errors` counter.
///
/// `kind` must be a stable category such as `timeout` or `io`, never a raw
/// error message; `domain` is the subsystem, such as `instance` or `network`.
pub fn record_error(domain: &'static str, kind: &'static str) {
    telemetry().counter("axolotl.errors").add(
        1,
        &[
            Attribute::new(attrs::OUTCOME, "error"),
            Attribute::new(attrs::ERROR_KIND, kind),
            Attribute::new("domain", domain),
        ],
    );
}

/// Flush every provider. Returns `false` when a flush failed or the timeout
/// elapsed first. With only local capture installed there is nothing to push,
/// so this reports success. Never panics.
pub fn flush(timeout: Duration) -> bool {
    let guard_ok = GUARD
        .get()
        .and_then(Option::as_ref)
        .is_none_or(|guard| guard.flush(timeout));
    let sinks_ok = telemetry().flush().is_ok();
    guard_ok && sinks_ok
}

/// Build the providers and install the OTLP sink.
///
/// Called once by the logger; later calls return a clone of the same guard.
/// Returns `None` when export is disabled or misconfigured, in which case the
/// launcher behaves exactly as it did before observability existed.
pub fn start(app_identifier: &str) -> Option<ObservabilityGuard> {
    GUARD
        .get_or_init(|| {
            let config = match config::ObservabilityConfig::from_env() {
                Ok(Some(config)) => config,
                Ok(None) => return None,
                Err(error) => {
                    eprintln!("axolotl: observability is disabled: {error}");
                    return None;
                }
            };

            let session_id = uuid::Uuid::new_v4().to_string();
            let signals: &'static [&'static str] = if config.logs.is_some() {
                &["logs", "metrics", "traces"]
            } else {
                &["metrics", "traces"]
            };
            let _ = SIGNALS.set(signals);
            match runtime::build(&config, app_identifier, &session_id) {
                Ok(guard) => {
                    telemetry().install(
                        sink::OtlpSink::new(
                            guard.logger(),
                            guard.tracer(),
                            guard.meter(),
                        ),
                        session_id.clone(),
                    );
                    let _ = SESSION_ID.set(session_id);
                    Some(guard)
                }
                Err(error) => {
                    eprintln!(
                        "axolotl: failed to start observability: {error}"
                    );
                    None
                }
            }
        })
        .clone()
}

pub(crate) use command_metrics::CommandMetricsLayer;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Capture is process-wide, so these tests must not interleave.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[test]
    fn disabled_without_the_opt_in_variable() {
        let config = config::ObservabilityConfig::from_env_with(|_| None);
        assert!(matches!(config, Ok(None)));
    }

    #[test]
    fn flush_succeeds_when_only_capture_is_installed() {
        let _guard = lock();
        // Local capture keeps data in memory; there is nothing to push, so a
        // flush must not report failure just because OTLP is off.
        let capture = capture::start(4);
        assert!(capture.is_recording());
        assert!(flush(Duration::from_millis(1)));
        capture::stop();
    }

    #[test]
    fn flush_without_any_consumer_succeeds() {
        let _guard = lock();
        capture::stop();
        assert!(flush(Duration::from_millis(1)));
    }

    #[test]
    fn restarting_capture_does_not_register_the_sink_twice() {
        let _guard = lock();
        capture::stop();
        capture::reset_for_test();
        capture::start(8);
        assert!(capture::is_registered());
        telemetry().event(events::APP_STARTED).emit().unwrap();

        // The settings toggle stops and starts capture on every change; the
        // registry has no remove_sink, so a second registration would make
        // every event arrive twice. `stop()` also drops what was retained, so
        // the first emit is gone and only the second one can be counted.
        capture::stop();
        capture::start(8);
        telemetry().event(events::APP_STARTED).emit().unwrap();

        let recorded =
            capture::active().map(|capture| capture.events(10, None, None));
        capture::stop();
        capture::reset_for_test();
        assert_eq!(
            recorded.expect("capture installed").len(),
            1,
            "one emit after the restart must be recorded once, not once per start()"
        );
    }

    #[test]
    fn local_capture_does_not_widen_the_log_filter() {
        // The file log must keep the user's level: only OTLP needs INFO domain
        // spans collected globally, and capture filters per layer instead.
        let _guard = lock();
        capture::stop();
        capture::reset_for_test();
        capture::start(4);
        assert!(is_enabled());
        assert!(!otlp_enabled());
        capture::stop();
        capture::reset_for_test();
    }

    #[test]
    fn capture_alone_reports_telemetry_as_enabled() {
        let _guard = lock();
        capture::stop();
        capture::reset_for_test();
        let started = super::capture::start(4);
        assert!(is_enabled());
        assert!(started.is_recording());
        super::capture::stop();
        assert!(!started.is_recording());
        assert!(!is_enabled());
    }
}
