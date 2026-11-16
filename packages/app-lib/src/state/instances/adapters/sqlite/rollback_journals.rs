//! The journal an interrupted legacy content replacement is recovered from.

use crate::state::store::recovery_documents;
use sqlx::{Sqlite, SqliteConnection, Transaction};

/// Records the journal of a replacement that is about to start.
pub(crate) async fn set_journal(
    instance_id: &str,
    payload: &str,
    exec: &mut SqliteConnection,
) -> crate::Result<()> {
    if recovery_documents::active(exec).await? {
        if recovery_documents::read::<serde_json::Value>(
            exec,
            "resource_legacy_rollback_journals",
            instance_id,
        )
        .await?
        .is_some()
        {
            return Err(crate::ErrorKind::InputError(
                "Legacy recovery already pending".into(),
            )
            .into());
        }
        return recovery_documents::write(
            exec,
            "resource_legacy_rollback_journals",
            instance_id,
            Some(&serde_json::json!({"payload":payload})),
        )
        .await;
    }

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
pub(crate) async fn get_journal(
    instance_id: &str,
    exec: &mut SqliteConnection,
) -> crate::Result<Option<String>> {
    if recovery_documents::active(exec).await? {
        return Ok(recovery_documents::read::<serde_json::Value>(
            exec,
            "resource_legacy_rollback_journals",
            instance_id,
        )
        .await?
        .and_then(|value| value["payload"].as_str().map(str::to_owned)));
    }

    let payload: Option<String> = sqlx::query_scalar(
        "SELECT payload FROM resource_legacy_rollback_journals WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(payload)
}

/// Forgets the journal `instance_id` left behind.
pub(crate) async fn delete_journal(
    instance_id: &str,
    exec: &mut SqliteConnection,
) -> crate::Result<()> {
    if recovery_documents::active(exec).await? {
        return recovery_documents::write::<serde_json::Value>(
            exec,
            "resource_legacy_rollback_journals",
            instance_id,
            None,
        )
        .await;
    }

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
    if recovery_documents::active(&mut **tx).await? {
        if recovery_documents::read::<serde_json::Value>(
            &mut **tx,
            "resource_legacy_rollback_journals",
            instance_id,
        )
        .await?
        .is_some_and(|value| value["payload"].as_str() == Some(payload))
        {
            recovery_documents::write::<serde_json::Value>(
                &mut **tx,
                "resource_legacy_rollback_journals",
                instance_id,
                None,
            )
            .await?;
        }
        return Ok(());
    }

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
pub(crate) async fn get_journal_instance_ids(
    exec: &mut SqliteConnection,
) -> crate::Result<Vec<String>> {
    if recovery_documents::active(exec).await? {
        return Ok(recovery_documents::all::<serde_json::Value>(
            exec,
            "resource_legacy_rollback_journals",
        )
        .await?
        .into_iter()
        .map(|(key, _)| key)
        .collect());
    }

    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT instance_id FROM resource_legacy_rollback_journals",
    )
    .fetch_all(exec)
    .await?;

    Ok(rows)
}
