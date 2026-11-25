use crate::ErrorKind;
use std::path::Path;

/// Break a link by replacing it with a copy of its current contents. Never
/// restore a deleted file from generated output, and never read a large file
/// into memory just to disable synchronization.
pub(super) async fn detach_existing_file(target: &Path) -> crate::Result<()> {
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || -> crate::Result<()> {
        match std::fs::metadata(&target) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
        let parent = target.parent().ok_or_else(|| {
            ErrorKind::InputError("Invalid servers.dat path".into())
        })?;
        let temporary = tempfile::NamedTempFile::new_in(parent)?;
        std::fs::copy(&target, temporary.path())?;
        temporary.persist(&target).map_err(|error| error.error)?;
        Ok(())
    })
    .await
    .map_err(|error| ErrorKind::FSError(error.to_string()))?
}
