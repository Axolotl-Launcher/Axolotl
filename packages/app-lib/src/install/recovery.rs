use super::events::emit_install_job;
use super::model::{
    InstallCleanup, InstallContentRollbackScope,
    InstallContentRollbackSnapshot, InstallContentRollbackStage,
    InstallErrorView, InstallInterruptReason, InstallJobDisplay,
    InstallJobEventKind, InstallJobState, InstallJobStatus,
    InstallPhaseDetails, InstallPhaseId, InstallRequest,
    InstallRollbackContentEntry, InstallRollbackFile,
    InstallRollbackProviderRef, InstallTarget,
};
use super::store;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::{ContentOwnershipKind, State};
use path_util::SafeRelativeUtf8UnixPathBuf;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub async fn recover_interrupted_jobs(state: &State) -> crate::Result<()> {
    crate::state::instances::commands::recover_legacy_rollbacks(state).await?;
    let jobs = store::list_interrupted_candidates(state).await?;

    for mut job in jobs {
        if crate::resources::plans::frozen_request_completed(
            &job.state.request,
            &state.pool,
        )
        .await?
        {
            if let Err(error) = apply_cleanup(&mut job.state, state).await {
                tracing::warn!(job_id = %job.id, %error, "Committed resource task staging cleanup deferred");
            }
            job.state.progress.phase = InstallPhaseId::Finalizing;
            job.state.progress.progress = None;
            job.state.progress.details = InstallPhaseDetails::Empty;
            job.state.progress.parallel = None;
            job.state.error = None;
            job.state.rollback_error = None;
            job.state.context = None;
            job.state.pause_reason = None;
            job.state.missing_content = None;
            job.state.rollback = None;
            job.state.record_event(InstallJobEventKind::JobSucceeded {
                instance_id: Some(job.instance_id.clone().ok_or_else(
                    || {
                        crate::ErrorKind::InputError(
                            "committed resource job has no instance".into(),
                        )
                    },
                )?),
            });
            if let Some(record) =
                store::complete_running_job(job.id, &job.state, state).await?
            {
                emit_install_job(&record.snapshot()).await?;
                continue;
            }
        }
        if job.state.display.is_none() {
            job.state.display = display_from_request(&job.state);
        }
        let interrupted_phase = job.state.progress.phase;
        job.state.record_event(InstallJobEventKind::Interrupted {
            reason: InstallInterruptReason::AppClosed,
            phase: interrupted_phase,
        });
        job.state.progress.phase = InstallPhaseId::RollingBack;
        job.state.progress.progress = None;
        job.state.progress.details = InstallPhaseDetails::Empty;
        job.state.progress.parallel = None;
        job.state.error = Some(InstallErrorView::from_message(
            "app_closed",
            interrupted_phase,
            "App closed while install was running",
        ));

        job.state
            .record_event(InstallJobEventKind::RollbackStarted {
                cleanup: job.state.cleanup.clone(),
            });
        let cleanup_succeeded = match apply_cleanup(&mut job.state, state).await
        {
            Err(error) => {
                tracing::error!(
                    "Error cleaning up interrupted install job {}: {error}",
                    job.id
                );
                job.state.rollback_error = Some(InstallErrorView::from_error(
                    "rollback_error",
                    InstallPhaseId::RollingBack,
                    &error,
                    None,
                ));
                job.state.record_event(InstallJobEventKind::RollbackFailed {
                    message: error.to_string(),
                });
                false
            }
            Ok(()) => true,
        };
        finalize_rollback_state(&mut job.state, cleanup_succeeded);
        if cleanup_succeeded {
            clear_deleted_new_instance_id(&mut job.state);
        }

        let record = store::update_status(
            job.id,
            InstallJobStatus::Interrupted,
            &job.state,
            state,
        )
        .await?;
        emit_install_job(&record.snapshot()).await?;
    }

    Ok(())
}

pub(crate) fn finalize_rollback_state(
    job_state: &mut InstallJobState,
    cleanup_succeeded: bool,
) {
    if !cleanup_succeeded {
        return;
    }
    job_state.record_event(InstallJobEventKind::RollbackCompleted);
    job_state.progress.phase = InstallPhaseId::Finalizing;
    job_state.progress.progress = None;
    job_state.progress.details = InstallPhaseDetails::Empty;
    job_state.progress.parallel = None;
}

fn clear_deleted_new_instance_id(job_state: &mut InstallJobState) {
    if matches!(job_state.cleanup, InstallCleanup::DeleteNewInstance { .. }) {
        job_state.target = InstallTarget::NewInstance { instance_id: None };
        job_state.cleanup =
            InstallCleanup::DeleteNewInstance { instance_id: None };
    }
}

