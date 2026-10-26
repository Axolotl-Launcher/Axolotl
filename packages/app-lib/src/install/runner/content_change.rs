use super::*;
use crate::util::download::constants::INSTALL_FILE_REPORTED_ATTEMPTS;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone)]
enum DownloadedContent {
    Modrinth(crate::state::instances::commands::DownloadedProjectVersion),
    CurseForge(crate::api::curseforge::StagedCurseForgeUpgrade),
}

pub(super) async fn run(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    instance_id: &str,
    intent: &crate::install::ContentChangeIntent,
) -> crate::Result<()> {
    let state = State::get().await?;
    let reporter = InstallProgressReporter::new(job_id, job_state.clone());
    let cancellation = reporter.cancellation_token();
    let mut actions = cancelable(
        &cancellation,
        load_or_resolve_actions(job_state, instance_id, intent, &reporter),
    )
    .await?;
    cancelable(
        &cancellation,
        prepare_actions(
            job_state,
            instance_id,
            &mut actions,
            &reporter,
            &state,
            &cancellation,
        ),
    )
    .await?;

    let downloaded = download_prepared_files(
        job_state,
        instance_id,
        &mut actions,
        &reporter,
        &state,
        &cancellation,
    )
    .await?;
    publish_actions(
        job_state,
        instance_id,
        &mut actions,
        &reporter,
        &cancellation,
        &downloaded,
        &state,
    )
    .await?;

    let failures = actions
        .iter()
        .filter(|action| {
            action.effective_status()
                == crate::install::ContentChangeActionStatus::Failed
        })
        .count();
    if failures > 0 {
        return Err(crate::ErrorKind::OtherError(format!(
            "{failures} content change(s) failed"
        ))
        .into());
    }
    Ok(())
}

