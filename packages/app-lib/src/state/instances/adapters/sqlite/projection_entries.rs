//! The servers.dat an instance is written from.

use sqlx::{Executor, Sqlite, Transaction};

/// One projected entry: a server, and whose copy of it this is.
#[derive(Debug, Clone)]
pub(crate) struct ProjectionEntryRow {
    pub owner: String,
    pub server_id: String,
    pub nbt: Vec<u8>,
    pub position: i64,
}

/// Replaces everything `instance_id` projects with `entries`.
pub(crate) async fn replace_projection(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    entries: &[ProjectionEntryRow],
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_server_projection_entries WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(&mut **tx)
    .await?;

    for entry in entries {
        sqlx::query(
            "
            INSERT INTO instance_server_projection_entries
                (instance_id, owner, server_id, nbt, position)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(instance_id, owner, server_id) DO UPDATE SET
                nbt = excluded.nbt,
                position = excluded.position
            ",
        )
        .bind(instance_id)
        .bind(&entry.owner)
        .bind(&entry.server_id)
        .bind(&entry.nbt)
        .bind(entry.position)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Forgets what `instance_id` projected, whatever owner it was projected for.
pub(crate) async fn clear_projection(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_server_projection_entries WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Every entry `instance_id` projects, in position order.
pub(crate) async fn get_projection_entries<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<ProjectionEntryRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(String, String, Vec<u8>, i64)> = sqlx::query_as(
        "
        SELECT owner, server_id, nbt, position
        FROM instance_server_projection_entries
        WHERE instance_id = ?
        ORDER BY position
        ",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(owner, server_id, nbt, position)| ProjectionEntryRow {
            owner,
            server_id,
            nbt,
            position,
        })
        .collect())
}

/// How many entries `instance_id` projects and how many payload bytes they
/// carry, read before the rows themselves so an oversized set is refused unread.
pub(crate) async fn get_projection_size<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<(usize, usize)>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: (i64, i64) = sqlx::query_as(
        "
        SELECT COUNT(*), COALESCE(SUM(length(nbt)), 0)
        FROM instance_server_projection_entries
        WHERE instance_id = ?
        ",
    )
    .bind(instance_id)
    .fetch_one(exec)
    .await?;

    Ok((
        row.0.try_into().unwrap_or(usize::MAX),
        row.1.try_into().unwrap_or(usize::MAX),
    ))
}
