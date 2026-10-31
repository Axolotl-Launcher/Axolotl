//! The admission queue. A queued job waits for its slot in bounded steps: the
//! acquisition future is kept across steps, so the job keeps its place in
//! line, and between steps the job's stored status decides whether it still
//! waits. A step running out is therefore never a failure.

use super::*;
use std::future::Future;
use std::time::Duration;

type Slot<'a> = (
    Option<tokio::sync::OwnedMutexGuard<()>>,
    tokio::sync::SemaphorePermit<'a>,
);

pub(super) async fn wait_for_slot<'a>(
    state: &'a State,
    job: &store::InstallJobRecord,
    step: Duration,
) -> crate::Result<Option<Slot<'a>>> {
    // The same key worker recovery locks, so a waiting worker and a
    // recovery of the same job always exclude each other.
    let lock = state
        .install_execution_locks
        .entry(execution_lock_key(job))
        .or_default()
        .clone();
    let Some(execution) =
        wait_in_line(state, job.id, step, lock.lock_owned()).await?
    else {
        return Ok(None);
    };
    let execution = Some(execution);
    let Some(global) = wait_in_line(
        state,
        job.id,
        step,
        state.install_job_semaphore.acquire(),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some((execution, global?)))
}

/// Awaits `acquire` without dropping it. Every `step` the job is read again;
/// once it has finished or started canceling the wait ends with `None`.
async fn wait_in_line<T>(
    state: &State,
    job_id: Uuid,
    step: Duration,
    acquire: impl Future<Output = T>,
) -> crate::Result<Option<T>> {
    tokio::pin!(acquire);
    let mut steps = 0u32;
    loop {
        if let Ok(value) = tokio::time::timeout(step, &mut acquire).await {
            return Ok(Some(value));
        }
        steps += 1;
        let status = store::get_required(job_id, state).await?.status;
        if status.is_finished() || status == InstallJobStatus::Canceling {
            tracing::info!(%job_id, status = status.as_str(), "Install job left the queue before getting a slot");
            return Ok(None);
        }
        tracing::debug!(%job_id, steps, "Install job still queued for an execution slot");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const STEP: Duration = Duration::from_millis(50);

    async fn fixture() -> (tempfile::TempDir, Arc<State>) {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "queue-test".into(),
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        for id in ["first", "second", "third", "fourth"] {
            sqlx::query("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES (?, ?, 'installed', '1', ?, 1, 1)").bind(id).bind(id).bind(id).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES (?, ?, 'content', 'local', 'available', '1.20.1', 'fabric', 1, 1)").bind(format!("set-{id}")).bind(id).execute(&pool).await.unwrap();
            sqlx::query(
                "UPDATE instances SET applied_content_set_id = ? WHERE id = ?",
            )
            .bind(format!("set-{id}"))
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let state = crate::state::test_state(directories, pool).await.unwrap();
        (temp, state)
    }

    fn request(instance: &str) -> InstallRequest {
        InstallRequest::ChangeContent {
            instance_id: instance.into(),
            intent: crate::install::ContentChangeIntent::UpdateOne {
                content_id: "mod".into(),
                target_release_id: Some("version".into()),
            },
            display_title: "Change content".into(),
            display_icon: None,
        }
    }

    #[tokio::test]
    async fn admission_timeout_falls_back_to_queue() {
        let (_temp, state) = fixture().await;
        let job = enqueue(request("first"), &state).await.unwrap();
        let record = store::get_required(job.job_id, &state).await.unwrap();
        let holder = wait_for_slot(&state, &record, STEP).await.unwrap();
        assert!(holder.is_some());

        let waiter = wait_for_slot(&state, &record, STEP);
        tokio::pin!(waiter);
        assert!(
            tokio::time::timeout(STEP * 6, &mut waiter).await.is_err(),
            "a timed-out wait for the execution lock keeps the job queued"
        );
        assert_eq!(
            store::get_required(job.job_id, &state)
                .await
                .unwrap()
                .status,
            InstallJobStatus::Queued
        );
        drop(holder);
        let slot = tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("the queued job gets the lock once it is released")
            .unwrap();
        assert!(slot.is_some());
        state.pool.close().await;
    }

    #[tokio::test]
    async fn second_request_succeeds_after_queueing() {
        let (_temp, state) = fixture().await;
        let mut records = Vec::new();
        for instance in ["first", "second", "third"] {
            let job = enqueue(request(instance), &state).await.unwrap();
            records
                .push(store::get_required(job.job_id, &state).await.unwrap());
        }
        let first = wait_for_slot(&state, &records[0], STEP).await.unwrap();
        let second = wait_for_slot(&state, &records[1], STEP).await.unwrap();
        assert!(first.is_some() && second.is_some());

        let third = wait_for_slot(&state, &records[2], STEP);
        tokio::pin!(third);
        assert!(
            tokio::time::timeout(STEP * 6, &mut third).await.is_err(),
            "the third job waits through several steps instead of failing"
        );
        drop(first);
        let third = tokio::time::timeout(Duration::from_secs(5), third)
            .await
            .expect("the queued job gets the freed slot")
            .unwrap();
        assert!(third.is_some());
        drop(second);
        state.pool.close().await;
    }

    #[tokio::test]
    async fn canceled_job_leaves_the_queue() {
        let (_temp, state) = fixture().await;
        let mut records = Vec::new();
        for instance in ["first", "second", "third"] {
            let job = enqueue(request(instance), &state).await.unwrap();
            records
                .push(store::get_required(job.job_id, &state).await.unwrap());
        }
        let _first = wait_for_slot(&state, &records[0], STEP).await.unwrap();
        let _second = wait_for_slot(&state, &records[1], STEP).await.unwrap();

        let third = wait_for_slot(&state, &records[2], STEP);
        tokio::pin!(third);
        assert!(tokio::time::timeout(STEP * 2, &mut third).await.is_err());
        store::update_status(
            records[2].id,
            InstallJobStatus::Canceled,
            &records[2].state,
            &state,
        )
        .await
        .unwrap();
        let left = tokio::time::timeout(Duration::from_secs(5), third)
            .await
            .expect("a canceled job stops waiting at its next step")
            .unwrap();
        assert!(left.is_none());
        state.pool.close().await;
    }

    #[tokio::test]
    async fn canceling_job_leaves_the_queue() {
        let (_temp, state) = fixture().await;
        let mut records = Vec::new();
        for instance in ["first", "second", "third"] {
            let job = enqueue(request(instance), &state).await.unwrap();
            records
                .push(store::get_required(job.job_id, &state).await.unwrap());
        }
        let _first = wait_for_slot(&state, &records[0], STEP).await.unwrap();
        let _second = wait_for_slot(&state, &records[1], STEP).await.unwrap();

        let third = wait_for_slot(&state, &records[2], STEP);
        tokio::pin!(third);
        assert!(tokio::time::timeout(STEP * 2, &mut third).await.is_err());
        // Canceling is not finished, so this takes the dedicated branch.
        store::update_status(
            records[2].id,
            InstallJobStatus::Canceling,
            &records[2].state,
            &state,
        )
        .await
        .unwrap();
        let left = tokio::time::timeout(Duration::from_secs(5), third)
            .await
            .expect("a canceling job stops waiting at its next step")
            .unwrap();
        assert!(left.is_none());
        assert_eq!(
            store::get_required(records[2].id, &state)
                .await
                .unwrap()
                .status,
            InstallJobStatus::Canceling
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn queued_jobs_get_freed_slots_in_arrival_order() {
        let (_temp, state) = fixture().await;
        let mut records = Vec::new();
        for instance in ["first", "second", "third", "fourth"] {
            let job = enqueue(request(instance), &state).await.unwrap();
            records
                .push(store::get_required(job.job_id, &state).await.unwrap());
        }
        let first = wait_for_slot(&state, &records[0], STEP).await.unwrap();
        let second = wait_for_slot(&state, &records[1], STEP).await.unwrap();
        assert!(first.is_some() && second.is_some());

        let third = wait_for_slot(&state, &records[2], STEP);
        tokio::pin!(third);
        assert!(tokio::time::timeout(STEP * 2, &mut third).await.is_err());
        let fourth = wait_for_slot(&state, &records[3], STEP);
        tokio::pin!(fourth);
        // Both wait through several steps together, the later one in line
        // polled first each time.
        for _ in 0..3 {
            assert!(tokio::time::timeout(STEP, &mut fourth).await.is_err());
            assert!(tokio::time::timeout(STEP, &mut third).await.is_err());
        }

        drop(first);
        assert!(
            tokio::time::timeout(STEP * 4, &mut fourth).await.is_err(),
            "the later waiter must not take the first freed slot"
        );
        let third = tokio::time::timeout(Duration::from_secs(5), third)
            .await
            .expect("the earlier waiter gets the first freed slot")
            .unwrap();
        assert!(third.is_some());

        drop(second);
        let fourth = tokio::time::timeout(Duration::from_secs(5), fourth)
            .await
            .expect("the later waiter gets the next freed slot")
            .unwrap();
        assert!(fourth.is_some());
        state.pool.close().await;
    }
}