async fn prepare_actions(
    job_state: &mut InstallJobState,
    instance_id: &str,
    actions: &mut [crate::install::ContentChangeAction],
    reporter: &InstallProgressReporter,
    state: &std::sync::Arc<State>,
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<()> {
    let pending = actions
        .iter()
        .filter(|action| needs_preparation(action))
        .count() as u64;
    reporter
        .update(
            InstallPhaseId::StagingContent,
            Some(InstallProgress {
                current: 0,
                total: pending.max(1),
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;

    let concurrency = state.download_concurrency().max(1);
    let mut completed = 0_u64;
    {
        let pending_actions = actions
            .iter()
            .enumerate()
            .filter(|(_, action)| needs_preparation(action))
            .map(|(index, action)| (index, action.clone()))
            .collect::<Vec<_>>();
        let mut preparations = spawn_bounded(pending_actions.into_iter().map(
            |(index, mut action)| {
                let cancellation = cancellation.clone();
                let instance_id = instance_id.to_string();
                let state = state.clone();
                async move {
                    check_canceled(&cancellation)?;
                    action.set_status(
                        crate::install::ContentChangeActionStatus::Pending,
                    );
                    let result = match action.provider {
                        ContentProvider::Modrinth => {
                            crate::state::instances::commands::prepare_modrinth_content_change_action(
                                &instance_id,
                                &mut action,
                                &state,
                            )
                            .await
                        }
                        ContentProvider::CurseForge => {
                            crate::api::curseforge::prepare_curseforge_content_change_action(
                                &instance_id,
                                &mut action,
                            )
                            .await
                        }
                        provider => Err(crate::ErrorKind::InputError(format!(
                            "Provider {} does not support content changes",
                            provider.as_str()
                        ))
                        .into()),
                    };
                    if let Err(error) = result {
                        action.files.clear();
                        action.modrinth_plan = None;
                        action.status =
                            crate::install::ContentChangeActionStatus::Failed;
                        action.error = Some(error.to_string());
                    }
                    Ok::<_, crate::Error>((index, action))
                }
            },
        ), concurrency);

        while let Some(result) = preparations.join_next().await {
            let (index, action) = result.map_err(|error| {
                crate::ErrorKind::OtherError(error.to_string())
            })??;
            actions[index] = action;
            completed += 1;
            reporter
                .update(
                    InstallPhaseId::StagingContent,
                    Some(InstallProgress {
                        current: completed,
                        total: pending.max(1),
                        secondary: None,
                    }),
                    InstallPhaseDetails::Empty,
                )
                .await?;
        }
    }
    check_canceled(cancellation)?;
    persist_actions(job_state, reporter, actions).await
}

async fn load_or_resolve_actions(
    job_state: &mut InstallJobState,
    instance_id: &str,
    intent: &crate::install::ContentChangeIntent,
    reporter: &InstallProgressReporter,
) -> crate::Result<Vec<crate::install::ContentChangeAction>> {
    let mut actions = match job_state.continuation.clone() {
        Some(InstallContinuationState::ChangeContent {
            version,
            mut actions,
        }) => {
            let switched_content_id = match intent {
                crate::install::ContentChangeIntent::SwitchVersion {
                    content_id,
                    ..
                } => Some(content_id.as_str()),
                _ => None,
            };
            for action in &mut actions {
                if version < crate::install::CONTENT_CHANGE_PLAN_VERSION {
                    action.operation = if switched_content_id
                        == Some(action.content_id.as_str())
                    {
                        crate::install::ContentChangeOperation::SwitchVersion
                    } else {
                        crate::install::ContentChangeOperation::Update
                    };
                }
                action.status = action.effective_status();
                if action.status
                    == crate::install::ContentChangeActionStatus::Failed
                {
                    action.set_status(
                        crate::install::ContentChangeActionStatus::Pending,
                    );
                }
            }
            actions
        }
        _ => {
            crate::api::instance::resolve_content_change_actions(
                instance_id,
                intent,
            )
            .await?
        }
    };
    for action in &mut actions {
        if action.final_relative_path.is_none() {
            action.final_relative_path = action.relative_path.clone();
        }
    }
    persist_actions(job_state, reporter, &actions).await?;
    Ok(actions)
}

async fn download_prepared_files(
    job_state: &mut InstallJobState,
    instance_id: &str,
    actions: &mut [crate::install::ContentChangeAction],
    reporter: &InstallProgressReporter,
    state: &std::sync::Arc<State>,
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<HashMap<String, DownloadedContent>> {
    let files = unique_pending_files(actions);
    if files.is_empty() {
        return Ok(HashMap::new());
    }
    let total_bytes = files
        .iter()
        .map(|file| file.integrity.size)
        .collect::<Option<Vec<_>>>()
        .map(|sizes| sizes.into_iter().sum::<u64>());
    let mut events = Vec::with_capacity(files.len() + 1);
    events.push(InstallJobEventKind::ContentDownloadStarted {
        files: files.len() as u64,
        bytes: total_bytes,
    });
    for file in &files {
        events.push(InstallJobEventKind::ContentFileQueued {
            path: file.target_relative_path.clone(),
            bytes_total: file.integrity.size,
            max_attempts: INSTALL_FILE_REPORTED_ATTEMPTS,
        });
        if !file.urls.is_empty() {
            events.push(InstallJobEventKind::ContentFileBrowserOptions {
                path: file.target_relative_path.clone(),
                urls: file.urls.clone(),
            });
        }
    }
    reporter
        .update(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: total_bytes.unwrap_or(files.len() as u64).max(1),
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;
    reporter.record_events(events).await?;

    let worker_reporter = reporter.clone().without_phase_updates();
    let concurrency = state.download_concurrency().max(1);
    let mut downloads = spawn_bounded(
        files.into_iter().map(|file| {
            let reporter = worker_reporter.clone();
            let instance_id = instance_id.to_string();
            let state = state.clone();
            async move {
                if file.manual_download_url.is_some() && file.urls.is_empty() {
                    return (
                        file,
                        Err(crate::Error::from(crate::ErrorKind::InputError(
                            "This content file requires a manual download"
                                .to_string(),
                        ))),
                    );
                }
                let result = match file.provider {
                ContentProvider::Modrinth => {
                    crate::state::instances::commands::download_project_version_with_reporter(
                        &instance_id,
                        &file.release_id,
                        if file.role == crate::install::ContentChangeFileRole::Primary {
                            DownloadReason::Update
                        } else {
                            DownloadReason::Dependency
                        },
                        None,
                        reporter,
                        &state,
                    )
                    .await
                    .map(DownloadedContent::Modrinth)
                }
                ContentProvider::CurseForge => {
                    match (
                        file.project_id.parse::<u32>(),
                        file.release_id.parse::<u32>(),
                    ) {
                        (Ok(project_id), Ok(file_id)) => {
                            crate::api::curseforge::stage_curseforge_upgrade_file(
                                project_id,
                                file_id,
                                None,
                                Some(&reporter),
                            )
                            .await
                                .map(DownloadedContent::CurseForge)
                        }
                        _ => Err(crate::ErrorKind::InputError(
                            "Invalid CurseForge project or file ID".to_string(),
                        )
                        .into()),
                    }
                }
                provider => Err(crate::ErrorKind::InputError(format!(
                    "Provider {} cannot be downloaded automatically",
                    provider.as_str()
                ))
                .into()),
            };
                (file, result)
            }
        }),
        concurrency,
    );
    let progress = async {
        let mut ticker = tokio::time::interval(Duration::from_millis(150));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let current = reporter.current_state().await?;
            let summary = current.download_summary();
            let (current, total) = match total_bytes {
                Some(total) => {
                    (summary.bytes_downloaded.min(total), total.max(1))
                }
                None => (
                    summary.files_completed,
                    summary.files_total.unwrap_or_default().max(1),
                ),
            };
            reporter
                .update(
                    InstallPhaseId::DownloadingContent,
                    Some(InstallProgress {
                        current,
                        total,
                        secondary: None,
                    }),
                    InstallPhaseDetails::Empty,
                )
                .await?;
        }
    };
    let download = async {
        let mut failed_files = HashSet::new();
        let mut downloaded = HashMap::new();
        while let Some(result) = downloads.join_next().await {
            let (file, result) = result.map_err(|error| {
                crate::ErrorKind::OtherError(error.to_string())
            })?;
            match result {
                Ok(content) => {
                    downloaded.insert(file.id.clone(), content);
                    reporter
                        .record_events(vec![
                            InstallJobEventKind::ContentFileCompleted {
                                path: file.target_relative_path,
                                bytes: file.integrity.size.unwrap_or_default(),
                            },
                        ])
                        .await?;
                }
                Err(error) => {
                    failed_files.insert(file.id.clone());
                    reporter
                        .record_events(vec![
                            InstallJobEventKind::ContentFileFailed {
                                path: file.target_relative_path,
                                reason: error.to_string(),
                                project_id: Some(file.project_id),
                                version_id: Some(file.release_id),
                            },
                        ])
                        .await?;
                }
            }
        }
        Ok((failed_files, downloaded))
    };
    let (failed_files, downloaded) =
        drive_downloads(cancellation, download, progress).await?;
    for action in actions.iter_mut().filter(|action| !action.is_complete()) {
        if action.effective_status()
            == crate::install::ContentChangeActionStatus::Failed
        {
            continue;
        }
        if action
            .files
            .iter()
            .any(|file| failed_files.contains(&file.id))
        {
            action.status = crate::install::ContentChangeActionStatus::Failed;
            action.error = Some(
                "One or more content files could not be downloaded".to_string(),
            );
        } else {
            action.set_status(
                crate::install::ContentChangeActionStatus::Downloaded,
            );
        }
    }
    persist_actions(job_state, reporter, actions).await?;
    Ok(downloaded)
}

async fn publish_actions(
    job_state: &mut InstallJobState,
    instance_id: &str,
    actions: &mut [crate::install::ContentChangeAction],
    reporter: &InstallProgressReporter,
    cancellation: &tokio_util::sync::CancellationToken,
    downloaded: &HashMap<String, DownloadedContent>,
    state: &State,
) -> crate::Result<()> {
    let total = actions.len() as u64;
    for index in 0..actions.len() {
        if actions[index].is_complete()
            || actions[index].effective_status()
                == crate::install::ContentChangeActionStatus::Failed
        {
            continue;
        }
        check_canceled(cancellation)?;
        reporter
            .update(
                InstallPhaseId::ApplyingContent,
                Some(InstallProgress {
                    current: index as u64,
                    total: total.max(1),
                    secondary: None,
                }),
                InstallPhaseDetails::Empty,
            )
            .await?;
        let current = {
            let _instance_lock = state
                .lock_instance_content_with_timeout(
                    instance_id,
                    std::time::Duration::from_secs(20),
                )
                .await?;
            crate::api::instance::projects::content_mutation_target(
                instance_id,
                &actions[index].content_id,
            )
            .await
        };
        check_canceled(cancellation)?;
        let result = match current {
            Ok(current)
                if current.provider_release_id.as_deref()
                    == Some(actions[index].target_release_id.as_str()) =>
            {
                actions[index].set_status(
                    crate::install::ContentChangeActionStatus::Skipped,
                );
                Ok(())
            }
            Ok(current)
                if current.provider == Some(actions[index].provider)
                    && current.provider_release_id
                        == actions[index].expected_release_id =>
            {
                actions[index].set_status(
                    crate::install::ContentChangeActionStatus::Applying,
                );
                // Progress persistence uses the install database semaphore.
                // The content lock was released above before awaiting it so
                // this task cannot form a lock-order cycle with DB workers.
                persist_actions(job_state, reporter, actions).await?;
                check_canceled(cancellation)?;
                let _instance_lock = state
                    .lock_instance_content_with_timeout(
                        instance_id,
                        std::time::Duration::from_secs(20),
                    )
                    .await?;
                publish_downloaded_action(
                    instance_id,
                    &actions[index],
                    downloaded,
                    state,
                )
                .await
            }
            Ok(_) => Err(crate::ErrorKind::InputError(format!(
                "Content {} changed after the task was queued",
                actions[index].content_id
            ))
            .into()),
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                if !actions[index].is_complete() {
                    actions[index].set_status(
                        crate::install::ContentChangeActionStatus::Completed,
                    );
                    crate::api::instance::emit_content_changed(instance_id)
                        .await?;
                }
            }
            Err(error) => {
                actions[index].status =
                    crate::install::ContentChangeActionStatus::Failed;
                actions[index].error = Some(error.to_string());
            }
        }
        // The content lock is intentionally released before persisting the
        // install-job checkpoint (see the Applying branch above).
        persist_actions(job_state, reporter, actions).await?;
    }
    Ok(())
}

async fn persist_actions(
    job_state: &mut InstallJobState,
    reporter: &InstallProgressReporter,
    actions: &[crate::install::ContentChangeAction],
) -> crate::Result<()> {
    let continuation = InstallContinuationState::ChangeContent {
        version: crate::install::CONTENT_CHANGE_PLAN_VERSION,
        actions: actions.to_vec(),
    };
    job_state.continuation = Some(continuation.clone());
    reporter.set_continuation(Some(continuation)).await
}

fn needs_preparation(action: &crate::install::ContentChangeAction) -> bool {
    !action.is_complete()
        && (action.files.is_empty()
            || (action.provider == ContentProvider::Modrinth
                && action.modrinth_plan.is_none()))
}

async fn publish_downloaded_action(
    instance_id: &str,
    action: &crate::install::ContentChangeAction,
    downloaded: &HashMap<String, DownloadedContent>,
    state: &State,
) -> crate::Result<()> {
    use crate::state::instances::adapters::sqlite::content_rows;
    use crate::state::instances::commands as content;
    use crate::state::instances::{ContentOwnershipKind, ContentSourceKind};

    let old_path = action.relative_path.as_deref().ok_or_else(|| {
        crate::ErrorKind::InputError("Missing current content path".into())
    })?;
    let final_path =
        action.final_relative_path.as_deref().ok_or_else(|| {
            crate::ErrorKind::InputError("Missing final content path".into())
        })?;
    let scope =
        content::resolve_content_scope(instance_id, None, state).await?;
    let base = content::instance_full_path(state, &scope.instance);
    path_util::SafeRelativeUtf8UnixPathBuf::try_from(final_path.to_string())?;
    if final_path != old_path && base.join(final_path).exists() {
        return Err(crate::ErrorKind::InputError(format!(
            "Content target {final_path} already exists"
        ))
        .into());
    }
    let ownership =
        content::content_ownership_for_path(instance_id, old_path, state)
            .await?;
    let primary_index = action
        .files
        .iter()
        .position(|file| {
            file.role == crate::install::ContentChangeFileRole::Primary
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Content plan has no primary file".into(),
            )
        })?;
    let mut paths = vec![String::new(); action.files.len()];
    let apply_order = (0..action.files.len())
        .filter(|index| *index != primary_index)
        .chain(std::iter::once(primary_index));
    for index in apply_order {
        let file = &action.files[index];
        let primary =
            file.role == crate::install::ContentChangeFileRole::Primary;
        if !primary
            && let Some(entry) =
                content_rows::get_content_entry_by_provider_ref(
                    &scope.content_set_id,
                    file.provider,
                    &file.project_id,
                    &file.release_id,
                    &state.pool,
                )
                .await?
        {
            let target = content_rows::get_content_mutation_target(
                instance_id,
                &entry.id,
                &state.pool,
            )
            .await?;
            if let Some(path) = target.and_then(|target| target.relative_path)
                && base.join(&path).is_file()
            {
                paths[index] = path;
                continue;
            }
        }
        let artifact = downloaded.get(&file.id).ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Missing staged content {}",
                file.id
            ))
        })?;
        let ownership = if primary {
            ownership
        } else {
            ContentOwnershipKind::UserAdded
        };
        let install_path = if primary {
            final_path.to_string()
        } else {
            let folder = match artifact {
                DownloadedContent::Modrinth(downloaded) => {
                    downloaded.project_type.get_folder()
                }
                DownloadedContent::CurseForge(staged) => {
                    staged.project_type.get_folder()
                }
            };
            format!(
                "{folder}/{}",
                crate::util::io::sanitize_download_file_name(&file.file_name)?
            )
        };
        path_util::SafeRelativeUtf8UnixPathBuf::try_from(install_path.clone())?;
        if (!primary || install_path != old_path)
            && base.join(&install_path).exists()
        {
            return Err(crate::ErrorKind::InputError(format!(
                "Content target {install_path} already exists"
            ))
            .into());
        }
        let path = match artifact {
            DownloadedContent::Modrinth(downloaded) => {
                content::apply_downloaded_project_version_at_path(
                    instance_id,
                    &install_path,
                    downloaded.clone(),
                    ContentSourceKind::Local,
                    ownership,
                    state,
                )
                .await?
            }
            DownloadedContent::CurseForge(staged) => {
                crate::api::curseforge::apply_staged_curseforge_upgrade_file_with_state(
                    instance_id,
                    staged.clone(),
                    ownership,
                    &install_path,
                    state,
                )
                .await?
            }
        };
        if !primary
            && let Some(entry) =
                content_rows::get_content_entry_by_relative_path(
                    &scope.content_set_id,
                    &path,
                    &state.pool,
                )
                .await?
        {
            content_rows::set_content_entry_auto_dependency(
                &entry.id,
                true,
                &state.pool,
            )
            .await?;
        }
        paths[index] = path;
    }
    content::finalize_updated_project_path(
        instance_id,
        old_path,
        &paths[primary_index],
        action.current_provider_file_name.as_deref(),
        action.target_provider_file_name.as_deref().ok_or_else(|| {
            crate::ErrorKind::InputError("Missing target filename".into())
        })?,
        state,
    )
    .await?;
    if let Some(plan) = &action.modrinth_plan {
        content::persist_resolved_plan_dependency_edges(
            instance_id,
            &paths,
            plan,
            state,
        )
        .await?;
    } else {
        persist_prepared_dependency_edges(
            &scope.content_set_id,
            action,
            &paths,
            downloaded,
            state,
        )
        .await?;
    }
    Ok(())
}

