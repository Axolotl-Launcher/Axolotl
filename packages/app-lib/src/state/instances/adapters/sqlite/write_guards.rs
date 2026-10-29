use sqlx::{Sqlite, Transaction};

/// Ensures the instance a content write is about to reference still exists.
///
/// Runs inside the write transaction so a concurrent instance deletion that
/// already committed is reported as a clean error instead of a raw SQLite
/// foreign-key violation (`SQLITE_CONSTRAINT_FOREIGNKEY`, code 787).
pub(crate) async fn ensure_instance_exists(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let instance_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instances WHERE id = ?)",
    )
    .bind(instance_id)
    .fetch_one(&mut **tx)
    .await?;
    if !instance_exists {
        return Err(crate::ErrorKind::InputError(
            "This instance has been deleted".to_string(),
        )
        .into());
    }

    Ok(())
}
/// Ensures the instance and content set a content write is about to reference
/// still exist, returning a clean error instead of a foreign-key violation.
pub(crate) async fn ensure_content_write_parents(
    instance_id: &str,
    content_set_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    ensure_instance_exists(instance_id, tx).await?;

    let content_set_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM instance_content_sets WHERE id = ?)",
    )
    .bind(content_set_id)
    .fetch_one(&mut **tx)
    .await?;
    if !content_set_exists {
        return Err(crate::ErrorKind::InputError(
            "The content set for this instance has been deleted".to_string(),
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
#[path = "write_guards_tests.rs"]
mod tests;
