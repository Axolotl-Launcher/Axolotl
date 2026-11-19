//! Shared leases protect metadata users against concurrent storage cleanup.

use fs4::tokio::AsyncFileExt;
use std::path::Path;
use tokio::fs::{File, OpenOptions};

/// No cleanup can run before global state is initialized, including isolated
/// library callers that operate on their own database and directories.
pub(crate) async fn lease() -> crate::Result<Option<File>> {
    if crate::state::DirectoryInfo::global_handle_if_ready().is_none() {
        return Ok(None);
    }
    let file = lock_file().await?;
    let file = tokio::task::spawn_blocking(move || {
        file.lock_shared()?;
        Ok::<_, std::io::Error>(file)
    })
    .await
    .map_err(std::io::Error::other)??;
    Ok(Some(file))
}

pub(super) async fn exclusive() -> crate::Result<File> {
    let file = lock_file().await?;
    if !file.try_lock_exclusive()? {
        return Err(crate::ErrorKind::InputError(
            "Metadata is in use. Wait for launch preparation or installation to finish.".into(),
        ).into());
    }
    Ok(file)
}

async fn lock_file() -> crate::Result<File> {
    let directories = super::dirs()?;
    open_lock_file(&directories.config_dir).await
}

async fn open_lock_file(config_dir: &Path) -> crate::Result<File> {
    tokio::fs::create_dir_all(config_dir)
        .await
        .map_err(|error| storage_error(error, config_dir))?;
    let path = config_dir.join(".metadata-use.lock");
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .await
        .map_err(|error| storage_error(error, &path))
}

fn storage_error(error: std::io::Error, path: &Path) -> crate::Error {
    crate::util::io::IOError::with_path(
        std::io::Error::new(
            error.kind(),
            format!(
                "Cannot access launcher storage: {error}. Check that the storage drive is connected and the directory is writable, or change the launcher storage directory in Settings"
            ),
        ),
        path,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_storage_directory_is_created() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("launcher/data");
        let file = open_lock_file(&root).await.unwrap();
        assert!(root.join(".metadata-use.lock").is_file());
        assert_eq!(file.metadata().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn inaccessible_storage_reports_path_and_can_be_retried() {
        let temp = tempfile::tempdir().unwrap();
        let blocked = temp.path().join("blocked");
        tokio::fs::write(&blocked, b"file in place of directory")
            .await
            .unwrap();
        let root = blocked.join("launcher");

        let error = open_lock_file(&root).await.unwrap_err().to_string();
        assert!(error.contains(root.to_string_lossy().as_ref()), "{error}");
        assert!(error.contains("storage drive is connected"), "{error}");

        tokio::fs::remove_file(&blocked).await.unwrap();
        open_lock_file(&root).await.unwrap();
        assert!(root.join(".metadata-use.lock").is_file());
    }

    #[tokio::test]
    async fn inaccessible_lock_reports_its_path() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".metadata-use.lock");
        tokio::fs::create_dir(&path).await.unwrap();

        let error = open_lock_file(temp.path()).await.unwrap_err().to_string();
        assert!(error.contains(path.to_string_lossy().as_ref()), "{error}");
        assert!(error.contains("directory is writable"), "{error}");
    }
}
