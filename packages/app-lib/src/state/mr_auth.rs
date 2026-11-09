use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::modrinth_users::{ModrinthUserRecord, ModrinthUsers};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModrinthCredentials {
    pub session: String,
    pub expires: DateTime<Utc>,
    pub user_id: String,
    pub active: bool,
}

impl ModrinthCredentials {
    fn from_record(user_id: String, record: ModrinthUserRecord) -> Self {
        Self {
            session: record.session_id,
            expires: Utc
                .timestamp_opt(record.expires, 0)
                .single()
                .unwrap_or_else(Utc::now),
            user_id,
            active: record.active,
        }
    }

    fn to_record(&self) -> ModrinthUserRecord {
        ModrinthUserRecord {
            active: self.active,
            session_id: self.session.clone(),
            expires: self.expires.timestamp(),
        }
    }

    /// The session the launcher signed in with.
    pub(crate) async fn get_active(
        handle: &Handle,
    ) -> crate::Result<Option<Self>> {
        Ok(ModrinthUsers::get_scoped(handle, &true)
            .await?
            .into_iter()
            .next()
            .map(|(user_id, record)| Self::from_record(user_id, record)))
    }

    pub(crate) async fn upsert(&self, handle: &Handle) -> crate::Result<()> {
        ModrinthUsers::set(handle, &self.user_id, Some(&self.to_record())).await
    }
}
