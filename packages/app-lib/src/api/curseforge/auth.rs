//! CurseForge API key capability detection.

use super::constants::MINECRAFT_GAME_ID;
use super::dto::{
    CurseForgeCapability, CurseForgeCapabilityStatus, CurseForgeProject,
    CurseForgeResponse,
};
use super::transport::{MirrorPolicy, api_key_with_source, request_json};
use crate::State;
use reqwest::Method;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static UNAUTHORIZED: AtomicBool = AtomicBool::new(false);
/// Bumped whenever the effective key is replaced. A response that arrives after
/// that can only describe the previous key, so it must not change the flag.
static API_KEY_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn capability() -> CurseForgeCapability {
    let (key, key_source) = api_key_with_source();
    let configured = key.is_some();
    let status = if !configured {
        CurseForgeCapabilityStatus::MissingKey
    } else if UNAUTHORIZED.load(Ordering::Relaxed) {
        CurseForgeCapabilityStatus::Unauthorized
    } else {
        CurseForgeCapabilityStatus::Ready
    };

    CurseForgeCapability {
        status,
        configured,
        key_source,
    }
}

/// Stores the API key the user entered in the launcher. `None` removes the
/// override so the key the launcher was started or built with applies again,
/// when there is one.
pub async fn set_api_key_override(
    api_key: Option<String>,
) -> crate::Result<CurseForgeCapability> {
    let state = State::get().await?;
    state.update_curseforge_api_key(api_key).await?;
    // Invalidate responses still in flight for the previous key before clearing
    // the flag, so they cannot report an outdated status for the new one.
    API_KEY_GENERATION.fetch_add(1, Ordering::Relaxed);
    UNAUTHORIZED.store(false, Ordering::Relaxed);
    Ok(capability())
}

pub(crate) fn api_key_generation() -> u64 {
    API_KEY_GENERATION.load(Ordering::Relaxed)
}

/// Applies the authorization outcome of a response only while it still belongs
/// to the current key, so a late response cannot speak for a replaced one.
pub(crate) fn record_key_response(generation: u64, unauthorized: bool) {
    if generation != api_key_generation() {
        return;
    }
    UNAUTHORIZED.store(unauthorized, Ordering::Relaxed);
}

/// The API key the user entered in the launcher, if any, for redisplaying it.
pub async fn api_key_override() -> crate::Result<Option<String>> {
    let state = State::get().await?;
    Ok(state.curseforge_api_key_override())
}

pub async fn validate_credentials() -> crate::Result<CurseForgeCapability> {
    let generation = api_key_generation();
    let _: CurseForgeResponse<Vec<CurseForgeProject>> = request_json(
        Method::GET,
        "/v1/mods/search",
        vec![
            ("gameId".to_string(), MINECRAFT_GAME_ID.to_string()),
            ("classId".to_string(), "6".to_string()),
            ("pageSize".to_string(), "1".to_string()),
        ],
        None,
        MirrorPolicy::OfficialOnly,
    )
    .await?;
    record_key_response(generation, false);
    Ok(capability())
}
