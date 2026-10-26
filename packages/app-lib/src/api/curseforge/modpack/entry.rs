//! CurseForge modpack install entry.

use super::*;
use crate::api::curseforge::archive::{
    cache_instance_icon_from_url, loader_type, materialize_modpack_overrides,
    modpack_target, modpack_zip_error, read_modpack_manifest,
    settle_materialized_modpack_overrides,
};
use crate::api::curseforge::catalog::{
    get_file, get_modpack_files, get_modpack_projects, get_project,
};
use crate::api::curseforge::download::resolve_download_url as resolve_curseforge_download_url;
use crate::api::curseforge::dto::{
    CurseForgeFile, CurseForgeInstallResult, CurseForgeManualDownload,
    CurseForgeModpackInstallRequest, CurseForgeModpackInstallResult,
    CurseForgeModpackManifest, CurseForgeModpackTarget,
};
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::selection::{
    curseforge_file_page_url, format_bytes, persist_manual_download,
};
use crate::api::curseforge::verify::download_curseforge_archive;
use crate::event::LoadingBarType;
use crate::event::emit::{emit_loading, init_loading};
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    ContentSourceKind, EditInstance, InstanceLink, ModLoader, Settings,
};
use crate::util::fetch::FetchProgressFn;
use crate::{ErrorKind, State};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn install_modpack(
    request: CurseForgeModpackInstallRequest,
) -> crate::Result<CurseForgeModpackInstallResult> {
    install_modpack_with_reporter(request, None).await
}

pub async fn get_modpack_target(
    project_id: u32,
    file_id: u32,
) -> crate::Result<CurseForgeModpackTarget> {
    let pack_file = get_file(project_id, file_id).await?;
    let project = get_project(project_id).await?;
    let download_url = resolve_curseforge_download_url(
        project_id, file_id, &project, &pack_file,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The CurseForge modpack manifest cannot be downloaded automatically"
                .to_string(),
        )
    })?;

    let icon_url = project.logo.as_ref().and_then(|logo| {
        if !logo.thumbnail_url.is_empty() {
            Some(logo.thumbnail_url.clone())
        } else if !logo.url.is_empty() {
            Some(logo.url.clone())
        } else {
            None
        }
    });
    let loading_bar = init_loading(
        LoadingBarType::PackFileDownload {
            instance_id: String::new(),
            pack_name: project.name.clone(),
            icon: icon_url,
            pack_version: pack_file.display_name.clone(),
        },
        pack_file.file_length.max(1) as f64,
        &format!("Downloading {}", pack_file.file_name),
    )
    .await?;
    let mut last_downloaded = 0_u64;
    let mut progress = |current: u64,
                        _total: u64|
     -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::Result<()>> + Send>,
    > {
        let delta = current.saturating_sub(last_downloaded);
        last_downloaded = current;
        let result = emit_loading(
            &loading_bar,
            delta as f64,
            Some("Downloading CurseForge modpack"),
        );
        Box::pin(async move { result })
    };
    let pack_download = download_curseforge_archive(
        project_id,
        file_id,
        &pack_file,
        &download_url,
        Some(&mut progress as &mut FetchProgressFn<'_>),
        None,
    )
    .await?;
    let pack_path = pack_download.path;
    let target = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&pack_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        let manifest = read_modpack_manifest(&mut archive)?;
        modpack_target(&manifest)
    })
    .await??;
    Ok(target)
}

