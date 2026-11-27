//! What the install jobs collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::content_update_jobs::{
    ContentUpdateJobRecord, ContentUpdateJobs,
};
use crate::state::store::install_jobs::{
    InstallJobBlocker, InstallJobRecord, InstallJobScope, InstallJobs,
    JobFinish,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn job(status: &str) -> InstallJobRecord {
    let mut state = crate::install::model::InstallJobState::new(
        crate::install::model::InstallRequest::InstallExistingInstance {
            instance_id: "instance".into(),
            force: false,
        },
    );
    state.events.clear();
    InstallJobRecord {
        instance_id: None,
        kind: "install_existing_instance".to_string(),
        status: status.to_string(),
        state: serde_json::to_string(&state).unwrap(),
        created: 1,
        modified: 2,
        finished: None,
        dismissed: false,
    }
}

#[tokio::test]
async fn a_job_round_trips_and_clears() {
    let handle = support::handle().await;
    let id = "job".to_string();

    InstallJobs::set(&handle, &id, Some(&job("running")))
        .await
        .unwrap();
    assert_eq!(
        InstallJobs::get(&handle, &id).await.unwrap(),
        Some(job("running"))
    );

    InstallJobs::set(&handle, &id, None).await.unwrap();
    assert_eq!(InstallJobs::get(&handle, &id).await.unwrap(), None);
}

#[tokio::test]
async fn a_status_reads_only_the_jobs_it_names() {
    let handle = support::handle().await;
    for (id, status) in [
        ("queued", "queued"),
        ("running", "running"),
        ("finished", "succeeded"),
    ] {
        InstallJobs::set(&handle, &id.to_string(), Some(&job(status)))
            .await
            .unwrap();
    }

    let running = InstallJobs::get_scoped(
        &handle,
        &InstallJobScope::Status("running".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(running.len(), 1);
    assert_eq!(running[0].0, "running");
    assert_eq!(running[0].1.status, "running");
}

#[tokio::test]
async fn a_replacement_writes_exactly_what_it_was_given() {
    let handle = support::handle().await;
    InstallJobs::set(&handle, &"kept".to_string(), Some(&job("running")))
        .await
        .unwrap();
    InstallJobs::set(&handle, &"dropped".to_string(), Some(&job("running")))
        .await
        .unwrap();

    InstallJobs::set_all(&handle, &[("kept".to_string(), job("queued"))])
        .await
        .unwrap();

    assert_eq!(
        InstallJobs::get(&handle, &"dropped".to_string())
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        InstallJobs::get(&handle, &"kept".to_string())
            .await
            .unwrap(),
        Some(job("queued"))
    );
}

#[tokio::test]
async fn a_job_that_names_an_instance_round_trips() {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    let handle = handle_of(&pool);
    let mut record = job("queued");
    record.instance_id = Some("instance".to_string());

    InstallJobs::set(&handle, &"job".to_string(), Some(&record))
        .await
        .unwrap();

    assert_eq!(
        InstallJobs::get(&handle, &"job".to_string()).await.unwrap(),
        Some(record)
    );
}

#[tokio::test]
async fn a_plan_link_round_trips_and_clears() {
    let pool = support::pool().await;
    // The link names the job it was queued as through a foreign key, so the
    // job has to be there before the link is written.
    support::seed_install_job(&pool, "job", None).await;
    let handle = handle_of(&pool);
    ContentUpdateJobs::set(
        &handle,
        &"plan".to_string(),
        Some(&ContentUpdateJobRecord {
            job_id: Some("job".to_string()),
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        ContentUpdateJobs::get(&handle, &"plan".to_string())
            .await
            .unwrap(),
        Some(ContentUpdateJobRecord {
            job_id: Some("job".to_string())
        })
    );

    ContentUpdateJobs::set(&handle, &"plan".to_string(), None)
        .await
        .unwrap();
    assert_eq!(
        ContentUpdateJobs::get(&handle, &"plan".to_string())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_scoped_replacement_refuses_a_job_of_another_scope() {
    let handle = support::handle().await;

    let refused = InstallJobs::set_scoped(
        &handle,
        &InstallJobScope::Status("running".to_string()),
        &[("job".to_string(), job("queued"))],
    )
    .await;

    assert!(refused.is_err());
    assert_eq!(
        InstallJobs::get(&handle, &"job".to_string()).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn an_instance_scope_reads_the_jobs_of_that_instance() {
    let pool = support::pool().await;
    support::add_instance(&pool, "one").await;
    support::add_instance(&pool, "two").await;
    let handle = handle_of(&pool);
    for (id, instance_id) in [("first", "one"), ("second", "two")] {
        let mut record = job("queued");
        record.instance_id = Some(instance_id.to_string());
        InstallJobs::set(&handle, &id.to_string(), Some(&record))
            .await
            .unwrap();
    }

    let jobs = InstallJobs::get_scoped(
        &handle,
        &InstallJobScope::Instance("one".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].0, "first");
}

#[tokio::test]
async fn a_job_is_opened_once_and_a_second_opening_is_refused() {
    let handle = support::handle().await;
    let id = "job".to_string();
    let mut connection = handle.pool().acquire().await.unwrap();

    InstallJobs::insert_in(&mut *connection, &id, &job("queued"))
        .await
        .unwrap();
    let refused =
        InstallJobs::insert_in(&mut *connection, &id, &job("running")).await;
    assert!(refused.is_err());
    drop(connection);

    assert_eq!(
        InstallJobs::get(&handle, &id).await.unwrap(),
        Some(job("queued"))
    );
}

#[tokio::test]
async fn finishing_a_job_lands_only_while_it_is_still_running() {
    let handle = support::handle().await;
    let id = "job".to_string();
    let mut connection = handle.pool().acquire().await.unwrap();

    InstallJobs::insert_in(&mut *connection, &id, &job("running"))
        .await
        .unwrap();
    let reached =
        InstallJobs::finish_in(&mut *connection, &finish(&id, "succeeded"))
            .await
            .unwrap();
    assert_eq!(reached, 1);
    drop(connection);

    let stored = InstallJobs::get(&handle, &id).await.unwrap().unwrap();

    assert_eq!(stored.status, "succeeded");
    assert_eq!(stored.finished, Some(30));
}

#[tokio::test]
async fn a_job_that_moved_on_is_not_finished_again() {
    let handle = support::handle().await;
    let id = "job".to_string();
    let mut connection = handle.pool().acquire().await.unwrap();

    InstallJobs::insert_in(&mut *connection, &id, &job("succeeded"))
        .await
        .unwrap();
    let reached =
        InstallJobs::finish_in(&mut *connection, &finish(&id, "failed"))
            .await
            .unwrap();
    assert_eq!(reached, 0);
    drop(connection);

    let stored = InstallJobs::get(&handle, &id).await.unwrap().unwrap();

    assert_eq!(stored.status, "succeeded");
}

#[tokio::test]
async fn the_guards_see_the_jobs_that_still_occupy_an_instance() {
    let pool = support::pool().await;
    support::add_instance(&pool, "one").await;
    let handle = handle_of(&pool);
    let mut connection = handle.pool().acquire().await.unwrap();

    let mut running = job("running");
    running.instance_id = Some("one".to_string());
    InstallJobs::insert_in(&mut *connection, &"running".to_string(), &running)
        .await
        .unwrap();
    let mut finished = job("succeeded");
    finished.instance_id = Some("one".to_string());
    InstallJobs::insert_in(
        &mut *connection,
        &"finished".to_string(),
        &finished,
    )
    .await
    .unwrap();

    assert!(
        InstallJobs::unfinished_in(&mut *connection, "one", &["running"])
            .await
            .unwrap()
    );
    assert_eq!(
        InstallJobs::blocker_in(&mut *connection, "one", &["running"])
            .await
            .unwrap(),
        Some(InstallJobBlocker::Active {
            id: "running".into(),
            status: "running".into()
        }),
    );
    assert!(
        !InstallJobs::unfinished_in(&mut *connection, "one", &["queued"])
            .await
            .unwrap()
    );
    // Neither job is queued and neither holds a rollback, so nothing occupies
    // the runtime under those terms. The job that does is added just below.
    assert!(
        !InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap()
    );

    // A job that is only waiting its turn is one of the jobs that occupy the
    // instance: the guard above says nothing is waiting, so this is what makes
    // it answer true.
    let mut queued = job("queued");
    queued.instance_id = Some("one".to_string());
    InstallJobs::insert_in(&mut *connection, &"queued".to_string(), &queued)
        .await
        .unwrap();
    assert!(
        InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap(),
        "a queued job still holds the runtime it was queued for"
    );
    assert!(
        InstallJobs::unfinished_in(&mut *connection, "one", &["queued"])
            .await
            .unwrap(),
        "and the instance it names is the one that waits on it"
    );

    InstallJobs::write(&mut *connection, &"queued".to_string(), None)
        .await
        .unwrap();
    assert!(
        !InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap(),
        "removing it leaves the guard where it stood"
    );
}

#[tokio::test]
async fn a_finished_job_that_holds_rollback_content_still_occupies_the_runtime()
{
    let handle = support::handle().await;
    let mut connection = handle.pool().acquire().await.unwrap();
    let mut with_rollback = job("succeeded");
    with_rollback.state =
        "{\"rollback\":{\"after\":true,\"content\":{\"staging_id\":\"s\"}}}"
            .to_string();

    InstallJobs::insert_in(
        &mut *connection,
        &"job".to_string(),
        &with_rollback,
    )
    .await
    .unwrap();

    assert!(
        InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap()
    );
    assert!(
        !InstallJobs::unfinished_in(&mut *connection, "instance", &["queued"])
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_completed_rollback_no_longer_occupies_the_runtime() {
    let handle = support::handle().await;
    let mut connection = handle.pool().acquire().await.unwrap();
    // A failed or interrupted job whose rollback finished discards the
    // content snapshot but keeps the restored instance metadata.
    for (key, status) in [("failed", "failed"), ("interrupted", "interrupted")]
    {
        let mut rolled_back = job(status);
        rolled_back.state = "{\"rollback\":{\"install_stage\":\"installed\"},\"rollback_error\":null}".to_string();
        InstallJobs::insert_in(
            &mut *connection,
            &key.to_string(),
            &rolled_back,
        )
        .await
        .unwrap();
    }

    assert!(
        !InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap(),
        "a rollback that has already been restored pins nothing"
    );

    let mut restore_failed = job("failed");
    restore_failed.state = "{\"rollback\":{\"install_stage\":\"installed\"},\"rollback_error\":{\"code\":\"rollback_error\"}}".to_string();
    InstallJobs::insert_in(
        &mut *connection,
        &"restore_failed".to_string(),
        &restore_failed,
    )
    .await
    .unwrap();
    assert!(
        InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap(),
        "a rollback whose restore failed is still pending recovery"
    );
}

fn finish<'a>(key: &'a str, status: &'a str) -> JobFinish<'a> {
    JobFinish {
        key,
        instance_id: None,
        status,
        state: "{\"progress\":{}}",
        modified: 20,
        finished: 30,
        running_status: "running",
        canceling_status: "canceling",
        accept_canceling: false,
    }
}

async fn detached_fixture() -> (tempfile::TempDir, Handle) {
    let root = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.path().join("system.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    crate::state::store::recovery_documents::install(
        &mut *pool.acquire().await.unwrap(),
    )
    .await
    .unwrap();
    (root, Handle::new(pool))
}

async fn publish_detached_job(
    handle: &Handle,
    root: &std::path::Path,
    key: &str,
    revision: i64,
    row: &InstallJobRecord,
) {
    let path = root.join(format!("{key}-{revision}.json"));
    let hash = crate::state::recovery::record::write(
        &path,
        "install_jobs",
        key,
        None,
        revision,
        serde_json::to_value(row).unwrap(),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO operation_publications(kind,id,revision,object_path,sha256) VALUES('install_jobs',?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET revision=excluded.revision, object_path=excluded.object_path, sha256=excluded.sha256, deleted=0")
        .bind(key).bind(revision).bind(path.to_string_lossy().into_owned()).bind(hash)
        .execute(handle.pool()).await.unwrap();
}

#[tokio::test]
async fn slow_job_payload_releases_the_only_connection_and_core_writer() {
    use crate::state::store::recovery_documents::snapshot_pause;
    use std::sync::Arc;
    let (root, handle) = detached_fixture().await;
    publish_detached_job(&handle, root.path(), "running", 1, &job("running"))
        .await;
    let reached = Arc::new(tokio::sync::Notify::new());
    let resume = Arc::new(tokio::sync::Notify::new());
    let reading =
        snapshot_pause::PAUSE.scope((reached.clone(), resume.clone()), async {
            InstallJobs::snapshot(
                &handle,
                None,
                Some(&["queued", "running", "failed"]),
                false,
            )
            .await
            .unwrap()
        });
    let writing = async {
        reached.notified().await;
        let transaction =
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                let transaction =
                    handle.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
                transaction.commit().await.unwrap();
            })
            .await;
        resume.notify_one();
        transaction.expect("payload IO must hold neither the sole pooled connection nor a core transaction");
    };
    let (rows, ()) = tokio::join!(reading, writing);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.status, "running");
}

#[tokio::test]
async fn selected_jobs_keep_their_revision_and_opacity_after_update_and_tombstone()
 {
    use crate::state::store::recovery_documents::select_records;
    let (root, handle) = detached_fixture().await;
    publish_detached_job(&handle, root.path(), "one", 1, &job("running")).await;
    let selected = select_records(
        &mut *handle.pool().acquire().await.unwrap(),
        "install_jobs",
        None,
        None,
    )
    .await
    .unwrap();
    publish_detached_job(&handle, root.path(), "one", 2, &job("succeeded"))
        .await;
    sqlx::query("UPDATE operation_publications SET deleted=1; INSERT INTO operation_opaque(kind) VALUES('install_jobs')")
        .execute(handle.pool()).await.unwrap();
    assert_eq!(
        selected[0]
            .decode::<InstallJobRecord>()
            .await
            .unwrap()
            .status,
        "running"
    );
    sqlx::query("UPDATE operation_publications SET deleted=0")
        .execute(handle.pool())
        .await
        .unwrap();
    let opaque = select_records(
        &mut *handle.pool().acquire().await.unwrap(),
        "install_jobs",
        None,
        None,
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM operation_opaque")
        .execute(handle.pool())
        .await
        .unwrap();
    assert!(opaque[0].decode::<InstallJobRecord>().await.is_err());
    let current = InstallJobs::snapshot(&handle, Some("one"), None, true)
        .await
        .unwrap();
    assert_eq!(current[0].1.status, "succeeded");
}

#[tokio::test]
async fn detached_browsing_skips_future_or_corrupt_jobs_but_cleanup_fails_closed()
 {
    let (root, handle) = detached_fixture().await;
    publish_detached_job(&handle, root.path(), "good", 1, &job("running"))
        .await;
    publish_detached_job(
        &handle,
        root.path(),
        "future",
        1,
        &job("future-status"),
    )
    .await;
    publish_detached_job(&handle, root.path(), "broken", 1, &job("queued"))
        .await;
    let path = root.path().join("broken-1.json");
    tokio::fs::write(&path, b"broken").await.unwrap();
    tokio::fs::write(format!("{}.previous", path.display()), b"broken")
        .await
        .unwrap();
    let rows = InstallJobs::snapshot(&handle, None, None, false)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "good");
    assert!(
        InstallJobs::snapshot(&handle, Some("future"), None, true)
            .await
            .is_err()
    );
    assert!(
        InstallJobs::snapshot(&handle, Some("broken"), None, true)
            .await
            .is_err()
    );
    assert!(
        InstallJobs::snapshot_unfinished_or_rollback(&handle, &["queued"])
            .await
            .is_err()
    );
    assert!(path.exists(), "unreadable authorities must be retained");
}
