use crate::state::instances::{
    DailyPlaytime, DailyPlaytimeEntry, playtime_to_storage,
};
use chrono::NaiveDate;
use futures::StreamExt;
use sqlx::SqlitePool;

/// Reads the playtime totals of each day in the range.
async fn get_daily_playtime_in(
    start_date: NaiveDate,
    end_date: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytime>> {
    let start_date = start_date.format("%Y-%m-%d").to_string();
    let end_date = end_date.format("%Y-%m-%d").to_string();
    let mut rows = sqlx::query_as::<_, (String, String, i64, i64)>(
        r#"
        SELECT played_on, instance_name, played_seconds, session_count
        FROM instance_daily_playtime
        WHERE played_on BETWEEN ? AND ?
            AND typeof(played_on) = 'text'
            AND typeof(instance_id) = 'text'
            AND typeof(instance_name) = 'text'
            AND typeof(played_seconds) = 'integer' AND played_seconds >= 0
            AND typeof(session_count) = 'integer' AND session_count >= 0
        ORDER BY played_on
        "#,
    )
    .bind(start_date)
    .bind(end_date)
    .fetch(pool);

    // Aggregate in one pass. SQLite's signed SUM can fail the entire range
    // even when a day's total still fits the public unsigned result type.
    let mut totals: Vec<DailyPlaytime> = Vec::new();
    let mut top_seconds = 0;
    let mut undecodable = 0;
    while let Some(row) = rows.next().await {
        let (date, name, seconds, count) = match row {
            Ok(row) => row,
            Err(sqlx::Error::ColumnDecode { .. }) => {
                undecodable += 1;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if totals.last().is_none_or(|daily| daily.date != date) {
            totals.push(DailyPlaytime {
                date,
                played_seconds: 0,
                session_count: 0,
                top_instance_name: None,
            });
            top_seconds = 0;
        }
        let daily = totals.last_mut().expect("the current day is present");
        let seconds = seconds as u64;
        daily.played_seconds = daily.played_seconds.saturating_add(seconds);
        daily.session_count = daily.session_count.saturating_add(count as u64);
        if seconds > 0
            && (seconds > top_seconds
                || (seconds == top_seconds
                    && daily
                        .top_instance_name
                        .as_ref()
                        .is_some_and(|top| name < *top)))
        {
            daily.top_instance_name = Some(name);
            top_seconds = seconds;
        }
    }
    if undecodable > 0 {
        tracing::warn!(undecodable, "Skipped unreadable daily playtime rows");
    }
    Ok(totals)
}

/// Reads each instance's share of one day's playtime.
async fn get_daily_playtime_details_in(
    played_on: NaiveDate,
    pool: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytimeEntry>> {
    let date = played_on.format("%Y-%m-%d").to_string();
    let mut rows = sqlx::query_as::<_, (String, String, i64, i64)>(
        r#"
        SELECT instance_id, instance_name, played_seconds, session_count
        FROM instance_daily_playtime
        WHERE played_on = ?
            AND typeof(instance_id) = 'text'
            AND typeof(instance_name) = 'text'
            AND typeof(played_seconds) = 'integer' AND played_seconds > 0
            AND typeof(session_count) = 'integer' AND session_count >= 0
        ORDER BY played_seconds DESC, instance_name ASC, instance_id ASC
        "#,
    )
    .bind(date)
    .fetch(pool);

    let mut entries = Vec::new();
    let mut undecodable = 0;
    while let Some(row) = rows.next().await {
        let (instance_id, instance_name, seconds, count) = match row {
            Ok(row) => row,
            Err(sqlx::Error::ColumnDecode { .. }) => {
                undecodable += 1;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        entries.push(DailyPlaytimeEntry {
            instance_id,
            instance_name,
            played_seconds: seconds as u64,
            session_count: count as u64,
        });
    }
    if undecodable > 0 {
        tracing::warn!(undecodable, "Skipped unreadable daily playtime rows");
    }
    Ok(entries)
}

/// Adds playtime to the instance's row for one day, creating the row when the
/// day has none yet. A stored total that would overflow saturates instead.
async fn upsert_daily_playtime_in(
    instance_id: &str,
    instance_name: &str,
    played_on: NaiveDate,
    played_seconds: u64,
    session_count: u64,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
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
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(())
}

/// Adds locally recorded playtime while preserving the historical submitted total.
pub(crate) async fn add_recent_playtime(
    instance_id: &str,
    seconds: u64,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
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
    .execute(&mut *transaction)
    .await?;
    crate::state::store::configuration::commit(transaction, pool).await?;

    Ok(())
}

/// Daily history is optional; a damaged history file cannot disable instances.
pub(crate) async fn get_daily_playtime(
    start: NaiveDate,
    end: NaiveDate,
    core: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytime>> {
    let Some(pool) = crate::state::store::records_db::pool_for(core).await
    else {
        return Ok(Vec::new());
    };
    match get_daily_playtime_in(start, end, &pool).await {
        Ok(rows) => Ok(rows),
        Err(error) => {
            tracing::warn!(%error,"Daily history unavailable");
            Ok(Vec::new())
        }
    }
}
pub(crate) async fn get_daily_playtime_details(
    day: NaiveDate,
    core: &SqlitePool,
) -> crate::Result<Vec<DailyPlaytimeEntry>> {
    let Some(pool) = crate::state::store::records_db::pool_for(core).await
    else {
        return Ok(Vec::new());
    };
    match get_daily_playtime_details_in(day, &pool).await {
        Ok(rows) => Ok(rows),
        Err(error) => {
            tracing::warn!(%error,"Daily history unavailable");
            Ok(Vec::new())
        }
    }
}
pub(crate) async fn upsert_daily_playtime(
    id: &str,
    name: &str,
    day: NaiveDate,
    seconds: u64,
    count: u64,
    core: &SqlitePool,
) -> crate::Result<()> {
    playtime_to_storage(seconds, "played_seconds")?;
    playtime_to_storage(count, "session_count")?;
    let Some(pool) = crate::state::store::records_db::pool_for(core).await
    else {
        return Ok(());
    };
    if let Err(error) =
        upsert_daily_playtime_in(id, name, day, seconds, count, &pool).await
    {
        tracing::warn!(%error,"Daily playtime could not be recorded; core state remains available");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn history_with(insert: &'static str) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE instance_daily_playtime (
                played_on TEXT NOT NULL, instance_id TEXT NOT NULL,
                instance_name TEXT NOT NULL, played_seconds INTEGER NOT NULL,
                session_count INTEGER NOT NULL,
                PRIMARY KEY (played_on, instance_id)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(insert).execute(&pool).await.unwrap();
        pool
    }

    fn day(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    #[tokio::test]
    async fn malformed_history_does_not_hide_valid_records() {
        let pool = history_with(
            "INSERT INTO instance_daily_playtime VALUES
                ('2026-11-15', '1', 'Zulu', 30, 1),
                ('2026-11-15', '2', 'Alpha', 30, 2),
                ('2026-11-15', '3', 'Negative time', -90, 1),
                ('2026-11-15', '4', 'Negative count', 90, -1),
                ('2026-11-15', '5', 'Text time', 'invalid', 1),
                ('2026-11-15', '6', 'Fractional count', 90, 0.5),
                ('2026-11-15', '7', CAST(X'FF' AS TEXT), 100, 1),
                ('2026-11-15', '8', 'After malformed row', 1, 1),
                ('2026-11-15', '9', X'61', 90, 1),
                ('2026-11-16', '1', 'Zero time', 0, 1)",
        )
        .await;

        let totals = get_daily_playtime_in(day(15), day(16), &pool)
            .await
            .unwrap();
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].played_seconds, 61);
        assert_eq!(totals[0].session_count, 4);
        assert_eq!(totals[0].top_instance_name.as_deref(), Some("Alpha"));
        assert_eq!(totals[1].played_seconds, 0);
        assert!(totals[1].top_instance_name.is_none());

        let details =
            get_daily_playtime_details_in(day(15), &pool).await.unwrap();
        assert_eq!(
            details
                .iter()
                .map(|entry| entry.instance_name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Zulu", "After malformed row"],
        );
        assert!(
            get_daily_playtime_details_in(day(16), &pool)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn summary_uses_unsigned_totals_and_saturates_only_the_overflow() {
        let pool = history_with(
            "INSERT INTO instance_daily_playtime VALUES
                ('2026-11-15', '1', 'Alpha', 9223372036854775807, 1),
                ('2026-11-15', '2', 'Beta', 9223372036854775807, 2),
                ('2026-11-15', '3', 'Gamma', 5, 3),
                ('2026-11-16', '1', 'Alpha', 9223372036854775807, 9223372036854775807),
                ('2026-11-16', '2', 'Beta', 5, 5),
                ('2026-11-17', '1', 'Alpha', 10, 1)",
        )
        .await;

        let totals = get_daily_playtime_in(day(15), day(17), &pool)
            .await
            .unwrap();
        assert_eq!(
            totals
                .iter()
                .map(|entry| (entry.played_seconds, entry.session_count))
                .collect::<Vec<_>>(),
            [
                (u64::MAX, 6),
                (i64::MAX as u64 + 5, i64::MAX as u64 + 5),
                (10, 1)
            ],
        );
    }
}