async fn persist_prepared_dependency_edges(
    content_set_id: &str,
    action: &crate::install::ContentChangeAction,
    paths: &[String],
    downloaded: &HashMap<String, DownloadedContent>,
    state: &State,
) -> crate::Result<()> {
    use crate::state::instances::adapters::sqlite::content_rows;
    let mut edges = Vec::new();
    for dependency in &action.dependencies {
        let parent_index = action.files.iter().position(|file| {
            file.id == dependency.parent_file_id
                || (file.provider == ContentProvider::CurseForge
                    && format!("curseforge:{}:unknown", file.project_id)
                        == dependency.parent_file_id)
        });
        let child_index = action
            .files
            .iter()
            .position(|file| file.id == dependency.child_file_id);
        let (Some(parent_index), Some(child_index)) =
            (parent_index, child_index)
        else {
            continue;
        };
        let parent = &action.files[parent_index];
        let child = &action.files[child_index];
        let parent_entry = content_rows::get_content_entry_by_relative_path(
            content_set_id,
            &paths[parent_index],
            &state.pool,
        )
        .await?;
        let child_entry = content_rows::get_content_entry_by_relative_path(
            content_set_id,
            &paths[child_index],
            &state.pool,
        )
        .await?;
        let (Some(parent_entry), Some(child_entry)) =
            (parent_entry, child_entry)
        else {
            continue;
        };
        let now = chrono::Utc::now();
        edges.push(crate::state::instances::ContentDependencyEdge {
            id: format!("content-dependency:{}", Uuid::new_v4()),
            content_set_id: content_set_id.to_string(),
            parent_entry_id: parent_entry.id,
            child_entry_id: child_entry.id,
            evidence_provider: action.provider,
            parent_provider: parent.provider,
            child_provider: child.provider,
            dependency_kind: match downloaded.get(&parent.id) {
                Some(DownloadedContent::CurseForge(staged)) if child.provider == ContentProvider::CurseForge => {
                    staged.file.dependencies.iter().find(|reference| reference.mod_id.to_string() == child.project_id)
                        .map(|reference| if reference.relation().is_required() {
                            crate::state::instances::ContentDependencyKind::Required
                        } else {
                            crate::state::instances::ContentDependencyKind::Include
                        })
                }
                _ => None,
            }.or(dependency.kind).unwrap_or(crate::state::instances::ContentDependencyKind::Required),
            parent_project_id: parent.project_id.clone(),
            parent_release_id: parent.release_id.clone(),
            child_project_id: child.project_id.clone(),
            child_release_id: child.release_id.clone(),
            created_at: now,
            modified_at: now,
        });
    }
    if !edges.is_empty() {
        let mut tx = state.pool.begin().await?;
        for edge in edges {
            content_rows::upsert_content_dependency_edge_in_transaction(
                &edge, &mut tx,
            )
            .await?;
        }
        tx.commit().await?;
    }
    Ok(())
}

