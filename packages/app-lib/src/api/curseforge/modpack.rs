//! Modpack archive installation.

use super::catalog::{get_file, get_project};
use super::download::resolve_download_url as resolve_curseforge_download_url;
use super::dto::{
    CurseForgeFailedDownload, CurseForgeFile, CurseForgeInstallRequest,
    CurseForgeInstallResult, CurseForgeInstalledFile, CurseForgeManifestFile,
    CurseForgeManualDownload, CurseForgeModpackInstallRequest,
    CurseForgeModpackInstallResult, CurseForgeModpackManifest,
    CurseForgeModpackTarget, CurseForgeProject, get_modpack_expected_members,
};
use super::install::install_file_with_metrics;
use super::mapping::project_type_for_class;
use super::metrics::CurseForgeDownloadMetrics;
use super::selection::{
    content_target_folder, curseforge_file_page_url, format_bytes,
    install_modpack_world_file, managed_project_type,
    manual_download_from_file, modpack_target_folder, persist_manual_download,
    report_modpack_progress,
};
use super::update::{
    cache_instance_icon_from_url, loader_type, materialize_modpack_overrides,
    merge_install_result, modpack_target, modpack_zip_error,
    read_modpack_manifest, settle_materialized_modpack_overrides,
};
use super::validation::validate_file_name;
use super::verify::{
    ActiveCurseForgeDownload, CurseForgeDatabaseTask,
    CurseForgeVerificationContext, CurseForgeVerificationTask,
    DownloadInstalledFileRequest, download_curseforge_archive,
    download_installed_file, spawn_curseforge_database_worker,
    spawn_curseforge_verification_worker,
};
use super::{get_modpack_files, get_modpack_projects};
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::event::LoadingBarType;
use crate::event::emit::{
    emit_loading, init_loading, loading_try_for_each_concurrent,
};
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::{
    ContentSourceKind, EditInstance, InstanceLink, ModLoader, ProjectType,
    Settings,
};
use crate::util::download::constants::{
    MODPACK_DATABASE_QUEUE_CAPACITY, MODPACK_FILE_INSTALL_ATTEMPTS,
    MODPACK_VERIFICATION_QUEUE_CAPACITY,
};
use crate::util::fetch::FetchProgressFn;
use crate::{ErrorKind, State};
use futures::stream;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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

