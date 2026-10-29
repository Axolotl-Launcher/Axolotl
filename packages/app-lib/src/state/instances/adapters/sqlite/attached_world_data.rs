//! What the launcher has recorded about a world it found in an instance.
//!
//! One row per world an instance holds: how it should be displayed, and — for a
//! world that came from a project — which project and kind of content it is.
//! The columns are only ever written one at a time, so each write keeps the
//! other two as the row already had them.

use sqlx::{Executor, Sqlite};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct AttachedWorldDataRow {
    pub world_type: String,
    pub world_id: String,
    pub display_status: String,
    pub project_id: Option<String>,
    pub content_kind: Option<String>,
}

pub(crate) async fn get_for_world<'e, E>(
    instance_id: &str,
    world_type: &str,
    world_id: &str,
    exec: E,
) -> crate::Result<Option<AttachedWorldDataRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "
        SELECT display_status, project_id, content_kind
        FROM attached_world_data
        WHERE instance_id = ? and world_type = ? and world_id = ?
        ",
    )
    .bind(instance_id)
    .bind(world_type)
    .bind(world_id)
    .fetch_optional(exec)
    .await?;

    Ok(row.map(|(display_status, project_id, content_kind)| {
        AttachedWorldDataRow {
            world_type: world_type.to_string(),
            world_id: world_id.to_string(),
            display_status,
            project_id,
            content_kind,
        }
    }))
}

pub(crate) async fn get_all_for_instance<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Vec<AttachedWorldDataRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query_as::<_, AttachedWorldDataRow>(
        "
        SELECT world_type, world_id, display_status, project_id, content_kind
        FROM attached_world_data
        WHERE instance_id = ?
        ",
    )
    .bind(instance_id)
    .fetch_all(exec)
    .await?;

    Ok(rows)
}

/// Forgets what was recorded about one world.
pub(crate) async fn remove_for_world<'e, E>(
    instance_id: &str,
    world_type: &str,
    world_id: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "
        DELETE FROM attached_world_data
        WHERE instance_id = ? and world_type = ? and world_id = ?
        ",
    )
    .bind(instance_id)
    .bind(world_type)
    .bind(world_id)
    .execute(exec)
    .await?;

    Ok(())
}

// The three writes below answer a single upsert each, so they take the
// caller's connection instead of a unit it opened: every caller of theirs holds
// a pool and nothing else to put in the same unit.
pub(crate) async fn set_display_status<'e, E>(
    instance_id: &str,
    world_type: &str,
    world_id: &str,
    display_status: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "
        INSERT INTO attached_world_data (instance_id, world_type, world_id, display_status)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (instance_id, world_type, world_id) DO UPDATE
            SET display_status = excluded.display_status
        ",
    )
    .bind(instance_id)
    .bind(world_type)
    .bind(world_id)
    .bind(display_status)
    .execute(exec)
    .await?;

    Ok(())
}

pub(crate) async fn set_project_id<'e, E>(
    instance_id: &str,
    world_type: &str,
    world_id: &str,
    project_id: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "
        INSERT INTO attached_world_data (instance_id, world_type, world_id, project_id)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (instance_id, world_type, world_id) DO UPDATE
            SET project_id = excluded.project_id
        ",
    )
    .bind(instance_id)
    .bind(world_type)
    .bind(world_id)
    .bind(project_id)
    .execute(exec)
    .await?;

    Ok(())
}

pub(crate) async fn set_content_kind<'e, E>(
    instance_id: &str,
    world_type: &str,
    world_id: &str,
    content_kind: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "
        INSERT INTO attached_world_data (instance_id, world_type, world_id, content_kind)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (instance_id, world_type, world_id) DO UPDATE
            SET content_kind = excluded.content_kind
        ",
    )
    .bind(instance_id)
    .bind(world_type)
    .bind(world_id)
    .bind(content_kind)
    .execute(exec)
    .await?;

    Ok(())
}

#[cfg(test)]
#[path = "attached_world_data_tests.rs"]
mod tests;
