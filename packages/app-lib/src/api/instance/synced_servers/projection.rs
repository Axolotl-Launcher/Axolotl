use super::super::synced_options::instance_game_dir;
use crate::state::InstanceMetadata;
use crate::{ErrorKind, State};
use std::path::{Path, PathBuf};

pub(super) fn existing_game_dir(
    metadata: &InstanceMetadata,
    state: &State,
) -> crate::Result<Option<PathBuf>> {
    match instance_game_dir(metadata, state) {
        Ok(path) => Ok(Some(path)),
        Err(error)
            if matches!(error.raw.as_ref(), ErrorKind::IOError(error)
            if error.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Check provenance by physical directory as well as hash: two records may
/// refer to the same servers.dat, but have different per-instance projections.
pub(super) async fn sibling_wrote_file(
    instance_id: &str,
    game_dir: &Path,
    actual_sha1: &str,
    state: &State,
) -> crate::Result<bool> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT instance_id FROM instance_sync_checkpoints
         WHERE feature = 'multiplayer_servers' AND expected_sha1 = ?
           AND instance_id != ?",
    )
    .bind(actual_sha1)
    .bind(instance_id)
    .fetch_all(&state.pool)
    .await?;
    for id in ids {
        if let Some(metadata) =
            crate::state::get_instance(&id, &state.pool).await?
            && existing_game_dir(&metadata, state)?.as_deref() == Some(game_dir)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

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
