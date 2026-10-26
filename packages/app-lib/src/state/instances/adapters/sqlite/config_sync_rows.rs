#![allow(dead_code)]

use chrono::Utc;
use sqlx::{Executor, Sqlite, SqlitePool};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct InstanceConfigSyncRow {
    pub instance_id: String,
    pub path: String,
    pub config_updated_at: Option<i64>,
    pub generated_at: Option<i64>,
}

pub(crate) async fn upsert_config_updated_at<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let now = Utc::now().timestamp();

    sqlx::query(
        "
        INSERT INTO instance_config_sync_state (
            instance_id,
            config_updated_at
        )
        VALUES (?, ?)
        ON CONFLICT (instance_id) DO UPDATE SET
            config_updated_at = excluded.config_updated_at
        ",
    )
    .bind(instance_id)
    .bind(now)
    .execute(exec)
    .await?;

    Ok(())
}

pub(crate) async fn mark_all_config_dirty<'e, E>(exec: E) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let now = Utc::now().timestamp();

    sqlx::query(
        "
        UPDATE instance_config_sync_state
        SET config_updated_at = ?
        ",
    )
    .bind(now)
    .execute(exec)
    .await?;

    Ok(())
}

pub(crate) async fn get_config_sync_generated_at<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<Option<i64>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let generated_at: Option<Option<i64>> = sqlx::query_scalar(
        "
        SELECT generated_at
        FROM instance_config_sync_state
        WHERE instance_id = ?
        ",
    )
    .bind(instance_id)
    .fetch_optional(exec)
    .await?;

    Ok(generated_at.flatten())
}

pub(crate) async fn update_config_sync_generated_at<'e, E>(
    instance_id: &str,
    generated_at: i64,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "
        INSERT INTO instance_config_sync_state (
            instance_id,
            config_updated_at,
            generated_at
        )
        VALUES (?, ?, ?)
        ON CONFLICT (instance_id) DO UPDATE SET
            generated_at = excluded.generated_at
        ",
    )
    .bind(instance_id)
    .bind(generated_at)
    .bind(generated_at)
    .execute(exec)
    .await?;

    Ok(())
}

pub(crate) async fn list_instance_config_sync_rows(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstanceConfigSyncRow>> {
    let rows = sqlx::query_as::<_, InstanceConfigSyncRow>(
        "
        SELECT
            i.id AS instance_id,
            i.path AS path,
            s.config_updated_at AS config_updated_at,
            s.generated_at AS generated_at
        FROM instances i
        LEFT JOIN instance_config_sync_state s
            ON s.instance_id = i.id
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
