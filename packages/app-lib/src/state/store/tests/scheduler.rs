//! What the scheduler collections promise.

use super::support;
use crate::scheduler::tag::Level;
use crate::state::store::Collection as _;
use crate::state::store::scheduler_restart_budget::{
    RestartBudgetRecord, SchedulerRestartBudget,
};
use crate::state::store::scheduler_tags::{SchedulerTagRecord, SchedulerTags};

fn tag(owner: &str, level: Level) -> SchedulerTagRecord {
    SchedulerTagRecord {
        generation: 3,
        level,
        origin: "install".to_string(),
        owner: owner.to_string(),
        created_at: 10,
    }
}

#[tokio::test]
async fn a_tag_record_round_trips_and_clears() {
    let handle = support::handle().await;
    SchedulerTags::set(&handle, &7, Some(&tag("job", Level::L3)))
        .await
        .unwrap();
    assert_eq!(
        SchedulerTags::get(&handle, &7).await.unwrap(),
        Some(tag("job", Level::L3))
    );
    SchedulerTags::set(&handle, &7, None).await.unwrap();
    assert_eq!(SchedulerTags::get(&handle, &7).await.unwrap(), None);
}

#[tokio::test]
async fn every_level_is_stored_by_name() {
    let handle = support::handle().await;
    let levels = [Level::L0, Level::L1, Level::L2, Level::L3, Level::L4];
    for (id, level) in levels.into_iter().enumerate() {
        SchedulerTags::set(&handle, &(id as i64), Some(&tag("job", level)))
            .await
            .unwrap();
    }
    let stored: Vec<_> = SchedulerTags::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, record)| record.level)
        .collect();
    assert_eq!(stored, levels);
}

#[tokio::test]
async fn an_unknown_level_is_refused_by_the_schema() {
    let pool = support::pool().await;
    let refused = sqlx::query(
        "INSERT INTO scheduler_tags (id, generation, level, origin, owner, created_at)
         VALUES (1, 1, 'l9', 'x', 'y', 0)",
    )
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("CHECK"), "{refused}");
}

#[tokio::test]
async fn an_unknown_level_errors_for_one_key_and_is_skipped_in_bulk() {
    let pool = support::pool().await;
    // An older or edited database can hold a row the CHECK refuses today.
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO scheduler_tags (id, generation, level, origin, owner, created_at)
         VALUES (1, 1, 'l9', 'install', 'old', 0), (2, 1, 'l3', 'install', 'job', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let handle = crate::state::store::Handle::new(pool);
    let error = SchedulerTags::get(&handle, &1).await.unwrap_err();
    assert!(
        error.to_string().contains("level this build does not know"),
        "an existing row must not read as an absent key: {error}"
    );
    assert_eq!(
        SchedulerTags::get(&handle, &2)
            .await
            .unwrap()
            .map(|record| record.owner),
        Some("job".to_string())
    );
    assert!(SchedulerTags::get_scoped(&handle, &1).await.is_err());
    assert_eq!(
        SchedulerTags::get_scoped(&handle, &2).await.unwrap().len(),
        1
    );
    let stored: Vec<_> = SchedulerTags::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, record)| (id, record.owner))
        .collect();
    assert_eq!(stored, vec![(2, "job".to_string())]);
}

