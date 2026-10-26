use super::*;
use std::sync::Arc;

async fn fixture() -> (tempfile::TempDir, Arc<State>) {
    let temp = tempfile::tempdir().unwrap();
    let directories = crate::state::DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "admission-test".into(),
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

fn request(instance: &str, content: &str) -> InstallRequest {
    InstallRequest::ChangeContent {
        instance_id: instance.into(),
        intent: crate::install::ContentChangeIntent::UpdateOne {
            content_id: content.into(),
            target_release_id: Some("version".into()),
        },
        display_title: "Change content".into(),
        display_icon: None,
    }
}

#[tokio::test]
async fn interactive_mutations_share_admission_and_reject_active_tasks() {
    let (_temp, state) = fixture().await;
    let guard = interactive_content_guard(&state, "first").await.unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(25),
            enqueue(request("first", "mod"), &state)
        )
        .await
        .is_err()
    );
    drop(guard);
    let job = enqueue(request("first", "mod"), &state).await.unwrap();
    assert!(interactive_content_guard(&state, "first").await.is_err());
    assert!(interactive_content_guard(&state, "second").await.is_ok());
    EXECUTING_JOB
        .scope(job.job_id, async {
            assert!(
                interactive_content_guard(&state, "first")
                    .await
                    .unwrap()
                    .is_none()
            );
        })
        .await;
    state.pool.close().await;
}

#[tokio::test]
async fn concurrent_duplicate_requests_return_one_job_before_instance_mutation()
{
    let (_temp, state) = fixture().await;
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let state = state.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            enqueue(request("first", "mod"), &state).await.unwrap()
        }));
    }
    barrier.wait().await;
    let first = tasks.remove(0).await.unwrap();
    assert_eq!(tasks.remove(0).await.unwrap().job_id, first.job_id);
    assert_eq!(store::list(false, &state).await.unwrap().len(), 1);
    let different = enqueue(request("first", "other"), &state)
        .await
        .unwrap_err();
    assert!(different.to_string().contains("Another task"));
    // Pack initialization changes the instance stage. Admission must reject it first.
    let pack = InstallRequest::InstallPackToExistingInstance {
        instance_id: "first".into(),
        location: CreatePackLocation::FromVersionId {
            project_id: "pack".into(),
            version_id: "release".into(),
            title: "Pack".into(),
            icon_url: None,
        },
        post_install_edit: None,
    };
    assert!(enqueue(pack, &state).await.is_err());
    assert_eq!(crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_id("first", &state.pool).await.unwrap().unwrap().install_stage, InstanceInstallStage::Installed);
    assert!(enqueue(request("second", "mod"), &state).await.is_ok());
    state.pool.close().await;
}

#[tokio::test]
async fn two_instances_execute_while_same_instance_and_third_global_task_wait()
{
    let (_temp, state) = fixture().await;
    let first = enqueue(request("first", "mod"), &state).await.unwrap();
    let second = enqueue(request("second", "mod"), &state).await.unwrap();
    let third = enqueue(request("third", "mod"), &state).await.unwrap();
    let first_record = store::get_required(first.job_id, &state).await.unwrap();
    let second_record =
        store::get_required(second.job_id, &state).await.unwrap();
    let third_record = store::get_required(third.job_id, &state).await.unwrap();
    let first_slot = execution_slot(&state, &first_record).await.unwrap();
    let second_slot = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        execution_slot(&state, &second_record),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(state.install_job_semaphore.available_permits(), 0);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(25),
            execution_slot(&state, &first_record)
        )
        .await
        .is_err()
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(25),
            execution_slot(&state, &third_record)
        )
        .await
        .is_err()
    );
    assert_eq!(
        store::get_required(third.job_id, &state)
            .await
            .unwrap()
            .status,
        InstallJobStatus::Queued
    );
    store::update_status(
        third.job_id,
        InstallJobStatus::Canceled,
        &third_record.state,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        store::get_required(third.job_id, &state)
            .await
            .unwrap()
            .status,
        InstallJobStatus::Canceled
    );
    drop(first_slot);
    let replacement = execution_slot(&state, &first_record).await.unwrap();
    assert_eq!(state.install_job_semaphore.available_permits(), 0);
    drop(replacement);
    drop(second_slot);
    assert_eq!(state.install_job_semaphore.available_permits(), 2);
    state.pool.close().await;
}

