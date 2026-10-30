use super::*;

pub(super) enum StagedUpgradeDownload {
    Modrinth(crate::state::instances::commands::DownloadedProjectVersion),
    CurseForge(crate::api::curseforge::StagedCurseForgeUpgrade),
}

pub(super) struct StagedUpgradeMutation {
    pub(super) existing_path: Option<String>,
    pub(super) target_path: String,
    pub(super) ownership: crate::state::instances::ContentOwnershipKind,
    pub(super) auto_dependency: bool,
    pub(super) enabled: bool,
    pub(super) download: StagedUpgradeDownload,
}

struct UpgradeStagingRequest {
    index: usize,
    provider: ContentProvider,
    project_id: String,
    release_id: String,
    auto_dependency: bool,
    enabled: bool,
    existing_path: Option<String>,
    ownership: crate::state::instances::ContentOwnershipKind,
    project_type: Option<crate::state::ProjectType>,
}

pub(super) struct AppliedUpgradeContent {
    skipped: Vec<String>,
    launcher_expected_files:
        HashMap<String, Option<crate::state::InstanceUpgradeSourceFile>>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_instance_upgrade(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    source_instance_id: &str,
    target_instance_id: &str,
    plan_id: &str,
    execution: InstanceUpgradeExecution,
    create_full_backup: bool,
    shared_upgrade_mode: SharedUpgradeMode,
    display_names: InstanceUpgradeDisplayNames,
) -> crate::Result<()> {
    let compatibility_warning_details =
        upgrade_compatibility_warning_details(&execution);
    let (upgrade_source_files, upgrade_watch) = if shared_upgrade_mode
        == SharedUpgradeMode::CopyAndUpgrade
    {
        update_progress(
            job_id,
            job_state,
            state,
            InstallPhaseId::CreatingBackup,
            InstallPhaseDetails::Empty,
        )
        .await?;
        copy_physical_instance_contents(
            job_id,
            job_state,
            state,
            source_instance_id,
            target_instance_id,
        )
        .await?;
        let files = crate::state::instances::commands::scan_instance_upgrade_source_files(
                target_instance_id,
                state,
            )
            .await?;
        let watch = state
            .file_watcher
            .track_upgrade_source(
                target_instance_id,
                files.iter().map(|file| file.relative_path.clone()),
            )
            .await
            .map(|snapshot| InstanceUpgradeWatchBaseline {
                epoch: snapshot.epoch,
                generation: snapshot.generation,
                dirty_paths: snapshot.dirty_paths.into_iter().collect(),
            });
        (files, watch)
    } else {
        (
            execution.source_files.clone(),
            execution.source_watch.clone(),
        )
    };

    let backup_instance_id = if should_create_upgrade_backup(
        create_full_backup,
        shared_upgrade_mode,
    ) {
        update_progress(
            job_id,
            job_state,
            state,
            InstallPhaseId::CreatingBackup,
            InstallPhaseDetails::Empty,
        )
        .await?;
        Some(
            create_upgrade_backup(
                job_id,
                job_state,
                state,
                source_instance_id,
                display_names.backup.as_deref(),
            )
            .await?,
        )
    } else {
        None
    };
    InstallProgressReporter::new(job_id, job_state.clone())
        .set_upgrade_result(InstanceUpgradeResult {
            plan_id: plan_id.to_string(),
            source_instance_id: source_instance_id.to_string(),
            target_instance_id: target_instance_id.to_string(),
            backup_instance_id: backup_instance_id.clone(),
            source_environment: Some(execution.source_environment.clone()),
            target_environment: Some(execution.target_environment.clone()),
            solution: execution.solution.clone(),
            compatibility_warnings: execution.warnings.clone(),
            compatibility_warning_details: compatibility_warning_details
                .clone(),
            external_changes: Vec::new(),
            skipped_due_to_external_conflict: Vec::new(),
        })
        .await?;

    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::StagingContent,
        InstallPhaseDetails::Empty,
    )
    .await?;
    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::DownloadingContent,
        InstallPhaseDetails::Empty,
    )
    .await?;
    let staged = stage_upgrade_content(
        target_instance_id,
        &execution,
        Some(InstallProgressReporter::new(job_id, job_state.clone())),
        state,
    )
    .await?;

    let mut external_changes = collect_upgrade_external_changes(
        target_instance_id,
        &upgrade_source_files,
        upgrade_watch.as_ref(),
        state,
    )
    .await?;
    for relative_path in
        collect_unsafe_upgrade_paths(target_instance_id).await?
    {
        if !external_changes
            .iter()
            .any(|change| change.relative_path == relative_path)
        {
            external_changes.push(InstanceUpgradeExternalChange {
                relative_path,
                kind: InstanceUpgradeExternalChangeKind::Modified,
            });
        }
    }
    let mut external_paths = external_changes
        .iter()
        .map(|change| change.relative_path.clone())
        .collect::<HashSet<_>>();
    for mutation in &staged {
        if let Some(path) = mutation.existing_path.as_deref()
            && source_file_changed(
                path,
                &upgrade_source_files,
                target_instance_id,
            )
            .await?
            && external_paths.insert(path.to_string())
        {
            external_changes.push(InstanceUpgradeExternalChange {
                relative_path: path.to_string(),
                kind: InstanceUpgradeExternalChangeKind::Modified,
            });
        }
    }
    if shared_upgrade_mode == SharedUpgradeMode::Direct {
        let replacement_paths = staged
            .iter()
            .map(|mutation| mutation.target_path.clone())
            .collect::<Vec<_>>();
        recovery::prepare_existing_upgrade_content_rollback(
            job_id,
            job_state,
            state,
            replacement_paths,
        )
        .await?;
        recovery::restore_upgrade_db_baseline(job_state, state).await?;
    }

    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::ApplyingContent,
        InstallPhaseDetails::Empty,
    )
    .await?;
    let applied = apply_upgrade_content(
        target_instance_id,
        staged,
        &execution,
        &external_paths,
        &upgrade_source_files,
        state,
    )
    .await?;
    if !applied.skipped.is_empty() {
        InstallProgressReporter::new(job_id, job_state.clone())
            .record_events(
                applied
                    .skipped
                    .iter()
                    .map(|relative_path| {
                        InstallJobEventKind::UpgradeItemSkipped {
                            relative_path: relative_path.clone(),
                            reason: "external_conflict".to_string(),
                        }
                    })
                    .collect(),
            )
            .await?;
    }

    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::UpdatingLoader,
        InstallPhaseDetails::Minecraft {
            game_version: execution.target_environment.game_version.clone(),
            loader: execution.target_environment.mod_loader,
        },
    )
    .await?;
    let upgraded_target_name = display_names
        .should_auto_rename
        .then(|| default_upgrade_instance_name(&execution.target_environment))
        .or(display_names.upgraded_target.clone());
    crate::state::edit_instance(
        target_instance_id,
        crate::state::EditInstance {
            name: upgraded_target_name,
            content_set_patch: Some(crate::state::AppliedContentSetPatch {
                source_kind: Some(
                    crate::state::instances::ContentSourceKind::Local,
                ),
                game_version: Some(
                    execution.target_environment.game_version.clone(),
                ),
                loader: Some(execution.target_environment.mod_loader),
                loader_version: Some(
                    execution.target_environment.mod_loader_version.clone(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        },
        &state.pool,
    )
    .await?;
    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::DownloadingMinecraft,
        InstallPhaseDetails::Empty,
    )
    .await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            target_instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown upgrade target".to_string())
        })?;
    crate::launcher::install_minecraft_with_reporter(
        &context,
        false,
        Some(InstallProgressReporter::new(job_id, job_state.clone())),
        crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
    )
    .await?;

    update_progress(
        job_id,
        job_state,
        state,
        InstallPhaseId::Verifying,
        InstallPhaseDetails::Empty,
    )
    .await?;
    crate::state::instances::commands::get_content_snapshot(
        target_instance_id,
        true,
        state,
    )
    .await?;
    let final_files =
        crate::state::instances::commands::scan_instance_upgrade_source_files(
            target_instance_id,
            state,
        )
        .await?;
    merge_upgrade_external_changes(
        &mut external_changes,
        final_upgrade_external_changes(
            &upgrade_source_files,
            &final_files,
            &applied.launcher_expected_files,
        ),
    );
    if !external_changes.is_empty() {
        InstallProgressReporter::new(job_id, job_state.clone())
            .record_events(
                external_changes
                    .iter()
                    .map(|change| InstallJobEventKind::UpgradeExternalChange {
                        relative_path: change.relative_path.clone(),
                        kind: change.kind,
                    })
                    .collect(),
            )
            .await?;
    }
    InstallProgressReporter::new(job_id, job_state.clone())
        .set_upgrade_result(InstanceUpgradeResult {
            plan_id: plan_id.to_string(),
            source_instance_id: source_instance_id.to_string(),
            target_instance_id: target_instance_id.to_string(),
            backup_instance_id,
            source_environment: Some(execution.source_environment),
            target_environment: Some(execution.target_environment),
            solution: execution.solution,
            compatibility_warnings: execution.warnings,
            compatibility_warning_details: compatibility_warning_details
                .clone(),
            external_changes,
            skipped_due_to_external_conflict: applied.skipped,
        })
        .await?;
    Ok(())
}

