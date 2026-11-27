use crate::api::Result;
use crate::api::search_cancellation::{self, SearchCancellation};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;
use theseus::storage::{
    CacheCleanupResult, MetaAnalysis, MetaCleanupResult, StoragePath,
    StoragePathKind, StorageScanEvent, StorageTree,
};

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("storage")
        .invoke_handler(tauri::generate_handler![
            storage_scan,
            storage_scan_cancel,
            storage_cache_clear,
            storage_meta_analyze,
            storage_meta_clean,
            storage_open_paths
        ])
        .build()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StorageOpenResult {
    pub opened: Vec<String>,
    pub failed: Vec<StorageOpenFailure>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StorageOpenFailure {
    pub path: String,
    pub reason: String,
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn storage_scan(
    force: bool,
    request_id: uuid::Uuid,
    on_progress: tauri::ipc::Channel<StorageScanEvent>,
) -> Result<Option<StorageTree>> {
    let cancellation =
        SearchCancellation::register(format!("storage-scan:{request_id}"));
    Ok(theseus::storage::scan_storage_until_cancelled(
        force,
        &move |event| {
            let _ = on_progress.send(event);
        },
        cancellation.cancelled(),
    )
    .await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub fn storage_scan_cancel(request_id: uuid::Uuid) {
    if let Some(pending) =
        search_cancellation::cancel(&format!("storage-scan:{request_id}"))
    {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            pending.expire();
        });
    }
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn storage_cache_clear() -> Result<CacheCleanupResult> {
    Ok(theseus::storage::clear_cache().await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn storage_meta_analyze() -> Result<MetaAnalysis> {
    Ok(theseus::storage::analyze_meta().await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn storage_meta_clean(
    id: String,
    groups: Vec<String>,
) -> Result<MetaCleanupResult> {
    Ok(theseus::storage::clean_meta(id, groups).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn storage_open_paths<R: Runtime>(
    app: tauri::AppHandle<R>,
    paths: Vec<StoragePath>,
) -> StorageOpenResult {
    let mut opened = Vec::new();
    let mut failed = Vec::new();

    for storage_path in paths {
        let path = PathBuf::from(&storage_path.path);
        let result = match storage_path.kind {
            StoragePathKind::File => {
                app.opener().reveal_item_in_dir(path.clone()).map(|_| ())
            }
            StoragePathKind::Directory => app
                .opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map(|_| ()),
        };

        match result {
            Ok(()) => opened.push(storage_path.path),
            Err(error) => {
                tracing::warn!(
                    path = %storage_path.path,
                    error = %error,
                    "Failed to open storage path"
                );
                failed.push(StorageOpenFailure {
                    path: storage_path.path,
                    reason: error.to_string(),
                });
            }
        }
    }

    StorageOpenResult { opened, failed }
}
