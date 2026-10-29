//! Failed scans include paths that do not yet have a tracked file or entry.
use super::Collection;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

pub(crate) struct ContentRefreshScanFailures;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScanFailureKey {
    pub instance_id: String,
    pub location_key: String,
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ScanFailureRecord {
    pub attempted_at: i64,
    pub error: String,
}

impl Collection for ContentRefreshScanFailures {
    type Key = ScanFailureKey;
    type Scope = String;
    type Record = ScanFailureRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == &key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM content_refresh_scan_failures WHERE instance_id = ? AND location_key = ? AND relative_path = ?")
            .bind(&key.instance_id).bind(&key.location_key).bind(&key.relative_path).fetch_optional(conn).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        if let Some(record) = record {
            if key.location_key.is_empty()
                || key.relative_path.is_empty()
                || record.error.is_empty()
            {
                return Err(crate::ErrorKind::InputError(
                    "Invalid scan failure".into(),
                )
                .into());
            }
            sqlx::query("INSERT INTO content_refresh_scan_failures (instance_id, location_key, relative_path, payload) VALUES (?, ?, ?, ?) ON CONFLICT (instance_id, location_key, relative_path) DO UPDATE SET payload = excluded.payload")
                .bind(&key.instance_id).bind(&key.location_key).bind(&key.relative_path).bind(serde_json::to_string(record)?).execute(conn).await?;
        } else {
            sqlx::query("DELETE FROM content_refresh_scan_failures WHERE instance_id = ? AND location_key = ? AND relative_path = ?")
                .bind(&key.instance_id).bind(&key.location_key).bind(&key.relative_path).execute(conn).await?;
        }
        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as("SELECT instance_id, location_key, relative_path, payload FROM content_refresh_scan_failures").fetch_all(conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(
                |(instance_id, location_key, relative_path, payload)| {
                    Some((
                        ScanFailureKey {
                            instance_id,
                            location_key,
                            relative_path,
                        },
                        decode(&payload)?,
                    ))
                },
            )
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM content_refresh_scan_failures")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT location_key, relative_path, payload FROM content_refresh_scan_failures WHERE instance_id = ?").bind(scope).fetch_all(conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(location_key, relative_path, payload)| {
                Some((
                    ScanFailureKey {
                        instance_id: scope.clone(),
                        location_key,
                        relative_path,
                    },
                    decode(&payload)?,
                ))
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query(
            "DELETE FROM content_refresh_scan_failures WHERE instance_id = ?",
        )
        .bind(scope)
        .execute(&mut *conn)
        .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }
}

fn decode(payload: &str) -> Option<ScanFailureRecord> {
    match serde_json::from_str::<ScanFailureRecord>(payload) {
        Ok(record) if !record.error.is_empty() => Some(record),
        _ => {
            tracing::warn!("Skipping an invalid scan failure record");
            None
        }
    }
}
