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
async fn retry_checkpoint_reuses_only_the_original_incomplete_pack_instance() {
    let (temp, state) = fixture().await;
    let pack_path = temp.path().join("pack.mrpack");
    fn write_pack(path: &std::path::Path, content: &[u8]) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        zip.start_file(
            "modrinth.index.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(content).unwrap();
        zip.finish().unwrap();
    }
    write_pack(&pack_path, b"original");
    crate::state::instances::commands::set_instance_install_stage(
        "first",
        InstanceInstallStage::PackInstalling,
        &state.pool,
    )
    .await
    .unwrap();
    let mut job = InstallJobState::new(InstallRequest::CreateModpackInstance {
        location: CreatePackLocation::FromFile {
            path: pack_path.clone(),
        },
        post_install_edit: None,
    });
    job.target = InstallTarget::NewInstance {
        instance_id: Some("first".into()),
    };
    job.cleanup = InstallCleanup::DeleteNewInstance {
        instance_id: Some("first".into()),
    };
    job.local_pack_retry_identity = Some(
        crate::api::pack::install_from::local_pack_retry_identity(&pack_path)
            .await
            .unwrap(),
    );
    assert!(lifecycle::retain_pack_download(
        &mut job,
        &ErrorKind::NetworkError("offline".into()).into()
    ));
    assert!(prepare_pack_retry(&mut job, &state).await.unwrap());
    assert_eq!(current_instance_id(&job).as_deref(), Some("first"));
    let busy_id = Uuid::new_v4();
    store::insert(
        busy_id,
        &InstallJobState::new(request("first", "other")),
        InstallJobStatus::Running,
        &state,
    )
    .await
    .unwrap();
    assert!(
        existing_for_instance(&state, "first", &job.request, None)
            .await
            .is_err()
    );
    let busy = store::get_required(busy_id, &state).await.unwrap();
    store::update_status(
        busy_id,
        InstallJobStatus::Failed,
        &busy.state,
        &state,
    )
    .await
    .unwrap();
    let id = Uuid::new_v4();
    store::insert(id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    crate::install::recovery::recover_interrupted_jobs(&state)
        .await
        .unwrap();
    let restored = store::get_required(id, &state).await.unwrap();
    assert_eq!(restored.status, InstallJobStatus::Failed);
    assert!(restored.state.retry_from_checkpoint);
    assert!(
        crate::state::get_instance("first", &state.pool)
            .await
            .unwrap()
            .is_some()
    );
    write_pack(&pack_path, b"replaced");
    assert!(
        prepare_pack_retry(&mut job, &state)
            .await
            .unwrap_err()
            .to_string()
            .contains("archive changed")
    );
    crate::state::instances::commands::set_instance_install_stage(
        "first",
        InstanceInstallStage::Installed,
        &state.pool,
    )
    .await
    .unwrap();
    assert!(prepare_pack_retry(&mut job, &state).await.is_err());
    job.target = InstallTarget::NewInstance {
        instance_id: Some("removed".into()),
    };
    assert!(!prepare_pack_retry(&mut job, &state).await.unwrap());
    assert!(!job.retry_from_checkpoint);
    state.pool.close().await;
}

#[tokio::test]
async fn cancellation_during_failed_cleanup_wins_and_checkpoint_is_durable_before_cleanup()
 {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let mut job = InstallJobState::new(request("first", "mod"));
    job.retry_from_checkpoint = true;
    store::insert(id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    lifecycle::begin_failed_job_rollback(
        &mut job,
        &ErrorKind::InputError("invalid archive".into()).into(),
    );
    store::checkpoint_running_state(id, &job, &state)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !store::get_required(id, &state)
            .await
            .unwrap()
            .state
            .retry_from_checkpoint
    );
    store::update_status_if(
        id,
        InstallJobStatus::Running,
        InstallJobStatus::Canceling,
        &job,
        &state,
    )
    .await
    .unwrap()
    .unwrap();
    lifecycle::persist_failed_cleanup(id, &job, &state)
        .await
        .unwrap();
    let canceled = store::get_required(id, &state).await.unwrap();
    assert_eq!(canceled.status, InstallJobStatus::Canceled);
    assert_eq!(canceled.state.error.unwrap().code, "canceled");
    lifecycle::persist_failed_cleanup(id, &job, &state)
        .await
        .unwrap();
    assert_eq!(
        store::get_required(id, &state).await.unwrap().status,
        InstallJobStatus::Canceled
    );
    state.pool.close().await;
}

#[tokio::test]
async fn stopped_worker_recovery_waits_for_execution_and_preserves_terminal_results()
 {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let job = InstallJobState::new(request("first", "mod"));
    let record = store::insert(id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    let owner = execution_lock(&state, &record).await;
    let mut recovery = tokio::spawn({
        let state = state.clone();
        async move {
            lifecycle::recover_stopped_job(
                id,
                &state,
                None,
                &ErrorKind::OtherError("worker exited".into()).into(),
            )
            .await
        }
    });
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(30),
            &mut recovery
        )
        .await
        .is_err()
    );
    assert_eq!(
        store::get_required(id, &state).await.unwrap().status,
        InstallJobStatus::Running
    );
    drop(owner);
    recovery.await.unwrap().unwrap();
    let failed = store::get_required(id, &state).await.unwrap();
    assert_eq!(failed.status, InstallJobStatus::Failed);
    assert!(
        failed
            .state
            .error
            .unwrap()
            .message
            .contains("worker exited")
    );
    let terminal = Uuid::new_v4();
    store::insert(terminal, &job, InstallJobStatus::Succeeded, &state)
        .await
        .unwrap();
    lifecycle::recover_stopped_job(
        terminal,
        &state,
        None,
        &ErrorKind::OtherError("late failure".into()).into(),
    )
    .await
    .unwrap();
    assert_eq!(
        store::get_required(terminal, &state).await.unwrap().status,
        InstallJobStatus::Succeeded
    );
    state.pool.close().await;
}

