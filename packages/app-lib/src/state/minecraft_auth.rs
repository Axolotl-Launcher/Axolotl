use crate::ErrorKind;
use crate::util::fetch::authentication_client;
use crate::util::mojang::{mojang_service_url, should_use_mojang_mirror};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, TimeZone, Utc};
use futures::TryStreamExt;
use heck::ToTitleCase;
use p256::ecdsa::SigningKey;
use p256::pkcs8::{EncodePrivateKey, LineEnding};
use rand::Rng;
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use sha2::Digest;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::{Handle, RuntimeFlavor};
use tokio::sync::Mutex;
use tokio::task;
use url::Url;
use uuid::Uuid;

mod yggdrasil;
pub use yggdrasil::*;

#[derive(Debug, Clone, Copy)]
pub enum MinecraftAuthStep {
    GetDeviceToken,
    SisuAuthenticate,
    GetOAuthToken,
    RefreshOAuthToken,
    SisuAuthorize,
    XstsAuthorize,
    MinecraftToken,
    MinecraftEntitlements,
    MinecraftProfile,
}

#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthenticationError {
    #[error("Error reading public key during generation")]
    ReadingPublicKey,
    #[error("Failed to serialize private key to PEM: {0}")]
    PEMSerialize(#[from] p256::pkcs8::Error),
    #[error("Failed to serialize body to JSON during step {step:?}: {source}")]
    SerializeBody {
        step: MinecraftAuthStep,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Failed to deserialize response to JSON during step {step:?}: {source}. Status Code: {status_code} Body: {raw}"
    )]
    DeserializeResponse {
        step: MinecraftAuthStep,
        raw: String,
        #[source]
        source: serde_json::Error,
        status_code: StatusCode,
    },
    #[error("Request failed during step {step:?}: {source}")]
    Request {
        step: MinecraftAuthStep,
        #[source]
        source: reqwest::Error,
    },
    #[error("Error reading XBOX Session ID header")]
    NoSessionId,
    #[error("Error reading user hash")]
    NoUserHash,
    #[error("The Microsoft account does not own Minecraft: Java Edition")]
    NoMinecraftEntitlement,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftLoginFlow {
    pub verifier: String,
    pub state: String,
    pub auth_request_uri: String,
    pub redirect_uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftDeviceLoginFlow {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Serialize, Debug)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MinecraftDeviceLoginPoll {
    Pending { slow_down: bool },
    Complete { credentials: Credentials },
}

#[tracing::instrument]
pub async fn login_begin(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<MinecraftLoginFlow> {
    login_begin_with_redirect(AUTH_REPLY_URL, exec).await
}

#[tracing::instrument]
pub async fn login_begin_with_redirect(
    redirect_uri: &str,
    _exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<MinecraftLoginFlow> {
    let verifier = generate_oauth_challenge();
    let state = generate_oauth_challenge();
    let result = sha2::Sha256::digest(&verifier);
    let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);
    let mut auth_request_uri = Url::parse(MICROSOFT_AUTHORIZE_URL)?;
    auth_request_uri.query_pairs_mut().extend_pairs([
        ("client_id", MICROSOFT_CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("response_mode", "query"),
        ("scope", REQUESTED_SCOPE),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("state", &state),
        ("prompt", "select_account"),
    ]);

    Ok(MinecraftLoginFlow {
        verifier,
        state,
        auth_request_uri: auth_request_uri.into(),
        redirect_uri: redirect_uri.to_owned(),
    })
}

#[tracing::instrument]
pub async fn browser_login_begin(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<MinecraftLoginFlow> {
    login_begin_with_redirect(BROWSER_AUTH_REPLY_URL, exec).await
}

#[tracing::instrument]
pub async fn device_login_begin() -> crate::Result<MinecraftDeviceLoginFlow> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        authentication_client()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;
    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        }
    })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
        .into()
    })
}

#[tracing::instrument]
pub async fn device_login_poll(
    device_code: &str,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<MinecraftDeviceLoginPoll> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("device_code", device_code);
    query.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code");

    let res = auth_retry(|| {
        authentication_client()
            .post(MICROSOFT_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;
    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        }
    })?;

    if let Ok(error) = serde_json::from_str::<MicrosoftOAuthError>(&text)
        && (error.error == "authorization_pending"
            || error.error == "slow_down")
    {
        return Ok(MinecraftDeviceLoginPoll::Pending {
            slow_down: error.error == "slow_down",
        });
    }

    let token = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;
    let credentials = finish_microsoft_login(
        RequestWithDate {
            date: current_date,
            value: token,
        },
        exec,
    )
    .await?;

    Ok(MinecraftDeviceLoginPoll::Complete { credentials })
}

#[tracing::instrument]
pub async fn login_finish(
    code: &str,
    state: &str,
    flow: MinecraftLoginFlow,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<Credentials> {
    if state != flow.state {
        return Err(crate::ErrorKind::InputError(
            "Microsoft sign-in response did not match the active login request"
                .into(),
        )
        .as_error());
    }
    let oauth_token =
        oauth_token(code, &flow.verifier, &flow.redirect_uri).await?;
    finish_microsoft_login(oauth_token, exec).await
}

async fn finish_microsoft_login(
    oauth_token: RequestWithDate<OAuthToken>,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<Credentials> {
    let xbl_token =
        xbox_live_authorize(&oauth_token.value.access_token).await?;
    let xsts_token = xsts_authorize_xbl(&xbl_token.value.token).await?;
    let minecraft_token = minecraft_token(xsts_token.value).await?;

    minecraft_entitlements(&minecraft_token.access_token).await?;

    let mut credentials = Credentials {
        account_id: None,
        offline_profile: MinecraftProfile::default(),
        account_type: MinecraftAccountType::Microsoft,
        access_token: minecraft_token.access_token,
        refresh_token: oauth_token.value.refresh_token,
        expires: oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64),
        active: true,
        yggdrasil: None,
    };

    let online_profile = credentials
        .online_profile()
        .await
        .ok_or(io::Error::other("Failed to fetch player profile"))?;
    credentials.offline_profile = MinecraftProfile {
        id: online_profile.id,
        name: online_profile.name.clone(),
        ..credentials.offline_profile
    };
    credentials.account_id = Some(format!(
        "microsoft:{}",
        credentials.offline_profile.id.as_hyphenated()
    ));

    credentials.upsert(exec).await?;

    crate::anti_piracy::mark_official_login().await.map_err(|error| {
        tracing::warn!(%error, "Could not persist official Minecraft login proof");
        crate::ErrorKind::InputError(
            "Your Minecraft account was saved, but its official login record could not be saved. Offline account access was not unlocked. Please sign in again to retry.".to_string(),
        ).as_error()
    })?;

    Ok(credentials)
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq,
)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftAccountType {
    #[default]
    Microsoft,
    Offline,
    Yggdrasil,
}

impl MinecraftAccountType {
    fn from_database(value: &str) -> Self {
        match value {
            "offline" => Self::Offline,
            "yggdrasil" => Self::Yggdrasil,
            _ => Self::Microsoft,
        }
    }

    fn as_database(self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::Offline => "offline",
            Self::Yggdrasil => "yggdrasil",
        }
    }
}

