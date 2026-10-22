/*
    tracing is set basd on the environment variable RUST_LOG=xxx, depending on the amount of logs to show
        ERROR > WARN > INFO > DEBUG > TRACE
    eg. RUST_LOG=info will show info, warn, and error logs
        RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)
        RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)

    Error messages returned to Tauri will display as traced error logs if they return an error.
    This will also include an attached span trace if the error is from a tracing error, and the level is set to info, debug, or trace

    on unix:
        RUST_LOG="theseus=trace" {run command}

    The default is theseus=show, meaning only logs from theseus will be displayed, and at the info or higher level.

*/

#[cfg(debug_assertions)]
const CONSOLE_LOG_MAX_LINES: usize = 5;
#[cfg(debug_assertions)]
const DEFAULT_CONSOLE_COLUMNS: usize = 80;
#[cfg(debug_assertions)]
const CONSOLE_TRUNCATION_MARKER: &str = "... [console output truncated]";

const LAUNCHER_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
const LAUNCHER_WARN_ERROR_MAX_BYTES: u64 = 30 * 1024 * 1024;
const LAUNCHER_LOG_MAX_FILES: usize = 5;
const LAUNCHER_LOG_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(3 * 24 * 60 * 60);

/// Logs younger than this are kept at full fidelity, including TRACE.
const LAUNCHER_LOG_FULL_FIDELITY_AGE: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);
/// Logs older than this drop TRACE; older than the debug window they also
/// drop DEBUG.
const LAUNCHER_LOG_DEBUG_FIDELITY_AGE: std::time::Duration =
    std::time::Duration::from_secs(2 * 60 * 60);
const LAUNCHER_LOG_PRUNE_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

/// Level used until the stored user preference is applied.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Dependency directives that stay pinned regardless of the user level, so
/// noisy third-party crates never drown out launcher logs.
const THIRD_PARTY_LOG_DIRECTIVES: [&str; 4] =
    ["h2=info", "hyper=info", "hyper_util=info", "sqlx=warn"];

type LogFilterReloadHandle = tracing_subscriber::reload::Handle<
    tracing_subscriber::EnvFilter,
    tracing_subscriber::Registry,
>;

static LOG_FILTER_RELOAD: std::sync::OnceLock<LogFilterReloadHandle> =
    std::sync::OnceLock::new();

fn log_level_directives(level: &str) -> String {
    format!("theseus={level},theseus_gui={level}")
}

fn add_third_party_directives(
    mut filter: tracing_subscriber::EnvFilter,
) -> tracing_subscriber::EnvFilter {
    for directive in THIRD_PARTY_LOG_DIRECTIVES {
        if let Ok(parsed) = directive.parse() {
            filter = filter.add_directive(parsed);
        }
    }
    filter
}

/// Filter used when the logger starts: `RUST_LOG` wins when present, otherwise
/// the given default level seeds collection until the stored preference is
/// applied.
fn initial_log_filter(
    default_level: &str,
) -> Option<tracing_subscriber::EnvFilter> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new(log_level_directives(
                default_level,
            ))
        });
    Some(add_third_party_directives(filter))
}

/// Validates a stored log level, returning its canonical directive value.
fn normalize_log_level(level: &str) -> crate::Result<&'static str> {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => Ok("error"),
        "warn" => Ok("warn"),
        "info" => Ok("info"),
        "debug" => Ok("debug"),
        "trace" => Ok("trace"),
        other => Err(crate::ErrorKind::InputError(format!(
            "Invalid log level {other:?}; expected one of error, warn, info, debug, trace"
        ))
        .into()),
    }
}

/// Applies the lowest level written to disk (and the console in debug builds).
///
/// The launcher starts logging at [`DEFAULT_LOG_LEVEL`] because the logger is
/// initialized before the database is available; the stored preference is
/// applied here once settings have loaded, and again whenever the user changes
/// it. `RUST_LOG` still takes precedence for the initial filter so developers
/// keep their escape hatch, but an explicit change from the settings page
/// always wins from then on.
pub fn set_log_level(level: &str) -> crate::Result<()> {
    let level = normalize_log_level(level)?;

    let Some(handle) = LOG_FILTER_RELOAD.get() else {
        // The logger was never started (or is not reloadable yet).
        return Ok(());
    };
    reload_log_level(handle, level)
}

fn reload_log_level(
    handle: &LogFilterReloadHandle,
    level: &str,
) -> crate::Result<()> {
    let level = normalize_log_level(level)?;
    let filter = add_third_party_directives(
        tracing_subscriber::EnvFilter::new(log_level_directives(level)),
    );
    handle.reload(filter).map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Failed to apply log level {level:?}: {error}"
        ))
        .into()
    })
}

#[derive(Clone)]
struct RotatingLogWriter {
    state: std::sync::Arc<std::sync::Mutex<RotatingLogState>>,
    normal_max_bytes: u64,
    warn_error_max_bytes: u64,
}

struct RotatingLogState {
    logs_dir: std::path::PathBuf,
    session_name: String,
    segment: usize,
    file: std::fs::File,
    bytes_written: u64,
    max_files: usize,
    max_age: std::time::Duration,
}