#[tokio::test]
async fn cancellation_without_a_token_waits_for_owner_then_cleans_up_once() {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let job = InstallJobState::new(request("first", "mod"));
    let record = store::insert(id, &job, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    let owner = execution_lock(&state, &record).await;
    assert_eq!(
        cancel_job_with_state(id, state.clone())
            .await
            .unwrap()
            .status,
        InstallJobStatus::Canceling
    );
    assert_eq!(
        cancel_job_with_state(id, state.clone())
            .await
            .unwrap()
            .status,
        InstallJobStatus::Canceling
    );
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(
        store::get_required(id, &state).await.unwrap().status,
        InstallJobStatus::Canceling
    );
    drop(owner);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let current = store::get_required(id, &state).await.unwrap();
            if current.status == InstallJobStatus::Canceled {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let current = store::get_required(id, &state).await.unwrap();
    assert_eq!(
        current
            .state
            .events
            .iter()
            .filter(|event| matches!(
                event.kind,
                InstallJobEventKind::RollbackStarted { .. }
            ))
            .count(),
        1
    );
    state.pool.close().await;
}

#[tokio::test]
async fn repeated_cancellation_recovers_an_orphaned_canceling_job() {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let original = crate::state::get_instance("first", &state.pool)
        .await
        .unwrap()
        .unwrap();
    let mut job = InstallJobState::new(request("first", "mod"));
    job.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: "first".into(),
    };
    job.rollback = Some(InstallRollbackState {
        instance: original.clone(),
        install_stage: InstanceInstallStage::Installed,
        content: None,
    });
    begin_canceling_job(&mut job);
    let mut partial = original;
    partial.instance.name = "incomplete installation".into();
    crate::state::restore_instance_metadata(&partial, &state.pool)
        .await
        .unwrap();
    store::insert(id, &job, InstallJobStatus::Canceling, &state)
        .await
        .unwrap();
    assert!(!state.install_job_cancellations.contains_key(&id));
    assert_eq!(
        cancel_job_with_state(id, state.clone())
            .await
            .unwrap()
            .status,
        InstallJobStatus::Canceling
    );
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if store::get_required(id, &state).await.unwrap().status
                == InstallJobStatus::Canceled
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        crate::state::get_instance("first", &state.pool)
            .await
            .unwrap()
            .unwrap()
            .instance
            .name,
        "first"
    );
    state.pool.close().await;
}

#[tokio::test]
async fn cancel_cleanup_rechecks_terminal_state_after_the_execution_owner_finishes()
 {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let original = crate::state::get_instance("first", &state.pool)
        .await
        .unwrap()
        .unwrap();
    let mut job = InstallJobState::new(request("first", "mod"));
    job.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: "first".into(),
    };
    job.rollback = Some(InstallRollbackState {
        instance: original.clone(),
        install_stage: InstanceInstallStage::Installed,
        content: None,
    });
    begin_canceling_job(&mut job);
    let record = store::insert(id, &job, InstallJobStatus::Canceling, &state)
        .await
        .unwrap();
    let owner = execution_lock(&state, &record).await;
    let cleanup = finish_canceling_job(id, &state);
    tokio::pin!(cleanup);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(30),
            cleanup.as_mut()
        )
        .await
        .is_err()
    );
    store::update_status(id, InstallJobStatus::Canceled, &job, &state)
        .await
        .unwrap();
    let mut newer = original;
    newer.instance.name = "newer operation".into();
    crate::state::restore_instance_metadata(&newer, &state.pool)
        .await
        .unwrap();
    drop(owner);
    tokio::time::timeout(std::time::Duration::from_secs(3), cleanup)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::state::get_instance("first", &state.pool)
            .await
            .unwrap()
            .unwrap()
            .instance
            .name,
        "newer operation"
    );
    assert_eq!(
        store::get_required(id, &state).await.unwrap().status,
        InstallJobStatus::Canceled
    );
    state.pool.close().await;
}

