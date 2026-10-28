use super::sync_content_files::instance_content_root;
use crate::resources::ArtifactIntegrity;
use crate::resources::execution::{checked_path, matches_file};
use crate::state::State;
use crate::state::store::{
    content_file_fingerprints::ContentFileFingerprints,
    content_file_operations as journals,
};
use crate::util::io;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(test)]
#[path = "content_file_operations_tests.rs"]
mod tests;

#[derive(Serialize, Deserialize)]
struct FileMove {
    original: String,
    destination: String,
    integrity: ArtifactIntegrity,
}

#[derive(Serialize, Deserialize)]
struct Journal {
    root: String,
    delete: bool,
    files: Vec<FileMove>,
}

fn recovery_error(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

/// Caller holds the instance lock through publication and the database commit.
pub(super) async fn prepare(
    state: &State,
    instance_id: &str,
    root: &Path,
    paths: Vec<(String, String)>,
    delete: bool,
) -> crate::Result<()> {
    recover(state, instance_id, root).await?;
    let mut files = Vec::new();
    for (original, destination) in paths {
        if original == destination {
            continue;
        }
        let target = checked_path(root, &destination)?;
        if tokio::fs::try_exists(&target).await? {
            return Err(recovery_error(format!(
                "Content destination already exists: {destination}"
            )));
        }
        let previous = ContentFileFingerprints::get_current(
            &state.data,
            instance_id,
            &original,
        )
        .await?;
        let resolved = state
            .file_fingerprints
            .resolve_scan(
                root,
                &original,
                previous.as_ref(),
                false,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .map_err(crate::ErrorKind::ResourceError)?;
        if let Some(record) = &resolved.record {
            ContentFileFingerprints::set_current(
                &state.data,
                instance_id,
                record,
            )
            .await?;
        }
        files.push(FileMove {
            original,
            destination,
            integrity: resolved.identity.integrity,
        });
    }
    let journal = Journal {
        root: io::canonicalize(root)?.to_string_lossy().into_owned(),
        delete,
        files,
    };
    journals::set(&state.data, instance_id, &serde_json::to_string(&journal)?)
        .await?;
    Ok(())
}

pub(super) async fn publish(
    state: &State,
    instance_id: &str,
    root: &Path,
) -> crate::Result<()> {
    let (payload, _) = journals::get(&state.data, instance_id)
        .await?
        .ok_or_else(|| {
            recovery_error("Content operation journal disappeared")
        })?;
    let journal: Journal = serde_json::from_str(&payload)?;
    for file in journal.files {
        let original = checked_path(root, &file.original)?;
        let destination = checked_path(root, &file.destination)?;
        let previous = ContentFileFingerprints::get_current(
            &state.data,
            instance_id,
            &file.original,
        )
        .await?;
        let current = state
            .file_fingerprints
            .resolve_scan(
                root,
                &file.original,
                previous.as_ref(),
                false,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .map_err(crate::ErrorKind::ResourceError)?;
        if !crate::resources::identity::exact_integrity(
            &current.identity.integrity,
            &file.integrity,
        ) || tokio::fs::try_exists(&destination).await?
        {
            return Err(recovery_error(format!(
                "Content changed before file operation: {}",
                file.original
            )));
        }
        io::rename_or_move(original, destination).await?;
    }
    Ok(())
}

pub(super) use crate::state::store::content_file_operations::mark_committed;

/// Roll back an uncommitted rename or finish cleanup after a committed delete.
pub(super) async fn recover(
    state: &State,
    instance_id: &str,
    root: &Path,
) -> crate::Result<()> {
    let row = journals::get(&state.data, instance_id).await?;
    let Some((payload, committed)) = row else {
        return Ok(());
    };
    let journal: Journal = serde_json::from_str(&payload)?;
    if io::canonicalize(root)? != io::canonicalize(&journal.root)? {
        return Err(recovery_error(
            "Instance directory changed before content recovery",
        ));
    }
    for file in journal.files.iter().rev() {
        let original = checked_path(root, &file.original)?;
        let destination = checked_path(root, &file.destination)?;
        if committed {
            if !journal.delete {
                ContentFileFingerprints::transfer(
                    &state.data,
                    instance_id,
                    &file.original,
                    &file.destination,
                    root,
                    &file.integrity,
                )
                .await?;
            }
            if journal.delete && tokio::fs::try_exists(&destination).await? {
                if !matches_file(&destination, &file.integrity).await? {
                    return Err(recovery_error(format!(
                        "Deleted content staging file changed: {}",
                        file.destination
                    )));
                }
                io::remove_file(destination).await?;
            }
        } else if tokio::fs::try_exists(&destination).await? {
            if tokio::fs::try_exists(&original).await?
                || !matches_file(&destination, &file.integrity).await?
            {
                return Err(recovery_error(format!(
                    "Content recovery conflict: {}",
                    file.original
                )));
            }
            io::rename_or_move(destination, original).await?;
        } else if !matches_file(&original, &file.integrity).await? {
            return Err(recovery_error(format!(
                "Original content could not be recovered: {}",
                file.original
            )));
        }
    }
    journals::remove(&state.data, instance_id, &payload).await?;
    Ok(())
}

pub(super) async fn rollback_error(
    state: &State,
    instance_id: &str,
    root: &Path,
    error: crate::Error,
) -> crate::Error {
    match recover(state, instance_id, root).await {
        Ok(()) => error,
        Err(recovery) => recovery_error(format!(
            "{error}; content recovery requires retry: {recovery}"
        )),
    }
}

pub(super) async fn finish(state: &State, instance_id: &str, root: &Path) {
    if let Err(error) = recover(state, instance_id, root).await {
        tracing::warn!(%instance_id, %error, "Committed content operation cleanup deferred");
    }
}

pub(crate) async fn recover_content_file_operations(
    state: &State,
) -> crate::Result<()> {
    let instances = journals::get_instance_ids(&state.data).await?;
    let mut errors = Vec::new();
    for instance_id in instances {
        let result = async {
            let _lock = state
                .lock_instance_content_with_timeout(
                    &instance_id,
                    std::time::Duration::from_secs(20),
                )
                .await?;
            let scope =
                super::resolve_content_scope(&instance_id, None, state).await?;
            let root =
                instance_content_root(&state.directories, &scope.instance)?;
            recover(state, &instance_id, &root).await
        }
        .await;
        if let Err(error) = result {
            errors.push(format!("{instance_id}: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(recovery_error(errors.join("; ")))
    }
}
