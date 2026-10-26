//! CurseForge modpack install local.

use super::*;
use crate::api::curseforge::archive::{
    loader_type, materialize_modpack_overrides, modpack_target,
    modpack_zip_error, read_modpack_manifest,
    settle_materialized_modpack_overrides,
};
use crate::api::curseforge::catalog::{
    get_modpack_files, get_modpack_projects,
};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::{
    CurseForgeFile, CurseForgeInstallRequest, CurseForgeInstallResult,
    CurseForgeInstalledFile, CurseForgeManifestFile,
    CurseForgeModpackInstallResult, CurseForgeProject,
};
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::selection::{
    content_target_folder, managed_project_type, manual_download_from_file,
    persist_manual_download,
};
use crate::api::curseforge::validation::validate_file_name;
use crate::api::curseforge::verify::{
    DownloadInstalledFileRequest, download_installed_file,
};
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    ContentSourceKind, EditInstance, InstanceLink, ModLoader, Settings,
};
use crate::{ErrorKind, State};
use std::collections::HashSet;
use std::sync::Arc;

pub(super) async fn install_preloaded_modpack_file(
    request: &CurseForgeInstallRequest,
    project: &CurseForgeProject,
    file: &CurseForgeFile,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
) -> crate::Result<CurseForgeInstallResult> {
    if project.id != request.project_id
        || file.mod_id != request.project_id
        || file.id != request.file_id
    {
        return Err(ErrorKind::InputError(
            "Preloaded CurseForge metadata does not match the manifest file"
                .to_string(),
        )
        .into());
    }
    let project_type = managed_project_type(&request.project_type)?;
    let download_url = resolve_curseforge_download_url(
        request.project_id,
        request.file_id,
        project,
        file,
    )
    .await?;
    let Some(download_url) = download_url else {
        let target_folder =
            content_target_folder(project_type, request.world_name.as_deref())?;
        let manual_download = manual_download_from_file(
            request.project_id,
            request.file_id,
            file,
            project,
            project_type.get_name(),
            target_folder,
            request.ownership_kind,
            request.manual_operation_kind,
        );
        persist_manual_download(&request.instance_id, &manual_download).await?;
        return Ok(CurseForgeInstallResult {
            manual_downloads: vec![manual_download],
            ..Default::default()
        });
    };
    validate_file_name(&file.file_name)?;
    let downloaded = download_installed_file(DownloadInstalledFileRequest {
        instance_id: &request.instance_id,
        url: &download_url,
        file,
        project_type,
        world_name: request.world_name.as_deref(),
        project_id: request.project_id,
        file_id: request.file_id,
        project_slug: &project.slug,
        ownership_kind: request.ownership_kind,
        download_metrics,
        defer_persistence: request.defer_persistence,
        verification_tx: request.verification_tx.as_ref(),
        pre_resolved_relative_path: request
            .pre_resolved_relative_path
            .as_deref(),
        expected_file_name: request.expected_file_name.as_deref(),
    })
    .await?;
    Ok(CurseForgeInstallResult {
        installed: vec![CurseForgeInstalledFile {
            project_id: request.project_id,
            file_id: request.file_id,
            relative_path: downloaded.relative_path,
            dependency: false,
        }],
        ..Default::default()
    })
}

pub(super) fn install_error_chain(error: &crate::Error) -> String {
    let mut chain = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        chain.push_str("\nCaused by: ");
        chain.push_str(&cause.to_string());
        source = cause.source();
    }
    chain
}

