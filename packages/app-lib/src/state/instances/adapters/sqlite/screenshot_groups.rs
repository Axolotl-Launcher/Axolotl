//! The named groups the screenshots of an instance can be filed under.

use sqlx::{Executor, Sqlite, Transaction};

/// Creates a group at the front of the list, pushing the rest back.
pub(crate) async fn create_group(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    name: &str,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE screenshot_groups SET display_order = display_order + 1",
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO screenshot_groups (id, name, display_order) VALUES (?, ?, 0)",
    )
    .bind(id)
    .bind(name)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Renames a group, reporting whether it exists.
pub(crate) async fn rename_group(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    name: &str,
) -> crate::Result<bool> {
    let result =
        sqlx::query("UPDATE screenshot_groups SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&mut **tx)
            .await?;

    Ok(result.rows_affected() != 0)
}

/// Deletes a group, reporting whether it existed.
pub(crate) async fn delete_group(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> crate::Result<bool> {
    let result = sqlx::query("DELETE FROM screenshot_groups WHERE id = ?")
        .bind(id)
        .execute(&mut **tx)
        .await?;

    Ok(result.rows_affected() != 0)
}

/// The instances whose screenshots `group_id` holds.
pub(crate) async fn get_group_instance_ids<'e, E>(
    group_id: &str,
    exec: E,
) -> crate::Result<Vec<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<String> = sqlx::query_scalar(
        "
        SELECT DISTINCT screenshots.instance_id
        FROM screenshot_group_memberships memberships
        INNER JOIN screenshots ON screenshots.id = memberships.screenshot_id
        WHERE memberships.group_id = ?
        ",
    )
    .bind(group_id)
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

/// Whether `group_id` names a group.
pub(crate) async fn group_exists<'e, E>(
    group_id: &str,
    exec: E,
) -> crate::Result<bool>
where
    E: Executor<'e, Database = Sqlite>,
{
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM screenshot_groups WHERE id = ?)",
    )
    .bind(group_id)
    .fetch_one(exec)
    .await?;

    Ok(exists)
}

/// Files one screenshot under `group_id`, or files it nowhere when that is
/// `None`.
pub(crate) async fn set_membership(
    tx: &mut Transaction<'_, Sqlite>,
    screenshot_id: &str,
    group_id: Option<&str>,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM screenshot_group_memberships WHERE screenshot_id = ?",
    )
    .bind(screenshot_id)
    .execute(&mut **tx)
    .await?;

    if let Some(group_id) = group_id {
        sqlx::query(
            "INSERT INTO screenshot_group_memberships (screenshot_id, group_id) VALUES (?, ?)",
        )
        .bind(screenshot_id)
        .bind(group_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Files one screenshot under `group_id`, replacing the group it was in.
pub(crate) async fn upsert_membership(
    tx: &mut Transaction<'_, Sqlite>,
    screenshot_id: &str,
    group_id: &str,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO screenshot_group_memberships (screenshot_id, group_id)
        VALUES (?, ?)
        ON CONFLICT(screenshot_id) DO UPDATE SET group_id = excluded.group_id
        ",
    )
    .bind(screenshot_id)
    .bind(group_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// The instance the screenshot `screenshot_id` belongs to.
pub(crate) async fn get_screenshot_instance_id<'e, E>(
    screenshot_id: &str,
    exec: E,
) -> crate::Result<Option<String>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let instance_id: Option<String> =
        sqlx::query_scalar("SELECT instance_id FROM screenshots WHERE id = ?")
            .bind(screenshot_id)
            .fetch_optional(exec)
            .await?;

    Ok(instance_id)
}

/// The display order an imported group is appended after.
pub(crate) async fn get_next_display_order<'e, E>(exec: E) -> crate::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(display_order) + 1, 0) FROM screenshot_groups",
    )
    .fetch_one(exec)
    .await?;

    Ok(order)
}

/// Stores an imported group, keeping the name already known under `id`.
pub(crate) async fn upsert_imported_group(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    name: &str,
    display_order: i64,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO screenshot_groups (id, name, display_order)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET name = excluded.name
        ",
    )
    .bind(id)
    .bind(name)
    .bind(display_order)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
