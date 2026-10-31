use super::{business_error, page};
use launcher_tools::{LogKind, ToolError, log_text};
use serde_json::{Value, json};

fn native(kind: LogKind) -> theseus::logs::LogType {
    match kind {
        LogKind::Info => theseus::logs::LogType::InfoLog,
        LogKind::Crash => theseus::logs::LogType::CrashReport,
        LogKind::Jvm => theseus::logs::LogType::JvmCrash,
        LogKind::Launcher => theseus::logs::LogType::LauncherLog,
    }
}

pub(super) async fn list(
    instance_id: String,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let logs = theseus::logs::get_logs(&instance_id, Some(true))
        .await
        .map_err(business_error)?;
    page(logs.into_iter().filter(|log| log_text::valid_filename(&log.filename)).map(|log| json!({
        "filename":log.filename,"modified_at":log.age,"log_type":match log.log_type {
            theseus::logs::LogType::InfoLog => "info", theseus::logs::LogType::CrashReport => "crash",
            theseus::logs::LogType::JvmCrash => "jvm", theseus::logs::LogType::LauncherLog => "launcher",
        }
    })).collect(), offset, limit)
}

async fn document(
    instance_id: &str,
    log_type: LogKind,
    filename: &str,
) -> Result<theseus::logs::LogDocument, ToolError> {
    // Exact listing membership protects the flat API from arbitrary files in
    // the game root, where JVM and launcher logs live alongside other files.
    if !log_text::valid_filename(filename) {
        return Err(ToolError::new("invalid_filename", "Invalid log filename"));
    }
    let kind = native(log_type);
    let logs = theseus::logs::get_logs(instance_id, Some(true))
        .await
        .map_err(business_error)?;
    if !logs
        .iter()
        .any(|log| log.log_type == kind && log.filename == filename)
    {
        return Err(ToolError::new(
            "not_found",
            "Log file not found in this instance and log type",
        ));
    }
    theseus::logs::read_log_document(instance_id, kind, filename)
        .await
        .map_err(business_error)
}

pub(super) async fn read(
    instance_id: String,
    log_type: LogKind,
    filename: String,
    offset: usize,
    max_bytes: usize,
) -> Result<Value, ToolError> {
    let doc = document(&instance_id, log_type, &filename).await?;
    Ok(log_text::read(
        doc.output.as_str(),
        offset,
        max_bytes,
        doc.source_truncated,
    ))
}

pub(super) async fn search(
    instance_id: String,
    log_type: LogKind,
    filename: String,
    query: String,
    start_line: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let doc = document(&instance_id, log_type, &filename).await?;
    Ok(log_text::search(
        doc.output.as_str(),
        &query,
        start_line,
        limit,
        doc.source_truncated,
    ))
}
