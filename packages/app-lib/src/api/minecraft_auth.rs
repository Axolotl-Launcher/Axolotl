//! Authentication flow interface

pub(super) mod profile_cache;

use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Serialize;
use std::time::Duration;
use uuid::Uuid;

use crate::State;
pub use crate::state::YggdrasilLoginResult;
use crate::state::{
    Credentials, MinecraftAccountType, MinecraftLoginFlow, MinecraftProfile,
    YggdrasilAccount,
};
pub use crate::state::{MinecraftDeviceLoginFlow, MinecraftDeviceLoginPoll};
use crate::util::fetch::authentication_client;
use crate::util::mojang::{mojang_service_url, should_use_mojang_mirror};

/// Budget for the Mojang reachability probes.
///
/// These probes decide whether Mojang requests go out directly or through the
/// Fallen mirror, and the frontend repeats the check every few minutes while
/// the launcher sits idle. Latency to these hosts on a proxied connection was
/// measured between roughly one and six seconds, so a budget inside that band
/// reports a healthy service as unreachable and latches the mirror for the
/// rest of the session — which then logs a connection failure every time the
/// idle check runs.
const REACHABILITY_TIMEOUT: Duration = Duration::from_secs(12);

#[tracing::instrument]
pub async fn check_reachable() -> crate::Result<()> {
    let statuses =
        probe_mojang_services(should_use_mojang_mirror(), true).await;
    if let Some(failed) = statuses.iter().find(|status| !status.reachable) {
        return Err(crate::ErrorKind::NetworkError(format!(
            "Mojang {} service is unreachable",
            failed.service,
        ))
        .into());
    }
    Ok(())
}

pub async fn set_mojang_auth_use_mirror(
    use_mirror: bool,
    expected_source: crate::state::DownloadSourceMode,
) -> crate::Result<bool> {
    let applied = crate::api::settings::apply_mojang_auth_source(
        expected_source,
        use_mirror,
    )
    .await?;
    if applied
        && use_mirror
        && matches!(
            expected_source,
            crate::state::DownloadSourceMode::Auto
                | crate::state::DownloadSourceMode::OfficialPreferred
        )
    {
        tracing::info!(
            "Mojang services are unreachable; routing Mojang service requests through the Fallen proxy"
        );
    }
    Ok(applied)
}

/// Whether Mojang service requests currently go out through the Fallen proxy.
pub fn mojang_auth_use_mirror() -> bool {
    should_use_mojang_mirror()
}

#[derive(Clone, Debug, Serialize)]
pub struct MojangServiceStatus {
    pub service: &'static str,
    pub url: &'static str,
    pub reachable: bool,
}

const MOJANG_SERVICES: [(&str, &str); 4] = [
    ("account", "https://api.mojang.com/"),
    (
        "session",
        "https://sessionserver.mojang.com/session/minecraft/hasJoined",
    ),
    ("services", "https://api.minecraftservices.com/publickeys"),
    (
        "profiles",
        "https://api.mojang.com/users/profiles/minecraft/Notch",
    ),
];

fn service_response_reachable(service: &str, status: StatusCode) -> bool {
    if service == "session" {
        status == StatusCode::NO_CONTENT
    } else {
        status.is_success()
            || service == "account" && status == StatusCode::NOT_FOUND
    }
}

pub async fn check_mojang_services() -> Vec<MojangServiceStatus> {
    check_mojang_services_for(false).await
}

#[tracing::instrument]
pub async fn check_mojang_services_for(
    use_mirror: bool,
) -> Vec<MojangServiceStatus> {
    probe_mojang_services(use_mirror, false).await
}

