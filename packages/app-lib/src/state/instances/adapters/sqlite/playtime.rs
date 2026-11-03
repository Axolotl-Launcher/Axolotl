use crate::state::instances::{
    DailyPlaytime, DailyPlaytimeEntry, playtime_to_storage,
};
use chrono::NaiveDate;
use sqlx::SqlitePool;

/// Reads the playtime totals of each day in the range.
pub(crate) async fn get_daily_playtime(
    start_date: NaiveDate,
    end_date: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytime>> {
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

/// Reads each instance's share of one day's playtime.
pub(crate) async fn get_daily_playtime_details(
    played_on: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytimeEntry>> {
    let date = played_on.format("%Y-%m-%d").to_string();
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

/// Adds playtime to the instance's row for one day, creating the row when the
/// day has none yet. A stored total that would overflow saturates instead.
pub(crate) async fn upsert_daily_playtime(
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

/// Adds playtime that has not been submitted yet; zero is a no-op.
pub(crate) async fn add_recent_playtime(
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
    let modified = chrono::Utc::now().timestamp();

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

/// Moves the unsubmitted playtime into the submitted total; zero is a no-op.
pub(crate) async fn mark_playtime_submitted(
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
    let modified = chrono::Utc::now().timestamp();

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