async fn download_modpack_archive_with_reporter(
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

    tracing::info!(
        selected_manifest_files = selected_files.len(),
        "Resolved CurseForge modpack manifest files"
    );
    let content = Arc::new(Mutex::new(CurseForgeInstallResult::default()));
    let files_done = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let active_downloads = Arc::new(AtomicU64::new(0));
    let cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();
    // Keep pipeline memory and staged-file growth bounded independently of
    // the manifest size. The buffer absorbs normal verifier/SQLite jitter;
    // sustained downstream pressure intentionally reaches the producers.
    let (verification_tx, verification_rx) =
        mpsc::channel::<CurseForgeVerificationTask>(
            MODPACK_VERIFICATION_QUEUE_CAPACITY,
        );
    let verification_context = CurseForgeVerificationContext {
        reporter: reporter.clone(),
        loading_bar: loading_bar.clone(),
        details: pack_details.clone(),
        files_done: files_done.clone(),
        bytes_done: bytes_done.clone(),
        active_downloads: active_downloads.clone(),
        total_files: total_files as u64,
        total_bytes: content_total_bytes,
        cancellation: cancellation.clone(),
    };
    let (database_tx, database_rx) = mpsc::channel::<CurseForgeDatabaseTask>(
        MODPACK_DATABASE_QUEUE_CAPACITY,
    );
    let database_worker = spawn_curseforge_database_worker(
        database_rx,
        verification_context.clone(),
    );
    let verification_worker = spawn_curseforge_verification_worker(
        verification_rx,
        verification_context,
        database_tx.clone(),
    );
    let download_metrics = reporter.as_ref().map(|reporter| {
        Arc::new(CurseForgeDownloadMetrics::with_reporter(reporter.clone()))
    });
    let projects = Arc::new(projects);
    let file_meta = Arc::new(file_meta);
    let minecraft_version = manifest.minecraft.version.clone();
    let download_result = loading_try_for_each_concurrent(
        stream::iter(selected_files.into_iter().map(Ok::<_, crate::Error>)),
        Some(state.download_concurrency()),
        // Progress is updated manually with file+byte counts below.
        None,
        1.0,
        total_files,
        None,
        |manifest_file| {
            let content = content.clone();
            let projects = projects.clone();
            let file_meta = file_meta.clone();
            let files_done = files_done.clone();
            let bytes_done = bytes_done.clone();
            let active_downloads = active_downloads.clone();
            let loading_bar = loading_bar.clone();
            let reporter = reporter.clone();
            let download_metrics = download_metrics.clone();
            let pack_details = pack_details.clone();
            let minecraft_version = minecraft_version.clone();
            let request = request.clone();
            let verification_tx = verification_tx.clone();
            let cancellation = cancellation.clone();
            let existing_relative_paths = existing_relative_paths.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err(ErrorKind::OtherError("download canceled".to_string()).into());
                }
                let project = projects
                    .get(&manifest_file.project_id)
                    .ok_or_else(|| {
                        ErrorKind::OtherError(format!(
                            "CurseForge project metadata is missing for {}",
                            manifest_file.project_id
                        ))
                    })?;
                let project_type = project_type_for_class(project.class_id);
                let folder = modpack_target_folder(project_type)?;
                let meta = file_meta.get(&manifest_file.file_id).ok_or_else(|| {
                    ErrorKind::OtherError(format!(
                        "CurseForge file metadata is missing for {}",
                        manifest_file.file_id
                    ))
                })?;
                let file_name = crate::util::io::sanitize_download_file_name(&meta.file_name)?;
                let original_relative_path = format!("{folder}/{file_name}");
                let localized_candidate = (project_type != "mod" && project_type != "world")
                    .then(|| {
                        chinese_file_title_for_curseforge_slug(&project.slug)
                            .and_then(|title| {
                                localized_content_file_name(&file_name, &title)
                            })
                            .map(|file_name| format!("{folder}/{file_name}"))
                    })
                    .flatten();
                let pre_resolved_relative_path = if existing_relative_paths
                    .contains(&original_relative_path)
                {
                    original_relative_path
                } else if let Some(localized) = localized_candidate {
                    if existing_relative_paths.contains(&localized) || prefer_localized_names {
                        localized
                    } else {
                        original_relative_path
                    }
                } else {
                    original_relative_path
                };

                let mut active_download =
                    ActiveCurseForgeDownload::start(active_downloads.clone());
                let (installed_result, failed_result, failure_reason) =
                    retry_modpack_file_install(
                        &request.instance_id,
                        &manifest_file,
                        project_type,
                        &minecraft_version,
                        loader_type_value,
                        if request.allow_target_change {
                            crate::state::instances::ManualDownloadOperationKind::PackUpdate
                        } else {
                            crate::state::instances::ManualDownloadOperationKind::PackInstall
                        },
                        download_metrics.as_deref(),
                        file_meta
                            .get(&manifest_file.file_id)
                            .map(|file| file.file_name.as_str())
                        .unwrap_or("<unknown>"),
                        cancellation.clone(),
                        Some(verification_tx.clone()),
                        Some(pre_resolved_relative_path),
                        Some((project.clone(), meta.clone())),
                    )
                    .await;
                active_download.finish();

                if cancellation.is_cancelled() {
                    return Err(ErrorKind::OtherError("download canceled".to_string()).into());
                }

                let Some(item_result) = installed_result else {
                    let mut failed_result = failed_result.unwrap_or_default();
                    let file_name = file_meta
                        .get(&manifest_file.file_id)
                        .map(|file| file.file_name.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "project-{}-file-{}",
                                manifest_file.project_id,
                                manifest_file.file_id
                            )
                        });
                    let manual_download =
                        failed_result.manual_downloads.first().cloned();
                    let event = if let Some(manual_download) =
                        manual_download.as_ref()
                    {
                        InstallJobEventKind::ContentFileSkipped {
                            path: manual_download.file_name.clone(),
                            reason: "CurseForge requires manual download"
                                .to_string(),
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                            manual_url: manual_download.website_url.clone(),
                        }
                    } else {
                        let reason = format!(
                            "Failed after {MODPACK_FILE_INSTALL_ATTEMPTS} attempts: {failure_reason}"
                        );
                        failed_result.failed_downloads.push(
                            CurseForgeFailedDownload {
                                project_id: manifest_file.project_id,
                                file_id: manifest_file.file_id,
                                file_name: file_name.clone(),
                                reason: reason.clone(),
                            },
                        );
                        InstallJobEventKind::ContentFileFailed {
                            path: file_name,
                            reason,
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                        }
                    };
                    {
                        let mut content =
                            content.lock().expect("content mutex");
                        merge_install_result(&mut content, failed_result);
                    }
                    report_modpack_progress(
                        loading_bar.as_deref(),
                        reporter.as_ref(),
                        pack_details,
                        &files_done,
                        &bytes_done,
                        &active_downloads,
                        total_files as u64,
                        content_total_bytes,
                        0,
                        event,
                    )
                    .await?;
                    return Ok(());
                };
                if project_type == "world" {
                    report_modpack_progress(loading_bar.as_deref(), reporter.as_ref(), pack_details.clone(), &files_done, &bytes_done, &active_downloads, total_files as u64, content_total_bytes, meta.file_length, InstallJobEventKind::ContentFileCompleted {
                        path: item_result.installed[0].relative_path.clone(), bytes: meta.file_length,
                    }).await?;
                }
                {
                    let mut content = content.lock().expect("content mutex");
                    merge_install_result(&mut content, item_result);
                }
                Ok(())
            }
        },
    )
    .await;

    if let Err(error) = &download_result {
        if let Some(reporter) = &reporter {
            reporter.fail_pipeline(error);
        }
        cancellation.cancel();
    }

    drop(verification_tx);
    let verification_result = verification_worker.await.map_err(|e| {
        ErrorKind::OtherError(format!("verification worker failed: {e}"))
    })?;
    drop(database_tx);
    let database_result = database_worker.await.map_err(|e| {
        ErrorKind::OtherError(format!("database worker failed: {e}"))
    })?;
    if let Some(error) = reporter
        .as_ref()
        .and_then(InstallProgressReporter::pipeline_failure)
    {
        return Err(error);
    }
    verification_result?;
    database_result?;
    download_result?;

    // Every selected file has now passed verification and database
    // registration. Publish an authoritative item-count completion snapshot
    // so a final byte-progress sample cannot leave CurseForge installs at
    // 99% in the frontend.
    if let Some(reporter) = reporter.as_ref() {
        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: total_files as u64,
                    total: total_files as u64,
                    secondary: Some(InstallProgressSecondary {
                        current: content_total_bytes,
                        total: content_total_bytes,
                    }),
                }),
                pack_details.clone(),
            )
            .await?;
    }

    if let (Some(reporter), Some(download_metrics)) =
        (reporter.as_ref(), download_metrics.as_ref())
    {
        download_metrics.finish(reporter).await?;
    }

    let content = Arc::try_unwrap(content)
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state was still shared after completion"
                    .to_string(),
            )
        })?
        .into_inner()
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state mutex was poisoned".to_string(),
            )
        })?;
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

