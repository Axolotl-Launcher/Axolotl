use crate::api::search_cancellation::SearchCancellation;
use crate::api::{Result, TheseusSerializableError};
use theseus::curseforge::{
    CurseForgeCapability, CurseForgeCategory, CurseForgeManualDownload,
    CurseForgeManualDownloadImport, CurseForgeManualDownloadScanResult,
    CurseForgeModpackInstallRequest, CurseForgeModpackInstallResult,
    CurseForgeRecognitionResult, CurseForgeSearchRequest,
    UnifiedSearchResponse,
};
use theseus::prelude::{CacheBehaviour, State};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("curseforge")
        .invoke_handler(tauri::generate_handler![
            curseforge_capability,
            curseforge_api_key_override,
            curseforge_set_api_key,
            curseforge_validate_credentials,
            curseforge_search_projects,
            curseforge_get_download_url,
            curseforge_get_categories,
            curseforge_recognize_instance_files,
            curseforge_import_manual_downloads,
            curseforge_list_pending_manual_downloads,
            curseforge_import_pending_manual_download_file,
            curseforge_configure_manual_download_watcher,
            curseforge_install_modpack,
            curseforge_update_managed_modpack,
        ])
        .build()
}

#[tauri::command]
pub async fn curseforge_capability() -> Result<CurseForgeCapability> {
    // Wait for the state so the answer includes a key saved in the launcher
    // instead of reporting the launcher key while settings are still loading.
    State::get().await?;
    Ok(theseus::curseforge::capability())
}

#[tauri::command]
pub async fn curseforge_api_key_override() -> Result<Option<String>> {
    Ok(theseus::curseforge::api_key_override().await?)
}

#[tauri::command]
pub async fn curseforge_set_api_key(
    api_key: Option<String>,
) -> Result<CurseForgeCapability> {
    Ok(theseus::curseforge::set_api_key_override(api_key).await?)
}

#[tauri::command]
pub async fn curseforge_validate_credentials() -> Result<CurseForgeCapability> {
    Ok(theseus::curseforge::validate_credentials().await?)
}

#[tauri::command]
pub async fn curseforge_search_projects(
    request: CurseForgeSearchRequest,
    request_id: Option<String>,
) -> Result<UnifiedSearchResponse> {
    let Some(request_id) = request_id else {
        return Ok(theseus::curseforge::search_projects(request).await?);
    };
    let cancellation = SearchCancellation::register(request_id);

    tokio::select! {
        result = theseus::curseforge::search_projects(request) => Ok(result?),
        _ = cancellation.cancelled() => Err(TheseusSerializableError::SearchCancelled(
            "CurseForge browse search".to_string(),
        )),
    }
}

#[tauri::command]
pub async fn curseforge_get_download_url(
    project_id: u32,
    file_id: u32,
) -> Result<Option<String>> {
    Ok(theseus::curseforge::get_download_url(project_id, file_id).await?)
}

#[tauri::command]
pub async fn curseforge_get_categories(
    class_id: Option<u32>,
) -> Result<Vec<CurseForgeCategory>> {
    Ok(theseus::curseforge::get_categories(class_id).await?)
}

#[tauri::command]
pub async fn curseforge_recognize_instance_files(
    instance_id: String,
) -> Result<CurseForgeRecognitionResult> {
    Ok(theseus::curseforge::recognize_instance_files(&instance_id).await?)
}

#[tauri::command]
pub async fn curseforge_import_manual_downloads(
    instance_id: String,
    scan_directory: Option<std::path::PathBuf>,
) -> Result<CurseForgeManualDownloadScanResult> {
    Ok(theseus::curseforge::import_manual_downloads(
        &instance_id,
        scan_directory,
    )
    .await?)
}

#[tauri::command]
pub async fn curseforge_list_pending_manual_downloads(
    instance_id: String,
) -> Result<Vec<CurseForgeManualDownload>> {
    Ok(
        theseus::curseforge::list_pending_manual_downloads(&instance_id)
            .await?,
    )
}

#[tauri::command]
pub async fn curseforge_import_pending_manual_download_file(
    instance_id: String,
    project_id: u32,
    file_id: u32,
    source_path: std::path::PathBuf,
) -> Result<CurseForgeManualDownloadImport> {
    Ok(theseus::curseforge::import_pending_manual_download_file(
        &instance_id,
        project_id,
        file_id,
        source_path,
    )
    .await?)
}

#[tauri::command]
pub async fn curseforge_configure_manual_download_watcher(
    enabled: bool,
    scan_directory: Option<std::path::PathBuf>,
) -> Result<Option<String>> {
    Ok(theseus::curseforge::configure_manual_download_watcher(
        enabled,
        scan_directory,
    )
    .await?)
}

#[tauri::command]
pub async fn curseforge_install_modpack(
    request: CurseForgeModpackInstallRequest,
) -> Result<CurseForgeModpackInstallResult> {
    Ok(theseus::curseforge::install_modpack(request).await?)
}

#[tauri::command]
pub async fn curseforge_update_managed_modpack(
    instance_id: String,
    file_id: u32,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(theseus::install::runner::update_managed_curseforge_modpack(
        instance_id,
        file_id,
    )
    .await?)
}
