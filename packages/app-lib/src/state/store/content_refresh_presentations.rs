//! Persisted row presentation, independently refreshed from update candidates.

use super::Collection;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

pub(crate) struct ContentRefreshPresentations;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PresentationKey {
    pub instance_id: String,
    pub file_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PresentationRecord {
    pub location_key: String,
    pub saved_at: i64,
    pub content: crate::state::ContentItem,
}

impl PresentationRecord {
    fn valid(&self) -> bool {
        !self.location_key.is_empty()
            && self.content.id.len() == 40
            && self.content.id.bytes().all(|byte| byte.is_ascii_hexdigit())
            && self.content.size <= i64::MAX as u64
            && !self.content.file_path.is_empty()
    }
}

impl Collection for ContentRefreshPresentations {
    type Key = PresentationKey;
    type Scope = String;
    type Record = PresentationRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == &key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM content_refresh_presentations WHERE instance_id = ? AND file_id = ?"
        ).bind(&key.instance_id).bind(&key.file_id).fetch_optional(&mut *conn).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM content_refresh_presentations WHERE instance_id = ? AND file_id = ?")
                .bind(&key.instance_id).bind(&key.file_id).execute(&mut *conn).await?;
            return Ok(());
        };
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid content_refresh_presentations record".into(),
            )
            .into());
        }
        sqlx::query("INSERT INTO content_refresh_presentations (instance_id, file_id, payload) VALUES (?, ?, ?) ON CONFLICT (instance_id, file_id) DO UPDATE SET payload = excluded.payload")
            .bind(&key.instance_id).bind(&key.file_id).bind(serde_json::to_string(record)?)
            .execute(&mut *conn).await?;
        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT instance_id, file_id, payload FROM content_refresh_presentations"
        ).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, file_id, payload)| {
                Some((
                    PresentationKey {
                        instance_id,
                        file_id,
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
        sqlx::query("DELETE FROM content_refresh_presentations")
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
            "SELECT file_id, payload FROM content_refresh_presentations WHERE instance_id = ?"
        ).bind(scope).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(file_id, payload)| {
                Some((
                    PresentationKey {
                        instance_id: scope.clone(),
                        file_id,
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
            "DELETE FROM content_refresh_presentations WHERE instance_id = ?",
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

fn decode(payload: &str) -> Option<PresentationRecord> {
    match serde_json::from_str::<PresentationRecord>(payload) {
        Ok(record) if record.valid() => Some(record),
        _ => {
            tracing::warn!(
                "Skipping an invalid content_refresh_presentations record"
            );
            None
        }
    }
}
