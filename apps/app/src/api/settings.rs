use std::path::{Path, PathBuf};

use crate::api::utils::{write_zip_entry, zip_error};
use crate::api::{Result, TheseusSerializableError};
use async_zip::tokio::write::ZipFileWriter;
use serde_json::{Map, Value};
use tauri::{Emitter, Runtime};
use theseus::prelude::*;
use theseus::settings::{ImportResult, ImportScan, SettingLayer};
use theseus::{ProxyConfig, ProxyTestResult};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("settings")
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_patch,
            settings_layers,
            settings_override_clear,
            settings_sanitise,
            settings_import_scan,
            settings_import,
            settings_export,
            privacy_get,
            privacy_set,
            telemetry_set,
            discord_rpc_set,
            cancel_directory_change,
            proxy_get,
            proxy_set,
            proxy_test,
            network_test,
            network_test_cancel
        ])
        .build()
}

pub(crate) fn log_level_for_channel(channel: &str, log_level: &str) -> String {
    if channel == "beta" && !matches!(log_level, "debug" | "trace") {
        "debug".to_string()
    } else {
        log_level.to_string()
    }
}

static NETWORK_TEST_SLOTS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(2);

#[tauri::command]
pub async fn network_test(
    request_id: uuid::Uuid,
    target: theseus::NetworkTestTarget,
    mode: theseus::NetworkTestMode,
) -> Result<Option<theseus::NetworkTestResult>> {
    let cancellation = super::search_cancellation::SearchCancellation::register(
        format!("network-test:{request_id}"),
    );
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Ok(None),
        result = async {
            let _permit = NETWORK_TEST_SLOTS.acquire().await.map_err(|_| {
                std::io::Error::other("Network diagnostics are unavailable")
            })?;
            Ok(Some(theseus::test_network_connection(target, mode).await?))
        } => result,
    }
}

#[tauri::command]
pub fn network_test_cancel(request_id: uuid::Uuid) {
    if let Some(pending) = super::search_cancellation::cancel(&format!(
        "network-test:{request_id}"
    )) {
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            pending.expire();
        });
    }
}

// Get full settings
// invoke('plugin:settings|settings_get')
#[tauri::command]
pub async fn settings_get() -> Result<Settings> {
    let res = settings::get().await?;
    Ok(res)
}

/// Merges only the edited fields into current settings.
#[tauri::command]
pub async fn settings_patch(
    app: tauri::AppHandle<impl Runtime>,
    mut patch: serde_json::Value,
) -> Result<Settings> {
    let channel = crate::resolve_update_channel(&app).await?;
    if let Some(log_level) =
        patch.get("log_level").and_then(|value| value.as_str())
    {
        let log_level = log_level_for_channel(&channel, log_level);
        patch["log_level"] = serde_json::Value::String(log_level);
    }
    let settings = settings::patch(patch).await?;
    let log_level = settings.log_level.clone();
    // Apply the log level right away so the new verbosity takes effect without
    // a restart. Invalid values are rejected by the settings table itself.
    if let Err(error) = theseus::set_log_level(&log_level) {
        tracing::warn!("Keeping the previous log level: {error}");
    }
    let _ = app.emit("settings", ());
    Ok(settings)
}

/// Every settings key with what each layer says about it: what the deployment
/// provides, what the local document overrides, and what the key reads as.
#[tauri::command]
pub async fn settings_layers() -> Result<Vec<SettingLayer>> {
    Ok(settings::layers().await?)
}

/// Drops one key from the local settings document so the deployment's answer
/// takes over again; `expected` is the value the caller saw, and the key is only
/// cleared while it still holds it.
#[tauri::command]
pub async fn settings_override_clear(
    app: tauri::AppHandle<impl Runtime>,
    key: String,
    expected: serde_json::Value,
) -> Result<bool> {
    let cleared = settings::clear_override(&key, &expected).await?;
    // A key of a document is not a row write, so nothing else tells the
    // frontend that what it reads as the settings has moved.
    let _ = app.emit("settings", ());
    Ok(cleared)
}

/// Rewrites every local settings document in the shape this build knows, which
/// is what releases a document a newer build wrote from being read-only.
#[tauri::command]
pub async fn settings_sanitise() -> Result<usize> {
    Ok(settings::sanitise().await?)
}

/// The largest archive an import reads, in bytes, so a file a user was handed
/// cannot cost more than that before anything in it is decompressed.
const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024;
/// The most documents an archive may carry.
const MAX_ARCHIVE_ENTRIES: usize = 32;
/// The most one document may hold once decompressed.
const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

