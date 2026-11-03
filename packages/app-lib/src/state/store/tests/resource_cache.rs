//! What the resource cache, plan and history collections promise.

use super::support;
use crate::resources::cache::ResourceCache;
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::atom;
use crate::state::store::resource_artifact_evidence::{
    ResourceArtifactEvidence, ResourceArtifactEvidenceRecord,
};
use crate::state::store::resource_cache_entries::{
    ResourceCacheEntries, ResourceCacheEntryKey, ResourceCacheEntryRecord,
};
use crate::state::store::resource_cache_state::{
    ResourceCacheState, ResourceCacheStateRecord,
};
use crate::state::store::resource_history::{
    ResourceHistory, ResourceHistoryRecord, ResourceHistoryScope,
};
use crate::state::store::resource_operation_plans::{
    ResourceOperationPlanRecord, ResourceOperationPlans,
};

fn handle_of(pool: &sqlx::SqlitePool) -> Handle {
    Handle::new(pool.clone())
}

/// A handle on a database the resource cache migrated, which is a database of
/// its own: the cache keeps its own schema and its own file.
async fn cache_handle() -> (tempfile::TempDir, Handle) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resource-cache.db");
    let cache = ResourceCache::open(path.clone()).await;
    assert!(cache.available());
    cache.close().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    (temp, handle_of(&pool))
}

fn entry() -> ResourceCacheEntryKey {
    ResourceCacheEntryKey {
        namespace: "modrinth-v2-project".to_string(),
        source: "modrinth".to_string(),
        query_key: "query".to_string(),
    }
}

fn answer() -> ResourceCacheEntryRecord {
    ResourceCacheEntryRecord {
        value_version: 1,
        payload: "{\"answer\":true}".to_string(),
        retrieved_at: 1_000_000_000,
        expires_at: 2_000_000_000,
    }
}

fn plan(instance_id: &str, revision: i64) -> ResourceOperationPlanRecord {
    ResourceOperationPlanRecord {
        instance_id: instance_id.to_string(),
        revision,
        payload: "{\"plan\":true}".to_string(),
        job_id: None,
        queued: false,
        completed: false,
    }
}

fn history(recorded_at: i64) -> ResourceHistoryRecord {
    history_of("instance", recorded_at)
}

fn history_of(instance_id: &str, recorded_at: i64) -> ResourceHistoryRecord {
    ResourceHistoryRecord {
        instance_id: instance_id.to_string(),
        entry_id: "entry".to_string(),
        recorded_at,
        payload: "{\"history\":true}".to_string(),
        backup_relative_path: Some("backup.jar".to_string()),
        backup_size: Some(3),
        backup_sha1: Some("abc".to_string()),
        backup_available: true,
    }
}

/// An instance, since every plan and every history entry belongs to one.
async fn handle_with_instance() -> Handle {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    support::add_instance(&pool, "other").await;
    handle_of(&pool)
}

