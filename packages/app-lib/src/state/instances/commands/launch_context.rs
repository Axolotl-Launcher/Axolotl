use crate::state::InstanceInstallStage;
use crate::state::instances::{
    InstanceLaunchContext, LoaderComponent,
    adapters::sqlite::{
        config_sync_rows, instance_rows, loader_component_rows,
    },
    config_sync, playtime_to_storage,
};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use sqlx::SqlitePool;

use crate::state::instances::{DailyPlaytime, DailyPlaytimeEntry};

pub(crate) async fn get_instance_launch_context(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Option<InstanceLaunchContext>> {
    instance_rows::get_instance_launch_context(instance_id, pool).await
}

pub(crate) async fn replace_instance_loader_components(
    instance_id: &str,
    components: &[LoaderComponent],
    pool: &SqlitePool,
) -> crate::Result<()> {
    let (loader, loader_version) =
        crate::state::project_loader_components(components)?;
    let mut tx = pool.begin().await?;
    loader_component_rows::replace_loader_components(
        instance_id,
        components,
        &mut tx,
    )
    .await?;
    sqlx::query(
        "UPDATE instance_content_sets
         SET loader = ?, loader_version = ?, modified = ?
         WHERE id = (
            SELECT applied_content_set_id FROM instances WHERE id = ?
         )",
    )
    .bind(loader.as_str())
    .bind(loader_version)
    .bind(Utc::now().timestamp())
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    config_sync_rows::upsert_config_updated_at(instance_id, &mut *tx).await?;
    tx.commit().await?;
    config_sync::mark_dirty(instance_id);
    Ok(())
}

pub(crate) async fn set_instance_install_stage(
    instance_id: &str,
    install_stage: InstanceInstallStage,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let install_stage = install_stage.as_str();
    let modified = Utc::now().timestamp();

    sqlx::query!(
        "
        UPDATE instances
        SET install_stage = ?, modified = ?
        WHERE id = ?
        ",
        install_stage,
        modified,
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn set_applied_content_set_loader_version(
    instance_id: &str,
    loader_version: Option<&str>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let modified = Utc::now().timestamp();
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "
        UPDATE instance_content_sets
        SET loader_version = ?, modified = ?
        WHERE id = (
            SELECT applied_content_set_id
            FROM instances
            WHERE id = ?
        )
        ",
        loader_version,
        modified,
        instance_id,
    )
    .execute(&mut *tx)
    .await?;
    config_sync_rows::upsert_config_updated_at(instance_id, &mut *tx).await?;
    tx.commit().await?;

    config_sync::mark_dirty(instance_id);

    Ok(())
}

pub(crate) async fn set_applied_content_set_protocol_version(
    instance_id: &str,
    protocol_version: Option<u32>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let protocol_version = protocol_version.map(i64::from);
    let modified = Utc::now().timestamp();
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "
        UPDATE instance_content_sets
        SET protocol_version = ?, modified = ?
        WHERE id = (
            SELECT applied_content_set_id
            FROM instances
            WHERE id = ?
        )
        ",
        protocol_version,
        modified,
        instance_id,
    )
    .execute(&mut *tx)
    .await?;
    config_sync_rows::upsert_config_updated_at(instance_id, &mut *tx).await?;
    tx.commit().await?;

    config_sync::mark_dirty(instance_id);

    Ok(())
}

pub(crate) async fn set_instance_last_played(
    instance_id: &str,
    last_played: DateTime<Utc>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let last_played = last_played.timestamp();
    let modified = Utc::now().timestamp();

    sqlx::query!(
        "
        UPDATE instances
        SET last_played = ?, modified = ?
        WHERE id = ?
        ",
        last_played,
        modified,
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn set_instance_pinned(
    instance_id: &str,
    pinned: bool,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let pinned_at = pinned.then(|| Utc::now().timestamp());
    let modified = Utc::now().timestamp();
    let result = sqlx::query!(
        "
        UPDATE instances
        SET pinned_at = ?, modified = ?
        WHERE id = ?
        ",
        pinned_at,
        modified,
        instance_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(crate::ErrorKind::InputError(
            "Unknown instance".to_string(),
        )
        .into());
    }

    Ok(())
}

pub(crate) async fn record_instance_play_session(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let now = Utc::now();
    let instance_name = get_instance_name(instance_id, pool).await?;
    upsert_daily_playtime(
        instance_id,
        &instance_name,
        now.with_timezone(&Local).date_naive(),
        0,
        1,
        pool,
    )
    .await
}

pub(crate) async fn record_instance_daily_playtime(
    instance_id: &str,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if ended_at <= started_at {
        return Ok(());
    }

    let instance_name = get_instance_name(instance_id, pool).await?;
    for (played_on, elapsed) in
        split_daily_playtime(started_at, ended_at, &Local)?
    {
        upsert_daily_playtime(
            instance_id,
            &instance_name,
            played_on,
            elapsed,
            0,
            pool,
        )
        .await?;
    }

    Ok(())
}

fn split_daily_playtime<Tz: TimeZone>(
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    time_zone: &Tz,
) -> crate::Result<Vec<(NaiveDate, u64)>> {
    if ended_at <= started_at {
        return Ok(Vec::new());
    }

    let mut segment_start = started_at;
    let mut segments = Vec::new();
    while segment_start < ended_at {
        let local_start = segment_start.with_timezone(time_zone);
        let played_on = local_start.date_naive();
        let next_day = played_on.succ_opt().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unable to determine the next local day".to_string(),
            )
        })?;
        let next_midnight = next_day.and_hms_opt(0, 0, 0).ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Unable to determine the next local midnight".to_string(),
            )
        })?;
        let next_boundary = time_zone
            .from_local_datetime(&next_midnight)
            .earliest()
            .or_else(|| time_zone.from_local_datetime(&next_midnight).latest())
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Unable to resolve the next local day boundary".to_string(),
                )
            })?
            .with_timezone(&Utc);
        let segment_end = next_boundary.min(ended_at);
        let elapsed = (segment_end - segment_start).num_seconds();
        if elapsed <= 0 {
            return Err(crate::ErrorKind::InputError(
                "Unable to advance local playtime boundary".to_string(),
            )
            .into());
        }

        segments.push((played_on, elapsed as u64));
        segment_start = segment_end;
    }

    Ok(segments)
}

