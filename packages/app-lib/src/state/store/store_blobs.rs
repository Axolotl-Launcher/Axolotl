//! The content store's objects: the digests that have been verified and
//! published here.
//!
//! The row is a claim about a file already on disk, so publishing the same
//! digest again refreshes the row it has instead of adding a second one.

use sqlx::{Executor, Sqlite};

/// Records that `digest` was published with `size`, at `published_at`.
pub(crate) async fn record<'e, E>(
    exec: E,
    digest: &str,
    size: u64,
    published_at: i64,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO store_blobs (digest, size, state, created_at, last_used_at) VALUES (?, ?, 'ready', ?, ?) ON CONFLICT(digest) DO UPDATE SET size = excluded.size, state = 'ready', last_used_at = excluded.last_used_at",
    )
    .bind(digest)
    .bind(size as i64)
    .bind(published_at)
    .bind(published_at)
    .execute(exec)
    .await?;

    Ok(())
}
