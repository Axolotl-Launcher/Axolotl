use base64::Engine;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::Serialize;
use sha2::Sha256;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};

use crate::State;
use crate::state::store::Collection as _;
use crate::state::store::official_login_proof::{
    Installation, OfficialLoginProof, OfficialLoginProofRecord,
};

const MAINLAND_CHINA_TIMEZONES: &[&str] = &[
    "Asia/Shanghai",
    "Asia/Urumqi",
    "Asia/Chongqing",
    "Asia/Chungking",
    "Asia/Harbin",
    "Asia/Kashgar",
    "PRC",
    // Common non-IANA spelling, accepted for compatibility.
    "Asia/Beijing",
];
const PROOF_KEY: &str = "official-minecraft-login-proof-v1";
const PROOF_MESSAGE: &[u8] = b"axolotl:official-minecraft-login:1:1";
const RESTRICTED_ERROR: &str = "OFFLINE_ACCOUNT_RESTRICTED";
static REGION: AtomicU8 = AtomicU8::new(0);
static SESSION_OFFICIAL_LOGIN: AtomicBool = AtomicBool::new(false);
static STATUS_REVISION: AtomicU64 = AtomicU64::new(0);
static SESSION_GATE: RwLock<()> = RwLock::const_new(());
static PROOF_WRITE: Mutex<()> = Mutex::const_new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Checking,
    Cn,
    NonCn,
    Unavailable,
}