#[tokio::test]
async fn canceling_terminal_jobs_returns_the_authoritative_status_without_reopening_them()
 {
    let (_temp, state) = fixture().await;
    for status in [
        InstallJobStatus::Succeeded,
        InstallJobStatus::Failed,
        InstallJobStatus::Interrupted,
        InstallJobStatus::Canceled,
    ] {
        let id = Uuid::new_v4();
        let job = InstallJobState::new(request("first", "mod"));
        store::insert(id, &job, status, &state).await.unwrap();
        assert_eq!(
            cancel_job_with_state(id, state.clone())
                .await
                .unwrap()
                .status,
            status
        );
        assert_eq!(
            store::get_required(id, &state).await.unwrap().status,
            status
        );
    }
    state.pool.close().await;
}

#[tokio::test]
async fn queued_and_waiting_cancellation_returns_before_background_cleanup_finishes()
 {
    let (_temp, state) = fixture().await;
    for status in [InstallJobStatus::Queued, InstallJobStatus::WaitingForUser] {
        let id = Uuid::new_v4();
        let job = InstallJobState::new(request("first", "mod"));
        store::insert(id, &job, status, &state).await.unwrap();
        let accepted = cancel_job_with_state(id, state.clone()).await.unwrap();
        assert_eq!(accepted.status, InstallJobStatus::Canceling);
        let writer = state.install_db_semaphore.acquire().await.unwrap();
        tokio::task::yield_now().await;
        let repeated = cancel_job_with_state(id, state.clone()).await.unwrap();
        assert_eq!(repeated.status, InstallJobStatus::Canceling);
        drop(writer);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if store::get_required(id, &state).await.unwrap().status
                    == InstallJobStatus::Canceled
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    state.pool.close().await;
}

#[tokio::test]
async fn background_cancel_cleanup_preserves_rollback_failure_details() {
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let mut job = InstallJobState::new(request("first", "mod"));
    job.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: "first".into(),
    };
    job.rollback = Some(InstallRollbackState {
        instance: crate::state::get_instance("second", &state.pool)
            .await
            .unwrap()
            .unwrap(),
        install_stage: InstanceInstallStage::Installed,
        content: None,
    });
    store::insert(id, &job, InstallJobStatus::WaitingForUser, &state)
        .await
        .unwrap();
    assert_eq!(
        cancel_job_with_state(id, state.clone())
            .await
            .unwrap()
            .status,
        InstallJobStatus::Canceling
    );
    let terminal =
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let record = store::get_required(id, &state).await.unwrap();
                if record.status == InstallJobStatus::Canceled {
                    break record;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    assert!(terminal.state.rollback_error.is_some());
    assert!(terminal.state.rollback.is_some());
    assert!(terminal.state.events.iter().any(|event| matches!(
        event.kind,
        InstallJobEventKind::RollbackFailed { .. }
    )));
    state.pool.close().await;
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

#[tokio::test]
async fn delayed_old_worker_checkpoints_cannot_overwrite_a_retried_generation()
{
    let (_temp, state) = fixture().await;
    let id = Uuid::new_v4();
    let old = InstallJobState::new(request("first", "mod"));
    store::insert(id, &old, InstallJobStatus::Running, &state)
        .await
        .unwrap();
    let mut retried = old.clone();
    retried.record_event(InstallJobEventKind::JobQueued {
        kind: retried.request.kind(),
    });
    retried.events.last_mut().unwrap().at =
        old.events[0].at + chrono::TimeDelta::seconds(1);
    retried.retry_from_checkpoint = true;
    store::update_status(id, InstallJobStatus::Running, &retried, &state)
        .await
        .unwrap();
    assert!(
        store::checkpoint_running_state(id, &old, &state)
            .await
            .unwrap()
            .is_none()
    );
    let returned = store::update_state(id, &old, &state).await.unwrap();
    assert!(returned.state.retry_from_checkpoint);
    assert!(
        store::get_required(id, &state)
            .await
            .unwrap()
            .state
            .retry_from_checkpoint
    );
    state.pool.close().await;
}
