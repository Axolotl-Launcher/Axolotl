use super::*;

pub(super) async fn resolve_required_adjuncts(
    game_version: &str,
    loader: ModLoader,
    adjuncts: &mut Vec<LoaderComponent>,
    _state: &State,
) -> crate::Result<()> {
    for adjunct in adjuncts.iter() {
        match adjunct.kind {
            LoaderComponentKind::OptiFine
                if !matches!(
                    loader,
                    ModLoader::Forge
                        | ModLoader::NeoForge
                        | ModLoader::Fabric
                        | ModLoader::LegacyFabric
                ) =>
            {
                return Err(ErrorKind::InputError(format!(
                    "OptiFine is not supported with {}",
                    loader.as_str()
                ))
                .into());
            }
            LoaderComponentKind::LiteLoader if loader != ModLoader::Forge => {
                return Err(ErrorKind::InputError(format!(
                    "LiteLoader is not supported with {}",
                    loader.as_str()
                ))
                .into());
            }
            _ => {}
        }
    }
    for adjunct in adjuncts.iter_mut() {
        adjunct.role = LoaderComponentRole::Adjunct;
        adjunct.instance_id.clear();
        match adjunct.kind {
            LoaderComponentKind::OptiFine => {
                let resolved =
                    crate::launcher::optifine::resolve_loader_version(
                        game_version,
                        adjunct.version.as_deref(),
                    )
                    .await?
                    .ok_or_else(|| {
                        ErrorKind::InputError(format!(
                            "No OptiFine version supports Minecraft {game_version}"
                        ))
                    })?;
                adjunct.version = Some(resolved.id);
            }
            LoaderComponentKind::LiteLoader => {
                let resolved =
                    crate::launcher::get_loader_version_from_profile(
                        game_version,
                        ModLoader::LiteLoader,
                        adjunct.version.as_deref(),
                    )
                    .await?
                    .ok_or_else(|| {
                        ErrorKind::InputError(format!(
                            "No LiteLoader version supports Minecraft {game_version}"
                        ))
                    })?;
                adjunct.version = Some(resolved.id);
            }
            _ => {}
        }
    }
    if adjuncts
        .iter()
        .any(|component| component.kind == LoaderComponentKind::OptiFine)
        && matches!(loader, ModLoader::Fabric | ModLoader::LegacyFabric)
        && !adjuncts
            .iter()
            .any(|component| component.kind == LoaderComponentKind::OptiFabric)
    {
        let version_id = resolve_optifabric_version(game_version).await?;
        adjuncts.push(LoaderComponent {
            instance_id: String::new(),
            kind: LoaderComponentKind::OptiFabric,
            version: Some(version_id),
            role: LoaderComponentRole::Adjunct,
            provider_metadata: Some(serde_json::json!({
                "projectId": OPTIFABRIC_CURSEFORGE_PROJECT_ID,
                "provider": "curseforge"
            })),
        });
    }
    let mut components =
        vec![LoaderComponent::new_primary(String::new(), loader, None)];
    components.extend(adjuncts.iter().cloned());
    validate_loader_components(&components)
}

