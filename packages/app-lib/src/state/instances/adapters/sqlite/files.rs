use super::values::{timestamp, unsigned};
use crate::state::instances::InstanceFile;
use chrono::Utc;
use sqlx::{Executor, Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceFileRow {
    pub id: String,
    pub instance_id: String,
    pub relative_path: String,
    pub file_name: String,
    pub enabled: i64,
    pub sha1: String,
    pub size: i64,
    pub missing: i64,
    pub added_at: i64,
    pub modified_at: i64,
    pub local_mod_data: Option<String>,
    pub icon_path: Option<String>,
}

impl TryFrom<InstanceFileRow> for InstanceFile {
    type Error = crate::Error;

    fn try_from(row: InstanceFileRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            instance_id: row.instance_id,
            relative_path: row.relative_path,
            file_name: row.file_name,
            enabled: row.enabled == 1,
            sha1: row.sha1,
            size: unsigned(row.size, "size")?,
            missing: row.missing == 1,
            added_at: timestamp(row.added_at),
            modified_at: timestamp(row.modified_at),
            local_mod_data: row.local_mod_data,
            icon_path: row.icon_path,
        })
    }
}

pub(crate) async fn get_instance_files<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<InstanceFile>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as::<_, InstanceFileRow>(
        "
        SELECT *
        FROM instance_files
        WHERE instance_id = ?
        ORDER BY relative_path ASC
        ",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn mark_instance_files_missing(
    instance_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let modified_at = Utc::now().timestamp();

    sqlx::query!(
        "
        UPDATE instance_files
        SET
            missing = 1,
            modified_at = ?
        WHERE instance_id = ?
        ",
        modified_at,
        instance_id,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) async fn upsert_instance_file(
    file: &InstanceFile,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let id = file.id.as_str();
    let instance_id = file.instance_id.as_str();
    let relative_path = file.relative_path.as_str();
    let file_name = file.file_name.as_str();
    let enabled = i64::from(file.enabled);
    let sha1 = file.sha1.as_str();
    let size = file.size as i64;
    let missing = i64::from(file.missing);
    let added_at = file.added_at.timestamp();
    let modified_at = file.modified_at.timestamp();
    let local_mod_data = file.local_mod_data.as_deref();
    let icon_path = file.icon_path.as_deref();

    sqlx::query!(
        "
        INSERT INTO instance_files (
            id,
            instance_id,
            relative_path,
            file_name,
            enabled,
            sha1,
            size,
            missing,
            added_at,
            modified_at,
            local_mod_data,
            icon_path
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (instance_id, relative_path) DO UPDATE SET
            file_name = excluded.file_name,
            enabled = excluded.enabled,
            sha1 = excluded.sha1,
            size = excluded.size,
            missing = excluded.missing,
            modified_at = excluded.modified_at,
            local_mod_data = COALESCE(excluded.local_mod_data, instance_files.local_mod_data),
            icon_path = COALESCE(excluded.icon_path, instance_files.icon_path)
        ",
        id,
        instance_id,
        relative_path,
        file_name,
        enabled,
        sha1,
        size,
        missing,
        added_at,
        modified_at,
        local_mod_data,
        icon_path,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub(crate) struct UpsertInstanceFile<'a> {
    pub instance_id: &'a str,
    pub relative_path: &'a str,
    pub file_name: &'a str,
    pub enabled: bool,
    pub sha1: &'a str,
    pub size: u64,
    pub missing: bool,
    pub local_mod_data: Option<&'a str>,
    pub icon_path: Option<&'a str>,
}

pub(crate) async fn get_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceFile>> {
    let row = sqlx::query_as::<_, InstanceFileRow>(
        "
        SELECT *
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn upsert_instance_file_from_parts(
    input: UpsertInstanceFile<'_>,
    pool: &SqlitePool,
) -> crate::Result<InstanceFile> {
    let mut tx = pool.begin().await?;
    let file =
        upsert_instance_file_from_parts_in_transaction(input, &mut tx).await?;
    tx.commit().await?;
    Ok(file)
}

pub(crate) async fn upsert_instance_file_from_parts_in_transaction(
    input: UpsertInstanceFile<'_>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<InstanceFile> {
    let existing: Option<InstanceFile> = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(input.instance_id)
    .bind(input.relative_path)
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| row.try_into())
    .transpose()?;
    let local_mod_data = input
        .local_mod_data
        .map(ToString::to_string)
        .or_else(|| existing.as_ref().and_then(|f| f.local_mod_data.clone()));
    let icon_path = input
        .icon_path
        .map(ToString::to_string)
        .or_else(|| existing.as_ref().and_then(|f| f.icon_path.clone()));

    let file = InstanceFile {
        id: existing
            .as_ref()
            .map(|file| file.id.clone())
            .unwrap_or_else(|| format!("instance-file:{}", Uuid::new_v4())),
        instance_id: input.instance_id.to_string(),
        relative_path: input.relative_path.to_string(),
        file_name: input.file_name.to_string(),
        enabled: input.enabled,
        sha1: input.sha1.to_string(),
        size: input.size,
        missing: input.missing,
        added_at: existing
            .as_ref()
            .map(|file| file.added_at)
            .unwrap_or_else(Utc::now),
        modified_at: Utc::now(),
        local_mod_data,
        icon_path,
    };

    upsert_instance_file(&file, &mut *tx).await?;

    Ok(file)
}

pub(crate) async fn rename_instance_file(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceFile>> {
    let mut tx = pool.begin().await?;

    let file = rename_instance_file_in_transaction(
        instance_id,
        old_relative_path,
        new_relative_path,
        new_file_name,
        enabled,
        &mut tx,
    )
    .await?;
    tx.commit().await?;

    Ok(file)
}

pub(crate) async fn rename_instance_file_in_transaction(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let enabled = i64::from(enabled);
    let modified_at = Utc::now().timestamp();

    let source_id = sqlx::query_scalar!(
        "
        SELECT id
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        old_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let target_id = sqlx::query_scalar!(
        "
        SELECT id
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let (Some(source_id), Some(target_id)) =
        (source_id.as_deref(), target_id.as_deref())
        && source_id != target_id
    {
        sqlx::query!(
            "
                DELETE FROM instance_content_entries
                WHERE id IN (
                    SELECT target_entry.id
                    FROM instance_content_entries target_entry
                    WHERE target_entry.instance_id = ?
                        AND target_entry.file_id = ?
                        AND EXISTS (
                            SELECT 1
                            FROM instance_content_entries source_entry
                            WHERE source_entry.instance_id = target_entry.instance_id
                                AND source_entry.content_set_id = target_entry.content_set_id
                                AND source_entry.file_id = ?
                        )
                )
                ",
            instance_id,
            target_id,
            source_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
                UPDATE instance_content_entries
                SET file_id = ?, modified_at = ?
                WHERE instance_id = ? AND file_id = ?
                ",
            source_id,
            modified_at,
            instance_id,
            target_id,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query!(
            "
                DELETE FROM instance_files
                WHERE id = ?
                ",
            target_id,
        )
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query!(
        "
        UPDATE instance_files
        SET
            relative_path = ?,
            file_name = ?,
            enabled = ?,
            missing = 0,
            modified_at = ?
        WHERE instance_id = ? AND relative_path = ?
        ",
        new_relative_path,
        new_file_name,
        enabled,
        modified_at,
        instance_id,
        old_relative_path,
    )
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(new_relative_path)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn move_instance_file_in_transaction(
    instance_id: &str,
    old_relative_path: &str,
    new_relative_path: &str,
    new_file_name: &str,
    enabled: bool,
    sha1: &str,
    size: u64,
    local_mod_data: Option<&str>,
    icon_path: Option<&str>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<Option<InstanceFile>> {
    let source_id = sqlx::query_scalar!(
        "
        SELECT id
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        old_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let target_id = sqlx::query_scalar!(
        "
        SELECT id
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(source_id) = source_id else {
        return Ok(None);
    };
    if target_id
        .as_deref()
        .is_some_and(|target_id| target_id != source_id)
    {
        return Ok(None);
    }

    sqlx::query(
        "
        UPDATE instance_files
        SET
            relative_path = ?,
            file_name = ?,
            enabled = ?,
            sha1 = ?,
            size = ?,
            missing = 0,
            modified_at = ?,
            local_mod_data = COALESCE(?, local_mod_data),
            icon_path = COALESCE(?, icon_path)
        WHERE instance_id = ? AND relative_path = ?
        ",
    )
    .bind(new_relative_path)
    .bind(new_file_name)
    .bind(i64::from(enabled))
    .bind(sha1)
    .bind(size as i64)
    .bind(Utc::now().timestamp())
    .bind(local_mod_data)
    .bind(icon_path)
    .bind(instance_id)
    .bind(old_relative_path)
    .execute(&mut **tx)
    .await?;

    let row = sqlx::query_as::<_, InstanceFileRow>(
        "SELECT * FROM instance_files
         WHERE instance_id = ? AND relative_path = ?",
    )
    .bind(instance_id)
    .bind(new_relative_path)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(TryInto::try_into).transpose()
}

pub(crate) async fn adopt_untracked_file_in_transaction(
    instance_id: &str,
    new_relative_path: &str,
    tracked_file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<bool> {
    let untracked_file_id = sqlx::query_scalar!(
        "
        SELECT id
        FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        new_relative_path,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(untracked_file_id) = untracked_file_id else {
        return Ok(false);
    };
    if untracked_file_id == tracked_file_id {
        return Ok(false);
    }
    let updated = sqlx::query(
        "UPDATE instance_content_entries
         SET file_id = ?, modified_at = ?
         WHERE instance_id = ? AND file_id = ?",
    )
    .bind(untracked_file_id)
    .bind(Utc::now().timestamp())
    .bind(instance_id)
    .bind(tracked_file_id)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(false);
    }

    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND id = ?",
    )
    .bind(instance_id)
    .bind(tracked_file_id)
    .execute(&mut **tx)
    .await?;

    Ok(true)
}

pub(crate) async fn remove_instance_file_by_relative_path(
    instance_id: &str,
    relative_path: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query!(
        "
        DELETE FROM instance_files
        WHERE instance_id = ? AND relative_path = ?
        ",
        instance_id,
        relative_path,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Whether the instance has a present file that no content entry claims.
pub(crate) async fn has_unclassified_present_file(
    instance_id: &str,
    content_set_id: &str,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let unclassified: i64 = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM instance_files file
            LEFT JOIN instance_content_entries entry
                ON entry.content_set_id = ? AND entry.file_id = file.id
            WHERE file.instance_id = ? AND file.missing = 0
                AND entry.id IS NULL
        )",
    )
    .bind(content_set_id)
    .bind(instance_id)
    .fetch_one(pool)
    .await?;

    Ok(unclassified != 0)
}

/// Reads the instance's present files with the entry that already claims each
/// of them, as the pack snapshot needs them to classify overrides.
pub(crate) async fn get_present_files_with_entry<'e, E>(
    instance_id: &str,
    content_set_id: &str,
    exec: E,
) -> crate::Result<Vec<(String, String, bool, Option<String>)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        "SELECT file.id AS file_id, file.relative_path, file.enabled,
            (
                SELECT entry.id
                FROM instance_content_entries entry
                WHERE entry.content_set_id = ? AND entry.file_id = file.id
                ORDER BY entry.modified_at DESC
                LIMIT 1
            ) AS entry_id
         FROM instance_files file
         WHERE file.instance_id = ? AND file.missing = 0",
    )
    .bind(content_set_id)
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("relative_path")?,
                row.try_get::<String, _>("file_id")?,
                row.try_get::<i64, _>("enabled")? != 0,
                row.try_get::<Option<String>, _>("entry_id")?,
            ))
        })
        .collect()
}

/// Drops one indexed file of an instance.
pub(crate) async fn delete_file_in_transaction(
    instance_id: &str,
    file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND id = ?",
    )
    .bind(instance_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records the path, name and enabled flag of a file that kept its identity
/// while its stored row is renamed in place.
pub(crate) async fn rename_file_in_place_in_transaction(
    instance_id: &str,
    relative_path: &str,
    final_relative_path: &str,
    file_name: &str,
    enabled: bool,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("UPDATE instance_files SET relative_path = ?, file_name = ?, enabled = ?, local_mod_data = NULL, icon_path = NULL WHERE instance_id = ? AND relative_path = ?")
        .bind(final_relative_path)
        .bind(file_name)
        .bind(enabled)
        .bind(instance_id)
        .bind(relative_path)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Drops one indexed file of an instance unless an entry still names it.
pub(crate) async fn delete_unreferenced_file_in_transaction(
    instance_id: &str,
    relative_path: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND relative_path = ?
           AND NOT EXISTS (
             SELECT 1 FROM instance_content_entries
             WHERE file_id = instance_files.id
           )",
    )
    .bind(instance_id)
    .bind(relative_path)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Drops one indexed file of an instance by its id unless an entry still names
/// it.
pub(crate) async fn delete_unreferenced_file_by_id_in_transaction(
    instance_id: &str,
    file_id: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_files
         WHERE instance_id = ? AND id = ?
           AND NOT EXISTS (
               SELECT 1 FROM instance_content_entries
               WHERE file_id = ?
           )",
    )
    .bind(instance_id)
    .bind(file_id)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Moves the file an entry claims to its new path and name, leaving a file at
/// any other path alone.
pub(crate) async fn rename_entry_file_in_transaction(
    instance_id: &str,
    entry_id: &str,
    current_relative_path: &str,
    target_relative_path: &str,
    file_name: &str,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_files SET relative_path = ?, file_name = ?
         WHERE id = (SELECT file_id FROM instance_content_entries WHERE id = ?)
           AND instance_id = ? AND relative_path = ?",
    )
    .bind(target_relative_path)
    .bind(file_name)
    .bind(entry_id)
    .bind(instance_id)
    .bind(current_relative_path)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Records the metadata read from a file's own bytes, dropping the icon that
/// described the bytes it replaced.
pub(crate) async fn set_local_mod_data_in_transaction(
    file_id: &str,
    local_mod_data: Option<&str>,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE instance_files SET local_mod_data = ?, icon_path = NULL WHERE id = ?",
    )
    .bind(local_mod_data)
    .bind(file_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Reads the metadata stored for files that carry exactly these bytes.
pub(crate) async fn get_local_mod_data_for_artifact<'e, E>(
    sha1: &str,
    size: i64,
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_scalar(
        "SELECT local_mod_data FROM instance_files WHERE missing = 0 AND local_mod_data IS NOT NULL AND sha1 = ? COLLATE NOCASE AND size = ?",
    )
    .bind(sha1)
    .bind(size)
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