/// Installs a CurseForge modpack from a local archive on disk (a zip with a
/// `manifest.json`), downloading the listed files through the CurseForge API
/// and extracting the overrides folder. Unlike [`install_modpack_with_reporter`]
/// this needs no project/file id — undownloadable files land on the manual
/// download list exactly like API-driven installs.
pub async fn install_modpack_from_local_archive_with_reporter(
    instance_id: String,
    archive_path: std::path::PathBuf,
    _base_folder: String,
    source_filename: Option<String>,
    install_optional: bool,
    reporter: InstallProgressReporter,
    completion_policy: crate::launcher::InstanceCompletionPolicy,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let manifest_archive_path = archive_path.clone();
    let manifest = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&manifest_archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        read_modpack_manifest(&mut archive)
    })
    .await??;

    let target = modpack_target(&manifest)?;
    let loader = (target.loader != ModLoader::Vanilla)
        .then(|| target.loader.as_str().to_string());
    let pack_name = manifest
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            source_filename.as_ref().map(|name| {
                std::path::Path::new(name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
        })
        .unwrap_or_else(|| "CurseForge Modpack".to_string());
    let pack_details = InstallPhaseDetails::Modpack {
        project_id: None,
        version_id: None,
        title: Some(pack_name.clone()),
    };
    reporter
        .update(InstallPhaseId::ResolvingPack, None, pack_details.clone())
        .await?;

    let loader_version = if target.loader != ModLoader::Vanilla {
        crate::launcher::get_loader_version_from_profile(
            &manifest.minecraft.version,
            target.loader,
            target.loader_version.as_deref(),
        )
        .await?
    } else {
        None
    };

    crate::api::instance::edit(
        &instance_id,
        EditInstance {
            install_stage: Some(
                crate::state::InstanceInstallStage::PackInstalling,
            ),
            name: Some(pack_name.clone()),
            link: Some(InstanceLink::ImportedModpack {
                project_id: None,
                version_id: None,
                name: Some(pack_name.clone()),
                version_number: manifest.version.clone(),
                filename: source_filename,
            }),
            content_set_patch: Some(crate::state::AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::CurseForge),
                game_version: Some(manifest.minecraft.version.clone()),
                protocol_version: Some(None),
                loader: Some(target.loader),
                loader_version: Some(loader_version.map(|version| version.id)),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    let minecraft_install = (completion_policy
        == crate::launcher::InstanceCompletionPolicy::DeferToInstallJob)
        .then(|| {
            crate::api::pack::parallel_minecraft_install::ParallelMinecraftInstall::start(
                instance_id.clone(),
                reporter.clone(),
            )
        });

    let content = install_local_manifest_files(
        &instance_id,
        manifest.files.clone(),
        install_optional,
        &manifest.minecraft.version,
        loader.as_deref(),
        pack_details.clone(),
        &reporter,
    )
    .await?;

    if !content.manual_downloads.is_empty() {
        if let Some(minecraft_install) = minecraft_install {
            minecraft_install.abort().await;
        }
        return Ok(CurseForgeModpackInstallResult {
            content,
            overrides_written: 0,
            minecraft_version: manifest.minecraft.version,
            loader,
        });
    }

    reporter
        .update(
            InstallPhaseId::ExtractingOverrides,
            None,
            pack_details.clone(),
        )
        .await?;
    let instance_path =
        crate::api::instance::get_full_path(&instance_id).await?;
    let overrides_archive_path = archive_path.clone();
    let override_cancellation = reporter.cancellation_token();
    let (overrides_written, override_replacements) =
        crate::api::pack::archive_util::run_cancellable_blocking_instance_write(
            instance_id.clone(),
            override_cancellation.clone(),
            move |cancellation| {
                materialize_modpack_overrides(
                    &overrides_archive_path,
                    &instance_path,
                    Some(cancellation),
                )
            },
        )
        .await?;

    let post_override_result: crate::Result<()> = async {
        if let Some(minecraft_install) = minecraft_install {
            minecraft_install.join().await?;
        } else {
            crate::launcher::install_minecraft_for_instance_id_with_reporter(
                &instance_id,
                false,
                Some(reporter.clone()),
                completion_policy,
            )
            .await?;
        }
        reporter.clear_context().await?;
        Ok(())
    }
    .await;
    match post_override_result {
        Ok(()) => {
            settle_materialized_modpack_overrides(
                instance_id.clone(),
                override_cancellation,
                override_replacements,
                true,
            )
            .await?;
        }
        Err(error) => {
            if let Err(rollback_error) = settle_materialized_modpack_overrides(
                instance_id.clone(),
                override_cancellation,
                override_replacements,
                false,
            )
            .await
            {
                return Err(ErrorKind::OtherError(format!(
                    "{error}; failed to restore CurseForge ZIP overrides: {rollback_error}"
                ))
                .into());
            }
            return Err(error);
        }
    }

    Ok(CurseForgeModpackInstallResult {
        content,
        overrides_written,
        minecraft_version: manifest.minecraft.version,
        loader,
    })
}

/// Downloads and installs the files listed in a local CurseForge manifest.
/// Mirrors the file loop of [`install_modpack_with_reporter`], simplified to
/// the job-reporter path used by local imports. Also used by the MCBBS
/// installer for its CurseForge-style `files` array.
pub(crate) async fn install_local_manifest_files(
    instance_id: &str,
    manifest_files: Vec<CurseForgeManifestFile>,
    install_optional: bool,
    minecraft_version: &str,
    loader: Option<&str>,
    pack_details: InstallPhaseDetails,
    reporter: &InstallProgressReporter,
) -> crate::Result<CurseForgeInstallResult> {
    let reporter = &reporter.clone().with_pipeline_cancellation();
    let state = State::get().await?;
    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "Instance has no applied content set".to_string(),
        )
    })?;
    let installed_releases = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
        &content_set.id,
        &state.pool,
    )
    .await?
    .into_iter()
    .filter(|member| {
        member.materialization_state
            == crate::state::instances::PackMemberMaterializationState::Present
    })
    .filter_map(|member| {
        Some((member.provider_project_id?, member.provider_release_id?))
    })
    .collect::<HashSet<_>>();
    let selected_files = manifest_files
        .into_iter()
        .filter(|file| file.required || install_optional)
        .filter(|file| {
            !installed_releases.contains(&(
                file.project_id.to_string(),
                file.file_id.to_string(),
            ))
        })
        .collect::<Vec<_>>();
    let loader_type_value = loader.and_then(loader_type);
    let project_ids = selected_files
        .iter()
        .map(|file| file.project_id)
        .collect::<Vec<_>>();
    let projects = get_modpack_projects(project_ids).await?;

    let total_files = selected_files.len().max(1);
    let file_ids = selected_files
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let file_meta = get_modpack_files(file_ids).await?;
    let existing_relative_paths = Arc::new(
        crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
            instance_id,
            &state.pool,
        )
        .await?
        .into_iter()
        .map(|file| file.relative_path)
        .collect::<HashSet<_>>(),
    );
    let prefer_localized_names =
        Settings::get(&state.pool).await?.resource_chinese_filenames;
    let content_total_bytes = selected_files
        .iter()
        .map(|file| {
            file_meta
                .get(&file.file_id)
                .map(|meta| meta.file_length)
                .unwrap_or(0)
        })
        .sum::<u64>();
    reporter
        .update_with_events(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: total_files as u64,
                secondary: Some(InstallProgressSecondary {
                    current: 0,
                    total: content_total_bytes,
                }),
            }),
            pack_details.clone(),
            vec![InstallJobEventKind::ContentDownloadStarted {
                files: total_files as u64,
                bytes: Some(content_total_bytes),
            }],
        )
        .await?;

    install_pack_files(
        PackFileInstall {
            source: "local",
            instance_id,
            minecraft_version,
            loader_type_value,
            operation_kind:
                crate::state::instances::ManualDownloadOperationKind::PackUpdate,
            pack_details,
            reporter: Some(reporter.clone()),
            loading_bar: None,
            download_metrics: Some(Arc::new(
                CurseForgeDownloadMetrics::with_reporter(reporter.clone()),
            )),
            selected_files,
            projects,
            file_meta,
            existing_relative_paths,
            prefer_localized_names,
            total_files,
            content_total_bytes,
        },
        &state,
    )
    .await
}
