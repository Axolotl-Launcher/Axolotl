//! An independently versioned SQLx pool. No joins or transactions use app.db.

mod legacy;
mod migrations;

use chrono::Utc;
use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use super::{ipc::ResourceCacheInfo, providers::ProviderValue, *};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::atom;
use crate::state::store::resource_artifact_evidence::{
    ResourceArtifactEvidence, ResourceArtifactEvidenceRecord,
};
use crate::state::store::resource_cache_entries::{
    ResourceCacheEntries, ResourceCacheEntryKey, ResourceCacheEntryRecord,
};
use crate::state::store::resource_cache_state::{
    ResourceCacheState, ResourceCacheStateRecord,
};
use crate::state::store::resource_legacy_import::{
    ResourceLegacyImport, ResourceLegacyImportRecord, Singleton as Checkpoint,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CacheKey {
    pub namespace: String,
    pub source: SourceId,
    pub query: String,
}

impl CacheKey {
    pub fn new(
        namespace: String,
        source: SourceId,
        query: &impl Serialize,
    ) -> ResourceResult<Self> {
        let payload = serde_json::to_vec(query)?;
        Ok(Self {
            namespace,
            source,
            query: format!("{:x}", Sha256::digest(payload)),
        })
    }
}

pub struct ResourceCache {
    path: PathBuf,
    pool: Option<SqlitePool>,
    file_lock: Mutex<Option<tokio::fs::File>>,
    gate: RwLock<()>,
    last_maintenance: std::sync::atomic::AtomicI64,
}

impl ResourceCache {
    /// Cache creation failure is a performance limitation, never a launcher
    /// initialization failure. Unknown migration versions are preserved.
    pub async fn open(path: PathBuf) -> Self {
        match Self::connect(&path).await {
            Ok((pool, lock)) => Self {
                path,
                pool: Some(pool),
                file_lock: Mutex::new(Some(lock)),
                gate: RwLock::new(()),
                last_maintenance: std::sync::atomic::AtomicI64::new(0),
            },
            Err(error) => {
                tracing::warn!(error = %error, "Resource cache disabled; live source queries remain available");
                Self {
                    path,
                    pool: None,
                    file_lock: Mutex::new(None),
                    gate: RwLock::new(()),
                    last_maintenance: std::sync::atomic::AtomicI64::new(0),
                }
            }
        }
    }

    async fn connect(
        path: &Path,
    ) -> ResourceResult<(SqlitePool, tokio::fs::File)> {
        let parent = path.parent().ok_or_else(|| {
            ResourceError::Cache("cache directory missing".into())
        })?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(cache_error)?;
        let lock = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join("resource-cache.lock"))
            .await
            .map_err(cache_error)?;
        let exclusive = lock.try_lock_exclusive().map_err(cache_error)?;
        if !exclusive {
            acquire_shared(&lock).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);
        let attempt = open_pool(options.clone()).await;
        let mut pool = match attempt {
            Ok(pool) => pool,
            Err(error) if exclusive && is_corrupt(&error) => {
                for path in [
                    path.to_path_buf(),
                    sidecar(path, "-wal"),
                    sidecar(path, "-shm"),
                ] {
                    match tokio::fs::remove_file(path).await {
                        Ok(()) => {}
                        Err(error)
                            if error.kind() == std::io::ErrorKind::NotFound => {
                        }
                        Err(error) => return Err(cache_error(error)),
                    }
                }
                open_pool(options.clone()).await.map_err(cache_error)?
            }
            Err(error) => return Err(cache_error(error)),
        };
        if let Err(error) = migrations::migrator().run(&pool).await {
            pool.close().await;
            let corrupt = match &error {
                sqlx::migrate::MigrateError::Execute(error)
                | sqlx::migrate::MigrateError::ExecuteMigration(error, _) => {
                    is_corrupt(error)
                }
                _ => false,
            };
            if !exclusive || !corrupt {
                return Err(cache_error(error));
            }
            for file in [
                path.to_path_buf(),
                sidecar(path, "-wal"),
                sidecar(path, "-shm"),
            ] {
                match tokio::fs::remove_file(file).await {
                    Ok(()) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(cache_error(error)),
                }
            }
            pool = open_pool(options).await.map_err(cache_error)?;
            migrations::migrator()
                .run(&pool)
                .await
                .map_err(cache_error)?;
        }
        if exclusive {
            lock.unlock().map_err(cache_error)?;
            acquire_shared(&lock).await?;
        }
        Ok((pool, lock))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn available(&self) -> bool {
        self.pool.as_ref().is_some_and(|pool| !pool.is_closed())
    }

    /// The same database, reached the way every caller reaches its records.
    fn data(&self) -> Option<Handle> {
        self.pool.as_ref().map(|pool| Handle::new(pool.clone()))
    }

    pub(crate) async fn generation(&self) -> i64 {
        let Some(data) = self.data() else {
            return -1;
        };
        ResourceCacheState::get(&data, &())
            .await
            .ok()
            .flatten()
            .map_or(-1, |state| state.generation)
    }

    pub(crate) async fn get(&self, key: &CacheKey) -> Option<ProviderValue> {
        self.get_cached(key, true, None)
            .await
            .map(|(value, _)| value)
    }

    pub(crate) async fn get_any(
        &self,
        key: &CacheKey,
    ) -> Option<ProviderValue> {
        self.get_cached(key, false, None)
            .await
            .map(|(value, _)| value)
    }

    pub(crate) async fn get_since(
        &self,
        key: &CacheKey,
        since: i64,
    ) -> Option<ProviderValue> {
        self.get_cached(key, true, Some(since))
            .await
            .map(|(value, _)| value)
    }

    pub(crate) async fn get_with_freshness(
        &self,
        key: &CacheKey,
    ) -> Option<(ProviderValue, bool)> {
        self.get_cached(key, false, None).await
    }

    async fn get_cached(
        &self,
        key: &CacheKey,
        fresh: bool,
        since: Option<i64>,
    ) -> Option<(ProviderValue, bool)> {
        let _guard = self.gate.read().await;
        let data = self.data()?;
        let key = entry_key(key);
        let record = ResourceCacheEntries::get(&data, &key).await.ok()??;
        if record.value_version != i64::from(RESOURCE_CONTRACT_VERSION)
            || since.is_some_and(|since| record.retrieved_at < since)
        {
            return None;
        }
        let expires = record
            .expires_at
            .min(record.retrieved_at / 1_000_000 + 3600);
        if fresh && expires <= Utc::now().timestamp() {
            return None;
        }
        match serde_json::from_str(&record.payload) {
            Ok(value) => Some((value, expires > Utc::now().timestamp())),
            Err(_) => {
                discard_stale(&data, &key, &record.payload).await;
                None
            }
        }
    }

    pub(crate) async fn put(
        &self,
        key: &CacheKey,
        value: &ProviderValue,
        ttl: i64,
        generation: i64,
        started_at: i64,
    ) {
        let _guard = self.gate.read().await;
        let Some(data) = self.data() else {
            return;
        };
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        let now = Utc::now().timestamp();
        self.prune_expired(now).await;
        let record = ResourceCacheEntryRecord {
            value_version: i64::from(RESOURCE_CONTRACT_VERSION),
            payload,
            retrieved_at: started_at,
            expires_at: now.saturating_add(ttl),
        };
        if let Err(error) =
            store_entry(&data, &entry_key(key), &record, generation).await
        {
            tracing::warn!(error = %error, "Resource response cache write failed");
        }
    }

    /// Drops one bounded page of long expired answers, every so often. The
    /// pruning spans the whole table, so it is the one statement of a write
    /// that no collection answers.
    async fn prune_expired(&self, now: i64) {
        let previous = self
            .last_maintenance
            .load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(previous) < 300 {
            return;
        }
        if self
            .last_maintenance
            .compare_exchange(
                previous,
                now,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            return;
        }
        let Some(pool) = &self.pool else {
            return;
        };
        let _ = sqlx::query("DELETE FROM resource_cache_entries WHERE rowid IN (SELECT rowid FROM resource_cache_entries WHERE expires_at < ? LIMIT 1000)").bind(now - 30 * 24 * 60 * 60).execute(pool).await;
    }

    pub async fn clear(&self) -> ResourceResult<()> {
        let _guard = self.gate.write().await;
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let data = Handle::new(pool.clone());
        let mut unit = atom(&data).await.map_err(cache_error)?;
        let checkpoint = ResourceLegacyImport::atom_get(&mut unit, &Checkpoint)
            .await
            .map_err(cache_error)?
            .unwrap_or(ResourceLegacyImportRecord {
                last_rowid: 0,
                completed: false,
            });
        ResourceLegacyImport::atom_set(
            &mut unit,
            &Checkpoint,
            Some(&ResourceLegacyImportRecord {
                completed: true,
                ..checkpoint
            }),
        )
        .await
        .map_err(cache_error)?;
        let current = ResourceCacheState::atom_get(&mut unit, &())
            .await
            .map_err(cache_error)?;
        if let Some(state) = current {
            let bumped = ResourceCacheStateRecord {
                // The generation is what tells a write started before this
                // clear that it no longer belongs here, so it may not stand
                // still: at the end of its range the cache has to be reset by
                // hand rather than cleared into the same generation.
                generation: state.generation.checked_add(1).ok_or_else(|| {
                    cache_error(crate::ErrorKind::OtherError(
                        "The cache generation is exhausted; reset the cache database".to_string(),
                    ))
                })?,
            };
            ResourceCacheState::atom_set(&mut unit, &(), Some(&bumped))
                .await
                .map_err(cache_error)?;
        }
        ResourceCacheEntries::atom_set_all(&mut unit, &[])
            .await
            .map_err(cache_error)?;
        ResourceArtifactEvidence::atom_set_all(&mut unit, &[])
            .await
            .map_err(cache_error)?;
        unit.commit().await.map_err(cache_error)
    }

    /// Explicit storage maintenance returns free database pages to the filesystem.
    pub(crate) async fn compact(&self) -> ResourceResult<()> {
        let _guard = self.gate.write().await;
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| ResourceError::Cache("pool disabled".into()))?;
        sqlx::query("VACUUM")
            .execute(pool)
            .await
            .map_err(cache_error)?;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(pool)
            .await
            .map_err(cache_error)?;
        Ok(())
    }

    pub async fn info(&self) -> ResourceResult<ResourceCacheInfo> {
        let _guard = self.gate.read().await;
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| ResourceError::Cache("pool disabled".into()))?;
        let _ = sqlx::query(
            "DELETE FROM resource_cache_entries WHERE expires_at <= ?",
        )
        .bind(Utc::now().timestamp() - 30 * 24 * 60 * 60)
        .execute(pool)
        .await;
        let count: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM resource_cache_entries) + (SELECT COUNT(*) FROM resource_artifact_evidence)").fetch_one(pool).await.map_err(cache_error)?;
        let mut size_bytes = 0u64;
        for path in [
            self.path.clone(),
            sidecar(&self.path, "-wal"),
            sidecar(&self.path, "-shm"),
        ] {
            size_bytes = size_bytes.saturating_add(
                tokio::fs::metadata(path)
                    .await
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
            );
        }
        Ok(ResourceCacheInfo {
            schema_version: migrations::CACHE_SCHEMA_VERSION,
            size_bytes,
            entry_count: count as u64,
        })
    }

    pub async fn close(&self) {
        let _guard = self.gate.write().await;
        if let Some(pool) = &self.pool {
            pool.close().await;
        }
        self.file_lock.lock().await.take();
    }

    pub(crate) async fn store_evidence<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        generation: i64,
    ) {
        let _guard = self.gate.read().await;
        let Some(data) = self.data() else {
            return;
        };
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        let record = ResourceArtifactEvidenceRecord {
            value_version: i64::from(RESOURCE_CONTRACT_VERSION),
            payload,
            verified_at: Utc::now().timestamp(),
        };
        let Ok(mut unit) = atom(&data).await else {
            return;
        };
        let current = ResourceCacheState::atom_get(&mut unit, &()).await;
        if !matches!(current, Ok(Some(state)) if state.generation == generation)
        {
            return;
        }
        if ResourceArtifactEvidence::atom_set(
            &mut unit,
            &key.to_owned(),
            Some(&record),
        )
        .await
        .is_ok()
        {
            let _ = unit.commit().await;
        }
    }

    pub(crate) async fn evidence<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Option<T> {
        let _guard = self.gate.read().await;
        let data = self.data()?;
        let record = ResourceArtifactEvidence::get(&data, &key.to_owned())
            .await
            .ok()??;
        if record.value_version != i64::from(RESOURCE_CONTRACT_VERSION) {
            return None;
        }
        serde_json::from_str(&record.payload).ok()
    }

    pub(crate) async fn invalidate_evidence(&self, key: &str) {
        let _guard = self.gate.read().await;
        if let Some(data) = self.data() {
            let _ = ResourceArtifactEvidence::set(&data, &key.to_owned(), None)
                .await;
        }
    }
}

