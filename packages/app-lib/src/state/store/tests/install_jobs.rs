//! What the install jobs collection promises.

use super::support;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::content_update_jobs::{
    ContentUpdateJobRecord, ContentUpdateJobs,
};
use crate::state::store::install_jobs::{
    InstallJobRecord, InstallJobScope, InstallJobs,
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
