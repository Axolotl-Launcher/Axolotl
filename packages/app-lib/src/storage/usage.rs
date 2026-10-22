//! Shared leases protect metadata users against concurrent storage cleanup.

use fs4::tokio::AsyncFileExt;
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
    tokio::fs::create_dir_all(&directories.config_dir).await?;
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directories.config_dir.join(".metadata-use.lock"))
        .await?)
}