pub(crate) async fn get_daily_playtime(
    start_date: NaiveDate,
    end_date: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytime>> {
    if start_date > end_date {
        return Err(crate::ErrorKind::InputError(
            "Start date must not be after end date".to_string(),
        )
        .into());
    }

    let start_date = start_date.format("%Y-%m-%d").to_string();
    let end_date = end_date.format("%Y-%m-%d").to_string();
    let rows = sqlx::query!(
        r#"
        SELECT
            daily.played_on AS "date!: String",
            SUM(daily.played_seconds) AS "played_seconds!: i64",
            SUM(daily.session_count) AS "session_count!: i64",
            (
                SELECT candidate.instance_name
                FROM instance_daily_playtime candidate
                WHERE candidate.played_on = daily.played_on
                    AND candidate.played_seconds > 0
                ORDER BY candidate.played_seconds DESC, candidate.instance_name ASC
                LIMIT 1
            ) AS "top_instance_name?: String"
        FROM instance_daily_playtime daily
        WHERE daily.played_on BETWEEN ? AND ?
        GROUP BY daily.played_on
        ORDER BY daily.played_on
        "#,
        start_date,
        end_date,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(DailyPlaytime {
                date: row.date,
                played_seconds: u64::try_from(row.played_seconds).map_err(
                    |_| {
                        crate::ErrorKind::InputError(
                            "Invalid daily playtime value".to_string(),
                        )
                    },
                )?,
                session_count: u64::try_from(row.session_count).map_err(
                    |_| {
                        crate::ErrorKind::InputError(
                            "Invalid daily session count".to_string(),
                        )
                    },
                )?,
                top_instance_name: row.top_instance_name,
            })
        })
        .collect()
}

