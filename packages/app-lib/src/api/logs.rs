use std::fmt::Write as _;
use std::io::{BufRead, Read, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};

use crate::{
    State,
    prelude::Credentials,
    state::instances::adapters::sqlite::instance_rows,
    util::io::{self, IOError},
};

mod analysis_settings;
pub use analysis_settings::{
    CrashAnalysisAiSettings, get_crash_analysis_ai_settings,
    update_crash_analysis_ai_settings,
};

pub mod logagent;

mod crash_analysis;
pub use crash_analysis::{
    CrashAnalysis, CrashAnalysisEvidence, CrashAnalysisFinding,
    CrashAnalysisMod, CrashAnalysisSource, CrashModChange,
    CrashModChangeCounts, analyze_crash, save_successful_mod_snapshot,
    undo_added_mod,
};

mod logshare;
pub use logshare::{
    LogShareDeleteResponse, LogShareSettings, LogShareUploadResponse,
    SharedLog, analyse_crash_direct, delete_log, delete_shared_log,
    get_insights, get_log_share_settings, list_shared_logs, record_shared_log,
    update_log_share_settings, upload_crash,
};

#[derive(Serialize, Debug)]
pub struct Logs {
    pub log_type: LogType,
    pub filename: String,
    pub age: u64,
    pub output: Option<CensoredString>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub enum LogType {
    InfoLog,
    CrashReport,
    JvmCrash,
    LauncherLog,
}

fn isolated_minecraft_root(instance_root: &Path) -> Option<PathBuf> {
    let versions_dir = instance_root.parent()?;
    if versions_dir
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        versions_dir.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn log_directories_for_type(
    state: &State,
    game_dir: &Path,
    log_type: LogType,
) -> Vec<PathBuf> {
    match log_type {
        LogType::InfoLog => vec![state.directories.game_logs_dir(game_dir)],
        LogType::CrashReport => {
            vec![state.directories.game_crash_reports_dir(game_dir)]
        }
        LogType::JvmCrash => {
            let mut directories = vec![game_dir.to_path_buf()];
            if let Some(root) = isolated_minecraft_root(game_dir) {
                directories.push(root);
            }
            directories
        }
        LogType::LauncherLog => {
            let mut directories = vec![game_dir.to_path_buf()];
            if let Some(root) = isolated_minecraft_root(game_dir) {
                directories.push(root);
            }
            directories
        }
    }
}

fn is_log_file_for_type(log_type: LogType, file_name: &str) -> bool {
    match log_type {
        LogType::JvmCrash => {
            let lower = file_name.to_ascii_lowercase();
            lower.starts_with("hs_err") && lower.ends_with(".log")
        }
        LogType::LauncherLog => matches!(
            file_name.to_ascii_lowercase().as_str(),
            "launcher_log.txt" | "latest_stdout.log"
        ),
        _ => true,
    }
}

const LOG_COMPACTION_THRESHOLD: usize = 20;
const MAX_LOG_DISPLAY_BYTES: usize = 2 * 1024 * 1024;
const MAX_LOG_DISPLAY_LINE_BYTES: usize = 64 * 1024;
const LOG_DISPLAY_TRUNCATION_MARKER: &str =
    "\n… [log display truncated by Axolotl] …\n";

#[derive(Serialize, Debug)]
pub struct LatestLogCursor {
    pub cursor: u64,
    pub output: CensoredString,
    pub new_file: bool,
}

#[derive(Serialize, Debug)] // Not deserialize
#[serde(transparent)]
pub struct CensoredString(String);
impl CensoredString {
    pub fn censor(mut s: String, credentials_list: &[Credentials]) -> Self {
        let username = whoami::username();
        s = s
            .replace(&format!("/{username}/"), "/{COMPUTER_USERNAME}/")
            .replace(&format!("\\{username}\\"), "\\{COMPUTER_USERNAME}\\");
        for credentials in credentials_list {
            // Use the offline profile to guarantee that this function does not cause
            // Mojang API request, and is never delayed by a network request. The offline
            // profile is optimistically updated on upsert from time to time anyway
            if credentials.access_token.len() >= 8 {
                s = s.replace(
                    &credentials.access_token,
                    "{MINECRAFT_ACCESS_TOKEN}",
                );
            }
            s = s
                .replace(
                    &credentials.offline_profile.name,
                    "{MINECRAFT_USERNAME}",
                )
                .replace(
                    &credentials.offline_profile.id.as_simple().to_string(),
                    "{MINECRAFT_UUID}",
                )
                .replace(
                    &credentials.offline_profile.id.as_hyphenated().to_string(),
                    "{MINECRAFT_UUID}",
                );
        }

        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LogCompactionStats {
    compacted_runs: usize,
    compacted_lines: usize,
}

struct CompactedLog {
    output: String,
    stats: LogCompactionStats,
    display_truncated: bool,
}

async fn resolve_instance_path(
    instance: &str,
    state: &State,
) -> crate::Result<(String, Option<String>)> {
    let instance =
        match instance_rows::get_instance_by_id(instance, &state.pool).await? {
            Some(instance) => instance,
            None => {
                // Preserve the historical id-or-path lookup, with id winning.
                match instance_rows::get_instance_by_path(instance, &state.pool)
                    .await?
                {
                    Some(instance) => instance,
                    None => {
                        return Err(crate::ErrorKind::InputError(format!(
                            "Unknown instance id or path: {instance}"
                        ))
                        .as_error());
                    }
                }
            }
        };

    // Directly associated instances have no profile folder of their own:
    // their logs and crash reports live inside the externally managed
    // installation, at the real game directory the launch runs from. That
    // path is absolute, so `resolve_game_dir` returns it unchanged and
    // `instance_logs_dir` / `crash_reports_dir` naturally land on
    // `<game>/logs` / `<game>/crash-reports`.
    if instance.is_direct_linked() {
        if let Some(game_dir) = crate::launcher::linked_game_dir(&instance) {
            return Ok((game_dir.to_string_lossy().into_owned(), None));
        }
        tracing::warn!(
            instance_id = %instance.id,
            "Directly linked instance metadata is incomplete; falling back \
             to the recorded linked `.minecraft` root for log resolution"
        );
        if let Some(linked) = instance
            .linked_dot_minecraft
            .as_deref()
            .map(str::trim)
            .filter(|linked| !linked.is_empty())
        {
            return Ok((linked.to_string(), None));
        }
    }

    Ok((instance.path, instance.game_dir_override))
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(line) = line.strip_suffix("\r\n") {
        (line, "\r\n")
    } else if let Some(line) = line.strip_suffix('\n') {
        (line, "\n")
    } else if let Some(line) = line.strip_suffix('\r') {
        (line, "\r")
    } else {
        (line, "")
    }
}

fn push_compacted_log_run(
    output: &mut String,
    stats: &mut LogCompactionStats,
    line: &str,
    line_ending: &str,
    count: usize,
) {
    if count >= LOG_COMPACTION_THRESHOLD {
        output.push_str(line);
        let _ =
            write!(output, " (x{count} times - compacted by Axolotl Launcher)");
        output.push_str(line_ending);
        stats.compacted_runs += 1;
        stats.compacted_lines += count;
    } else {
        for _ in 0..count {
            output.push_str(line);
            output.push_str(line_ending);
        }
    }
}

fn read_compacted_log<R: BufRead>(
    reader: &mut R,
) -> std::io::Result<CompactedLog> {
    let mut output = String::new();
    let mut stats = LogCompactionStats::default();
    let mut buffer = Vec::new();
    let mut current_line: Option<String> = None;
    let mut current_line_ending = String::new();
    let mut current_count = 0usize;

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let line = String::from_utf8_lossy(&buffer);
        let (line, line_ending) = split_line_ending(&line);

        match current_line.as_deref() {
            Some(current) if current == line => {
                current_count += 1;
                if current_line_ending.is_empty() && !line_ending.is_empty() {
                    current_line_ending = line_ending.to_string();
                }
            }
            _ => {
                if let Some(current) = current_line.take() {
                    push_compacted_log_run(
                        &mut output,
                        &mut stats,
                        &current,
                        &current_line_ending,
                        current_count,
                    );
                }

                current_line = Some(line.to_string());
                current_line_ending = line_ending.to_string();
                current_count = 1;
            }
        }
    }

    if let Some(current) = current_line {
        push_compacted_log_run(
            &mut output,
            &mut stats,
            &current,
            &current_line_ending,
            current_count,
        );
    }

    Ok(CompactedLog {
        output,
        stats,
        display_truncated: false,
    })
}

fn compact_duplicate_lines(input: &str) -> CompactedLog {
    let mut reader = std::io::Cursor::new(input.as_bytes());
    read_compacted_log(&mut reader)
        .expect("compacting an in-memory log should not fail")
}

#[cfg(test)]
fn compact_log_for_display(input: &str) -> CompactedLog {
    cap_log_for_display(compact_duplicate_lines(input), false)
}

fn truncate_display_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }

    let retained_bytes =
        max_bytes.saturating_sub(LOG_DISPLAY_TRUNCATION_MARKER.len());
    let prefix_end = char_boundary_before(value, retained_bytes / 2);
    let suffix_start = char_boundary_after(
        value,
        value.len().saturating_sub(retained_bytes - prefix_end),
    );
    format!(
        "{}{}{}",
        &value[..prefix_end],
        LOG_DISPLAY_TRUNCATION_MARKER,
        &value[suffix_start..]
    )
}

fn char_boundary_before(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn char_boundary_after(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index < value.len() && !value.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn cap_log_for_display(
    mut compacted: CompactedLog,
    source_truncated: bool,
) -> CompactedLog {
    let mut output = String::with_capacity(
        compacted.output.len().min(MAX_LOG_DISPLAY_BYTES),
    );
    let mut truncated = source_truncated;

    for original_line in compacted.output.split_inclusive('\n') {
        let line =
            truncate_display_text(original_line, MAX_LOG_DISPLAY_LINE_BYTES);
        truncated |= line.len() != original_line.len();
        if output.len().saturating_add(line.len())
            > MAX_LOG_DISPLAY_BYTES
                .saturating_sub(LOG_DISPLAY_TRUNCATION_MARKER.len())
        {
            truncated = true;
            break;
        }
        output.push_str(&line);
    }

    if truncated {
        output.push_str(LOG_DISPLAY_TRUNCATION_MARKER);
    }
    compacted.output = output;
    compacted.display_truncated = truncated;
    compacted
}

fn read_limited_compacted_log<R: Read>(
    reader: &mut R,
) -> std::io::Result<CompactedLog> {
    let mut bytes = Vec::with_capacity(MAX_LOG_DISPLAY_BYTES);
    reader
        .take((MAX_LOG_DISPLAY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let source_truncated = bytes.len() > MAX_LOG_DISPLAY_BYTES;
    bytes.truncate(MAX_LOG_DISPLAY_BYTES);
    let input = String::from_utf8_lossy(&bytes);
    Ok(cap_log_for_display(
        compact_duplicate_lines(&input),
        source_truncated,
    ))
}

fn format_count(count: usize) -> String {
    let raw = count.to_string();
    let mut formatted = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, character) in raw.chars().enumerate() {
        if index > 0 && (raw.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
}

async fn maybe_emit_log_compaction_warning(
    file_name: &str,
    stats: LogCompactionStats,
) {
    if stats.compacted_runs == 0 {
        return;
    }

    let _ = crate::event::emit::emit_warning(&format!(
        "Axolotl Launcher has compacted {} repeated log lines in {} before displaying it for performance reasons.",
        format_count(stats.compacted_lines),
        file_name,
    ))
    .await;
}

async fn maybe_emit_log_display_truncation_warning(
    file_name: &str,
    truncated: bool,
) {
    if !truncated {
        return;
    }

    let _ = crate::event::emit::emit_warning(&format!(
        "Axolotl Launcher truncated {file_name} before displaying it to keep the console responsive. The original log file remains unchanged.",
    ))
    .await;
}

impl Logs {
    async fn build(
        log_type: LogType,
        age: SystemTime,
        game_dir: &Path,
        filename: String,
        clear_contents: Option<bool>,
    ) -> crate::Result<Self> {
        Ok(Self {
            log_type,
            age: age
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_secs(),
            output: if clear_contents.unwrap_or(false) {
                None
            } else {
                let state = State::get().await?;
                Some(
                    get_output_by_filename_from_path(
                        &state, game_dir, log_type, &filename,
                    )
                    .await?,
                )
            },
            filename,
        })
    }
}

#[tracing::instrument]
pub async fn get_logs_from_type(
    instance_id: &str,
    log_type: LogType,
    clear_contents: Option<bool>,
    logs: &mut Vec<crate::Result<Logs>>,
) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());

    let mut seen_files = std::collections::HashSet::new();
    for logs_folder in log_directories_for_type(&state, &game_dir, log_type) {
        if !logs_folder.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&logs_folder)
            .map_err(|e| IOError::with_path(e, &logs_folder))?
        {
            let entry: std::fs::DirEntry =
                entry.map_err(|e| IOError::with_path(e, &logs_folder))?;
            let age = entry
                .metadata()?
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(file_name) = path.file_name() {
                let file_name = file_name.to_string_lossy().to_string();
                if !is_log_file_for_type(log_type, &file_name) {
                    continue;
                }
                if !seen_files.insert(file_name.clone()) {
                    continue;
                }
                logs.push(
                    Logs::build(
                        log_type,
                        age,
                        &game_dir,
                        file_name,
                        clear_contents,
                    )
                    .await,
                );
            }
        }
    }
    Ok(())
}

#[tracing::instrument]
pub async fn get_logs(
    instance_id: &str,
    clear_contents: Option<bool>,
) -> crate::Result<Vec<Logs>> {
    let mut logs = Vec::new();
    get_logs_from_type(
        instance_id,
        LogType::InfoLog,
        clear_contents,
        &mut logs,
    )
    .await?;
    get_logs_from_type(
        instance_id,
        LogType::CrashReport,
        clear_contents,
        &mut logs,
    )
    .await?;
    get_logs_from_type(
        instance_id,
        LogType::JvmCrash,
        clear_contents,
        &mut logs,
    )
    .await?;
    get_logs_from_type(
        instance_id,
        LogType::LauncherLog,
        clear_contents,
        &mut logs,
    )
    .await?;

    let mut logs = logs.into_iter().collect::<crate::Result<Vec<Logs>>>()?;
    logs.sort_by(|a, b| b.age.cmp(&a.age).then(b.filename.cmp(&a.filename)));
    Ok(logs)
}

#[tracing::instrument]
pub async fn get_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> crate::Result<Logs> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());

    let path = log_directories_for_type(&state, &game_dir, log_type)
        .into_iter()
        .map(|directory| directory.join(&filename))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Log file not found: {filename}"
            ))
        })?;

    let metadata = std::fs::metadata(&path)?;
    let age = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    Logs::build(log_type, age, &game_dir, filename, Some(true)).await
}

