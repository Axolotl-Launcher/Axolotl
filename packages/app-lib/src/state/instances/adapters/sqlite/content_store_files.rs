//! Which content-store bytes each instance path is made of.

use sqlx::{Executor, Sqlite, Transaction};

/// Forgets the store rows of `paths` under `instance_id`.
pub(crate) async fn remove_instance_files(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    paths: &[&str],
) -> crate::Result<()> {
    for path in paths {
        sqlx::query(
            "DELETE FROM store_instance_files WHERE instance_id = ? AND relative_path = ?",
        )
        .bind(instance_id)
        .bind(path)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Records which content-store object one path of an instance is made of now.
pub(crate) async fn record_instance_file(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    relative_path: &str,
    digest: &str,
    created_at: i64,
    modified_at: i64,
) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO store_instance_files(instance_id, relative_path, digest, created_at, modified_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(instance_id, relative_path) DO UPDATE SET digest = excluded.digest, modified_at = excluded.modified_at",
    )
    .bind(instance_id)
    .bind(relative_path)
    .bind(digest)
    .bind(created_at)
    .bind(modified_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// The content-store object one path of an instance was made of.
pub(crate) async fn get_instance_file_digest<'e, E>(
    instance_id: &str,
    relative_path: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let digest = sqlx::query_scalar(
        "SELECT digest FROM store_instance_files WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(exec)
    .await?;

    Ok(digest)
}

/// Every path an instance currently holds, whether an entry file or a
/// content-store row names it.
pub(crate) async fn get_current_paths<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let paths = sqlx::query_scalar(
        "SELECT file.relative_path FROM instance_files file JOIN instance_content_entries entry ON entry.file_id = file.id WHERE file.instance_id = ? UNION ALL SELECT relative_path FROM store_instance_files WHERE instance_id = ?",
    )
    .bind(instance_id)
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    Ok(paths)
}

#[cfg(test)]
#[path = "content_store_files_tests.rs"]
mod tests;