pub(super) fn default_upgrade_instance_name(
    environment: &crate::state::InstanceUpgradeEnvironment,
) -> String {
    let loader = match environment.mod_loader {
        ModLoader::Vanilla => "Vanilla",
        ModLoader::Forge => "Forge",
        ModLoader::Fabric => "Fabric",
        ModLoader::Quilt => "Quilt",
        ModLoader::NeoForge => "NeoForge",
        ModLoader::OptiFine => "OptiFine",
        ModLoader::Cleanroom => "Cleanroom",
        ModLoader::LiteLoader => "LiteLoader",
        ModLoader::LegacyFabric => "Legacy Fabric",
        ModLoader::Babric => "Babric",
    };
    let loader_version = environment
        .mod_loader_version
        .as_deref()
        .filter(|version| !matches!(*version, "latest" | "stable"))
        .map(|version| format!(" {version}"))
        .unwrap_or_default();
    format!("{}-{loader}{loader_version}", environment.game_version)
}

pub(super) fn upgrade_compatibility_warning_details(
    execution: &InstanceUpgradeExecution,
) -> Vec<InstanceUpgradeCompatibilityWarning> {
    let physical_details = execution
        .items
        .iter()
        .filter_map(|item| {
            let code = physical_upgrade_warning_code(execution, item)?;
            execution
                .warnings
                .iter()
                .any(|warning| warning.code == code)
                .then(|| InstanceUpgradeCompatibilityWarning {
                    code,
                    relative_path: Some(item.relative_path.clone()),
                    content_id: Some(item.content_id.clone()),
                    provider: item.provider,
                    project_id: item.project_id.clone(),
                    conflicting_project_id: None,
                })
        })
        .collect::<Vec<_>>();
    let mut details = execution
        .warnings
        .iter()
        .filter_map(|warning| {
            let item = warning
                .content_id
                .as_ref()
                .and_then(|content_id| {
                    execution
                        .items
                        .iter()
                        .find(|item| item.content_id == *content_id)
                })
                .or_else(|| {
                    let mut matches = execution.items.iter().filter(|item| {
                        item.provider == warning.provider
                            && item.project_id == warning.project_id
                    });
                    let item = matches.next()?;
                    matches.next().is_none().then_some(item)
                });
            if item.is_none()
                && physical_details
                    .iter()
                    .any(|detail| detail.code == warning.code)
            {
                return None;
            }
            Some(InstanceUpgradeCompatibilityWarning {
                code: warning.code,
                relative_path: item.map(|item| item.relative_path.clone()),
                content_id: item
                    .map(|item| item.content_id.clone())
                    .or_else(|| warning.content_id.clone()),
                provider: warning.provider,
                project_id: warning.project_id.clone(),
                conflicting_project_id: warning.conflicting_project_id.clone(),
            })
        })
        .collect::<Vec<_>>();
    for detail in physical_details {
        if !details.iter().any(|existing| {
            existing.code == detail.code
                && existing.content_id == detail.content_id
                && existing.relative_path == detail.relative_path
        }) {
            details.push(detail);
        }
    }
    details
}

