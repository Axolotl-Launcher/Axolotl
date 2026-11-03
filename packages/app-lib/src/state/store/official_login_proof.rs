//! Proof that the official Minecraft login succeeded on this installation.

use sqlx::SqliteConnection;

use super::Collection;

/// The proof that official login was verified once.
pub(crate) struct OfficialLoginProof;

/// There is one proof per installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Installation;

/// What the proof carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfficialLoginProofRecord {
    pub mac: String,
}

impl Collection for OfficialLoginProof {
    type Key = Installation;
    type Scope = Installation;
    type Record = OfficialLoginProofRecord;

    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let mac: Option<String> = sqlx::query_scalar(
            "
            SELECT mac
            FROM official_login_proof
            WHERE id = 0 AND verified = 1 AND version = 1
            ",
        )
        .fetch_optional(&mut *conn)
        .await?;

        Ok(mac.map(|mac| Self::Record { mac }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        _key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM official_login_proof WHERE id = 0")
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
        sqlx::query("DELETE FROM official_login_proof")
            .execute(&mut *conn)
            .await?;
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
    record: &OfficialLoginProofRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO official_login_proof (id, verified, version, mac)
        VALUES (0, 1, 1, ?)
        ON CONFLICT (id) DO UPDATE SET
            verified = 1,
            version = 1,
            mac = excluded.mac
        ",
    )
    .bind(&record.mac)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// The proof as the row holds it, for the document handover and for the
/// fallback the domain answers with while it has no document.
///
/// Only a row this build counts as a proof is read: one that is verified and
/// carries the format this build knows.
pub(crate) async fn read_legacy_official_login_proof(
    pool: &sqlx::SqlitePool,
) -> crate::Result<Option<String>> {
    let mac: Option<String> = sqlx::query_scalar(
        "SELECT mac FROM official_login_proof WHERE id = 0 AND verified = 1 AND version = 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(mac)
}