#[derive(sqlx::FromRow)]
struct StoredCredentials {
    account_id: String,
    uuid: String,
    active: i64,
    username: String,
    account_type: String,
    access_token: String,
    refresh_token: String,
    expires: i64,
    yggdrasil_api_root: String,
    yggdrasil_server_name: String,
    yggdrasil_login: String,
    yggdrasil_client_token: String,
}

#[derive(Deserialize, Debug)]
pub struct Credentials {
    #[serde(default)]
    pub account_id: Option<String>,
    /// The offline profile of the user these credentials are for.
    ///
    /// Such a profile can only be relied upon to have a proper player UUID, which is
    /// never changed. A potentially stale username may be available, but no other data
    /// such as skins or capes is available.
    #[serde(rename = "profile")]
    pub offline_profile: MinecraftProfile,
    #[serde(default)]
    pub account_type: MinecraftAccountType,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
    #[serde(default)]
    pub yggdrasil: Option<YggdrasilAccount>,
}

/// An entry in the player profile cache, keyed by player UUID.
pub(super) enum ProfileCacheEntry {
    /// A cached profile that is valid, even though it may be stale.
    Hit(Arc<MinecraftProfile>),
    /// A negative profile fetch result due to an authentication error,
    /// from which we're recovering by holding off from repeatedly
    /// attempting to fetch the profile until the token is refreshed
    /// or some time has passed.
    AuthErrorBackoff {
        likely_expired_token: String,
        last_attempt: Instant,
    },
}

/// A thread-safe cache of online profiles, used to avoid fetching the
/// same profile multiple times as long as they don't get too stale.
///
/// The cache has to be static because credential objects are short lived
/// and disposable, and in the future several threads may be interested in
/// profile data.
pub(super) static PROFILE_CACHE: Mutex<
    HashMap<Uuid, ProfileCacheEntry, BuildHasherDefault<DefaultHasher>>,
> = Mutex::const_new(HashMap::with_hasher(BuildHasherDefault::new()));

const ONLINE_PROFILE_CACHE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(60);
const ONLINE_PROFILE_LIVE_STATE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(5);
const ONLINE_PROFILE_AUTH_ERROR_BACKOFF: std::time::Duration =
    std::time::Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
enum OnlineProfileCacheIntent {
    NormalRead,
    LiveStateRead,
    RefreshFromMojang,
}

impl OnlineProfileCacheIntent {
    fn max_age(self) -> std::time::Duration {
        match self {
            Self::NormalRead => ONLINE_PROFILE_CACHE_MAX_AGE,
            Self::LiveStateRead => ONLINE_PROFILE_LIVE_STATE_MAX_AGE,
            Self::RefreshFromMojang => std::time::Duration::ZERO,
        }
    }

    fn can_use_stale_on_fetch_error(self) -> bool {
        matches!(self, Self::LiveStateRead)
    }
}

fn validate_offline_username(username: &str) -> crate::Result<String> {
    let username = username.trim();
    if !(1..=16).contains(&username.chars().count())
        || !username
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
    {
        return Err(ErrorKind::InputError(
            "Minecraft usernames must be 1-16 characters and contain only letters, numbers, and underscores"
                .to_string(),
        )
        .as_error());
    }
    Ok(username.to_string())
}

struct RefreshedTokens {
    access_token: String,
    refresh_token: String,
    expires: DateTime<Utc>,
    yggdrasil: Option<YggdrasilAccount>,
}
#[derive(Default)]
struct CredentialRefreshState {
    refreshed: Option<RefreshedTokens>,
    source_token: Option<String>,
    failure: Option<(Instant, Arc<ErrorKind>)>,
}

static CREDENTIAL_REFRESHES: std::sync::LazyLock<
    dashmap::DashMap<String, Arc<Mutex<CredentialRefreshState>>>,
> = std::sync::LazyLock::new(dashmap::DashMap::new);

impl Credentials {
    pub fn account_id(&self) -> String {
        self.account_id
            .clone()
            .unwrap_or_else(|| match &self.yggdrasil {
                Some(account) => format!(
                    "{}:{}:{}",
                    self.account_type.as_database(),
                    account.api_root,
                    self.offline_profile.id.as_hyphenated()
                ),
                None => format!(
                    "{}:{}",
                    self.account_type.as_database(),
                    self.offline_profile.id.as_hyphenated()
                ),
            })
    }

    pub fn offline(username: &str) -> crate::Result<Self> {
        let username = validate_offline_username(username)?;
        let mut uuid_bytes =
            md5::compute(format!("OfflinePlayer:{username}")).0;
        uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x30;
        uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80;

        Self::offline_with_uuid(&username, Uuid::from_bytes(uuid_bytes))
    }

    pub fn offline_with_uuid(
        username: &str,
        uuid: Uuid,
    ) -> crate::Result<Self> {
        let username = validate_offline_username(username)?;

        Ok(Self {
            account_id: Some(format!("offline:{}", uuid.as_hyphenated())),
            offline_profile: MinecraftProfile {
                id: uuid,
                name: username,
                ..MinecraftProfile::default()
            },
            account_type: MinecraftAccountType::Offline,
            access_token: "0".to_string(),
            refresh_token: String::new(),
            expires: Utc::now(),
            active: true,
            yggdrasil: None,
        })
    }

    pub fn is_offline(&self) -> bool {
        self.account_type == MinecraftAccountType::Offline
    }

    pub fn is_microsoft(&self) -> bool {
        self.account_type == MinecraftAccountType::Microsoft
    }

    pub fn is_yggdrasil(&self) -> bool {
        self.account_type == MinecraftAccountType::Yggdrasil
    }

