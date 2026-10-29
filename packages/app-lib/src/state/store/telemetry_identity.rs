//! The installation the launcher reports telemetry as.

use sqlx::SqliteConnection;

use super::Collection;

/// The telemetry identity, kept once for the whole installation.
pub(crate) struct TelemetryIdentity;

/// There is one identity row per installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Installation;

/// What the identity says about this installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TelemetryIdentityRecord {
    pub installation_id: String,
    pub last_heartbeat_day: Option<String>,
}

impl Collection for TelemetryIdentity {
    type Key = Installation;
    type Scope = Installation;
    type Record = TelemetryIdentityRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "
            SELECT installation_id, last_heartbeat_day
            FROM telemetry_identity
            WHERE id = 0
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(installation_id, last_heartbeat_day)| Self::Record {
                installation_id,
                last_heartbeat_day,
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM telemetry_identity WHERE id = 0")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, &Installation)
            .await?
            .map(|record| vec![(Installation, record)])
            .unwrap_or_default())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        if records.is_empty() {
            sqlx::query("DELETE FROM telemetry_identity")
                .execute(&mut *conn)
                .await?;
            return Ok(());
        }

        for (_, record) in records {
            write_row(conn, record).await?;
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
    record: &TelemetryIdentityRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO telemetry_identity (
            id, installation_id, last_heartbeat_day
        )
        VALUES (0, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            installation_id = excluded.installation_id,
            last_heartbeat_day = excluded.last_heartbeat_day
        ",
    )
    .bind(&record.installation_id)
    .bind(&record.last_heartbeat_day)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
