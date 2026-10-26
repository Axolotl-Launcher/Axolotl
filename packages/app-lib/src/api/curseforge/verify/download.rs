//! Staged download and installation of CurseForge files.

use super::record::{
    DownloadedCurseForgeFile, record_verified_curseforge_file_with_permit,
    verify_installed_curseforge_file,
};
use super::worker::CurseForgeVerificationTask;
use crate::api::content_search::{
    chinese_file_title_for_curseforge_slug, localized_content_file_name,
};
use crate::api::curseforge::dto::CurseForgeFile;
use crate::api::curseforge::metrics::CurseForgeDownloadMetrics;
use crate::api::curseforge::selection::{
    content_target_folder, curseforge_candidate_urls,
    curseforge_content_validation, curseforge_integrity,
    curseforge_modpack_h2_range_concurrency, is_forge_cdn_url,
};
use crate::api::curseforge::transport::api_key;
use crate::api::curseforge::validation::validate_file_name;
use crate::install::InstallProgressReporter;
use crate::state::ProjectType;
use crate::util::fetch::{
    ContentValidation, DownloadRequest, FetchProgressFn, ResourceClass,
    download_to_path,
};
use crate::{ErrorKind, State};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

pub(crate) async fn download_curseforge_path(
    url: &str,
    file: &CurseForgeFile,
    destination: &Path,
    validation: ContentValidation,
    progress: Option<&mut FetchProgressFn<'_>>,
    tracking: Option<(&InstallProgressReporter, &str)>,
    h2_range_concurrency: Option<usize>,
    allow_http1_segmented_download: bool,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    let state = State::get().await?;
    let mut request = DownloadRequest::new(url, ResourceClass::CurseForge)
        .with_candidate_urls(curseforge_candidate_urls(url)?)
        .with_integrity(curseforge_integrity(file, validation))
        .with_http1_segmented_download(allow_http1_segmented_download);
    if let Some(concurrency) = h2_range_concurrency {
        request = request.with_h2_range_concurrency(concurrency);
    }
    let parsed = reqwest::Url::parse(url)?;
    if is_forge_cdn_url(&parsed)
        && let Some(key) = api_key()
    {
        request = request.with_header("x-api-key", key);
    }
    if let Some((reporter, item_id)) = tracking {
        request = request.with_install_tracking(
            reporter.clone(),
            item_id,
            file.file_name.clone(),
        );
    }
    download_to_path(
        request,
        destination,
        &state.download_semaphore,
        &state.pool,
        progress,
    )
    .await
}

pub(crate) async fn download_curseforge_archive(
    project_id: u32,
    file_id: u32,
    file: &CurseForgeFile,
    url: &str,
    progress: Option<&mut FetchProgressFn<'_>>,
    reporter: Option<&InstallProgressReporter>,
) -> crate::Result<crate::util::fetch::DownloadResult> {
    validate_file_name(&file.file_name)?;
    let state = State::get().await?;
    let path = state
        .directories
        .caches_dir()
        .join("curseforge")
        .join("modpacks")
        .join(project_id.to_string())
        .join(file_id.to_string())
        .join(crate::util::io::sanitize_download_file_name(
            &file.file_name,
        )?);
    let tracking_item_id = path.display().to_string();
    download_curseforge_path(
        url,
        file,
        &path,
        ContentValidation::Jar,
        progress,
        reporter.map(|reporter| (reporter, tracking_item_id.as_str())),
        curseforge_modpack_h2_range_concurrency(file.file_length),
        true,
    )
    .await
}

pub(crate) struct DownloadInstalledFileRequest<'a> {
    pub(crate) instance_id: &'a str,
    pub(crate) url: &'a str,
    pub(crate) file: &'a CurseForgeFile,
    pub(crate) project_type: ProjectType,
    pub(crate) world_name: Option<&'a str>,
    pub(crate) project_id: u32,
    pub(crate) file_id: u32,
    pub(crate) project_slug: &'a str,
    pub(crate) ownership_kind: crate::state::instances::ContentOwnershipKind,
    pub(crate) download_metrics: Option<&'a CurseForgeDownloadMetrics>,
    pub(crate) defer_persistence: bool,
    pub(crate) verification_tx:
        Option<&'a mpsc::Sender<CurseForgeVerificationTask>>,
    pub(crate) pre_resolved_relative_path: Option<&'a str>,
    pub(crate) expected_file_name: Option<&'a str>,
}

pub(crate) struct ActiveCurseForgeDownload {
    pub(crate) counter: Arc<AtomicU64>,
    pub(crate) active: bool,
}

impl ActiveCurseForgeDownload {
    pub(crate) fn start(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self {
            counter,
            active: true,
        }
    }

    pub(crate) fn finish(&mut self) {
        if self.active {
            self.counter.fetch_sub(1, Ordering::Relaxed);
            self.active = false;
        }
    }
}