fn select_modpack_manifest_files(
    manifest: &CurseForgeModpackManifest,
    pack_members: &[crate::state::instances::PackMember],
    install_optional: bool,
) -> (Vec<CurseForgeManifestFile>, HashSet<String>) {
    let preserved_pack_projects = pack_members
        .iter()
        .filter(|member| {
            matches!(
                member.override_kind,
                crate::state::instances::PackMemberOverrideKind::Version
                    | crate::state::instances::PackMemberOverrideKind::Removed
            )
        })
        .filter_map(|member| member.provider_project_id.clone())
        .collect::<HashSet<_>>();
    let disabled_pack_projects = pack_members
        .iter()
        .filter(|member| {
            member.override_kind
                == crate::state::instances::PackMemberOverrideKind::Disabled
        })
        .filter_map(|member| member.provider_project_id.clone())
        .collect::<HashSet<_>>();
    let installed_pack_releases = pack_members
        .iter()
        .filter(|member| {
            member.materialization_state
                == crate::state::instances::PackMemberMaterializationState::Present
        })
        .filter_map(|member| {
            Some((
                member.provider_project_id.clone()?,
                member.provider_release_id.clone()?,
            ))
        })
        .collect::<HashSet<_>>();
    let selected_files = manifest
        .files
        .iter()
        .filter(|file| file.required || install_optional)
        .filter(|file| {
            !preserved_pack_projects.contains(&file.project_id.to_string())
        })
        .filter(|file| {
            !installed_pack_releases.contains(&(
                file.project_id.to_string(),
                file.file_id.to_string(),
            ))
        })
        .cloned()
        .collect::<Vec<_>>();
    (selected_files, disabled_pack_projects)
}