impl RotatingLogWriter {
    fn new(
        logs_dir: std::path::PathBuf,
        session_name: String,
        normal_max_bytes: u64,
        warn_error_max_bytes: u64,
        max_files: usize,
        max_age: std::time::Duration,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(&logs_dir)?;
        let path = rotating_log_path(&logs_dir, &session_name, 0);
        let file = open_log_file(&path)?;
        let bytes_written = file.metadata()?.len();
        let state = RotatingLogState {
            logs_dir,
            session_name,
            segment: 0,
            file,
            bytes_written,
            max_files: max_files.max(1),
            max_age,
        };
        let writer = Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(state)),
            normal_max_bytes: normal_max_bytes.max(1),
            warn_error_max_bytes: warn_error_max_bytes
                .max(normal_max_bytes)
                .max(1),
        };
        Ok(writer)
    }

    fn write_event(
        &self,
        buffer: &[u8],
        max_file_bytes: u64,
    ) -> std::io::Result<()> {
        let mut state = self.state.lock().map_err(|_| {
            std::io::Error::other("launcher log writer lock poisoned")
        })?;
        state.write_event(buffer, max_file_bytes)
    }

    fn flush(&self) -> std::io::Result<()> {
        let mut state = self.state.lock().map_err(|_| {
            std::io::Error::other("launcher log writer lock poisoned")
        })?;
        std::io::Write::flush(&mut state.file)
    }

    fn cleanup_old_logs(&self) {
        if let Ok(state) = self.state.lock() {
            let active_path = rotating_log_path(
                &state.logs_dir,
                &state.session_name,
                state.segment,
            );
            cleanup_launcher_logs(
                &state.logs_dir,
                state.max_files,
                state.max_age,
                &active_path,
            );
        }
    }
}

impl RotatingLogState {
    fn write_event(
        &mut self,
        buffer: &[u8],
        max_file_bytes: u64,
    ) -> std::io::Result<()> {
        if self.bytes_written > 0
            && self.bytes_written.saturating_add(buffer.len() as u64)
                > max_file_bytes.max(1)
        {
            self.rotate()?;
        }

        std::io::Write::write_all(&mut self.file, buffer)?;
        self.bytes_written =
            self.bytes_written.saturating_add(buffer.len() as u64);
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.file)?;
        self.segment += 1;
        let path =
            rotating_log_path(&self.logs_dir, &self.session_name, self.segment);
        self.file = open_log_file(&path)?;
        self.bytes_written = self.file.metadata()?.len();
        cleanup_launcher_logs(
            &self.logs_dir,
            self.max_files,
            self.max_age,
            &path,
        );
        Ok(())
    }
}

fn rotating_log_path(
    logs_dir: &std::path::Path,
    session_name: &str,
    segment: usize,
) -> std::path::PathBuf {
    if segment == 0 {
        logs_dir.join(format!("{session_name}.log"))
    } else {
        logs_dir.join(format!("{session_name}_{segment:03}.log"))
    }
}

fn open_log_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

fn cleanup_launcher_logs(
    logs_dir: &std::path::Path,
    max_files: usize,
    max_age: std::time::Duration,
    active_path: &std::path::Path,
) {
    cleanup_launcher_logs_at(
        logs_dir,
        max_files,
        max_age,
        active_path,
        std::time::SystemTime::now(),
    );
}

fn cleanup_launcher_logs_at(
    logs_dir: &std::path::Path,
    max_files: usize,
    max_age: std::time::Duration,
    active_path: &std::path::Path,
    now: std::time::SystemTime,
) {
    let Ok(entries) = std::fs::read_dir(logs_dir) else {
        return;
    };
    let mut logs = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !entry.file_type().ok()?.is_file()
                || !name.starts_with("session_")
                || path.extension()?.to_str()? != "log"
            {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            let created =
                metadata.created().or_else(|_| metadata.modified()).ok();
            Some((created, path))
        })
        .collect::<Vec<_>>();
    logs.retain(|(created, path)| {
        if path != active_path
            && created.is_some_and(|created| {
                launcher_log_is_expired(created, now, max_age)
            })
        {
            return std::fs::remove_file(path).is_err();
        }
        true
    });
    logs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    while logs.len() > max_files.max(1) {
        let Some(index) = logs.iter().position(|(_, path)| path != active_path)
        else {
            break;
        };
        let (_, path) = logs.remove(index);
        let _ = std::fs::remove_file(path);
    }
}

fn launcher_log_is_expired(
    created: std::time::SystemTime,
    now: std::time::SystemTime,
    max_age: std::time::Duration,
) -> bool {
    now.duration_since(created).is_ok_and(|age| age > max_age)
}

/// Lowest level kept for a given log age: full fidelity (TRACE) inside the
/// full window, DEBUG up to the debug window, and INFO beyond it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LogFidelity {
    Trace,
    Debug,
    Info,
}

fn log_fidelity_for_age(age: std::time::Duration) -> LogFidelity {
    if age <= LAUNCHER_LOG_FULL_FIDELITY_AGE {
        LogFidelity::Trace
    } else if age <= LAUNCHER_LOG_DEBUG_FIDELITY_AGE {
        LogFidelity::Debug
    } else {
        LogFidelity::Info
    }
}

/// Ordinal for the levels the launcher writes, ordered from most to least
/// verbose. Unknown tokens are ignored so unrecognized lines are never pruned.
fn log_level_ordinal(token: &str) -> Option<u8> {
    match token {
        "TRACE" => Some(0),
        "DEBUG" => Some(1),
        "INFO" => Some(2),
        "WARN" => Some(3),
        "ERROR" => Some(4),
        _ => None,
    }
}

fn minimum_kept_ordinal(fidelity: LogFidelity) -> u8 {
    match fidelity {
        LogFidelity::Trace => 0,
        LogFidelity::Debug => 1,
        LogFidelity::Info => 2,
    }
}

fn log_level_minimum_ordinal(level: &str) -> Option<u8> {
    match level {
        "trace" => Some(0),
        "debug" => Some(1),
        "info" => Some(2),
        "warn" => Some(3),
        "error" => Some(4),
        _ => None,
    }
}

/// How a log line relates to the surrounding entries.
enum LogEntryHeader {
    /// Timestamp and level parsed: the entry can be filtered.
    Classified(chrono::DateTime<chrono::FixedOffset>, u8),
    /// Timestamp parsed but the level is unknown: keep it rather than
    /// dropping content this logic does not understand.
    Unclassified,
}