impl Drop for ActiveCurseForgeDownload {
    fn drop(&mut self) {
        self.finish();
    }
}

pub(crate) async fn download_installed_file(
    request: DownloadInstalledFileRequest<'_>,
) -> crate::Result<DownloadedCurseForgeFile> {
    let DownloadInstalledFileRequest {
        instance_id,
        url,
        file,
        project_type,
        world_name,
        project_id,
        file_id,
        project_slug,
        ownership_kind,
        download_metrics,
        defer_persistence,
        verification_tx,
        pre_resolved_relative_path,
        expected_file_name,
    } = request;
    if file.mod_id != project_id || file.id != file_id {
        return Err(ErrorKind::InputError(
            "CurseForge returned metadata for a different project or file"
                .to_string(),
        )
        .into());
    }
    let state = State::get().await?;
    let file_name =
        crate::util::io::sanitize_download_file_name(&file.file_name)?;
    let folder = content_target_folder(project_type, world_name)?;
    let localized_candidate = if project_type == ProjectType::Mod {
        None
    } else {
        chinese_file_title_for_curseforge_slug(project_slug)
            .and_then(|title| localized_content_file_name(&file_name, &title))
            .map(|file_name| format!("{folder}/{file_name}"))
    };
    let relative_path = match pre_resolved_relative_path {
        Some(path) => path.to_string(),
        None => {
            crate::state::resolve_content_install_relative_path(
                instance_id,
                format!("{folder}/{file_name}"),
                localized_candidate,
                &state.pool,
            )
            .await?
        }
    };
    let full_path = crate::api::instance::get_full_path(instance_id)
        .await?
        .join(&relative_path);
    if project_type == ProjectType::Mod
        && let Some(expected_file_name) = expected_file_name
        && Path::new(&relative_path).file_name()
            != Some(std::ffi::OsStr::new(
                &crate::util::io::sanitize_download_file_name(
                    expected_file_name,
                )?,
            ))
    {
        return Err(ErrorKind::OtherError(format!(
            "CurseForge install context mismatch before download: expected_file={expected_file_name} installed_path={relative_path}",
        ))
        .into());
    }
    let mut download_path = full_path.as_os_str().to_os_string();
    download_path.push(".installing.download");
    let download_path = Path::new(&download_path);
    let result = download_curseforge_path(
        url,
        file,
        download_path,
        curseforge_content_validation(&file.file_name),
        None,
        download_metrics
            .and_then(|metrics| metrics.reporter.as_ref())
            .map(|reporter| (reporter, relative_path.as_str())),
        None,
        false,
    )
    .await?;
    if let Some(download_metrics) = download_metrics {
        download_metrics.record(&result);
    }
    if defer_persistence {
        if let Some(sender) = verification_tx {
            let cancellation = download_metrics
                .and_then(|metrics| metrics.reporter.as_ref())
                .map(InstallProgressReporter::cancellation_token)
                .unwrap_or_default();
            let verification_task = CurseForgeVerificationTask {
                instance_id: instance_id.to_string(),
                relative_path: relative_path.clone(),
                download_path: download_path.to_path_buf(),
                full_path: full_path.clone(),
                file: file.clone(),
                project_type,
                ownership_kind,
                expected_bytes: file.file_length,
                download_result: Some(result.clone()),
                cancellation: cancellation.clone(),
            };
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => {
                    return Err(ErrorKind::OtherError(
                        "curseforge verification enqueue canceled".to_string(),
                    ).into());
                }
                result = sender.send(verification_task) => {
                    result.map_err(|_| ErrorKind::OtherError(
                        "curseforge verification worker stopped".to_string(),
                    ))?;
                }
            }
        }
        return Ok(DownloadedCurseForgeFile { relative_path });
    }
    let verified = verify_installed_curseforge_file(
        download_path,
        file,
        None,
        Some(&result),
    )
    .await?;
    // Acquire the writer before the instance lock so a progress checkpoint
    // cannot leave this materialization holding the lock indefinitely.
    let database_permit = Some(state.acquire_install_db_permit().await?);
    let _instance_lock = state.lock_instance_content(instance_id).await;
    let previous_path =
        crate::state::materialize_project_download(download_path, &full_path)
            .await?;
    crate::util::io::remove_file(download_path).await?;
    let record_result = record_verified_curseforge_file_with_permit(
        instance_id,
        &relative_path,
        file,
        project_type,
        ownership_kind,
        database_permit,
        &state,
        verified,
    )
    .await;
    match record_result {
        Ok(()) => {
            crate::state::finalize_project_materialization(
                previous_path.as_deref(),
            )
            .await?;
        }
        Err(error) => {
            crate::state::restore_project_materialization(
                &full_path,
                previous_path.as_deref(),
            )
            .await?;
            return Err(error);
        }
    }
    Ok(DownloadedCurseForgeFile { relative_path })
}
