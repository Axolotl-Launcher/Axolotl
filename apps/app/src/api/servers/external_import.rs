//! Native IPC adapters for external server import; command names stay stable.
use crate::api::Result;
use theseus::servers;

#[tauri::command]
pub async fn servers_inspect_import(
    source_path: String,
) -> std::result::Result<Option<servers::ServerImportPreview>, String> {
    tokio::task::spawn_blocking(move || {
        servers::inspect_configurable_server_import(std::path::Path::new(
            &source_path,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn servers_import_external(
    source_path: String,
    expected: servers::ServerImportPreview,
    name: String,
    java_path: String,
    memory_mb: u32,
    configuration: Option<servers::ServerImportConfiguration>,
) -> Result<servers::ServerManifest> {
    Ok(servers::import_external_server(
        source_path,
        expected,
        name,
        java_path,
        memory_mb,
        configuration,
    )
    .await?)
}

#[tauri::command]
pub async fn servers_retry_external(
    server_id: String,
) -> Result<servers::ServerManifest> {
    Ok(servers::retry_external_server(&server_id).await?)
}
