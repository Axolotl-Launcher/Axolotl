use crate::api::Result;
use async_zip::tokio::write::ZipFileWriter;
use std::path::PathBuf;
use theseus::logs::LogType;
use theseus::logs::{
    self, CensoredString, CrashAnalysis, CrashAnalysisAiSettings,
    LatestLogCursor, LogShareSettings, LogShareUploadResponse, Logs, SharedLog,
};

/*
A log is a struct containing the filename string, stdout, and stderr, as follows:

pub struct Logs {
    pub filename:  String,
    pub stdout: String,
    pub stderr: String,
}
*/

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("logs")
        .invoke_handler(tauri::generate_handler![
            logs_get_logs,
            logs_get_logs_by_filename,
            logs_get_output_by_filename,
            logs_delete_logs,
            logs_delete_logs_by_filename,
            logs_get_latest_log_cursor,
            logs_get_minecraft_latest_log_cursor,
            logs_get_live_log_buffer,
            logs_clear_live_log_buffer,
            logs_analyze_crash,
            logs_get_crash_analysis_ai_settings,
            logs_update_crash_analysis_ai_settings,
            logs_logagent_prepare,
            logs_logagent_model_turn,
            logs_logagent_tool,
            logs_logagent_cloud,
            logs_logagent_cancel,
            logs_undo_added_mod,
            logs_export_crash_context,
            logs_get_log_share_settings,
            logs_update_log_share_settings,
            logs_logshare_upload_crash,
            logs_logshare_get_insights,
            logs_logshare_analyse_crash_direct,
            logs_logshare_delete,
            logs_list_shared_logs,
            logs_record_shared_log,
            logs_delete_shared_log,
        ])
        .build()
}

/// Get all logs for an instance, sorted by filename.
#[tauri::command]
pub async fn logs_get_logs(
    instance_id: &str,
    clear_contents: Option<bool>,
) -> Result<Vec<Logs>> {
    let val = logs::get_logs(instance_id, clear_contents).await?;

    Ok(val)
}

/// Get a log struct for an instance by filename.
#[tauri::command]
pub async fn logs_get_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<Logs> {
    Ok(logs::get_logs_by_filename(instance_id, log_type, filename).await?)
}

/// Get the output for an instance by filename.
#[tauri::command]
pub async fn logs_get_output_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<CensoredString> {
    Ok(logs::get_output_by_filename(instance_id, log_type, &filename).await?)
}

/// Delete all logs for an instance.
#[tauri::command]
pub async fn logs_delete_logs(instance_id: &str) -> Result<()> {
    Ok(logs::delete_logs(instance_id).await?)
}

/// Delete a log for an instance by filename.
#[tauri::command]
pub async fn logs_delete_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<()> {
    Ok(logs::delete_logs_by_filename(instance_id, log_type, &filename).await?)
}

/// Get live log from a cursor
#[tauri::command]
pub async fn logs_get_latest_log_cursor(
    instance_id: &str,
    cursor: u64, // 0 to start at beginning of file
) -> Result<LatestLogCursor> {
    Ok(logs::get_latest_log_cursor(instance_id, cursor).await?)
}

/// Get Minecraft's logs/latest.log from a cursor.
#[tauri::command]
pub async fn logs_get_minecraft_latest_log_cursor(
    instance_id: &str,
    cursor: u64, // 0 to start at beginning of file
) -> Result<LatestLogCursor> {
    Ok(
        logs::get_generic_live_log_cursor(instance_id, "latest.log", cursor)
            .await?,
    )
}

/// Get all buffered live log lines for an instance.
#[tauri::command]
pub async fn logs_get_live_log_buffer(
    instance_id: &str,
) -> Result<CensoredString> {
    Ok(logs::get_live_log_buffer(instance_id).await?)
}

/// Clear the live log buffer for an instance.
#[tauri::command]
pub async fn logs_clear_live_log_buffer(instance_id: &str) -> Result<()> {
    logs::clear_live_log_buffer(instance_id);
    Ok(())
}

/// Collect and locally analyze the files produced by the instance's latest run.
#[tauri::command]
pub async fn logs_analyze_crash(instance_id: &str) -> Result<CrashAnalysis> {
    Ok(logs::analyze_crash(instance_id).await?)
}

#[tauri::command]
pub async fn logs_get_crash_analysis_ai_settings()
-> Result<CrashAnalysisAiSettings> {
    Ok(logs::get_crash_analysis_ai_settings().await?)
}

#[tauri::command]
pub async fn logs_update_crash_analysis_ai_settings(
    settings: CrashAnalysisAiSettings,
) -> Result<()> {
    Ok(logs::update_crash_analysis_ai_settings(settings).await?)
}

