//! What the install jobs collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::content_update_jobs::{
    ContentUpdateJobRecord, ContentUpdateJobs,
};
use crate::state::store::install_jobs::{
    InstallJobRecord, InstallJobScope, InstallJobs, JobFinish,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

fn job(status: &str) -> InstallJobRecord {
    InstallJobRecord {
        instance_id: None,
        kind: "pack".to_string(),
        status: status.to_string(),
        state: "{\"progress\":{}}".to_string(),
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
    let handle = support::handle().await;
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
    assert!(
        !InstallJobs::unfinished_in(&mut *connection, "one", &["queued"])
            .await
            .unwrap()
    );
    assert!(
        InstallJobs::unfinished_or_rollback(&mut *connection, &["queued"])
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_finished_job_that_holds_a_rollback_still_occupies_the_runtime() {
    let handle = support::handle().await;
    let mut connection = handle.pool().acquire().await.unwrap();
    let mut with_rollback = job("succeeded");
    with_rollback.state = "{\"rollback\":{\"after\":true}}".to_string();

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
