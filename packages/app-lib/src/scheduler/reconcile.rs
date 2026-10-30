//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction), and scheduling tags whose owner is gone. The shutdown loss
//! list points here too.
//!
//! Running games are tracked in memory only (`ProcessManager`), so a process
//! exit an L0 event could not report leaves nothing behind for a restart to
//! repair; the old `processes` table is retired upstream.

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