pub(crate) fn validate_loader_components(
    components: &[LoaderComponent],
) -> crate::Result<()> {
    let primary = components
        .iter()
        .find(|component| component.role == LoaderComponentRole::Primary)
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Loader selection has no primary loader".to_string(),
            )
        })?;
    let has = |kind| {
        components.iter().any(|component| {
            component.role == LoaderComponentRole::Adjunct
                && component.kind == kind
        })
    };
    if has(LoaderComponentKind::OptiFine) {
        match primary.kind {
            LoaderComponentKind::Vanilla => {}
            LoaderComponentKind::Forge | LoaderComponentKind::NeoForge => {}
            LoaderComponentKind::Fabric | LoaderComponentKind::LegacyFabric
                if has(LoaderComponentKind::OptiFabric) => {}
            _ => {
                return Err(ErrorKind::InputError(format!(
                    "OptiFine is not supported with {}",
                    primary.kind.as_str()
                ))
                .into());
            }
        }
    }
    if has(LoaderComponentKind::OptiFabric)
        && !has(LoaderComponentKind::OptiFine)
    {
        return Err(ErrorKind::InputError(
            "OptiFabric can only be installed with OptiFine".to_string(),
        )
        .into());
    }
    if has(LoaderComponentKind::OptiFabric)
        && !matches!(
            primary.kind,
            LoaderComponentKind::Fabric | LoaderComponentKind::LegacyFabric
        )
    {
        return Err(ErrorKind::InputError(format!(
            "OptiFabric is not supported with {}",
            primary.kind.as_str()
        ))
        .into());
    }
    if has(LoaderComponentKind::LiteLoader)
        && !matches!(
            primary.kind,
            LoaderComponentKind::Vanilla | LoaderComponentKind::Forge
        )
    {
        return Err(ErrorKind::InputError(format!(
            "LiteLoader is not supported with {}",
            primary.kind.as_str()
        ))
        .into());
    }
    if components.iter().any(|component| {
        component.role == LoaderComponentRole::Adjunct
            && !matches!(
                component.kind,
                LoaderComponentKind::OptiFine
                    | LoaderComponentKind::LiteLoader
                    | LoaderComponentKind::OptiFabric
            )
    }) {
        return Err(ErrorKind::InputError(
            "Only OptiFine, LiteLoader, and OptiFabric can be adjunct loaders"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

pub(crate) async fn resolve_optifabric_version(
    game_version: &str,
) -> crate::Result<String> {
    let files = crate::api::curseforge::get_files(
        OPTIFABRIC_CURSEFORGE_PROJECT_ID,
        crate::api::curseforge::CurseForgeFilesRequest {
            game_version: None,
            mod_loader_type: None,
            game_version_type_id: None,
            index: 0,
            page_size: 50,
        },
    )
    .await?
    .files;
    select_optifabric_file_id(&files, game_version)
        .map(|file_id| file_id.to_string())
        .ok_or_else(|| {
            ErrorKind::InputError(format!(
                "OptiFine requires OptiFabric, but no OptiFabric version supports Minecraft {game_version}"
            ))
            .into()
        })
}

pub(super) fn select_optifabric_file_id(
    files: &[crate::api::curseforge::CurseForgeFile],
    game_version: &str,
) -> Option<u32> {
    files
        .iter()
        .find(|file| {
            file.is_available
                && file
                    .game_versions
                    .iter()
                    .any(|version| version == game_version)
        })
        .map(|file| file.id)
}

pub(crate) async fn install_optifabric_file(
    instance_id: &str,
    game_version: &str,
    version: &str,
) -> crate::Result<String> {
    let file_id = version.parse::<u32>().map_err(|_| {
        ErrorKind::InputError(
            "OptiFabric CurseForge file ID is invalid".to_string(),
        )
    })?;
    let file = crate::api::curseforge::get_file(
        OPTIFABRIC_CURSEFORGE_PROJECT_ID,
        file_id,
    )
    .await?;
    if file.mod_id != OPTIFABRIC_CURSEFORGE_PROJECT_ID
        || !file.is_available
        || !file
            .game_versions
            .iter()
            .any(|version| version == game_version)
    {
        return Err(ErrorKind::InputError(format!(
            "OptiFabric file {file_id} does not support Minecraft {game_version}"
        ))
        .into());
    }

    let result = crate::api::curseforge::install_file(
        crate::api::curseforge::CurseForgeInstallRequest {
            instance_id: instance_id.to_string(),
            project_id: OPTIFABRIC_CURSEFORGE_PROJECT_ID,
            file_id,
            project_type: "mod".to_string(),
            ownership_kind: crate::state::instances::ContentOwnershipKind::UserAdded,
            manual_operation_kind:
                crate::state::instances::ManualDownloadOperationKind::ContentInstall,
            game_version: Some(game_version.to_string()),
            mod_loader_type: Some(4),
            world_name: None,
            install_dependencies: false,
            excluded_dependency_project_ids: Vec::new(),
            force_dependency_project_ids: Vec::new(),
            dependency_plan_id: None,
            defer_persistence: false,
            verification_tx: None,
            pre_resolved_relative_path: None,
                    expected_file_name: None,
        },
    )
    .await?;
    if !result.manual_downloads.is_empty() {
        return Err(ErrorKind::InputError(
            "OptiFabric requires a manual CurseForge download".to_string(),
        )
        .into());
    }
    if let Some(failure) = result.failed_downloads.first() {
        return Err(ErrorKind::InputError(format!(
            "Failed to install OptiFabric: {}",
            failure.reason
        ))
        .into());
    }
    if !result.installed.iter().any(|installed| {
        !installed.dependency
            && installed.project_id == OPTIFABRIC_CURSEFORGE_PROJECT_ID
            && installed.file_id == file_id
    }) {
        return Err(ErrorKind::InputError(
            "OptiFabric was not installed".to_string(),
        )
        .into());
    }
    Ok(file_id.to_string())
}

pub(super) async fn install_adjunct_components(
    state: &State,
    instance_id: &str,
    adjuncts: &[LoaderComponent],
    game_version: &str,
    loader: ModLoader,
    cancellation: tokio_util::sync::CancellationToken,
) -> crate::Result<()> {
    if adjuncts.is_empty() {
        return Ok(());
    }
    let metadata = crate::api::instance::get(instance_id)
        .await?
        .ok_or_else(|| ErrorKind::InputError("Unknown instance".to_string()))?;
    let instance_path = state.directories.instance_game_dir(&metadata.instance);
    let mut components = metadata.loader_components.clone();

    for adjunct in adjuncts {
        match adjunct.kind {
            LoaderComponentKind::OptiFine => {
                let version = crate::launcher::optifine::resolve_loader_version(
                    game_version,
                    adjunct.version.as_deref(),
                )
                .await?
                .ok_or_else(|| {
                    ErrorKind::InputError(format!(
                        "No OptiFine version supports Minecraft {game_version}"
                    ))
                })?;
                crate::api::pack::install_mcbbs::install_optifine_mod(
                    state,
                    instance_id,
                    cancellation.clone(),
                    game_version,
                    &version.id,
                    &instance_path,
                )
                .await?;
                set_component_version(
                    &mut components,
                    LoaderComponentKind::OptiFine,
                    version.id,
                );
            }
            LoaderComponentKind::OptiFabric => {
                let version_id = match &adjunct.version {
                    Some(version) => version.clone(),
                    None => resolve_optifabric_version(game_version).await?,
                };
                let version_id = install_optifabric_file(
                    instance_id,
                    game_version,
                    &version_id,
                )
                .await?;
                set_component_version(
                    &mut components,
                    LoaderComponentKind::OptiFabric,
                    version_id,
                );
            }
            LoaderComponentKind::LiteLoader => {
                let version = install_liteloader_adjunct(
                    state,
                    &metadata,
                    game_version,
                    loader,
                    adjunct.version.as_deref(),
                )
                .await?;
                set_component_version(
                    &mut components,
                    LoaderComponentKind::LiteLoader,
                    version,
                );
            }
            _ => {}
        }
    }
    crate::state::instances::commands::replace_instance_loader_components(
        instance_id,
        &components,
        &state.pool,
    )
    .await
}

fn set_component_version(
    components: &mut [LoaderComponent],
    kind: LoaderComponentKind,
    version: String,
) {
    if let Some(component) = components
        .iter_mut()
        .find(|component| component.kind == kind)
    {
        component.version = Some(version);
    }
}

pub(crate) async fn install_liteloader_adjunct(
    state: &State,
    metadata: &crate::state::InstanceMetadata,
    game_version: &str,
    primary_loader: ModLoader,
    requested_version: Option<&str>,
) -> crate::Result<String> {
    let version = crate::launcher::get_loader_version_from_profile(
        game_version,
        ModLoader::LiteLoader,
        requested_version,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(format!(
            "No LiteLoader version supports Minecraft {game_version}"
        ))
    })?;
    install_liteloader_adjunct_resolved(
        state,
        metadata,
        game_version,
        primary_loader,
        &version,
    )
    .await
}

pub(crate) async fn install_liteloader_adjunct_resolved(
    state: &State,
    metadata: &crate::state::InstanceMetadata,
    game_version: &str,
    primary_loader: ModLoader,
    version: &daedalus::modded::LoaderVersion,
) -> crate::Result<String> {
    let partial = crate::api::loader_metadata::resolve_loader_profile(
        state,
        game_version,
        version,
    )
    .await?;
    let primary_version = metadata
        .applied_content_set
        .loader_version
        .as_deref()
        .ok_or_else(|| {
            ErrorKind::InputError(format!(
                "{} adjunct installation requires a pinned primary version",
                primary_loader.as_str()
            ))
        })?;
    let version_id = format!("{game_version}-{primary_version}");
    let path = state
        .directories
        .version_dir(&version_id)
        .join(format!("{version_id}.json"));
    let bytes = crate::util::io::read(&path).await?;
    let primary: daedalus::minecraft::VersionInfo =
        serde_json::from_slice(&bytes)?;
    let mut merged = daedalus::modded::merge_partial_version(partial, primary);
    merged.id.clone_from(&version_id);
    crate::launcher::download::download_libraries(
        state,
        None,
        &merged.libraries,
        &version_id,
        None,
        0.0,
        std::env::consts::ARCH,
        false,
        false,
        None,
    )
    .await?;
    crate::util::io::write(&path, serde_json::to_vec(&merged)?).await?;
    Ok(version.id.clone())
}
