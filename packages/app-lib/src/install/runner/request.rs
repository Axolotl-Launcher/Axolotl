use super::super::model::InstallContentBatchItem;
use super::*;

pub(super) fn require_historical_release(
    version_id: Option<String>,
) -> crate::Result<String> {
    version_id.filter(|id| !id.is_empty()).ok_or_else(|| ErrorKind::InputError("This historical task has no saved release; prepare a new content preview instead of selecting a new version during recovery".into()).into())
}

pub(super) async fn run_request(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<InstallExecutionOutcome<Option<String>>> {
    match job_state.request.clone() {
        InstallRequest::CreateInstance {
            name,
            game_version,
            loader,
            loader_version: _,
            adjuncts,
            icon_path: _,
            link,
            game_dir_override: _,
        } => {
            let Some(instance_id) = current_instance_id(job_state) else {
                return Err(crate::ErrorKind::InputError(
                    "Install job is missing its instance id".to_string(),
                )
                .into());
            };
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::PreparingInstance,
                InstallPhaseDetails::Instance { name: name.clone() },
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            let mut parallel_minecraft_install = None;
            if let InstanceLink::CurseForgeModpack {
                project_id,
                version_id,
            } = link
            {
                let project_id = project_id.parse::<u32>().map_err(|_| {
                    ErrorKind::InputError(
                        "CurseForge project ID is invalid".to_string(),
                    )
                })?;
                let file_id = version_id.parse::<u32>().map_err(|_| {
                    ErrorKind::InputError(
                        "CurseForge file ID is invalid".to_string(),
                    )
                })?;
                crate::state::instances::commands::set_instance_install_stage(
                    &instance_id,
                    InstanceInstallStage::PackInstalling,
                    &state.pool,
                )
                .await?;
                emit_instance(&instance_id, InstancePayloadType::Edited)
                    .await?;
                parallel_minecraft_install = Some(
                    crate::api::pack::parallel_minecraft_install::ParallelMinecraftInstall::start(
                        instance_id.clone(),
                        reporter.clone(),
                    ),
                );
                let result = crate::api::curseforge::install_modpack_with_reporter(
                    crate::api::curseforge::CurseForgeModpackInstallRequest {
                        instance_id: instance_id.clone(),
                        project_id,
                        file_id,
                        install_optional: false,
                        allow_target_change: false,
                    },
                    Some(reporter.clone()),
                )
                .await?;
                if let Some(reason) = pack::curseforge_manual_download_pause(
                    &result,
                    &job_state.skipped_missing_content_paths,
                ) {
                    if let Some(minecraft_install) =
                        parallel_minecraft_install.take()
                    {
                        minecraft_install.abort().await;
                    }
                    return Ok(InstallExecutionOutcome::WaitingForUser(reason));
                }
            }
            if let Some(minecraft_install) = parallel_minecraft_install {
                minecraft_install.join().await?;
            } else {
                reporter
                    .update(
                        InstallPhaseId::DownloadingMinecraft,
                        None,
                        InstallPhaseDetails::Minecraft {
                            game_version: game_version.clone(),
                            loader,
                        },
                    )
                    .await?;
                let context =
                    crate::state::instances::commands::get_instance_launch_context(
                        &instance_id,
                        &state.pool,
                    )
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError("Unknown instance".to_string())
                    })?;
                crate::launcher::install_minecraft_with_reporter(
                    &context,
                    false,
                    Some(reporter.clone()),
                    crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
                )
                .await?;
            }
            adjunct::install_adjunct_components(
                state,
                &instance_id,
                &adjuncts,
                &game_version,
                loader,
                reporter.cancellation_token(),
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::CreateModpackInstance {
            location,
            post_install_edit,
        } => {
            let Some(instance_id) = current_instance_id(job_state) else {
                return Err(crate::ErrorKind::InputError(
                    "Install job is missing its instance id".to_string(),
                )
                .into());
            };
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::ResolvingPack,
                modpack_details(&location),
            )
            .await?;
            if let InstallExecutionOutcome::WaitingForUser(reason) =
                pack::install_pack(
                    job_id,
                    job_state,
                    location,
                    instance_id.clone(),
                    DownloadReason::Modpack,
                )
                .await?
            {
                return Ok(InstallExecutionOutcome::WaitingForUser(reason));
            }
            apply_post_install_edit(&instance_id, post_install_edit).await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::ImportInstance {
            launcher_type,
            base_path,
            instance_folder,
            instance_path,
            symlink,
            game_version,
            loader,
            loader_version,
            game_dir_override: _,
        } => {
            tracing::debug!(
                "InstallRequest::ImportInstance: launcher_type={launcher_type} base_path={} instance_folder={instance_folder} symlink={symlink}",
                base_path.display()
            );
            let Some(instance_id) = current_instance_id(job_state) else {
                return Err(crate::ErrorKind::InputError(
                    "Install job is missing its instance id".to_string(),
                )
                .into());
            };
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::PreparingInstance,
                InstallPhaseDetails::Import {
                    launcher_type,
                    instance_folder: instance_folder.clone(),
                },
            )
            .await?;
            crate::api::pack::import::import_instance_with_reporter(
                &instance_id,
                launcher_type,
                base_path,
                instance_folder,
                instance_path,
                crate::api::pack::import::ImportOverrides {
                    game_version,
                    loader,
                    loader_version,
                },
                // TODO(B2): apply overrides to launcher-specific importers
                // (MultiMC/Prism/ATLauncher/GDLauncher/Curseforge/ModrinthApp);
                // generic/PCL/HMCL/Axolotl paths already consume them.
                InstallProgressReporter::new(job_id, job_state.clone()),
                symlink,
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::DuplicateInstance { source_instance_id } => {
            let Some(instance_id) = current_instance_id(job_state) else {
                return Err(crate::ErrorKind::InputError(
                    "Install job is missing its instance id".to_string(),
                )
                .into());
            };
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::PreparingInstance,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let state = State::get().await?;
            crate::api::pack::import::copy_dotminecraft_with_reporter(
                &instance_id,
                crate::api::instance::get_full_path(&source_instance_id)
                    .await?,
                &state.io_semaphore,
                InstallProgressReporter::new(job_id, job_state.clone()),
                InstallPhaseDetails::Empty,
            )
            .await?;
            let context =
                crate::state::instances::commands::get_instance_launch_context(
                    &instance_id,
                    &state.pool,
                )
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError("Unknown instance".to_string())
                })?;
            crate::launcher::install_minecraft_with_reporter(
                &context,
                false,
                Some(InstallProgressReporter::new(job_id, job_state.clone())),
                crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::UpgradeUnmanagedInstance {
            instance_id: source_instance_id,
            plan_id,
            execution,
            create_full_backup,
            shared_upgrade_mode,
            display_names,
        } => {
            let target_instance_id = current_instance_id(job_state)
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(
                        "Upgrade job is missing its target instance id"
                            .to_string(),
                    )
                })?;
            upgrade::run_instance_upgrade(
                job_id,
                job_state,
                state,
                &source_instance_id,
                &target_instance_id,
                &plan_id,
                execution,
                create_full_backup,
                shared_upgrade_mode,
                display_names,
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(target_instance_id)))
        }
        InstallRequest::InstallExistingInstance { instance_id, force } => {
            prepare_existing_rollback(job_state, state, &instance_id).await?;
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
                    &instance_id,
                    &state.pool,
                )
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError("Unknown instance".to_string())
                })?;
            crate::launcher::install_minecraft_with_reporter(
                &context,
                force,
                Some(InstallProgressReporter::new(job_id, job_state.clone())),
                crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::InstallPackToExistingInstance {
            instance_id,
            location,
            post_install_edit,
        } => {
            prepare_existing_rollback(job_state, state, &instance_id).await?;
            let disabled_project_ids = match job_state.continuation.clone() {
                Some(InstallContinuationState::InstallingPackToExistingInstance {
                    disabled_project_ids,
                }) => disabled_project_ids.into_iter().collect(),
                Some(_) => {
                    return Err(crate::ErrorKind::InputError(
                        "Install job continuation does not match its request"
                            .to_string(),
                    )
                    .into());
                }
                None => {
                    let disabled_project_ids = remove_existing_pack_content(
                        job_id,
                        job_state,
                        state,
                        &instance_id,
                    )
                    .await?;
                    let mut persisted_ids = disabled_project_ids
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    persisted_ids.sort_unstable();
                    let continuation = InstallContinuationState::InstallingPackToExistingInstance {
                        disabled_project_ids: persisted_ids,
                    };
                    job_state.continuation = Some(continuation.clone());
                    InstallProgressReporter::new(job_id, job_state.clone())
                        .set_continuation(Some(continuation))
                        .await?;
                    disabled_project_ids
                }
            };
            if let InstallExecutionOutcome::WaitingForUser(reason) =
                pack::install_pack(
                    job_id,
                    job_state,
                    location,
                    instance_id.clone(),
                    DownloadReason::Modpack,
                )
                .await?
            {
                return Ok(InstallExecutionOutcome::WaitingForUser(reason));
            }
            restore_disabled_projects(
                &instance_id,
                disabled_project_ids,
                state,
            )
            .await?;
            job_state.continuation = None;
            InstallProgressReporter::new(job_id, job_state.clone())
                .set_continuation(None)
                .await?;
            apply_post_install_edit(&instance_id, post_install_edit).await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::InstallContent {
            instance_id,
            project_id,
            version_id,
            content_type,
            selected,
            excluded_project_ids,
            display_title: _,
            display_icon: _,
        } => {
            let version_id = Some(require_historical_release(version_id)?);
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::DownloadingContent,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let plan = crate::state::instances::commands::resolve_install_plan(
                &instance_id,
                crate::state::instances::commands::InstanceInstallProjectRequest {
                    project_id: project_id.clone(),
                    version_id,
                    content_type,
                    selected,
                    excluded_project_ids,
                    force_project_ids: Vec::new(),
                },
                state,
            )
            .await?;
            let total = (plan.dependencies.len() + 1) as u64;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            reporter
                .update(
                    InstallPhaseId::DownloadingContent,
                    Some(InstallProgress {
                        current: 0,
                        total,
                        secondary: None,
                    }),
                    InstallPhaseDetails::Empty,
                )
                .await?;
            crate::state::instances::commands::install_resolved_content_plan_with_reporter(
                &instance_id,
                &plan,
                Some(reporter.clone()),
                state,
            )
            .await?;
            reporter
                .update(
                    InstallPhaseId::DownloadingContent,
                    Some(InstallProgress {
                        current: total,
                        total,
                        secondary: None,
                    }),
                    InstallPhaseDetails::Empty,
                )
                .await?;
            crate::api::instance::emit_content_changed(&instance_id).await?;
            let dependency_project_ids = plan
                .dependencies
                .iter()
                .map(|dependency| dependency.project_id.clone())
                .collect::<Vec<_>>();
            emit_instance(
                &instance_id,
                InstancePayloadType::ContentInstallFinished {
                    project_ids: std::iter::once(project_id.clone())
                        .chain(dependency_project_ids.iter().cloned())
                        .collect(),
                    dependency_project_ids,
                },
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::InstallCurseForgeContent {
            request,
            display_title: _,
            display_icon: _,
        } => {
            let instance_id = request.instance_id.clone();
            let primary_project_id = request.project_id;
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::DownloadingContent,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            let result = crate::api::curseforge::install_file_with_reporter(
                request, reporter,
            )
            .await?;
            crate::api::instance::emit_content_changed(&instance_id).await?;
            let dependency_project_ids = result
                .installed
                .iter()
                .filter(|installed| installed.dependency)
                .map(|installed| format!("curseforge:{}", installed.project_id))
                .collect::<Vec<_>>();
            emit_instance(
                &instance_id,
                InstancePayloadType::ContentInstallFinished {
                    project_ids: std::iter::once(format!(
                        "curseforge:{primary_project_id}"
                    ))
                    .chain(dependency_project_ids.iter().cloned())
                    .collect(),
                    dependency_project_ids,
                },
            )
            .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::InstallCurseForgeWorld {
            request,
            display_title: _,
            display_icon: _,
        } => {
            let instance_id = request.instance_id.clone();
            if pack::curseforge_world_was_imported_manually(job_state, &request)
            {
                return Ok(InstallExecutionOutcome::Completed(Some(
                    instance_id,
                )));
            }
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::DownloadingContent,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            let result = crate::api::curseforge::install_world_with_reporter(
                request.clone(),
                reporter.clone(),
            )
            .await?;
            if let Some(manual_download) = result.manual_download {
                let path = format!("saves/{}", manual_download.file_name);
                let manual_url = manual_download.website_url.clone().or_else(|| {
					Some(format!(
						"https://www.curseforge.com/minecraft/worlds/{}/download/{}",
						manual_download.project_slug, manual_download.file_id
					))
				});
                reporter
                    .record_events(vec![
                        InstallJobEventKind::ContentFileSkipped {
                            path: path.clone(),
                            reason: "CurseForge requires a manual download"
                                .to_string(),
                            project_id: Some(
                                manual_download.project_id.to_string(),
                            ),
                            version_id: Some(
                                manual_download.file_id.to_string(),
                            ),
                            manual_url,
                        },
                    ])
                    .await?;
                return Ok(InstallExecutionOutcome::WaitingForUser(
                    InstallPauseReason::MissingRequiredContent {
                        failed_files: 1,
                        paths: vec![path],
                    },
                ));
            }
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::InstallContentBatch {
            instance_id, items, ..
        } => {
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::DownloadingContent,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            // Every item in this request targets the same instance. Keep the
            // plan pipeline serial so a content materialization cannot hold
            // the instance lock while another plan waits on the install DB
            // writer (or vice versa). The download helper still uses the
            // global transfer concurrency for individual files.
            let results = futures::stream::iter(items)
                .map(|item| {
                    let reporter = reporter.clone();
                    let instance_id = instance_id.clone();
                    async move {
                        match item {
                            InstallContentBatchItem::Modrinth {
                                project_id,
                                version_id,
                                content_type,
                                selected,
                                excluded_project_ids,
                                force_project_ids,
                            } => {
                                let version_id = Some(require_historical_release(version_id)?);
                                let plan = crate::state::instances::commands::resolve_install_plan(
                                    &instance_id,
                                    crate::state::instances::commands::InstanceInstallProjectRequest {
                                        project_id: project_id.clone(),
                                        version_id,
                                        content_type,
                                        selected,
                                        excluded_project_ids,
                                        force_project_ids,
                                    },
                                    state,
                                ).await?;
                                crate::state::instances::commands::install_resolved_content_plan_with_reporter(
                                    &instance_id, &plan, Some(reporter.clone()), state,
                                ).await?;
                                Ok::<Option<InstallPauseReason>, crate::Error>(None)
                            }
                            InstallContentBatchItem::CurseForge { request } => {
                                let result = crate::api::curseforge::install_file_with_reporter(
                                    request, reporter.clone(),
                                ).await?;
                                Ok(if result.manual_downloads.is_empty() {
                                    None
                                } else {
                                    Some(InstallPauseReason::MissingRequiredContent {
                                        failed_files: result.manual_downloads.len() as u64,
                                        paths: result.manual_downloads.iter().map(|download| download.file_name.clone()).collect(),
                                    })
                                })
                            }
                            InstallContentBatchItem::CurseForgeWorld { request } => {
                                let result = crate::api::curseforge::install_world_with_reporter(
                                    request, reporter,
                                ).await?;
                                Ok(result.manual_download.map(|download| InstallPauseReason::MissingRequiredContent {
                                    failed_files: 1,
                                    paths: vec![format!("saves/{}", download.file_name)],
                                }))
                            }
                            InstallContentBatchItem::FrozenResource { plan } => {
                                Box::pin(crate::resources::plans::execute_frozen_plan(
                                    &instance_id,
                                    &plan,
                                    Some(reporter.clone()),
                                    state,
                                )).await?;
                                Ok(None)
                            }
                        }
                    }
                })
                .buffer_unordered(1)
                .collect::<Vec<_>>()
                .await;
            let mut pause_reason = None;
            for result in results {
                if let Some(reason) = result? {
                    pause_reason = Some(reason);
                }
            }
            if let Some(reason) = pause_reason {
                return Ok(InstallExecutionOutcome::WaitingForUser(reason));
            }
            crate::api::instance::emit_content_changed(&instance_id).await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::ChangeContent {
            instance_id,
            intent,
            ..
        } => {
            run_content_change(job_id, job_state, &instance_id, &intent)
                .await?;
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::UpdateManagedCurseForgeModpack {
            instance_id,
            file_id,
        } => {
            prepare_existing_rollback(job_state, state, &instance_id).await?;
            crate::state::instances::commands::set_instance_install_stage(
                &instance_id,
                InstanceInstallStage::PackInstalling,
                &state.pool,
            )
            .await?;
            emit_instance(&instance_id, InstancePayloadType::Edited).await?;
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::DownloadingContent,
                InstallPhaseDetails::Empty,
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            let result =
                crate::api::curseforge::update_managed_modpack_with_reporter(
                    &instance_id,
                    file_id,
                    Some(reporter.clone()),
                )
                .await?;
            if !result.content.failed_downloads.is_empty() {
                return Err(ErrorKind::NetworkError(format!(
                    "{} CurseForge files could not be downloaded automatically",
                    result.content.failed_downloads.len()
                ))
                .into());
            }
            if let Some(reason) = pack::curseforge_manual_download_pause(
                &result,
                &job_state.skipped_missing_content_paths,
            ) {
                return Ok(InstallExecutionOutcome::WaitingForUser(reason));
            }
            Ok(InstallExecutionOutcome::Completed(Some(instance_id)))
        }
        InstallRequest::ExportResource {
            plan,
            output_path,
            resolution,
        } => {
            if let Some(checkpoint) = resolution {
                export::resolve(
                    job_id,
                    job_state,
                    &plan,
                    &output_path,
                    *checkpoint,
                )
                .await?;
            } else {
                export::run(job_id, job_state, &plan, &output_path).await?;
            }
            Ok(InstallExecutionOutcome::Completed(Some(plan.instance_id)))
        }
        InstallRequest::DownloadJava { vendor, version } => {
            update_progress(
                job_id,
                job_state,
                state,
                InstallPhaseId::PreparingJava,
                InstallPhaseDetails::Java {
                    major_version: version,
                    step: InstallJavaStep::FetchingMetadata,
                },
            )
            .await?;
            let reporter =
                InstallProgressReporter::new(job_id, job_state.clone());
            let path = crate::api::jre::download_java_from_feed_with_reporter(
                &vendor, version, reporter,
            )
            .await?;
            let _ = path;
            Ok(InstallExecutionOutcome::Completed(None))
        }
    }
}

async fn run_content_change(
    job_id: uuid::Uuid,
    job_state: &mut InstallJobState,
    instance_id: &str,
    intent: &crate::install::ContentChangeIntent,
) -> crate::Result<()> {
    content_change::run(job_id, job_state, instance_id, intent).await
}