async fn install_preloaded_modpack_file(
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

pub(super) async fn retry_modpack_file_install(
    instance_id: &str,
    manifest_file: &CurseForgeManifestFile,
    project_type: &str,
    minecraft_version: &str,
    loader_type_value: Option<u32>,
    manual_operation_kind: crate::state::instances::ManualDownloadOperationKind,
    download_metrics: Option<&CurseForgeDownloadMetrics>,
    expected_file_name: &str,
    cancellation: CancellationToken,
    verification_tx: Option<mpsc::Sender<CurseForgeVerificationTask>>,
    pre_resolved_relative_path: Option<String>,
    preloaded: Option<(CurseForgeProject, CurseForgeFile)>,
) -> (
    Option<CurseForgeInstallResult>,
    Option<CurseForgeInstallResult>,
    String,
) {
    let mut installed_result = None;
    let mut failed_result = None;
    let mut failure_reason = "no file was installed".to_string();
    let mut attempt_failures = Vec::new();
    for attempt in 1..=MODPACK_FILE_INSTALL_ATTEMPTS {
        if cancellation.is_cancelled() {
            return (None, None, "download canceled".to_string());
        }
        let install_request = CurseForgeInstallRequest {
            instance_id: instance_id.to_string(),
            project_id: manifest_file.project_id,
            file_id: manifest_file.file_id,
            project_type: project_type.to_string(),
            ownership_kind:
                crate::state::instances::ContentOwnershipKind::PackManaged,
            manual_operation_kind,
            game_version: Some(minecraft_version.to_string()),
            mod_loader_type: loader_type_value,
            world_name: None,
            install_dependencies: false,
            excluded_dependency_project_ids: Vec::new(),
            force_dependency_project_ids: Vec::new(),
            dependency_plan_id: None,
            defer_persistence: verification_tx.is_some(),
            verification_tx: verification_tx.clone(),
            pre_resolved_relative_path: pre_resolved_relative_path.clone(),
            expected_file_name: Some(expected_file_name.to_string()),
        };
        let result = match preloaded.as_ref() {
            Some((project, file)) if project_type == "world" => {
                install_modpack_world_file(
                    instance_id,
                    project,
                    file,
                    download_metrics
                        .and_then(|metrics| metrics.reporter.as_ref()),
                    manual_operation_kind,
                )
                .await
            }
            Some((project, file)) => {
                install_preloaded_modpack_file(
                    &install_request,
                    project,
                    file,
                    download_metrics,
                )
                .await
            }
            None => {
                install_file_with_metrics(install_request, download_metrics)
                    .await
            }
        };
        match result {
            Ok(item_result) if !item_result.installed.is_empty() => {
                let installed_path = &item_result.installed[0].relative_path;
                let expected_name =
                    crate::util::io::sanitize_download_file_name(
                        expected_file_name,
                    )
                    .ok();
                if project_type == ProjectType::Mod.get_name()
                    && Path::new(installed_path).file_name()
                        != expected_name.as_deref().map(std::ffi::OsStr::new)
                {
                    failure_reason = format!(
                        "CurseForge install context mismatch: project_id={} file_id={} expected_file={} installed_path={}",
                        manifest_file.project_id,
                        manifest_file.file_id,
                        expected_file_name,
                        installed_path,
                    );
                    attempt_failures
                        .push(format!("attempt {attempt}: {failure_reason}"));
                    continue;
                }
                installed_result = Some(item_result);
                break;
            }
            Ok(item_result) => {
                failure_reason = item_result
                    .manual_downloads
                    .first()
                    .map(|file| {
                        format!("{} requires manual download", file.file_name)
                    })
                    .unwrap_or_else(|| "no file was installed".to_string());
                let manual_download_required =
                    !item_result.manual_downloads.is_empty();
                failed_result = Some(item_result);
                if manual_download_required {
                    break;
                }
            }
            Err(error) => failure_reason = install_error_chain(&error),
        }
        attempt_failures.push(format!("attempt {attempt}: {failure_reason}"));
        tracing::warn!(
            project_id = manifest_file.project_id,
            file_id = manifest_file.file_id,
            attempt,
            max_attempts = MODPACK_FILE_INSTALL_ATTEMPTS,
            expected_file = expected_file_name,
            reason = %attempt_failures.last().expect("attempt failure recorded"),
            "Failed to install required CurseForge file"
        );
        if attempt < MODPACK_FILE_INSTALL_ATTEMPTS {
            tokio::select! {
                _ = cancellation.cancelled() => return (None, None, "download canceled".to_string()),
                _ = tokio::time::sleep(Duration::from_millis(250 * attempt as u64)) => {}
            }
        }
    }
    if !attempt_failures.is_empty() {
        failure_reason = attempt_failures.join("\n");
    }
    (installed_result, failed_result, failure_reason)
}

fn install_error_chain(error: &crate::Error) -> String {
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

    tracing::info!(
        selected_manifest_files = selected_files.len(),
        "Resolved local CurseForge modpack manifest files"
    );
    let content = Arc::new(Mutex::new(CurseForgeInstallResult::default()));
    let download_metrics =
        Arc::new(CurseForgeDownloadMetrics::with_reporter(reporter.clone()));
    let projects = Arc::new(projects);
    let file_meta = Arc::new(file_meta);
    let files_done = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let active_downloads = Arc::new(AtomicU64::new(0));
    let cancellation = reporter.cancellation_token();
    let (verification_tx, verification_rx) =
        mpsc::channel::<CurseForgeVerificationTask>(
            MODPACK_VERIFICATION_QUEUE_CAPACITY,
        );
    let verification_context = CurseForgeVerificationContext {
        reporter: Some(reporter.clone()),
        loading_bar: None,
        details: pack_details.clone(),
        files_done: files_done.clone(),
        bytes_done: bytes_done.clone(),
        active_downloads: active_downloads.clone(),
        total_files: total_files as u64,
        total_bytes: content_total_bytes,
        cancellation: cancellation.clone(),
    };
    let (database_tx, database_rx) = mpsc::channel::<CurseForgeDatabaseTask>(
        MODPACK_DATABASE_QUEUE_CAPACITY,
    );
    let database_worker = spawn_curseforge_database_worker(
        database_rx,
        verification_context.clone(),
    );
    let verification_worker = spawn_curseforge_verification_worker(
        verification_rx,
        verification_context,
        database_tx.clone(),
    );

    let download_result = loading_try_for_each_concurrent(
        stream::iter(selected_files.into_iter().map(Ok::<_, crate::Error>)),
        Some(state.download_concurrency()),
        None,
        1.0,
        total_files,
        None,
        |manifest_file| {
            let content = content.clone();
            let projects = projects.clone();
            let file_meta = file_meta.clone();
            let files_done = files_done.clone();
            let bytes_done = bytes_done.clone();
            let active_downloads = active_downloads.clone();
            let reporter = reporter.clone();
            let download_metrics = download_metrics.clone();
            let pack_details = pack_details.clone();
            let instance_id = instance_id.to_string();
            let minecraft_version = minecraft_version.to_string();
            let verification_tx = verification_tx.clone();
            let existing_relative_paths = existing_relative_paths.clone();
            let cancellation = cancellation.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err(
                        ErrorKind::OtherError("download canceled".to_string())
                            .into(),
                    );
                }
                let expected_file_name = file_meta
                    .get(&manifest_file.file_id)
                    .map(|file| file.file_name.as_str())
                    .unwrap_or("<unknown>");
                let project = projects
                    .get(&manifest_file.project_id)
                    .ok_or_else(|| {
                        ErrorKind::OtherError(format!(
                            "CurseForge project metadata is missing for {}",
                            manifest_file.project_id
                        ))
                    })?;
                let project_type = project_type_for_class(project.class_id);
                let folder = modpack_target_folder(project_type)?;
                let meta = file_meta.get(&manifest_file.file_id).ok_or_else(|| {
                    ErrorKind::OtherError(format!(
                        "CurseForge file metadata is missing for {}",
                        manifest_file.file_id
                    ))
                })?;
                let file_name = crate::util::io::sanitize_download_file_name(&meta.file_name)?;
                let original_relative_path = format!("{folder}/{file_name}");
                let localized_candidate = (project_type != "mod" && project_type != "world")
                    .then(|| {
                        chinese_file_title_for_curseforge_slug(&project.slug)
                            .and_then(|title| {
                                localized_content_file_name(&file_name, &title)
                            })
                            .map(|file_name| format!("{folder}/{file_name}"))
                    })
                    .flatten();
                let pre_resolved_relative_path = if existing_relative_paths
                    .contains(&original_relative_path)
                {
                    original_relative_path
                } else if let Some(localized) = localized_candidate {
                    if existing_relative_paths.contains(&localized) || prefer_localized_names {
                        localized
                    } else {
                        original_relative_path
                    }
                } else {
                    original_relative_path
                };

                let mut active_download =
                    ActiveCurseForgeDownload::start(active_downloads.clone());
                let (installed_result, failed_result, failure_reason) =
                    retry_modpack_file_install(
                        &instance_id,
                        &manifest_file,
                        project_type,
                        &minecraft_version,
                        loader_type_value,
                        crate::state::instances::ManualDownloadOperationKind::PackUpdate,
                        Some(&download_metrics),
                        expected_file_name,
                        cancellation.clone(),
                        Some(verification_tx.clone()),
                        Some(pre_resolved_relative_path),
                        Some((project.clone(), meta.clone())),
                    )
                    .await;
                active_download.finish();

                if cancellation.is_cancelled() {
                    return Err(
                        ErrorKind::OtherError("download canceled".to_string())
                            .into(),
                    );
                }

                let Some(item_result) = installed_result else {
                    let mut failed_result = failed_result.unwrap_or_default();
                    let file_name = file_meta
                        .get(&manifest_file.file_id)
                        .map(|file| file.file_name.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "project-{}-file-{}",
                                manifest_file.project_id,
                                manifest_file.file_id
                            )
                        });
                    let manual_download =
                        failed_result.manual_downloads.first().cloned();
                    let event = if let Some(manual_download) =
                        manual_download.as_ref()
                    {
                        InstallJobEventKind::ContentFileSkipped {
                            path: manual_download.file_name.clone(),
                            reason: "CurseForge requires manual download"
                                .to_string(),
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                            manual_url: manual_download.website_url.clone(),
                        }
                    } else {
                        let reason = format!(
                            "Failed after {MODPACK_FILE_INSTALL_ATTEMPTS} attempts: {failure_reason}"
                        );
                        failed_result.failed_downloads.push(
                            CurseForgeFailedDownload {
                                project_id: manifest_file.project_id,
                                file_id: manifest_file.file_id,
                                file_name: file_name.clone(),
                                reason: reason.clone(),
                            },
                        );
                        InstallJobEventKind::ContentFileFailed {
                            path: file_name,
                            reason,
                            project_id: Some(
                                manifest_file.project_id.to_string(),
                            ),
                            version_id: Some(manifest_file.file_id.to_string()),
                        }
                    };
                    {
                        let mut content =
                            content.lock().expect("content mutex");
                        merge_install_result(&mut content, failed_result);
                    }
                    report_modpack_progress(
                        None,
                        Some(&reporter),
                        pack_details,
                        &files_done,
                        &bytes_done,
                        &active_downloads,
                        total_files as u64,
                        content_total_bytes,
                        0,
                        event,
                    )
                    .await?;
                    return Ok(());
                };
                if project_type == "world" {
                    report_modpack_progress(None, Some(&reporter), pack_details.clone(), &files_done, &bytes_done, &active_downloads, total_files as u64, content_total_bytes, meta.file_length, InstallJobEventKind::ContentFileCompleted {
                        path: item_result.installed[0].relative_path.clone(), bytes: meta.file_length,
                    }).await?;
                }
                {
                    let mut content = content.lock().expect("content mutex");
                    merge_install_result(&mut content, item_result);
                }
                Ok(())
            }
        },
    )
    .await;

    if let Err(error) = &download_result {
        reporter.fail_pipeline(error);
        cancellation.cancel();
    }

    drop(verification_tx);
    let verification_result = verification_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "local CurseForge verification worker failed: {error}"
        ))
    })?;
    drop(database_tx);
    let database_result = database_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "local CurseForge database worker failed: {error}"
        ))
    })?;
    if let Some(error) = reporter.pipeline_failure() {
        return Err(error);
    }
    verification_result?;
    database_result?;
    download_result?;

    reporter
        .update(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: total_files as u64,
                total: total_files as u64,
                secondary: Some(InstallProgressSecondary {
                    current: content_total_bytes,
                    total: content_total_bytes,
                }),
            }),
            pack_details.clone(),
        )
        .await?;

    download_metrics.finish(reporter).await?;

    Arc::try_unwrap(content)
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state was still shared after completion"
                    .to_string(),
            )
        })?
        .into_inner()
        .map_err(|_| {
            ErrorKind::OtherError(
                "CurseForge install state mutex was poisoned".to_string(),
            )
        })
        .map_err(Into::into)
}
