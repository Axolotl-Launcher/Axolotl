//! Third-party grants stay in the native runtime; webviews receive only account summaries.

use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use crate::state::store::secrets;
use crate::util::oauth_credentials::{self, Store};

const REDIRECT_URI: &str = "axolotl://oauth/callback";
const HOST: &str = "https://www.mcimg.com";
const CLIENT_ID: &str = "axolotl";
const FLOW_TTL: u64 = 300;
const FLOW_CREDENTIAL: &str = "pending-authorization";
static SESSION: LazyLock<Mutex<Session>> = LazyLock::new(Mutex::default);

pub mod shimu;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub user_id: u64,
    pub user_name: String,
    #[serde(default)]
    pub nick_name: Option<String>,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct Connection {
    access_token: String,
    expires_at: u64,
    user: AccountProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    Disconnected,
    Connected,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthorizationStatus {
    Waiting,
    Exchanging,
    Denied,
    TimedOut,
    Failed,
    Succeeded,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub provider_id: &'static str,
    pub available: bool,
    pub status: AccountStatus,
    pub user: Option<AccountProfile>,
    pub expires_at: Option<u64>,
    pub flow_id: Option<String>,
    pub authorization: Option<AuthorizationStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationStart {
    pub flow_id: String,
    pub authorization_url: String,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct PendingFlow {
    state: String,
    #[serde(default)]
    code_verifier: String,
    deadline: u64,
    exchanging: bool,
}

#[derive(Clone, Default)]
struct Session {
    restored: bool,
    pending: Option<PendingFlow>,
    outcome: Option<AuthorizationStatus>,
    completed_flow_id: Option<String>,
}

impl Session {
    fn restore(store: &impl Store, now: u64) -> crate::Result<Self> {
        let pending = store
            .read(FLOW_CREDENTIAL)?
            .map(|value| {
                serde_json::from_str::<PendingFlow>(&value)
                    .map_err(|_| storage_error())
            })
            .transpose()?;
        let previous = pending.clone();
        let mut session = Self {
            pending,
            restored: true,
            ..Self::default()
        };
        session.expire(now);
        if session.pending.as_ref().is_some_and(|flow| {
            flow.exchanging || !valid_code_verifier(&flow.code_verifier)
        }) {
            // Never replay a previously claimed exchange or resume a legacy
            // flow that was started without a PKCE challenge.
            session.completed_flow_id =
                session.pending.take().map(|flow| flow.state);
            session.outcome = Some(AuthorizationStatus::Failed);
        }
        if session.pending != previous {
            session.persist_pending(store)?;
        }
        Ok(session)
    }

    fn persist_pending(&self, store: &impl Store) -> crate::Result<()> {
        let value = self
            .pending
            .as_ref()
            .map(|flow| {
                serde_json::to_string(flow).map_err(|_| storage_error())
            })
            .transpose()?;
        store.write(FLOW_CREDENTIAL, value.as_deref())
    }

    fn update<T>(
        &mut self,
        store: &impl Store,
        update: impl FnOnce(&mut Self) -> T,
    ) -> crate::Result<T> {
        let previous = self.clone();
        let result = update(self);
        if self.pending != previous.pending
            && let Err(error) = self.persist_pending(store)
        {
            *self = previous;
            return Err(error);
        }
        Ok(result)
    }

    fn cancel(&mut self, flow_id: &str) {
        if self
            .pending
            .as_ref()
            .is_some_and(|flow| flow.state == flow_id)
        {
            self.pending = None;
            self.outcome = None;
            self.completed_flow_id = None;
        }
    }

    fn complete(
        &mut self,
        state: &str,
        result: crate::Result<Connection>,
        store: &impl Store,
        now: u64,
    ) {
        self.expire(now);
        if !self
            .pending
            .as_ref()
            .is_some_and(|flow| flow.state == state && flow.exchanging)
        {
            return;
        }
        let result = result.and_then(|connection| {
            let value = serde_json::to_string(&connection)
                .map_err(|_| storage_error())?;
            oauth_credentials::write(store, Some(&value))
        });
        self.pending = None;
        self.completed_flow_id = Some(state.to_string());
        self.outcome = Some(if result.is_ok() {
            AuthorizationStatus::Succeeded
        } else {
            AuthorizationStatus::Failed
        });
    }

    fn expire(&mut self, now: u64) {
        if self
            .pending
            .as_ref()
            .is_some_and(|flow| flow.deadline <= now)
        {
            self.completed_flow_id =
                self.pending.as_ref().map(|flow| flow.state.clone());
            self.pending = None;
            self.outcome = Some(AuthorizationStatus::TimedOut);
        }
    }

    fn claim(&mut self, callback: &Url, now: u64) -> Option<String> {
        self.expire(now);
        let flow = self.pending.as_mut()?;
        if flow.exchanging || !valid_code_verifier(&flow.code_verifier) {
            return None;
        }
        let mut state = None;
        let mut code = None;
        let mut error = None;
        for (key, value) in callback.query_pairs() {
            let slot = match key.as_ref() {
                "state" => &mut state,
                "code" => &mut code,
                "error" => &mut error,
                _ => continue,
            };
            if slot.replace(value.into_owned()).is_some() {
                return None;
            }
        }
        if state.as_deref() != Some(flow.state.as_str()) {
            return None;
        }
        match (code, error) {
            (Some(code), None) if !code.is_empty() && code.len() <= 4096 => {
                flow.exchanging = true;
                Some(code)
            }
            (None, Some(error)) => {
                self.completed_flow_id =
                    self.pending.as_ref().map(|flow| flow.state.clone());
                self.pending = None;
                self.outcome = Some(if error == "access_denied" {
                    AuthorizationStatus::Denied
                } else {
                    AuthorizationStatus::Failed
                });
                None
            }
            _ => None,
        }
    }
}

struct CredentialStore;

impl Store for CredentialStore {
    fn read(&self, name: &str) -> crate::Result<Option<String>> {
        secrets::read_entry(&secrets::key(&["third-party", "shimu", name]))
    }

    fn write(&self, name: &str, value: Option<&str>) -> crate::Result<()> {
        secrets::write_entry(
            &secrets::key(&["third-party", "shimu", name]),
            value,
        )
    }
}

fn failure(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.to_string()).into()
}

fn storage_error() -> crate::Error {
    failure(
        "Could not access third-party account credentials in the system credential store",
    )
}

fn session() -> crate::Result<std::sync::MutexGuard<'static, Session>> {
    let mut session = SESSION
        .lock()
        .map_err(|_| failure("Third-party account state is unavailable"))?;
    if !session.restored {
        *session = Session::restore(&CredentialStore, now())?;
    }
    Ok(session)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_code_verifier() -> crate::Result<String> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes).map_err(|_| {
        failure("Could not generate the Shimu authorization proof")
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn valid_code_verifier(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~".contains(&byte))
}

fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn read_connection(store: &impl Store) -> crate::Result<Option<Connection>> {
    oauth_credentials::read(store)?
        .map(|value| serde_json::from_str(&value).map_err(|_| storage_error()))
        .transpose()
}

fn summary(
    connection: Option<Connection>,
    session: &Session,
    now: u64,
) -> AccountSummary {
    AccountSummary {
        provider_id: "shimu",
        available: true,
        status: match &connection {
            None => AccountStatus::Disconnected,
            Some(connection) if connection.expires_at <= now => {
                AccountStatus::Expired
            }
            Some(_) => AccountStatus::Connected,
        },
        expires_at: connection.as_ref().map(|connection| connection.expires_at),
        flow_id: session
            .pending
            .as_ref()
            .map(|flow| flow.state.clone())
            .or_else(|| session.completed_flow_id.clone()),
        user: connection.map(|connection| connection.user),
        authorization: session
            .pending
            .as_ref()
            .map(|flow| {
                if flow.exchanging {
                    AuthorizationStatus::Exchanging
                } else {
                    AuthorizationStatus::Waiting
                }
            })
            .or(session.outcome),
    }
}

pub fn get_account() -> crate::Result<AccountSummary> {
    let mut session = session()?;
    session.update(&CredentialStore, |session| session.expire(now()))?;
    Ok(summary(read_connection(&CredentialStore)?, &session, now()))
}

pub fn begin_authorization() -> crate::Result<AuthorizationStart> {
    let mut session = session()?;
    session.update(&CredentialStore, |session| session.expire(now()))?;
    if session.pending.is_some() {
        return Err(failure("An authorization is already in progress"));
    }
    let state = Uuid::new_v4().to_string();
    let code_verifier = new_code_verifier()?;
    let url = authorization_url(&state, &code_verifier);
    session.update(&CredentialStore, |session| {
        session.pending = Some(PendingFlow {
            state: state.clone(),
            code_verifier,
            deadline: now() + FLOW_TTL,
            exchanging: false,
        });
        session.outcome = None;
        session.completed_flow_id = None;
    })?;
    Ok(AuthorizationStart {
        flow_id: state,
        authorization_url: url,
    })
}

fn authorization_url(state: &str, code_verifier: &str) -> String {
    let mut url = Url::parse(&format!("{HOST}/oauth/authorize"))
        .expect("fixed OAuth URL");
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", "user:info,post:read,file:download")
        .append_pair("state", state)
        .append_pair("response_type", "code")
        .append_pair("code_challenge", &code_challenge(code_verifier))
        .append_pair("code_challenge_method", "S256");
    url.into()
}

pub fn cancel_authorization(flow_id: &str) -> crate::Result<()> {
    let mut session = session()?;
    session.update(&CredentialStore, |session| session.cancel(flow_id))
}

pub fn disconnect() -> crate::Result<()> {
    let mut session = session()?;
    // Invalidate an in-flight exchange before it can commit its result.
    session.update(&CredentialStore, |session| {
        session.pending = None;
        session.outcome = None;
        session.completed_flow_id = None;
    })?;
    oauth_credentials::write(&CredentialStore, None)
}

pub fn is_callback(command: &str) -> bool {
    callback_url(command).is_some()
}

/// Includes malformed OAuth links so rejected callbacks never expose codes in logs.
pub fn is_callback_candidate(command: &str) -> bool {
    Url::parse(command).is_ok_and(|url| {
        url.scheme() == "axolotl" && url.host_str() == Some("oauth")
    })
}

fn callback_url(command: &str) -> Option<Url> {
    let url = Url::parse(command).ok()?;
    (url.scheme() == "axolotl"
        && url.host_str() == Some("oauth")
        && url.path() == "/callback"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

pub async fn handle_callback(command: &str) -> crate::Result<()> {
    let Some(url) = callback_url(command) else {
        return Ok(());
    };
    let claimed = {
        let mut session = session()?;
        session
            .update(&CredentialStore, |session| session.claim(&url, now()))?
            .map(|code| {
                let flow =
                    session.pending.as_ref().expect("claimed authorization");
                (code, flow.state.clone(), flow.code_verifier.clone())
            })
    };
    let Some((code, flow_state, code_verifier)) = claimed else {
        return Ok(());
    };
    let exchanged = exchange_code(&code, &code_verifier).await;
    let mut session = session()?;
    session.update(&CredentialStore, |session| {
        session.complete(&flow_state, exchanged, &CredentialStore, now());
    })
}

async fn http_client() -> crate::Result<reqwest::Client> {
    let proxy = match crate::State::get_if_initialized() {
        Some(state) => state.proxy_config().await?,
        None => crate::util::proxy::ProxyConfig::default(),
    };
    proxy
        .apply(
            crate::util::http::client_builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .user_agent(crate::launcher_user_agent()),
        )?
        .build()
        .map_err(|_| failure("Could not initialize the Shimu connection"))
}

#[derive(Deserialize)]
struct TokenReply {
    code: u16,
    access_token: String,
    token_type: String,
    expires_in: u64,
    user: AccountProfile,
}

fn connection_from_reply(
    value: TokenReply,
    now: u64,
) -> crate::Result<Connection> {
    if value.code != 200
        || value.access_token.trim().is_empty()
        || !value.token_type.eq_ignore_ascii_case("bearer")
        || value.expires_in == 0
        || value.user.user_name.trim().is_empty()
    {
        return Err(failure(
            "Shimu returned an invalid authorization response",
        ));
    }
    Ok(Connection {
        access_token: value.access_token,
        expires_at: now
            .checked_add(value.expires_in)
            .ok_or_else(|| failure("Invalid authorization expiry"))?,
        user: value.user,
    })
}

fn token_form<'a>(
    code: &'a str,
    code_verifier: &'a str,
) -> crate::Result<[(&'static str, &'a str); 4]> {
    if !valid_code_verifier(code_verifier) {
        return Err(failure("Invalid Shimu authorization proof"));
    }
    Ok([
        ("code", code),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", code_verifier),
    ])
}

async fn exchange_code(
    code: &str,
    code_verifier: &str,
) -> crate::Result<Connection> {
    let form = token_form(code, code_verifier)?;
    let response = http_client()
        .await?
        .post(format!("{HOST}/shimu/api/oauth/token"))
        .form(&form)
        .send()
        .await
        .map_err(|_| failure("Could not connect to Shimu"))?;
    if !response.status().is_success() {
        return Err(failure("Shimu rejected the authorization exchange"));
    }
    let reply = response.json::<TokenReply>().await.map_err(|_| {
        failure("Shimu returned an invalid authorization response")
    })?;
    connection_from_reply(reply, now())
}

/// Validates this connection against the provider and returns its current identity.
/// Consumers use native service methods rather than receiving the bearer token over IPC.
pub async fn check_connection() -> crate::Result<AccountSummary> {
    let connection = {
        let _session = session()?;
        read_connection(&CredentialStore)?
            .ok_or_else(|| failure("Connect Shimu before using it"))?
    };
    if connection.expires_at <= now() {
        return get_account();
    }
    #[derive(Deserialize)]
    struct UserReply {
        code: u16,
        user: AccountProfile,
    }
    let response = http_client()
        .await?
        .get(format!("{HOST}/shimu/getInfo"))
        .bearer_auth(&connection.access_token)
        .send()
        .await
        .map_err(|_| failure("Could not connect to Shimu"))?;
    let unauthorized = response.status() == reqwest::StatusCode::UNAUTHORIZED;
    if !unauthorized && !response.status().is_success() {
        return Err(failure("Could not check the Shimu connection"));
    }
    let updated = if unauthorized {
        Connection {
            expires_at: 0,
            ..connection.clone()
        }
    } else {
        let value = response.json::<UserReply>().await.map_err(|_| {
            failure("Shimu returned an invalid account response")
        })?;
        if value.code != 200 || value.user.user_name.trim().is_empty() {
            return Err(failure("Could not check the Shimu connection"));
        }
        Connection {
            user: value.user,
            ..connection.clone()
        }
    };
    let session = session()?;
    write_checked_connection(&CredentialStore, &connection, &updated)?;
    Ok(summary(read_connection(&CredentialStore)?, &session, now()))
}

fn write_checked_connection(
    store: &impl Store,
    checked: &Connection,
    updated: &Connection,
) -> crate::Result<()> {
    // Compare the whole snapshot, including expiry, so a concurrent 401 cannot
    // be undone by a successful response started before that invalidation.
    if read_connection(store)?.as_ref() == Some(checked) {
        let value =
            serde_json::to_string(updated).map_err(|_| storage_error())?;
        oauth_credentials::write(store, Some(&value))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