pub(super) async fn download_modpack_archive_with_reporter(
    project_id: u32,
    file_id: u32,
    pack_file: &CurseForgeFile,
    download_url: &str,
    pack_details: InstallPhaseDetails,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<(PathBuf, CurseForgeModpackManifest)> {
    if let Some(reporter) = reporter.as_ref() {
        let state = State::get().await?;
        let item_path = state
            .directories
            .caches_dir()
            .join("curseforge")
            .join("modpacks")
            .join(project_id.to_string())
            .join(file_id.to_string())
            .join(crate::util::io::sanitize_download_file_name(
                &pack_file.file_name,
            )?)
            .display()
            .to_string();
        reporter
            .update_with_events(
                InstallPhaseId::DownloadingPackFile,
                Some(InstallProgress {
                    current: 0,
                    total: pack_file.file_length.max(1),
                    secondary: None,
                }),
                pack_details.clone(),
                vec![InstallJobEventKind::ContentFileQueued {
                    path: item_path,
                    bytes_total: Some(pack_file.file_length),
                    max_attempts: 5,
                }],
            )
            .await?;
        reporter.persist().await?;
    }

    let mut last_downloaded = 0_u64;
    let progress_reporter = reporter.clone();
    let progress_details = pack_details.clone();
    let mut progress = move |current: u64,
                             total: u64|
          -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::Result<()>> + Send>,
    > {
        let min_delta = (total / 200).max(256 * 1024);
        if current < total
            && current.saturating_sub(last_downloaded) < min_delta
        {
            return Box::pin(async { Ok(()) });
        }
        last_downloaded = current;
        let reporter = progress_reporter.clone();
        let details = progress_details.clone();
        Box::pin(async move {
            if let Some(reporter) = reporter {
                reporter
                    .update(
                        InstallPhaseId::DownloadingPackFile,
                        Some(InstallProgress {
                            current,
                            total,
                            secondary: None,
                        }),
                        details,
                    )
                    .await?;
            }
            Ok(())
        })
    };
    let progress = reporter
        .is_some()
        .then_some(&mut progress as &mut FetchProgressFn<'_>);
    let pack_download = download_curseforge_archive(
        project_id,
        file_id,
        pack_file,
        download_url,
        progress,
        reporter.as_ref(),
    )
    .await?;
    if let Some(reporter) = reporter.as_ref()
        && pack_download.attempts > 0
    {
        reporter
            .record_download_metrics(
                pack_download.source.as_str(),
                pack_download.fallback_count as u64,
            )
            .await?;
    }
    let pack_path = pack_download.path;
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::DownloadingPackFile,
                Some(InstallProgress {
                    current: pack_file.file_length,
                    total: pack_file.file_length.max(1),
                    secondary: None,
                }),
                pack_details.clone(),
            )
            .await?;
        reporter
            .update(InstallPhaseId::ReadingPackManifest, None, pack_details)
            .await?;
    }
    let pack_path_for_manifest = pack_path.clone();
    let manifest = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&pack_path_for_manifest)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(modpack_zip_error)?;
        read_modpack_manifest(&mut archive)
    })
    .await??;
    Ok((pack_path, manifest))
}