    fn from_stored(stored: StoredCredentials) -> Self {
        let account_type =
            MinecraftAccountType::from_database(&stored.account_type);
        let yggdrasil = (account_type == MinecraftAccountType::Yggdrasil)
            .then_some({
                YggdrasilAccount {
                    api_root: stored.yggdrasil_api_root,
                    server_name: stored.yggdrasil_server_name,
                    login: stored.yggdrasil_login,
                    client_token: stored.yggdrasil_client_token,
                }
            });

        Self {
            account_id: Some(stored.account_id),
            offline_profile: MinecraftProfile {
                id: Uuid::parse_str(&stored.uuid).unwrap_or_default(),
                name: stored.username,
                ..MinecraftProfile::default()
            },
            account_type,
            access_token: stored.access_token,
            refresh_token: stored.refresh_token,
            expires: Utc
                .timestamp_opt(stored.expires, 0)
                .single()
                .unwrap_or_else(Utc::now),
            active: stored.active == 1,
            yggdrasil,
        }
    }

    /// Refreshes the authentication tokens for this user if they are expired, or
    /// very close to expiration.
    async fn refresh(
        &mut self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        if self.is_offline() {
            return Ok(());
        }
        let refresh_margin = if self.is_yggdrasil() {
            Duration::zero()
        } else {
            Duration::minutes(5)
        };
        if self.expires > Utc::now() + refresh_margin {
            return Ok(());
        }
        if CREDENTIAL_REFRESHES.len() > 64 {
            CREDENTIAL_REFRESHES
                .retain(|_, state| Arc::strong_count(state) > 1);
        }
        let key = self.account_id();
        let state = CREDENTIAL_REFRESHES.entry(key).or_default().clone();
        let mut state = state.lock().await;
        if state
            .source_token
            .as_ref()
            .is_some_and(|token| token != &self.refresh_token)
            && state
                .refreshed
                .as_ref()
                .is_none_or(|cached| cached.refresh_token != self.refresh_token)
        {
            *state = CredentialRefreshState::default();
        }
        if let Some(refreshed) = &state.refreshed
            && state.source_token.as_ref() == Some(&self.refresh_token)
            && refreshed.expires > self.expires
        {
            self.access_token = refreshed.access_token.clone();
            self.refresh_token = refreshed.refresh_token.clone();
            self.expires = refreshed.expires;
            self.yggdrasil = refreshed.yggdrasil.clone();
        }
        if self.expires > Utc::now() + refresh_margin {
            return Ok(());
        }
        if let Some((failed_at, error)) = &state.failure
            && failed_at.elapsed() < std::time::Duration::from_secs(30)
        {
            return Err(crate::Error::from_shared_kind(Arc::clone(error)));
        }
        state.source_token = Some(self.refresh_token.clone());
        let result = self.refresh_tokens(exec).await;
        if result.is_ok() {
            // Keep only a short-lived refresh result for concurrent readers.
            state.refreshed = Some(RefreshedTokens {
                access_token: self.access_token.clone(),
                refresh_token: self.refresh_token.clone(),
                expires: self.expires,
                yggdrasil: self.yggdrasil.clone(),
            });
            state.failure = None;
        } else {
            state.failure = Some((
                Instant::now(),
                Arc::clone(&result.as_ref().unwrap_err().raw),
            ));
        }
        result
    }

    async fn refresh_tokens(
        &mut self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        if self.is_yggdrasil() {
            return refresh_yggdrasil_credentials(self, exec).await;
        }

        // Use a margin of 5 minutes to give e.g. Minecraft and potentially
        // other operations that depend on a fresh token 5 minutes to complete
        // from now, and deal with some classes of clock skew
        if self.expires > Utc::now() + Duration::minutes(5) {
            return Ok(());
        }

        let oauth_token = oauth_refresh(&self.refresh_token).await?;
        let xbl_token =
            xbox_live_authorize(&oauth_token.value.access_token).await?;
        let xsts_token = xsts_authorize_xbl(&xbl_token.value.token).await?;
        let minecraft_token = minecraft_token(xsts_token.value).await?;

        self.access_token = minecraft_token.access_token;
        self.refresh_token = oauth_token.value.refresh_token;
        self.expires = oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64);

        self.upsert(exec).await?;

