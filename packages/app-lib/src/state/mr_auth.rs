use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::modrinth_users::{ModrinthUserRecord, ModrinthUsers};
use crate::state::{CacheBehaviour, CachedEntry};
use crate::util::fetch::{FetchSemaphore, fetch_advanced};
use chrono::{DateTime, Duration, TimeZone, Utc};
use reqwest::Method;
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

    pub(crate) async fn get_and_refresh(
        handle: &Handle,
        semaphore: &FetchSemaphore,
    ) -> crate::Result<Option<Self>> {
        let creds = Self::get_active(handle).await?;

        if let Some(mut creds) = creds {
            if creds.expires < Utc::now() {
                #[derive(Deserialize)]
                struct Session {
                    session: String,
                }

                let resp = fetch_advanced(
                    Method::POST,
                    concat!(env!("MODRINTH_API_URL"), "session/refresh"),
                    None,
                    None,
                    Some(("Authorization", &*creds.session)),
                    None,
                    None,
                    Some("/v2/session/refresh"),
                    semaphore,
                )
                .await
                .ok()
                .and_then(|resp| serde_json::from_slice::<Session>(&resp).ok());

                if let Some(value) = resp {
                    creds.session = value.session;
                    creds.expires = Utc::now() + Duration::weeks(2);
                    creds.upsert(handle).await?;

                    Ok(Some(creds))
                } else {
                    Self::remove(&creds.user_id, handle).await?;

                    Ok(None)
                }
            } else {
                Ok(Some(creds))
            }
        } else {
            Ok(None)
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

    pub(crate) async fn remove(
        user_id: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        ModrinthUsers::set(handle, &user_id.to_string(), None).await
    }

    pub(crate) async fn refresh_all() -> crate::Result<()> {
        let state = crate::State::get().await?;
        let user_ids = ModrinthUsers::get_all(&state.data)
            .await?
            .into_iter()
            .map(|(user_id, _)| user_id)
            .collect::<Vec<_>>();

        CachedEntry::get_user_many(
            &user_ids.iter().map(|x| &**x).collect::<Vec<_>>(),
            Some(CacheBehaviour::Bypass),
            state.cache_pool(),
            &state.fetch_semaphore,
        )
        .await?;

        Ok(())
    }
}

pub const fn get_login_url() -> &'static str {
    concat!(env!("MODRINTH_URL"), "auth/sign-in")
}

pub async fn finish_login_flow(
    code: &str,
    semaphore: &FetchSemaphore,
) -> crate::Result<ModrinthCredentials> {
    // The authorization code actually is the access token, since labrinth doesn't
    // issue separate authorization codes. Therefore, this is equivalent to an
    // implicit OAuth grant flow, and no additional exchanging or finalization is
    // needed. TODO not do this for the reasons outlined at
    // https://oauth.net/2/grant-types/implicit/

    let info = fetch_info(code, semaphore).await?;

    Ok(ModrinthCredentials {
        session: code.to_string(),
        expires: Utc::now() + Duration::weeks(2),
        user_id: info.id,
        active: true,
    })
}

async fn fetch_info(
    token: &str,
    semaphore: &FetchSemaphore,
) -> crate::Result<crate::state::cache::User> {
    let result = fetch_advanced(
        Method::GET,
        concat!(env!("MODRINTH_API_URL"), "user"),
        None,
        None,
        Some(("Authorization", token)),
        None,
        None,
        Some("/v2/user"),
        semaphore,
    )
    .await?;
    let value = serde_json::from_slice(&result)?;

    Ok(value)
}
