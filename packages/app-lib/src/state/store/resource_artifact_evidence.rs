//! What the cache has already verified about an artifact.

use sqlx::SqliteConnection;

use super::Collection;

/// The verified facts, one per evidence key.
pub(crate) struct ResourceArtifactEvidence;

/// Evidence is identified by the key it was recorded under.
pub(crate) type ResourceArtifactEvidenceKey = String;

/// What evidence carries.
///
/// The fact itself is kept as the text it is stored as: what the text says is
/// the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceArtifactEvidenceRecord {
    pub value_version: i64,
    pub payload: String,
    pub verified_at: i64,
}

impl Collection for ResourceArtifactEvidence {
    type Key = ResourceArtifactEvidenceKey;
    type Scope = Self::Key;
    type Record = ResourceArtifactEvidenceRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(i64, String, i64)> = sqlx::query_as(
            "
            SELECT value_version, payload, verified_at
            FROM resource_artifact_evidence
            WHERE evidence_key = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(
            row.map(|(value_version, payload, verified_at)| Self::Record {
                value_version,
                payload,
                verified_at,
            }),
        )
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "DELETE FROM resource_artifact_evidence WHERE evidence_key = ?",
            )
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
        let rows: Vec<(String, i64, String, i64)> = sqlx::query_as(
            "
            SELECT evidence_key, value_version, payload, verified_at
            FROM resource_artifact_evidence
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(evidence_key, value_version, payload, verified_at)| {
                (
                    evidence_key,
                    ResourceArtifactEvidenceRecord {
                        value_version,
                        payload,
                        verified_at,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_artifact_evidence")
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
    record: &ResourceArtifactEvidenceRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_artifact_evidence
            (evidence_key, value_version, payload, verified_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (evidence_key) DO UPDATE SET
            value_version = excluded.value_version,
            payload = excluded.payload,
            verified_at = excluded.verified_at
        ",
    )
    .bind(key)
    .bind(record.value_version)
    .bind(&record.payload)
    .bind(record.verified_at)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
