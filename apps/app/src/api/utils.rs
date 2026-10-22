use serde::{Deserialize, Serialize};
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;
use theseus::{
    handler,
    prelude::{CommandPayload, DirectoryInfo, app_db_backup_dir},
};

use crate::api::{Result, TheseusSerializableError};
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use theseus::prelude::canonicalize;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use url::Url;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("utils")
        .invoke_handler(tauri::generate_handler![
            get_os,
            is_network_metered,
            should_disable_mouseover,
            highlight_in_folder,
            open_path,
            show_launcher_logs_folder,
            export_error_logs,
            export_launcher_logs,
            show_app_db_backups_folder,
            progress_bars_list,
            get_opening_command,
            get_minecraft_news
        ])
        .build()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum OS {
    Windows,
    Linux,
    MacOS,
}

/// Gets OS
#[tauri::command]
pub fn get_os() -> OS {
    #[cfg(target_os = "windows")]
    let os = OS::Windows;
    #[cfg(target_os = "linux")]
    let os = OS::Linux;
    #[cfg(target_os = "macos")]
    let os = OS::MacOS;
    os
}

#[tauri::command]
pub async fn is_network_metered() -> Result<bool> {
    Ok(theseus::prelude::is_network_metered().await?)
}

// Lists active progress bars
// Create a new HashMap with the same keys
// Values provided should not be used directly, as they are not guaranteed to be up-to-date
#[tauri::command]
pub async fn progress_bars_list()
-> Result<DashMap<uuid::Uuid, theseus::LoadingBar>> {
    let res = theseus::EventState::list_progress_bars().await?;
    Ok(res)
}

// disables mouseover and fixes a random crash error only fixed by recent versions of macos
#[tauri::command]
pub async fn should_disable_mouseover() -> bool {
    if cfg!(target_os = "macos") {
        // We try to match version to 12.2 or higher. If unrecognizable to pattern or lower, we default to the css with disabled mouseover for safety
        if let tauri_plugin_os::Version::Semantic(major, minor, _) =
            tauri_plugin_os::version()
            && major >= 12
            && minor >= 3
        {
            // Mac os version is 12.3 or higher, we allow mouseover
            return false;
        }
        true
    } else {
        // Not macos, we allow mouseover
        false
    }
}

#[tauri::command]
pub async fn highlight_in_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = app.opener().reveal_item_in_dir(path) {
            tracing::error!("Failed to highlight file in folder: {}", e);
        }
    })
    .await
    .ok();
}

#[tauri::command]
pub async fn open_path<R: Runtime>(app: tauri::AppHandle<R>, path: PathBuf) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) =
            app.opener().open_path(path.to_string_lossy(), None::<&str>)
        {
            tracing::error!("Failed to open path: {}", e);
        }
    })
    .await
    .ok();
}

#[tauri::command]
pub async fn show_launcher_logs_folder<R: Runtime>(app: tauri::AppHandle<R>) {
    if let Some(d) = DirectoryInfo::global_handle_if_ready() {
        let path = d.launcher_logs_dir().unwrap_or_default();
        // failure to get folder just opens filesystem
        // (ie: if in debug mode only and launcher_logs never created)
        open_path(app, path).await;
    }
}

#[tauri::command]
pub async fn export_error_logs(
    output_path: PathBuf,
    error_message: String,
) -> Result<()> {
    let archive = tokio::fs::File::create(&output_path).await?;
    let mut writer = ZipFileWriter::with_tokio(archive);
    let report = format!(
        "Axolotl Launcher error report\nExported at: {}\n\nError:\n{}\n",
        chrono::Local::now().to_rfc3339(),
        error_message
    );

    write_zip_entry(&mut writer, "error.txt", report.as_bytes()).await?;

    if let Some(directories) = DirectoryInfo::global_handle_if_ready()
        && let Some(logs_dir) = directories.launcher_logs_dir()
        && tokio::fs::try_exists(&logs_dir).await?
    {
        let mut entries = tokio::fs::read_dir(&logs_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            // Scratch files from an interrupted log rewrite are not logs.
            if file_name.ends_with(".log.pruning") {
                continue;
            }
            write_zip_file(
                &mut writer,
                &format!("launcher_logs/{file_name}"),
                &entry.path(),
            )
            .await?;
        }
    }

    writer.close().await.map_err(zip_error)?;
    Ok(())
}