pub(crate) async fn get_daily_playtime_details(
    date: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytimeEntry>> {
    let date = date.format("%Y-%m-%d").to_string();
    let rows = sqlx::query!(
        r#"
        SELECT
            daily.instance_id AS "instance_id!: String",
            daily.instance_name AS "instance_name!: String",
            daily.played_seconds AS "played_seconds!: i64",
            daily.session_count AS "session_count!: i64"
        FROM instance_daily_playtime daily
        WHERE daily.played_on = ?
            AND daily.played_seconds > 0
        ORDER BY daily.played_seconds DESC, daily.instance_name ASC
        "#,
        date,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(DailyPlaytimeEntry {
                instance_id: row.instance_id,
                instance_name: row.instance_name,
                played_seconds: u64::try_from(row.played_seconds).map_err(
                    |_| {
                        crate::ErrorKind::InputError(
                            "Invalid daily playtime value".to_string(),
                        )
                    },
                )?,
                session_count: u64::try_from(row.session_count).map_err(
                    |_| {
                        crate::ErrorKind::InputError(
                            "Invalid daily session count".to_string(),
                        )
                    },
                )?,
            })
        })
        .collect()
}

async fn get_instance_name(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<String> {
    sqlx::query_scalar!(
        "
        SELECT name
        FROM instances
        WHERE id = ?
        ",
        instance_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown instance".to_string()).into()
    })
}

