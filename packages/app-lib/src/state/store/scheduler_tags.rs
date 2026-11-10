//! Scheduling tags kept across a restart, as data. Reading one back grants
//! nothing. Startup only deletes the rows whose owner is gone or finished;
//! re-minting a
//! fresh tag for a surviving owner and re-deriving its level
//! (`scheduler::tag::TagRegistry::reconcile`) arrive with the supervisor's
//! restart path, and no production path writes a tag before then.

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

fn level_of(name: &str) -> Option<Level> {
    Some(match name {
        "l0" => Level::L0,
        "l1" => Level::L1,
        "l2" => Level::L2,
        "l3" => Level::L3,
        "l4" => Level::L4,
        _ => return None,
    })
}

type Row = (i64, i64, String, String, String, i64);

/// The tag a row describes, or nothing when its level is not one this build
/// knows. The schema's CHECK only guards new writes, so an older or edited
/// database can still hold such a row; one of them must not stop the rest
/// from being read.
fn record_of(row: Row) -> Option<(SchedulerTagKey, SchedulerTagRecord)> {
    let (id, generation, level, origin, owner, created_at) = row;
    let Some(level) = level_of(&level) else {
        tracing::warn!(
            id,
            %level,
            %owner,
            "A scheduling tag carries a level this build does not know"
        );
        return None;
    };
    Some((
        id,
        SchedulerTagRecord {
            generation,
            level,
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

    /// `Ok(None)` means the key holds no row. A row that exists but carries a
    /// level this build does not know is an error instead, so a caller cannot
    /// mistake it for an absent key and rebuild what it could not read.
    /// `read_all` still logs and skips such a row: one of them must not stop
    /// the rest of the collection from being read.
    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<Row> =
            sqlx::query_as(&format!("{SELECT} WHERE id = ?"))
                .bind(key)
                .fetch_optional(&mut *conn)
                .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        match record_of(row) {
            Some((_, record)) => Ok(Some(record)),
            None => Err(crate::ErrorKind::InputError(format!(
                "Scheduling tag {key} has a level this build does not know"
            ))
            .into()),
        }
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
        Ok(rows.into_iter().filter_map(record_of).collect())
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
