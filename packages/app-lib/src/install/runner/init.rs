use super::*;

pub(super) async fn prepare_initial_instance(
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<()> {
    match job_state.request.clone() {
        InstallRequest::CreateInstance {
            name,
            mut game_version,
            mut loader,
            mut loader_version,
            mut adjuncts,
            icon_path,
            link,
            game_dir_override,
        } => {
            if let InstanceLink::CurseForgeModpack {
                project_id,
                version_id,
            } = &link
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
                let target = crate::api::curseforge::get_modpack_target(
                    project_id, file_id,
                )
                .await?;
                game_version = target.game_version;
                loader = target.loader;
                loader_version = target.loader_version;
                adjuncts.clear();
                job_state.request = InstallRequest::CreateInstance {
                    name: name.clone(),
                    game_version: game_version.clone(),
                    loader,
                    loader_version: loader_version.clone(),
                    adjuncts: Vec::new(),
                    icon_path: icon_path.clone(),
                    link: link.clone(),
                    game_dir_override: game_dir_override.clone(),
                };
            }
            adjunct::resolve_required_adjuncts(
                &game_version,
                loader,
                &mut adjuncts,
                state,
            )
            .await?;
            job_state.request = InstallRequest::CreateInstance {
                name: name.clone(),
                game_version: game_version.clone(),
                loader,
                loader_version: loader_version.clone(),
                adjuncts: adjuncts.clone(),
                icon_path: icon_path.clone(),
                link: link.clone(),
                game_dir_override: game_dir_override.clone(),
            };
            let metadata = crate::api::instance::create(
                name,
                game_version,
                loader,
                loader_version,
                icon_path,
                link,
                None,
                game_dir_override,
            )
            .await?;
            if !adjuncts.is_empty() {
                let mut components = metadata.loader_components.clone();
                for adjunct in &mut adjuncts {
                    adjunct.instance_id = metadata.instance.id.clone();
                    adjunct.role = crate::state::LoaderComponentRole::Adjunct;
                }
                components.extend(adjuncts);
                adjunct::validate_loader_components(&components)?;
                crate::state::instances::commands::replace_instance_loader_components(
                    &metadata.instance.id,
                    &components,
                    &state.pool,
                )
                .await?;
            }
            set_display(
                job_state,
                metadata.instance.name,
                metadata.instance.icon_path,
            );
            set_instance_id(job_state, metadata.instance.id);
        }
        InstallRequest::CreateModpackInstance {
            location,
            post_install_edit,
        } => {
            let preview = get_instance_from_pack(location).await?;
            let name = post_install_edit
                .as_ref()
                .and_then(|edit| edit.name.clone())
                .unwrap_or_else(|| preview.name.clone());
            let icon_path = match post_install_edit
                .as_ref()
                .and_then(|edit| edit.icon_path.as_ref())
            {
                Some(icon_path) => icon_path.clone(),
                None => preview
                    .icon
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string())
                    .or_else(|| preview.icon_url.clone()),
            };
            let link = post_install_edit
                .as_ref()
                .and_then(|edit| edit.link.clone())
                .or_else(|| preview.link.clone())
                .unwrap_or(InstanceLink::Unmanaged);
            let metadata = crate::api::instance::create(
                name,
                preview.game_version,
                preview.modloader,
                preview.loader_version,
                icon_path,
                link,
                None,
                None,
            )
            .await?;
            set_display(
                job_state,
                metadata.instance.name,
                metadata.instance.icon_path,
            );
            set_instance_id(job_state, metadata.instance.id);
        }
        InstallRequest::ImportInstance {
            instance_folder,
            game_dir_override,
            ..
        } => {
            let metadata = crate::api::instance::create(
                instance_folder,
                "unknown".to_string(),
                ModLoader::Vanilla,
                None,
                None,
                InstanceLink::Unmanaged,
                None,
                game_dir_override,
            )
            .await?;
            set_display(
                job_state,
                metadata.instance.name,
                metadata.instance.icon_path,
            );
            set_instance_id(job_state, metadata.instance.id);
        }
        InstallRequest::DuplicateInstance { source_instance_id } => {
            let metadata =
                crate::state::get_instance(&source_instance_id, &state.pool)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "Unknown instance".to_string(),
                        )
                    })?;
            let created = crate::api::instance::create(
                metadata.instance.name,
                metadata.applied_content_set.game_version,
                metadata.applied_content_set.loader,
                metadata.applied_content_set.loader_version,
                metadata.instance.icon_path,
                metadata.link,
                None,
                None,
            )
            .await?;
            set_display(
                job_state,
                created.instance.name,
                created.instance.icon_path,
            );
            set_instance_id(job_state, created.instance.id);
        }
        InstallRequest::UpgradeUnmanagedInstance {
            instance_id,
            shared_upgrade_mode,
            display_names,
            ..
        } => {
            let metadata =
                crate::state::get_instance(&instance_id, &state.pool)
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "Unknown upgrade source instance".to_string(),
                        )
                    })?;
            set_display(
                job_state,
                metadata.instance.name.clone(),
                metadata.instance.icon_path.clone(),
            );
            match shared_upgrade_mode {
                SharedUpgradeMode::Direct => {
                    prepare_existing_rollback(job_state, state, &instance_id)
                        .await?;
                }
                SharedUpgradeMode::CopyAndUpgrade => {
                    let created = crate::api::instance::create(
                        display_names.copy.unwrap_or_else(|| {
                            format!(
                                "{} (Upgraded Copy)",
                                metadata.instance.name
                            )
                        }),
                        metadata.applied_content_set.game_version.clone(),
                        metadata.applied_content_set.loader,
                        metadata.applied_content_set.loader_version.clone(),
                        metadata.instance.icon_path.clone(),
                        InstanceLink::Unmanaged,
                        None,
                        None,
                    )
                    .await?;
                    set_instance_id(job_state, created.instance.id.clone());
                    if let Err(error) =
                        upgrade::clone_instance_loader_components(
                            &metadata.loader_components,
                            &created.instance.id,
                            state,
                        )
                        .await
                    {
                        return Err(cleanup_failed_initial_install(
                            job_state, state, error,
                        )
                        .await);
                    }
                }
            }
        }
        InstallRequest::InstallExistingInstance { instance_id, .. }
        | InstallRequest::InstallPackToExistingInstance {
            instance_id, ..
        }
        | InstallRequest::UpdateManagedCurseForgeModpack {
            instance_id, ..
        } => {
            prepare_existing_rollback(job_state, state, &instance_id).await?;
        }
        InstallRequest::InstallContent {
            instance_id,
            display_title,
            display_icon,
            ..
        } => {
            crate::state::get_instance(&instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown instance {instance_id}"
                    ))
                })?;
            set_display(job_state, display_title, display_icon);
        }
        InstallRequest::InstallCurseForgeContent {
            request,
            display_title,
            display_icon,
        } => {
            crate::state::get_instance(&request.instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown instance {}",
                        request.instance_id
                    ))
                })?;
            set_display(job_state, display_title, display_icon);
        }
        InstallRequest::InstallCurseForgeWorld {
            request,
            display_title,
            display_icon,
        } => {
            crate::state::get_instance(&request.instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown instance {}",
                        request.instance_id
                    ))
                })?;
            set_display(job_state, display_title, display_icon);
        }
        InstallRequest::InstallContentBatch {
            instance_id,
            display_title,
            display_icon,
            ..
        } => {
            crate::state::get_instance(&instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown instance {instance_id}"
                    ))
                })?;
            set_display(job_state, display_title, display_icon);
        }
        InstallRequest::ChangeContent {
            instance_id,
            display_title,
            display_icon,
            ..
        } => {
            crate::state::get_instance(&instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown instance {instance_id}"
                    ))
                })?;
            set_display(job_state, display_title, display_icon);
        }
        InstallRequest::ExportResource { plan, .. } => {
            crate::state::get_instance(&plan.instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    ErrorKind::InputError("Unknown export instance".into())
                })?;
            set_display(job_state, plan.package_info.name, None);
        }
        InstallRequest::DownloadJava { vendor, version } => {
            set_display(job_state, format!("Java {version} ({vendor})"), None);
        }
    }

    Ok(())
}