fn physical_upgrade_warning_code(
    execution: &InstanceUpgradeExecution,
    item: &crate::state::InstanceUpgradeItem,
) -> Option<crate::state::InstanceUpgradeIssueCode> {
    use crate::state::{
        InstanceUpgradeAction, InstanceUpgradeIssueCode,
        InstanceUpgradeItemStatus,
    };

    let action = execution
        .solution
        .selections
        .iter()
        .find(|selection| selection.content_id == item.content_id)
        .map(|selection| selection.action)
        .unwrap_or(item.resolution.action);
    match item.status {
        InstanceUpgradeItemStatus::Unidentified => {
            Some(InstanceUpgradeIssueCode::Unidentified)
        }
        InstanceUpgradeItemStatus::UnsupportedContentType => {
            Some(InstanceUpgradeIssueCode::UnsupportedContentType)
        }
        InstanceUpgradeItemStatus::NoCompatibleRelease
        | InstanceUpgradeItemStatus::UpgradeAvailable
            if action == InstanceUpgradeAction::Keep =>
        {
            Some(InstanceUpgradeIssueCode::KeepIncompatible)
        }
        InstanceUpgradeItemStatus::NoCompatibleShaderRuntime
            if action == InstanceUpgradeAction::Keep =>
        {
            Some(InstanceUpgradeIssueCode::NoCompatibleShaderRuntime)
        }
        InstanceUpgradeItemStatus::ShaderRuntimeMissing
            if action == InstanceUpgradeAction::Keep =>
        {
            Some(InstanceUpgradeIssueCode::ShaderRuntimeMissing)
        }
        InstanceUpgradeItemStatus::ShaderRuntimeUnknown
            if action == InstanceUpgradeAction::Keep =>
        {
            Some(InstanceUpgradeIssueCode::ShaderRuntimeUnknown)
        }
        _ => None,
    }
}

async fn copy_physical_instance_contents(
    job_id: Uuid,
    job_state: &InstallJobState,
    state: &State,
    source_instance_id: &str,
    target_instance_id: &str,
) -> crate::Result<()> {
    let source_path = crate::util::io::canonicalize(
        &crate::api::instance::get_full_path(source_instance_id).await?,
    )?;
    crate::api::pack::import::copy_dotminecraft_with_reporter(
        target_instance_id,
        source_path,
        &state.io_semaphore,
        InstallProgressReporter::new(job_id, job_state.clone()),
        InstallPhaseDetails::Empty,
    )
    .await?;
    crate::state::sync_content_files(target_instance_id, state).await?;
    Ok(())
}

pub(super) async fn create_upgrade_backup(
    job_id: Uuid,
    job_state: &InstallJobState,
    state: &State,
    source_instance_id: &str,
    backup_name: Option<&str>,
) -> crate::Result<String> {
    let source = crate::state::get_instance(source_instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown upgrade source".to_string())
        })?;
    let backup = crate::api::instance::create(
        backup_name.map(str::to_string).unwrap_or_else(|| {
            format!("{} (Upgrade Backup)", source.instance.name)
        }),
        source.applied_content_set.game_version.clone(),
        source.applied_content_set.loader,
        source.applied_content_set.loader_version.clone(),
        source.instance.icon_path.clone(),
        InstanceLink::Unmanaged,
        None,
        None,
    )
    .await?;
    let backup_result = async {
        clone_instance_loader_components(
            &source.loader_components,
            &backup.instance.id,
            state,
        )
        .await?;
        copy_physical_instance_contents(
            job_id,
            job_state,
            state,
            source_instance_id,
            &backup.instance.id,
        )
        .await?;
        clone_upgrade_backup_content_metadata(
            source_instance_id,
            &source.applied_content_set.id,
            &backup.instance.id,
            &backup.applied_content_set.id,
            state,
        )
        .await?;
        crate::state::instances::commands::set_instance_install_stage(
            &backup.instance.id,
            InstanceInstallStage::Installed,
            &state.pool,
        )
        .await
    }
    .await;
    if let Err(error) = backup_result {
        return match crate::state::remove_instance(&backup.instance.id, state)
            .await
        {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(crate::ErrorKind::OtherError(format!(
                "Upgrade backup creation failed: {error}; cleanup also failed: {cleanup_error}"
            ))
            .into()),
        };
    }
    Ok(backup.instance.id)
}

