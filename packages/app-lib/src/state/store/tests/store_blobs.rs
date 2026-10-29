//! What a content object the launcher published promises: it reads back, and
//! publishing it again refreshes the row it has instead of adding a second one.

use super::support;
use crate::state::store::store_blobs::record;

async fn published(
    pool: &sqlx::SqlitePool,
    digest: &str,
) -> (i64, String, i64, i64) {
    sqlx::query_as(
        "SELECT size, state, created_at, last_used_at
         FROM store_blobs
         WHERE digest = ?",
    )
    .bind(digest)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_published_object_reads_back() {
    let pool = support::pool().await;

    record(&pool, "blob", 42, 10).await.unwrap();

    assert_eq!(
        published(&pool, "blob").await,
        (42, "ready".to_string(), 10, 10)
    );
}

#[tokio::test]
async fn publishing_the_same_digest_again_refreshes_its_row() {
    let pool = support::pool().await;
    record(&pool, "blob", 42, 10).await.unwrap();

    record(&pool, "blob", 84, 20).await.unwrap();

    // The row keeps when it was first written and moves its last use forward.
    assert_eq!(
        published(&pool, "blob").await,
        (84, "ready".to_string(), 10, 20)
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM store_blobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}
