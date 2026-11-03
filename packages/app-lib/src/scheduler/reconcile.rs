//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction). The shutdown loss list points here too.
//!
//! Running games are tracked in memory only (`ProcessManager`), so a process
//! exit an L0 event could not report leaves nothing behind for a restart to
//! repair: the old `processes` table is retired upstream, and the spec's
//! `AppClosed` compensation has no persisted row left to repair. The loss is
//! bounded to the current session; a restart starts with no process records.
//! Interrupted install jobs are recovered by `install::recover_interrupted_jobs`
//! before this entry runs.

use crate::State;

pub(crate) async fn at_startup(state: &State) {
    if let Err(error) = crate::telemetry::reconcile(state).await {
        tracing::warn!(%error, "Telemetry startup reconciliation failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    const RECONCILIATION_FAILED: &str =
        "Telemetry startup reconciliation failed";

    /// A state over a database borrowed from the store half, whose settings
    /// row says whether telemetry is on. The statements stay in `state::store`;
    /// this suite only drives the scheduler's startup entry.
    async fn fixture(telemetry: bool) -> (tempfile::TempDir, Arc<State>) {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "reconcile-test".into(),
        };
        let pool = crate::state::store::tests::support::migrated_pool().await;
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let mut settings =
            crate::state::Settings::get(&state.data).await.unwrap();
        settings.telemetry = telemetry;
        settings.telemetry_consent_version = u32::from(telemetry);
        settings.update(&state.data).await.unwrap();
        (temp, state)
    }

    /// The heartbeat events waiting in the outbox, read through the collection
    /// rather than through a statement.
    async fn heartbeats(state: &State) -> Vec<String> {
        use crate::state::store::Collection;
        use crate::state::store::telemetry_outbox::{
            TelemetryEventType, TelemetryOutbox,
        };
        TelemetryOutbox::get_all(&state.data)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|(key, record)| {
                (record.event_type == TelemetryEventType::Heartbeat)
                    .then_some(key)
            })
            .collect()
    }

    /// Records every event the subscriber formats, tagged with its level, so a
    /// test can say what was reported and at which severity.
    struct WarningCapture(Arc<Mutex<Vec<String>>>);

    impl std::io::Write for WarningCapture {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            let text = String::from_utf8_lossy(buffer);
            self.0
                .lock()
                .unwrap()
                .extend(text.lines().map(str::to_owned));
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Installs a subscriber that records every formatted event for the rest
    /// of the guard's scope.
    fn capture_lines()
    -> (tracing::subscriber::DefaultGuard, Arc<Mutex<Vec<String>>>) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                // One `write` per event, so a line is a whole event rather
                // than a fragment of one.
                .with_writer({
                    let lines = lines.clone();
                    move || WarningCapture(lines.clone())
                }),
        );
        let guard = tracing::subscriber::set_default(subscriber);
        (guard, lines)
    }

    #[tokio::test]
    async fn a_startup_reconciliation_runs_the_telemetry_cycle() {
        let (_temp, state) = fixture(true).await;
        assert!(heartbeats(&state).await.is_empty());
        at_startup(&state).await;
        assert_eq!(
            heartbeats(&state).await.len(),
            1,
            "the startup entry reached the telemetry cycle"
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn a_failed_reconciliation_is_reported_and_startup_continues() {
        let (_temp, state) = fixture(true).await;
        // Hold the install database permit so the reconciliation cannot get
        // it: this is the one failure the entry swallows, and startup must not
        // return an error from it.
        let held = state.install_db_semaphore.acquire().await.unwrap();
        state.set_install_db_permit_timeout_for_test(
            std::time::Duration::from_millis(1),
        );

        let (_subscriber, lines) = capture_lines();
        at_startup(&state).await;
        drop(held);

        assert!(
            heartbeats(&state).await.is_empty(),
            "the blocked cycle did not queue a heartbeat"
        );
        let reported: Vec<_> = lines
            .lock()
            .unwrap()
            .iter()
            .filter(|line| line.contains(RECONCILIATION_FAILED))
            .cloned()
            .collect();
        assert_eq!(
            reported.len(),
            1,
            "the failure was reported once, not {reported:?}"
        );
        state.pool.close().await;
    }
}