/// Reads the timestamp and level of an entry line. Continuation lines of a
/// multi-line message carry no prefix and return `None`, so callers keep them
/// grouped with the entry they belong to.
fn parse_log_entry_header(line: &str) -> Option<LogEntryHeader> {
    let (timestamp, rest) = line.split_once(char::is_whitespace)?;
    let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    let level = rest.split_whitespace().next()?;
    Some(match log_level_ordinal(level) {
        Some(ordinal) => LogEntryHeader::Classified(timestamp, ordinal),
        None => LogEntryHeader::Unclassified,
    })
}

/// Keeps or drops whole log entries by a predicate over their timestamp and
/// level ordinal, returning `None` when nothing was dropped.
///
/// Multi-line messages are kept or dropped together with their header, the
/// bytes of kept lines are preserved exactly (no re-encoding), and entries
/// whose level cannot be classified are always kept.
fn filter_log_entries<F>(contents: &[u8], keep: F) -> Option<Vec<u8>>
where
    F: Fn(chrono::DateTime<chrono::FixedOffset>, u8) -> bool,
{
    let mut kept = Vec::with_capacity(contents.len());
    let mut changed = false;
    let mut keep_entry = true;

    for line in contents.split_inclusive(|byte| *byte == b'\n') {
        match parse_log_entry_header(&String::from_utf8_lossy(line)) {
            Some(LogEntryHeader::Classified(timestamp, ordinal)) => {
                keep_entry = keep(timestamp, ordinal);
            }
            Some(LogEntryHeader::Unclassified) => keep_entry = true,
            // Continuation line: it follows the decision of its entry.
            None => {}
        }

        if keep_entry {
            kept.extend_from_slice(line);
        } else {
            changed = true;
        }
    }

    changed.then_some(kept)
}

/// Drops log lines that fall outside the retention ladder.
fn prune_log_contents(
    contents: &[u8],
    now: chrono::DateTime<chrono::FixedOffset>,
) -> Option<Vec<u8>> {
    filter_log_entries(contents, |timestamp, ordinal| {
        let age = (now - timestamp).to_std().unwrap_or_default();
        ordinal >= minimum_kept_ordinal(log_fidelity_for_age(age))
    })
}

/// Filters launcher log bytes for an export: entries older than `max_age` and
/// entries below `min_level` are removed. Pass `None` for either bound to keep
/// everything on that axis. Multi-line messages stay intact.
pub fn filter_log_contents(
    contents: &[u8],
    max_age: Option<std::time::Duration>,
    min_level: Option<&str>,
) -> Vec<u8> {
    let now = chrono::Local::now().fixed_offset();
    let minimum = min_level.and_then(log_level_minimum_ordinal);

    filter_log_entries(contents, |timestamp, ordinal| {
        if let Some(minimum) = minimum
            && ordinal < minimum
        {
            return false;
        }
        if let Some(max_age) = max_age {
            let age = (now - timestamp).to_std().unwrap_or_default();
            if age > max_age {
                return false;
            }
        }
        true
    })
    .unwrap_or_else(|| contents.to_vec())
}

/// Path of the scratch file used while a log segment is being rewritten.
fn prune_temporary_path(path: &std::path::Path) -> std::path::PathBuf {
    path.with_extension("log.pruning")
}

/// Writes contents durably, so an interrupted rewrite can never leave a
/// partially written file behind.
fn write_file_durably(
    path: &std::path::Path,
    contents: &[u8],
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    std::io::Write::write_all(&mut file, contents)?;
    std::io::Write::flush(&mut file)?;
    file.sync_all()
}

