//! Failed scans include paths that do not yet have a tracked file or entry.
use super::Collection;
use super::derived_db::DerivedDb;
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

/// Failed scans belong to the derived database, so their callers reach them
/// through that database rather than through a handle.
impl ContentRefreshScanFailures {
    pub(crate) async fn failed(
        derived: &DerivedDb,
        instance_id: &str,
    ) -> crate::Result<Vec<(ScanFailureKey, ScanFailureRecord)>> {
        let scope = instance_id.to_string();
        derived
            .read(async |handle| Self::get_scoped(handle, &scope).await)
            .await
    }

    pub(crate) async fn upsert(
        derived: &DerivedDb,
        key: &ScanFailureKey,
        record: &ScanFailureRecord,
    ) -> crate::Result<()> {
        derived
            .write(async |handle| Self::set(handle, key, Some(record)).await)
            .await
    }

    pub(crate) async fn remove(
        derived: &DerivedDb,
        key: &ScanFailureKey,
    ) -> crate::Result<()> {
        derived
            .write(async |handle| Self::set(handle, key, None).await)
            .await
    }

    /// Removes every failed scan of an instance. The rows carry no foreign key
    /// to the table that owns them, so the deletion of an instance is what
    /// removes them, and running it again costs nothing.
    pub(crate) async fn forget_instance(
        derived: &DerivedDb,
        instance_id: &str,
    ) -> crate::Result<()> {
        derived
            .write(async |handle| {
                Self::set_scoped(handle, &instance_id.to_string(), &[]).await
            })
            .await
    }

    /// The instances this collection holds a record for.
    pub(crate) async fn instance_ids(
        derived: &DerivedDb,
    ) -> crate::Result<Vec<String>> {
        derived
            .read(async |handle| {
                let mut connection = handle.connection().await?;
                Ok(sqlx::query_scalar(
                    "SELECT DISTINCT instance_id FROM content_refresh_scan_failures",
                )
                .fetch_all(&mut *connection)
                .await?)
            })
            .await
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