fn check_canceled(
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<()> {
    if cancellation.is_cancelled() {
        return Err(crate::ErrorKind::InputError(
            "Content change canceled".to_string(),
        )
        .into());
    }
    Ok(())
}

/// Workers must keep polling while the consumer persists progress using the
/// same reporter and database pool. Dropping the set aborts all pending workers.
fn spawn_bounded<T, F>(
    work: impl IntoIterator<Item = F>,
    concurrency: usize,
) -> tokio::task::JoinSet<T>
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let permits =
        std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
    let mut tasks = tokio::task::JoinSet::new();
    for future in work {
        let permits = permits.clone();
        tasks.spawn(async move {
            let _permit = permits
                .acquire_owned()
                .await
                .expect("worker semaphore stays open");
            future.await
        });
    }
    tasks
}

async fn cancelable<T>(
    cancellation: &tokio_util::sync::CancellationToken,
    work: impl std::future::Future<Output = crate::Result<T>>,
) -> crate::Result<T> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            Err(crate::ErrorKind::InputError("Content change canceled".to_string()).into())
        }
        result = work => result,
    }
}

// Keep polling the download workers while progress waits for their reporter lock.
async fn drive_downloads<T>(
    cancellation: &tokio_util::sync::CancellationToken,
    downloads: impl std::future::Future<Output = crate::Result<T>>,
    progress: impl std::future::Future<Output = crate::Result<()>>,
) -> crate::Result<T> {
    cancelable(cancellation, async {
        tokio::pin!(downloads);
        tokio::select! {
            result = &mut downloads => result,
            result = progress => {
                result?;
                downloads.await
            }
        }
    })
    .await
}