        Ok(())
    }

    /// Returns online profile data when the cached copy is still recent enough.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::NormalRead,
        )
        .await
    }

    /// Returns profile data recent enough for skin and cape state.
    ///
    /// Reuses a profile read from the last few seconds so opening the skins page
    /// does not send several identical Mojang requests.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile_fresh(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::LiveStateRead,
        )
        .await
    }

    /// Fetches the online profile from Mojang after a skin or cape change.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_online_profile(
        &self,
    ) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::RefreshFromMojang,
        )
        .await
    }

    async fn online_profile_with_cache_intent(
        &self,
        cache_intent: OnlineProfileCacheIntent,
    ) -> Option<Arc<MinecraftProfile>> {
        if self.is_offline() {
            return None;
        }
        if self.is_yggdrasil() {
            return self.yggdrasil_online_profile(cache_intent).await;
        }

        let max_age = cache_intent.max_age();
        let stale_profile = {
            let mut profile_cache = PROFILE_CACHE.lock().await;
            let mut remove_cached_entry = false;

            let stale_profile = if let Some(cache_entry) =
                profile_cache.get(&self.offline_profile.id)
            {
                match cache_entry {
                    ProfileCacheEntry::Hit(profile)
                        if profile.is_fresh(max_age) =>
                    {
                        return Some(Arc::clone(profile));
                    }
                    ProfileCacheEntry::Hit(profile) => {
                        Some(Arc::clone(profile))
                    }
                    // Auth errors must be handled with a backoff strategy because it
                    // has been experimentally found that Mojang quickly rate limits
                    // the profile data endpoint on repeated attempts with bad auth
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token,
                        last_attempt,
                    } if &self.access_token != likely_expired_token
                        || Instant::now()
                            .saturating_duration_since(*last_attempt)
                            > ONLINE_PROFILE_AUTH_ERROR_BACKOFF =>
                    {
                        remove_cached_entry = true;
                        None
                    }
                    ProfileCacheEntry::AuthErrorBackoff { .. } => {
                        return None;
                    }
                }
            } else {
                None
            };

            if remove_cached_entry {
                profile_cache.remove(&self.offline_profile.id);
            }

            stale_profile
        };

        match minecraft_profile(&self.access_token).await {
            Ok(profile) => {
                let profile = Arc::new(profile);
                let cache_entry = ProfileCacheEntry::Hit(Arc::clone(&profile));

                let mut profile_cache = PROFILE_CACHE.lock().await;
                if self.offline_profile.id != profile.id {
                    profile_cache.remove(&self.offline_profile.id);
                }
                profile_cache.insert(profile.id, cache_entry);

                Some(profile)
            }
            Err(
                err @ MinecraftAuthenticationError::DeserializeResponse {
                    status_code: StatusCode::UNAUTHORIZED,
                    ..
                },
            ) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {} likely due to stale credentials, backing off: {err}",
                    self.offline_profile.id
                );

                let mut profile_cache = PROFILE_CACHE.lock().await;
                profile_cache.insert(
                    self.offline_profile.id,
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token: self.access_token.clone(),
                        last_attempt: Instant::now(),
                    },
                );

                None
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {}: {err}",
                    self.offline_profile.id
                );

                if cache_intent.can_use_stale_on_fetch_error() {
                    stale_profile
                } else {
                    None
                }
            }
        }
    }

    async fn yggdrasil_online_profile(
        &self,
        cache_intent: OnlineProfileCacheIntent,
    ) -> Option<Arc<MinecraftProfile>> {
        let account = self.yggdrasil.as_ref()?;
        let stale_profile = {
            let profile_cache = PROFILE_CACHE.lock().await;
            match profile_cache.get(&self.offline_profile.id) {
                Some(ProfileCacheEntry::Hit(profile))
                    if profile.is_fresh(cache_intent.max_age()) =>
                {
                    return Some(Arc::clone(profile));
                }
                Some(ProfileCacheEntry::Hit(profile)) => {
                    Some(Arc::clone(profile))
                }
                Some(ProfileCacheEntry::AuthErrorBackoff { .. }) | None => None,
            }
        };

        match fetch_yggdrasil_profile(account, self.offline_profile.id).await {
            Ok(Some(profile)) => {
                let profile = Arc::new(profile);
                PROFILE_CACHE.lock().await.insert(
                    self.offline_profile.id,
                    ProfileCacheEntry::Hit(Arc::clone(&profile)),
                );
                Some(profile)
            }
            Ok(None) => {
                if cache_intent.can_use_stale_on_fetch_error() {
                    stale_profile
                } else {
                    None
                }
            }
            Err(error) => {
                tracing::warn!(
                    "Failed to fetch Yggdrasil profile for UUID {} from {}: {error}",
                    self.offline_profile.id,
                    account.server_name
                );
                if cache_intent.can_use_stale_on_fetch_error() {
                    stale_profile
                } else {
                    None
                }
            }
        }
    }

    /// Attempts to fetch the online profile for this user if possible, and if that fails
    /// falls back to the known offline profile data.
    ///
    /// See also the [`online_profile`](Self::online_profile) method.
    pub async fn maybe_online_profile(
        &self,
    ) -> MaybeOnlineMinecraftProfile<'_> {
        let online_profile = self.online_profile().await;
        online_profile.map_or_else(
            || MaybeOnlineMinecraftProfile::Offline(&self.offline_profile),
            MaybeOnlineMinecraftProfile::Online,
        )
    }

    /// Like [`get_active`](Self::get_active), but enforces credentials to be
    /// successfully refreshed unless the network is unreachable or times out.
    #[tracing::instrument]
    pub async fn get_default_credential(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Credentials>> {
        let credentials = Self::get_active_without_refresh(exec).await?;

        if let Some(mut creds) = credentials {
            let res = creds.refresh(exec).await;

            match res {
                Ok(_) => Ok(Some(creds)),
                Err(err) => {
                    let network_unavailable = match &*err.raw {
                        ErrorKind::MinecraftAuthenticationError(
                            MinecraftAuthenticationError::Request {
                                source,
                                ..
                            },
                        )
                        | ErrorKind::FetchError(source) => {
                            source.is_connect() || source.is_timeout()
                        }
                        _ => false,
                    };
                    if network_unavailable {
                        return Ok(Some(creds));
                    }

                    Err(err)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Fetches the currently selected credentials from the database, attempting
    /// to refresh them if they are expired.
    pub async fn get_active(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Self>> {
        Self::get_active_with_refresh(exec, true).await
    }

    pub async fn get_active_without_refresh(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Self>> {
        Self::get_active_with_refresh(exec, false).await
    }

    async fn get_active_with_refresh(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
        refresh: bool,
    ) -> crate::Result<Option<Self>> {
        let res = sqlx::query_as::<_, StoredCredentials>(
            "
            SELECT
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login,
                yggdrasil_client_token
            FROM minecraft_users
            WHERE active = TRUE
            ",
        )
        .fetch_optional(exec)
        .await?;

        Ok(match res {
            Some(x) => {
                let mut credentials = Self::from_stored(x);
                if refresh {
                    credentials.refresh(exec).await.ok();
                }
                Some(credentials)
            }
            None => None,
        })
    }

    pub async fn get_all(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Vec<Self>> {
        Self::get_all_with_refresh(exec, true).await
    }

    pub async fn get_all_without_refresh(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Vec<Self>> {
        Self::get_all_with_refresh(exec, false).await
    }

    async fn get_all_with_refresh(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
        refresh: bool,
    ) -> crate::Result<Vec<Self>> {
        let res = sqlx::query_as::<_, StoredCredentials>(
            "
            SELECT
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login,
                yggdrasil_client_token
            FROM minecraft_users
            ",
        )
        .fetch(exec)
        .try_fold(Vec::new(), |mut acc, x| {
            let mut credentials = Self::from_stored(x);

            async move {
                if refresh {
                    credentials.refresh(exec).await.ok();
                }
                acc.push(credentials);

                Ok(acc)
            }
        })
        .await?;

        Ok(res)
    }

    pub async fn get_offline_credential(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Self>> {
        let active = Self::get_active_without_refresh(exec).await?;
        if active.as_ref().is_some_and(Self::is_offline) {
            return Ok(active);
        }

        let users = Self::get_all_without_refresh(exec).await?;
        Ok(users
            .into_iter()
            .filter(Self::is_offline)
            .min_by(|left, right| {
                left.offline_profile.name.cmp(&right.offline_profile.name)
            }))
    }

    pub async fn upsert(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        let profile = self.maybe_online_profile().await;
        let expires = self.expires.timestamp();
        let uuid = profile.id.as_hyphenated().to_string();
        let account_type = self.account_type.as_database();
        let account_id = self.account_id();

        let yggdrasil_api_root = self
            .yggdrasil
            .as_ref()
            .map_or("", |account| account.api_root.as_str());
        let yggdrasil_server_name = self
            .yggdrasil
            .as_ref()
            .map_or("", |account| account.server_name.as_str());
        let yggdrasil_login = self
            .yggdrasil
            .as_ref()
            .map_or("", |account| account.login.as_str());
        let yggdrasil_client_token = self
            .yggdrasil
            .as_ref()
            .map_or("", |account| account.client_token.as_str());

        if self.active {
            sqlx::query!(
                "
                UPDATE minecraft_users
                SET active = FALSE
                ",
            )
            .execute(exec)
            .await?;
        }

        sqlx::query(
            "
            INSERT INTO minecraft_users (
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login,
                yggdrasil_client_token
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (account_id) DO UPDATE SET
                uuid = $2,
                active = $3,
                username = $4,
                account_type = $5,
                access_token = $6,
                refresh_token = $7,
                expires = $8,
                yggdrasil_api_root = $9,
                yggdrasil_server_name = $10,
                yggdrasil_login = $11,
                yggdrasil_client_token = $12
            ",
        )
        .bind(account_id)
        .bind(uuid)
        .bind(self.active)
        .bind(&profile.name)
        .bind(account_type)
        .bind(&self.access_token)
        .bind(&self.refresh_token)
        .bind(expires)
        .bind(yggdrasil_api_root)
        .bind(yggdrasil_server_name)
        .bind(yggdrasil_login)
        .bind(yggdrasil_client_token)
        .execute(exec)
        .await?;

        Ok(())
    }

    pub async fn remove(
        account_id: &str,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM minecraft_users WHERE account_id = $1")
            .bind(account_id)
            .execute(exec)
            .await?;

        Ok(())
    }
}

impl Serialize for Credentials {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Opportunistically hydrate the profile with its online data if possible for frontend
        // consumption, transparently handling all the possible Tokio runtime states the current
        // thread may be in the most efficient way
        let profile = if self.is_offline() {
            MaybeOnlineMinecraftProfile::Offline(&self.offline_profile)
        } else {
            match Handle::try_current().ok() {
                Some(runtime)
                    if runtime.runtime_flavor()
                        == RuntimeFlavor::CurrentThread =>
                {
                    runtime.block_on(self.maybe_online_profile())
                }
                Some(runtime) => task::block_in_place(|| {
                    runtime.block_on(self.maybe_online_profile())
                }),
                None => tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_or_else(
                        |_| {
                            MaybeOnlineMinecraftProfile::Offline(
                                &self.offline_profile,
                            )
                        },
                        |runtime| runtime.block_on(self.maybe_online_profile()),
                    ),
            }
        };

        let mut ser = serializer.serialize_struct("Credentials", 8)?;
        ser.serialize_field("account_id", &self.account_id())?;
        ser.serialize_field("profile", &*profile)?;
        ser.serialize_field("account_type", &self.account_type)?;
        ser.serialize_field("access_token", &self.access_token)?;
        ser.serialize_field("refresh_token", &self.refresh_token)?;
        ser.serialize_field("expires", &self.expires)?;
        ser.serialize_field("active", &self.active)?;
        ser.serialize_field("yggdrasil", &self.yggdrasil)?;
        ser.end()
    }
}

#[cfg(test)]
mod offline_account_tests {
    use super::*;

    #[test]
    fn creates_java_compatible_offline_uuid() {
        let credentials = Credentials::offline("Notch").unwrap();

        assert_eq!(
            credentials.offline_profile.id,
            Uuid::parse_str("b50ad385-829d-3141-a216-7e7d7539ba7f").unwrap()
        );
        assert_eq!(credentials.account_type, MinecraftAccountType::Offline);
        assert_eq!(credentials.access_token, "0");
    }

    #[test]
    fn creates_offline_account_with_custom_uuid() {
        let uuid = Uuid::new_v4();
        let credentials =
            Credentials::offline_with_uuid("CustomUuid", uuid).unwrap();

        assert_eq!(credentials.offline_profile.id, uuid);
        assert_eq!(credentials.offline_profile.name, "CustomUuid");
        assert_eq!(credentials.account_type, MinecraftAccountType::Offline);
        assert_eq!(credentials.access_token, "0");
    }

    #[test]
    fn validates_offline_username() {
        assert!(Credentials::offline("abc").is_ok());
        assert!(Credentials::offline("Player_123").is_ok());
        assert!(Credentials::offline("玩家").is_ok());
        assert!(Credentials::offline("玩家_123").is_ok());
        assert!(Credentials::offline("").is_err());
        assert!(Credentials::offline("player name").is_err());
        assert!(Credentials::offline("玩家!").is_err());
        assert!(
            Credentials::offline(
                "这是一个超过十六个字符长度的中文离线玩家名称"
            )
            .is_err()
        );
    }

    #[test]
    fn serializes_new_offline_account_id_for_frontend_selection() {
        let credentials = Credentials::offline("OfflineUser").unwrap();
        let serialized = serde_json::to_value(&credentials).unwrap();

        assert_eq!(serialized["account_id"], credentials.account_id());
    }

    #[tokio::test]
    async fn persists_offline_account_as_active() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let credentials = Credentials::offline("OfflineUser").unwrap();
        credentials.upsert(&pool).await.unwrap();

        let stored = Credentials::get_active(&pool).await.unwrap().unwrap();
        assert_eq!(stored.offline_profile.name, "OfflineUser");
        assert_eq!(stored.offline_profile.id, credentials.offline_profile.id);
        assert!(stored.is_offline());
        assert!(stored.active);
    }

    #[tokio::test]
    async fn persists_offline_account_with_custom_uuid() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let uuid = Uuid::new_v4();
        let credentials =
            Credentials::offline_with_uuid("CustomUuid", uuid).unwrap();
        credentials.upsert(&pool).await.unwrap();

        let stored = Credentials::get_active(&pool).await.unwrap().unwrap();
        assert_eq!(stored.offline_profile.id, uuid);
        assert_eq!(stored.offline_profile.name, "CustomUuid");
        assert!(stored.is_offline());
    }

    #[tokio::test]
    async fn preserves_accounts_with_the_same_uuid_across_account_types() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let uuid = Uuid::new_v4();
        sqlx::query(
            "
            INSERT INTO minecraft_users (
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires
            )
            VALUES ($1, $2, TRUE, $3, 'yggdrasil', $4, $5, $6)
            ",
        )
        .bind(format!("yggdrasil:https://example.invalid:{uuid}"))
        .bind(uuid.as_hyphenated().to_string())
        .bind("ThirdPartyUser")
        .bind("third-party-token")
        .bind("")
        .bind(0_i64)
        .execute(&pool)
        .await
        .unwrap();

        let offline =
            Credentials::offline_with_uuid("OfflineUser", uuid).unwrap();
        offline.upsert(&pool).await.unwrap();

        let stored = Credentials::get_all_without_refresh(&pool).await.unwrap();
        assert_eq!(stored.len(), 2);
        assert!(stored.iter().any(Credentials::is_yggdrasil));
        assert!(stored.iter().any(Credentials::is_offline));
        assert_ne!(stored[0].account_id(), stored[1].account_id());
    }

    #[tokio::test]
    async fn selects_offline_account_when_online_account_is_active() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let mut offline = Credentials::offline("OfflineUser").unwrap();
        offline.active = false;
        offline.upsert(&pool).await.unwrap();

        sqlx::query(
            "
            INSERT INTO minecraft_users (
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires
            )
            VALUES ($1, $2, TRUE, $3, 'microsoft', $4, $5, $6)
            ",
        )
        .bind("microsoft:test-account")
        .bind(Uuid::new_v4().as_hyphenated().to_string())
        .bind("OnlineUser")
        .bind("expired-access-token")
        .bind("expired-refresh-token")
        .bind(0_i64)
        .execute(&pool)
        .await
        .unwrap();

        let selected = Credentials::get_offline_credential(&pool)
            .await
            .unwrap()
            .unwrap();

        assert!(selected.is_offline());
        assert_eq!(selected.offline_profile.id, offline.offline_profile.id);
    }
}

pub struct DeviceTokenPair {
    pub token: DeviceToken,
    pub key: DeviceTokenKey,
}

impl DeviceTokenPair {
    pub async fn upsert(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let uuid = self.key.id.as_hyphenated().to_string();
        let issue_instant = self.token.issue_instant.timestamp();
        let not_after = self.token.not_after.timestamp();
        let key = self
            .key
            .key
            .to_pkcs8_pem(LineEnding::default())
            .map_err(MinecraftAuthenticationError::PEMSerialize)?
            .to_string();
        let display_claims = serde_json::to_string(&self.token.display_claims)?;

        sqlx::query!(
            "
            INSERT INTO minecraft_device_tokens (id, uuid, private_key, x, y, issue_instant, not_after, token, display_claims)
            VALUES (0, $1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                uuid = $1,
                private_key = $2,
                x = $3,
                y = $4,
                issue_instant = $5,
                not_after = $6,
                token = $7,
                display_claims = jsonb($8)
            ",
            uuid,
            key,
            self.key.x,
            self.key.y,
            issue_instant,
            not_after,
            self.token.token,
            display_claims,
        )
            .execute(exec)
            .await?;

        Ok(())
    }
}

const MICROSOFT_CLIENT_ID: &str = "c7104738-eff0-4fc9-a4ad-59e2c72ec691";
const AUTH_REPLY_URL: &str = "https://login.live.com/oauth20_desktop.srf";
const BROWSER_AUTH_REPLY_URL: &str =
    "http://127.0.0.1:53682/oauth/microsoft/callback";
const MICROSOFT_AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const REQUESTED_SCOPE: &str = "XboxLive.signin offline_access";
pub const MINECRAFT_SERVICES_USER_AGENT: &str = "Axolotl Launcher";

pub struct RequestWithDate<T> {
    pub date: DateTime<Utc>,
    pub value: T,
}

// flow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceToken {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: String,
    pub display_claims: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct OAuthToken {
    // pub token_type: String,
    pub expires_in: u64,
    // pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    // pub user_id: String,
    // pub foci: String,
}

#[derive(Deserialize)]
struct MicrosoftOAuthError {
    error: String,
}

#[tracing::instrument]
async fn oauth_token(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("code", code);
    query.insert("code_verifier", verifier);
    query.insert("grant_type", "authorization_code");
    query.insert("redirect_uri", redirect_uri);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        authentication_client()
            .post(MICROSOFT_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

#[tracing::instrument]
async fn oauth_refresh(
    refresh_token: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("refresh_token", refresh_token);
    query.insert("grant_type", "refresh_token");
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        authentication_client()
            .post(MICROSOFT_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::RefreshOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::RefreshOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::RefreshOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxLiveToken {
    token: String,
}

#[tracing::instrument(skip(access_token))]
async fn xbox_live_authorize(
    access_token: &str,
) -> Result<RequestWithDate<XboxLiveToken>, MinecraftAuthenticationError> {
    send_xbox_json_request(
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
        MinecraftAuthStep::SisuAuthorize,
    )
    .await
}

#[tracing::instrument(skip(xbl_token))]
async fn xsts_authorize_xbl(
    xbl_token: &str,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthenticationError> {
    send_xbox_json_request(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token],
            },
        }),
        MinecraftAuthStep::XstsAuthorize,
    )
    .await
}

async fn send_xbox_json_request<T: DeserializeOwned>(
    url: &str,
    body: serde_json::Value,
    step: MinecraftAuthStep,
) -> Result<RequestWithDate<T>, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        authentication_client()
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;
    let status = res.status();
    let date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request { source, step }
    })?;
    let value = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate { date, value })
}

#[derive(Deserialize)]
struct MinecraftToken {
    // pub username: String,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u64,
}

#[tracing::instrument]
async fn minecraft_token(
    token: DeviceToken,
) -> Result<MinecraftToken, MinecraftAuthenticationError> {
    let uhs = token
        .display_claims
        .get("xui")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|x| x.as_str().map(String::from))
        .ok_or_else(|| MinecraftAuthenticationError::NoUserHash)?;

    let token = token.token;

    let url = mojang_service_url(
        "https://api.minecraftservices.com/launcher/login",
        should_use_mojang_mirror(),
    );
    let res = auth_retry(|| {
        authentication_client()
            .post(url.as_ref())
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .json(&json!({
                "platform": "PC_LAUNCHER",
                "xtoken": format!("XBL3.0 x={uhs};{token}"),
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftToken,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftToken,
        }
    })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftToken,
            status_code: status,
        }
    })
}

#[derive(
    sqlx::Type, Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq,
)]
#[serde(rename_all = "UPPERCASE")]
#[sqlx(rename_all = "UPPERCASE")]
pub enum MinecraftSkinVariant {
    /// The classic player model, with arms that are 4 pixels wide.
    Classic,
    /// The slim player model, with arms that are 3 pixels wide.
    Slim,
    /// The player model is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MinecraftCharacterExpressionState {
    /// This expression is selected for being displayed ingame.
    ///
    /// At the moment, at most one expression can be selected at a time.
    Active,
    /// This expression is not selected for being displayed ingame.
    Inactive,
    /// The expression selection status is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftSkin {
    /// The UUID of this skin object.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this UUID
    /// changes every time the player changes their skin, even if the skin
    /// texture is the same as before.
    pub id: Uuid,
    /// The selection state of the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always `ACTIVE`, as only a single skin representing the current
    /// skin is returned.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint the file
    /// name for this URL is a hash of the skin texture, so that different
    /// players using the same skin texture will share a texture URL.
    pub url: Arc<Url>,
    /// A hash of the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always set and the same as the file name of the skin texture URL.
    #[serde(
        default, // Defensive handling of unexpected Mojang API return values to
                 // prevent breaking the entire profile parsing
        rename = "textureKey"
    )]
    pub texture_key: Option<Arc<str>>,
    /// The player model variant this skin is for.
    pub variant: MinecraftSkinVariant,
    /// User-friendly name for the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this is
    /// only set if the player has not set a custom skin, and this skin object
    /// is therefore the default skin for the player's UUID.
    #[serde(
        default,
        rename = "alias",
        deserialize_with = "normalize_skin_alias_case"
    )]
    pub name: Option<String>,
}

