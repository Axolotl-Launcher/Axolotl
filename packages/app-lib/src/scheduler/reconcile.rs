//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction), process records an L0 exit event could not clear when its
//! reserved queue overflowed, and scheduling tags whose owner is gone. The
//! shutdown loss list points here too.

use crate::State;
use crate::state::store::Collection as _;
use crate::state::store::scheduler_tags::SchedulerTags;

pub(crate) async fn at_startup(state: &State) {
    if let Err(error) = crate::telemetry::reconcile(state).await {
        tracing::warn!(%error, "Telemetry startup reconciliation failed");
    }
    if let Err(error) = clear_dead_process_records(state).await {
        tracing::warn!(%error, "Process record reconciliation failed");
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

/// Removes `processes` rows whose pid no longer runs, so no instance stays
/// counted as running after its process is gone.
pub(crate) async fn clear_dead_process_records(
    state: &State,
) -> crate::Result<u64> {
    let pids: Vec<i64> = sqlx::query_scalar("SELECT pid FROM processes")
        .fetch_all(&state.pool)
        .await?;
    let mut cleared = 0;
    for pid in pids {
        if !process_is_alive(pid) {
            cleared += sqlx::query("DELETE FROM processes WHERE pid = ?")
                .bind(pid)
                .execute(&state.pool)
                .await?
                .rows_affected();
        }
    }
    Ok(cleared)
}

fn process_is_alive(pid: i64) -> bool {
    let Ok(pid) = u32::try_from(pid) else {
        return false;
    };
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dead_process_records_are_cleared_and_live_ones_kept() {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "reconcile-test".into(),
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('sample', 'sample', 'installed', '1', 'Sample', 1, 1)").execute(&pool).await.unwrap();
        let live = i64::from(std::process::id());
        let dead = i64::from(u32::MAX - 1);
        for pid in [live, dead] {
            sqlx::query("INSERT INTO processes(pid, start_time, name, executable, instance_id) VALUES (?, 1, 'java', 'java', 'sample')").bind(pid).execute(&pool).await.unwrap();
        }
        let state = crate::state::test_state(directories, pool).await.unwrap();

        assert_eq!(clear_dead_process_records(&state).await.unwrap(), 1);
        let left: Vec<i64> = sqlx::query_scalar("SELECT pid FROM processes")
            .fetch_all(&state.pool)
            .await
            .unwrap();
        assert_eq!(left, vec![live]);
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