fn entry_key(key: &CacheKey) -> ResourceCacheEntryKey {
    ResourceCacheEntryKey {
        namespace: key.namespace.clone(),
        source: key.source.as_str().to_string(),
        query_key: key.query.clone(),
    }
}

/// Writes an answer into the generation it was asked for, and only over an
/// answer that is not newer: an answer asked for before the cache was cleared
/// must not land in the emptied cache, and a slow one must not replace the
/// fresher answer already there.
async fn store_entry(
    data: &Handle,
    key: &ResourceCacheEntryKey,
    record: &ResourceCacheEntryRecord,
    generation: i64,
) -> crate::Result<()> {
    let mut unit = atom(data).await?;
    let current = ResourceCacheState::atom_get(&mut unit, &()).await?;
    if current.is_none_or(|state| state.generation != generation) {
        return Ok(());
    }
    let existing = ResourceCacheEntries::atom_get(&mut unit, key).await?;
    if existing
        .is_some_and(|existing| existing.retrieved_at > record.retrieved_at)
    {
        return Ok(());
    }
    ResourceCacheEntries::atom_set(&mut unit, key, Some(record)).await?;
    unit.commit().await
}

/// Drops the answer a reader could not interpret, unless another writer has
/// replaced it since it was read.
async fn discard_stale(
    data: &Handle,
    key: &ResourceCacheEntryKey,
    payload: &str,
) {
    let Ok(mut unit) = atom(data).await else {
        return;
    };
    let stale = ResourceCacheEntries::atom_get(&mut unit, key)
        .await
        .ok()
        .flatten()
        .is_some_and(|record| record.payload == payload);
    if !stale {
        return;
    }
    if ResourceCacheEntries::atom_set(&mut unit, key, None)
        .await
        .is_ok()
    {
        let _ = unit.commit().await;
    }
}

async fn open_pool(
    options: SqliteConnectOptions,
) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(options)
        .await
}

fn is_corrupt(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.code().and_then(|code| code.parse::<u32>().ok()).is_some_and(|code| matches!(code & 255, 11 | 26)))
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn cache_error(error: impl std::fmt::Display) -> ResourceError {
    ResourceError::Cache(error.to_string())
}

async fn acquire_shared(lock: &tokio::fs::File) -> ResourceResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if lock.try_lock_shared().map_err(cache_error)? {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| {
        ResourceError::Cache("cache initialization lock is busy".into())
    })?
}
