//! Which content-store objects are kept because something still refers to them.
//!
//! A hold names the reason that keeps the object and the moment it was taken.
//! The same reason never holds the same object twice, so taking a hold again
//! leaves the one already there alone, and dropping a reason's holds is how that
//! reason stops keeping them.

use sqlx::{Executor, Sqlite};

/// Keeps `digest` for `reason`, leaving a hold already taken under the same
/// reason in place.
pub(crate) async fn retain<'e, E>(
    exec: E,
    digest: &str,
    reason: &str,
    created_at: i64,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO store_retained_refs(digest, reason, created_at) VALUES (?, ?, ?)",
    )
    .bind(digest)
    .bind(reason)
    .bind(created_at)
    .execute(exec)
    .await?;

    Ok(())
}

/// Drops every hold `reason` keeps.
pub(crate) async fn release<'e, E>(exec: E, reason: &str) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM store_retained_refs WHERE reason = ?")
        .bind(reason)
        .execute(exec)
        .await?;

    Ok(())
}