/// Rewrites a rotated log file through a temporary sibling, so a crash during
/// pruning never leaves a truncated file behind.
fn prune_inactive_log_file(
    path: &std::path::Path,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> std::io::Result<()> {
    let contents = std::fs::read(path)?;
    let Some(pruned) = prune_log_contents(&contents, now) else {
        return Ok(());
    };

    let temporary = prune_temporary_path(path);
    if let Err(error) = write_file_durably(&temporary, &pruned) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

/// Rewrites the active segment by moving the pruned copy into place and
/// reopening it for appending.
///
/// Replacing the file instead of truncating it keeps a crash or a failed write
/// from shortening the log being written, and the replacement is only adopted
/// once the pruned copy is safely on disk. The writer holds the lock for the
/// whole swap, so no event can land in the discarded file.
fn prune_active_log_file(
    state: &mut RotatingLogState,
    path: &std::path::Path,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> std::io::Result<()> {
    let contents = std::fs::read(path)?;
    let Some(pruned) = prune_log_contents(&contents, now) else {
        return Ok(());
    };

    std::io::Write::flush(&mut state.file)?;

    let temporary = prune_temporary_path(path);
    if let Err(error) = write_file_durably(&temporary, &pruned) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    // Opening the replacement before the swap means a failure here leaves the
    // original segment untouched and still being appended to.
    let replacement = match open_log_file(&temporary) {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = std::fs::rename(&temporary, path) {
        drop(replacement);
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    state.file = replacement;
    state.bytes_written = pruned.len() as u64;
    Ok(())
}

/// Applies the retention ladder to every launcher log file, including the
/// active segment, and returns one message per file that could not be pruned.
///
/// Callers must hold the writer lock; failures are returned instead of logged
/// because emitting a tracing event here would deadlock on that same lock.
fn prune_launcher_logs(
    state: &mut RotatingLogState,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> Vec<String> {
    let active_path =
        rotating_log_path(&state.logs_dir, &state.session_name, state.segment);
    let Ok(entries) = std::fs::read_dir(&state.logs_dir) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        // A scratch file left behind by an interrupted rewrite is an orphan:
        // rewriting is serialized under the writer lock, so nothing else can
        // own it now.
        if name.starts_with("session_") && name.ends_with(".log.pruning") {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if name.starts_with("session_") && name.ends_with(".log") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut failures = Vec::new();
    for path in paths {
        let result = if path == active_path {
            prune_active_log_file(state, &path, now)
        } else {
            prune_inactive_log_file(&path, now)
        };
        if let Err(error) = result {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    failures
}

struct LogEventWriter {
    writer: RotatingLogWriter,
    buffer: Vec<u8>,
    max_file_bytes: u64,
}

impl LogEventWriter {
    fn commit(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.writer.write_event(&self.buffer, self.max_file_bytes)?;
        self.buffer.clear();
        Ok(())
    }
}

impl std::io::Write for LogEventWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.commit()?;
        self.writer.flush()
    }
}

impl Drop for LogEventWriter {
    fn drop(&mut self) {
        let _ = self.commit();
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingLogWriter {
    type Writer = LogEventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.event_writer(self.normal_max_bytes)
    }

    fn make_writer_for(
        &'a self,
        metadata: &tracing::Metadata<'_>,
    ) -> Self::Writer {
        let max_file_bytes = match *metadata.level() {
            tracing::Level::WARN | tracing::Level::ERROR => {
                self.warn_error_max_bytes
            }
            _ => self.normal_max_bytes,
        };
        self.event_writer(max_file_bytes)
    }
}

impl RotatingLogWriter {
    fn event_writer(&self, max_file_bytes: u64) -> LogEventWriter {
        LogEventWriter {
            writer: self.clone(),
            buffer: Vec::new(),
            max_file_bytes,
        }
    }

    /// Applies the retention ladder now. Failures are reported after the lock
    /// is released, since tracing into the held writer would deadlock.
    fn prune_old_logs(&self) {
        let failures = match self.state.lock() {
            Ok(mut state) => {
                let now = chrono::Local::now().fixed_offset();
                prune_launcher_logs(&mut state, now)
            }
            Err(_) => return,
        };
        for failure in failures {
            tracing::warn!("Failed to prune launcher log: {failure}");
        }
    }
}

#[cfg(debug_assertions)]
fn console_columns() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|columns| (40..=500).contains(columns))
        .unwrap_or(DEFAULT_CONSOLE_COLUMNS)
}

#[cfg(debug_assertions)]
fn next_console_token(input: &str, start: usize) -> (usize, usize, bool) {
    let bytes = input.as_bytes();
    if bytes[start] == 0x1b {
        let mut end = start + 1;
        if bytes.get(end) == Some(&b'[') {
            end += 1;
            while let Some(byte) = bytes.get(end) {
                end += 1;
                if (0x40..=0x7e).contains(byte) {
                    break;
                }
            }
        } else if end < bytes.len() {
            end += 1;
        }
        return (end, 0, false);
    }

    let character = input[start..].chars().next().expect("valid character");
    let end = start + character.len_utf8();
    let width = match character {
        '\n' | '\r' => 0,
        '\t' => 4,
        character if character.is_control() => 0,
        _ => 1,
    };
    (end, width, character == '\n')
}

#[cfg(debug_assertions)]
const MODRINTH_CONSOLE_MESSAGES: &[&str] = &[
    "Modrinth mirror resolved cached file",
    "Modrinth mirror redirected to official CDN; falling back to official source",
    "Modrinth mirror returned an unresolved redirect; falling back to official source",
    "Modrinth request attempt failed; retrying",
    "Modrinth mirror failed; falling back to official source",
    "Modrinth official request failed",
    "Modrinth CDN download progress",
    "Modrinth checksum validation failed; retrying",
    "Modrinth response body failed; retrying",
    "Modrinth mirror response failed; falling back to official source",
    "Modrinth official response failed",
    "Modrinth connection failed; retrying",
    "Modrinth mirror connection failed; falling back to official source",
    "Modrinth official connection failed",
    "Modrinth mirror redirected; treating as cache miss and falling back to official source",
];

#[cfg(debug_assertions)]
fn strip_console_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        let (end, _, _) = next_console_token(input, index);
        if input.as_bytes()[index] != 0x1b {
            output.push_str(&input[index..end]);
        }
        index = end;
    }

    output
}

#[cfg(debug_assertions)]
fn extract_console_field(input: &str, field: &str) -> Option<String> {
    let marker = format!(" {field}=");
    let value = input.rsplit_once(&marker)?.1;
    if let Some(value) = value.strip_prefix('"') {
        let mut escaped = false;
        for (index, character) in value.char_indices() {
            if character == '"' && !escaped {
                return Some(value[..index].to_string());
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
        }
        return Some(value.to_string());
    }

    Some(
        value
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string(),
    )
}

#[cfg(debug_assertions)]
fn compact_console_url(value: &str) -> String {
    const MAX_URL_CHARS: usize = 110;

    let value = value
        .split_once('?')
        .map(|(base, _)| format!("{base}?<query omitted>"))
        .unwrap_or_else(|| value.to_string());
    if value.chars().count() <= MAX_URL_CHARS {
        return value;
    }

    let mut output: String = value.chars().take(MAX_URL_CHARS - 3).collect();
    output.push_str("...");
    output
}

#[cfg(debug_assertions)]
fn compact_modrinth_console_event(input: &str) -> Option<String> {
    let input = strip_console_ansi(input);
    let (_, message) = MODRINTH_CONSOLE_MESSAGES
        .iter()
        .filter_map(|message| {
            input.find(message).map(|index| (index, *message))
        })
        .min_by_key(|(index, _)| *index)?;
    let level = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"]
        .into_iter()
        .find(|level| input.contains(&format!(" {level} ")))
        .unwrap_or("INFO");
    let source = extract_console_field(&input, "source");
    let mut output = format!("{level} {message}");

    if source.as_deref() == Some("Mirror") {
        let explicit_status = extract_console_field(&input, "mirror_status");
        let mirror_status = explicit_status.as_deref().unwrap_or_else(|| {
            if message.contains("redirected") {
                "cache_miss"
            } else if message.starts_with("Completed") {
                "completed"
            } else if message.contains("failed") {
                "failed"
            } else {
                "attempting"
            }
        });
        output.push_str(&format!(" mirror_status={mirror_status}"));
    }

    for field in [
        "source",
        "request_kind",
        "method",
        "status",
        "route",
        "attempt",
        "max_attempts",
        "url",
        "mirror_url",
        "redirect_url",
        "final_url",
        "cache_status",
        "elapsed_ms",
        "bytes",
        "downloaded_bytes",
        "expected_bytes",
    ] {
        let value = if field == "source" {
            source.clone()
        } else {
            extract_console_field(&input, field)
        };
        if let Some(mut value) = value {
            if matches!(
                field,
                "url" | "mirror_url" | "redirect_url" | "final_url"
            ) {
                value = compact_console_url(&value);
            }
            if value.chars().any(char::is_whitespace) {
                output.push_str(&format!(" {field}={value:?}"));
            } else {
                output.push_str(&format!(" {field}={value}"));
            }
        }
    }
    output.push('\n');
    Some(output)
}

#[cfg(debug_assertions)]
fn fits_within_console_lines(
    input: &str,
    columns: usize,
    max_lines: usize,
) -> bool {
    let mut index = 0;
    let mut line = 1;
    let mut column = 0;

    while index < input.len() {
        let (end, width, newline) = next_console_token(input, index);
        if newline {
            if end == input.len() {
                return true;
            }
            line += 1;
            column = 0;
        } else if width > 0 {
            if column + width > columns {
                line += 1;
                column = 0;
            }
            column += width;
        }
        if line > max_lines {
            return false;
        }
        index = end;
    }

    true
}

#[cfg(debug_assertions)]
fn truncate_console_event(
    input: &str,
    columns: usize,
    max_lines: usize,
) -> String {
    if columns == 0
        || max_lines == 0
        || fits_within_console_lines(input, columns, max_lines)
    {
        return input.to_string();
    }

    let marker_width = CONSOLE_TRUNCATION_MARKER.len().min(columns);
    let final_line_limit = columns.saturating_sub(marker_width);
    let mut index = 0;
    let mut line = 1;
    let mut column = 0;

    while index < input.len() {
        let (end, width, newline) = next_console_token(input, index);
        if newline {
            if line == max_lines {
                break;
            }
            line += 1;
            column = 0;
            index = end;
            continue;
        }

        if width > 0 && column + width > columns {
            if line == max_lines {
                break;
            }
            line += 1;
            column = 0;
        }
        if line == max_lines && column + width > final_line_limit {
            break;
        }
        column += width;
        index = end;
    }

    let mut output = input[..index].to_string();
    output.push_str("\x1b[0m");
    output.push_str(CONSOLE_TRUNCATION_MARKER);
    output.push('\n');
    output
}

#[cfg(debug_assertions)]
struct TruncatedConsoleWriter {
    stdout: std::io::Stdout,
}

#[cfg(debug_assertions)]
impl std::io::Write for TruncatedConsoleWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let input = String::from_utf8_lossy(buffer);
        let compacted = compact_modrinth_console_event(&input);
        let input = compacted.as_deref().unwrap_or(&input);
        let output = truncate_console_event(
            input,
            console_columns(),
            CONSOLE_LOG_MAX_LINES,
        );
        self.stdout.write_all(output.as_bytes())?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stdout)
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct CapturedLogWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLogWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn short_console_events_are_unchanged() {
        let input = "short log event\n";
        assert_eq!(truncate_console_event(input, 80, 5), input);
    }

    #[test]
    fn long_console_events_are_limited_to_five_visual_lines() {
        let input = format!("{}\n", "x".repeat(1_000));
        let output = truncate_console_event(&input, 80, 5);

        assert!(output.contains(CONSOLE_TRUNCATION_MARKER));
        assert!(fits_within_console_lines(&output, 80, 5));
        assert!(!output.contains(&"x".repeat(1_000)));
    }

    #[test]
    fn embedded_newlines_count_toward_console_limit() {
        let input = "one\ntwo\nthree\nfour\nfive\nsix\n";
        let output = truncate_console_event(input, 80, 5);

        assert!(output.contains(CONSOLE_TRUNCATION_MARKER));
        assert!(!output.contains("six"));
        assert!(fits_within_console_lines(&output, 80, 5));
    }

    #[test]
    fn ansi_sequences_do_not_count_as_visible_width() {
        let input = format!("\x1b[32m{}\x1b[0m\n", "x".repeat(500));
        let output = truncate_console_event(&input, 80, 5);

        assert!(output.starts_with("\x1b[32m"));
        assert!(output.contains("\x1b[0m"));
        assert!(output.contains(CONSOLE_TRUNCATION_MARKER));
        assert!(fits_within_console_lines(&output, 80, 5));
    }

    #[test]
    fn modrinth_events_drop_long_span_context_and_keep_route_fields() {
        let input = format!(
            "2026-07-20T14:20:45Z INFO generate_pack{{ids=\"{}\"}}:fetch{{method=GET url=\"https://cdn.modrinth.com/data/test/file.mrpack\"}}: Modrinth mirror resolved cached file source=Mirror request_kind=\"CDN\" method=GET url=https://mod.mcimirror.top/data/test/file.mrpack route=1 attempt=1 max_attempts=5\n",
            "x".repeat(1_000)
        );
        let output = compact_modrinth_console_event(&input).unwrap();

        assert!(
            output.starts_with("INFO Modrinth mirror resolved cached file")
        );
        assert!(output.contains("mirror_status=attempting"));
        assert!(output.contains("source=Mirror"));
        assert!(
            output.contains(
                "url=https://mod.mcimirror.top/data/test/file.mrpack"
            )
        );
        assert!(!output.contains(&"x".repeat(1_000)));
        assert!(fits_within_console_lines(&output, 80, 5));
    }

    #[test]
    fn modrinth_redirects_show_cache_miss_and_compact_long_urls() {
        let input = "2026-07-20T14:20:45Z WARN fetch: Modrinth mirror redirected; treating as cache miss and falling back to official source source=Mirror url=https://mod.mcimirror.top/data/test/file.mrpack?ids=very-long-query redirect_url=\"https://cdn.modrinth.com/data/test/file name.mrpack\" status=302 elapsed_ms=100\n";
        let output = compact_modrinth_console_event(input).unwrap();

        assert!(output.contains("mirror_status=cache_miss"));
        assert!(output.contains("status=302"));
        assert!(output.contains("?<query omitted>"));
        assert!(output.contains(
            "redirect_url=\"https://cdn.modrinth.com/data/test/file name.mrpack\""
        ));
        assert!(fits_within_console_lines(&output, 80, 5));
    }

    #[test]
    fn modrinth_cache_events_keep_explicit_status_and_urls() {
        let input = "2026-07-20T14:20:45Z INFO fetch: Modrinth mirror resolved cached file mirror_status=cache_hit source=Mirror mirror_url=https://mod.mcimirror.top/data/test/file.jar final_url=https://cache.mcimirror.top/data/test/file.jar cache_status=HIT status=200 elapsed_ms=100\n";
        let output = compact_modrinth_console_event(input).unwrap();

        assert!(output.contains("mirror_status=cache_hit"));
        assert!(output.contains(
            "mirror_url=https://mod.mcimirror.top/data/test/file.jar"
        ));
        assert!(output.contains(
            "final_url=https://cache.mcimirror.top/data/test/file.jar"
        ));
        assert!(output.contains("cache_status=HIT"));
        assert!(fits_within_console_lines(&output, 80, 5));
    }

    #[test]
    fn non_modrinth_events_are_not_compacted() {
        assert_eq!(
            compact_modrinth_console_event("INFO ordinary event\n"),
            None
        );
    }

    #[test]
    fn development_disk_logs_keep_complete_events_after_console_truncation() {
        use tracing_subscriber::prelude::*;

        let directory = tempfile::tempdir().unwrap();
        let session_name = "session_20261018_141000";
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            session_name.to_string(),
            LAUNCHER_LOG_MAX_BYTES,
            LAUNCHER_WARN_ERROR_MAX_BYTES,
            LAUNCHER_LOG_MAX_FILES,
            LAUNCHER_LOG_MAX_AGE,
        )
        .unwrap();
        let subscriber = tracing_subscriber::registry().with(Some(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer.clone()),
        ));
        let message =
            format!("{} end-of-event", "download diagnostic ".repeat(100));
        assert!(
            truncate_console_event(&message, 80, 5)
                .contains(CONSOLE_TRUNCATION_MARKER)
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!("{message}");
        });
        writer.flush().unwrap();
        let contents = std::fs::read_to_string(rotating_log_path(
            directory.path(),
            session_name,
            0,
        ))
        .unwrap();
        assert!(contents.contains(&message));
        assert!(!contents.contains(CONSOLE_TRUNCATION_MARKER));
        assert!(!contents.contains('\u{1b}'));
    }

    #[test]
    fn creating_writer_defers_history_maintenance() {
        let directory = tempfile::tempdir().unwrap();
        let old_path = directory.path().join("session_20260718_120000.log");
        let now = test_now();
        let contents =
            log_line(now, chrono::Duration::hours(3), "DEBUG", "old debug");
        std::fs::write(&old_path, &contents).unwrap();
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            "session_20260722_120000".to_string(),
            10_000,
            30_000,
            1,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&old_path).unwrap(), contents);
        writer.write_event(b"first event\n", 10_000).unwrap();
        writer.flush().unwrap();
        writer.cleanup_old_logs();
        writer.prune_old_logs();
        assert!(!old_path.exists());
        let active_path = directory.path().join("session_20260722_120000.log");
        assert_eq!(
            std::fs::read_to_string(active_path).unwrap(),
            "first event\n"
        );
    }

    #[test]
    fn launcher_logs_rotate_by_size_and_keep_only_recent_files() {
        let directory = tempfile::tempdir().unwrap();
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            "session_20260722_120000".to_string(),
            10,
            30,
            3,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
        )
        .unwrap();

        for index in 0..6 {
            writer
                .write_event(
                    format!("event{index}\n").as_bytes(),
                    writer.normal_max_bytes,
                )
                .unwrap();
        }
        writer.flush().unwrap();

        let mut logs = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        logs.sort();
        assert_eq!(logs.len(), 3);
        assert!(logs.iter().all(|path| path.metadata().unwrap().len() <= 10));
        assert!(
            logs.iter().any(
                |path| std::fs::read_to_string(path).unwrap() == "event5\n"
            )
        );
    }

    #[test]
    fn warn_and_error_events_use_the_complete_event_limit() {
        let directory = tempfile::tempdir().unwrap();
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            "session_20260722_120000".to_string(),
            10,
            30,
            10,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
        )
        .unwrap();

        writer.write_event(b"12345678", 10).unwrap();
        writer.write_event(b"abcdefgh", 30).unwrap();
        writer.write_event(b"ok", 10).unwrap();
        writer.write_event(&[b'w'; 29], 30).unwrap();
        writer.write_event(&[b'e'; 31], 30).unwrap();
        writer.flush().unwrap();

        let mut logs = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        logs.sort();
        let contents = logs
            .iter()
            .map(std::fs::read)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            contents,
            vec![
                b"12345678abcdefgh".to_vec(),
                b"ok".to_vec(),
                vec![b'w'; 29],
                vec![b'e'; 31],
            ]
        );
    }

    #[test]
    fn launcher_logs_older_than_three_days_are_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let old_path = directory.path().join("session_20260718_120000.log");
        let active_path = directory.path().join("session_20260722_120000.log");
        std::fs::write(&old_path, b"old").unwrap();
        std::fs::write(&active_path, b"active").unwrap();
        let created = old_path
            .metadata()
            .unwrap()
            .created()
            .or_else(|_| old_path.metadata().unwrap().modified())
            .unwrap();

        cleanup_launcher_logs_at(
            directory.path(),
            5,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
            &active_path,
            created + std::time::Duration::from_secs(4 * 24 * 60 * 60),
        );

        assert!(!old_path.exists());
        assert!(active_path.exists());
    }

    fn test_now() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::Local::now().fixed_offset()
    }

    fn log_line(
        now: chrono::DateTime<chrono::FixedOffset>,
        age: chrono::Duration,
        level: &str,
        message: &str,
    ) -> String {
        format!("{} {level} theseus: {message}\n", (now - age).to_rfc3339())
    }

    #[test]
    fn retention_ladder_degrades_old_log_levels() {
        let now = test_now();
        let contents = [
            log_line(now, chrono::Duration::minutes(1), "TRACE", "hot trace"),
            log_line(now, chrono::Duration::minutes(45), "TRACE", "cold trace"),
            log_line(now, chrono::Duration::minutes(45), "DEBUG", "cold debug"),
            log_line(now, chrono::Duration::hours(3), "DEBUG", "stale debug"),
            log_line(now, chrono::Duration::hours(3), "INFO", "stale info"),
            log_line(now, chrono::Duration::hours(3), "ERROR", "stale error"),
        ]
        .concat();

        let pruned = String::from_utf8(
            prune_log_contents(contents.as_bytes(), now)
                .expect("stale verbose lines are pruned"),
        )
        .unwrap();

        assert!(pruned.contains("hot trace"));
        assert!(!pruned.contains("cold trace"));
        assert!(pruned.contains("cold debug"));
        assert!(!pruned.contains("stale debug"));
        assert!(pruned.contains("stale info"));
        assert!(pruned.contains("stale error"));
    }

    #[test]
    fn retention_ladder_leaves_unprunable_contents_untouched() {
        let now = test_now();
        let contents = [
            log_line(now, chrono::Duration::minutes(1), "TRACE", "hot trace"),
            log_line(now, chrono::Duration::hours(3), "INFO", "stale info"),
        ]
        .concat();

        assert!(prune_log_contents(contents.as_bytes(), now).is_none());
    }

    #[test]
    fn multi_line_entries_are_pruned_as_one_unit() {
        let now = test_now();
        let stale = (now - chrono::Duration::hours(3)).to_rfc3339();
        let contents = format!(
            "{stale} DEBUG theseus: dropped header\n  dropped continuation\n{stale} INFO theseus: kept header\n  kept continuation\n"
        );

        let pruned = String::from_utf8(
            prune_log_contents(contents.as_bytes(), now)
                .expect("the stale debug entry is pruned"),
        )
        .unwrap();

        assert!(!pruned.contains("dropped header"));
        assert!(!pruned.contains("dropped continuation"));
        assert!(pruned.contains("kept header"));
        assert!(pruned.contains("kept continuation"));
    }

    #[test]
    fn pruning_the_active_segment_keeps_appends_working() {
        let directory = tempfile::tempdir().unwrap();
        let session_name = "session_20260722_120000".to_string();
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            session_name.clone(),
            10_000,
            30_000,
            5,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
        )
        .unwrap();

        let now = test_now();
        let contents = [
            log_line(now, chrono::Duration::hours(3), "TRACE", "stale trace"),
            log_line(now, chrono::Duration::minutes(1), "TRACE", "hot trace"),
        ]
        .concat();
        writer.write_event(contents.as_bytes(), 10_000).unwrap();

        writer.prune_old_logs();

        let path = directory.path().join(format!("{session_name}.log"));
        let pruned = std::fs::read_to_string(&path).unwrap();
        assert!(!pruned.contains("stale trace"));
        assert!(pruned.contains("hot trace"));
        assert_eq!(
            pruned.len() as u64,
            writer.state.lock().unwrap().bytes_written
        );

        writer.write_event(b"appended\n", 10_000).unwrap();
        writer.flush().unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .ends_with("appended\n")
        );
    }

    #[test]
    fn rotated_log_files_are_pruned_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let rotated = directory.path().join("session_20260722_110000.log");
        let now = test_now();
        let contents = [
            log_line(now, chrono::Duration::minutes(1), "TRACE", "hot trace"),
            log_line(now, chrono::Duration::hours(3), "DEBUG", "stale debug"),
        ]
        .concat();
        std::fs::write(&rotated, contents).unwrap();

        prune_inactive_log_file(&rotated, now).unwrap();

        let pruned = std::fs::read_to_string(&rotated).unwrap();
        assert!(pruned.contains("hot trace"));
        assert!(!pruned.contains("stale debug"));
        assert!(!rotated.with_extension("log.pruning").exists());
    }

    #[test]
    fn entries_with_unknown_levels_are_kept() {
        let now = test_now();
        let stale = (now - chrono::Duration::hours(3)).to_rfc3339();
        let fresh = (now - chrono::Duration::minutes(1)).to_rfc3339();
        let contents = format!(
            "{stale} DEBUG theseus: dropped header\n{stale} NOTICE theseus: unclassified line\n{fresh} TRACE theseus: kept trace\n"
        );

        let pruned = String::from_utf8(
            prune_log_contents(contents.as_bytes(), now)
                .expect("the stale debug entry is pruned"),
        )
        .unwrap();

        assert!(!pruned.contains("dropped header"));
        assert!(pruned.contains("unclassified line"));
        assert!(pruned.contains("kept trace"));
    }

    #[test]
    fn interrupted_rewrites_leave_no_scratch_files_behind() {
        let directory = tempfile::tempdir().unwrap();
        let session_name = "session_20260722_120000".to_string();
        let writer = RotatingLogWriter::new(
            directory.path().to_path_buf(),
            session_name.clone(),
            10_000,
            30_000,
            5,
            std::time::Duration::from_secs(3 * 24 * 60 * 60),
        )
        .unwrap();

        let orphan =
            directory.path().join(format!("{session_name}.log.pruning"));
        std::fs::write(&orphan, b"half written").unwrap();

        writer.prune_old_logs();

        assert!(!orphan.exists());
        assert!(
            directory
                .path()
                .join(format!("{session_name}.log"))
                .exists()
        );
    }

    #[test]
    fn log_levels_are_validated_before_being_applied() {
        assert_eq!(normalize_log_level(" TRACE ").unwrap(), "trace");
        assert_eq!(normalize_log_level("Debug").unwrap(), "debug");
        assert!(normalize_log_level("verbose").is_err());
        assert!(set_log_level("verbose").is_err());
        // No logger is installed in tests, so a valid level is a quiet no-op.
        assert!(set_log_level("debug").is_ok());
    }

    #[test]
    fn reloaded_log_level_changes_which_events_are_written() {
        use tracing_subscriber::prelude::*;

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = CapturedLogWriter(std::sync::Arc::clone(&captured));
        let (filter, reload_handle) = tracing_subscriber::reload::Layer::new(
            add_third_party_directives(tracing_subscriber::EnvFilter::new(
                log_level_directives("info"),
            )),
        );
        let subscriber = tracing_subscriber::registry().with(filter).with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_ansi(false)
                .with_target(false)
                .with_writer(move || writer.clone()),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(
                target: "theseus::logger::reload_test",
                "hidden before reload"
            );
            tracing::info!(
                target: "theseus::logger::reload_test",
                "visible before reload"
            );
            reload_log_level(&reload_handle, "debug").unwrap();
            tracing::debug!(
                target: "theseus::logger::reload_test",
                "visible after reload"
            );
        });

        let output =
            String::from_utf8(captured.lock().unwrap().clone()).unwrap();
        assert!(!output.contains("hidden before reload"));
        assert!(output.contains("visible before reload"));
        assert!(output.contains("visible after reload"));
    }
}