fn display_from_request(state: &InstallJobState) -> Option<InstallJobDisplay> {
    match &state.request {
        InstallRequest::CreateInstance { name, icon_path, .. } => {
            Some(InstallJobDisplay {
                title: name.clone(),
                icon: icon_path.clone(),
            })
        }
        InstallRequest::CreateModpackInstance { location, .. } => match location {
            crate::api::pack::install_from::CreatePackLocation::FromVersionId {
                title,
                icon_url,
                ..
            } => Some(InstallJobDisplay {
                title: title.clone(),
                icon: icon_url.clone(),
            }),
            crate::api::pack::install_from::CreatePackLocation::FromFile {
                ..
            } => None,
        },
        InstallRequest::ImportInstance {
            instance_folder, ..
        } => Some(InstallJobDisplay {
            title: instance_folder.clone(),
            icon: None,
        }),
        InstallRequest::DuplicateInstance { .. }
        | InstallRequest::InstallExistingInstance { .. }
        | InstallRequest::UpgradeUnmanagedInstance { .. }
        | InstallRequest::InstallPackToExistingInstance { .. }
        | InstallRequest::UpdateManagedCurseForgeModpack { .. } => {
            state.rollback.as_ref().map(|rollback| InstallJobDisplay {
                title: rollback.instance.instance.name.clone(),
                icon: rollback.instance.instance.icon_path.clone(),
            })
        }
        InstallRequest::InstallContent {
            display_title,
            display_icon,
            ..
        } => {
            Some(InstallJobDisplay {
                title: display_title.clone(),
                icon: display_icon.clone(),
            })
        }
        InstallRequest::InstallCurseForgeContent {
            display_title,
            display_icon,
            ..
        } => Some(InstallJobDisplay {
            title: display_title.clone(),
            icon: display_icon.clone(),
        }),
        InstallRequest::InstallCurseForgeWorld {
			display_title,
			display_icon,
			..
        } => Some(InstallJobDisplay {
            title: display_title.clone(),
            icon: display_icon.clone(),
        }),
        InstallRequest::InstallContentBatch {
            display_title,
            display_icon,
            ..
        }
        | InstallRequest::ChangeContent {
            display_title,
            display_icon,
            ..
        } => Some(InstallJobDisplay {
            title: display_title.clone(),
            icon: display_icon.clone(),
        }),
        InstallRequest::ExportResource { plan, .. } => Some(InstallJobDisplay { title: plan.package_info.name.clone(), icon: None }),
        InstallRequest::DownloadJava { vendor, version } => Some(InstallJobDisplay {
            title: format!("Java {version} ({vendor})"),
            icon: None,
        }),
    }
}