pub async fn install_modpack_with_reporter(
    request: CurseForgeModpackInstallRequest,
    reporter: Option<InstallProgressReporter>,
) -> crate::Result<CurseForgeModpackInstallResult> {
    let reporter =
        reporter.map(InstallProgressReporter::with_pipeline_cancellation);
    let pack_file = get_file(request.project_id, request.file_id).await?;
    let project = get_project(request.project_id).await?;
    let icon_url = project
        .logo
        .as_ref()
        .map(|logo| {
            if !logo.thumbnail_url.is_empty() {
                logo.thumbnail_url.clone()
            } else {
                logo.url.clone()
            }
        })
        .filter(|url| !url.is_empty());
    let download_url = resolve_curseforge_download_url(
        request.project_id,
        request.file_id,
        &project,
        &pack_file,
    )
    .await?;
    let Some(download_url) = download_url else {
        let manual_download = CurseForgeManualDownload {
            project_id: request.project_id,
            file_id: request.file_id,
            file_name: pack_file.file_name.clone(),
            ownership_kind: crate::state::instances::ContentOwnershipKind::PackManaged,
            operation_kind: crate::state::instances::ManualDownloadOperationKind::PackInstall,
            website_url: curseforge_file_page_url(
                project.links.website_url.as_deref(),
                request.file_id,
            ),
            project_type: "modpack".to_string(),
            project_slug: project.slug.clone(),
            target_folder: String::new(),
            hashes: pack_file.hashes.clone(),
            file_length: pack_file.file_length,
            file_fingerprint: pack_file.file_fingerprint,
        };
        persist_manual_download(&request.instance_id, &manual_download).await?;
        return Ok(CurseForgeModpackInstallResult {
            content: CurseForgeInstallResult {
                manual_downloads: vec![manual_download],
                ..Default::default()
            },
            ..Default::default()
        });
    };

    let cached_icon_path = if let Some(icon_url) = icon_url.as_ref() {
        match cache_instance_icon_from_url(icon_url).await {
            Ok(path) => {
                let _ = crate::api::instance::edit_icon(
                    &request.instance_id,
                    Some(path.as_path()),
                )
                .await;
                Some(path)
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to cache CurseForge modpack icon: {err}"
                );
                None
            }
        }
    } else {
        None
    };

    let pack_details = InstallPhaseDetails::Modpack {
        project_id: Some(request.project_id.to_string()),
        version_id: Some(request.file_id.to_string()),
        title: Some(project.name.clone()),
    };
    let (pack_path, manifest) = download_modpack_archive_with_reporter(
        request.project_id,
        request.file_id,
        &pack_file,
        &download_url,
        pack_details.clone(),
        reporter.clone(),
    )
    .await?;

    let state = State::get().await?;
    use sqlx::Row;
    let instance_target = sqlx::query(
        "SELECT content_set.game_version, content_set.loader
         FROM instances instance
         INNER JOIN instance_content_sets content_set
            ON content_set.id = instance.applied_content_set_id
         WHERE instance.id = ?",
    )
    .bind(&request.instance_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(
            "The selected instance has no active Minecraft installation"
                .to_string(),
        )
    })?;
    let instance_game_version =
        instance_target.try_get::<String, _>("game_version")?;
    let instance_loader = instance_target.try_get::<String, _>("loader")?;
    let target = modpack_target(&manifest)?;
    let loader = (target.loader != ModLoader::Vanilla)
        .then(|| target.loader.as_str().to_string());
    if (instance_game_version != manifest.minecraft.version
        || target.loader.as_str() != instance_loader)
        && !request.allow_target_change
    {
        return Err(ErrorKind::InputError(format!(
                "This modpack targets Minecraft {} with {}, while the selected instance uses {} with {}",
                manifest.minecraft.version,
                loader.as_deref().unwrap_or("vanilla"),
                instance_game_version,
                instance_loader
            ))
            .into());
    }

    let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
            &request.instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Instance has no applied content set".to_string(),
            )
        })?;
    let pack_members = crate::state::instances::adapters::sqlite::content_rows::get_pack_members(
            &content_set.id,
            &state.pool,
        )
        .await?;
    let (selected_files, disabled_pack_projects) =
        select_modpack_manifest_files(
            &manifest,
            &pack_members,
            request.install_optional,
        );
    let loader_type_value = loader.as_deref().and_then(loader_type);
    let project_ids = selected_files
        .iter()
        .map(|file| file.project_id)
        .collect::<Vec<_>>();
    let projects = get_modpack_projects(project_ids).await?;

    let instance_name = crate::api::instance::get(&request.instance_id)
        .await?
        .map(|metadata| metadata.instance.name)
        .unwrap_or_else(|| project.name.clone());
    let total_files = selected_files.len().max(1);
    let file_ids = selected_files
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let file_meta = get_modpack_files(file_ids).await?;
    let existing_relative_paths = Arc::new(
        crate::state::instances::adapters::sqlite::content_rows::get_instance_files(
            &request.instance_id,
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
    // Keep the LoadingBarId in an Arc. LoadingBarId::Drop removes the bar, so
    // cloning the ID itself would destroy progress as soon as the first task
    // finished. Arc clones only share ownership.
    let loading_bar = if reporter.is_none() {
        Some(Arc::new(
            init_loading(
                LoadingBarType::PackDownload {
                    instance_id: request.instance_id.clone(),
                    pack_name: project.name.clone(),
                    icon: cached_icon_path.clone(),
                    pack_id: Some(request.project_id.to_string()),
                    pack_version: Some(request.file_id.to_string()),
                },
                total_files as f64,
                &format!("Downloading {instance_name}"),
            )
            .await?,
        ))
    } else {
        None
    };
    if let Some(loading_bar) = loading_bar.as_ref() {
        let _ = emit_loading(
            loading_bar.as_ref(),
            0.0,
            Some(&format!(
                "0/{total_files} files · 0 / {}",
                format_bytes(content_total_bytes)
            )),
        );
    }
    if let Some(reporter) = reporter.as_ref() {
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
    }

    let content = install_pack_files(
        PackFileInstall {
            source: "remote",
            instance_id: &request.instance_id,
            minecraft_version: &manifest.minecraft.version,
            loader_type_value,
            operation_kind: if request.allow_target_change {
                crate::state::instances::ManualDownloadOperationKind::PackUpdate
            } else {
                crate::state::instances::ManualDownloadOperationKind::PackInstall
            },
            pack_details: pack_details.clone(),
            reporter: reporter.clone(),
            loading_bar: loading_bar.clone(),
            download_metrics: reporter.as_ref().map(|reporter| {
                Arc::new(CurseForgeDownloadMetrics::with_reporter(
                    reporter.clone(),
                ))
            }),
            selected_files,
            projects: HashMap::clone(&projects),
            file_meta: HashMap::clone(&file_meta),
            existing_relative_paths,
            prefer_localized_names,
            total_files,
            content_total_bytes,
        },
        &state,
    )
    .await?;

    if request.allow_target_change {
        for installed in &content.installed {
            if disabled_pack_projects
                .contains(&installed.project_id.to_string())
                && !installed.relative_path.ends_with(".disabled")
            {
                crate::state::instances::commands::toggle_disable_project(
                    &request.instance_id,
                    &installed.relative_path,
                    Some(false),
                    &state,
                )
                .await?;
            }
        }
    }

    let instance_path =
        crate::api::instance::get_full_path(&request.instance_id).await?;
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::ExtractingOverrides,
                None,
                pack_details.clone(),
            )
            .await?;
    }
    let update_ready = content.manual_downloads.is_empty()
        && content.failed_downloads.is_empty();
    let should_commit = content.manual_downloads.is_empty()
        && (!request.allow_target_change || update_ready);
    let override_cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();
    let materialized_overrides = if should_commit {
        Some(
            crate::api::pack::archive_util::run_cancellable_blocking_instance_write(
                request.instance_id.clone(),
                override_cancellation.clone(),
                move |cancellation| {
                    materialize_modpack_overrides(
                        &pack_path,
                        &instance_path,
                        Some(cancellation),
                    )
                },
            )
            .await?,
        )
    } else {
        None
    };
    let overrides_written = materialized_overrides
        .as_ref()
        .map_or(0, |(files_written, _)| *files_written);
    let post_override_result: crate::Result<()> = async {
        if let Some(reporter) = reporter.as_ref() {
            reporter
                .update(InstallPhaseId::Finalizing, None, pack_details)
                .await?;
        }
        if should_commit && !request.allow_target_change {
            crate::api::instance::edit(
            &request.instance_id,
            EditInstance {
                name: (!request.allow_target_change)
                    .then_some(project.name.clone()),
                icon_path: (!request.allow_target_change).then(|| {
                    cached_icon_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string())
                }),
                link: Some(InstanceLink::CurseForgeModpack {
                    project_id: request.project_id.to_string(),
                    version_id: request.file_id.to_string(),
                }),
                content_set_patch: Some(crate::state::AppliedContentSetPatch {
                    source_kind: Some(ContentSourceKind::CurseForge),
                    game_version: Some(manifest.minecraft.version.clone()),
                    protocol_version: Some(None),
                    loader: Some(target.loader),
                    loader_version: Some(target.loader_version.clone()),
                }),
                ..EditInstance::default()
            },
            )
            .await?;
            let content_set = crate::state::instances::adapters::sqlite::content_rows::get_applied_content_set(
            &request.instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Instance has no applied content set".to_string(),
            )
            })?;
            crate::state::sync_content_files(&request.instance_id, &state)
                .await?;
            match get_modpack_expected_members(
                request.project_id,
                request.file_id,
            )
            .await
            {
                Ok(expected) => {
                    crate::state::instances::commands::reconcile_curseforge_members(
                    &request.instance_id,
                    &content_set.id,
                    &expected,
                    &state,
                )
                    .await?;
                }
                Err(error) => tracing::warn!(
                    "Unable to persist the complete CurseForge pack manifest: {error}"
                ),
            }
        }
        Ok(())
    }
    .await;
    if let Some((_, replacements)) = materialized_overrides {
        match post_override_result {
            Ok(()) => {
                settle_materialized_modpack_overrides(
                    request.instance_id.clone(),
                    override_cancellation,
                    replacements,
                    true,
                )
                .await?;
            }
            Err(error) => {
                if let Err(rollback_error) =
                    settle_materialized_modpack_overrides(
                        request.instance_id.clone(),
                        override_cancellation,
                        replacements,
                        false,
                    )
                    .await
                {
                    return Err(ErrorKind::OtherError(format!(
                        "{error}; failed to restore CurseForge overrides: {rollback_error}"
                    ))
                    .into());
                }
                return Err(error);
            }
        }
    } else {
        post_override_result?;
    }
    Ok(CurseForgeModpackInstallResult {
        content,
        overrides_written,
        minecraft_version: manifest.minecraft.version,
        loader,
    })
}