fn launcher_log_writer(app_identifier: &str) -> Option<RotatingLogWriter> {
    use crate::prelude::DirectoryInfo;
    use chrono::Local;
    // Initialize and get logs directory path
    let logs_dir = if let Some(d) =
        DirectoryInfo::launcher_logs_dir_path(app_identifier)
    {
        d
    } else {
        eprintln!("Could not start logger");
        return None;
    };

    let session_name =
        format!("session_{}", Local::now().format("%Y%m%d_%H%M%S"));
    match RotatingLogWriter::new(
        logs_dir,
        session_name,
        LAUNCHER_LOG_MAX_BYTES,
        LAUNCHER_WARN_ERROR_MAX_BYTES,
        LAUNCHER_LOG_MAX_FILES,
        LAUNCHER_LOG_MAX_AGE,
    ) {
        Ok(writer) => Some(writer),
        Err(e) => {
            eprintln!("Could not start launcher log writer: {e}");
            None
        }
    }
}

/// Saves complete session logs in all builds, with compact console output in development.
pub fn start_logger(app_identifier: &str) -> Option<()> {
    use tracing_subscriber::fmt::time::ChronoLocal;
    use tracing_subscriber::prelude::*;

    let writer = launcher_log_writer(app_identifier);
    #[cfg(not(debug_assertions))]
    if writer.is_none() {
        return None;
    }

    let filter = initial_log_filter(DEFAULT_LOG_LEVEL)?;
    let (filter_layer, reload_handle) =
        tracing_subscriber::reload::Layer::new(filter);

    let subscriber = tracing_subscriber::registry().with(filter_layer).with(
        writer.clone().map(|writer| {
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_timer(ChronoLocal::rfc_3339())
        }),
    );
    #[cfg(debug_assertions)]
    let subscriber =
        subscriber.with(tracing_subscriber::fmt::layer().with_writer(|| {
            TruncatedConsoleWriter {
                stdout: std::io::stdout(),
            }
        }));
    subscriber.with(tracing_error::ErrorLayer::default()).init();
    let _ = LOG_FILTER_RELOAD.set(reload_handle);

    // Retention degrades by age (30 minutes / 2 hours), so a coarse periodic
    // sweep is enough; rotating on size does not need to trigger one.
    if let Some(prune_writer) = writer {
        std::thread::Builder::new()
            .name("launcher-log-prune".to_string())
            .spawn(move || {
                // Let the shell begin creating its window before scanning history.
                std::thread::sleep(std::time::Duration::from_secs(2));
                loop {
                    prune_writer.cleanup_old_logs();
                    prune_writer.prune_old_logs();
                    std::thread::sleep(LAUNCHER_LOG_PRUNE_INTERVAL);
                }
            })
            .ok();
    }

    Some(())
}