/// Time window included in a launcher log export.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogExportRange {
    Last30Minutes,
    Last2Hours,
    All,
}

impl LogExportRange {
    fn max_age(self) -> Option<std::time::Duration> {
        match self {
            Self::Last30Minutes => {
                Some(std::time::Duration::from_secs(30 * 60))
            }
            Self::Last2Hours => {
                Some(std::time::Duration::from_secs(2 * 60 * 60))
            }
            Self::All => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Last30Minutes => "last 30 minutes",
            Self::Last2Hours => "last 2 hours",
            Self::All => "all sessions",
        }
    }
}

/// Lowest level included in a launcher log export.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogExportLevel {
    All,
    Debug,
    Info,
}

impl LogExportLevel {
    fn minimum(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Debug => Some("debug"),
            Self::Info => Some("info"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all levels",
            Self::Debug => "debug and above",
            Self::Info => "info and above",
        }
    }
}

/// Instance log tail included in an export, in bytes.
const LOG_EXPORT_INSTANCE_LOG_TAIL_BYTES: u64 = 512 * 1024;

/// Export launcher logs (filtered by time range and level) plus optional
/// system information, the most recent instance log, and a crash analysis
/// summary and fresh network connection tests into a ZIP archive.
#[tauri::command]
pub async fn export_launcher_logs(
    output_path: PathBuf,
    range: LogExportRange,
    level: LogExportLevel,
    include_system_info: bool,
    include_instance_logs: bool,
    include_crash_analysis: bool,
    include_network_connections: bool,
) -> Result<()> {
    // Censoring needs the app state (credentials, IPs). Without it an export
    // would silently ship secrets, so fail rather than degrade.
    let state = theseus::State::get().await?;
    let archive = tokio::fs::File::create(&output_path).await?;
    let mut writer = ZipFileWriter::with_tokio(archive);

    let (exported_logs, skipped_logs) =
        write_exported_logs(&mut writer, range, level, &state).await?;

    let mut manifest = format!(
        "Axolotl Launcher log export\n\
         Exported at: {}\n\
         App version: {}\n\
         Time range: {}\n\
         Level filter: {}\n\
         System information: {}\n\
         Recent instance log: {}\n\
         Crash analysis: {}\n\
         Network connections: {}\n\
         Log files: {exported_logs}\n\n\
         Access tokens and Minecraft tokens are replaced with placeholders.\n\
         IP addresses are hidden in logs and preserved in network connection reports.\n",
        chrono::Local::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        range.label(),
        level.label(),
        if include_system_info {
            "included"
        } else {
            "omitted"
        },
        if include_instance_logs {
            "included"
        } else {
            "omitted"
        },
        if include_crash_analysis {
            "included"
        } else {
            "omitted"
        },
        if include_network_connections {
            "included (fresh tests using launcher network settings)"
        } else {
            "omitted"
        },
    );
    if !skipped_logs.is_empty() {
        manifest.push_str("\nSkipped unreadable log files:\n");
        for name in &skipped_logs {
            manifest.push_str(&format!("- {name}\n"));
        }
    }
    write_zip_entry(&mut writer, "manifest.txt", manifest.as_bytes()).await?;

    if include_system_info {
        let environment = build_environment_report();
        let environment = censor_export_text(environment, &state).await?;
        write_zip_entry(
            &mut writer,
            "system-information.txt",
            environment.as_bytes(),
        )
        .await?;
    }

    let recent_instance = if include_instance_logs || include_crash_analysis {
        newest_instance_log(&state).await
    } else {
        None
    };

    if include_instance_logs
        && let Some((instance_id, path)) = recent_instance.as_ref()
    {
        let tail = read_file_tail(path, LOG_EXPORT_INSTANCE_LOG_TAIL_BYTES)?;
        let tail = censor_export_text(tail, &state).await?;
        write_zip_entry(
            &mut writer,
            &format!("minecraft/{instance_id}/latest.log"),
            tail.as_bytes(),
        )
        .await?;
    }

    if include_crash_analysis
        && let Some((instance_id, _)) = recent_instance.as_ref()
        && let Ok(analysis) = theseus::logs::analyze_crash(instance_id).await
        && (analysis.crashed || !analysis.findings.is_empty())
        && let Ok(report) = serde_json::to_vec_pretty(&analysis)
    {
        // The analysis carries raw log text and Windows event messages, so it
        // goes through the same censoring as the exported logs.
        let report = censor_export_text(
            String::from_utf8_lossy(&report).into_owned(),
            &state,
        )
        .await?;
        write_zip_entry(
            &mut writer,
            &format!("minecraft/{instance_id}/crash-analysis.json"),
            report.as_bytes(),
        )
        .await?;
    }

    if include_network_connections {
        let report = build_network_report(|target| {
            theseus::test_network_connection(
                target,
                theseus::NetworkTestMode::Launcher,
            )
        })
        .await;
        let report = serde_json::to_string_pretty(&report)
            .map_err(theseus::Error::from)?;
        let report =
            theseus::install::censor_shared_credentials(report, &state).await?;
        write_zip_entry(
            &mut writer,
            "network-connections.json",
            report.as_bytes(),
        )
        .await?;
    }

    writer.close().await.map_err(zip_error)?;
    Ok(())
}