fn unique_pending_files(
    actions: &[crate::install::ContentChangeAction],
) -> Vec<crate::install::ContentChangeFile> {
    let mut files = HashMap::new();
    for action in actions.iter().filter(|action| !action.is_complete()) {
        if action.effective_status()
            == crate::install::ContentChangeActionStatus::Failed
        {
            continue;
        }
        for file in &action.files {
            files.entry(file.id.clone()).or_insert_with(|| file.clone());
        }
    }
    let mut files = files.into_values().collect::<Vec<_>>();
    files.sort_by(|left, right| left.id.cmp(&right.id));
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{
        ContentChangeAction, ContentChangeActionStatus, ContentChangeFile,
        ContentChangeFileIntegrity, ContentChangeFileRole,
        ContentChangeOperation,
    };

    #[tokio::test]
    async fn progress_wait_does_not_stop_polling_download_workers() {
        let lock = tokio::sync::Mutex::new(());
        let (locked_tx, locked_rx) = tokio::sync::oneshot::channel();
        let (sampling_tx, sampling_rx) = tokio::sync::oneshot::channel();
        let downloads = async {
            let _guard = lock.lock().await;
            locked_tx.send(()).unwrap();
            sampling_rx.await.unwrap();
            tokio::task::yield_now().await;
            Ok(42)
        };
        let progress = async {
            locked_rx.await.unwrap();
            sampling_tx.send(()).unwrap();
            let _guard = lock.lock().await;
            Ok(())
        };
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            drive_downloads(
                &tokio_util::sync::CancellationToken::new(),
                downloads,
                progress,
            ),
        )
        .await
        .expect("progress must not suspend a worker holding the reporter lock")
        .unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn workers_release_resources_while_consumer_waits_to_persist() {
        let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
        let held = std::sync::Arc::new(tokio::sync::Notify::new());
        let persisting = std::sync::Arc::new(tokio::sync::Notify::new());
        let mut tasks = spawn_bounded(
            (0..2).map(|index| {
                let lock = lock.clone();
                let held = held.clone();
                let persisting = persisting.clone();
                async move {
                    if index == 0 {
                        held.notified().await;
                    } else {
                        let _guard = lock.lock().await;
                        held.notify_one();
                        persisting.notified().await;
                        tokio::task::yield_now().await;
                    }
                    index
                }
            }),
            2,
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            assert_eq!(tasks.join_next().await.unwrap().unwrap(), 0);
            persisting.notify_one();
            let _guard = lock.lock().await;
            assert_eq!(tasks.join_next().await.unwrap().unwrap(), 1);
        })
        .await
        .expect(
            "worker must release its lock while consumer persists a completion",
        );
    }

    #[tokio::test]
    async fn dropping_workers_aborts_in_flight_work_and_respects_limit() {
        let active = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let tasks = spawn_bounded(
            (0..4).map(|_| {
                let active = active.clone();
                let started = started.clone();
                async move {
                    let _permit = active
                        .try_acquire()
                        .expect("at most two downloads may run");
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
            }),
            2,
        );
        started.notified().await;
        drop(tasks);
        let _permits = tokio::time::timeout(
            Duration::from_secs(1),
            active.acquire_many(2),
        )
        .await
        .expect("aborted downloads must release their resources")
        .unwrap();
    }

    #[tokio::test]
    async fn cancel_interrupts_pending_downloads_and_progress() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let held = tokio::sync::Mutex::new(());
        let downloads = async {
            let _guard = held.lock().await;
            started_tx.send(()).unwrap();
            std::future::pending::<crate::Result<()>>().await
        };
        let cancel = async {
            started_rx.await.unwrap();
            cancellation.cancel();
        };
        let (result, ()) =
            tokio::time::timeout(Duration::from_secs(1), async {
                tokio::join!(
                    drive_downloads(
                        &cancellation,
                        downloads,
                        std::future::pending()
                    ),
                    cancel,
                )
            })
            .await
            .expect(
                "cancellation must not wait for the pending network operation",
            );
        assert!(result.is_err());
        assert!(held.try_lock().is_ok(), "download future must be dropped");
    }

    #[tokio::test]
    async fn canceled_preparation_does_not_start_more_work() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        let result = cancelable::<()>(&cancellation, async {
            panic!("canceled work must not be polled")
        })
        .await;
        assert!(result.is_err());
    }

    fn file(id: &str) -> ContentChangeFile {
        ContentChangeFile {
            id: id.to_string(),
            role: ContentChangeFileRole::Dependency,
            provider: ContentProvider::Modrinth,
            project_id: id.to_string(),
            release_id: id.to_string(),
            file_name: format!("{id}.jar"),
            target_relative_path: format!("cache/{id}.jar"),
            urls: Vec::new(),
            manual_download_url: None,
            integrity: ContentChangeFileIntegrity::default(),
        }
    }

    fn action(
        id: &str,
        status: ContentChangeActionStatus,
        files: Vec<ContentChangeFile>,
    ) -> ContentChangeAction {
        ContentChangeAction {
            content_id: id.to_string(),
            operation: ContentChangeOperation::Update,
            provider: ContentProvider::Modrinth,
            project_id: Some(id.to_string()),
            expected_release_id: Some("old".to_string()),
            target_release_id: "new".to_string(),
            relative_path: Some(format!("mods/{id}.jar")),
            current_provider_file_name: None,
            target_provider_file_name: None,
            final_relative_path: Some(format!("mods/{id}.jar")),
            files,
            dependencies: Vec::new(),
            modrinth_plan: None,
            status,
            error: None,
            completed: false,
        }
    }

    #[test]
    fn prepared_plan_survives_serialization_and_retry() {
        let mut prepared = action(
            "primary",
            ContentChangeActionStatus::Failed,
            vec![file("primary")],
        );
        assert!(needs_preparation(&prepared));
        prepared.modrinth_plan = Some(content_management::ResolveContentPlan {
            primary: content_management::ResolvedContent {
                project_id: "projectA".into(),
                version_id: "versionA".into(),
                dependent_on_version_id: None,
                required: true,
                metadata: None,
            },
            dependencies: Vec::new(),
            skipped: Vec::new(),
        });
        let serialized = serde_json::to_string(&prepared).unwrap();
        let restored: ContentChangeAction =
            serde_json::from_str(&serialized).unwrap();
        assert!(!needs_preparation(&restored));
        assert_eq!(prepared.modrinth_plan, restored.modrinth_plan);
        let mut old = serde_json::to_value(&prepared).unwrap();
        old.as_object_mut().unwrap().remove("modrinth_plan");
        let restored: ContentChangeAction =
            serde_json::from_value(old).unwrap();
        assert!(needs_preparation(&restored));
    }

    async fn publish_fixture()
    -> (tempfile::TempDir, std::sync::Arc<State>, String) {
        use crate::state::instances::{CreateInstance, create_instance};
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let temp = tempfile::tempdir().unwrap();
        let directories = crate::state::DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "content-change-test".into(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let instance = create_instance(
            CreateInstance {
                name: "Content change".into(),
                path: Some("content-change".into()),
                game_version: "1.21.4".into(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                link: InstanceLink::Unmanaged,
                symlink_target: None,
                game_dir_override: None,
            },
            &state,
        )
        .await
        .unwrap();
        (temp, state, instance.id)
    }

    #[tokio::test]
    async fn publish_downloaded_content_is_offline_and_preserves_custom_disabled_name()
     {
        use crate::state::instances::adapters::sqlite::content_rows;
        use crate::state::instances::commands as content;
        let (temp, state, instance_id) = publish_fixture().await;
        let scope = content::resolve_content_scope(&instance_id, None, &state)
            .await
            .unwrap();
        let base = content::instance_full_path(&state, &scope.instance);
        std::fs::create_dir_all(base.join("mods")).unwrap();
        let old_path = "mods/[中文名]old.jar.disabled";
        let final_path = "mods/[中文名]new.jar.disabled";
        std::fs::write(base.join(old_path), b"old content").unwrap();
        let cache = temp.path().join("downloaded.jar");
        std::fs::write(&cache, b"new content").unwrap();
        let (size, sha1) =
            crate::util::fetch::sha1_file_async(&cache).await.unwrap();
        let mut primary = file("primary");
        primary.role = ContentChangeFileRole::Primary;
        primary.project_id = "projectA".into();
        primary.release_id = "versionA".into();
        primary.file_name = "new.jar".into();
        let mut action = action(
            "primary",
            ContentChangeActionStatus::Downloaded,
            vec![primary],
        );
        action.relative_path = Some(old_path.into());
        action.final_relative_path = Some(final_path.into());
        action.current_provider_file_name = Some("old.jar".into());
        action.target_provider_file_name = Some("new.jar".into());
        let artifacts = HashMap::from([(
            "primary".into(),
            DownloadedContent::Modrinth(content::DownloadedProjectVersion {
                file_name: "new.jar".into(),
                path: cache,
                sha1,
                size,
                project_type: crate::state::ProjectType::Mod,
                project_id: "projectA".into(),
                version_id: "versionA".into(),
            }),
        )]);
        tokio::time::timeout(
            Duration::from_secs(2),
            publish_downloaded_action(
                &instance_id,
                &action,
                &artifacts,
                &state,
            ),
        )
        .await
        .expect("publishing must not resolve metadata or download again")
        .unwrap();
        assert_eq!(
            std::fs::read(base.join(final_path)).unwrap(),
            b"new content"
        );
        assert!(!base.join(old_path).exists());
        assert!(!base.join("mods/new.jar").exists());
        let entry = content_rows::get_content_entry_by_relative_path(
            &scope.content_set_id,
            final_path,
            &state.pool,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!entry.enabled);
        let refs =
            content_rows::get_content_provider_refs(&entry.id, &state.pool)
                .await
                .unwrap();
        assert!(
            refs.iter()
                .any(|reference| reference.database_release_id().as_deref()
                    == Some("versionA"))
        );

        std::fs::write(base.join(old_path), b"original").unwrap();
        let result = publish_downloaded_action(
            &instance_id,
            &action,
            &artifacts,
            &state,
        )
        .await;
        assert!(
            result.is_err(),
            "existing destination must fail before writing"
        );
        assert_eq!(std::fs::read(base.join(old_path)).unwrap(), b"original");
        assert_eq!(
            std::fs::read(base.join(final_path)).unwrap(),
            b"new content"
        );
    }

    #[tokio::test]
    async fn publish_curseforge_content_is_offline_and_restores_file_on_index_failure()
     {
        use crate::state::instances::commands as content;
        let (temp, state, instance_id) = publish_fixture().await;
        let scope = content::resolve_content_scope(&instance_id, None, &state)
            .await
            .unwrap();
        let base = content::instance_full_path(&state, &scope.instance);
        std::fs::create_dir_all(base.join("mods")).unwrap();
        let path = "mods/custom.jar.disabled";
        std::fs::write(base.join(path), b"original").unwrap();
        let cache = temp.path().join("downloaded.jar");
        std::fs::write(&cache, b"downloaded").unwrap();
        let (size, sha1) =
            crate::util::fetch::sha1_file_async(&cache).await.unwrap();
        let staged_file = serde_json::from_value(serde_json::json!({
            "id": 456, "modId": 123, "gameId": 432, "isAvailable": true,
            "displayName": "new", "fileName": "new.jar", "releaseType": 1, "fileStatus": 4,
            "fileDate": "", "fileLength": size, "downloadCount": 0, "fileFingerprint": 0,
            "hashes": [{ "algo": 1, "value": sha1 }]
        })).unwrap();
        let mut artifacts = HashMap::from([(
            "primary".into(),
            DownloadedContent::CurseForge(
                crate::api::curseforge::StagedCurseForgeUpgrade {
                    path: cache.clone(),
                    file: staged_file,
                    project_type: crate::state::ProjectType::Mod,
                },
            ),
        )]);
        let mut primary = file("primary");
        primary.role = ContentChangeFileRole::Primary;
        primary.provider = ContentProvider::CurseForge;
        primary.project_id = "123".into();
        primary.release_id = "456".into();
        let mut action = action(
            "primary",
            ContentChangeActionStatus::Downloaded,
            vec![primary],
        );
        action.provider = ContentProvider::CurseForge;
        action.relative_path = Some(path.into());
        action.final_relative_path = Some(path.into());
        action.current_provider_file_name = Some("old.jar".into());
        action.target_provider_file_name = Some("new.jar".into());

        sqlx::query("CREATE TRIGGER reject_content_insert BEFORE INSERT ON instance_content_entries BEGIN SELECT RAISE(ABORT, 'injected index failure'); END")
            .execute(&state.pool).await.unwrap();
        assert!(
            publish_downloaded_action(
                &instance_id,
                &action,
                &artifacts,
                &state
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read(base.join(path)).unwrap(), b"original");
        sqlx::query("DROP TRIGGER reject_content_insert")
            .execute(&state.pool)
            .await
            .unwrap();
        let mut dependency = file("dependency");
        dependency.project_id = "projectA".into();
        dependency.release_id = "versionA".into();
        action.files.push(dependency);
        action
            .dependencies
            .push(crate::install::ContentChangeDependency {
                parent_file_id: "primary".into(),
                child_file_id: "dependency".into(),
                provider: ContentProvider::Modrinth,
                project_id: "projectA".into(),
                release_id: "versionA".into(),
                kind: Some(
                    crate::state::instances::ContentDependencyKind::Include,
                ),
            });
        let (size, sha1) =
            crate::util::fetch::sha1_file_async(&cache).await.unwrap();
        artifacts.insert(
            "dependency".into(),
            DownloadedContent::Modrinth(content::DownloadedProjectVersion {
                file_name: "dependency.jar".into(),
                path: cache,
                sha1,
                size,
                project_type: crate::state::ProjectType::Mod,
                project_id: "projectA".into(),
                version_id: "versionA".into(),
            }),
        );
        tokio::time::timeout(
            Duration::from_secs(2),
            publish_downloaded_action(
                &instance_id,
                &action,
                &artifacts,
                &state,
            ),
        )
        .await
        .expect("CurseForge publishing must not access the API")
        .unwrap();
        assert_eq!(std::fs::read(base.join(path)).unwrap(), b"downloaded");
        assert!(!base.join("mods/new.jar").exists());
        let kind: String = sqlx::query_scalar("SELECT dependency_kind FROM instance_content_dependencies WHERE content_set_id = ?")
            .bind(&scope.content_set_id).fetch_one(&state.pool).await.unwrap();
        assert_eq!(kind, "include");
    }

    #[test]
    fn pending_downloads_are_deduplicated_across_actions() {
        let files = unique_pending_files(&[
            action(
                "first",
                ContentChangeActionStatus::Prepared,
                vec![file("primary-a"), file("shared")],
            ),
            action(
                "second",
                ContentChangeActionStatus::Prepared,
                vec![file("primary-b"), file("shared")],
            ),
        ]);

        assert_eq!(
            files
                .iter()
                .map(|file| file.id.as_str())
                .collect::<Vec<_>>(),
            vec!["primary-a", "primary-b", "shared"]
        );
    }

    #[test]
    fn completed_and_failed_actions_do_not_schedule_downloads() {
        let files = unique_pending_files(&[
            action(
                "completed",
                ContentChangeActionStatus::Completed,
                vec![file("completed")],
            ),
            action(
                "failed",
                ContentChangeActionStatus::Failed,
                vec![file("failed")],
            ),
            action(
                "pending",
                ContentChangeActionStatus::Prepared,
                vec![file("pending")],
            ),
        ]);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "pending");
    }
}
