//! Successful update results survive failed attempts and page lifetimes.

use crate::resources::{SourceId, SourceReleaseRef, ipc::ResourceTarget};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use super::Collection;

pub(crate) struct ContentRefreshChecks;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefreshCheckKey {
    pub instance_id: String,
    pub entry_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefreshCheckContext {
    pub location_key: String,
    pub content_root: String,
    pub sha1: String,
    pub size: u64,
    pub current_release: Option<SourceReleaseRef>,
    pub target: ResourceTarget,
    pub preferred_source: SourceId,
}

pub(crate) fn location_key(
    instance: &crate::state::instances::Instance,
) -> String {
    serde_json::json!([
        instance.path,
        instance.applied_content_set_id,
        instance.game_dir_override,
        instance.linked_launcher,
        instance.linked_launcher_root,
        instance.linked_dot_minecraft,
        instance.linked_version_id,
        instance.linked_version_json_path,
        instance.linked_game_dir_mode,
    ])
    .to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SuccessfulCheck {
    pub checked_at: i64,
    /// None means the full check succeeded and confirmed there is no update.
    pub candidate: Option<SourceReleaseRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CheckAttempt {
    Succeeded,
    Unrecognized,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefreshCheckRecord {
    pub file_id: String,
    pub context: RefreshCheckContext,
    pub last_success: Option<SuccessfulCheck>,
    pub attempted_at: i64,
    pub attempt: CheckAttempt,
    pub error: Option<String>,
}

impl RefreshCheckRecord {
    pub(crate) fn for_context(
        previous: Option<Self>,
        file_id: String,
        context: RefreshCheckContext,
        attempted_at: i64,
    ) -> Self {
        previous
            .filter(|record| {
                record.file_id == file_id && record.context == context
            })
            .unwrap_or(Self {
                file_id,
                context,
                last_success: None,
                attempted_at,
                attempt: CheckAttempt::Unrecognized,
                error: None,
            })
    }

    pub(crate) fn succeeded(
        &mut self,
        checked_at: i64,
        candidate: Option<SourceReleaseRef>,
    ) {
        self.last_success = Some(SuccessfulCheck {
            checked_at,
            candidate,
        });
        self.attempted_at = checked_at;
        self.attempt = CheckAttempt::Succeeded;
        self.error = None;
    }

    pub(crate) fn failed(&mut self, attempted_at: i64, error: String) {
        self.attempted_at = attempted_at;
        self.attempt = CheckAttempt::Failed;
        self.error = Some(error);
    }

    fn valid(&self) -> bool {
        !self.file_id.is_empty()
            && !self.context.location_key.is_empty()
            && !self.context.content_root.is_empty()
            && self.context.sha1.len() == 40
            && self
                .context
                .sha1
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && self.context.size <= i64::MAX as u64
            && match self.attempt {
                CheckAttempt::Succeeded => {
                    self.last_success.is_some() && self.error.is_none()
                }
                CheckAttempt::Failed => self.error.is_some(),
                CheckAttempt::Unrecognized => self.error.is_none(),
            }
    }
}

impl Collection for ContentRefreshChecks {
    type Key = RefreshCheckKey;
    type Scope = String;
    type Record = RefreshCheckRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == &key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM content_refresh_checks WHERE instance_id = ? AND entry_id = ?"
        ).bind(&key.instance_id).bind(&key.entry_id).fetch_optional(&mut *conn).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM content_refresh_checks WHERE instance_id = ? AND entry_id = ?")
                .bind(&key.instance_id).bind(&key.entry_id).execute(&mut *conn).await?;
            return Ok(());
        };
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid content_refresh_checks record".into(),
            )
            .into());
        }
        sqlx::query("INSERT INTO content_refresh_checks (instance_id, entry_id, payload, file_id) VALUES (?, ?, ?, ?) ON CONFLICT (instance_id, entry_id) DO UPDATE SET payload = excluded.payload, file_id = excluded.file_id")
            .bind(&key.instance_id).bind(&key.entry_id).bind(serde_json::to_string(record)?)
        .bind(&record.file_id)
            .execute(&mut *conn).await?;
        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT instance_id, entry_id, payload FROM content_refresh_checks",
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, entry_id, payload)| {
                Some((
                    RefreshCheckKey {
                        instance_id,
                        entry_id,
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
        sqlx::query("DELETE FROM content_refresh_checks")
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
            "SELECT entry_id, payload FROM content_refresh_checks WHERE instance_id = ?"
        ).bind(scope).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(entry_id, payload)| {
                Some((
                    RefreshCheckKey {
                        instance_id: scope.clone(),
                        entry_id,
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
        sqlx::query("DELETE FROM content_refresh_checks WHERE instance_id = ?")
            .bind(scope)
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }
}

fn decode(payload: &str) -> Option<RefreshCheckRecord> {
    match serde_json::from_str::<RefreshCheckRecord>(payload) {
        Ok(record) if record.valid() => Some(record),
        _ => {
            tracing::warn!("Skipping an invalid content_refresh_checks record");
            None
        }
    }
}
