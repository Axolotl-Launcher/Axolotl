//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction), and scheduling tags whose owner is gone. The shutdown loss
//! list points here too.
//!
//! Running games are tracked in memory only (`ProcessManager`), so a process
//! exit an L0 event could not report leaves nothing behind for a restart to
//! repair: the old `processes` table is retired upstream, and the spec's
//! `AppClosed` compensation has no persisted row left to repair. The loss is
//! bounded to the current session; a restart starts with no process records.
//! Interrupted install jobs are recovered by `install::recover_interrupted_jobs`
//! before this entry runs.

use crate::State;
use crate::state::store::Collection as _;
use crate::state::store::scheduler_tags::SchedulerTags;

pub(crate) async fn at_startup(state: &State) {
    if let Err(error) = crate::telemetry::reconcile(state).await {
        tracing::warn!(%error, "Telemetry startup reconciliation failed");
    }
    match clear_orphaned_tags(state).await {
        Ok(kept) if kept > 0 => {
            tracing::info!(
                kept,
                "Scheduling tags of unfinished install jobs survive the restart"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, "Scheduling tag reconciliation failed");
        }
    }
}

/// Deletes `scheduler_tags` rows whose owner, an install job, is gone or
/// finished, and returns how many rows are left. A surviving row is only a
/// reconciliation key: its tag is minted afresh, and its level re-derived,
/// when the job runs again (spec §5, `TagRegistry::reconcile`).
pub(crate) async fn clear_orphaned_tags(state: &State) -> crate::Result<u64> {
    let mut kept = 0;
    for (id, record) in SchedulerTags::get_all(&state.data).await? {
        let live: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM install_jobs WHERE id = ? AND finished IS NULL",
        )
        .bind(&record.owner)
        .fetch_optional(&state.pool)
        .await?;
        if live.is_some() {
            kept += 1;
        } else {
            SchedulerTags::set(&state.data, &id, None).await?;
        }
    }
    Ok(kept)
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

    #[tokio::test]
    async fn tags_of_gone_or_finished_jobs_are_cleared() {
        use crate::scheduler::tag::Level;
        use crate::state::store::scheduler_tags::SchedulerTagRecord;

        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "reconcile-tags-test".into(),
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query("INSERT INTO install_jobs(id, kind, status, state, created, modified, finished) VALUES ('running', 'pack', 'running', '{}', 1, 1, NULL), ('done', 'pack', 'succeeded', '{}', 1, 1, 2)").execute(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        for (id, owner) in [(1, "running"), (2, "done"), (3, "missing")] {
            let record = SchedulerTagRecord {
                generation: 1,
                level: Level::L3,
                origin: "install".into(),
                owner: owner.into(),
                created_at: 1,
            };
            SchedulerTags::set(&state.data, &id, Some(&record))
                .await
                .unwrap();
        }

        assert_eq!(clear_orphaned_tags(&state).await.unwrap(), 1);
        let left: Vec<_> = SchedulerTags::get_all(&state.data)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, record)| (id, record.owner))
            .collect();
        assert_eq!(left, vec![(1, "running".to_string())]);
        state.pool.close().await;
    }
}