#[tokio::test]
async fn a_restart_budget_round_trips_and_resets() {
    let handle = support::handle().await;
    let failing = RestartBudgetRecord {
        failures: 4,
        first_failure_at: Some(100),
        revoked: true,
        updated_at: 200,
    };
    let scope = "l1:config_sync".to_string();
    SchedulerRestartBudget::set(&handle, &scope, Some(&failing))
        .await
        .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &scope).await.unwrap(),
        Some(failing.clone())
    );
    let other_scope = "l2:rebuild".to_string();
    let other = RestartBudgetRecord {
        failures: 1,
        first_failure_at: None,
        revoked: false,
        updated_at: 250,
    };
    SchedulerRestartBudget::set(&handle, &other_scope, Some(&other))
        .await
        .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &other_scope)
            .await
            .unwrap(),
        Some(other)
    );
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &scope).await.unwrap(),
        Some(failing)
    );
    let live = RestartBudgetRecord {
        failures: 0,
        first_failure_at: None,
        revoked: false,
        updated_at: 300,
    };
    SchedulerRestartBudget::set(&handle, &scope, Some(&live))
        .await
        .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &scope).await.unwrap(),
        Some(live.clone())
    );
    SchedulerRestartBudget::set(&handle, &other_scope, None)
        .await
        .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &other_scope)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &scope).await.unwrap(),
        Some(live)
    );
}

#[tokio::test]
async fn the_budget_schema_refuses_invalid_values() {
    let pool = support::pool().await;
    let negative = sqlx::query(
        "INSERT INTO scheduler_restart_budget (scope, failures, updated_at)
         VALUES ('negative', -1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(negative.to_string().contains("CHECK"), "{negative}");
    let revoked = sqlx::query(
        "INSERT INTO scheduler_restart_budget
            (scope, failures, revoked, updated_at)
         VALUES ('revoked', 0, 2, 0)",
    )
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(revoked.to_string().contains("CHECK"), "{revoked}");
}

#[tokio::test]
async fn tags_round_trip_through_scoped_and_replacement_verbs() {
    let handle = support::handle().await;
    SchedulerTags::set_scoped(&handle, &7, &[(7, tag("job", Level::L2))])
        .await
        .unwrap();
    assert_eq!(
        SchedulerTags::get_scoped(&handle, &7).await.unwrap(),
        vec![(7, tag("job", Level::L2))]
    );
    assert!(
        SchedulerTags::get_scoped(&handle, &8)
            .await
            .unwrap()
            .is_empty()
    );
    SchedulerTags::set_scoped(&handle, &7, &[]).await.unwrap();
    assert_eq!(SchedulerTags::get(&handle, &7).await.unwrap(), None);

    SchedulerTags::set_all(
        &handle,
        &[(1, tag("one", Level::L0)), (2, tag("two", Level::L4))],
    )
    .await
    .unwrap();
    let stored: Vec<_> = SchedulerTags::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, record)| (id, record.owner))
        .collect();
    assert_eq!(stored, vec![(1, "one".to_string()), (2, "two".to_string())]);

    SchedulerTags::set_all(&handle, &[(3, tag("three", Level::L1))])
        .await
        .unwrap();
    let stored: Vec<_> = SchedulerTags::get_all(&handle)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, record)| (id, record.owner))
        .collect();
    assert_eq!(stored, vec![(3, "three".to_string())]);
}

#[tokio::test]
async fn budgets_round_trip_through_scoped_and_replacement_verbs() {
    let handle = support::handle().await;
    let scope = "l1:config_sync".to_string();
    let record = RestartBudgetRecord {
        failures: 2,
        first_failure_at: Some(5),
        revoked: true,
        updated_at: 6,
    };
    SchedulerRestartBudget::set_scoped(
        &handle,
        &scope,
        &[(scope.clone(), record.clone())],
    )
    .await
    .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get_scoped(&handle, &scope)
            .await
            .unwrap(),
        vec![(scope.clone(), record.clone())]
    );
    SchedulerRestartBudget::set_scoped(&handle, &scope, &[])
        .await
        .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get(&handle, &scope).await.unwrap(),
        None
    );

    let other_scope = "l2:rebuild".to_string();
    let other = RestartBudgetRecord {
        failures: 0,
        first_failure_at: None,
        revoked: false,
        updated_at: 7,
    };
    SchedulerRestartBudget::set_all(
        &handle,
        &[
            (scope.clone(), record.clone()),
            (other_scope.clone(), other.clone()),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        SchedulerRestartBudget::get_all(&handle).await.unwrap(),
        vec![(scope, record), (other_scope, other)]
    );
}
