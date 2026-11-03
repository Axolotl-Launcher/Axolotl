//! Saved pack metadata is independent from per-file refresh progress.
use super::Collection;
use super::derived_db::DerivedDb;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
pub(crate) struct ContentRefreshPacks;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackKey {
    pub instance_id: String,
    pub content_set_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PackRecord {
    pub location_key: String,
    pub link_key: String,
    pub saved_at: i64,
    pub pack: Option<crate::state::InstanceContentPack>,
    #[serde(default)]
    pub attempted_at: i64,
    #[serde(default)]
    pub last_error: Option<String>,
}
impl PackRecord {
    fn valid(&self) -> bool {
        !self.location_key.is_empty() && !self.link_key.is_empty()
    }
}
impl Collection for ContentRefreshPacks {
    type Key = PackKey;
    type Scope = String;
    type Record = PackRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == &key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM content_refresh_packs WHERE instance_id = ? AND content_set_id = ?"
        ).bind(&key.instance_id).bind(&key.content_set_id).fetch_optional(&mut *conn).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM content_refresh_packs WHERE instance_id = ? AND content_set_id = ?")
                .bind(&key.instance_id).bind(&key.content_set_id).execute(&mut *conn).await?;
            return Ok(());
        };
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid content_refresh_packs record".into(),
            )
            .into());
        }
        sqlx::query("INSERT INTO content_refresh_packs (instance_id, content_set_id, payload) VALUES (?, ?, ?) ON CONFLICT (instance_id, content_set_id) DO UPDATE SET payload = excluded.payload")
            .bind(&key.instance_id).bind(&key.content_set_id).bind(serde_json::to_string(record)?)
            .execute(&mut *conn).await?;
        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT instance_id, content_set_id, payload FROM content_refresh_packs"
        ).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, content_set_id, payload)| {
                Some((
                    PackKey {
                        instance_id,
                        content_set_id,
                    },
                    decode(&payload)?,
                ))
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM content_refresh_packs")
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
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT content_set_id, payload FROM content_refresh_packs WHERE instance_id = ?"
        ).bind(scope).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(content_set_id, payload)| {
                Some((
                    PackKey {
                        instance_id: scope.clone(),
                        content_set_id,
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
        sqlx::query("DELETE FROM content_refresh_packs WHERE instance_id = ?")
            .bind(scope)
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }
}

/// The saved pack belongs to the derived database, so its callers read and
/// write it through that database rather than through a handle.
impl ContentRefreshPacks {
    pub(crate) async fn saved(
        derived: &DerivedDb,
        key: &PackKey,
    ) -> crate::Result<Option<PackRecord>> {
        derived
            .read(async |handle| Self::get(handle, key).await)
            .await
    }

    pub(crate) async fn upsert(
        derived: &DerivedDb,
        key: &PackKey,
        record: &PackRecord,
    ) -> crate::Result<()> {
        derived
            .write(async |handle| Self::set(handle, key, Some(record)).await)
            .await
    }

    /// Removes every saved pack of an instance. The rows carry no foreign key
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
                    "SELECT DISTINCT instance_id FROM content_refresh_packs",
                )
                .fetch_all(&mut *connection)
                .await?)
            })
            .await
    }
}

fn decode(payload: &str) -> Option<PackRecord> {
    match serde_json::from_str::<PackRecord>(payload) {
        Ok(record) if record.valid() => Some(record),
        _ => {
            tracing::warn!("Skipping an invalid content_refresh_packs record");
            None
        }
    }
}
