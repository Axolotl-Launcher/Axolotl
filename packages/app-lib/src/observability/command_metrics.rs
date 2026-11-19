//! IPC command duration histogram.
//!
//! Tauri command handlers are instrumented with spans whose target starts with
//! `theseus_gui`; this layer turns the span lifetime into the
//! `axolotl.ipc.duration` histogram. Nothing is recorded when observability is
//! disabled, and the layer never emits tracing events itself, so it cannot
//! feed back into the subscriber.
//!
//! The layer is attached for the whole process rather than only when OTLP
//! starts, because local capture can be enabled from the settings page long
//! after the subscriber exists. Each span therefore checks [`is_enabled`]
//! itself: with no consumer installed the cost is one atomic load.

use std::sync::Arc;
use std::time::Instant;

use tracing::Subscriber;
use tracing::span::{Attributes, Id};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use launcher_observability::Attribute;

const COMMAND_TARGET_PREFIX: &str = "theseus_gui";
const DURATION_HISTOGRAM: &str = "axolotl.ipc.duration";

type Recorder = Arc<dyn Fn(&str, &str, f64) + Send + Sync>;

pub(crate) struct CommandMetricsLayer {
    record: Recorder,
}

impl CommandMetricsLayer {
    pub(crate) fn new() -> Self {
        Self {
            record: Arc::new(|command, domain, duration_ms| {
                // Capture can start after this layer is attached, so the check
                // has to happen per span, not when the subscriber is built.
                if !crate::observability::is_enabled() {
                    return;
                }
                crate::observability::telemetry()
                    .histogram(DURATION_HISTOGRAM)
                    .record(
                        duration_ms,
                        &[
                            Attribute::new("command", command.to_owned()),
                            Attribute::new("domain", domain.to_owned()),
                        ],
                    );
            }),
        }
    }

    #[cfg(test)]
    fn with_recorder(record: Recorder) -> Self {
        Self { record }
    }
}

struct SpanStart(Instant);

fn is_command_target(target: &str) -> bool {
    target == COMMAND_TARGET_PREFIX
        || target
            .strip_prefix(COMMAND_TARGET_PREFIX)
            .is_some_and(|rest| rest.starts_with("::"))
}

/// Last target segment, such as `instance` for `theseus_gui::api::instance`.
fn command_domain(target: &str) -> &str {
    target.rsplit("::").next().unwrap_or(target)
}

impl<S> Layer<S> for CommandMetricsLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attributes: &Attributes<'_>,
        id: &Id,
        context: Context<'_, S>,
    ) {
        if !is_command_target(attributes.metadata().target()) {
            return;
        }
        if let Some(span) = context.span(id) {
            span.extensions_mut().insert(SpanStart(Instant::now()));
        }
    }

    fn on_close(&self, id: Id, context: Context<'_, S>) {
        let Some(span) = context.span(&id) else {
            return;
        };
        let target = span.metadata().target();
        if !is_command_target(target) {
            return;
        }
        let Some(start) =
            span.extensions().get::<SpanStart>().map(|start| start.0)
        else {
            return;
        };
        let duration_ms = start.elapsed().as_secs_f64() * 1_000.0;
        (self.record)(span.name(), command_domain(target), duration_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::test_lock;
    use std::sync::Mutex;
    use tracing_subscriber::prelude::*;

    /// The crate has one lock for every test that flips the shared statics.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        test_lock()
    }

    #[test]
    fn records_durations_with_only_local_capture_installed() {
        let _guard = lock();
        crate::observability::capture::stop();
        crate::observability::capture::reset_for_test();
        crate::observability::start_capture();
        assert!(crate::observability::is_enabled());

        let subscriber =
            tracing_subscriber::registry().with(CommandMetricsLayer::new());
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                target: "theseus_gui::api::instance",
                "list_instances"
            );
            drop(span);
        });

        let histograms = crate::observability::capture::active()
            .expect("capture")
            .histograms();
        crate::observability::stop_capture();

        let recorded = histograms
            .iter()
            .find(|histogram| histogram.name == DURATION_HISTOGRAM)
            .expect("ipc duration recorded");
        assert_eq!(recorded.count, 1);
        assert_eq!(
            recorded.attributes.get("command").and_then(|value| {
                match value {
                    launcher_observability::AttributeValue::String(value) => {
                        Some(value.as_str())
                    }
                    _ => None,
                }
            }),
            Some("list_instances")
        );
    }

    #[test]
    fn matches_only_the_command_target_namespace() {
        assert!(is_command_target("theseus_gui"));
        assert!(is_command_target("theseus_gui::api::instance"));
        assert!(!is_command_target("theseus_gui_extra"));
        assert!(!is_command_target("theseus"));
        assert!(!is_command_target(""));
    }

    #[test]
    fn extracts_the_last_target_segment() {
        assert_eq!(command_domain("theseus_gui::api::instance"), "instance");
        assert_eq!(command_domain("theseus_gui"), "theseus_gui");
    }

    #[test]
    fn records_closed_command_spans() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = captured.clone();
        let layer = CommandMetricsLayer::with_recorder(Arc::new(
            move |command: &str, domain: &str, duration_ms: f64| {
                sink.lock().unwrap().push((
                    command.to_owned(),
                    domain.to_owned(),
                    duration_ms,
                ));
            },
        ));
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                target: "theseus_gui::api::instance",
                "list_instances"
            );
            drop(span);

            let ignored = tracing::info_span!(
                target: "theseus_gui_extra",
                "not_a_command"
            );
            drop(ignored);
        });

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0, "list_instances");
        assert_eq!(captured[0].1, "instance");
        assert!(captured[0].2 >= 0.0);
    }
}