impl MinecraftSkin {
    /// Robustly computes the texture key for this skin, falling back to its
    /// URL file name and finally to the skin UUID when necessary.
    pub fn texture_key(&self) -> Arc<str> {
        self.texture_key.as_ref().cloned().unwrap_or_else(|| {
            self.url
                .path_segments()
                .and_then(|mut path_segments| {
                    path_segments.next_back().map(String::from)
                })
                .unwrap_or_else(|| self.id.as_simple().to_string())
                .into()
        })
    }
}

fn normalize_skin_alias_case<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // Skin aliases have been spotted to be returned in all caps, so make sure
    // they are normalized to a prettier title case
    Ok(<Option<Cow<'_, str>>>::deserialize(deserializer)?
        .map(|alias| alias.to_title_case()))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftCape {
    /// The UUID of the cape.
    pub id: Uuid,
    /// The selection state of the cape.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the cape texture.
    pub url: Arc<Url>,
    /// The user-friendly name for the cape.
    #[serde(rename = "alias")]
    pub name: Arc<str>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct MinecraftProfile {
    /// The UUID of the player.
    #[serde(default)]
    pub id: Uuid,
    /// The username of the player.
    pub name: String,
    /// The skins the player is known to have.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint every
    /// player has a single skin.
    pub skins: Vec<MinecraftSkin>,
    /// The capes the player is known to have.
    pub capes: Vec<MinecraftCape>,
    /// The instant when the profile was fetched. See also [Self::is_fresh].
    #[serde(skip)]
    pub fetch_time: Option<Instant>,
}

impl MinecraftProfile {
    /// Checks whether the profile data is fresh (i.e., highly likely to be
    /// up-to-date because it was fetched recently) or stale. If it is not
    /// known when this profile data has been fetched from Mojang servers (i.e.,
    /// `fetch_time` is `None`), the profile is considered stale.
    ///
    /// This can be used to determine if the profile data should be fetched again
    /// from the Mojang API: the vanilla launcher was seen refreshing profile
    /// data every 60 seconds when re-entering the skin selection screen, and
    /// external applications may change this data at any time.
    fn is_fresh(&self, max_age: std::time::Duration) -> bool {
        self.fetch_time.is_some_and(|last_profile_fetch_time| {
            Instant::now().saturating_duration_since(last_profile_fetch_time)
                < max_age
        })
    }

    /// Returns the currently selected skin for this profile.
    pub fn current_skin(&self) -> crate::Result<&MinecraftSkin> {
        Ok(self
            .skins
            .iter()
            .find(|skin| {
                skin.state == MinecraftCharacterExpressionState::Active
            })
            // There should always be one active skin, even when the player uses their default skin
            .ok_or_else(|| {
                ErrorKind::OtherError("No active skin found".into())
            })?)
    }

    /// Returns the currently selected cape for this profile.
    pub fn current_cape(&self) -> Option<&MinecraftCape> {
        self.capes.iter().find(|cape| {
            cape.state == MinecraftCharacterExpressionState::Active
        })
    }
}

pub enum MaybeOnlineMinecraftProfile<'profile> {
    /// An online profile, fetched from the Mojang API.
    Online(Arc<MinecraftProfile>),
    /// An offline profile, which has not been fetched from the Mojang API.
    Offline(&'profile MinecraftProfile),
}

impl Deref for MaybeOnlineMinecraftProfile<'_> {
    type Target = MinecraftProfile;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Online(profile) => profile,
            Self::Offline(profile) => profile,
        }
    }
}