async fn get_output_by_filename_from_path(
    state: &State,
    game_dir: &Path,
    log_type: LogType,
    file_name: &str,
) -> crate::Result<CensoredString> {
    let path = log_directories_for_type(state, game_dir, log_type)
        .into_iter()
        .map(|directory| directory.join(file_name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Log file not found: {file_name}"
            ))
        })?;

    let credentials = Credentials::get_all(&state.pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    if let Some(ext) = path.extension() {
        if ext == "gz" {
            let file = std::fs::File::open(&path)
                .map_err(|e| IOError::with_path(e, &path))?;
            let gz =
                flate2::read::GzDecoder::new(std::io::BufReader::new(file));
            let mut reader = std::io::BufReader::new(gz);
            let compacted = read_limited_compacted_log(&mut reader)
                .map_err(|e| IOError::with_path(e, &path))?;
            maybe_emit_log_compaction_warning(file_name, compacted.stats).await;
            maybe_emit_log_display_truncation_warning(
                file_name,
                compacted.display_truncated,
            )
            .await;
            return Ok(CensoredString::censor(compacted.output, &credentials));
        } else if ext == "log" || ext == "txt" {
            let file = std::fs::File::open(&path)
                .map_err(|e| IOError::with_path(e, &path))?;
            let mut reader = std::io::BufReader::new(file);
            let compacted = read_limited_compacted_log(&mut reader)
                .map_err(|e| IOError::with_path(e, &path))?;
            maybe_emit_log_compaction_warning(file_name, compacted.stats).await;
            maybe_emit_log_display_truncation_warning(
                file_name,
                compacted.display_truncated,
            )
            .await;
            return Ok(CensoredString::censor(compacted.output, &credentials));
        }
    }
    Err(crate::ErrorKind::OtherError(format!(
        "File extension not supported: {}",
        path.display()
    ))
    .into())
}