pub async fn apply_cleanup(
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<()> {
    if let InstallRequest::ExportResource {
        plan, output_path, ..
    } = &job_state.request
    {
        crate::resources::export::writer::cleanup_partial(plan, output_path)
            .await
            .map_err(|e| crate::ErrorKind::InputError(e.to_string()))?;
    }
    if let InstallRequest::InstallContentBatch { items, .. } =
        &job_state.request
    {
        for item in items {
            if let super::model::InstallContentBatchItem::FrozenResource {
                plan,
            } = item
            {
                Box::pin(crate::resources::plans::recover_frozen_plan(
                    plan, state,
                ))
                .await?;
            }
        }
    }
    match job_state.cleanup.clone() {
        InstallCleanup::DeleteNewInstance { instance_id } => {
            if let Some(instance_id) = instance_id
                && !job_state.instance_deleted()
            {
                let preserve_external_files = matches!(
                    &job_state.request,
                    InstallRequest::ImportInstance { symlink: true, .. }
                );
                if preserve_external_files {
                    crate::state::remove_instance_preserving_external_files(
                        &instance_id,
                        state,
                    )
                    .await?;
                } else {
                    crate::state::remove_instance(&instance_id, state).await?;
                }
                job_state.record_event(
                    InstallJobEventKind::TargetInstanceDeleted {
                        instance_id: instance_id.clone(),
                    },
                );
                if let Err(error) =
                    emit_instance(&instance_id, InstancePayloadType::Removed)
                        .await
                {
                    tracing::warn!(
                        instance_id,
                        error = %error,
                        "Install cleanup deleted a new instance, but its removal event could not be emitted"
                    );
                }
            }
        }
        InstallCleanup::RestoreExistingInstance { instance_id } => {
            if job_state.rollback.is_some() {
                restore_existing_instance(job_state, state, &instance_id)
                    .await?;
                if let Err(error) =
                    emit_instance(&instance_id, InstancePayloadType::Edited)
                        .await
                {
                    tracing::warn!(
                        instance_id,
                        error = %error,
                        "Install cleanup restored an instance, but its edited event could not be emitted"
                    );
                }
            }
        }
        InstallCleanup::None => {}
    }

    Ok(())
}

pub(crate) async fn prepare_existing_content_rollback(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    extra_relative_paths: Vec<String>,
) -> crate::Result<()> {
    prepare_existing_content_rollback_with_scope(
        job_id,
        job_state,
        state,
        extra_relative_paths,
        InstallContentRollbackScope::PackManaged,
    )
    .await
}

pub(crate) async fn prepare_existing_upgrade_content_rollback(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    replacement_paths: Vec<String>,
) -> crate::Result<()> {
    prepare_existing_content_rollback_with_scope(
        job_id,
        job_state,
        state,
        replacement_paths,
        InstallContentRollbackScope::AllContent,
    )
    .await
}

async fn prepare_existing_content_rollback_with_scope(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    extra_relative_paths: Vec<String>,
    scope: InstallContentRollbackScope,
) -> crate::Result<()> {
    if job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
        .is_some()
    {
        return Ok(());
    }

    let rollback = job_state.rollback.as_ref().ok_or_else(|| {
        crate::ErrorKind::OtherError(
            "Existing instance rollback metadata is missing".to_string(),
        )
    })?;
    let instance_id = rollback.instance.instance.id.clone();
    let content_set_id = rollback.instance.applied_content_set.id.clone();
    let instance_base = state
        .directories
        .instance_game_dir(&rollback.instance.instance);
    let _instance_lock = state.lock_instance_content(&instance_id).await;

    let all_entries =
        content_rows::get_content_entries(&content_set_id, &state.pool).await?;
    let selected_entries = all_entries
        .into_iter()
        .filter(|entry| {
            scope == InstallContentRollbackScope::AllContent
                || entry.ownership_kind == ContentOwnershipKind::PackManaged
        })
        .collect::<Vec<_>>();
    let instance_files =
        content_rows::get_instance_files(&instance_id, &state.pool).await?;
    let files_by_id = instance_files
        .iter()
        .map(|file| (file.id.clone(), file.clone()))
        .collect::<HashMap<_, _>>();
    let user_owned_paths = if scope == InstallContentRollbackScope::PackManaged
    {
        user_owned_paths(
            &selected_entries,
            &instance_files,
            &content_set_id,
            &state.pool,
        )
        .await?
    } else {
        HashSet::new()
    };

    let mut entry_snapshots = Vec::with_capacity(selected_entries.len());
    for entry in &selected_entries {
        let provider_refs =
            content_rows::get_content_provider_refs_with_origin(
                &entry.id,
                &state.pool,
            )
            .await?
            .into_iter()
            .map(|(provider_ref, origin)| InstallRollbackProviderRef {
                provider_ref,
                origin,
            })
            .collect();
        let update_check =
            content_rows::get_content_update_check(&entry.id, &state.pool)
                .await?;
        entry_snapshots.push(InstallRollbackContentEntry {
            entry: entry.clone(),
            provider_refs,
            update_check,
        });
    }

    let mut paths = selected_entries
        .iter()
        .filter_map(|entry| entry.file_id.as_ref())
        .filter_map(|file_id| files_by_id.get(file_id))
        .map(|file| file.relative_path.clone())
        .chain(extra_relative_paths)
        .filter(|path| !user_owned_paths.contains(path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    let mut file_snapshots = Vec::with_capacity(paths.len());
    let mut protected_paths = Vec::new();
    for (index, relative_path) in paths.into_iter().enumerate() {
        let source = checked_instance_path(&instance_base, &relative_path)?;
        let instance_file = instance_files
            .iter()
            .find(|file| file.relative_path == relative_path)
            .cloned();
        let metadata = match tokio::fs::symlink_metadata(&source).await {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(metadata) = &metadata
            && (!metadata.is_file()
                || crate::util::io::is_symlink_or_reparse(metadata))
        {
            if scope == InstallContentRollbackScope::AllContent
                && crate::util::io::is_symlink_or_reparse(metadata)
            {
                protected_paths.push(relative_path);
                continue;
            }
            return Err(crate::ErrorKind::FSError(format!(
                "Rollback source is not a regular managed file: {relative_path}"
            ))
            .into());
        }
        let (physical_sha1, physical_size) = if metadata.is_some() {
            let (size, sha1) =
                crate::util::fetch::sha1_file_async(&source).await?;
            (Some(sha1), Some(size))
        } else {
            (None, None)
        };
        file_snapshots.push(InstallRollbackFile {
            relative_path,
            staged_name: format!("{index:08}.file"),
            existed: metadata.is_some(),
            physical_sha1,
            physical_size,
            instance_file,
        });
    }

    let snapshot = InstallContentRollbackSnapshot {
        staging_id: job_id.to_string(),
        stage: InstallContentRollbackStage::Planned,
        scope,
        files: file_snapshots,
        entries: entry_snapshots,
        dependency_edges: content_rows::get_content_dependency_edges(
            &content_set_id,
            &state.pool,
        )
        .await?,
        pack_members: content_rows::get_pack_members(
            &content_set_id,
            &state.pool,
        )
        .await?,
        replacement_paths: Vec::new(),
        protected_paths,
    };
    job_state.rollback.as_mut().unwrap().content = Some(snapshot);
    super::events::InstallProgressReporter::new(job_id, job_state.clone())
        .set_rollback(job_state.rollback.clone())
        .await?;

    let staging_root = staging_root(job_state, state)?;
    crate::util::io::create_dir_all(staging_root.join("files")).await?;
    let stage_result =
        stage_snapshot_files(job_state, &instance_base, state).await;
    if let Err(error) = stage_result {
        let restore_result =
            restore_snapshot_files(job_state, &instance_base, state).await;
        if let Err(restore_error) = restore_result {
            return Err(crate::ErrorKind::OtherError(format!(
                "Failed to stage existing pack content: {error}; partial staging restore failed: {restore_error}"
            ))
            .into());
        }
        discard_content_rollback(job_state, state).await?;
        super::events::InstallProgressReporter::new(job_id, job_state.clone())
            .set_rollback(job_state.rollback.clone())
            .await?;
        return Err(error);
    }

    job_state
        .rollback
        .as_mut()
        .unwrap()
        .content
        .as_mut()
        .unwrap()
        .stage = InstallContentRollbackStage::Ready;
    super::events::InstallProgressReporter::new(job_id, job_state.clone())
        .set_rollback(job_state.rollback.clone())
        .await?;
    remove_snapshotted_db_state(job_state, state).await?;
    Ok(())
}

pub(crate) async fn discard_content_rollback(
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<()> {
    let Some(snapshot) = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
    else {
        return Ok(());
    };
    let staging = staging_root_from_id(&snapshot.staging_id, state)?;
    match crate::util::io::remove_dir_all(&staging).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(rollback) = job_state.rollback.as_mut() {
        rollback.content = None;
    }
    Ok(())
}

pub(crate) async fn restore_upgrade_db_baseline(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<()> {
    let scope = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
        .map(|snapshot| snapshot.scope);
    if scope != Some(InstallContentRollbackScope::AllContent) {
        return Err(crate::ErrorKind::OtherError(
            "Upgrade rollback snapshot is missing".to_string(),
        )
        .into());
    }
    restore_snapshotted_db_state(job_state, state).await
}

async fn restore_existing_instance(
    job_state: &mut InstallJobState,
    state: &State,
    instance_id: &str,
) -> crate::Result<()> {
    let rollback = job_state.rollback.as_ref().ok_or_else(|| {
        crate::ErrorKind::OtherError(
            "Existing instance rollback metadata is missing".to_string(),
        )
    })?;
    if rollback.instance.instance.id != instance_id {
        return Err(crate::ErrorKind::InputError(
            "Rollback instance does not match cleanup target".to_string(),
        )
        .into());
    }
    let instance_base = state
        .directories
        .instance_game_dir(&rollback.instance.instance);
    let _instance_lock = state.lock_instance_content(instance_id).await;

    let recovery_root = instance_base.clone();
    tokio::task::spawn_blocking(move || {
        crate::api::pack::archive_util::recover_interrupted_archive_replacements(
            &recovery_root,
        )
    })
    .await??;

    if rollback.content.is_some() {
        clean_replacement_files(job_state, &instance_base, state).await?;
        restore_snapshot_files(job_state, &instance_base, state).await?;
        restore_snapshotted_db_state(job_state, state).await?;
    }
    let metadata = job_state.rollback.as_ref().unwrap().instance.clone();
    crate::state::restore_instance_metadata(&metadata, &state.pool).await?;
    verify_restored_snapshot(job_state, &instance_base, state).await?;
    discard_content_rollback(job_state, state).await?;
    Ok(())
}

async fn user_owned_paths(
    managed_entries: &[crate::state::ContentEntry],
    files: &[crate::state::InstanceFile],
    content_set_id: &str,
    pool: &sqlx::SqlitePool,
) -> crate::Result<HashSet<String>> {
    let managed_ids = managed_entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let file_paths = files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.clone()))
        .collect::<HashMap<_, _>>();
    Ok(content_rows::get_content_entries(content_set_id, pool)
        .await?
        .into_iter()
        .filter(|entry| !managed_ids.contains(entry.id.as_str()))
        .filter_map(|entry| entry.file_id)
        .filter_map(|file_id| file_paths.get(file_id.as_str()).cloned())
        .collect())
}

pub(crate) fn checked_instance_path(
    instance_base: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    let safe =
        SafeRelativeUtf8UnixPathBuf::try_from(relative_path.to_string())?;
    let mut current = instance_base.to_path_buf();
    for component in safe.as_str().split('/') {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata)
                if crate::util::io::is_symlink_or_reparse(&metadata) =>
            {
                return Err(crate::ErrorKind::FSError(format!(
                    "Instance path crosses a symlink or reparse point: {relative_path}"
                ))
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(current)
}

fn staging_root(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<PathBuf> {
    let snapshot = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Content rollback snapshot is missing".to_string(),
            )
        })?;
    staging_root_from_id(&snapshot.staging_id, state)
}

fn staging_root_from_id(
    staging_id: &str,
    state: &State,
) -> crate::Result<PathBuf> {
    let id = Uuid::parse_str(staging_id).map_err(|_| {
        crate::ErrorKind::InputError(
            "Invalid persisted rollback staging ID".to_string(),
        )
    })?;
    Ok(state
        .directories
        .install_rollbacks_dir()
        .join(id.to_string()))
}

async fn stage_snapshot_files(
    job_state: &InstallJobState,
    instance_base: &Path,
    state: &State,
) -> crate::Result<()> {
    let staging = staging_root(job_state, state)?.join("files");
    let snapshot = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
        .unwrap();
    for file in snapshot.files.iter().filter(|file| file.existed) {
        let source = checked_instance_path(instance_base, &file.relative_path)?;
        let target = staging.join(&file.staged_name);
        if target.exists() {
            verify_file(&target, file).await?;
            if source.exists()
                && snapshot.scope == InstallContentRollbackScope::PackManaged
            {
                verify_file(&source, file).await?;
                crate::util::io::remove_file(&source).await?;
            }
            continue;
        }
        if snapshot.scope == InstallContentRollbackScope::AllContent {
            crate::util::io::copy(&source, &target).await?;
            verify_file(&target, file).await?;
        } else {
            move_file_verified(&source, &target, file).await?;
        }
    }
    Ok(())
}

async fn restore_snapshot_files(
    job_state: &InstallJobState,
    instance_base: &Path,
    state: &State,
) -> crate::Result<()> {
    let staging = staging_root(job_state, state)?.join("files");
    let snapshot = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
        .unwrap();
    for file in snapshot.files.iter().filter(|file| file.existed) {
        let source = staging.join(&file.staged_name);
        let target = checked_instance_path(instance_base, &file.relative_path)?;
        if target.exists() {
            if verify_file(&target, file).await.is_ok() {
                if source.exists() {
                    crate::util::io::remove_file(&source).await?;
                }
                continue;
            }
            crate::util::io::remove_file(&target).await?;
        }
        if !source.exists() {
            return Err(crate::ErrorKind::FSError(format!(
                "Rollback staged file is missing: {}",
                file.relative_path
            ))
            .into());
        }
        if let Some(parent) = target.parent() {
            crate::util::io::create_dir_all(parent).await?;
        }
        move_file_verified(&source, &target, file).await?;
    }
    Ok(())
}

async fn move_file_verified(
    source: &Path,
    target: &Path,
    expected: &InstallRollbackFile,
) -> crate::Result<()> {
    if let Some(parent) = target.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    match tokio::fs::rename(source, target).await {
        Ok(()) => verify_file(target, expected).await,
        Err(_) => {
            crate::util::io::copy(source, target).await?;
            if let Err(error) = verify_file(target, expected).await {
                let _ = crate::util::io::remove_file(target).await;
                return Err(error);
            }
            crate::util::io::remove_file(source).await?;
            Ok(())
        }
    }
}

async fn verify_file(
    path: &Path,
    expected: &InstallRollbackFile,
) -> crate::Result<()> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.is_file()
        || crate::util::io::is_symlink_or_reparse(&metadata)
        || expected.physical_size != Some(metadata.len())
    {
        return Err(crate::ErrorKind::FSError(format!(
            "Rollback file integrity check failed: {}",
            expected.relative_path
        ))
        .into());
    }
    let (_, actual_sha1) = crate::util::fetch::sha1_file_async(path).await?;
    if expected.physical_sha1.as_deref() != Some(actual_sha1.as_str()) {
        return Err(crate::ErrorKind::FSError(format!(
            "Rollback file integrity check failed: {}",
            expected.relative_path
        ))
        .into());
    }
    Ok(())
}

async fn remove_snapshotted_db_state(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<()> {
    let rollback = job_state.rollback.as_ref().unwrap();
    let snapshot = rollback.content.as_ref().unwrap();
    let content_set_id = &rollback.instance.applied_content_set.id;
    let instance_id = &rollback.instance.instance.id;
    let file_ids = snapshot
        .files
        .iter()
        .filter_map(|file| file.instance_file.as_ref())
        .map(|file| file.id.clone())
        .collect::<Vec<_>>();
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM instance_pack_members WHERE content_set_id = ?")
        .bind(content_set_id)
        .execute(&mut *tx)
        .await?;
    match snapshot.scope {
        InstallContentRollbackScope::PackManaged => {
            sqlx::query(
                "DELETE FROM instance_content_entries
                 WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
            )
            .bind(content_set_id)
            .execute(&mut *tx)
            .await?;
        }
        InstallContentRollbackScope::AllContent => {
            sqlx::query(
                "DELETE FROM instance_content_entries WHERE content_set_id = ?",
            )
            .bind(content_set_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    for file_id in file_ids {
        sqlx::query(
            "DELETE FROM instance_files
             WHERE instance_id = ? AND id = ?
               AND NOT EXISTS (
                   SELECT 1 FROM instance_content_entries
                   WHERE file_id = ?
               )",
        )
        .bind(instance_id)
        .bind(&file_id)
        .bind(&file_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn clean_replacement_files(
    job_state: &InstallJobState,
    instance_base: &Path,
    state: &State,
) -> crate::Result<()> {
    let rollback = job_state.rollback.as_ref().unwrap();
    let snapshot = rollback.content.as_ref().unwrap();
    let current_entries = content_rows::get_content_entries(
        &rollback.instance.applied_content_set.id,
        &state.pool,
    )
    .await?;
    let current_files = content_rows::get_instance_files(
        &rollback.instance.instance.id,
        &state.pool,
    )
    .await?;
    let paths_by_id = current_files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let rollback_paths = current_entries
        .iter()
        .filter(|entry| {
            snapshot.scope == InstallContentRollbackScope::AllContent
                || entry.ownership_kind == ContentOwnershipKind::PackManaged
        })
        .filter_map(|entry| entry.file_id.as_deref())
        .filter_map(|id| paths_by_id.get(id).copied())
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    let protected_paths = current_entries
        .iter()
        .filter(|entry| {
            snapshot.scope == InstallContentRollbackScope::PackManaged
                && entry.ownership_kind != ContentOwnershipKind::PackManaged
        })
        .filter_map(|entry| entry.file_id.as_deref())
        .filter_map(|id| paths_by_id.get(id).copied())
        .collect::<HashSet<_>>();
    let old_paths = snapshot
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    let cleanup_paths = rollback_paths
        .into_iter()
        .chain(snapshot.replacement_paths.iter().cloned())
        .collect::<HashSet<_>>();
    for relative_path in cleanup_paths {
        if snapshot.protected_paths.contains(&relative_path) {
            continue;
        }
        if protected_paths.contains(relative_path.as_str()) {
            return Err(crate::ErrorKind::FSError(format!(
                "Refusing to remove user-owned rollback path: {relative_path}"
            ))
            .into());
        }
        if old_paths.contains(relative_path.as_str()) {
            continue;
        }
        let path = checked_instance_path(instance_base, &relative_path)?;
        match crate::util::io::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn restore_snapshotted_db_state(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<()> {
    let rollback = job_state.rollback.as_ref().unwrap();
    let snapshot = rollback.content.as_ref().unwrap();
    let content_set_id = &rollback.instance.applied_content_set.id;
    let instance_id = &rollback.instance.instance.id;
    let current_entries =
        content_rows::get_content_entries(content_set_id, &state.pool).await?;
    let current_file_ids = current_entries
        .iter()
        .filter(|entry| {
            snapshot.scope == InstallContentRollbackScope::AllContent
                || entry.ownership_kind == ContentOwnershipKind::PackManaged
        })
        .filter_map(|entry| entry.file_id.clone())
        .collect::<Vec<_>>();

    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM instance_pack_members WHERE content_set_id = ?")
        .bind(content_set_id)
        .execute(&mut *tx)
        .await?;
    match snapshot.scope {
        InstallContentRollbackScope::PackManaged => {
            sqlx::query(
                "DELETE FROM instance_content_entries
                 WHERE content_set_id = ? AND ownership_kind = 'pack_managed'",
            )
            .bind(content_set_id)
            .execute(&mut *tx)
            .await?;
        }
        InstallContentRollbackScope::AllContent => {
            sqlx::query(
                "DELETE FROM instance_content_entries WHERE content_set_id = ?",
            )
            .bind(content_set_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    for file_id in current_file_ids {
        sqlx::query(
            "DELETE FROM instance_files
             WHERE instance_id = ? AND id = ?
               AND NOT EXISTS (
                   SELECT 1 FROM instance_content_entries WHERE file_id = ?
               )",
        )
        .bind(instance_id)
        .bind(&file_id)
        .bind(&file_id)
        .execute(&mut *tx)
        .await?;
    }
    for file in snapshot
        .files
        .iter()
        .filter_map(|file| file.instance_file.as_ref())
    {
        content_rows::upsert_instance_file(file, &mut tx).await?;
    }
    for entry in &snapshot.entries {
        content_rows::restore_content_entry_in_transaction(
            &entry.entry,
            &mut tx,
        )
        .await?;
        for provider in &entry.provider_refs {
            content_rows::upsert_content_provider_ref_in_transaction(
                &entry.entry.id,
                &provider.provider_ref,
                provider.origin,
                &mut tx,
            )
            .await?;
        }
        if let Some(check) = &entry.update_check {
            content_rows::restore_content_update_check_in_transaction(
                check, &mut tx,
            )
            .await?;
        }
    }
    for edge in &snapshot.dependency_edges {
        content_rows::upsert_content_dependency_edge_in_transaction(
            edge, &mut tx,
        )
        .await?;
    }
    for member in &snapshot.pack_members {
        content_rows::upsert_pack_member_in_transaction(member, &mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn verify_restored_snapshot(
    job_state: &InstallJobState,
    instance_base: &Path,
    state: &State,
) -> crate::Result<()> {
    let Some(snapshot) = job_state
        .rollback
        .as_ref()
        .and_then(|rollback| rollback.content.as_ref())
    else {
        return Ok(());
    };
    for file in snapshot.files.iter().filter(|file| file.existed) {
        let path = checked_instance_path(instance_base, &file.relative_path)?;
        verify_file(&path, file).await?;
    }
    let rollback = job_state.rollback.as_ref().unwrap();
    let entries = content_rows::get_content_entries(
        &rollback.instance.applied_content_set.id,
        &state.pool,
    )
    .await?;
    let entry_ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    if snapshot
        .entries
        .iter()
        .any(|entry| !entry_ids.contains(entry.entry.id.as_str()))
    {
        return Err(crate::ErrorKind::OtherError(
            "Rollback DB verification found missing content entries"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

#[cfg(all(test, not(feature = "tauri")))]
mod tests {
    use super::*;
    use crate::api::pack::install_from::CreatePackLocation;
    use crate::install::model::{InstallRollbackState, InstallTarget};
    use crate::state::{
        AppliedContentSetPatch, ContentProviderRef, ContentSourceKind,
        EditInstance, InstanceInstallStage, InstanceLink, ModLoader,
        ModrinthProjectId, ModrinthVersionId, ProjectType,
    };

    #[tokio::test]
    async fn b10_restores_existing_pack_files_and_database_state() {
        crate::event::EventState::init().await.unwrap();
        let root = tempfile::tempdir().unwrap().keep();
        let state = State::init_for_test(root.to_string_lossy().to_string())
            .await
            .unwrap();
        let instance_count_before = crate::state::list_instances(&state.pool)
            .await
            .unwrap()
            .len();
        let old_link = InstanceLink::ImportedModpack {
            project_id: Some("old-project".to_string()),
            version_id: Some("old-version".to_string()),
            name: Some("Old Pack".to_string()),
            version_number: Some("1.0.0".to_string()),
            filename: Some("old.mrpack".to_string()),
        };
        let created = crate::api::instance::create(
            "B10 Existing".to_string(),
            "1.20.1".to_string(),
            ModLoader::Vanilla,
            None,
            None,
            old_link.clone(),
            None,
            None,
        )
        .await
        .unwrap();
        let instance_id = created.instance.id.clone();
        crate::state::instances::commands::set_instance_install_stage(
            &instance_id,
            InstanceInstallStage::Installed,
            &state.pool,
        )
        .await
        .unwrap();
        let instance_base = state
            .directories
            .instances_dir()
            .join(&created.instance.path);
        crate::util::io::create_dir_all(instance_base.join("mods"))
            .await
            .unwrap();

        let old_files = [
            ("mods/old-a.jar", b"old-a-content".as_slice()),
            ("mods/old-b.jar", b"old-b-content".as_slice()),
        ];
        for (index, (relative_path, bytes)) in old_files.iter().enumerate() {
            let path = instance_base.join(relative_path);
            crate::util::io::write(&path, bytes).await.unwrap();
            let (_, sha1) =
                crate::util::fetch::sha1_file_async(&path).await.unwrap();
            let provider = ContentProviderRef::Modrinth {
                project_id: ModrinthProjectId::new(format!("project-{index}"))
                    .unwrap(),
                version_id: Some(
                    ModrinthVersionId::new(format!("version-{index}")).unwrap(),
                ),
            };
            crate::state::record_project_file_atomic(
                &instance_id,
                relative_path,
                &sha1,
                bytes.len() as u64,
                ProjectType::Mod,
                ContentSourceKind::ImportedModpack,
                ContentOwnershipKind::PackManaged,
                Some(&provider),
                true,
                None,
                &state,
            )
            .await
            .unwrap();
        }
        crate::util::io::write(
            instance_base.join("B10_KEEP_ME.txt"),
            b"user-data",
        )
        .await
        .unwrap();
        let user_mod_path = instance_base.join("mods/user-added.jar");
        crate::util::io::write(&user_mod_path, b"user-mod")
            .await
            .unwrap();
        let (_, user_mod_sha1) =
            crate::util::fetch::sha1_file_async(&user_mod_path)
                .await
                .unwrap();
        crate::state::record_project_file_atomic(
            &instance_id,
            "mods/user-added.jar",
            &user_mod_sha1,
            8,
            ProjectType::Mod,
            ContentSourceKind::Local,
            ContentOwnershipKind::UserAdded,
            None,
            false,
            None,
            &state,
        )
        .await
        .unwrap();

        let original = crate::state::get_instance(&instance_id, &state.pool)
            .await
            .unwrap()
            .unwrap();
        let original_entries = content_rows::get_content_entries(
            &original.applied_content_set.id,
            &state.pool,
        )
        .await
        .unwrap();
        let original_members = content_rows::get_pack_members(
            &original.applied_content_set.id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(
            original_entries
                .iter()
                .filter(|entry| {
                    entry.ownership_kind == ContentOwnershipKind::PackManaged
                })
                .count(),
            2
        );
        assert_eq!(original_members.len(), 2);

        let request = InstallRequest::InstallPackToExistingInstance {
            instance_id: instance_id.clone(),
            location: CreatePackLocation::FromFile {
                path: root.join("new.mrpack"),
            },
            post_install_edit: None,
        };
        let mut job_state = InstallJobState::new(request);
        job_state.rollback = Some(InstallRollbackState {
            instance: original.clone(),
            install_stage: InstanceInstallStage::Installed,
            content: None,
        });
        job_state.cleanup = InstallCleanup::RestoreExistingInstance {
            instance_id: instance_id.clone(),
        };
        assert!(matches!(
            job_state.target,
            InstallTarget::ExistingInstance { instance_id: ref target }
                if target == &instance_id
        ));
        let job_id = Uuid::new_v4();
        store::insert(job_id, &job_state, InstallJobStatus::Queued, &state)
            .await
            .unwrap();

        prepare_existing_content_rollback(
            job_id,
            &mut job_state,
            &state,
            Vec::new(),
        )
        .await
        .unwrap();
        job_state = store::get_required(job_id, &state).await.unwrap().state;
        let staging_id = job_state
            .rollback
            .as_ref()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .staging_id
            .clone();
        assert!(!instance_base.join("mods/old-a.jar").exists());
        assert!(!instance_base.join("mods/old-b.jar").exists());
        assert!(instance_base.join("B10_KEEP_ME.txt").exists());
        let entries_after_remove = content_rows::get_content_entries(
            &original.applied_content_set.id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(entries_after_remove.len(), 1);
        assert_eq!(
            entries_after_remove[0].ownership_kind,
            ContentOwnershipKind::UserAdded
        );

        let new_path = instance_base.join("mods/new-pack.jar");
        crate::util::io::write(&new_path, b"new-pack-partial")
            .await
            .unwrap();
        let (_, new_sha1) = crate::util::fetch::sha1_file_async(&new_path)
            .await
            .unwrap();
        crate::state::record_project_file_atomic(
            &instance_id,
            "mods/new-pack.jar",
            &new_sha1,
            16,
            ProjectType::Mod,
            ContentSourceKind::ImportedModpack,
            ContentOwnershipKind::PackManaged,
            None,
            false,
            None,
            &state,
        )
        .await
        .unwrap();
        crate::api::instance::edit(
            &instance_id,
            EditInstance {
                install_stage: Some(InstanceInstallStage::PackInstalling),
                name: Some("New Partial Pack".to_string()),
                link: Some(InstanceLink::ImportedModpack {
                    project_id: None,
                    version_id: None,
                    name: Some("New Pack".to_string()),
                    version_number: Some("2.0.0".to_string()),
                    filename: Some("new.mrpack".to_string()),
                }),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: Some(ContentSourceKind::ImportedModpack),
                    game_version: Some("1.21.1".to_string()),
                    protocol_version: Some(None),
                    loader: Some(ModLoader::Vanilla),
                    loader_version: Some(None),
                }),
                ..EditInstance::default()
            },
        )
        .await
        .unwrap();
        let untracked_new_path = instance_base.join("config/new-pack.cfg");
        crate::util::io::create_dir_all(untracked_new_path.parent().unwrap())
            .await
            .unwrap();
        crate::util::io::write(&untracked_new_path, b"partial-config")
            .await
            .unwrap();
        job_state
            .rollback
            .as_mut()
            .unwrap()
            .content
            .as_mut()
            .unwrap()
            .replacement_paths
            .push("config/new-pack.cfg".to_string());

        apply_cleanup(&mut job_state, &state).await.unwrap();
        apply_cleanup(&mut job_state, &state).await.unwrap();
        store::update_status(
            job_id,
            InstallJobStatus::Canceled,
            &job_state,
            &state,
        )
        .await
        .unwrap();

        assert_eq!(
            crate::util::io::read(instance_base.join("mods/old-a.jar"))
                .await
                .unwrap(),
            b"old-a-content"
        );
        assert_eq!(
            crate::util::io::read(instance_base.join("mods/old-b.jar"))
                .await
                .unwrap(),
            b"old-b-content"
        );
        assert!(!new_path.exists());
        assert!(!untracked_new_path.exists());
        assert_eq!(
            crate::util::io::read(instance_base.join("B10_KEEP_ME.txt"))
                .await
                .unwrap(),
            b"user-data"
        );
        assert_eq!(
            crate::util::io::read(&user_mod_path).await.unwrap(),
            b"user-mod"
        );
        let restored = crate::state::get_instance(&instance_id, &state.pool)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.instance.id, instance_id);
        assert_eq!(
            restored.instance.install_stage,
            InstanceInstallStage::Installed
        );
        assert_eq!(restored.instance.name, original.instance.name);
        assert_eq!(
            serde_json::to_value(&restored.link).unwrap(),
            serde_json::to_value(&old_link).unwrap()
        );
        assert_eq!(
            restored.applied_content_set.game_version,
            original.applied_content_set.game_version
        );
        let restored_entries = content_rows::get_content_entries(
            &original.applied_content_set.id,
            &state.pool,
        )
        .await
        .unwrap();
        assert_eq!(restored_entries.len(), 3);
        let restored_pack_entries = restored_entries
            .iter()
            .filter(|entry| {
                entry.ownership_kind == ContentOwnershipKind::PackManaged
            })
            .collect::<Vec<_>>();
        assert_eq!(restored_pack_entries.len(), 2);
        assert!(restored_pack_entries.iter().all(|entry| {
            entry.source_kind == ContentSourceKind::ImportedModpack
        }));
        for entry in restored_pack_entries {
            let refs = content_rows::get_content_provider_refs_with_origin(
                &entry.id,
                &state.pool,
            )
            .await
            .unwrap();
            assert_eq!(refs.len(), 1);
            assert!(refs[0].1);
        }
        assert_eq!(
            content_rows::get_pack_members(
                &original.applied_content_set.id,
                &state.pool,
            )
            .await
            .unwrap()
            .len(),
            original_members.len()
        );
        assert!(
            !state
                .directories
                .install_rollbacks_dir()
                .join(staging_id)
                .exists()
        );
        assert!(job_state.rollback.as_ref().unwrap().content.is_none());
        let persisted = store::get_required(job_id, &state).await.unwrap();
        assert_eq!(persisted.status, InstallJobStatus::Canceled);
        assert!(persisted.state.rollback.as_ref().unwrap().content.is_none());
        assert!(persisted.state.rollback_error.is_none());
        assert_eq!(
            crate::state::list_instances(&state.pool)
                .await
                .unwrap()
                .len(),
            instance_count_before + 1
        );

        let new_instance = crate::api::instance::create(
            "Canceled New Instance".to_string(),
            "1.21.1".to_string(),
            ModLoader::Vanilla,
            None,
            None,
            InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();
        let new_instance_id = new_instance.instance.id.clone();
        let new_instance_base = state
            .directories
            .instances_dir()
            .join(&new_instance.instance.path);
        crate::util::io::create_dir_all(new_instance_base.join("mods"))
            .await
            .unwrap();
        crate::util::io::write(
            new_instance_base.join("mods/partial.jar"),
            b"partial install",
        )
        .await
        .unwrap();
        let mut new_instance_job =
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".to_string(),
                version: 21,
            });
        new_instance_job.cleanup = InstallCleanup::DeleteNewInstance {
            instance_id: Some(new_instance_id.clone()),
        };
        let cancellation = tokio_util::sync::CancellationToken::new();
        let writer_cancellation = cancellation.clone();
        let writer_path = new_instance_base.join("config/late-write.cfg");
        let writer_lock = state
            .lock_instance_content_exclusive(&new_instance_id)
            .await;
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let writer = tokio::task::spawn_blocking(move || {
            let _writer_lock = writer_lock;
            let _ = started_tx.send(());
            while !writer_cancellation.is_cancelled() {
                std::thread::yield_now();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::fs::create_dir_all(writer_path.parent().unwrap()).unwrap();
            std::fs::write(writer_path, b"late write").unwrap();
            let _ = done_tx.send(());
        });
        started_rx.await.unwrap();
        cancellation.cancel();
        drop(writer);
        apply_cleanup(&mut new_instance_job, &state).await.unwrap();
        done_rx.await.unwrap();
        assert!(new_instance_job.instance_deleted());
        assert!(!new_instance_base.exists());
        assert!(
            crate::state::get_instance(&new_instance_id, &state.pool)
                .await
                .unwrap()
                .is_none()
        );

        let external_instance =
            root.join("external-launcher/versions/source-instance");
        crate::util::io::create_dir_all(&external_instance)
            .await
            .unwrap();
        let external_marker = external_instance.join("original-world.dat");
        crate::util::io::write(&external_marker, b"must survive cancellation")
            .await
            .unwrap();
        let linked_import = crate::api::instance::create(
            "Canceled Symlink Import".to_string(),
            "1.21.1".to_string(),
            ModLoader::Vanilla,
            None,
            None,
            InstanceLink::Unmanaged,
            None,
            Some(external_instance.to_string_lossy().to_string()),
        )
        .await
        .unwrap();
        let linked_import_id = linked_import.instance.id.clone();
        let managed_import_path = state
            .directories
            .instances_dir()
            .join(&linked_import.instance.path);
        let mut linked_import_job =
            InstallJobState::new(InstallRequest::ImportInstance {
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: root.clone(),
                instance_folder: "source-instance".to_string(),
                instance_path: Some(
                    external_instance.to_string_lossy().to_string(),
                ),
                symlink: true,
                game_version: None,
                loader: None,
                loader_version: None,
                game_dir_override: Some(
                    external_instance.to_string_lossy().to_string(),
                ),
            });
        linked_import_job.cleanup = InstallCleanup::DeleteNewInstance {
            instance_id: Some(linked_import_id.clone()),
        };

        apply_cleanup(&mut linked_import_job, &state).await.unwrap();

        assert!(external_marker.exists());
        assert!(!managed_import_path.exists());
        assert!(
            crate::state::get_instance(&linked_import_id, &state.pool)
                .await
                .unwrap()
                .is_none()
        );

        let mut missing_instance_job =
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".to_string(),
                version: 21,
            });
        missing_instance_job.cleanup = InstallCleanup::DeleteNewInstance {
            instance_id: Some("missing-instance".to_string()),
        };
        apply_cleanup(&mut missing_instance_job, &state)
            .await
            .unwrap();
        assert!(
            crate::state::get_instance("missing-instance", &state.pool)
                .await
                .unwrap()
                .is_none()
        );
    }
}