/// One document of an archive, in the shape the launcher's own files take.
///
/// `schema_version` is part of that shape and is not acted on: an import writes
/// this build's documents with this build's version, so the version a file was
/// written by cannot be used to lower them.
#[derive(serde::Deserialize)]
struct ImportedDocument {
    data: Map<String, Value>,
}

/// The error an import answers about a file it cannot read.
fn refused(message: String) -> TheseusSerializableError {
    theseus::Error::from(theseus::ErrorKind::InputError(message)).into()
}

/// The documents an archive holds, keyed by the domain whose file each one was.
///
/// These are files someone else gave the user, so each is bounded before it is
/// decompressed and read as a settings document: an archive that is not one, one
/// that carries more files than an import reads, and one whose file is larger
/// than a document can be are refused with the disk untouched.
async fn read_archive(input_path: &Path) -> Result<Map<String, Value>> {
    let length = tokio::fs::metadata(input_path).await?.len();
    if length > MAX_ARCHIVE_BYTES {
        return Err(refused(format!(
            "{} is larger than a settings archive can be",
            input_path.display()
        )));
    }
    let archive = async_zip::tokio::read::fs::ZipFileReader::new(input_path)
        .await
        .map_err(|error| {
            refused(format!(
                "{} is not a settings archive: {error}",
                input_path.display()
            ))
        })?;
    let entries = archive.file().entries();
    if entries.len() > MAX_ARCHIVE_ENTRIES {
        return Err(refused(format!(
            "{} carries more documents than a settings archive can",
            input_path.display()
        )));
    }
    // A member is a document only when its name is a domain's, so a directory
    // or a file beside them is not one — and a name that carries a path is not
    // one either, because the launcher's own documents sit beside each other.
    let mut members = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.uncompressed_size() > MAX_DOCUMENT_BYTES {
            return Err(refused(format!(
                "A document in {} is larger than a settings document can be",
                input_path.display()
            )));
        }
        let Ok(name) = entry.filename().as_str() else {
            continue;
        };
        let Some(domain) = name.strip_suffix(".json") else {
            continue;
        };
        if domain.is_empty() || domain.contains('/') || domain.contains('\\') {
            continue;
        }
        members.push((index, domain.to_string()));
    }
    let mut documents = Map::new();
    for (index, domain) in members {
        let mut reader =
            archive.reader_with_entry(index).await.map_err(|error| {
                refused(format!(
                    "Failed to read {domain}.json from {}: {error}",
                    input_path.display()
                ))
            })?;
        // The size the header states was checked above and is not what decides
        // how much is read: the entry is read no further than one buffer past
        // what a document may hold, so a header that understates its own entry
        // cannot decompress more than that into memory.
        let mut contents = Vec::new();
        let mut buffer = vec![0_u8; 8 * 1024];
        loop {
            let read =
                futures::io::AsyncReadExt::read(&mut reader, &mut buffer)
                    .await
                    .map_err(|error| {
                        refused(format!(
                            "Failed to read {domain}.json from {}: {error}",
                            input_path.display()
                        ))
                    })?;
            if read == 0 {
                break;
            }
            contents.extend_from_slice(&buffer[..read]);
            if contents.len() as u64 > MAX_DOCUMENT_BYTES {
                return Err(refused(format!(
                    "A document in {} is larger than a settings document can be",
                    input_path.display()
                )));
            }
        }
        let document: ImportedDocument = serde_json::from_slice(&contents)
            .map_err(|error| {
                refused(format!(
                    "{domain}.json is not a settings document: {error}"
                ))
            })?;
        documents.insert(domain, Value::Object(document.data));
    }
    Ok(documents)
}

/// What importing the archive at `input_path` would find in it, so the user can
/// be told what it carries before any of it is applied.
#[tauri::command]
pub async fn settings_import_scan(input_path: PathBuf) -> Result<ImportScan> {
    let documents = read_archive(&input_path).await?;
    Ok(settings::import_scan(&Value::Object(documents)).await?)
}

/// What to do with the settings of the danger zone an imported archive carries:
/// the keys that can make the launcher run an arbitrary command.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDanger {
    /// Leave them out and import everything else.
    Skip,
    /// Import them as the file states them.
    Allow,
}

