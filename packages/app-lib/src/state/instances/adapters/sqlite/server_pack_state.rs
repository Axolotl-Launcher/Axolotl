//! Which modpack version an instance's server list was taken from.

use sqlx::{Executor, Sqlite, Transaction};

/// Whether the instance's server list was ever built from its modpack.
pub(crate) async fn pack_state_exists<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let exists: bool = sqlx::query_scalar(
        "
        SELECT EXISTS(
            SELECT 1 FROM instance_server_pack_state WHERE instance_id = ?
        )
        ",
    )
    .bind(instance_id)
    .fetch_one(exec)
    .await?;

    Ok(exists)
}

/// The modpack version the instance's server list was taken from.
///
/// `None` means no record; `Some(None)` means the record names no version.
pub(crate) async fn get_pack_state_version<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<Option<String>>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<Option<String>> = sqlx::query_scalar(
        "
        SELECT version_id
        FROM instance_server_pack_state
        WHERE instance_id = ?
        ",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(row)
}

/// Records the modpack version the instance's server list was taken from.
pub(crate) async fn set_pack_state_version(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    version_id: Option<&str>,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO instance_server_pack_state (instance_id, version_id)
        VALUES (?, ?)
        ON CONFLICT(instance_id) DO UPDATE SET
            version_id = excluded.version_id
        ",
    )
    .bind(instance_id)
    .bind(version_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
