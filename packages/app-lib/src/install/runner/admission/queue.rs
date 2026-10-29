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

/// When a job waiting in line gives up its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeaveWhen {
    /// Admission: the job finished or started canceling.
    FinishedOrCanceling,
    /// Settling after a runner failure: only once the job finished, since
    /// finishing a cancellation needs the slot as much as an interruption.
    Finished,
}

impl LeaveWhen {
    fn leaves(self, status: InstallJobStatus) -> bool {
        status.is_finished()
            || (self == Self::FinishedOrCanceling
                && status == InstallJobStatus::Canceling)
    }
}

pub(super) async fn wait_for_slot<'a>(
    state: &'a State,
    job: &store::InstallJobRecord,
    step: Duration,
) -> crate::Result<Option<Slot<'a>>> {
    wait_for_slot_until(state, job, step, LeaveWhen::FinishedOrCanceling).await
}

pub(super) async fn wait_for_slot_until<'a>(
    state: &'a State,
    job: &store::InstallJobRecord,
    step: Duration,
    leave: LeaveWhen,
) -> crate::Result<Option<Slot<'a>>> {
    let instance = job
        .instance_id
        .clone()
        .or_else(|| request_instance(&job.state.request));
    let execution = match instance {
        Some(instance) => {
            let lock = state
                .install_execution_locks
                .entry(instance)
                .or_default()
                .clone();
            let Some(guard) =
                wait_in_line(state, job.id, step, leave, lock.lock_owned())
                    .await?
            else {
                return Ok(None);
            };
            Some(guard)
        }
        None => None,
    };
    let Some(global) = wait_in_line(
        state,
        job.id,
        step,
        leave,
        state.install_job_semaphore.acquire(),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some((execution, global?)))
}

/// Awaits `acquire` without dropping it. Every `step` the job is read again;
/// once `leave` says it gives up its place the wait ends with `None`.
async fn wait_in_line<T>(
    state: &State,
    job_id: Uuid,
    step: Duration,
    leave: LeaveWhen,
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
        if leave.leaves(status) {
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
        for id in ["first", "second", "third"] {
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
    async fn a_canceling_job_keeps_its_place_when_settling() {
        let (_temp, state) = fixture().await;
        let mut records = Vec::new();
        for instance in ["first", "second", "third"] {
            let job = enqueue(request(instance), &state).await.unwrap();
            records
                .push(store::get_required(job.job_id, &state).await.unwrap());
        }
        let first = wait_for_slot(&state, &records[0], STEP).await.unwrap();
        let _second = wait_for_slot(&state, &records[1], STEP).await.unwrap();
        store::update_status(
            records[2].id,
            InstallJobStatus::Canceling,
            &records[2].state,
            &state,
        )
        .await
        .unwrap();

        let third =
            wait_for_slot_until(&state, &records[2], STEP, LeaveWhen::Finished);
        tokio::pin!(third);
        assert!(
            tokio::time::timeout(STEP * 3, &mut third).await.is_err(),
            "canceling alone does not give up a settlement slot"
        );
        drop(first);
        let slot = tokio::time::timeout(Duration::from_secs(5), third)
            .await
            .expect("the slot comes once another job releases it")
            .unwrap();
        assert!(slot.is_some());
        state.pool.close().await;
    }
}