async fn build_network_report<F, Fut>(mut probe: F) -> serde_json::Value
where
    F: FnMut(theseus::NetworkTestTarget) -> Fut,
    Fut: std::future::Future<
            Output = theseus::Result<theseus::NetworkTestResult>,
        >,
{
    use theseus::NetworkTestTarget;

    let generated_at = chrono::Utc::now().to_rfc3339();
    let mut tests = Vec::new();
    // Keep the download sample from competing with other probes for bandwidth.
    for target in [
        NetworkTestTarget::CurseforgeApi,
        NetworkTestTarget::ModrinthApi,
        NetworkTestTarget::ModrinthCdn,
        NetworkTestTarget::MojangApi,
        NetworkTestTarget::MinecraftServices,
    ] {
        tests.push(match probe(target).await {
            Ok(result) => serde_json::json!({
                "target": target,
                "state": "done",
                "result": result,
            }),
            // Configuration errors may contain proxy credentials; retain only a safe code.
            Err(_) => serde_json::json!({
                "target": target,
                "state": "failed",
                "error": "request",
            }),
        });
    }

    serde_json::json!({
        "schema_version": 1,
        "generated_at": generated_at,
        "tests": tests,
    })
}

#[cfg(test)]
mod network_export_tests {
    use super::*;

    #[tokio::test]
    async fn exports_fresh_results_and_continues_after_setup_failure() {
        let mut attempts = Vec::new();
        for _ in 0..2 {
            let report = build_network_report(|target| {
                attempts.push(serde_json::to_value(target).unwrap());
                async move {
                    if matches!(target, theseus::NetworkTestTarget::ModrinthApi)
                    {
                        return Err(theseus::ErrorKind::InputError(
                            "proxy https://user:secret@localhost".to_string(),
                        )
                        .as_error());
                    }
                    Ok(theseus::NetworkTestResult {
                        target,
                        mode: theseus::NetworkTestMode::Launcher,
                        tested_at: "2026-10-19T00:00:00Z".to_string(),
                        launcher_version: "test".to_string(),
                        platform: "test".to_string(),
                        url: "https://example.com".to_string(),
                        proxy_mode: "none".to_string(),
                        dns_mode: "system".to_string(),
                        tls_verification: true,
                        resolved_ips: vec!["192.0.2.1".to_string()],
                        dns_ms: 1,
                        dns_error: None,
                        peer_ip: Some("192.0.2.1".to_string()),
                        status: Some(200),
                        response_ms: Some(2),
                        total_ms: 3,
                        http_version: Some("HTTP/2.0".to_string()),
                        server: None,
                        served_by: None,
                        cache: None,
                        bytes_received: 0,
                        sample_ms: None,
                        sample_mib_per_second: None,
                        error: None,
                    })
                }
            })
            .await;

            assert_eq!(report["schema_version"], 1);
            assert_eq!(report["tests"].as_array().unwrap().len(), 5);
            assert_eq!(report["tests"][0]["result"]["status"], 200);
            assert_eq!(report["tests"][0]["result"]["dns_ms"], 1);
            assert_eq!(report["tests"][0]["result"]["mode"], "launcher");
            assert_eq!(report["tests"][1]["state"], "failed");
            assert_eq!(report["tests"][1]["error"], "request");
            assert_eq!(report["tests"][4]["state"], "done");
            assert!(!report.to_string().contains("secret"));
        }
        assert_eq!(
            attempts,
            [
                "curseforge_api",
                "modrinth_api",
                "modrinth_cdn",
                "mojang_api",
                "minecraft_services",
            ]
            .repeat(2)
            .into_iter()
            .map(serde_json::Value::from)
            .collect::<Vec<_>>()
        );
    }
}