/// Merges the documents of the archive at `input_path` into the local
/// documents, and reports what was written.
///
/// The archive is read here so a malformed one is refused before any document is
/// rewritten, and `danger` is the user's answer about what `settings_import_scan`
/// reported; `Skip` is the one that keeps a file from choosing what the launcher
/// runs.
#[tauri::command]
pub async fn settings_import(
    app: tauri::AppHandle<impl Runtime>,
    input_path: PathBuf,
    danger: ImportDanger,
) -> Result<ImportResult> {
    let documents = read_archive(&input_path).await?;
    let allow_danger = matches!(danger, ImportDanger::Allow);
    let imported =
        settings::import(Value::Object(documents), allow_danger).await?;
    // The imported documents are settings the frontend shows, and an import
    // writes documents rather than the row, so this event is the only word it
    // gets that they moved.
    let _ = app.emit("settings", ());
    Ok(imported)
}

/// Which settings an export writes.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsExportKind {
    /// Every setting as it currently reads.
    Effective,
    /// Only the keys that differ from the build's defaults, which is what a
    /// deployment's own document is written from.
    Patch,
}

/// Writes the settings to `output_path` as an archive of one document per
/// domain, each in the shape the launcher's own files take and indented for a
/// human to read and edit: a deployment's document is a file its owner keeps by
/// hand.
#[tauri::command]
pub async fn settings_export(
    output_path: PathBuf,
    kind: SettingsExportKind,
) -> Result<()> {
    let settings = settings::get().await?;
    let documents = settings::export_documents(
        &settings,
        match kind {
            SettingsExportKind::Effective => settings::ExportKind::Effective,
            SettingsExportKind::Patch => settings::ExportKind::Patch,
        },
    );
    let directory = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(directory).await?;
    let archive = tokio::fs::File::create(&output_path).await?;
    let mut writer = ZipFileWriter::with_tokio(archive);
    for (name, document) in &documents {
        let mut contents = serde_json::to_vec_pretty(document)
            .map_err(theseus::Error::from)?;
        // A file that ends without a newline is one a text editor reports as
        // changed on the first save, and these are files meant to be edited.
        contents.push(b'\n');
        // A document is named by its domain alone: the archive is flat, so the
        // files are what a settings directory would hold.
        write_zip_entry(&mut writer, &format!("{name}.json"), &contents)
            .await?;
    }
    writer.close().await.map_err(zip_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::log_level_for_channel;

    #[test]
    fn beta_only_allows_verbose_log_levels() {
        assert_eq!(log_level_for_channel("beta", "info"), "debug");
        assert_eq!(log_level_for_channel("beta", "debug"), "debug");
        assert_eq!(log_level_for_channel("beta", "trace"), "trace");
    }

    #[test]
    fn release_preserves_the_selected_log_level() {
        assert_eq!(log_level_for_channel("release", "error"), "error");
        assert_eq!(log_level_for_channel("release", "info"), "info");
        assert_eq!(log_level_for_channel("release", "trace"), "trace");
    }
}

#[tauri::command]
pub async fn privacy_get() -> Result<PrivacySettings> {
    Ok(settings::get_privacy().await?)
}

#[tauri::command]
pub async fn privacy_set(privacy: PrivacySettings) -> Result<PrivacySettings> {
    Ok(settings::set_privacy(privacy).await?)
}

#[tauri::command]
pub async fn telemetry_set(enabled: bool) -> Result<PrivacySettings> {
    Ok(settings::set_telemetry(enabled).await?)
}

#[tauri::command]
pub async fn discord_rpc_set(enabled: bool) -> Result<PrivacySettings> {
    Ok(settings::set_discord_rpc(enabled).await?)
}

#[tauri::command]
pub async fn cancel_directory_change<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let identifier = &app.config().identifier;
    settings::cancel_directory_change(identifier).await?;
    Ok(())
}

#[tauri::command]
pub async fn proxy_get() -> Result<ProxyConfig> {
    let state = State::get().await?;
    Ok(state.proxy_config().await?)
}

#[tauri::command]
pub async fn proxy_set(
    app: tauri::AppHandle<impl Runtime>,
    config: ProxyConfig,
) -> Result<()> {
    let state = State::get().await?;
    config.validate()?;
    state.update_proxy_config(&config).await?;
    let _ = app.emit("settings", ());
    Ok(())
}

#[tauri::command]
pub async fn proxy_test(config: ProxyConfig) -> Result<ProxyTestResult> {
    Ok(theseus::test_proxy_connection(&config).await?)
}