async fn probe_mojang_services(
    use_mirror: bool,
    core_only: bool,
) -> Vec<MojangServiceStatus> {
    let client = authentication_client();
    futures::future::join_all(
        MOJANG_SERVICES
            .into_iter()
            .filter(|(service, _)| {
                !core_only || matches!(*service, "session" | "services")
            })
            .map(|(service, url)| {
                let client = client.clone();
                async move {
                    let reachable = client
                        .get(mojang_service_url(url, use_mirror).as_ref())
                        .timeout(REACHABILITY_TIMEOUT)
                        .send()
                        .await
                        .is_ok_and(|response| {
                            service_response_reachable(
                                service,
                                response.status(),
                            )
                        });
                    MojangServiceStatus {
                        service,
                        url,
                        reachable,
                    }
                }
            }),
    )
    .await
}

#[tracing::instrument]
pub async fn begin_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;

    crate::state::login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_browser_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::browser_login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_device_login() -> crate::Result<MinecraftDeviceLoginFlow> {
    crate::state::device_login_begin().await
}

#[tracing::instrument]
pub async fn poll_device_login(
    device_code: &str,
) -> crate::Result<MinecraftDeviceLoginPoll> {
    let state = State::get().await?;
    crate::state::device_login_poll(device_code, &state.pool).await
}

#[tracing::instrument]
pub async fn finish_login(
    code: &str,
    state: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<Credentials> {
    let app_state = State::get().await?;

    crate::state::login_finish(code, state, flow, &app_state.pool).await
}

#[tracing::instrument]
pub async fn add_offline_user(
    username: &str,
    uuid: Option<Uuid>,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    crate::anti_piracy::ensure_offline_allowed().await?;
    let credentials = match uuid {
        Some(uuid) => Credentials::offline_with_uuid(username, uuid)?,
        None => Credentials::offline(username)?,
    };

    if uuid.is_some() {
        let users = Credentials::get_all_without_refresh(&state.pool).await?;
        if users
            .iter()
            .any(|user| user.account_id() == credentials.account_id())
        {
            return Err(crate::ErrorKind::InputError(
                "An account with this UUID already exists".to_string(),
            )
            .as_error());
        }
    }

    let _eligibility = crate::anti_piracy::offline_action_guard().await?;
    credentials.upsert(&state.pool).await?;
    Ok(credentials)
}

#[tracing::instrument(skip(password))]
pub async fn begin_yggdrasil_login(
    api_root: &str,
    login: &str,
    password: &str,
) -> crate::Result<YggdrasilLoginResult> {
    let state = State::get().await?;
    crate::state::begin_yggdrasil_login(api_root, login, password, &state.pool)
        .await
}

#[tracing::instrument]
pub async fn finish_yggdrasil_login(
    flow_id: uuid::Uuid,
    profile_id: uuid::Uuid,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    crate::state::finish_yggdrasil_login(flow_id, profile_id, &state.pool).await
}

pub fn normalize_yggdrasil_api_root(api_root: &str) -> crate::Result<String> {
    crate::state::normalize_api_root(api_root)
}

#[tracing::instrument]
pub async fn get_default_user(
    offline_mode: bool,
) -> crate::Result<Option<String>> {
    let state = State::get().await?;
    let user = if offline_mode {
        Credentials::get_offline_credential(&state.pool).await?
    } else {
        Credentials::get_active_without_refresh(&state.pool).await?
    };
    Ok(user.map(|user| user.account_id()))
}

#[tracing::instrument]
pub async fn set_default_user(account_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let users = Credentials::get_all_without_refresh(&state.pool).await?;
    let mut user = users
        .into_iter()
        .find(|user| user.account_id() == account_id)
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to get nonexistent user with ID {account_id}"
            ))
            .as_error()
        })?;

    let _eligibility = if user.is_offline() {
        Some(crate::anti_piracy::offline_action_guard().await?)
    } else {
        None
    };

    user.active = true;
    user.upsert(&state.pool).await?;

    Ok(())
}

