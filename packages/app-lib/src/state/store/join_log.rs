//! When each server of an instance was last joined.

use chrono::{DateTime, TimeZone, Utc};

use sqlx::SqliteConnection;

use super::Collection;

/// The server join log, kept per instance and server address.
pub(crate) struct JoinLog;

/// One record of the log, identified by the instance and the server joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JoinLogKey {
    pub instance_id: String,
    pub host: String,
    pub port: u16,
}

/// Every server of one instance.
pub(crate) struct InstanceJoins {
    pub instance_id: String,
}

/// What a join left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JoinLogEntry {
    pub join_time: DateTime<Utc>,
}

impl Collection for JoinLog {
    type Key = JoinLogKey;
    type Scope = InstanceJoins;
    type Record = JoinLogEntry;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope.instance_id == key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let join_time: Option<i64> = sqlx::query_scalar(
            "
            SELECT join_time
            FROM join_log
            WHERE instance_id = ? AND host = ? AND port = ?
            ",
        )
        .bind(&key.instance_id)
        .bind(&key.host)
        .bind(i64::from(key.port))
        .fetch_optional(&mut *conn)
        .await?;

        Ok(join_time.map(|join_time| Self::Record {
            join_time: to_time(join_time),
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "
                DELETE FROM join_log
                WHERE instance_id = ? AND host = ? AND port = ?
                ",
            )
            .bind(&key.instance_id)
            .bind(&key.host)
            .bind(i64::from(key.port))
            .execute(&mut *conn)
            .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT instance_id, host, port, join_time FROM join_log",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, host, port, join_time)| {
                to_record(instance_id, host, port, join_time)
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM join_log")
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
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "
            SELECT host, port, join_time
            FROM join_log
            WHERE instance_id = ?
            ",
        )
        .bind(&scope.instance_id)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(host, port, join_time)| {
                to_record(scope.instance_id.clone(), host, port, join_time)
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM join_log WHERE instance_id = ?")
            .bind(&scope.instance_id)
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &JoinLogKey,
    record: &JoinLogEntry,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO join_log (instance_id, host, port, join_time)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (instance_id, host, port) DO UPDATE SET
            join_time = excluded.join_time
        ",
    )
    .bind(&key.instance_id)
    .bind(&key.host)
    .bind(i64::from(key.port))
    .bind(record.join_time.timestamp())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// The record a row describes, or nothing when its port does not fit the key.
fn to_record(
    instance_id: String,
    host: String,
    port: i64,
    join_time: i64,
) -> Option<(JoinLogKey, JoinLogEntry)> {
    let Some(port) = u16::try_from(port).ok() else {
        tracing::warn!(
            %instance_id,
            %host,
            port,
            "Ignoring a join log entry whose port is out of range"
        );
        return None;
    };
    Some((
        JoinLogKey {
            instance_id,
            host,
            port,
        },
        JoinLogEntry {
            join_time: to_time(join_time),
        },
    ))
}

fn to_time(timestamp: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now)
}