async fn clone_upgrade_backup_content_metadata(
    source_instance_id: &str,
    source_content_set_id: &str,
    target_instance_id: &str,
    target_content_set_id: &str,
    state: &State,
) -> crate::Result<()> {
    use crate::state::instances::adapters::sqlite::{content_entries, files};

    let source_files =
        files::get_instance_files(source_instance_id, &state.pool).await?;
    let target_files =
        files::get_instance_files(target_instance_id, &state.pool).await?;
    let source_entries = content_entries::get_content_entries(
        source_content_set_id,
        &state.pool,
    )
    .await?;
    let source_edges = content_entries::get_content_dependency_edges(
        source_content_set_id,
        &state.pool,
    )
    .await?;
    let dependency_backfilled =
        content_entries::get_dependency_backfilled_entry_ids(
            source_content_set_id,
            &state.pool,
        )
        .await?;
    let source_paths = source_files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let target_file_ids = target_files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut provider_refs = HashMap::new();
    for entry in &source_entries {
        provider_refs.insert(
            entry.id.as_str(),
            content_entries::get_content_provider_refs_with_origin(
                &entry.id,
                &state.pool,
            )
            .await?,
        );
    }

    let mut tx = state.pool.begin().await?;
    content_entries::delete_dependency_edges_for_set_in_transaction(
        target_content_set_id,
        &mut tx,
    )
    .await?;
    let mut entry_ids = HashMap::new();
    for source_entry in &source_entries {
        let target_file_id = match source_entry.file_id.as_deref() {
            Some(source_file_id) => {
                let relative_path = source_paths.get(source_file_id).ok_or_else(
                    || {
                        crate::ErrorKind::FSError(format!(
                            "Upgrade backup source content entry {} has no file",
                            source_entry.id
                        ))
                    },
                )?;
                Some(*target_file_ids.get(relative_path).ok_or_else(|| {
                    crate::ErrorKind::FSError(format!(
                        "Upgrade backup is missing copied content file {relative_path}"
                    ))
                })?)
            }
            None => None,
        };
        let target_entry =
            content_entries::upsert_content_entry_from_parts_in_transaction(
                content_entries::UpsertContentEntry {
                    instance_id: target_instance_id,
                    content_set_id: target_content_set_id,
                    file_id: target_file_id,
                    project_type: source_entry.project_type,
                    source_kind: source_entry.source_kind,
                    ownership_kind: source_entry.ownership_kind,
                    auto_dependency: source_entry.auto_dependency,
                    server_requirement: source_entry.server_requirement,
                    client_requirement: source_entry.client_requirement,
                    enabled: source_entry.enabled,
                },
                &mut tx,
            )
            .await?;
        content_entries::delete_provider_refs_for_entry_in_transaction(
            &target_entry.id,
            &mut tx,
        )
        .await?;
        for (provider_ref, origin) in &provider_refs[source_entry.id.as_str()] {
            content_entries::upsert_content_provider_ref_in_transaction(
                &target_entry.id,
                provider_ref,
                *origin,
                &mut tx,
            )
            .await?;
        }
        if dependency_backfilled.contains(&source_entry.id) {
            content_entries::set_content_entry_dependency_backfilled_in_transaction(
                &target_entry.id,
                &mut tx,
            )
            .await?;
        }
        entry_ids.insert(source_entry.id.as_str(), target_entry.id);
    }
    for source_edge in source_edges {
        let parent_entry_id = entry_ids
            .get(source_edge.parent_entry_id.as_str())
            .ok_or_else(|| {
                crate::ErrorKind::FSError(format!(
                    "Upgrade backup cannot map dependency parent {}",
                    source_edge.parent_entry_id
                ))
            })?;
        let child_entry_id = entry_ids
            .get(source_edge.child_entry_id.as_str())
            .ok_or_else(|| {
                crate::ErrorKind::FSError(format!(
                    "Upgrade backup cannot map dependency child {}",
                    source_edge.child_entry_id
                ))
            })?;
        let now = chrono::Utc::now();
        content_entries::upsert_content_dependency_edge_in_transaction(
            &crate::state::instances::ContentDependencyEdge {
                id: format!("content-dependency:{}", Uuid::new_v4()),
                content_set_id: target_content_set_id.to_string(),
                parent_entry_id: parent_entry_id.clone(),
                child_entry_id: child_entry_id.clone(),
                created_at: now,
                modified_at: now,
                ..source_edge
            },
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub(super) async fn clone_instance_loader_components(
    components: &[LoaderComponent],
    target_instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let components = components
        .iter()
        .cloned()
        .map(|mut component| {
            component.instance_id = target_instance_id.to_string();
            component
        })
        .collect::<Vec<_>>();
    crate::state::instances::commands::replace_instance_loader_components(
        target_instance_id,
        &components,
        &state.pool,
    )
    .await
}

async fn stage_upgrade_content(
    instance_id: &str,
    execution: &InstanceUpgradeExecution,
    reporter: Option<InstallProgressReporter>,
    state: &State,
) -> crate::Result<Vec<StagedUpgradeMutation>> {
    let metadata = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Unknown upgrade target".to_string())
        })?;
    let entries = crate::state::instances::adapters::sqlite::content_entries::get_content_entries(
        &metadata.applied_content_set.id,
        &state.pool,
    )
    .await?;
    let files =
        crate::state::instances::adapters::sqlite::files::get_instance_files(
            instance_id,
            &state.pool,
        )
        .await?;
    let files_by_id = files
        .iter()
        .map(|file| (file.id.as_str(), file.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let entries_by_path = entries
        .iter()
        .filter_map(|entry| {
            Some((
                files_by_id.get(entry.file_id.as_deref()?)?.to_string(),
                entry,
            ))
        })
        .collect::<HashMap<_, _>>();
    let item_paths = execution
        .items
        .iter()
        .map(|item| (item.content_id.as_str(), item.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    let mut requests = Vec::<(
        Option<String>,
        ContentProvider,
        String,
        String,
        bool,
        bool,
    )>::new();
    for selection in &execution.solution.selections {
        if selection.action != InstanceUpgradeAction::Upgrade {
            continue;
        }
        let Some(provider) = selection.provider else {
            continue;
        };
        let project_id = selection.project_id.clone().ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Upgrade selection {} has no project id",
                selection.content_id
            ))
        })?;
        let target_release_id =
            selection.target_release_id.clone().ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Upgrade selection {} has no target release",
                    selection.content_id
                ))
            })?;
        requests.push((
            Some(selection.content_id.clone()),
            provider,
            project_id,
            target_release_id,
            false,
            selection.enabled,
        ));
    }
    for change in &execution.solution.dependency_changes {
        if !matches!(
            change.kind,
            InstanceUpgradeDependencyChangeKind::Add
                | InstanceUpgradeDependencyChangeKind::Upgrade
        ) {
            continue;
        }
        requests.push((
            change.existing_content_id.clone(),
            change.provider,
            change.project_id.clone(),
            change.target_release_id.clone().ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Dependency {} has no target release",
                    change.project_id
                ))
            })?,
            true,
            change.enabled,
        ));
    }

    let mut seen = HashSet::new();
    requests.retain(|(content_id, provider, project_id, release_id, ..)| {
        seen.insert(format!(
            "{}:{}:{}:{}",
            content_id.as_deref().unwrap_or("new"),
            provider.as_str(),
            project_id,
            release_id
        ))
    });
    let contexts = requests
        .into_iter()
        .enumerate()
        .map(
            |(
                index,
                (
                    content_id,
                    provider,
                    project_id,
                    release_id,
                    auto_dependency,
                    enabled,
                ),
            )| {
                let source_path = content_id
                    .as_deref()
                    .and_then(|content_id| item_paths.get(content_id).copied());
                let existing_entry = content_id
                    .as_deref()
                    .and_then(|content_id| {
                        entries.iter().find(|entry| entry.id == content_id)
                    })
                    .or_else(|| {
                        source_path.and_then(|path| entries_by_path.get(path).copied())
                    });
                let existing_path = existing_entry
                    .and_then(|entry| entry.file_id.as_deref())
                    .and_then(|file_id| files_by_id.get(file_id).copied())
                    .map(ToString::to_string)
                    .or_else(|| source_path.map(ToString::to_string));
                let ownership = existing_entry
                    .map(|entry| entry.ownership_kind)
                    .unwrap_or(
                        crate::state::instances::ContentOwnershipKind::UserAdded,
                    );
                let project_type = existing_entry
                    .map(|entry| entry.project_type)
                    .or_else(|| {
                        source_path.and_then(
                            crate::state::instances::adapters::filesystem::project_type_from_relative_path,
                        )
                    });
                UpgradeStagingRequest {
                    index,
                    provider,
                    project_id,
                    release_id,
                    auto_dependency,
                    enabled,
                    existing_path,
                    ownership,
                    project_type,
                }
            },
        )
        .collect::<Vec<_>>();
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .record_events(vec![InstallJobEventKind::ContentDownloadStarted {
                files: contexts.len() as u64,
                bytes: None,
            }])
            .await?;
    }
    let mut downloads = contexts
        .into_iter()
        .map(|context| {
            let reporter = reporter.clone();
            async move {
                let index = context.index;
                let mutation = stage_one_upgrade_request(
                    instance_id,
                    context,
                    reporter,
                    state,
                )
                .await?;
                Ok::<_, crate::Error>((index, mutation))
            }
        })
        .collect::<FuturesUnordered<_>>();
    collect_ordered_upgrade_staging(&mut downloads).await
}

