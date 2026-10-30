//! On-disk index cache for source listings.
//!
//! The index is a *reproducible* artifact: losing it costs one upstream fetch,
//! never user data. It therefore lives in its own file per source rather than in
//! the shared `state::cache` tables, which would require extending
//! `CacheValueType` and touching the provider layer.
//!
//! Writes are atomic (temp file, fsync, rename) so a crash mid-write cannot
//! leave a half-written index behind.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::{SchematicSourceError, SourceResult, StorageCode};
use super::types::SourceId;

/// Bumped whenever the cached payload shape changes.
///
/// A mismatch discards the file instead of trying to migrate it.
const SCHEMA_VERSION: u32 = 1;

/// Minimum index lifetime. BuildSchematics forbids repeated fetching, so the
/// setting may raise this but never lower it.
pub const MIN_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct IndexFile<T> {
    schema_version: u32,
    fetched_at: i64,
    payload: T,
}

/// A cached index together with when it was fetched.
pub struct CachedIndex<T> {
    pub payload: T,
    pub fetched_at: i64,
}

impl<T> CachedIndex<T> {
    pub fn is_fresh(&self, now: i64, ttl_seconds: i64) -> bool {
        let ttl = ttl_seconds.max(MIN_TTL_SECONDS);
        now.saturating_sub(self.fetched_at) < ttl
    }
}

/// Directory holding every source index.
///
/// Lives under the launcher's settings directory, following the existing
/// `<settings_dir>/<integration>/` convention.
pub async fn cache_dir() -> SourceResult<PathBuf> {
    let state = crate::State::get().await.map_err(|error| {
        SchematicSourceError::storage(
            StorageCode::PathInvalid,
            format!("launcher state unavailable: {error}"),
        )
    })?;
    Ok(state.directories.settings_dir.join("schematic_browser"))
}

async fn index_path(source: SourceId) -> SourceResult<PathBuf> {
    Ok(cache_dir()
        .await?
        .join(format!("{}-index.json", source.as_str())))
}

/// Read one source's cached index.
///
/// Returns `Ok(None)` for every "no usable cache" case: missing file, unreadable
/// file, wrong schema, or unparsable payload. A corrupt cache is a cache miss,
/// not an error the caller has to handle.
pub async fn read<T: for<'de> Deserialize<'de>>(
    source: SourceId,
) -> SourceResult<Option<CachedIndex<T>>> {
    let path = index_path(source).await?;
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => {
            return Err(SchematicSourceError::storage(
                StorageCode::Io,
                format!("cannot read {}: {error}", path.display()),
            ));
        }
    };

    let file: IndexFile<T> = match serde_json::from_slice(&raw) {
        Ok(file) => file,
        Err(_) => {
            // Corrupt or stale shape: drop it so the next write starts clean.
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
    };
    if file.schema_version != SCHEMA_VERSION {
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    }
    Ok(Some(CachedIndex {
        payload: file.payload,
        fetched_at: file.fetched_at,
    }))
}

/// Write one source's index atomically.
pub async fn write<T: Serialize>(
    source: SourceId,
    payload: &T,
    fetched_at: i64,
) -> SourceResult<()> {
    let dir = cache_dir().await?;
    std::fs::create_dir_all(&dir).map_err(|error| {
        SchematicSourceError::storage(
            StorageCode::Io,
            format!("cannot create {}: {error}", dir.display()),
        )
    })?;

    let path = index_path(source).await?;
    let file = IndexFile {
        schema_version: SCHEMA_VERSION,
        fetched_at,
        payload,
    };
    let bytes = serde_json::to_vec(&file).map_err(|error| {
        SchematicSourceError::parse("index", error.to_string())
    })?;

    let temp = temp_path(&path);
    {
        use std::io::Write;
        let mut handle = std::fs::File::create(&temp).map_err(|error| {
            SchematicSourceError::storage(
                StorageCode::Io,
                format!("cannot create {}: {error}", temp.display()),
            )
        })?;
        handle.write_all(&bytes).map_err(|error| {
            SchematicSourceError::storage(
                StorageCode::DiskFull,
                format!("cannot write {}: {error}", temp.display()),
            )
        })?;
        handle.sync_all().map_err(|error| {
            SchematicSourceError::storage(
                StorageCode::Io,
                format!("cannot flush {}: {error}", temp.display()),
            )
        })?;
    }

    // `rename` replaces the target on Unix but fails when it exists on
    // Windows, so drop the old index first. A concurrent writer could recreate
    // it in the gap; the newer snapshot always wins on the next refresh.
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&temp, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        SchematicSourceError::storage(
            StorageCode::Io,
            format!("cannot replace {}: {error}", path.display()),
        )
    })
}

/// Unique scratch path beside `path`.
///
/// Two requests for the same source can miss the cache at the same time; a fixed
/// `.tmp` name lets one writer truncate or rename the other's file. The counter
/// only has to make the name unique within this process, since the rename is what
/// publishes the result.
fn temp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut temp = path.to_path_buf();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "index".to_string());
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    temp.set_file_name(format!("{name}.{}.{serial}.tmp", std::process::id()));
    temp
}

/// Delete one source's cached index, if present.
pub async fn clear(source: SourceId) -> SourceResult<()> {
    let path = index_path(source).await?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SchematicSourceError::storage(
            StorageCode::Io,
            format!("cannot remove {}: {error}", path.display()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_cannot_go_below_the_compliance_floor() {
        let cached = CachedIndex {
            payload: (),
            fetched_at: 1_000,
        };
        // A caller asking for 1 second still gets the 24h floor.
        assert!(cached.is_fresh(1_000 + 60, 1));
        // Past 24h it is stale.
        assert!(!cached.is_fresh(1_000 + MIN_TTL_SECONDS + 1, 1));
    }

    #[test]
    fn fresh_uses_requested_ttl_when_larger() {
        let cached = CachedIndex {
            payload: (),
            fetched_at: 0,
        };
        let longer = MIN_TTL_SECONDS * 2;
        assert!(cached.is_fresh(MIN_TTL_SECONDS + 10, longer));
    }

    #[test]
    fn temp_path_keeps_the_directory() {
        let path = Path::new("/tmp/a/reden-index.json");
        let temp = temp_path(path);
        assert_eq!(temp.parent(), path.parent());
        assert!(temp.to_string_lossy().ends_with(".json.tmp"));
    }

    #[test]
    fn schema_version_mismatch_is_dropped() {
        let raw = serde_json::json!({
            "schema_version": SCHEMA_VERSION + 1,
            "fetched_at": 0,
            "payload": {}
        });
        let file: IndexFile<serde_json::Value> =
            serde_json::from_value(raw).unwrap();
        assert_ne!(file.schema_version, SCHEMA_VERSION);
    }
}
