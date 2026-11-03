//! The servers an instance keeps for itself.

use sqlx::{Executor, Sqlite, Transaction};

/// One server as the instance keeps it.
#[derive(Debug, Clone)]
pub(crate) struct LocalServerRow {
    pub id: String,
    pub source: String,
    pub excluded_synced_server_id: Option<String>,
    pub nbt: Vec<u8>,
    pub position: i64,
}

/// Replaces every server `instance_id` keeps with `servers`.
pub(crate) async fn replace_local_servers(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    servers: &[LocalServerRow],
) -> crate::Result<()> {
    sqlx::query("DELETE FROM instance_servers WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut **tx)
        .await?;

    for server in servers {
        sqlx::query(
            "
            INSERT INTO instance_servers
                (instance_id, id, source, excluded_synced_server_id,
                 nbt, position)
            VALUES (?, ?, ?,
                (SELECT id FROM synced_servers WHERE id = ?), ?, ?)
            ",
        )
        .bind(instance_id)
        .bind(&server.id)
        .bind(&server.source)
        .bind(&server.excluded_synced_server_id)
        .bind(&server.nbt)
        .bind(server.position)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Every server `instance_id` keeps, in the order it keeps them.
pub(crate) async fn get_local_servers<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<LocalServerRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<(String, String, Option<String>, Vec<u8>, i64)> =
        sqlx::query_as(
            "
            SELECT id, source, excluded_synced_server_id, nbt, position
            FROM instance_servers
            WHERE instance_id = ?
            ORDER BY position
            ",
        )
        .bind(instance_id)
        .fetch_all(exec)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(id, source, excluded_synced_server_id, nbt, position)| {
            LocalServerRow {
                id,
                source,
                excluded_synced_server_id,
                nbt,
                position,
            }
        })
        .collect())
}

/// How many servers `instance_id` keeps and how many payload bytes they carry,
/// read before the rows themselves so an oversized set is refused unread.
pub(crate) async fn get_local_server_size<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<(usize, usize)>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: (i64, i64) = sqlx::query_as(
        "
        SELECT COUNT(*), COALESCE(SUM(length(nbt)), 0)
        FROM instance_servers
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

/// Whether one of the servers `instance_id` keeps is a local copy of a synced
/// server the user detached.
pub(crate) async fn has_desynced_synced_server<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instance_servers
         WHERE instance_id = ? AND source = 'local_desynced'
           AND excluded_synced_server_id IS NOT NULL",
    )
    .bind(instance_id)
    .fetch_one(exec)
    .await?;

    Ok(count > 0)
}
