//! Process-wide observability wiring for the launcher runtime.
//!
//! The contract crate is transport-free; this module reads the OTLP
//! configuration, owns the SDK providers and installs the bridge sink. Every
//! public helper is cheap and safe to call when export is disabled.

mod command_metrics;
mod config;
mod runtime;
mod sink;

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

/// Whether OTLP export started successfully. Callers use this to skip building
/// expensive attributes when telemetry is off.
pub fn is_enabled() -> bool {
    GUARD.get().is_some_and(Option::is_some)
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

/// Flush every provider. Returns `false` when export is disabled, a flush
/// failed, or the timeout elapsed first. Never panics.
pub fn flush(timeout: Duration) -> bool {
    let Some(guard) = GUARD.get().and_then(Option::as_ref) else {
        return false;
    };
    let guard_ok = guard.flush(timeout);
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

    #[test]
    fn disabled_without_the_opt_in_variable() {
        let config = config::ObservabilityConfig::from_env_with(|_| None);
        assert!(matches!(config, Ok(None)));
    }

    #[test]
    fn flush_is_false_when_not_started() {
        assert!(!flush(Duration::from_millis(1)));
    }
}
