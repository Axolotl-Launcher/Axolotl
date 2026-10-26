use crate::state::State;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::instances::config_sync;
use crate::util::io;
use std::path::{Path, PathBuf};

pub(crate) async fn remove_instance(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    remove_instance_with_policy(instance_id, state, false).await
}

pub(crate) async fn remove_instance_preserving_external_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    remove_instance_with_policy(instance_id, state, true).await
}

async fn remove_instance_with_policy(
    instance_id: &str,
    state: &State,
    preserve_external_files: bool,
) -> crate::Result<()> {
    let _maintenance_guard =
        crate::api::instance::lock_instance_maintenance(instance_id).await;
    crate::api::instance::begin_instance_deletion(instance_id).await?;
    let result = remove_instance_files_and_state(
        instance_id,
        state,
        preserve_external_files,
    )
    .await;
    if let Err(error) = result {
        if let Err(cancel_error) =
            crate::api::instance::cancel_instance_deletion(instance_id).await
        {
            tracing::warn!(
                instance_id,
                %cancel_error,
                "Failed to clear pending backup deletion marker"
            );
        }
        return Err(error);
    }
    crate::api::instance::delete_instance_backups(instance_id).await
}

async fn remove_instance_files_and_state(
    instance_id: &str,
    state: &State,
    preserve_external_files: bool,
) -> crate::Result<()> {
    let _synced_options_lock = state.lock_synced_options().await;
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let Some(instance) =
        instance_rows::get_instance_by_id(instance_id, &state.pool).await?
    else {
        // The external-instance watcher can remove a record while a stale
        // library view still has its delete dialog open. Deletion is
        // idempotent, so finish cleanup successfully instead of surfacing a
        // spurious "Unknown instance" error.
        tracing::debug!(instance_id, "Instance was already removed");
        return Ok(());
    };

    super::content_file_operations::recover(
        state,
        instance_id,
        &super::instance_full_path(state, &instance),
    )
    .await?;

    // Directly associated instances have no Axolotl profile directory. Their
    // version directory is the instance itself, so removal deliberately
    // deletes the externally managed content in place. Keep the shared
    // `.minecraft` root (assets/libraries/other versions) intact.
    let managed_path = state.directories.instances_dir().join(&instance.path);
    let path = if preserve_external_files || instance.symlink_target.is_some() {
        managed_path
    } else if instance.is_direct_linked() {
        crate::launcher::DirectLinkedLaunch::from_instance(&instance)?
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(
                    "Direct instance link metadata is incomplete".to_string(),
                )
            })?
            .version_dir()
    } else if let Some(game_dir_override) = instance
        .game_dir_override
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| is_version_isolated_game_dir(path))
    {
        // New instances created against a configured `.minecraft` root use
        // a private `versions/<name>` directory. Remove that external
        // directory when the instance is deleted, while preserving shared
        // (non-isolated) overrides for backwards compatibility.
        game_dir_override
    } else {
        managed_path
    };
    let watched_path = crate::launcher::linked_game_dir(&instance)
        .or_else(|| instance.linked_dot_minecraft.as_deref().map(PathBuf::from))
        .unwrap_or_else(|| state.directories.instance_game_dir(&instance));
    crate::state::instances::watcher::unwatch_instance_folder(
        &instance.path,
        &watched_path,
        &state.file_watcher,
    )
    .await;
    crate::api::instance::remove_generated_instance_files(instance_id, state)
        .await?;
    io::remove_dir_all(&path).await?;

    let jobs = crate::install::store::mark_instance_deleted(instance_id, state)
        .await?;
    instance_rows::delete_instance_by_id(&instance.id, &state.pool).await?;
    config_sync::remove_config_file(&state.directories, &instance.path).await?;
    for job in jobs {
        if let Err(error) =
            crate::install::events::emit_install_job(&job.snapshot()).await
        {
            tracing::warn!(
                "Failed to emit deleted instance download state: {error}"
            );
        }
    }

    Ok(())
}

fn is_version_isolated_game_dir(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("versions")
}