#[tauri::command]
pub async fn logs_logagent_prepare(
    instance_id: &str,
    run_id: String,
    source: String,
) -> Result<logs::logagent::PreparedSession> {
    Ok(logs::logagent::prepare(instance_id, run_id, source).await?)
}

#[tauri::command]
pub async fn logs_logagent_model_turn(
    run_id: &str,
    turn: theseus::ai::chat::ChatTurn,
    on_event: tauri::ipc::Channel<logs::logagent::AnalysisEvent>,
) -> Result<theseus::ai::chat::ChatMessage> {
    Ok(logs::logagent::model_turn(run_id, turn, |event| {
        let _ = on_event.send(event);
    })
    .await?)
}

#[tauri::command]
pub async fn logs_logagent_tool(
    run_id: &str,
    request: logs::logagent::ToolRequest,
) -> Result<serde_json::Value> {
    Ok(logs::logagent::tool(run_id, request)?)
}

#[tauri::command]
pub async fn logs_logagent_cloud(
    run_id: &str,
    ticket: Option<LogShareUploadResponse>,
    on_event: tauri::ipc::Channel<logs::logagent::AnalysisEvent>,
) -> Result<logs::logagent::CloudResult> {
    Ok(logs::logagent::cloud(run_id, ticket, |event| {
        let _ = on_event.send(event);
    })
    .await?)
}

#[tauri::command]
pub async fn logs_logagent_cancel(run_id: &str) -> Result<()> {
    Ok(logs::logagent::cancel(run_id)?)
}

#[tauri::command]
pub async fn logs_undo_added_mod(
    instance_id: &str,
    filename: &str,
    expected_hash: &str,
) -> Result<()> {
    Ok(logs::undo_added_mod(instance_id, filename, expected_hash).await?)
}

/// Export the latest run's censored diagnostic context as a ZIP archive.
#[tauri::command]
pub async fn logs_export_crash_context(
    instance_id: &str,
    output_path: PathBuf,
) -> Result<()> {
    let analysis = logs::analyze_crash(instance_id).await?;
    let archive = tokio::fs::File::create(&output_path).await?;
    let mut writer = ZipFileWriter::with_tokio(archive);
    let report = serde_json::to_vec_pretty(&analysis).map_err(|error| {
        theseus::Error::from(theseus::ErrorKind::OtherError(format!(
            "Failed to serialize crash analysis: {error}"
        )))
    })?;
    crate::api::utils::write_zip_entry(&mut writer, "analysis.json", &report)
        .await?;
    for source in &analysis.sources {
        let safe_name = source.filename.replace(['/', '\\'], "_");
        crate::api::utils::write_zip_entry(
            &mut writer,
            &format!("logs/{safe_name}"),
            source.content.as_str().as_bytes(),
        )
        .await?;
    }
    writer.close().await.map_err(crate::api::utils::zip_error)?;
    Ok(())
}

#[tauri::command]
pub async fn logs_get_log_share_settings() -> Result<LogShareSettings> {
    Ok(logs::get_log_share_settings().await?)
}

#[tauri::command]
pub async fn logs_update_log_share_settings(
    settings: LogShareSettings,
) -> Result<()> {
    Ok(logs::update_log_share_settings(settings).await?)
}

#[tauri::command]
pub async fn logs_logshare_upload_crash(
    instance_id: &str,
) -> Result<LogShareUploadResponse> {
    Ok(logs::upload_crash(instance_id).await?)
}

#[tauri::command]
pub async fn logs_logshare_get_insights(id: &str) -> Result<serde_json::Value> {
    Ok(logs::get_insights(id).await?)
}

#[tauri::command]
pub async fn logs_logshare_analyse_crash_direct(
    instance_id: &str,
) -> Result<serde_json::Value> {
    Ok(logs::analyse_crash_direct(instance_id).await?)
}

#[tauri::command]
pub async fn logs_logshare_delete(id: &str, token: &str) -> Result<()> {
    logs::delete_log(id, token).await?;
    Ok(())
}

#[tauri::command]
pub async fn logs_list_shared_logs() -> Result<Vec<SharedLog>> {
    Ok(logs::list_shared_logs().await?)
}

#[tauri::command]
pub async fn logs_record_shared_log(log: SharedLog) -> Result<()> {
    Ok(logs::record_shared_log(log).await?)
}

#[tauri::command]
pub async fn logs_delete_shared_log(id: String, token: String) -> Result<()> {
    Ok(logs::delete_shared_log(id, token).await?)
}