/// Remove a user account from the database
#[tracing::instrument]
pub async fn remove_user(account_id: &str) -> crate::Result<()> {
    let state = State::get().await?;

    let mut users = Credentials::get_all_without_refresh(&state.pool).await?;

    if let Some(index) = users
        .iter()
        .position(|user| user.account_id() == account_id)
    {
        let user = users.remove(index);
        Credentials::remove(account_id, &state.pool).await?;
        profile_cache::remove(
            &state.directories.caches_dir().join("minecraft-profiles"),
            &user,
        )
        .await;

        if user.active
            && let Some(mut user) = users.into_iter().next()
        {
            user.active = true;
            user.upsert(&state.pool).await?;
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct MinecraftUser {
    pub account_id: String,
    pub profile: MinecraftProfile,
    pub account_type: MinecraftAccountType,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
    pub yggdrasil: Option<YggdrasilAccount>,
}

impl MinecraftUser {
    async fn from_credentials(
        credentials: Credentials,
        cached_only: bool,
        cache_dir: &std::path::Path,
    ) -> Self {
        let cached = profile_cache::read(cache_dir, &credentials).await;
        let profile = if cached_only {
            cached.unwrap_or_else(|| credentials.offline_profile.clone())
        } else {
            let profile = (*credentials.maybe_online_profile().await).clone();
            if !profile.skins.is_empty() {
                profile_cache::write(cache_dir, &credentials, &profile).await;
                profile
            } else {
                cached.unwrap_or(profile)
            }
        };
        Self {
            account_id: credentials.account_id(),
            profile,
            account_type: credentials.account_type,
            access_token: credentials.access_token,
            refresh_token: credentials.refresh_token,
            expires: credentials.expires,
            active: credentials.active,
            yggdrasil: credentials.yggdrasil,
        }
    }
}

/// Get a copy of the list of all user credentials with profile data ready for
/// serialization.
#[tracing::instrument]
pub async fn users(offline_mode: bool) -> crate::Result<Vec<MinecraftUser>> {
    users_with_cache(offline_mode, false).await
}

/// Reads saved account and skin metadata without refreshing tokens or making
/// profile requests when `cached_only` is set.
pub async fn users_with_cache(
    offline_mode: bool,
    cached_only: bool,
) -> crate::Result<Vec<MinecraftUser>> {
    let state = State::get().await?;
    let users = if offline_mode || cached_only {
        Credentials::get_all_without_refresh(&state.pool).await?
    } else {
        Credentials::get_all(&state.pool).await?
    };
    let credentials = users
        .into_iter()
        .filter(|credentials| !offline_mode || credentials.is_offline());
    let mut hydrated_users = Vec::new();
    let cache_dir = state.directories.caches_dir().join("minecraft-profiles");
    for credentials in credentials {
        hydrated_users.push(
            MinecraftUser::from_credentials(
                credentials,
                cached_only,
                &cache_dir,
            )
            .await,
        );
    }
    Ok(hydrated_users)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachability_rejects_gateway_errors_and_captive_portal_pages() {
        for status in [
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::BAD_GATEWAY,
        ] {
            for service in ["account", "session", "services", "profiles"] {
                assert!(!service_response_reachable(service, status));
            }
        }
        assert!(!service_response_reachable("session", StatusCode::OK));
        assert!(service_response_reachable(
            "session",
            StatusCode::NO_CONTENT
        ));
        assert!(service_response_reachable("services", StatusCode::OK));
        assert!(service_response_reachable("account", StatusCode::NOT_FOUND));
    }

    #[test]
    fn mojang_monitor_probes_only_current_required_services() {
        let services = MOJANG_SERVICES.map(|(service, _)| service);
        assert_eq!(services, ["account", "session", "services", "profiles"]);
    }

    /// The probes run against hosts that answer in one to six seconds on a
    /// proxied connection, so a budget inside that range declares a healthy
    /// service unreachable and switches the launcher to the mirror for good.
    #[test]
    fn reachability_budget_leaves_room_for_a_slow_but_working_connection() {
        assert!(REACHABILITY_TIMEOUT > Duration::from_secs(6));
    }
}
