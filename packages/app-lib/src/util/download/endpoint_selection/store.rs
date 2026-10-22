//! Discardable, bounded endpoint hints. Persisted hints never become leases.

use super::candidates::is_public_ip;
use super::model::{CandidateSource, EndpointCandidate};
use crate::util::download::constants::{
    ENDPOINT_HINT_LIFETIME, ENDPOINT_STORE_FILE_NAME,
    STORE_MISSING_HINTS_DIRECTORY,
};
use crate::util::download::modrinth_cdn::HOST;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;

use crate::util::download::constants::{
    ENDPOINT_STORE_MAX_AGE as MAX_AGE, ENDPOINT_STORE_MAX_BYTES as MAX_BYTES,
    ENDPOINT_STORE_MAX_ENTRIES as MAX_ENTRIES,
    ENDPOINT_STORE_VERSION as VERSION,
};
static WRITER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize, Deserialize)]
struct Hints {
    version: u8,
    saved_at: u64,
    entries: Vec<Hint>,
}

#[derive(Serialize, Deserialize)]
struct Hint {
    host: String,
    ip: IpAddr,
    validated_at: u64,
    rule_version: Option<u32>,
    #[serde(default)]
    source: Option<String>,
}

fn now_seconds(now: SystemTime) -> Option<u64> {
    now.duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

fn cache_path() -> Option<PathBuf> {
    crate::State::get_if_initialized().map(|state| {
        state
            .directories
            .caches_dir()
            .join(ENDPOINT_STORE_FILE_NAME)
    })
}

fn decode(
    bytes: &[u8],
    wall: SystemTime,
    now: Instant,
) -> Vec<EndpointCandidate> {
    if bytes.len() > MAX_BYTES as usize {
        return Vec::new();
    }
    let Some(seconds) = now_seconds(wall) else {
        return Vec::new();
    };
    let Ok(hints) = serde_json::from_slice::<Hints>(bytes) else {
        return Vec::new();
    };
    if hints.version != VERSION
        || hints.entries.len() > MAX_ENTRIES
        || hints.saved_at > seconds
        || seconds.saturating_sub(hints.saved_at) > MAX_AGE
    {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    hints
        .entries
        .into_iter()
        .filter(|hint| {
            hint.host == HOST
                && is_public_ip(hint.ip)
                && hint.validated_at <= hints.saved_at
                && hint.validated_at > 0
                && seconds.saturating_sub(hint.validated_at) <= MAX_AGE
                && seen.insert(hint.ip)
        })
        .map(|hint| EndpointCandidate {
            ip: hint.ip,
            source: CandidateSource::History,
            // This is a candidate lifetime, not the original DNS TTL or
            // permission to pin. Every restored IP must pass fresh probes.
            expires_at: now + ENDPOINT_HINT_LIFETIME,
        })
        .collect()
}

async fn load_path(path: &Path, wall: SystemTime) -> Vec<EndpointCandidate> {
    let Ok(file) = tokio::fs::File::open(path).await else {
        return Vec::new();
    };
    let Ok(metadata) = file.metadata().await else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .is_err()
    {
        return Vec::new();
    }
    decode(&bytes, wall, Instant::now())
}

pub(super) async fn load() -> Vec<EndpointCandidate> {
    let Some(path) = cache_path() else {
        return Vec::new();
    };
    load_path(&path, SystemTime::now()).await
}

async fn save_path(
    path: PathBuf,
    candidates: Vec<EndpointCandidate>,
    wall: SystemTime,
) -> std::io::Result<()> {
    let _writer = WRITER.lock().await;
    let Some(seconds) = now_seconds(wall) else {
        return Ok(());
    };
    let mut seen = HashSet::new();
    let entries = candidates
        .into_iter()
        .filter(|candidate| {
            is_public_ip(candidate.ip)
                && candidate.expires_at > Instant::now()
                && seen.insert(candidate.ip)
        })
        .take(MAX_ENTRIES)
        .map(|candidate| Hint {
            host: HOST.into(),
            ip: candidate.ip,
            validated_at: seconds,
            source: Some(candidate.source.as_str().to_string()),
            rule_version: match candidate.source {
                CandidateSource::FastlyRule(version) => Some(version),
                _ => None,
            },
        })
        .collect();
    let bytes = serde_json::to_vec(&Hints {
        version: VERSION,
        saved_at: seconds,
        entries,
    })?;
    if bytes.len() > MAX_BYTES as usize {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || {
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::other(STORE_MISSING_HINTS_DIRECTORY)
        })?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.flush()?;
        temporary.persist(&path).map_err(|error| error.error)?;
        Ok(())
    })
    .await
    .map_err(std::io::Error::other)?
}

pub(super) async fn save(candidates: Vec<EndpointCandidate>) {
    let Some(path) = cache_path() else { return };
    if let Err(error) = save_path(path, candidates, SystemTime::now()).await {
        tracing::debug!(%error, "Unable to persist Modrinth endpoint hints");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(seconds: u64) -> Hints {
        Hints {
            version: VERSION,
            saved_at: seconds,
            entries: vec![Hint {
                host: HOST.into(),
                ip: "151.101.1.242".parse().unwrap(),
                validated_at: seconds,
                rule_version: Some(1),
                source: Some("fastly_rule".into()),
            }],
        }
    }

    #[test]
    fn malformed_large_future_old_and_nonpublic_hints_are_discarded() {
        let seconds = 1_792_540_800;
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(seconds);
        assert!(decode(b"broken", wall, now).is_empty());
        assert!(decode(&vec![0; MAX_BYTES as usize + 1], wall, now).is_empty());
        for case in 0..7 {
            let mut hints = document(seconds);
            match case {
                0 => hints.version += 1,
                1 => hints.saved_at += 1,
                2 => hints.saved_at -= MAX_AGE + 1,
                3 => hints.entries[0].ip = "198.18.1.1".parse().unwrap(),
                4 => hints.entries[0].host = "api.modrinth.com".into(),
                5 => hints.entries[0].validated_at += 1,
                6 => hints.entries[0].validated_at -= MAX_AGE + 1,
                _ => unreachable!(),
            }
            assert!(
                decode(&serde_json::to_vec(&hints).unwrap(), wall, now)
                    .is_empty(),
                "case {case}"
            );
        }
        let restored =
            decode(&serde_json::to_vec(&document(seconds)).unwrap(), wall, now);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].source, CandidateSource::History);
    }

    #[tokio::test]
    async fn atomic_replacement_is_bounded_and_restores_only_unvalidated_hints()
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hints.json");
        let candidates: Vec<_> = (1..=64)
            .map(|last| EndpointCandidate {
                ip: std::net::Ipv4Addr::new(151, 101, 1, last).into(),
                source: CandidateSource::Verified,
                expires_at: Instant::now() + Duration::from_secs(60),
            })
            .collect();
        save_path(path.clone(), candidates.clone(), SystemTime::now())
            .await
            .unwrap();
        save_path(path.clone(), candidates, SystemTime::now())
            .await
            .unwrap();
        assert_eq!(load_path(&path, SystemTime::now()).await.len(), 32);
        assert!(tokio::fs::metadata(&path).await.unwrap().len() < MAX_BYTES);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        tokio::fs::write(&path, vec![0; MAX_BYTES as usize + 1])
            .await
            .unwrap();
        assert!(load_path(&path, SystemTime::now()).await.is_empty());
    }
}
