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
    .await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn a_restart_budget_round_trips_and_resets() {
    let handle = support::handle().await;
    let failing = RestartBudgetRecord {
        failures: 4,
        first_failure_at: Some(100),
        revoked: false,
        updated_at: 200,
    };
    let scope = "l1:config_sync".to_string();
    SchedulerRestartBudget::set(&handle, &scope, Some(&failing))
        .await
        .unwrap();
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
        Some(live)
    );
}
