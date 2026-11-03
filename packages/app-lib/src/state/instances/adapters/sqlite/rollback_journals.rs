//! The journal an interrupted legacy content replacement is recovered from.

use sqlx::{Executor, Sqlite, Transaction};

/// Records the journal of a replacement that is about to start.
pub(crate) async fn set_journal<'e, E>(
    instance_id: &str,
    payload: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO resource_legacy_rollback_journals(instance_id, payload) VALUES (?, ?)",
    )
    .bind(instance_id)
    .bind(payload)
    .execute(exec)
    .await?;

    Ok(())
}

/// The journal `instance_id` left behind, if any.
pub(crate) async fn get_journal<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let payload: Option<String> = sqlx::query_scalar(
        "SELECT payload FROM resource_legacy_rollback_journals WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(payload)
}

/// Forgets the journal `instance_id` left behind.
pub(crate) async fn delete_journal<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "DELETE FROM resource_legacy_rollback_journals WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(exec)
    .await?;

    Ok(())
}

/// Forgets the journal of `instance_id` only while it still says `payload`, so
/// a journal another attempt replaced is kept.
pub(crate) async fn delete_journal_matching(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    payload: &str,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM resource_legacy_rollback_journals WHERE instance_id = ? AND payload = ?",
    )
    .bind(instance_id)
    .bind(payload)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Every instance that left a journal behind.
pub(crate) async fn get_journal_instance_ids<'e, E>(
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT instance_id FROM resource_legacy_rollback_journals",
    )
    .fetch_all(exec)
    .await?;

    Ok(rows)
}