#[tracing::instrument(skip(token))]
async fn minecraft_profile(
    token: &str,
) -> Result<MinecraftProfile, MinecraftAuthenticationError> {
    let url = mojang_service_url(
        "https://api.minecraftservices.com/minecraft/profile",
        should_use_mojang_mirror(),
    );
    let res = auth_retry(|| {
        authentication_client()
            .get(url.as_ref())
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .bearer_auth(token)
            // Profiles may be refreshed periodically in response to user actions,
            // so we want each refresh to be fast
            .timeout(std::time::Duration::from_secs(10))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftProfile,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftProfile,
        }
    })?;

    let mut profile =
        serde_json::from_str::<MinecraftProfile>(&text).map_err(|source| {
            MinecraftAuthenticationError::DeserializeResponse {
                source,
                raw: text,
                step: MinecraftAuthStep::MinecraftProfile,
                status_code: status,
            }
        })?;
    profile.fetch_time = Some(Instant::now());

    tracing::debug!(
        "Successfully fetched Minecraft profile for {}",
        profile.name
    );

    Ok(profile)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {
    items: Vec<MinecraftEntitlement>,
}

#[derive(Deserialize)]
struct MinecraftEntitlement {
    name: String,
}

#[tracing::instrument]
async fn minecraft_entitlements(
    token: &str,
) -> Result<MinecraftEntitlements, MinecraftAuthenticationError> {
    let request_url = format!(
        "https://api.minecraftservices.com/entitlements/license?requestId={}",
        Uuid::new_v4()
    );
    let url = mojang_service_url(&request_url, should_use_mojang_mirror());
    let res = auth_retry(|| {
        authentication_client()
            .get(url.as_ref())
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .bearer_auth(token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftEntitlements,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftEntitlements,
        }
    })?;

    let entitlements = serde_json::from_str::<MinecraftEntitlements>(&text)
        .map_err(|source| {
            MinecraftAuthenticationError::DeserializeResponse {
                source,
                raw: text,
                step: MinecraftAuthStep::MinecraftEntitlements,
                status_code: status,
            }
        })?;

    if entitlements
        .items
        .iter()
        .any(|item| item.name == "game_minecraft")
    {
        Ok(entitlements)
    } else {
        Err(MinecraftAuthenticationError::NoMinecraftEntitlement)
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::util::download::proxy_context::with_snapshot;
    use crate::util::fetch::DownloadClients;
    use crate::util::proxy::{ProxyConfig, ProxyMode};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn xbox_auth_uses_the_configured_proxy_and_direct_modes() {
        for mode in [ProxyMode::Custom, ProxyMode::None] {
            let listener =
                tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 1024];
                    let read = stream.read(&mut buffer).await.unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&buffer[..read]);
                    if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}").await.unwrap();
                String::from_utf8(bytes).unwrap()
            });
            let clients = DownloadClients::build(
                &ProxyConfig {
                    mode,
                    // In direct mode this deliberately unusable proxy must be ignored.
                    url: if mode == ProxyMode::Custom {
                        format!("http://{address}")
                    } else {
                        "http://127.0.0.1:1".into()
                    },
                    ..Default::default()
                },
                false,
                false,
            )
            .unwrap();
            let url = if mode == ProxyMode::Custom {
                "http://auth.invalid/xbox".into()
            } else {
                format!("http://{address}/xbox")
            };
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                with_snapshot(
                    clients,
                    send_xbox_json_request::<serde_json::Value>(
                        &url,
                        json!({"token":"test"}),
                        MinecraftAuthStep::XstsAuthorize,
                    ),
                ),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(response.value, json!({"ok":true}));
            let request = server.await.unwrap();
            assert!(request.starts_with(if mode == ProxyMode::Custom {
                "POST http://auth.invalid/xbox "
            } else {
                "POST /xbox "
            }));
        }
    }
}

