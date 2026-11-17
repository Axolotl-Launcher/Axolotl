//! What the resource operation records promise: a plan that remembers whether
//! it completed, the journal of an executing plan, and the cleanup plans that
//! remove old backups.

use super::support;
use crate::state::store::Handle;
use crate::state::store::resource_execution_journals::ResourceExecutionJournals;
use crate::state::store::resource_history_cleanup_plans::ResourceHistoryCleanupPlans;
use crate::state::store::resource_operation_plans::ResourceOperationPlans;

async fn handle() -> Handle {
    support::handle_with_instances(&["instance"]).await
}

async fn detached_handle() -> (tempfile::TempDir, Handle) {
    let directory = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(directory.path().join("system.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    super::super::recovery_documents::install(
        &mut *pool.acquire().await.unwrap(),
    )
    .await
    .unwrap();
    (directory, Handle::new(pool))
}

async fn publish_detached(
    root: &std::path::Path,
    handle: &Handle,
    kind: &str,
    revision: i64,
    value: serde_json::Value,
) {
    let path = root.join(format!("{kind}-{revision}.json"));
    let hash = crate::state::recovery::record::write(
        &path,
        kind,
        "plan",
        Some("instance"),
        revision,
        value,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO operation_publications(kind,id,instance_id,revision,object_path,sha256) VALUES(?,'plan','instance',?,?,?) ON CONFLICT(kind,id) DO UPDATE SET revision=excluded.revision, object_path=excluded.object_path, sha256=excluded.sha256")
        .bind(kind).bind(revision).bind(path.to_string_lossy().into_owned()).bind(hash).execute(handle.pool()).await.unwrap();
}

fn detached_plan(completed: bool) -> serde_json::Value {
    serde_json::json!({"instance_id":"instance","revision":1,"payload":"{\"plan\":true}","job_id":null,"queued":false,"completed":completed})
}

#[tokio::test]
async fn resource_reader_releases_core_and_keeps_plan_journal_generation_together()
 {
    use crate::state::store::recovery_documents::snapshot_pause;
    use std::sync::Arc;
    let (directory, handle) = detached_handle().await;
    publish_detached(
        directory.path(),
        &handle,
        "resource_operation_plans",
        1,
        detached_plan(false),
    )
    .await;
    publish_detached(
        directory.path(),
        &handle,
        "resource_execution_journals",
        1,
        serde_json::json!({"payload":"{\"old\":true}"}),
    )
    .await;
    let reached = Arc::new(tokio::sync::Notify::new());
    let resume = Arc::new(tokio::sync::Notify::new());
    let reading =
        snapshot_pause::PAUSE.scope((reached.clone(), resume.clone()), async {
            ResourceExecutionJournals::pending_payload(&handle, "plan")
                .await
                .unwrap()
        });
    let writer = async {
        reached.notified().await;
        tokio::time::timeout(std::time::Duration::from_secs(2),async {
            let tx = handle.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
            tx.commit().await.unwrap();
            publish_detached(directory.path(), &handle, "resource_operation_plans", 2, detached_plan(true)).await;
            publish_detached(directory.path(), &handle, "resource_execution_journals", 2, serde_json::json!({"payload":"{\"new\":true}"})).await;
        }).await.expect("resource file reads must not hold core or the only pool connection");
        // The hook pauses at both the plan and journal; release both reads.
        resume.notify_one();
        reached.notified().await;
        resume.notify_one();
    };
    let (payload, ()) = tokio::join!(reading, writer);
    assert_eq!(payload.as_deref(), Some("{\"old\":true}"));
    assert!(
        ResourceExecutionJournals::pending_payload(&handle, "plan")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn detached_resource_readers_keep_revision_and_unknown_guards() {
    let (directory, handle) = detached_handle().await;
    publish_detached(
        directory.path(),
        &handle,
        "resource_operation_plans",
        1,
        detached_plan(false),
    )
    .await;
    assert_eq!(
        ResourceOperationPlans::completed_at(&handle, "plan", 2)
            .await
            .unwrap(),
        None
    );
    assert!(
        ResourceOperationPlans::payload_at(&handle, "plan", 2)
            .await
            .is_err()
    );
    assert!(
        ResourceOperationPlans::instance_at(&handle, "plan", 2)
            .await
            .is_err()
    );
    assert_eq!(
        ResourceOperationPlans::completed(&handle, "plan")
            .await
            .unwrap(),
        0
    );
    publish_detached(
        directory.path(),
        &handle,
        "resource_execution_journals",
        1,
        serde_json::json!({"payload":"{}"}),
    )
    .await;
    assert!(
        ResourceExecutionJournals::pending_for_instance(&handle, "instance")
            .await
            .unwrap()
    );
    sqlx::query("INSERT INTO operation_opaque(kind) VALUES('resource_execution_journals')").execute(handle.pool()).await.unwrap();
    assert!(
        ResourceExecutionJournals::pending_for_instance(&handle, "instance")
            .await
            .is_err()
    );
    assert!(
        ResourceExecutionJournals::pending_payload(&handle, "plan")
            .await
            .is_err()
    );
    assert!(
        ResourceExecutionJournals::for_plan(&handle, "plan")
            .await
            .is_err()
    );
}

/// The same database, with the job a plan is queued as.
///
/// A plan's `job_id` is a foreign key into `install_jobs`, so the row the queue
/// step names has to be written before the step runs.
async fn handle_with_install_job() -> Handle {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    support::seed_install_job(&pool, "job", None).await;
    Handle::new(pool)
}

#[tokio::test]
async fn a_plan_answers_the_completion_of_the_revision_it_stored() {
    let handle = handle().await;
    ResourceOperationPlans::insert_prepared(
        &handle,
        "plan",
        "instance",
        1,
        "{\"plan\":true}",
    )
    .await
    .unwrap();

    assert_eq!(
        ResourceOperationPlans::completed_at(&handle, "plan", 1)
            .await
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        ResourceOperationPlans::payload_at(&handle, "plan", 1)
            .await
            .unwrap(),
        "{\"plan\":true}"
    );

    ResourceOperationPlans::mark_completed(&handle, "plan", 1)
        .await
        .unwrap();

    assert_eq!(
        ResourceOperationPlans::completed_at(&handle, "plan", 1)
            .await
            .unwrap(),
        Some(1)
    );
    assert_eq!(
        ResourceOperationPlans::completed(&handle, "plan")
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn only_an_unfinished_plan_keeps_a_journal_to_replay() {
    let handle = handle().await;
    ResourceOperationPlans::insert_prepared(
        &handle, "plan", "instance", 1, "{}",
    )
    .await
    .unwrap();
    ResourceExecutionJournals::save(&handle, "plan", "journal")
        .await
        .unwrap();

    assert_eq!(
        ResourceExecutionJournals::for_plan(&handle, "plan")
            .await
            .unwrap(),
        Some("journal".to_string())
    );
    assert!(
        ResourceExecutionJournals::pending_for_instance(&handle, "instance")
            .await
            .unwrap()
    );

    ResourceOperationPlans::mark_completed(&handle, "plan", 1)
        .await
        .unwrap();

    assert_eq!(
        ResourceExecutionJournals::pending_payload(&handle, "plan")
            .await
            .unwrap(),
        None
    );

    ResourceExecutionJournals::delete(&handle, "plan")
        .await
        .unwrap();

    assert_eq!(
        ResourceExecutionJournals::for_plan(&handle, "plan")
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_cleanup_plan_is_opened_at_its_revision_and_consumed_once() {
    let handle = handle().await;
    ResourceHistoryCleanupPlans::insert(&handle, "cleanup", "instance", 1, "a")
        .await
        .unwrap();

    assert_eq!(
        ResourceHistoryCleanupPlans::open_payload(&handle, "cleanup", 1)
            .await
            .unwrap(),
        Some("a".to_string())
    );
    assert_eq!(
        ResourceHistoryCleanupPlans::open_payload(&handle, "cleanup", 2)
            .await
            .unwrap(),
        None
    );
    assert!(
        !ResourceHistoryCleanupPlans::consumed(&handle, "cleanup")
            .await
            .unwrap()
    );

    ResourceHistoryCleanupPlans::save_progress(&handle, "cleanup", "b")
        .await
        .unwrap();
    assert_eq!(
        ResourceHistoryCleanupPlans::payloads_for_instance(&handle, "instance")
            .await
            .unwrap(),
        vec!["b".to_string()]
    );

    ResourceHistoryCleanupPlans::consume(&handle, "cleanup", "c")
        .await
        .unwrap();

    assert!(
        ResourceHistoryCleanupPlans::consumed(&handle, "cleanup")
            .await
            .unwrap()
    );
    assert_eq!(
        ResourceHistoryCleanupPlans::open_payload(&handle, "cleanup", 1)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn the_instance_and_job_a_plan_carries_read_back() {
    let handle = handle().await;
    ResourceOperationPlans::insert_prepared(
        &handle,
        "plan",
        "instance",
        1,
        "{\"plan\":true}",
    )
    .await
    .unwrap();

    assert_eq!(
        ResourceOperationPlans::instance_at(&handle, "plan", 1)
            .await
            .unwrap(),
        "instance"
    );

    let mut connection = handle.pool().acquire().await.unwrap();

    assert_eq!(
        ResourceOperationPlans::job_at_in(&mut connection, "plan", 1)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        ResourceOperationPlans::queueable_payload_in(
            &mut connection,
            "plan",
            1
        )
        .await
        .unwrap(),
        Some("{\"plan\":true}".to_string())
    );
}

#[tokio::test]
async fn a_plan_is_queued_once() {
    let handle = handle_with_install_job().await;
    ResourceOperationPlans::insert_prepared(
        &handle,
        "plan",
        "instance",
        1,
        "{\"plan\":true}",
    )
    .await
    .unwrap();
    let mut connection = handle.pool().acquire().await.unwrap();

    let queued =
        ResourceOperationPlans::queue_in(&mut connection, "plan", 1, "job")
            .await
            .unwrap();

    assert_eq!(queued, 1);
    assert_eq!(
        ResourceOperationPlans::job_at_in(&mut connection, "plan", 1)
            .await
            .unwrap(),
        Some("job".to_string())
    );
    assert_eq!(
        ResourceOperationPlans::queueable_payload_in(
            &mut connection,
            "plan",
            1
        )
        .await
        .unwrap(),
        None
    );

    let requeued =
        ResourceOperationPlans::queue_in(&mut connection, "plan", 1, "other")
            .await
            .unwrap();

    assert_eq!(requeued, 0);
    assert_eq!(
        ResourceOperationPlans::job_at_in(&mut connection, "plan", 1)
            .await
            .unwrap(),
        Some("job".to_string())
    );
}
