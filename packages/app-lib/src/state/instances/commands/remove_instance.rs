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
    state.content_refresh.invalidate(instance_id).await;
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
        forget_derived_rows(instance_id, &state.derived).await;
        return Ok(());
    };

    let managed_path = state.directories.instances_dir().join(&instance.path);
    let missing_managed_profile = !instance.is_direct_linked()
        && instance.symlink_target.is_none()
        && instance.game_dir_override.is_none()
        && managed_profile_is_missing(
            &managed_path,
            &state.directories.instances_dir(),
        )
        .await?;
    super::content_file_operations::recover_before_instance_removal(
        state,
        instance_id,
        &super::instance_full_path(state, &instance),
        missing_managed_profile,
    )
    .await?;

    // Directly associated instances have no Axolotl profile directory. Their
    // version directory is the instance itself, so removal deliberately
    // deletes the externally managed content in place. Keep the shared
    // `.minecraft` root (assets/libraries/other versions) intact.
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
    forget_derived_rows(&instance.id, &state.derived).await;
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

/// A missing profile can be forgotten only while its storage is reachable.
async fn managed_profile_is_missing(
    profile: &Path,
    instances: &Path,
) -> crate::Result<bool> {
    match tokio::fs::symlink_metadata(profile).await {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = tokio::fs::metadata(instances)
                .await
                .map_err(|error| io::IOError::with_path(error, instances))?;
            if !parent.is_dir() {
                return Err(crate::ErrorKind::InputError(
                    "The instance storage is not an accessible directory"
                        .into(),
                )
                .into());
            }
            Ok(true)
        }
        Err(error) => Err(io::IOError::with_path(error, profile).into()),
    }
}

fn is_version_isolated_game_dir(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some("versions")
}

