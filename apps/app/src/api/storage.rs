use crate::api::Result;
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

#[tauri::command]
pub async fn storage_scan(
    force: bool,
    on_progress: tauri::ipc::Channel<StorageScanEvent>,
) -> Result<StorageTree> {
    Ok(
        theseus::storage::scan_storage_with_progress(force, &move |event| {
            let _ = on_progress.send(event);
        })
        .await?,
    )
}

#[tauri::command]
pub async fn storage_cache_clear() -> Result<CacheCleanupResult> {
    Ok(theseus::storage::clear_cache().await?)
}

#[tauri::command]
pub async fn storage_meta_analyze() -> Result<MetaAnalysis> {
    Ok(theseus::storage::analyze_meta().await?)
}

#[tauri::command]
pub async fn storage_meta_clean(
    id: String,
    groups: Vec<String>,
) -> Result<MetaCleanupResult> {
    Ok(theseus::storage::clean_meta(id, groups).await?)
}

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
