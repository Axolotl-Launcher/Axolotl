//! The one startup reconciliation entry. Work that a previous session could
//! leave half done is repaired here before any background loop starts:
//! telemetry's multi-step writes (each step atomic or idempotent, but not one
//! transaction), and process records an L0 exit event could not clear when
//! its reserved queue overflowed. The shutdown loss list points here too.

use crate::State;

pub(crate) async fn at_startup(state: &State) {
    if let Err(error) = crate::telemetry::reconcile(state).await {
        tracing::warn!(%error, "Telemetry startup reconciliation failed");
    }
    if let Err(error) = clear_dead_process_records(state).await {
        tracing::warn!(%error, "Process record reconciliation failed");
    }
}

/// Removes `processes` rows whose process no longer runs, so no instance
/// stays counted as running after its process is gone. A live pid is not
/// enough: the system may have handed it to another process since, so the
/// recorded start time has to match as well.
pub(crate) async fn clear_dead_process_records(
    state: &State,
) -> crate::Result<u64> {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT pid, start_time FROM processes")
            .fetch_all(&state.pool)
            .await?;
    let mut cleared = 0;
    for (pid, start_time) in rows {
        if !process_is_alive(pid, start_time) {
            cleared += sqlx::query("DELETE FROM processes WHERE pid = ?")
                .bind(pid)
                .execute(&state.pool)
                .await?
                .rows_affected();
        }
    }
    Ok(cleared)
}

/// Seconds a recorded start time may differ from the system's own.
const START_TIME_TOLERANCE: u64 = 2;

/// Whether `pid` runs and is still the process that started at `start_time`
/// (seconds since the Unix epoch).
fn process_is_alive(pid: i64, start_time: i64) -> bool {
    let (Ok(pid), Ok(recorded)) =
        (u32::try_from(pid), u64::try_from(start_time))
    else {
        return false;
    };
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some_and(|process| {
        process.start_time().abs_diff(recorded) <= START_TIME_TOLERANCE
    })
}

#[cfg(test)]
fn own_start_time() -> i64 {
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .map_or(0, |process| process.start_time() as i64)
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
        for (pid, started) in [(live, own_start_time()), (dead, 1)] {
            sqlx::query("INSERT INTO processes(pid, start_time, name, executable, instance_id) VALUES (?, ?, 'java', 'java', 'sample')").bind(pid).bind(started).execute(&pool).await.unwrap();
        }
        let state = crate::state::test_state(directories, pool).await.unwrap();

        assert_eq!(clear_dead_process_records(&state).await.unwrap(), 1);
        let left: Vec<i64> = sqlx::query_scalar("SELECT pid FROM processes")
            .fetch_all(&state.pool)
            .await
            .unwrap();
        assert_eq!(left, vec![live]);

        // The same live pid recorded with another start time is a pid the
        // system reused: that record is gone too.
        sqlx::query("UPDATE processes SET start_time = 1 WHERE pid = ?")
            .bind(live)
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(clear_dead_process_records(&state).await.unwrap(), 1);
        let left: Vec<i64> = sqlx::query_scalar("SELECT pid FROM processes")
            .fetch_all(&state.pool)
            .await
            .unwrap();
        assert!(left.is_empty());
        state.pool.close().await;
    }
}