pub(super) async fn collect_ordered_upgrade_staging<F, T>(
    downloads: &mut FuturesUnordered<F>,
) -> crate::Result<Vec<T>>
where
    F: Future<Output = crate::Result<(usize, T)>>,
{
    let mut staged = Vec::with_capacity(downloads.len());
    while let Some(result) = downloads.next().await {
        staged.push(result?);
    }
    staged.sort_by_key(|(index, _)| *index);
    Ok(staged.into_iter().map(|(_, mutation)| mutation).collect())
}

async fn stage_one_upgrade_request(
    instance_id: &str,
    context: UpgradeStagingRequest,
    reporter: Option<InstallProgressReporter>,
    state: &State,
) -> crate::Result<StagedUpgradeMutation> {
    let download = match context.provider {
            ContentProvider::Modrinth => StagedUpgradeDownload::Modrinth(
                match reporter.as_ref() {
                    Some(reporter) => crate::state::instances::commands::download_project_version_with_reporter(
                        instance_id,
                        &context.release_id,
                        if context.auto_dependency {
                            DownloadReason::Dependency
                        } else {
                            DownloadReason::Update
                        },
                        None,
                        reporter.clone(),
                        state,
                    )
                    .await?,
                    None => crate::state::instances::commands::download_project_version(
                        instance_id,
                        &context.release_id,
                        if context.auto_dependency {
                            DownloadReason::Dependency
                        } else {
                            DownloadReason::Update
                        },
                        None,
                        state,
                    )
                    .await?,
                },
            ),
            ContentProvider::CurseForge => {
                let project_id = context.project_id.parse::<u32>().map_err(|_| {
                    crate::ErrorKind::InputError(
                        "CurseForge project id is invalid".to_string(),
                    )
                })?;
                let file_id = context.release_id.parse::<u32>().map_err(|_| {
                    crate::ErrorKind::InputError(
                        "CurseForge file id is invalid".to_string(),
                    )
                })?;
                StagedUpgradeDownload::CurseForge(
                    crate::api::curseforge::stage_curseforge_upgrade_file(
                        project_id,
                        file_id,
                        context.project_type,
                        reporter.as_ref(),
                    )
                    .await?,
                )
            }
            ContentProvider::McArchive => {
                return Err(crate::ErrorKind::InputError(
                    "MCArchive content cannot be downloaded by upgrade execution"
                        .to_string(),
                )
                .into());
            }
            ContentProvider::Local => {
                return Err(crate::ErrorKind::InputError(
                    "Local-only content cannot be downloaded by upgrade execution"
                        .to_string(),
                )
                .into());
            }
        };
    let target_path =
        context
            .existing_path
            .clone()
            .unwrap_or_else(|| match &download {
                StagedUpgradeDownload::Modrinth(download) => format!(
                    "{}/{}",
                    download.project_type.get_folder(),
                    download.file_name
                ),
                StagedUpgradeDownload::CurseForge(download) => format!(
                    "{}/{}",
                    download.project_type.get_folder(),
                    download.file.file_name
                ),
            });
    let download_size = match &download {
        StagedUpgradeDownload::Modrinth(download) => download.size,
        StagedUpgradeDownload::CurseForge(download) => {
            download.file.file_length
        }
    };
    if let Some(reporter) = reporter {
        reporter
            .record_events(vec![InstallJobEventKind::ContentFileCompleted {
                path: target_path.clone(),
                bytes: download_size,
            }])
            .await?;
    }
    Ok(StagedUpgradeMutation {
        existing_path: context.existing_path,
        target_path,
        ownership: context.ownership,
        auto_dependency: context.auto_dependency,
        enabled: context.enabled,
        download,
    })
}