#[tokio::test]
async fn waiting_jobs_reserve_their_instance_and_canceling_jobs_are_not_reused()
{
    let (_temp, state) = fixture().await;
    let queued = enqueue(request("first", "mod"), &state).await.unwrap();
    let record = store::get_required(queued.job_id, &state).await.unwrap();
    store::update_status(
        queued.job_id,
        InstallJobStatus::WaitingForUser,
        &record.state,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        enqueue(request("first", "mod"), &state)
            .await
            .unwrap()
            .job_id,
        queued.job_id
    );
    assert!(
        enqueue(request("first", "different"), &state)
            .await
            .is_err()
    );
    store::update_status(
        queued.job_id,
        InstallJobStatus::Canceling,
        &record.state,
        &state,
    )
    .await
    .unwrap();
    assert!(enqueue(request("first", "mod"), &state).await.is_err());
    store::update_status(
        queued.job_id,
        InstallJobStatus::Canceled,
        &record.state,
        &state,
    )
    .await
    .unwrap();
    assert_ne!(
        enqueue(request("first", "mod"), &state)
            .await
            .unwrap()
            .job_id,
        queued.job_id
    );
    state.pool.close().await;
}

#[tokio::test]
async fn accepted_plan_is_idempotent_after_completion_and_database_reopen() {
    let (temp, state) = fixture().await;
    let revision =
        crate::resources::plans::instance_revision(&state.pool, "first")
            .await
            .unwrap();
    let first = enqueue_with_content_plan(
        request("first", "mod"),
        Some(("plan", revision)),
        &state,
    )
    .await
    .unwrap();
    let duplicate = enqueue_with_content_plan(
        request("first", "mod"),
        Some(("plan", revision)),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(first.job_id, duplicate.job_id);
    let record = store::get_required(first.job_id, &state).await.unwrap();
    store::update_status(
        first.job_id,
        InstallJobStatus::Succeeded,
        &record.state,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        enqueue_with_content_plan(
            request("first", "mod"),
            Some(("plan", revision)),
            &state
        )
        .await
        .unwrap()
        .job_id,
        first.job_id
    );
    state.pool.close().await;
    drop(state);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("state.db"))
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let reopened = crate::state::test_state(
        crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "admission-test".into(),
        },
        pool,
    )
    .await
    .unwrap();
    assert_eq!(
        content_plan_job("plan", &reopened)
            .await
            .unwrap()
            .unwrap()
            .job_id,
        first.job_id
    );
    sqlx::query("DELETE FROM install_jobs WHERE id = ?")
        .bind(first.job_id.to_string())
        .execute(&reopened.pool)
        .await
        .unwrap();
    assert!(
        enqueue_with_content_plan(
            request("first", "mod"),
            Some(("plan", revision)),
            &reopened
        )
        .await
        .is_err()
    );
    assert!(store::list(false, &reopened).await.unwrap().is_empty());
    reopened.pool.close().await;
}

#[tokio::test]
async fn stale_review_and_failed_job_insert_leave_no_plan_receipt() {
    let (_temp, state) = fixture().await;
    let revision =
        crate::resources::plans::instance_revision(&state.pool, "first")
            .await
            .unwrap();
    assert!(
        enqueue_with_content_plan(
            request("first", "mod"),
            Some(("stale", revision + 1)),
            &state
        )
        .await
        .is_err()
    );
    sqlx::raw_sql("CREATE TRIGGER fail_job BEFORE INSERT ON install_jobs BEGIN SELECT RAISE(FAIL, 'injected job failure'); END;").execute(&state.pool).await.unwrap();
    assert!(
        enqueue_with_content_plan(
            request("first", "mod"),
            Some(("failed", revision)),
            &state
        )
        .await
        .is_err()
    );
    assert!(content_plan_job("failed", &state).await.unwrap().is_none());
    assert!(store::list(false, &state).await.unwrap().is_empty());
    sqlx::raw_sql("DROP TRIGGER fail_job")
        .execute(&state.pool)
        .await
        .unwrap();
    enqueue_with_content_plan(
        request("first", "mod"),
        Some(("failed", revision)),
        &state,
    )
    .await
    .unwrap();
    assert!(content_plan_job("failed", &state).await.unwrap().is_some());
    state.pool.close().await;
}