#[tracing::instrument]
pub async fn get_output_by_filename(
    instance_id: &str,
    log_type: LogType,
    file_name: &str,
) -> crate::Result<CensoredString> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());
    get_output_by_filename_from_path(&state, &game_dir, log_type, file_name)
        .await
}

#[tracing::instrument]
pub async fn delete_logs(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());

    let logs_folder = state.directories.game_logs_dir(&game_dir);
    for entry in std::fs::read_dir(&logs_folder)
        .map_err(|e| IOError::with_path(e, &logs_folder))?
    {
        let entry = entry.map_err(|e| IOError::with_path(e, &logs_folder))?;
        let path = entry.path();
        if path.is_dir() {
            io::remove_dir_all(&path).await?;
        }
    }
    Ok(())
}

#[tracing::instrument]
pub async fn delete_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());

    let path = log_directories_for_type(&state, &game_dir, log_type)
        .into_iter()
        .map(|directory| directory.join(filename))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Log file not found: {filename}"
            ))
        })?;
    io::remove_file(&path).await?;
    Ok(())
}

#[tracing::instrument]
pub async fn get_live_log_buffer(
    instance_id: &str,
) -> crate::Result<CensoredString> {
    let state = State::get().await?;
    let lines = crate::state::get_log_buffer(instance_id);
    let joined = lines.join("\n");
    let compacted =
        cap_log_for_display(compact_duplicate_lines(&joined), false);

    let credentials = Credentials::get_all(&state.pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    maybe_emit_log_compaction_warning("live log", compacted.stats).await;
    maybe_emit_log_display_truncation_warning(
        "live log",
        compacted.display_truncated,
    )
    .await;
    Ok(CensoredString::censor(compacted.output, &credentials))
}

pub fn clear_live_log_buffer(instance_id: &str) {
    crate::state::remove_log_buffer(instance_id);
}

#[tracing::instrument]
pub async fn get_latest_log_cursor(
    instance_id: &str,
    cursor: u64, // 0 to start at beginning of file
) -> crate::Result<LatestLogCursor> {
    get_generic_live_log_cursor(instance_id, "launcher_log.txt", cursor).await
}

#[tracing::instrument]
pub async fn get_generic_live_log_cursor(
    instance_id: &str,
    log_file_name: &str,
    mut cursor: u64, // 0 to start at beginning of file
) -> crate::Result<LatestLogCursor> {
    let state = State::get().await?;
    let (instance_path, game_dir_override) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, game_dir_override.as_deref());
    let logs_folder = state.directories.game_logs_dir(&game_dir);
    let path = logs_folder.join(log_file_name);
    if !path.exists() {
        // Allow silent failure if latest.log doesn't exist (as the instance may have been launched, but not yet created the file)
        return Ok(LatestLogCursor {
            cursor: 0,
            new_file: false,
            output: CensoredString("".to_string()),
        });
    }

    let mut file = File::open(&path)
        .await
        .map_err(|e| IOError::with_path(e, &path))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|e| IOError::with_path(e, &path))?;

    let mut new_file = false;
    if cursor > metadata.len() {
        // Cursor is greater than file length, reset cursor to 0
        // Likely cause is that the file was rotated while the log was being read
        cursor = 0;
        new_file = true;
    }

    let requested_start = cursor;
    let unread_bytes = metadata.len().saturating_sub(cursor);
    let display_start = if unread_bytes > MAX_LOG_DISPLAY_BYTES as u64 {
        metadata.len().saturating_sub(MAX_LOG_DISPLAY_BYTES as u64)
    } else {
        cursor
    };
    let source_truncated = display_start != requested_start;
    let mut buffer = Vec::with_capacity(MAX_LOG_DISPLAY_BYTES);
    file.seek(SeekFrom::Start(display_start))
        .map_err(|e| IOError::with_path(e, &path))
        .await?; // Seek to cursor
    file.take(MAX_LOG_DISPLAY_BYTES as u64)
        .read_to_end(&mut buffer)
        .map_err(|e| IOError::with_path(e, &path))
        .await?; // Read to end of file
    let output = String::from_utf8_lossy(&buffer); // Convert to String
    let compacted =
        cap_log_for_display(compact_duplicate_lines(&output), source_truncated);
    let cursor = metadata.len(); // Consume skipped output as well as the display window.

    let credentials = Credentials::get_all(&state.pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    maybe_emit_log_compaction_warning(log_file_name, compacted.stats).await;
    maybe_emit_log_display_truncation_warning(
        log_file_name,
        compacted.display_truncated,
    )
    .await;
    let output = CensoredString::censor(compacted.output, &credentials);
    Ok(LatestLogCursor {
        cursor,
        new_file,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CreateDirectLinkInstance;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// The launcher state is a process-wide singleton; initialize it once and
    /// reuse it so `State::get()` resolves inside these APIs. The state root
    /// is intentionally leaked (`.keep()`) because the shared state outlives
    /// this function.
    async fn global_state() -> Arc<State> {
        if !State::initialized() {
            let root = TempDir::new().unwrap().keep();
            let _ =
                State::init_for_test(root.to_string_lossy().to_string()).await;
        }
        State::get().await.unwrap()
    }

    /// Creates a directly associated instance whose linked `.minecraft`
    /// lives in a fresh temp dir (generic dialect: launches from the linked
    /// `.minecraft/versions/<id>` directory).
    async fn create_direct_link_fixture(
        label: &str,
    ) -> (TempDir, crate::state::InstanceMetadata) {
        let state = global_state().await;
        let minecraft = TempDir::new().unwrap();
        let version_dir = minecraft.path().join("versions").join(label);
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join(format!("{label}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "id": label,
                "inheritsFrom": "1.20.1",
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();

        let instance = crate::state::create_direct_link_instance(
            CreateDirectLinkInstance {
                name: None,
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: format!("versions/{label}"),
                instance_path: None,
                game_dir_mode: None,
            },
            &state,
        )
        .await
        .unwrap();
        let metadata = crate::state::get_instance(&instance.id, &state.pool)
            .await
            .unwrap()
            .expect("metadata for created fixture");
        (minecraft, metadata)
    }

    #[tokio::test]
    async fn direct_link_instance_resolves_to_linked_game_dir() {
        let state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("logs-resolve").await;

        let (resolved, game_dir_override) =
            resolve_instance_path(&metadata.instance.id, &state)
                .await
                .unwrap();

        // The generic dialect resolves to the isolated version directory; the
        // absolute path keeps helpers inside the linked installation instead
        // of a ghost `profiles/<path>` folder.
        assert_eq!(
            resolved,
            minecraft
                .path()
                .join("versions")
                .join("logs-resolve")
                .to_string_lossy()
        );
        assert!(game_dir_override.is_none());
        assert_eq!(
            state.directories.instance_logs_dir(&resolved),
            minecraft
                .path()
                .join("versions")
                .join("logs-resolve")
                .join("logs")
        );
        assert_eq!(
            state.directories.crash_reports_dir(&resolved),
            minecraft
                .path()
                .join("versions")
                .join("logs-resolve")
                .join("crash-reports")
        );
        assert!(
            !state
                .directories
                .instances_dir()
                .join(&metadata.instance.path)
                .exists(),
            "no profile directory may be created for a direct-link instance"
        );
    }

    #[test]
    fn display_compaction_bounds_oversized_nbt_line() {
        let log = format!("NBT: {}", "你".repeat(100_000));

        let compacted = compact_log_for_display(&log);

        assert!(compacted.display_truncated);
        assert!(compacted.output.len() <= MAX_LOG_DISPLAY_BYTES);
        assert!(compacted.output.contains("truncated by Axolotl"));
        assert!(compacted.output.is_char_boundary(compacted.output.len()));
    }

    #[test]
    fn display_compaction_bounds_many_distinct_lines() {
        let log = (0..300_000)
            .map(|index| format!("line-{index}\n"))
            .collect::<String>();

        let compacted = compact_log_for_display(&log);

        assert!(compacted.display_truncated);
        assert!(compacted.output.len() <= MAX_LOG_DISPLAY_BYTES);
    }

    #[tokio::test]
    async fn ordinary_instance_still_resolves_to_relative_path() {
        let state = global_state().await;
        let metadata = crate::api::instance::create(
            format!("logs-normal {}", uuid::Uuid::new_v4()),
            "1.20.1".to_string(),
            crate::state::ModLoader::Vanilla,
            None,
            None,
            crate::state::InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();

        let (resolved, game_dir_override) =
            resolve_instance_path(&metadata.instance.id, &state)
                .await
                .unwrap();
        assert_eq!(
            resolved, metadata.instance.path,
            "ordinary instances keep resolving through their relative profile path"
        );
        assert!(game_dir_override.is_none());
    }

    #[tokio::test]
    async fn direct_link_instance_logs_are_enumerated_from_linked_root() {
        let state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("logs-enum").await;

        let version_dir = minecraft.path().join("versions").join("logs-enum");
        std::fs::create_dir_all(version_dir.join("logs")).unwrap();
        std::fs::write(
            version_dir.join("logs").join("latest.log"),
            b"[12:00:00] [Render thread/INFO]: Setting user\n",
        )
        .unwrap();
        std::fs::create_dir_all(version_dir.join("crash-reports")).unwrap();
        std::fs::write(
            version_dir
                .join("crash-reports")
                .join("crash-2026-01-02_03.04.05-server.txt"),
            b"---- Minecraft Crash Report ----\n",
        )
        .unwrap();

        let logs = get_logs(&metadata.instance.id, None).await.unwrap();
        let filenames = logs
            .iter()
            .map(|log| log.filename.as_str())
            .collect::<Vec<_>>();
        assert!(
            filenames.contains(&"latest.log"),
            "logs must be enumerated from the linked root: {filenames:?}"
        );
        assert!(
            filenames.contains(&"crash-2026-01-02_03.04.05-server.txt"),
            "crash reports must be enumerated from the linked root: {filenames:?}"
        );
        // The log browser must never consult the nonexistent profile folder.
        assert!(
            !state
                .directories
                .instances_dir()
                .join(&metadata.instance.path)
                .exists()
        );
    }

    #[tokio::test]
    async fn root_crash_logs_are_enumerated_and_read_from_linked_roots() {
        let _state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("logs-jvm").await;
        let version_dir = minecraft.path().join("versions").join("logs-jvm");
        let local_name = "hs_err_pid100.log";
        let shared_name = "hs_err_pid200.log";
        std::fs::write(
            version_dir.join(local_name),
            b"local JVM crash details\n",
        )
        .unwrap();
        std::fs::write(
            minecraft.path().join(shared_name),
            b"shared JVM crash details\n",
        )
        .unwrap();
        std::fs::write(
            version_dir.join("launcher_log.txt"),
            b"launcher crash details\n",
        )
        .unwrap();

        let logs = get_logs(&metadata.instance.id, Some(true)).await.unwrap();
        assert!(logs.iter().any(|log| {
            log.log_type == LogType::JvmCrash && log.filename == local_name
        }));
        assert!(logs.iter().any(|log| {
            log.log_type == LogType::JvmCrash && log.filename == shared_name
        }));
        assert!(logs.iter().any(|log| {
            log.log_type == LogType::LauncherLog
                && log.filename == "launcher_log.txt"
        }));

        let output = get_output_by_filename(
            &metadata.instance.id,
            LogType::JvmCrash,
            shared_name,
        )
        .await
        .unwrap();
        assert!(output.as_str().contains("shared JVM crash details"));
    }

    #[tokio::test]
    async fn logshare_collection_uses_relative_paths_without_diagnostics() {
        let _state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("logs-upload").await;
        let version_dir = minecraft.path().join("versions").join("logs-upload");
        std::fs::create_dir_all(version_dir.join("logs")).unwrap();
        std::fs::write(
            version_dir.join("logs").join("latest.log"),
            b"latest log content\n",
        )
        .unwrap();
        std::fs::create_dir_all(version_dir.join("crash-reports")).unwrap();
        std::fs::write(
            version_dir.join("crash-reports").join("crash-upload.txt"),
            b"crash report content\n",
        )
        .unwrap();
        std::fs::write(
            version_dir.join("hs_err_pid300.log"),
            b"JVM crash content\n",
        )
        .unwrap();

        let collected =
            crash_analysis::collect_crash_logs(&metadata.instance.id)
                .await
                .unwrap();
        let names = collected
            .sources
            .iter()
            .map(|source| source.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"latest.log"), "sources: {names:?}");
        assert!(
            names.contains(&"crash-reports/crash-upload.txt"),
            "sources: {names:?}"
        );
        assert!(names.contains(&"hs_err_pid300.log"), "sources: {names:?}");
        assert!(
            collected
                .combined_log
                .as_str()
                .contains("latest log content")
        );
    }

    #[tokio::test]
    async fn direct_link_crash_analysis_finds_linked_crash_reports() {
        let _state = global_state().await;
        let (minecraft, metadata) =
            create_direct_link_fixture("logs-crash").await;

        let version_dir = minecraft.path().join("versions").join("logs-crash");
        std::fs::create_dir_all(version_dir.join("logs")).unwrap();
        std::fs::write(
            version_dir.join("logs").join("latest.log"),
            b"[12:00:00] [Render thread/INFO]: Setting user\n",
        )
        .unwrap();
        std::fs::create_dir_all(version_dir.join("crash-reports")).unwrap();
        std::fs::write(
            version_dir
                .join("crash-reports")
                .join("crash-2026-01-02_03.04.05-server.txt"),
            b"---- Minecraft Crash Report ----\n\
              // Who set us up the TNT?\n\
              java.lang.OutOfMemoryError: Java heap space\n",
        )
        .unwrap();

        let analysis = crate::api::logs::analyze_crash(&metadata.instance.id)
            .await
            .unwrap();
        assert!(
            analysis
                .sources
                .iter()
                .any(|source| source.source_type == "crash_report"),
            "crash analysis must discover reports under the linked root"
        );
    }
}