/// Writes the matching launcher logs and reports how many were written plus
/// the names of any files that could not be read.
async fn write_exported_logs(
    writer: &mut ZipFileWriter<tokio::fs::File>,
    range: LogExportRange,
    level: LogExportLevel,
    state: &theseus::State,
) -> Result<(usize, Vec<String>)> {
    let Some(directories) = DirectoryInfo::global_handle_if_ready() else {
        return Ok((0, Vec::new()));
    };
    let Some(logs_dir) = directories.launcher_logs_dir() else {
        return Ok((0, Vec::new()));
    };
    if !tokio::fs::try_exists(&logs_dir).await? {
        return Ok((0, Vec::new()));
    }

    let mut entries = tokio::fs::read_dir(&logs_dir).await?;
    let mut paths = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let is_session_log =
            path.file_name().and_then(|name| name.to_str()).is_some_and(
                |name| name.starts_with("session_") && name.ends_with(".log"),
            );
        if is_session_log && entry.file_type().await?.is_file() {
            paths.push(path);
        }
    }
    paths.sort();

    let mut exported = 0;
    let mut skipped = Vec::new();
    for path in paths {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "session.log".to_string());

        // A single unreadable file must not lose the whole report.
        let Ok(contents) = tokio::fs::read(&path).await else {
            skipped.push(file_name);
            continue;
        };
        let filtered = theseus::filter_log_contents(
            &contents,
            range.max_age(),
            level.minimum(),
        );
        let text = String::from_utf8_lossy(&filtered).into_owned();
        let censored = censor_export_text(text, state).await?;
        write_zip_entry(
            writer,
            &format!("launcher_logs/{file_name}"),
            censored.as_bytes(),
        )
        .await?;
        exported += 1;
    }

    Ok((exported, skipped))
}

/// Replaces credentials and IP addresses with placeholders. Censoring failures
/// propagate so an export never ships unredacted content.
async fn censor_export_text(
    text: String,
    state: &theseus::State,
) -> Result<String> {
    Ok(theseus::install::censor_shared_text(text, state).await?)
}

fn build_environment_report() -> String {
    format!(
        "Axolotl Launcher environment\n\
         App version: {}\n\
         Operating system: {}\n\
         Architecture: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// The instance whose log was written most recently, if any.
async fn newest_instance_log(
    state: &theseus::State,
) -> Option<(String, PathBuf)> {
    let instances_dir = state.directories.instances_dir();
    let mut entries = tokio::fs::read_dir(&instances_dir).await.ok()?;
    let mut newest: Option<(std::time::SystemTime, String, PathBuf)> = None;

    while let Some(entry) = entries.next_entry().await.ok()? {
        if !entry.file_type().await.ok()?.is_dir() {
            continue;
        }
        let log_path = entry.path().join("logs").join("latest.log");
        let Ok(metadata) = tokio::fs::metadata(&log_path).await else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let is_newer = newest
            .as_ref()
            .is_none_or(|(newest_modified, ..)| modified > *newest_modified);
        if is_newer {
            let id = entry.file_name().to_string_lossy().into_owned();
            newest = Some((modified, id, log_path));
        }
    }

    newest.map(|(_, id, path)| (id, path))
}

fn read_file_tail(path: &Path, max_bytes: u64) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length > max_bytes {
        file.seek(SeekFrom::Start(length - max_bytes))?;
    }

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    Ok(String::from_utf8_lossy(&contents).into_owned())
}

async fn write_zip_file(
    writer: &mut ZipFileWriter<tokio::fs::File>,
    filename: &str,
    path: &Path,
) -> Result<()> {
    let mut stream = writer
        .write_entry_stream(
            ZipEntryBuilder::new(
                filename.to_string().into(),
                Compression::Deflate,
            )
            .build(),
        )
        .await
        .map_err(zip_error)?
        .compat_write();
    let mut source = tokio::fs::File::open(path).await?;
    tokio::io::copy(&mut source, &mut stream).await?;
    stream.into_inner().close().await.map_err(zip_error)?;
    Ok(())
}

pub(crate) async fn write_zip_entry(
    writer: &mut ZipFileWriter<tokio::fs::File>,
    filename: &str,
    contents: &[u8],
) -> Result<()> {
    writer
        .write_entry_whole(
            ZipEntryBuilder::new(
                filename.to_string().into(),
                Compression::Deflate,
            ),
            contents,
        )
        .await
        .map_err(zip_error)?;
    Ok(())
}

pub(crate) fn zip_error(
    error: async_zip::error::ZipError,
) -> TheseusSerializableError {
    theseus::Error::from(theseus::ErrorKind::OtherError(format!(
        "Failed to create ZIP archive: {error}"
    )))
    .into()
}

#[tauri::command]
pub async fn show_app_db_backups_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let path = app_db_backup_dir()?;
    tokio::fs::create_dir_all(&path).await?;
    open_path(app, path).await;
    Ok(())
}