#[tokio::test]
async fn a_cached_answer_round_trips_and_removes() {
    let (_temp, cache) = cache_handle().await;

    ResourceCacheEntries::set(&cache, &entry(), Some(&answer()))
        .await
        .unwrap();
    assert_eq!(
        ResourceCacheEntries::get(&cache, &entry()).await.unwrap(),
        Some(answer())
    );

    ResourceCacheEntries::set(&cache, &entry(), None)
        .await
        .unwrap();
    assert_eq!(
        ResourceCacheEntries::get(&cache, &entry()).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn writing_no_answer_at_all_empties_the_cache() {
    let (_temp, cache) = cache_handle().await;
    ResourceCacheEntries::set(&cache, &entry(), Some(&answer()))
        .await
        .unwrap();

    ResourceCacheEntries::set_all(&cache, &[]).await.unwrap();

    assert!(
        ResourceCacheEntries::get_all(&cache)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_cache_holds_one_generation_and_a_unit_moves_it() {
    let (_temp, cache) = cache_handle().await;
    let initial = ResourceCacheStateRecord { generation: 0 };
    assert_eq!(
        ResourceCacheState::get(&cache, &()).await.unwrap(),
        Some(initial)
    );

    let mut unit = atom(&cache).await.unwrap();
    let current = ResourceCacheState::atom_get(&mut unit, &())
        .await
        .unwrap()
        .unwrap();
    let bumped = ResourceCacheStateRecord {
        generation: current.generation + 1,
    };
    ResourceCacheState::atom_set(&mut unit, &(), Some(&bumped))
        .await
        .unwrap();
    unit.commit().await.unwrap();

    assert_eq!(
        ResourceCacheState::get(&cache, &()).await.unwrap(),
        Some(bumped)
    );
}

#[tokio::test]
async fn evidence_round_trips_and_removes() {
    let (_temp, cache) = cache_handle().await;
    let key = "artifact".to_string();
    let record = ResourceArtifactEvidenceRecord {
        value_version: 1,
        payload: "{\"verified\":true}".to_string(),
        verified_at: 5,
    };

    ResourceArtifactEvidence::set(&cache, &key, Some(&record))
        .await
        .unwrap();
    assert_eq!(
        ResourceArtifactEvidence::get(&cache, &key).await.unwrap(),
        Some(record)
    );

    ResourceArtifactEvidence::set(&cache, &key, None)
        .await
        .unwrap();
    assert_eq!(
        ResourceArtifactEvidence::get(&cache, &key).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn a_plan_round_trips_under_its_id() {
    let handle = handle_with_instance().await;
    let key = "plan".to_string();

    ResourceOperationPlans::set(&handle, &key, Some(&plan("instance", 2)))
        .await
        .unwrap();

    assert_eq!(
        ResourceOperationPlans::get(&handle, &key).await.unwrap(),
        Some(plan("instance", 2))
    );
}

#[tokio::test]
async fn a_plan_reads_only_the_plans_of_the_instance_it_names() {
    let pool = support::pool().await;
    support::add_instance(&pool, "instance").await;
    support::add_instance(&pool, "other").await;
    let handle = handle_of(&pool);
    ResourceOperationPlans::set(
        &handle,
        &"mine".to_string(),
        Some(&plan("instance", 1)),
    )
    .await
    .unwrap();
    ResourceOperationPlans::set(
        &handle,
        &"theirs".to_string(),
        Some(&plan("other", 1)),
    )
    .await
    .unwrap();

    let scoped =
        ResourceOperationPlans::get_scoped(&handle, &"instance".to_string())
            .await
            .unwrap();

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].0, "mine");
}

#[tokio::test]
async fn a_plan_replacement_refuses_another_instance() {
    let handle = handle_with_instance().await;

    let error = ResourceOperationPlans::set_scoped(
        &handle,
        &"instance".to_string(),
        &[("plan".to_string(), plan("other", 1))],
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("scope"));
    assert_eq!(
        ResourceOperationPlans::get(&handle, &"plan".to_string())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn the_history_of_one_entry_is_read_newest_first() {
    let handle = handle_with_instance().await;
    for (id, recorded_at) in [("older", 1), ("first", 5), ("second", 5)] {
        ResourceHistory::set(
            &handle,
            &id.to_string(),
            Some(&history(recorded_at)),
        )
        .await
        .unwrap();
    }

    let scoped = ResourceHistory::get_scoped(
        &handle,
        &ResourceHistoryScope::Entry {
            instance_id: "instance".to_string(),
            entry_id: "entry".to_string(),
        },
    )
    .await
    .unwrap();
    let ids: Vec<String> = scoped.into_iter().map(|(id, _)| id).collect();

    assert_eq!(ids, vec!["second", "first", "older"]);
}

#[tokio::test]
async fn the_history_of_one_entry_leaves_the_other_entries_out() {
    let handle = handle_with_instance().await;
    ResourceHistory::set(&handle, &"mine".to_string(), Some(&history(1)))
        .await
        .unwrap();
    let mut other = history(1);
    other.entry_id = "other".to_string();
    ResourceHistory::set(&handle, &"theirs".to_string(), Some(&other))
        .await
        .unwrap();
    ResourceHistory::set(
        &handle,
        &"elsewhere".to_string(),
        Some(&history_of("other", 1)),
    )
    .await
    .unwrap();

    let scoped = ResourceHistory::get_scoped(
        &handle,
        &ResourceHistoryScope::Entry {
            instance_id: "instance".to_string(),
            entry_id: "entry".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].0, "mine");
}

#[tokio::test]
async fn the_history_of_an_instance_reads_every_entry_of_it() {
    let handle = handle_with_instance().await;
    ResourceHistory::set(&handle, &"mine".to_string(), Some(&history(1)))
        .await
        .unwrap();
    let mut other = history(2);
    other.entry_id = "other".to_string();
    ResourceHistory::set(&handle, &"theirs".to_string(), Some(&other))
        .await
        .unwrap();
    ResourceHistory::set(
        &handle,
        &"elsewhere".to_string(),
        Some(&history_of("other", 3)),
    )
    .await
    .unwrap();

    let scoped = ResourceHistory::get_scoped(
        &handle,
        &ResourceHistoryScope::Instance("instance".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(scoped.len(), 2);
}