// auth utils
#[tracing::instrument(skip(reqwest_request))]
async fn auth_retry<F>(
    reqwest_request: impl Fn() -> F,
) -> Result<reqwest::Response, reqwest::Error>
where
    F: Future<Output = Result<Response, reqwest::Error>>,
{
    const RETRY_COUNT: usize = 5; // Does command 9 times
    const RETRY_WAIT: std::time::Duration =
        std::time::Duration::from_millis(250);

    let mut resp = reqwest_request().await;
    for i in 0..RETRY_COUNT {
        match &resp {
            Ok(_) => {
                break;
            }
            Err(err) => {
                if err.is_connect() || err.is_timeout() {
                    if i < RETRY_COUNT - 1 {
                        tracing::debug!(
                            "Request failed with connect error, retrying...",
                        );
                        tokio::time::sleep(RETRY_WAIT).await;
                        resp = reqwest_request().await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    resp
}

pub struct DeviceTokenKey {
    pub id: Uuid,
    pub key: SigningKey,
    pub x: String,
    pub y: String,
}

#[tracing::instrument]
fn get_date_header(headers: &HeaderMap) -> DateTime<Utc> {
    headers
        .get(reqwest::header::DATE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| DateTime::parse_from_rfc2822(x).ok())
        .map_or(Utc::now(), |x| x.with_timezone(&Utc))
}

#[tracing::instrument]
fn generate_oauth_challenge() -> String {
    let mut rng = rand::thread_rng();

    let bytes: Vec<u8> = (0..64).map(|_| rng.r#gen::<u8>()).collect();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod refresh_coalescing_tests {
    use super::*;
    fn credential() -> Credentials {
        let mut credential =
            Credentials::offline_with_uuid("RefreshTest", Uuid::new_v4())
                .unwrap();
        credential.account_type = MinecraftAccountType::Microsoft;
        credential.refresh_token = "old-refresh".into();
        credential.expires = Utc::now() - Duration::minutes(1);
        credential
    }
    #[tokio::test]
    async fn concurrent_readers_reuse_completed_refresh_without_network() {
        let mut first = credential();
        let mut second = Credentials::offline_with_uuid(
            "RefreshTest",
            first.offline_profile.id,
        )
        .unwrap();
        second.account_type = first.account_type;
        second.account_id = first.account_id.clone();
        second.refresh_token = first.refresh_token.clone();
        second.expires = first.expires;
        let key = first.account_id();
        CREDENTIAL_REFRESHES.insert(
            key.clone(),
            Arc::new(Mutex::new(CredentialRefreshState {
                source_token: Some("old-refresh".into()),
                failure: None,
                refreshed: Some(RefreshedTokens {
                    access_token: "fresh-access".into(),
                    refresh_token: "fresh-refresh".into(),
                    expires: Utc::now() + Duration::hours(1),
                    yggdrasil: None,
                }),
            })),
        );
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let (a, b) = tokio::join!(first.refresh(&pool), second.refresh(&pool));
        a.unwrap();
        b.unwrap();
        assert_eq!(first.access_token, "fresh-access");
        assert_eq!(second.refresh_token, "fresh-refresh");
        CREDENTIAL_REFRESHES.remove(&key);
    }
    #[tokio::test]
    async fn refresh_backoff_preserves_error_kind_and_new_login_bypasses_old_failure()
     {
        let mut credential = credential();
        let key = credential.account_id();
        let kind = Arc::new(ErrorKind::InputError("test rejection".into()));
        CREDENTIAL_REFRESHES.insert(
            key.clone(),
            Arc::new(Mutex::new(CredentialRefreshState {
                source_token: Some(credential.refresh_token.clone()),
                refreshed: None,
                failure: Some((Instant::now(), kind.clone())),
            })),
        );
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let error = credential.refresh(&pool).await.unwrap_err();
        assert!(Arc::ptr_eq(&error.raw, &kind));
        credential.refresh_token = "new-login".into();
        credential.expires = Utc::now() + Duration::hours(1);
        credential.refresh(&pool).await.unwrap();
        CREDENTIAL_REFRESHES.remove(&key);
    }
}
