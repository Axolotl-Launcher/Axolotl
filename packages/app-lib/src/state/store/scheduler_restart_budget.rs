//! Failed restarts or rebuilds per scope since its last success, so the
//! cumulative limits of the rebuild protocol survive a launcher restart.
// Eviction slots write this once they serve a real L1 service; until then
// only the tests use it.
#![cfg_attr(not(test), allow(dead_code))]

use sqlx::SqliteConnection;

use super::Collection;

pub(crate) struct SchedulerRestartBudget;

/// The work a budget belongs to, such as an L1 slot's name.
pub(crate) type RestartBudgetKey = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestartBudgetRecord {
    pub failures: i64,
    /// Milliseconds since the Unix epoch; `None` once the work is live again.
    pub first_failure_at: Option<i64>,
    pub revoked: bool,
    pub updated_at: i64,
}

type Row = (String, i64, Option<i64>, bool, i64);

fn record_of(
    (scope, failures, first_failure_at, revoked, updated_at): Row,
) -> (RestartBudgetKey, RestartBudgetRecord) {
    (
        scope,
        RestartBudgetRecord {
            failures,
            first_failure_at,
            revoked,
            updated_at,
        },
    )
}

const SELECT: &str = "SELECT scope, failures, first_failure_at, revoked, updated_at FROM scheduler_restart_budget";

impl Collection for SchedulerRestartBudget {
    type Key = RestartBudgetKey;
    type Scope = RestartBudgetKey;
    type Record = RestartBudgetRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<Row> =
            sqlx::query_as(&format!("{SELECT} WHERE scope = ?"))
                .bind(key)
                .fetch_optional(&mut *conn)
                .await?;
        Ok(row.map(|row| record_of(row).1))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM scheduler_restart_budget WHERE scope = ?")
                .bind(key)
                .execute(&mut *conn)
                .await?;
            return Ok(());
        };
        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<Row> =
            sqlx::query_as(&format!("{SELECT} ORDER BY scope"))
                .fetch_all(&mut *conn)
                .await?;
        Ok(rows.into_iter().map(record_of).collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM scheduler_restart_budget")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(scope.clone(), record)])
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
    key: &str,
    record: &RestartBudgetRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO scheduler_restart_budget (scope, failures, first_failure_at, revoked, updated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (scope) DO UPDATE SET
            failures = excluded.failures,
            first_failure_at = excluded.first_failure_at,
            revoked = excluded.revoked,
            updated_at = excluded.updated_at
        ",
    )
    .bind(key)
    .bind(record.failures)
    .bind(record.first_failure_at)
    .bind(record.revoked)
    .bind(record.updated_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