impl Region {
    fn current() -> Self {
        match REGION.load(Ordering::Acquire) {
            1 => Self::Cn,
            2 => Self::NonCn,
            3 => Self::Unavailable,
            _ => Self::Checking,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Status {
    pub region: Region,
    pub restricted: bool,
    pub revision: u64,
}

fn region_for_timezone(timezone: &str) -> Region {
    let timezone = timezone.trim();
    if timezone.is_empty() {
        Region::Unavailable
    } else if MAINLAND_CHINA_TIMEZONES.contains(&timezone) {
        Region::Cn
    } else {
        Region::NonCn
    }
}

async fn publish_region(region: Region) {
    let _guard = SESSION_GATE.write().await;
    REGION.store(
        match region {
            Region::Checking => 0,
            Region::Cn => 1,
            Region::NonCn => 2,
            Region::Unavailable => 3,
        },
        Ordering::Release,
    );
    STATUS_REVISION.fetch_add(1, Ordering::AcqRel);
}

pub async fn check_region() -> Status {
    let region = match tokio::task::spawn_blocking(iana_time_zone::get_timezone)
        .await
    {
        Ok(Ok(timezone)) => region_for_timezone(&timezone),
        Ok(Err(error)) => {
            tracing::warn!(%error, "Offline account eligibility check failed; offline accounts remain available");
            Region::Unavailable
        }
        Err(error) => {
            tracing::warn!(%error, "Offline account eligibility task failed; offline accounts remain available");
            Region::Unavailable
        }
    };
    publish_region(region).await;
    status().await
}

fn proof_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(crate::brand::BUNDLE_IDENTIFIER, PROOF_KEY)
}

fn proof_mac(key: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(PROOF_MESSAGE);
    base64::engine::general_purpose::STANDARD
        .encode(mac.finalize().into_bytes())
}

fn proof_valid(key: &[u8], stored_mac: &str) -> bool {
    let Ok(stored_mac) =
        base64::engine::general_purpose::STANDARD.decode(stored_mac)
    else {
        return false;
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(PROOF_MESSAGE);
    mac.verify_slice(&stored_mac).is_ok()
}

fn restricted(region: Region, has_proof: bool) -> bool {
    region == Region::NonCn && !has_proof
}

async fn has_proof() -> bool {
    let state = match State::get().await {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!(%error, "Cannot read official login proof");
            return false;
        }
    };
    let record = OfficialLoginProof::get(&state.data, &Installation).await;
    let Some(stored_mac) = (match record {
        Ok(record) => record.map(|record| record.mac),
        Err(error) => {
            tracing::warn!(%error, "Cannot read official login proof");
            return false;
        }
    }) else {
        return false;
    };
    let key =
        tokio::task::spawn_blocking(|| proof_entry()?.get_password()).await;
    let Ok(Ok(key)) = key else {
        tracing::warn!("Official login proof key is unavailable");
        return false;
    };
    let Ok(key) = base64::engine::general_purpose::STANDARD.decode(key) else {
        tracing::warn!("Official login proof key is invalid");
        return false;
    };
    if key.len() != 32 {
        tracing::warn!("Official login proof key has an invalid length");
        return false;
    }
    let valid = proof_valid(&key, &stored_mac);
    if !valid {
        tracing::warn!("Official login proof failed integrity verification");
    }
    valid
}

async fn status_unlocked() -> Status {
    let region = Region::current();
    let has_proof = if region == Region::NonCn
        && !SESSION_OFFICIAL_LOGIN.load(Ordering::Acquire)
    {
        has_proof().await
    } else {
        true
    };
    Status {
        region,
        restricted: restricted(region, has_proof),
        revision: STATUS_REVISION.load(Ordering::Acquire),
    }
}

pub async fn status() -> Status {
    let _guard = SESSION_GATE.read().await;
    status_unlocked().await
}

pub async fn ensure_offline_allowed() -> crate::Result<()> {
    if status().await.restricted {
        return Err(crate::ErrorKind::InputError(RESTRICTED_ERROR.to_string())
            .as_error());
    }
    Ok(())
}

/// Retain this guard until the account write or process spawn has committed.
pub async fn offline_action_guard()
-> crate::Result<RwLockReadGuard<'static, ()>> {
    let guard = SESSION_GATE.read().await;
    if status_unlocked().await.restricted {
        return Err(crate::ErrorKind::InputError(RESTRICTED_ERROR.to_string())
            .as_error());
    }
    Ok(guard)
}

pub async fn mark_official_login() -> crate::Result<()> {
    let _guard = PROOF_WRITE.lock().await;
    let state = State::get().await?;
    let key = tokio::task::spawn_blocking(|| {
        let entry = proof_entry()?;
        match entry.get_password() {
            Ok(encoded) => {
                if let Ok(key) =
                    base64::engine::general_purpose::STANDARD.decode(encoded)
                    && key.len() == 32
                {
                    return Ok(key);
                }
            }
            Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error),
        }
        let mut key = vec![0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        entry.set_password(
            &base64::engine::general_purpose::STANDARD.encode(&key),
        )?;
        Ok::<Vec<u8>, keyring::Error>(key)
    })
    .await
    .map_err(|error| {
        crate::ErrorKind::OtherError(error.to_string()).as_error()
    })?
    .map_err(|error| {
        crate::ErrorKind::OtherError(error.to_string()).as_error()
    })?;
    let mac = proof_mac(&key);
    OfficialLoginProof::set(
        &state.data,
        &Installation,
        Some(&OfficialLoginProofRecord { mac }),
    )
    .await?;
    let _session_guard = SESSION_GATE.write().await;
    SESSION_OFFICIAL_LOGIN.store(true, Ordering::Release);
    STATUS_REVISION.fetch_add(1, Ordering::AcqRel);
    Ok(())
}

pub async fn clear_official_login() -> crate::Result<()> {
    let _guard = PROOF_WRITE.lock().await;
    let _session_guard = SESSION_GATE.write().await;
    let state = State::get().await?;
    OfficialLoginProof::set(&state.data, &Installation, None).await?;
    SESSION_OFFICIAL_LOGIN.store(false, Ordering::Release);
    STATUS_REVISION.fetch_add(1, Ordering::AcqRel);
    drop(_session_guard);
    let result =
        tokio::task::spawn_blocking(|| proof_entry()?.delete_credential())
            .await
            .map_err(|error| {
                crate::ErrorKind::OtherError(error.to_string()).as_error()
            })?;
    match result {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => {
            tracing::warn!(%error, "Official login proof was cleared, but its key could not be removed");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_mainland_china_timezones_and_aliases() {
        for timezone in [
            "Asia/Shanghai",
            "Asia/Urumqi",
            "Asia/Beijing",
            "Asia/Chongqing",
            "Asia/Chungking",
            "Asia/Harbin",
            "Asia/Kashgar",
            "PRC",
        ] {
            assert_eq!(region_for_timezone(timezone), Region::Cn, "{timezone}");
        }
    }

    #[test]
    fn does_not_exempt_other_timezones_with_the_same_offset() {
        for timezone in [
            "Asia/Hong_Kong",
            "Asia/Macau",
            "Asia/Macao",
            "Asia/Taipei",
            "Asia/Singapore",
            "Asia/Kuala_Lumpur",
            "Etc/GMT-8",
            "UTC",
            "America/New_York",
        ] {
            assert_eq!(
                region_for_timezone(timezone),
                Region::NonCn,
                "{timezone}"
            );
        }
        assert_eq!(region_for_timezone(""), Region::Unavailable);
        assert_eq!(region_for_timezone("  "), Region::Unavailable);
    }

    #[test]
    fn proof_mac_is_stable_for_the_same_key() {
        assert_eq!(proof_mac(&[7; 32]), proof_mac(&[7; 32]));
        assert_ne!(proof_mac(&[7; 32]), proof_mac(&[8; 32]));
        assert!(proof_valid(&[7; 32], &proof_mac(&[7; 32])));
        assert!(!proof_valid(&[8; 32], &proof_mac(&[7; 32])));
        assert!(!proof_valid(&[7; 32], "tampered"));
    }

    #[test]
    fn only_explicit_non_cn_without_proof_is_restricted() {
        for region in [Region::Checking, Region::Cn, Region::Unavailable] {
            assert!(!restricted(region, false));
        }
        assert!(restricted(Region::NonCn, false));
        assert!(!restricted(Region::NonCn, true));
    }

    #[tokio::test]
    async fn region_verdict_waits_for_an_offline_commit() {
        publish_region(Region::Checking).await;
        let eligibility = offline_action_guard().await.unwrap();
        let verdict = tokio::spawn(publish_region(Region::NonCn));
        tokio::task::yield_now().await;
        assert!(!verdict.is_finished());
        drop(eligibility);
        verdict.await.unwrap();
        assert_eq!(Region::current(), Region::NonCn);
        assert!(restricted(Region::current(), false));
        publish_region(Region::Checking).await;
    }
}
