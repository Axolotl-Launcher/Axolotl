//! Scheduling tags kept across a restart, as data. Reading one back grants
//! nothing: startup re-mints a fresh tag for each owner that still exists and
//! re-derives its level (`scheduler::tag::TagRegistry::reconcile`).

use sqlx::SqliteConnection;

use super::Collection;
use crate::scheduler::tag::Level;

pub(crate) struct SchedulerTags;

/// A tag row is identified by the id it had when it was written.
pub(crate) type SchedulerTagKey = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SchedulerTagRecord {
    pub generation: i64,
    pub level: Level,
    pub origin: String,
    /// What the tag labelled, such as an install job id.
    pub owner: String,
    pub created_at: i64,
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::L0 => "l0",
        Level::L1 => "l1",
        Level::L2 => "l2",
        Level::L3 => "l3",
        Level::L4 => "l4",
    }
}

fn level_of(name: &str) -> crate::Result<Level> {
    Ok(match name {
        "l0" => Level::L0,
        "l1" => Level::L1,
        "l2" => Level::L2,
        "l3" => Level::L3,
        "l4" => Level::L4,
        other => {
            return Err(crate::ErrorKind::OtherError(format!(
                "Unknown scheduling level in scheduler_tags: {other}"
            ))
            .into());
        }
    })
}

type Row = (i64, i64, String, String, String, i64);

fn record_of(row: Row) -> crate::Result<(SchedulerTagKey, SchedulerTagRecord)> {
    let (id, generation, level, origin, owner, created_at) = row;
    Ok((
        id,
        SchedulerTagRecord {
            generation,
            level: level_of(&level)?,
            origin,
            owner,
            created_at,
        },
    ))
}

const SELECT: &str = "SELECT id, generation, level, origin, owner, created_at FROM scheduler_tags";

impl Collection for SchedulerTags {
    type Key = SchedulerTagKey;
    type Scope = SchedulerTagKey;
    type Record = SchedulerTagRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<Row> =
            sqlx::query_as(&format!("{SELECT} WHERE id = ?"))
                .bind(key)
                .fetch_optional(&mut *conn)
                .await?;
        row.map(record_of)
            .transpose()
            .map(|row| row.map(|(_, record)| record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM scheduler_tags WHERE id = ?")
                .bind(key)
                .execute(&mut *conn)
                .await?;
            return Ok(());
        };
        write_row(conn, *key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<Row> = sqlx::query_as(&format!("{SELECT} ORDER BY id"))
            .fetch_all(&mut *conn)
            .await?;
        rows.into_iter().map(record_of).collect()
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM scheduler_tags")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, *key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(*scope, record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: i64,
    record: &SchedulerTagRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO scheduler_tags (id, generation, level, origin, owner, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            generation = excluded.generation,
            level = excluded.level,
            origin = excluded.origin,
            owner = excluded.owner,
            created_at = excluded.created_at
        ",
    )
    .bind(key)
    .bind(record.generation)
    .bind(level_name(record.level))
    .bind(&record.origin)
    .bind(&record.owner)
    .bind(record.created_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
