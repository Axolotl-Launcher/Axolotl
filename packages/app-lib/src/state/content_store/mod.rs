//! Cross-instance verified content objects.

use crate::state::DirectoryInfo;
use crate::util::io;
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Weak};
use tokio::sync::Mutex;

static ACQUIRE_LOCKS: LazyLock<DashMap<String, Weak<Mutex<()>>>> =
    LazyLock::new(DashMap::new);

pub(crate) fn object_path(
    directories: &DirectoryInfo,
    sha512: &str,
) -> PathBuf {
    directories
        .content_store_dir()
        .join(sha512.to_ascii_lowercase())
}

pub(crate) async fn acquire_lock(sha512: &str) -> Arc<Mutex<()>> {
    if ACQUIRE_LOCKS.len() >= 256 {
        ACQUIRE_LOCKS.retain(|_, lock| lock.strong_count() > 0);
    }
    let mut slot = ACQUIRE_LOCKS
        .entry(sha512.to_ascii_lowercase())
        .or_default();
    if let Some(lock) = slot.upgrade() {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    *slot = Arc::downgrade(&lock);
    lock
}

pub(crate) async fn publish_verified(
    source: &Path,
    directories: &DirectoryInfo,
    sha512: &str,
) -> crate::Result<PathBuf> {
    if sha512.len() != 128
        || !sha512.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid content SHA-512 digest".into(),
        )
        .into());
    }
    let lock = acquire_lock(sha512).await;
    let _guard = lock.lock().await;
    let destination = object_path(directories, sha512);
    let integrity = crate::util::fetch::Integrity {
        sha512: Some(sha512.to_ascii_lowercase()),
        ..Default::default()
    };
    if tokio::fs::try_exists(&destination).await? {
        match crate::util::fetch::verify_file(&destination, &integrity).await {
            Ok(_) => return Ok(destination),
            Err(error) if crate::util::fetch::is_integrity_error(&error) => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(parent) = destination.parent() {
        io::create_dir_all(parent).await?;
    }
    let mut temporary = destination.as_os_str().to_os_string();
    temporary.push(".installing");
    let temporary = PathBuf::from(temporary);
    if tokio::fs::try_exists(&temporary).await.unwrap_or(false) {
        io::remove_file(&temporary).await?;
    }
    io::copy(source, &temporary).await?;
    if let Err(error) =
        crate::util::fetch::verify_file(&temporary, &integrity).await
    {
        let _ = io::remove_file(&temporary).await;
        return Err(error);
    }
    if let Err(error) =
        crate::util::fetch::finalize_download(&temporary, &destination).await
    {
        let _ = io::remove_file(&temporary).await;
        return Err(error);
    }
    Ok(destination)
}

pub(crate) async fn record_published(
    pool: &SqlitePool,
    digest: &str,
    size: u64,
) -> crate::Result<()> {
    crate::state::store::store_blobs::record(
        pool,
        digest,
        size,
        chrono::Utc::now().timestamp(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    #[tokio::test]
    async fn weak_acquire_locks_are_shared_until_the_last_holder_drops() {
        let first = acquire_lock("LOCK-REGRESSION").await;
        let second = acquire_lock("lock-regression").await;
        assert!(Arc::ptr_eq(&first, &second));
        let weak = Arc::downgrade(&first);
        drop(first);
        assert!(weak.upgrade().is_some());
        drop(second);
        ACQUIRE_LOCKS.retain(|_, lock| lock.strong_count() > 0);
        assert!(weak.upgrade().is_none());
        assert!(!ACQUIRE_LOCKS.contains_key("lock-regression"));
    }

    #[tokio::test]
    async fn corrupted_objects_are_replaced_only_by_a_verified_copy() {
        let directory = tempfile::tempdir().unwrap();
        let directories = DirectoryInfo {
            config_dir: directory.path().into(),
            settings_dir: directory.path().into(),
            app_identifier: "test".into(),
        };
        let source = directory.path().join("source");
        let good = b"verified shared content";
        let digest = format!("{:x}", Sha512::digest(good));
        tokio::fs::write(&source, good).await.unwrap();
        let object = publish_verified(&source, &directories, &digest)
            .await
            .unwrap();
        tokio::fs::write(&object, b"corrupted shared cache")
            .await
            .unwrap();
        assert_eq!(
            publish_verified(&source, &directories, &digest)
                .await
                .unwrap(),
            object
        );
        assert_eq!(tokio::fs::read(&object).await.unwrap(), good);
        tokio::fs::write(&object, b"corrupted again").await.unwrap();
        tokio::fs::write(&source, b"wrong source").await.unwrap();
        assert!(
            publish_verified(&source, &directories, &digest)
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(&object).await.unwrap(), b"corrupted again");
    }
}
