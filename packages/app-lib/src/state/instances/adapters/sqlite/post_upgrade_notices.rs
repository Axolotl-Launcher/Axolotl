use crate::state::InstancePostUpgradeNotice;
use sqlx::{Executor, Row, Sqlite, SqlitePool};

/// Reads the notice an upgraded instance still owes the user.
pub(crate) async fn get_post_upgrade_notice(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstancePostUpgradeNotice>> {
    let row = sqlx::query(
        "SELECT upgrade_job_id, target_game_version, consecutive_clean_launches, warnings_json FROM instance_post_upgrade_notices WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let warnings_json: String = row.try_get("warnings_json")?;
        Ok(InstancePostUpgradeNotice {
            instance_id: instance_id.to_string(),
            upgrade_job_id: row.try_get("upgrade_job_id")?,
            target_game_version: row.try_get("target_game_version")?,
            consecutive_clean_launches: row
                .try_get::<i64, _>("consecutive_clean_launches")?
                .clamp(0, u8::MAX as i64)
                as u8,
            warnings: serde_json::from_str(&warnings_json)?,
        })
    })
    .transpose()
}

/// Stores the notice, or drops it when it carries no warnings.
pub(crate) async fn replace_post_upgrade_notice<'e, E>(
    notice: &InstancePostUpgradeNotice,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    if notice.warnings.is_empty() {
        dismiss_stored_notice(&notice.instance_id, exec).await
    } else {
        store_notice(notice, exec).await
    }
}

async fn dismiss_stored_notice<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "DELETE FROM instance_post_upgrade_notices WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(exec)
    .await?;

    Ok(())
}

async fn store_notice<'e, E>(
    notice: &InstancePostUpgradeNotice,
    exec: E,
) -> crate::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO instance_post_upgrade_notices (instance_id, upgrade_job_id, target_game_version, consecutive_clean_launches, warnings_json) VALUES (?, ?, ?, ?, ?) ON CONFLICT(instance_id) DO UPDATE SET upgrade_job_id = excluded.upgrade_job_id, target_game_version = excluded.target_game_version, consecutive_clean_launches = excluded.consecutive_clean_launches, warnings_json = excluded.warnings_json, modified = CURRENT_TIMESTAMP",
    )
    .bind(&notice.instance_id)
    .bind(&notice.upgrade_job_id)
    .bind(&notice.target_game_version)
    .bind(i64::from(notice.consecutive_clean_launches))
    .bind(serde_json::to_string(&notice.warnings)?)
    .execute(exec)
    .await?;

    Ok(())
}

/// Drops the notice an instance no longer owes.
pub(crate) async fn dismiss_post_upgrade_notice(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM instance_post_upgrade_notices WHERE instance_id = ?",
    )
    .bind(instance_id)
    .execute(pool)
    .await?;

    Ok(())
}
