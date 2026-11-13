//! Exclusive ownership and recoverable replacement of one SQLite file family.

use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const SUFFIXES: [&str; 3] = ["-wal", "-shm", ""];

pub(super) struct Ownership {
    _file: tokio::fs::File,
}

impl Ownership {
    pub(super) async fn acquire(database: &Path) -> crate::Result<Self> {
        let parent = database.parent().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Database has no parent directory".into(),
            )
        })?;
        tokio::fs::create_dir_all(parent).await?;
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(sidecar(database, ".ownership.lock"))
            .await?;
        if !file.try_lock_exclusive()? {
            return Err(crate::ErrorKind::FSError(format!(
                "Database {} is in use by another launcher. Close it before opening or replacing this database",
                database.display()
            ))
            .into());
        }
        Ok(Self { _file: file })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    id: uuid::Uuid,
    original: [bool; 3],
}

impl Replacement {
    fn staged(&self, database: &Path) -> PathBuf {
        sidecar(database, &format!(".replace-{}.next", self.id))
    }

    fn archived(&self, database: &Path) -> PathBuf {
        sidecar(database, &format!(".replace-{}.previous", self.id))
    }
}

pub(super) fn sidecar(database: &Path, suffix: &str) -> PathBuf {
    let mut name = database.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// The caller holds Ownership across recovery, validation and opening its pool.
pub(super) async fn recover(database: &Path) -> crate::Result<()> {
    let marker = sidecar(database, ".replacement.json");
    let bytes = match tokio::fs::read(&marker).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let replacement: Replacement = serde_json::from_slice(&bytes)?;
    let staged = replacement.staged(database);
    let archived = replacement.archived(database);
    if tokio::fs::try_exists(&staged).await? {
        // Publication did not commit. Restore each moved original before any
        // SQLite connection can combine a partial family with an unrelated WAL.
        for (suffix, existed) in SUFFIXES.into_iter().zip(replacement.original)
        {
            let original = sidecar(database, suffix);
            let previous = sidecar(&archived, suffix);
            if tokio::fs::try_exists(&previous).await? {
                if tokio::fs::try_exists(&original).await? {
                    return Err(crate::ErrorKind::FSError(format!(
                        "Interrupted database replacement has conflicting files: {} and {}",
                        original.display(), previous.display()
                    ))
                    .into());
                }
                tokio::fs::rename(&previous, &original).await?;
            } else if existed && !tokio::fs::try_exists(&original).await? {
                return Err(crate::ErrorKind::FSError(format!(
                    "Interrupted database replacement is missing {}",
                    original.display()
                ))
                .into());
            }
        }
        tokio::fs::remove_file(&staged).await?;
        tracing::warn!(database = %database.display(), "Rolled back interrupted database replacement");
    } else if !tokio::fs::try_exists(database).await? {
        return Err(crate::ErrorKind::FSError(format!(
            "Interrupted database replacement has no published database: {}",
            database.display()
        ))
        .into());
    }
    tokio::fs::remove_file(marker).await?;
    Ok(())
}

/// Publishes a closed, verified, standalone snapshot on the same filesystem.
/// The previous main/WAL/SHM are retained together for manual recovery.
pub(super) async fn replace(
    database: &Path,
    staged: &Path,
) -> crate::Result<PathBuf> {
    recover(database).await?;
    let mut original = [false; 3];
    for (index, suffix) in SUFFIXES.iter().enumerate() {
        original[index] =
            tokio::fs::try_exists(sidecar(database, suffix)).await?;
    }
    let replacement = Replacement {
        id: uuid::Uuid::new_v4(),
        original,
    };
    let next = replacement.staged(database);
    let archived = replacement.archived(database);
    tokio::fs::rename(staged, &next).await?;
    let marker = sidecar(database, ".replacement.json");
    let temporary = sidecar(&marker, &format!(".{}", replacement.id));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await?;
    file.write_all(&serde_json::to_vec(&replacement)?).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temporary, &marker).await?;
    let result = async {
        for (suffix, existed) in SUFFIXES.into_iter().zip(original) {
            if existed {
                tokio::fs::rename(
                    sidecar(database, suffix),
                    sidecar(&archived, suffix),
                )
                .await?;
            }
        }
        tokio::fs::rename(&next, database).await?;
        Ok::<(), crate::Error>(())
    }
    .await;
    match result {
        Ok(()) => {
            tokio::fs::remove_file(marker).await?;
            Ok(archived)
        }
        Err(error) => {
            if let Err(rollback) = recover(database).await {
                return Err(crate::ErrorKind::FSError(format!(
                    "Database replacement failed: {error}; rollback needs recovery: {rollback}"
                ))
                .into());
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ownership_excludes_another_handle_until_release() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("app.db");
        let owner = Ownership::acquire(&database).await.unwrap();
        assert!(Ownership::acquire(&database).await.is_err());
        drop(owner);
        assert!(Ownership::acquire(&database).await.is_ok());
    }

    #[tokio::test]
    async fn replacement_preserves_old_family_and_leaves_no_stale_wal() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("app.db");
        let staged = dir.path().join("next.db");
        for suffix in SUFFIXES {
            tokio::fs::write(
                sidecar(&database, suffix),
                format!("old{suffix}"),
            )
            .await
            .unwrap();
        }
        tokio::fs::write(&staged, "new").await.unwrap();
        let archived = replace(&database, &staged).await.unwrap();
        assert_eq!(tokio::fs::read(&database).await.unwrap(), b"new");
        for suffix in SUFFIXES {
            assert_eq!(
                tokio::fs::read(sidecar(&archived, suffix)).await.unwrap(),
                format!("old{suffix}").as_bytes()
            );
            if !suffix.is_empty() {
                assert!(!sidecar(&database, suffix).exists());
            }
        }
    }

    #[tokio::test]
    async fn every_archive_cut_point_recovers_the_original_family() {
        for moved in 0..=3 {
            let dir = tempfile::tempdir().unwrap();
            let database = dir.path().join("app.db");
            let transaction = Replacement {
                id: uuid::Uuid::new_v4(),
                original: [true; 3],
            };
            let archived = transaction.archived(&database);
            tokio::fs::write(transaction.staged(&database), "new")
                .await
                .unwrap();
            tokio::fs::write(
                sidecar(&database, ".replacement.json"),
                serde_json::to_vec(&transaction).unwrap(),
            )
            .await
            .unwrap();
            for (index, suffix) in SUFFIXES.iter().enumerate() {
                let path = if index < moved {
                    sidecar(&archived, suffix)
                } else {
                    sidecar(&database, suffix)
                };
                tokio::fs::write(path, format!("old{suffix}"))
                    .await
                    .unwrap();
            }
            recover(&database).await.unwrap();
            recover(&database).await.unwrap();
            for suffix in SUFFIXES {
                assert_eq!(
                    tokio::fs::read(sidecar(&database, suffix)).await.unwrap(),
                    format!("old{suffix}").as_bytes()
                );
            }
        }
    }

    #[tokio::test]
    async fn a_committed_replacement_does_not_restore_its_old_wal() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("app.db");
        let transaction = Replacement {
            id: uuid::Uuid::new_v4(),
            original: [true; 3],
        };
        tokio::fs::write(&database, "new").await.unwrap();
        tokio::fs::write(
            sidecar(&database, ".replacement.json"),
            serde_json::to_vec(&transaction).unwrap(),
        )
        .await
        .unwrap();
        for suffix in SUFFIXES {
            tokio::fs::write(
                sidecar(&transaction.archived(&database), suffix),
                "old",
            )
            .await
            .unwrap();
        }
        recover(&database).await.unwrap();
        assert_eq!(tokio::fs::read(&database).await.unwrap(), b"new");
        assert!(!sidecar(&database, "-wal").exists());
    }
}
