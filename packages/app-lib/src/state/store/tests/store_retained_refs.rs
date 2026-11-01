//! What the retained references promise: a reason holds an object once, and
//! dropping that reason drops only its own holds.

use super::support;
use crate::state::store::store_retained_refs::{release, retain};

async fn pool_with_blobs(digests: &[&str]) -> sqlx::SqlitePool {
    let pool = support::pool().await;
    for digest in digests {
        sqlx::query(
            "INSERT INTO store_blobs (digest, size, state, created_at, last_used_at) VALUES (?, 1, 'ready', 1, 1)",
        )
        .bind(digest)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

async fn holds(pool: &sqlx::SqlitePool, reason: &str) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT digest, created_at FROM store_retained_refs WHERE reason = ? ORDER BY digest",
    )
    .bind(reason)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_hold_reads_back_and_releasing_its_reason_drops_it() {
    let pool = pool_with_blobs(&["blob"]).await;

    retain(&pool, "blob", "resource-history:one", 10)
        .await
        .unwrap();
    assert_eq!(
        holds(&pool, "resource-history:one").await,
        [("blob".to_string(), 10)]
    );

    release(&pool, "resource-history:one").await.unwrap();
    assert!(holds(&pool, "resource-history:one").await.is_empty());
}

#[tokio::test]
async fn the_same_reason_holds_an_object_once() {
    let pool = pool_with_blobs(&["blob"]).await;

    retain(&pool, "blob", "resource-history:one", 10)
        .await
        .unwrap();
    retain(&pool, "blob", "resource-history:one", 20)
        .await
        .unwrap();

    assert_eq!(
        holds(&pool, "resource-history:one").await,
        [("blob".to_string(), 10)]
    );
}

#[tokio::test]
async fn releasing_a_reason_leaves_the_other_reasons_holding() {
    let pool = pool_with_blobs(&["blob"]).await;

    retain(&pool, "blob", "resource-history:one", 10)
        .await
        .unwrap();
    retain(&pool, "blob", "resource-history:two", 20)
        .await
        .unwrap();

    release(&pool, "resource-history:one").await.unwrap();

    assert!(holds(&pool, "resource-history:one").await.is_empty());
    assert_eq!(
        holds(&pool, "resource-history:two").await,
        [("blob".to_string(), 20)]
    );
}