async fn upsert_daily_playtime(
    instance_id: &str,
    instance_name: &str,
    played_on: NaiveDate,
    played_seconds: u64,
    session_count: u64,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let played_seconds = playtime_to_storage(played_seconds, "played_seconds")?;
    let session_count = playtime_to_storage(session_count, "session_count")?;
    let played_on = played_on.format("%Y-%m-%d").to_string();
    let max_played_seconds_before_increment = i64::MAX - played_seconds;
    let max_session_count_before_increment = i64::MAX - session_count;

    sqlx::query!(
        "
        INSERT INTO instance_daily_playtime (
            played_on,
            instance_id,
            instance_name,
            played_seconds,
            session_count
        )
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(played_on, instance_id) DO UPDATE SET
            instance_name = excluded.instance_name,
            played_seconds = CASE
                WHEN instance_daily_playtime.played_seconds > ? THEN ?
                ELSE instance_daily_playtime.played_seconds + excluded.played_seconds
            END,
            session_count = CASE
                WHEN instance_daily_playtime.session_count > ? THEN ?
                ELSE instance_daily_playtime.session_count + excluded.session_count
            END
        ",
        played_on,
        instance_id,
        instance_name,
        played_seconds,
        session_count,
        max_played_seconds_before_increment,
        i64::MAX,
        max_session_count_before_increment,
        i64::MAX,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn add_instance_recent_playtime(
    instance_id: &str,
    seconds: u64,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if seconds == 0 {
        return Ok(());
    }

    let seconds = playtime_to_storage(seconds, "recent_time_played")?;
    let max_playtime = i64::MAX;
    let max_playtime_before_increment = max_playtime - seconds;
    let modified = Utc::now().timestamp();

    sqlx::query!(
        "
        UPDATE instances
        SET
            recent_time_played = CASE
                WHEN recent_time_played < 0 THEN ?
                WHEN recent_time_played > ? THEN ?
                ELSE recent_time_played + ?
            END,
            modified = ?
        WHERE id = ?
        ",
        seconds,
        max_playtime_before_increment,
        max_playtime,
        seconds,
        modified,
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn mark_instance_playtime_submitted(
    instance_id: &str,
    recent_time_played: u64,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if recent_time_played == 0 {
        return Ok(());
    }

    let recent_time_played =
        playtime_to_storage(recent_time_played, "recent_time_played")?;
    let max_playtime = i64::MAX;
    let max_playtime_before_increment = max_playtime - recent_time_played;
    let modified = Utc::now().timestamp();

    sqlx::query!(
        "
        UPDATE instances
        SET
            submitted_time_played = CASE
                WHEN submitted_time_played < 0 THEN ?
                WHEN submitted_time_played > ? THEN ?
                ELSE submitted_time_played + ?
            END,
            recent_time_played = 0,
            modified = ?
        WHERE id = ?
        ",
        recent_time_played,
        max_playtime_before_increment,
        max_playtime,
        recent_time_played,
        modified,
        instance_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use chrono_tz::America::New_York;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        get_daily_playtime, set_instance_pinned, split_daily_playtime,
        upsert_daily_playtime,
    };

    fn to_utc(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    ) -> chrono::DateTime<Utc> {
        New_York
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("valid New York local time")
            .with_timezone(&Utc)
    }

    #[test]
    fn splits_playtime_across_local_midnight() {
        let segments = split_daily_playtime(
            to_utc(2026, 2, 7, 23, 30),
            to_utc(2026, 2, 8, 1, 30),
            &New_York,
        )
        .expect("playtime should split");
        assert_eq!(
            segments,
            vec![
                ("2026-02-07".parse().unwrap(), 30 * 60),
                ("2026-02-08".parse().unwrap(), 90 * 60),
            ],
        );
    }

    #[test]
    fn measures_daylight_saving_transitions_by_elapsed_time() {
        let spring = split_daily_playtime(
            to_utc(2026, 3, 8, 0, 30),
            to_utc(2026, 3, 8, 3, 30),
            &New_York,
        )
        .expect("spring daylight saving playtime should split");
        let autumn = split_daily_playtime(
            to_utc(2026, 11, 1, 0, 30),
            to_utc(2026, 11, 1, 2, 30),
            &New_York,
        )
        .expect("autumn daylight saving playtime should split");

        assert_eq!(spring[0].1, 2 * 60 * 60);
        assert_eq!(autumn[0].1, 3 * 60 * 60);
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite pool");
        sqlx::query(
            "CREATE TABLE instances (id TEXT PRIMARY KEY, name TEXT NOT NULL, pinned_at INTEGER NULL, modified INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("instances table");
        sqlx::query(
            "CREATE TABLE instance_daily_playtime (played_on TEXT NOT NULL, instance_id TEXT NOT NULL, instance_name TEXT NOT NULL, played_seconds INTEGER NOT NULL DEFAULT 0, session_count INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (played_on, instance_id))",
        )
        .execute(&pool)
        .await
        .expect("daily playtime table");
        pool
    }

    #[tokio::test]
    async fn aggregates_sessions_and_selects_the_top_instance() {
        let pool = test_pool().await;
        upsert_daily_playtime(
            "first",
            "First instance",
            "2026-07-25".parse().unwrap(),
            60,
            1,
            &pool,
        )
        .await
        .unwrap();
        upsert_daily_playtime(
            "second",
            "Second instance",
            "2026-07-25".parse().unwrap(),
            240,
            2,
            &pool,
        )
        .await
        .unwrap();
        upsert_daily_playtime(
            "first",
            "First instance",
            "2026-07-25".parse().unwrap(),
            300,
            1,
            &pool,
        )
        .await
        .unwrap();

        let summary = get_daily_playtime(
            "2026-07-25".parse().unwrap(),
            "2026-07-25".parse().unwrap(),
            &pool,
        )
        .await
        .unwrap();
        assert_eq!(summary[0].played_seconds, 600);
        assert_eq!(summary[0].session_count, 4);
        assert_eq!(
            summary[0].top_instance_name.as_deref(),
            Some("First instance")
        );
    }

    #[tokio::test]
    async fn persists_pinned_state_without_erasing_playtime_history() {
        let pool = test_pool().await;
        sqlx::query("INSERT INTO instances (id, name, pinned_at, modified) VALUES ('instance', 'Instance', NULL, 0)")
            .execute(&pool)
            .await
            .unwrap();
        upsert_daily_playtime(
            "instance",
            "Instance",
            "2026-07-25".parse().unwrap(),
            120,
            1,
            &pool,
        )
        .await
        .unwrap();

        set_instance_pinned("instance", true, &pool).await.unwrap();
        let pinned_at: Option<i64> = sqlx::query_scalar(
            "SELECT pinned_at FROM instances WHERE id = 'instance'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pinned_at.is_some());
        set_instance_pinned("instance", false, &pool).await.unwrap();
        sqlx::query("DELETE FROM instances WHERE id = 'instance'")
            .execute(&pool)
            .await
            .unwrap();

        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_daily_playtime WHERE instance_id = 'instance'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 1);
    }
}
