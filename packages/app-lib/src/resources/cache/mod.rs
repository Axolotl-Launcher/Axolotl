//! An independently versioned SQLx pool. No joins or transactions use app.db.

mod legacy;
mod migrations;

use chrono::Utc;
use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use super::{ipc::ResourceCacheInfo, providers::ProviderValue, *};

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

    pub(crate) async fn generation(&self) -> i64 {
        let Some(pool) = &self.pool else {
            return -1;
        };
        sqlx::query_scalar(
            "SELECT generation FROM resource_cache_state WHERE id = 1",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(-1)
    }

    pub(crate) async fn get(&self, key: &CacheKey) -> Option<ProviderValue> {
        self.get_cached(key, true).await.map(|(value, _)| value)
    }

    pub(crate) async fn get_any(
        &self,
        key: &CacheKey,
    ) -> Option<ProviderValue> {
        self.get_cached(key, false).await.map(|(value, _)| value)
    }

    pub(crate) async fn get_with_freshness(
        &self,
        key: &CacheKey,
    ) -> Option<(ProviderValue, bool)> {
        self.get_cached(key, false).await
    }

    async fn get_cached(
        &self,
        key: &CacheKey,
        fresh: bool,
    ) -> Option<(ProviderValue, bool)> {
        let _guard = self.gate.read().await;
        let pool = self.pool.as_ref()?;
        let row = sqlx::query("SELECT payload, value_version, expires_at FROM resource_cache_entries WHERE namespace = ? AND source = ? AND query_key = ? AND (expires_at > ? OR ? = 0)")
            .bind(&key.namespace).bind(key.source.as_str()).bind(&key.query).bind(Utc::now().timestamp()).bind(fresh).fetch_optional(pool).await.ok()??;
        let version: i64 = row.try_get("value_version").ok()?;
        let payload: String = row.try_get("payload").ok()?;
        let expires: i64 = row.try_get("expires_at").ok()?;
        if version != i64::from(RESOURCE_CONTRACT_VERSION) {
            return None;
        }
        match serde_json::from_str(&payload) {
            Ok(value) => Some((value, expires > Utc::now().timestamp())),
            Err(_) => {
                let _ = sqlx::query("DELETE FROM resource_cache_entries WHERE namespace = ? AND source = ? AND query_key = ? AND payload = ?")
                    .bind(&key.namespace).bind(key.source.as_str()).bind(&key.query).bind(payload).execute(pool).await;
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
        let Some(pool) = &self.pool else {
            return;
        };
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        let now = Utc::now().timestamp();
        let previous = self
            .last_maintenance
            .load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(previous) >= 300
            && self
                .last_maintenance
                .compare_exchange(
                    previous,
                    now,
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
        {
            let _ = sqlx::query("DELETE FROM resource_cache_entries WHERE rowid IN (SELECT rowid FROM resource_cache_entries WHERE expires_at < ? LIMIT 1000)").bind(now - 30 * 24 * 60 * 60).execute(pool).await;
        }
        let result = sqlx::query("INSERT INTO resource_cache_entries (namespace, source, query_key, value_version, payload, retrieved_at, expires_at)
            SELECT ?, ?, ?, ?, ?, ?, ? FROM resource_cache_state WHERE id = 1 AND generation = ?
            ON CONFLICT(namespace, source, query_key) DO UPDATE SET value_version = excluded.value_version, payload = excluded.payload, retrieved_at = excluded.retrieved_at, expires_at = excluded.expires_at WHERE excluded.retrieved_at >= resource_cache_entries.retrieved_at")
            .bind(&key.namespace).bind(key.source.as_str()).bind(&key.query).bind(i64::from(RESOURCE_CONTRACT_VERSION)).bind(payload).bind(started_at).bind(now.saturating_add(ttl)).bind(generation).execute(pool).await;
        if let Err(error) = result {
            tracing::warn!(error = %error, "Resource response cache write failed");
        }
    }

    pub async fn clear(&self) -> ResourceResult<()> {
        let _guard = self.gate.write().await;
        let Some(pool) = &self.pool else {
            return Ok(());
        };
        let mut transaction = pool.begin().await.map_err(cache_error)?;
        sqlx::query("UPDATE resource_cache_state SET generation = generation + 1 WHERE id = 1").execute(&mut *transaction).await.map_err(cache_error)?;
        sqlx::query("DELETE FROM resource_cache_entries")
            .execute(&mut *transaction)
            .await
            .map_err(cache_error)?;
        sqlx::query("DELETE FROM resource_artifact_evidence")
            .execute(&mut *transaction)
            .await
            .map_err(cache_error)?;
        sqlx::query(
            "UPDATE resource_legacy_import SET completed = 1 WHERE id = 1",
        )
        .execute(&mut *transaction)
        .await
        .map_err(cache_error)?;
        transaction.commit().await.map_err(cache_error)?;
        Ok(())
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
        let Some(pool) = &self.pool else {
            return;
        };
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        let _ = sqlx::query("INSERT INTO resource_artifact_evidence (evidence_key, value_version, payload, verified_at) SELECT ?, ?, ?, ? FROM resource_cache_state WHERE id = 1 AND generation = ? ON CONFLICT(evidence_key) DO UPDATE SET payload = excluded.payload, value_version = excluded.value_version, verified_at = excluded.verified_at")
            .bind(key).bind(i64::from(RESOURCE_CONTRACT_VERSION)).bind(payload).bind(Utc::now().timestamp()).bind(generation).execute(pool).await;
    }

    pub(crate) async fn evidence<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Option<T> {
        let _guard = self.gate.read().await;
        let pool = self.pool.as_ref()?;
        let payload: String = sqlx::query_scalar("SELECT payload FROM resource_artifact_evidence WHERE evidence_key = ? AND value_version = ?").bind(key).bind(i64::from(RESOURCE_CONTRACT_VERSION)).fetch_optional(pool).await.ok()??;
        serde_json::from_str(&payload).ok()
    }

    pub(crate) async fn invalidate_evidence(&self, key: &str) {
        let _guard = self.gate.read().await;
        if let Some(pool) = &self.pool {
            let _ = sqlx::query(
                "DELETE FROM resource_artifact_evidence WHERE evidence_key = ?",
            )
            .bind(key)
            .execute(pool)
            .await;
        }
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