// Get opening command
// For example, if a user clicks on an .mrpack to open the app.
// This should be called once and only when the app is done booting up and ready to receive a command
// Returns a Command struct- see events.js
#[tauri::command]
#[cfg(target_os = "macos")]
pub async fn get_opening_command(
    state: tauri::State<'_, crate::macos::deep_link::InitialPayload>,
) -> Result<Option<CommandPayload>> {
    let payload = state.payload.lock().await;
    let cmd_arg = std::env::args_os()
        .nth(1)
        .map(|path| path.to_string_lossy().to_string());

    return if let Some(payload) = payload.as_ref() {
        tracing::info!("opening command {payload}");

        Ok(Some(handler::parse_command(payload).await?))
    } else if let Some(cmd_arg) = cmd_arg {
        tracing::info!("opening command {cmd_arg:?}");

        Ok(Some(handler::parse_command(&cmd_arg).await?))
    } else {
        Ok(None)
    };
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
pub async fn get_opening_command() -> Result<Option<CommandPayload>> {
    // Tauri is not CLI, we use arguments as path to file to call
    let cmd_arg = std::env::args_os().nth(1);

    tracing::info!("opening command {cmd_arg:?}");

    let cmd_arg = cmd_arg.map(|path| path.to_string_lossy().to_string());
    if let Some(cmd) = cmd_arg {
        tracing::debug!("Opening command: {:?}", cmd);
        return Ok(Some(handler::parse_command(&cmd).await?));
    }
    Ok(None)
}

#[tauri::command]
pub async fn get_minecraft_news(
    limit: Option<usize>,
) -> Result<Vec<theseus::minecraft_news::MinecraftNewsItem>> {
    Ok(
        theseus::minecraft_news::get_minecraft_news(limit.unwrap_or(12))
            .await?,
    )
}

// helper function called when redirected by a weblink (ie: modrith://do-something) or when redirected by a .mrpack file (in which case its a filepath)
// We hijack the deep link library (which also contains functionality for instance-checking)
pub async fn handle_command(command: String) -> Result<()> {
    tracing::info!("handle command: {command}");
    Ok(theseus::handler::parse_and_emit_command(&command).await?)
}

// Remove when (and if) https://github.com/tauri-apps/tauri/issues/12022 is implemented
pub(crate) fn tauri_convert_file_src(path: &Path) -> Result<Url> {
    #[cfg(any(windows, target_os = "android"))]
    const BASE: &str = "http://asset.localhost/";
    #[cfg(not(any(windows, target_os = "android")))]
    const BASE: &str = "asset://localhost/";

    macro_rules! theseus_try {
        ($test:expr) => {
            match $test {
                Ok(val) => val,
                Err(e) => {
                    return Err(TheseusSerializableError::Theseus(e.into()))
                }
            }
        };
    }

    let path = theseus_try!(canonicalize(path));
    let path = path.to_string_lossy();
    let encoded = urlencoding::encode(&path);

    Ok(theseus_try!(Url::parse(&format!("{BASE}{encoded}"))))
}