/// Drops the derived rows this instance owned. They moved to a database of
/// their own, so the foreign keys that used to cascade with the instance no
/// longer exist and this deletion is what removes them. Removing them again
/// costs nothing, which is what makes the call safe on a retry, and a derived
/// record is worth less than the deletion itself, so a failure is a warning.
pub(crate) async fn forget_derived_rows(
    instance_id: &str,
    derived: &crate::state::store::derived_db::DerivedDb,
) {
    use crate::state::store::content_refresh_pack::ContentRefreshPacks;
    use crate::state::store::content_refresh_scan_failures::ContentRefreshScanFailures;

    if let Err(error) =
        ContentRefreshPacks::forget_instance(derived, instance_id).await
    {
        tracing::warn!(
            instance_id,
            %error,
            "Failed to drop the saved pack metadata of a deleted instance"
        );
    }
    if let Err(error) =
        ContentRefreshScanFailures::forget_instance(derived, instance_id).await
    {
        tracing::warn!(
            instance_id,
            %error,
            "Failed to drop the scan failures of a deleted instance"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store::content_refresh_pack::{
        ContentRefreshPacks, PackKey, PackRecord,
    };
    use crate::state::store::content_refresh_scan_failures::{
        ContentRefreshScanFailures, ScanFailureKey, ScanFailureRecord,
    };
    use std::sync::Arc;

    async fn state_with_one_instance() -> (tempfile::TempDir, Arc<State>) {
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().into(),
            config_dir: temp.path().into(),
            app_identifier: "remove-instance-test".into(),
        };
        std::fs::create_dir_all(directories.instances_dir().join("example"))
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("app.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO instances(id, path, name, install_stage, launcher_feature_version, created, modified)
                VALUES ('instance', 'example', 'Example', 'installed', '1', 1, 1);
             INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
                VALUES ('set', 'instance', 'Main', 'local', 'available', '1.21', 'fabric', 1, 1);
             UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';",
        )
        .execute(&pool)
        .await
        .unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        (temp, state)
    }

    async fn record_file_journal(state: &State, root: &Path, committed: bool) {
        let payload = serde_json::json!({
            "root": io::canonicalize(root).unwrap().to_string_lossy(),
            "delete": true,
            "files": [{
                "original": "mods/previous.jar",
                "destination": "mods/previous.jar.removing",
                "integrity": { "size": 1, "hashes": [] }
            }]
        })
        .to_string();
        crate::state::store::content_file_operations::set(
            &state.data,
            "instance",
            &payload,
        )
        .await
        .unwrap();
        if committed {
            let mut tx = state.pool.begin().await.unwrap();
            crate::state::store::content_file_operations::mark_committed(
                "instance", &mut tx,
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    }

    #[tokio::test]
    async fn config_sync_waits_for_deletion_and_does_not_recreate_files() {
        for periodic in [false, true] {
            let (_temp, state) = state_with_one_instance().await;
            config_sync::sync_instance(&state, "instance")
                .await
                .unwrap();
            let profile = state.directories.instances_dir().join("example");
            assert!(profile.join(config_sync::CONFIG_FILE_NAME).is_file());

            let deleting = state.lock_instance_content("instance").await;
            std::fs::remove_file(profile.join(config_sync::CONFIG_FILE_NAME))
                .unwrap();
            let syncing_state = Arc::clone(&state);
            let mut syncing = tokio::spawn(async move {
                if periodic {
                    config_sync::reconcile_all(&syncing_state).await
                } else {
                    config_sync::sync_instance(&syncing_state, "instance").await
                }
            });
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    &mut syncing,
                )
                .await
                .is_err(),
                "config sync must wait while deletion holds the content lock"
            );
            assert!(!profile.join(config_sync::CONFIG_FILE_NAME).exists());

            remove_instance_files_and_state("instance", &state, false)
                .await
                .unwrap();
            drop(deleting);
            tokio::time::timeout(std::time::Duration::from_secs(5), syncing)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(!profile.exists());
            assert!(
                sqlx::query("PRAGMA foreign_key_check")
                    .fetch_all(&state.pool)
                    .await
                    .unwrap()
                    .is_empty()
            );
            state.pool.close().await;
        }
    }

    #[tokio::test]
    async fn deleting_a_missing_managed_profile_removes_its_record() {
        let (_temp, state) = state_with_one_instance().await;
        std::fs::remove_dir_all(
            state.directories.instances_dir().join("example"),
        )
        .unwrap();
        remove_instance_files_and_state("instance", &state, false)
            .await
            .unwrap();
        assert!(
            instance_rows::get_instance_by_id("instance", &state.pool)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&state.pool)
                .await
                .unwrap()
                .is_empty()
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn deleting_missing_profile_clears_only_committed_file_recovery() {
        for committed in [false, true] {
            let (_temp, state) = state_with_one_instance().await;
            let profile = state.directories.instances_dir().join("example");
            record_file_journal(&state, &profile, committed).await;
            std::fs::remove_dir_all(&profile).unwrap();
            let result =
                remove_instance_files_and_state("instance", &state, false)
                    .await;
            assert_eq!(result.is_ok(), committed);
            assert_eq!(
                instance_rows::get_instance_by_id("instance", &state.pool)
                    .await
                    .unwrap()
                    .is_none(),
                committed
            );
            assert_eq!(
                crate::state::store::content_file_operations::get(
                    &state.data,
                    "instance"
                )
                .await
                .unwrap()
                .is_none(),
                committed
            );
            state.pool.close().await;
        }
    }

    #[tokio::test]
    async fn unavailable_managed_storage_keeps_the_instance_record() {
        let (_temp, state) = state_with_one_instance().await;
        std::fs::remove_dir_all(state.directories.instances_dir()).unwrap();
        assert!(
            remove_instance_files_and_state("instance", &state, false)
                .await
                .is_err()
        );
        assert!(
            instance_rows::get_instance_by_id("instance", &state.pool)
                .await
                .unwrap()
                .is_some()
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn invalid_storage_is_not_treated_as_a_missing_profile() {
        let temp = tempfile::tempdir().unwrap();
        let storage = temp.path().join("not-a-directory");
        std::fs::write(&storage, b"keep").unwrap();
        assert!(
            managed_profile_is_missing(&storage.join("example"), &storage)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&storage).unwrap(), b"keep");
    }

    #[tokio::test]
    async fn missing_external_content_keeps_its_recovery_journal() {
        let (temp, state) = state_with_one_instance().await;
        let external = temp.path().join("external");
        std::fs::create_dir_all(&external).unwrap();
        sqlx::query(
            "UPDATE instances SET game_dir_override = ? WHERE id = 'instance'",
        )
        .bind(external.to_string_lossy().as_ref())
        .execute(&state.pool)
        .await
        .unwrap();
        record_file_journal(&state, &external, true).await;
        std::fs::remove_dir_all(&external).unwrap();
        assert!(
            remove_instance_files_and_state("instance", &state, false)
                .await
                .is_err()
        );
        assert!(
            instance_rows::get_instance_by_id("instance", &state.pool)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            crate::state::store::content_file_operations::get(
                &state.data,
                "instance"
            )
            .await
            .unwrap()
            .is_some()
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn a_changed_profile_root_does_not_discard_file_recovery() {
        let (temp, state) = state_with_one_instance().await;
        let different = temp.path().join("different");
        std::fs::create_dir_all(&different).unwrap();
        record_file_journal(&state, &different, true).await;
        std::fs::remove_dir_all(
            state.directories.instances_dir().join("example"),
        )
        .unwrap();
        assert!(
            remove_instance_files_and_state("instance", &state, false)
                .await
                .is_err()
        );
        assert!(
            crate::state::store::content_file_operations::get(
                &state.data,
                "instance"
            )
            .await
            .unwrap()
            .is_some()
        );
        assert!(different.is_dir());
        state.pool.close().await;
    }

    /// The instance is what owns these rows now: the foreign keys that used to
    /// remove them went with the tables, so the deletion has to.
    #[tokio::test]
    async fn deleting_an_instance_drops_its_derived_rows() {
        let (_temp, state) = state_with_one_instance().await;
        let pack = PackKey {
            instance_id: "instance".into(),
            content_set_id: "set".into(),
        };
        ContentRefreshPacks::upsert(
            &state.derived,
            &pack,
            &PackRecord {
                location_key: "root".into(),
                link_key: "link".into(),
                saved_at: 10,
                pack: None,
                attempted_at: 10,
                last_error: None,
            },
        )
        .await
        .unwrap();
        ContentRefreshScanFailures::upsert(
            &state.derived,
            &ScanFailureKey {
                instance_id: "instance".into(),
                location_key: "root".into(),
                relative_path: "mods/bad.jar".into(),
            },
            &ScanFailureRecord {
                attempted_at: 10,
                error: "locked".into(),
            },
        )
        .await
        .unwrap();

        remove_instance_files_and_state("instance", &state, false)
            .await
            .unwrap();

        assert!(
            instance_rows::get_instance_by_id("instance", &state.pool)
                .await
                .unwrap()
                .is_none(),
            "the deletion step removes the instance itself"
        );
        assert!(
            ContentRefreshPacks::saved(&state.derived, &pack)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            ContentRefreshScanFailures::failed(&state.derived, "instance")
                .await
                .unwrap()
                .is_empty()
        );
        state.pool.close().await;
    }

    /// A database a previous deletion never finished cleaning is left holding
    /// rows for an instance that is gone. Running the same step again finds no
    /// instance and still drops them, which is what makes it safe on a retry.
    #[tokio::test]
    async fn deleting_an_instance_that_is_gone_still_drops_its_derived_rows() {
        let (_temp, state) = state_with_one_instance().await;
        ContentRefreshScanFailures::upsert(
            &state.derived,
            &ScanFailureKey {
                instance_id: "instance".into(),
                location_key: "root".into(),
                relative_path: "mods/bad.jar".into(),
            },
            &ScanFailureRecord {
                attempted_at: 10,
                error: "locked".into(),
            },
        )
        .await
        .unwrap();
        instance_rows::delete_instance_by_id("instance", &state.pool)
            .await
            .unwrap();

        remove_instance_files_and_state("instance", &state, false)
            .await
            .unwrap();

        assert!(
            ContentRefreshScanFailures::failed(&state.derived, "instance")
                .await
                .unwrap()
                .is_empty()
        );
        state.pool.close().await;
    }
}
