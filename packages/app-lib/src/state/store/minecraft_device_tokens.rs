//! The launcher's device token for the Xbox Live service, kept as one row.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;

use super::Collection;

/// The device token, kept once for the whole launcher.
pub(crate) struct MinecraftDeviceTokens;

/// There is one device token per launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Singleton;

/// What a device token carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinecraftDeviceTokenRecord {
    pub uuid: String,
    pub private_key: String,
    pub x: String,
    pub y: String,
    pub issue_instant: i64,
    pub not_after: i64,
    pub token: String,
    /// The claims as the JSON text they are stored as.
    pub display_claims: String,
}

impl Collection for MinecraftDeviceTokens {
    type Key = Singleton;
    type Scope = Singleton;
    type Record = MinecraftDeviceTokenRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row = sqlx::query(
            "
            SELECT
                uuid, private_key, x, y, issue_instant, not_after, token,
                json(display_claims) AS display_claims
            FROM minecraft_device_tokens
            WHERE id = 0
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        row.map(to_record).transpose()
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM minecraft_device_tokens WHERE id = 0")
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, &Singleton)
            .await?
            .map(|record| vec![(Singleton, record)])
            .unwrap_or_default())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, &Singleton, records.first().map(|(_, record)| record))
            .await
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

fn to_record(row: SqliteRow) -> crate::Result<MinecraftDeviceTokenRecord> {
    Ok(MinecraftDeviceTokenRecord {
        uuid: row.try_get("uuid")?,
        private_key: row.try_get("private_key")?,
        x: row.try_get("x")?,
        y: row.try_get("y")?,
        issue_instant: row.try_get("issue_instant")?,
        not_after: row.try_get("not_after")?,
        token: row.try_get("token")?,
        display_claims: row.try_get("display_claims")?,
    })
}

async fn write_row(
    conn: &mut SqliteConnection,
    record: &MinecraftDeviceTokenRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO minecraft_device_tokens (
            id, uuid, private_key, x, y, issue_instant, not_after, token,
            display_claims
        )
        VALUES (0, ?, ?, ?, ?, ?, ?, ?, jsonb(?))
        ON CONFLICT (id) DO UPDATE SET
            uuid = excluded.uuid,
            private_key = excluded.private_key,
            x = excluded.x,
            y = excluded.y,
            issue_instant = excluded.issue_instant,
            not_after = excluded.not_after,
            token = excluded.token,
            display_claims = excluded.display_claims
        ",
    )
    .bind(&record.uuid)
    .bind(&record.private_key)
    .bind(&record.x)
    .bind(&record.y)
    .bind(record.issue_instant)
    .bind(record.not_after)
    .bind(&record.token)
    .bind(&record.display_claims)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