pub(super) async fn apply_upgrade_content(
    instance_id: &str,
    staged: Vec<StagedUpgradeMutation>,
    execution: &InstanceUpgradeExecution,
    external_paths: &HashSet<String>,
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    state: &State,
) -> crate::Result<AppliedUpgradeContent> {
    let mut skipped = Vec::new();
    let mut launcher_expected_files = HashMap::new();
    #[cfg(debug_assertions)]
    let fail_after_mutations = injected_upgrade_failure_after_mutations();
    #[cfg(debug_assertions)]
    tracing::warn!(
        raw_env = ?std::env::var("AXOLOTL_TEST_UPGRADE_FAIL_AFTER_MUTATIONS"),
        parsed = ?fail_after_mutations,
        "T11 upgrade fault injection enabled"
    );
    #[cfg(not(debug_assertions))]
    tracing::warn!(
        "T11 upgrade fault injection unavailable: debug_assertions=false"
    );
    #[cfg(debug_assertions)]
    let pause_after_mutations = injected_upgrade_pause_after_mutations();
    #[cfg(debug_assertions)]
    tracing::warn!(
        raw_env = ?std::env::var("AXOLOTL_TEST_UPGRADE_PAUSE_AFTER_MUTATIONS"),
        parsed = ?pause_after_mutations,
        "T12 upgrade crash-test pause configured"
    );
    #[cfg(debug_assertions)]
    let mut completed_mutations = 0_usize;
    for mutation in staged {
        let changed_after_staging = match mutation.existing_path.as_deref() {
            Some(path) => {
                source_file_changed(path, source_files, instance_id).await?
            }
            None => false,
        };
        if upgrade_mutation_conflicts(&mutation, external_paths)
            || changed_after_staging
        {
            skipped.push(mutation.target_path);
            continue;
        }
        let relative_path = match mutation.download {
            StagedUpgradeDownload::Modrinth(download) => {
                crate::state::instances::commands::apply_downloaded_project_version_at_path(
                    instance_id,
                    &mutation.target_path,
                    download,
                    crate::state::instances::ContentSourceKind::Local,
                    mutation.ownership,
                    state,
                )
                .await?
            }
            StagedUpgradeDownload::CurseForge(download) => {
                crate::api::curseforge::apply_staged_curseforge_upgrade_file(
                    instance_id,
                    download,
                    mutation.ownership,
                    &mutation.target_path,
                )
                .await?
            }
        };
        let scope = crate::state::instances::commands::resolve_content_scope(
            instance_id,
            None,
            state,
        )
        .await?;
        let mut final_relative_path = relative_path.clone();
        if let Some(entry) = crate::state::instances::adapters::sqlite::content_entries::get_content_entry_by_relative_path(
            &scope.content_set_id,
            &relative_path,
            &state.pool,
        )
        .await?
        {
            if mutation.auto_dependency {
                crate::state::instances::adapters::sqlite::content_entries::set_content_entry_auto_dependency(
                    &entry.id,
                    true,
                    &state.pool,
                )
                .await?;
            }
            if entry.enabled != mutation.enabled {
                let toggled = crate::state::instances::commands::toggle_content_entries(
                    instance_id,
                    &[entry.id],
                    Some(mutation.enabled),
                    state,
                )
                .await?;
                if let Some(toggled) = toggled.first() {
                    final_relative_path = toggled.path.clone();
                }
            }
        }
        record_launcher_expected_file(
            instance_id,
            &relative_path,
            &final_relative_path,
            &mut launcher_expected_files,
        )
        .await?;
        #[cfg(debug_assertions)]
        {
            completed_mutations += 1;
            if fail_after_mutations == Some(completed_mutations) {
                return Err(crate::ErrorKind::InputError(format!(
                    "Injected unmanaged instance upgrade failure after {completed_mutations} mutation(s)"
                ))
                .into());
            }
        }
        #[cfg(debug_assertions)]
        if pause_after_mutations == Some(completed_mutations) {
            tracing::warn!(
                "T12 upgrade crash-test pause after {completed_mutations} mutation(s); terminate process now"
            );
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    let item_paths = execution
        .items
        .iter()
        .map(|item| (item.content_id.as_str(), item.relative_path.as_str()))
        .collect::<HashMap<_, _>>();
    for item in &execution.items {
        let path = item.relative_path.as_str();
        if external_paths.contains(path)
            || source_file_changed(path, source_files, instance_id).await?
        {
            continue;
        }
        let (_, desired) = execution.final_physical_decision(item);
        let final_path =
            set_upgrade_path_enabled(instance_id, path, desired, state).await?;
        record_launcher_expected_file(
            instance_id,
            path,
            &final_path,
            &mut launcher_expected_files,
        )
        .await?;
    }
    for change in &execution.solution.dependency_changes {
        let Some(content_id) = change.existing_content_id.as_deref() else {
            continue;
        };
        let Some(path) = item_paths.get(content_id).copied() else {
            continue;
        };
        if external_paths.contains(path)
            || source_file_changed(path, source_files, instance_id).await?
        {
            if change.kind == InstanceUpgradeDependencyChangeKind::Remove
                && !skipped.iter().any(|skipped_path| skipped_path == path)
            {
                skipped.push(path.to_string());
            }
            continue;
        }
        match change.kind {
            InstanceUpgradeDependencyChangeKind::Remove => {
                crate::state::instances::commands::remove_project(
                    instance_id,
                    path,
                    state,
                )
                .await?;
                launcher_expected_files.insert(path.to_string(), None);
            }
            InstanceUpgradeDependencyChangeKind::Keep => {
                let final_path = set_upgrade_path_enabled(
                    instance_id,
                    path,
                    change.enabled,
                    state,
                )
                .await?;
                record_launcher_expected_file(
                    instance_id,
                    path,
                    &final_path,
                    &mut launcher_expected_files,
                )
                .await?;
            }
            InstanceUpgradeDependencyChangeKind::Add
            | InstanceUpgradeDependencyChangeKind::Upgrade => {}
        }
    }
    Ok(AppliedUpgradeContent {
        skipped,
        launcher_expected_files,
    })
}

#[cfg(debug_assertions)]
fn injected_upgrade_failure_after_mutations() -> Option<usize> {
    std::env::var("AXOLOTL_TEST_UPGRADE_FAIL_AFTER_MUTATIONS")
        .ok()
        .and_then(|value| debug_mutation_count(&value))
}

#[cfg(debug_assertions)]
fn injected_upgrade_pause_after_mutations() -> Option<usize> {
    std::env::var("AXOLOTL_TEST_UPGRADE_PAUSE_AFTER_MUTATIONS")
        .ok()
        .and_then(|value| debug_mutation_count(&value))
}

#[cfg(debug_assertions)]
pub(super) fn debug_mutation_count(value: &str) -> Option<usize> {
    value.trim().parse().ok().filter(|count| *count > 0)
}

async fn set_upgrade_path_enabled(
    instance_id: &str,
    relative_path: &str,
    enabled: bool,
    state: &State,
) -> crate::Result<String> {
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    if let Some(entry) = crate::state::instances::adapters::sqlite::content_entries::get_content_entry_by_relative_path(
        &scope.content_set_id,
        relative_path,
        &state.pool,
    )
    .await?
    {
        if entry.enabled != enabled {
            let toggled = crate::state::instances::commands::toggle_content_entries(
                instance_id,
                &[entry.id],
                Some(enabled),
                state,
            )
            .await?;
            if let Some(toggled) = toggled.first() {
                return Ok(toggled.path.clone());
            }
        }
        return Ok(relative_path.to_string());
    }
    crate::state::instances::commands::toggle_disable_project(
        instance_id,
        relative_path,
        Some(enabled),
        state,
    )
    .await
}

async fn collect_upgrade_external_changes(
    instance_id: &str,
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    baseline: Option<&crate::install::model::InstanceUpgradeWatchBaseline>,
    state: &State,
) -> crate::Result<Vec<InstanceUpgradeExternalChange>> {
    let current = state.file_watcher.content_watch_snapshot(instance_id).await;
    let requires_full_scan = match (baseline, current.as_ref()) {
        (Some(baseline), Some(current)) => {
            baseline.epoch != current.epoch
                || current.generation > baseline.generation
        }
        _ => true,
    };
    if requires_full_scan {
        let current_files = crate::state::instances::commands::scan_instance_upgrade_source_files(
            instance_id,
            state,
        )
        .await?;
        return Ok(diff_upgrade_source_files(source_files, &current_files));
    }
    let dirty_paths = match (baseline, current.as_ref()) {
        (Some(baseline), Some(current)) if baseline.epoch == current.epoch => {
            let baseline_paths = baseline
                .dirty_paths
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            current
                .dirty_paths
                .iter()
                .filter(|path| !baseline_paths.contains(path.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };
    let source_by_path = source_files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let base = crate::api::instance::get_full_path(instance_id).await?;
    let mut changes = Vec::new();
    for relative_path in dirty_paths {
        let path = match recovery::checked_instance_path(&base, &relative_path)
        {
            Ok(path) => path,
            Err(_) => {
                changes.push(InstanceUpgradeExternalChange {
                    relative_path,
                    kind: InstanceUpgradeExternalChangeKind::Modified,
                });
                continue;
            }
        };
        let exists = tokio::fs::symlink_metadata(&path).await.is_ok();
        let kind = classify_upgrade_external_change(
            source_by_path.contains_key(relative_path.as_str()),
            exists,
        );
        changes.push(InstanceUpgradeExternalChange {
            relative_path,
            kind,
        });
    }
    Ok(changes)
}

pub(super) fn diff_upgrade_source_files(
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    current_files: &[crate::state::InstanceUpgradeSourceFile],
) -> Vec<InstanceUpgradeExternalChange> {
    let source = source_files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let current = current_files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let mut paths = source
        .keys()
        .chain(current.keys())
        .copied()
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| match (source.get(path), current.get(path)) {
            (None, Some(_)) => Some(InstanceUpgradeExternalChange {
                relative_path: path.to_string(),
                kind: InstanceUpgradeExternalChangeKind::Added,
            }),
            (Some(_), None) => Some(InstanceUpgradeExternalChange {
                relative_path: path.to_string(),
                kind: InstanceUpgradeExternalChangeKind::Removed,
            }),
            (Some(source), Some(current))
                if source.sha1 != current.sha1
                    || source.size != current.size
                    || source.enabled != current.enabled =>
            {
                Some(InstanceUpgradeExternalChange {
                    relative_path: path.to_string(),
                    kind: InstanceUpgradeExternalChangeKind::Modified,
                })
            }
            _ => None,
        })
        .collect()
}

async fn collect_unsafe_upgrade_paths(
    instance_id: &str,
) -> crate::Result<Vec<String>> {
    let base = crate::api::instance::get_full_path(instance_id).await?;
    let resolved_base = crate::util::io::canonicalize(&base)?;
    let mut unsafe_paths = Vec::new();
    for path in
        crate::api::pack::import::get_all_subfiles(&resolved_base, false)
            .await?
    {
        let metadata = tokio::fs::symlink_metadata(&path).await?;
        if !crate::util::io::is_symlink_or_reparse(&metadata) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(&resolved_base) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if matches!(
            relative.split('/').next(),
            Some("mods" | "resourcepacks" | "shaderpacks" | "datapacks")
        ) {
            unsafe_paths.push(relative);
        }
    }
    unsafe_paths.sort_unstable();
    unsafe_paths.dedup();
    Ok(unsafe_paths)
}

async fn source_file_changed(
    relative_path: &str,
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    instance_id: &str,
) -> crate::Result<bool> {
    let Some(expected) = source_files
        .iter()
        .find(|file| file.relative_path == relative_path)
    else {
        return Ok(false);
    };
    let base = crate::api::instance::get_full_path(instance_id).await?;
    let path = match recovery::checked_instance_path(&base, relative_path) {
        Ok(path) => path,
        Err(_) => return Ok(true),
    };
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(true);
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || crate::util::io::is_symlink_or_reparse(&metadata)
        || metadata.len() != expected.size
    {
        return Ok(true);
    }
    let (_, sha1) = crate::util::fetch::sha1_file_async(&path).await?;
    Ok(sha1 != expected.sha1)
}

async fn record_launcher_expected_file(
    instance_id: &str,
    original_path: &str,
    final_path: &str,
    expected: &mut HashMap<
        String,
        Option<crate::state::InstanceUpgradeSourceFile>,
    >,
) -> crate::Result<()> {
    if original_path != final_path {
        expected.insert(original_path.to_string(), None);
    }
    expected.insert(
        final_path.to_string(),
        current_upgrade_source_file(instance_id, final_path).await?,
    );
    Ok(())
}

async fn current_upgrade_source_file(
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<Option<crate::state::InstanceUpgradeSourceFile>> {
    let base = crate::api::instance::get_full_path(instance_id).await?;
    let path = recovery::checked_instance_path(&base, relative_path)?;
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || crate::util::io::is_symlink_or_reparse(&metadata)
    {
        return Ok(None);
    }
    let (_, sha1) = crate::util::fetch::sha1_file_async(&path).await?;
    Ok(Some(crate::state::InstanceUpgradeSourceFile {
        relative_path: relative_path.to_string(),
        sha1,
        size: metadata.len(),
        enabled: !relative_path.ends_with(".disabled"),
    }))
}

pub(super) fn final_upgrade_external_changes(
    source_files: &[crate::state::InstanceUpgradeSourceFile],
    current_files: &[crate::state::InstanceUpgradeSourceFile],
    launcher_expected_files: &HashMap<
        String,
        Option<crate::state::InstanceUpgradeSourceFile>,
    >,
) -> Vec<InstanceUpgradeExternalChange> {
    diff_upgrade_source_files(source_files, current_files)
        .into_iter()
        .filter(|change| {
            let Some(expected) =
                launcher_expected_files.get(&change.relative_path)
            else {
                return true;
            };
            let current = current_files
                .iter()
                .find(|file| file.relative_path == change.relative_path);
            match (expected.as_ref(), current) {
                (None, None) => false,
                (Some(expected), Some(current)) => expected != current,
                _ => true,
            }
        })
        .collect()
}

pub(super) fn merge_upgrade_external_changes(
    changes: &mut Vec<InstanceUpgradeExternalChange>,
    additional: Vec<InstanceUpgradeExternalChange>,
) {
    for change in additional {
        if let Some(existing) = changes
            .iter_mut()
            .find(|existing| existing.relative_path == change.relative_path)
        {
            existing.kind = change.kind;
        } else {
            changes.push(change);
        }
    }
    changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
}

pub(super) fn should_create_upgrade_backup(
    requested: bool,
    mode: SharedUpgradeMode,
) -> bool {
    requested && mode == SharedUpgradeMode::Direct
}

pub(super) fn upgrade_mutation_conflicts(
    mutation: &StagedUpgradeMutation,
    external_paths: &HashSet<String>,
) -> bool {
    external_paths.contains(&mutation.target_path)
        || mutation
            .existing_path
            .as_ref()
            .is_some_and(|path| external_paths.contains(path))
}

pub(super) fn classify_upgrade_external_change(
    existed_at_start: bool,
    exists_now: bool,
) -> InstanceUpgradeExternalChangeKind {
    match (existed_at_start, exists_now) {
        (false, true) => InstanceUpgradeExternalChangeKind::Added,
        (true, false) => InstanceUpgradeExternalChangeKind::Removed,
        _ => InstanceUpgradeExternalChangeKind::Modified,
    }
}
