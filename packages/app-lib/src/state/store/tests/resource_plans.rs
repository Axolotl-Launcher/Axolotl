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
