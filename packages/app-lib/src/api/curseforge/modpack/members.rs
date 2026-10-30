//! CurseForge modpack install members.

use super::*;
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::api::curseforge::archive::merge_install_result;
use crate::api::curseforge::dto::{
    CurseForgeFailedDownload, CurseForgeFile, CurseForgeInstallRequest,
    CurseForgeInstallResult, CurseForgeManifestFile, CurseForgeModpackManifest,
    CurseForgeProject,
};
use crate::api::curseforge::install::install_file_with_metrics;
use crate::api::curseforge::mapping::project_type_for_class;
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::selection::{
    install_modpack_world_file, modpack_target_folder, report_modpack_progress,
};
use crate::api::curseforge::verify::{
    ActiveCurseForgeDownload, CurseForgeDatabaseTask,
    CurseForgeVerificationContext, CurseForgeVerificationTask,
    spawn_curseforge_database_worker, spawn_curseforge_verification_worker,
};
use crate::event::LoadingBarId;
use crate::event::emit::loading_try_for_each_concurrent;
use crate::install::{
    InstallJobEventKind, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter, InstallProgressSecondary,
};
use crate::state::ProjectType;
use crate::util::download::constants::{
    MODPACK_DATABASE_QUEUE_CAPACITY, MODPACK_FILE_INSTALL_ATTEMPTS,
    MODPACK_VERIFICATION_QUEUE_CAPACITY,
};
use crate::{ErrorKind, State};
use futures::stream;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) struct PackFileInstall<'a> {
    pub(crate) source: &'static str,
    pub(crate) instance_id: &'a str,
    pub(crate) minecraft_version: &'a str,
    pub(crate) loader_type_value: Option<u32>,
    pub(crate) operation_kind:
        crate::state::instances::ManualDownloadOperationKind,
    pub(crate) pack_details: InstallPhaseDetails,
    pub(crate) reporter: Option<InstallProgressReporter>,
    pub(crate) loading_bar: Option<Arc<LoadingBarId>>,
    pub(crate) download_metrics: Option<Arc<CurseForgeDownloadMetrics>>,
    pub(crate) selected_files: Vec<CurseForgeManifestFile>,
    pub(crate) projects: HashMap<u32, CurseForgeProject>,
    pub(crate) file_meta: HashMap<u32, CurseForgeFile>,
    pub(crate) existing_relative_paths: Arc<HashSet<String>>,
    pub(crate) prefer_localized_names: bool,
    pub(crate) total_files: usize,
    pub(crate) content_total_bytes: u64,
}

pub(super) async fn install_pack_files(
    ctx: PackFileInstall<'_>,
    state: &State,
) -> crate::Result<CurseForgeInstallResult> {
    let instance_id = ctx.instance_id.to_string();
    let minecraft_version = ctx.minecraft_version.to_string();
    let loader_type_value = ctx.loader_type_value;
    let operation_kind = ctx.operation_kind;
    let pack_details = ctx.pack_details.clone();
    let reporter = ctx.reporter.clone();
    let loading_bar = ctx.loading_bar.clone();
    let download_metrics = ctx.download_metrics.clone();
    let selected_files = ctx.selected_files.clone();
    let projects = ctx.projects.clone();
    let file_meta = ctx.file_meta.clone();
    let existing_relative_paths = ctx.existing_relative_paths.clone();
    let prefer_localized_names = ctx.prefer_localized_names;
    let total_files = ctx.total_files;
    let content_total_bytes = ctx.content_total_bytes;
    tracing::info!(
        selected_manifest_files = selected_files.len(),
        ctx.source,
        "Resolved CurseForge modpack manifest files"
    );

    let content = Arc::new(Mutex::new(CurseForgeInstallResult::default()));
    let projects = Arc::new(projects);
    let file_meta = Arc::new(file_meta);
    let files_done = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let active_downloads = Arc::new(AtomicU64::new(0));
    let cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();
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
            let loading_bar = loading_bar.clone();
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
                        operation_kind,
                        download_metrics.as_deref(),
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
        if let Some(reporter) = reporter.as_ref() {
            reporter.fail_pipeline(error);
        }
        cancellation.cancel();
    }

    drop(verification_tx);
    let verification_result = verification_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "CurseForge verification worker failed: {error}"
        ))
    })?;
    drop(database_tx);
    let database_result = database_worker.await.map_err(|error| {
        ErrorKind::OtherError(format!(
            "CurseForge database worker failed: {error}"
        ))
    })?;
    if let Some(error) = ctx
        .reporter
        .as_ref()
        .and_then(InstallProgressReporter::pipeline_failure)
    {
        return Err(error);
    }
    verification_result?;
    database_result?;
    download_result?;

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

pub(super) fn select_modpack_manifest_files(
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

pub(crate) async fn retry_modpack_file_install(
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
