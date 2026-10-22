//! Console output buffering and streaming for servers.

use base64::Engine;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncSeekExt, BufReader};

use crate::Result;
use crate::api::servers::lifecycle::is_server_running;
use crate::event::emit::emit_server;
use crate::event::{ExitReason, ServerPayloadType};
use crate::state::{clear_log_buffer, push_log_line};

const MAX_PTY_LINE_BYTES: usize = 256 * 1024;
const MAX_SERVER_LOG_LINE_BYTES: usize = 64 * 1024;
const SERVER_LOG_TRUNCATION_MARKER: &str =
    " … [log output truncated by Axolotl] … ";

async fn read_bounded_server_log_line<R>(
    reader: &mut R,
) -> std::io::Result<Option<String>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    let mut saw_bytes = false;
    let mut truncated = false;

    loop {
        let (consumed, reached_line_end) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                if !saw_bytes {
                    return Ok(None);
                }
                break;
            }

            saw_bytes = true;
            let consumed = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            let maximum_content_bytes = MAX_SERVER_LOG_LINE_BYTES
                .saturating_sub(SERVER_LOG_TRUNCATION_MARKER.len() + 1);
            let remaining = maximum_content_bytes.saturating_sub(line.len());
            let copied = remaining.min(consumed);
            line.extend_from_slice(&available[..copied]);
            truncated |= copied < consumed;
            (consumed, available[consumed - 1] == b'\n')
        };
        reader.consume(consumed);
        if reached_line_end {
            break;
        }
    }

    if truncated {
        while matches!(line.last(), Some(b'\r' | b'\n')) {
            line.pop();
        }
        line.extend_from_slice(SERVER_LOG_TRUNCATION_MARKER.as_bytes());
        line.push(b'\n');
    }

    Ok(Some(String::from_utf8_lossy(&line).into_owned()))
}

pub async fn get_log_buffer(server_id: &str) -> Result<Vec<String>> {
    Ok(crate::state::get_log_buffer(server_id))
}

pub fn get_log_delta(
    server_id: &str,
    cursor: Option<crate::state::LogCursor>,
) -> crate::state::LogDelta {
    crate::state::get_log_delta(server_id, cursor)
}

pub async fn clear_log(server_id: &str) -> Result<()> {
    clear_log_buffer(server_id);
    Ok(())
}

pub(super) async fn stream_server_pty_output(
    server_id: String,
    mut reader: Box<dyn std::io::Read + Send>,
) {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if sender.blocking_send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut pending_line = Vec::new();
    let mut jna_hint_emitted = false;
    let mut handshake = super::console::ConsoleHandshake::new(cfg!(windows));
    while let Some(bytes) = receiver.recv().await {
        let (bytes, reply) = handshake.consume(&bytes);
        if reply {
            super::lifecycle::send_console_input(
                &server_id,
                &base64::engine::general_purpose::STANDARD.encode(b"\x1b[1;1R"),
            )
            .await
            .ok();
        }
        super::console::publish_console_output(&server_id, &bytes).await;

        for line in take_complete_pty_lines(&mut pending_line, &bytes) {
            let text = String::from_utf8_lossy(&line);
            process_server_output_line(
                &server_id,
                text.trim_end_matches(['\r', '\n']),
                &mut jna_hint_emitted,
            )
            .await;
        }
    }
    if !pending_line.is_empty() {
        process_server_output_line(
            &server_id,
            &String::from_utf8_lossy(&pending_line),
            &mut jna_hint_emitted,
        )
        .await;
    }
}

fn take_complete_pty_lines(
    pending: &mut Vec<u8>,
    bytes: &[u8],
) -> Vec<Vec<u8>> {
    pending.extend_from_slice(bytes);
    let Some(last_newline) = pending.iter().rposition(|byte| *byte == b'\n')
    else {
        if pending.len() > MAX_PTY_LINE_BYTES {
            pending.clear();
        }
        return Vec::new();
    };

    let remainder = pending.split_off(last_newline + 1);
    let completed = std::mem::replace(pending, remainder);
    let mut lines = Vec::new();
    for line in completed.split_inclusive(|byte| *byte == b'\n') {
        if line.len() <= MAX_PTY_LINE_BYTES {
            lines.push(line.to_vec());
        }
    }
    lines
}

async fn process_server_output_line(
    server_id: &str,
    raw_line: &str,
    jna_hint_emitted: &mut bool,
) {
    let cleaned = strip_ansi(raw_line);
    super::lifecycle::observe_server_output(server_id, &cleaned);
    if cleaned.is_empty()
        || is_timestamped_log_line(&cleaned)
        || cleaned.starts_with("> ")
    {
        return;
    }
    push_log_line(server_id, cleaned.clone());
    emit_server(server_id, ServerPayloadType::Log { line: cleaned })
        .await
        .ok();
    if !*jna_hint_emitted && is_jna_macos_assertion(raw_line) {
        *jna_hint_emitted = true;
        for hint in JNA_CRASH_HINT_LINES {
            push_log_line(server_id, hint.to_string());
            emit_server(
                server_id,
                ServerPayloadType::Log {
                    line: hint.to_string(),
                },
            )
            .await
            .ok();
        }
    }
}

/// Streams the server's `logs/latest.log` file into the console buffer. The
/// Minecraft/Fabric log4j console output is frequently not delivered through
/// the process stdout pipe (it goes to the log file instead), so tailing this
/// file is the authoritative, lossless source of the server's own logs. Lines
/// already present in the buffer (e.g. delivered via the stdout/stderr pipes)
/// are skipped to avoid duplicates.
pub(super) async fn tail_server_log_file(server_id: String, dir: PathBuf) {
    let log_path = dir.join("logs").join("latest.log");
    let mut reader = loop {
        if !is_server_running(&server_id) {
            return;
        }
        match File::open(&log_path).await {
            Ok(file) => break BufReader::new(file),
            // The file only appears once the server starts logging; poll until then.
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    };

    loop {
        if !is_server_running(&server_id) {
            return;
        }
        match read_bounded_server_log_line(&mut reader).await {
            Ok(None) => {
                // Caught up. Detect log rotation (file replaced/truncated) and
                // otherwise wait for more output to be appended.
                if let Ok(meta) = tokio::fs::metadata(&log_path).await
                    && let Ok(pos) = reader.stream_position().await
                    && meta.len() < pos
                    && let Ok(file) = File::open(&log_path).await
                {
                    reader = BufReader::new(file);
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Ok(Some(line)) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                let cleaned = strip_ansi(trimmed);
                super::lifecycle::observe_server_output(&server_id, &cleaned);
                let already_present =
                    crate::state::log_buffer_contains(&server_id, &cleaned);
                if !cleaned.is_empty() && !already_present {
                    push_log_line(&server_id, cleaned.clone());
                    emit_server(
                        &server_id,
                        ServerPayloadType::Log { line: cleaned },
                    )
                    .await
                    .ok();
                }
            }
            Err(_) => break,
        }
    }
}

/// Matches the native abort of the known JNA (< 5.13.0) macOS bug (JNA issue
/// #1452): a failed library load overflows JNA's fixed error buffer and the
/// JVM dies with SIGABRT before any Java-level exception can be reported.
fn is_jna_macos_assertion(line: &str) -> bool {
    line.contains("Assertion failed:")
        && line.contains("snprintf() output has been truncated")
        && line.contains("dispatch.c")
}

/// Detects a server log4j line by its leading `[HH:MM:SS]` timestamp, covering
/// both console (`[HH:MM:SS INFO]:`) and file (`[HH:MM:SS] [Thread/INFO]:`)
/// formats. Used to suppress the process-pipe echo of server logs so they are
/// not duplicated by `tail_server_log_file`.
fn is_timestamped_log_line(line: &str) -> bool {
    let b = line.as_bytes();
    b.first() == Some(&b'[')
        && b.get(1).is_some_and(|c| c.is_ascii_digit())
        && b.get(2).is_some_and(|c| c.is_ascii_digit())
        && b.get(3) == Some(&b':')
        && b.get(4).is_some_and(|c| c.is_ascii_digit())
        && b.get(5).is_some_and(|c| c.is_ascii_digit())
        && b.get(6) == Some(&b':')
        && b.get(7).is_some_and(|c| c.is_ascii_digit())
        && b.get(8).is_some_and(|c| c.is_ascii_digit())
}

/// How many lines at the end of a server's output are inspected when
/// classifying why it exited.
const EXIT_ANALYSIS_TAIL_LINES: usize = 50;

/// Classifies why a server exited on its own by scanning the tail of its
/// console output, newest lines first. Returns `None` when nothing matches:
/// no guess is better than a wrong one, and unmatched exits simply behave as
/// before.
pub(super) fn analyze_exit_reason(lines: &[String]) -> Option<ExitReason> {
    lines
        .iter()
        .rev()
        .take(EXIT_ANALYSIS_TAIL_LINES)
        .find_map(|line| is_eula_refusal(line).then_some(ExitReason::Eula))
}

/// Matches the vanilla server's refusal to boot before the EULA has been
/// accepted; the process then writes `eula.txt` and exits immediately.
fn is_eula_refusal(line: &str) -> bool {
    line.contains("need to agree to the EULA")
}

const JNA_CRASH_HINT_LINES: [&str; 3] = [
    "[Axolotl] This crash matches a known JNA bug on macOS (java-native-access#1452):",
    "[Axolotl] mods bundling JNA below 5.13.0 abort when a native library fails to load.",
    "[Axolotl] Update or remove the affected mod, or ask the modpack author to bump JNA to 5.13.0+.",
];

/// Removes ANSI escape sequences (SGR colors, cursor control, OSC titles) that
/// servers emit when they assume an interactive terminal is attached.
fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((_, character)) = chars.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        match chars.peek().map(|&(_, c)| c) {
            // CSI sequence: parameter bytes, then a final byte in @..~
            Some('[') => {
                chars.next();
                for (_, c) in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            // OSC sequence: terminated by BEL or ST (ESC \)
            Some(']') => {
                chars.next();
                let mut saw_escape = false;
                for (_, c) in chars.by_ref() {
                    if c == '\u{7}' || (saw_escape && c == '\\') {
                        break;
                    }
                    saw_escape = c == '\u{1b}';
                }
            }
            // Stray escape byte without a recognized sequence
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_escape_sequences_from_server_output() {
        let line = "[16:02:30 INFO]: \u{1b}[38;2;255;170;0m/mspt: \u{1b}[38;2;255;255;255mView server tick times\u{1b}[0m";
        assert_eq!(
            strip_ansi(line),
            "[16:02:30 INFO]: /mspt: View server tick times"
        );

        assert_eq!(strip_ansi("\u{1b}]0;Server console\u{7}ready"), "ready");
        assert_eq!(strip_ansi("\u{1b}]0;Server console\u{1b}\\done"), "done");
        assert_eq!(strip_ansi("plain text stays"), "plain text stays");
        assert_eq!(strip_ansi("h\u{e9}llo \u{1b}[31mred"), "h\u{e9}llo red");
    }

    #[test]
    fn splits_pty_output_without_losing_partial_or_raw_bytes() {
        let mut pending = Vec::new();
        assert_eq!(
            take_complete_pty_lines(&mut pending, b"first\r\nsecond"),
            vec![b"first\r\n".to_vec()]
        );
        assert_eq!(pending, b"second");
        assert_eq!(
            take_complete_pty_lines(&mut pending, b"\nthird\n"),
            vec![b"second\n".to_vec(), b"third\n".to_vec()]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn bounds_unterminated_pty_output() {
        let mut pending = vec![b'x'; MAX_PTY_LINE_BYTES];
        assert!(take_complete_pty_lines(&mut pending, b"x").is_empty());
        assert!(pending.is_empty());

        let oversized = vec![b'x'; MAX_PTY_LINE_BYTES + 1];
        assert!(take_complete_pty_lines(&mut pending, &oversized).is_empty());
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn bounded_server_log_reader_consumes_oversized_line() {
        let input =
            format!("{}\nnext\n", "x".repeat(MAX_SERVER_LOG_LINE_BYTES * 2));
        let mut reader = BufReader::new(std::io::Cursor::new(input));

        let first = read_bounded_server_log_line(&mut reader)
            .await
            .unwrap()
            .unwrap();
        let second = read_bounded_server_log_line(&mut reader)
            .await
            .unwrap()
            .unwrap();

        assert!(first.contains("truncated by Axolotl"));
        assert!(first.len() <= MAX_SERVER_LOG_LINE_BYTES);
        assert_eq!(second, "next\n");
    }

    #[test]
    fn detects_timestamped_log_lines() {
        // Console format (no thread) — duplicated by Paper's stdout echo.
        assert!(is_timestamped_log_line(
            "[11:13:49 INFO]: [bootstrap] Running Java 25"
        ));
        // File format (with thread) — authoritative source from latest.log.
        assert!(is_timestamped_log_line(
            "[11:13:49] [ServerMain/INFO]: [bootstrap] Running"
        ));
        assert!(is_timestamped_log_line(
            "[11:13:49] [Server thread/INFO]: Stopped IO worker!"
        ));
        // Non-logged process output must NOT be suppressed.
        assert!(!is_timestamped_log_line("Downloading mojang_26.2.jar"));
        assert!(!is_timestamped_log_line("Applying patches"));
        assert!(!is_timestamped_log_line(
            "Starting org.bukkit.craftbukkit.Main"
        ));
        assert!(!is_timestamped_log_line(
            "WARNING: A terminally deprecated method in sun.misc.Unsafe"
        ));
        assert!(!is_timestamped_log_line(
            "2026-08-29T03:13:49.279070900Z ServerMain WARN Advanced terminal features",
        ));
    }

    #[test]
    fn detects_jna_macos_assertion() {
        let line = "Assertion failed: (count <= len && \"snprintf() output has been truncated\"), function LOAD_ERROR, file dispatch.c, line 74.";
        assert!(is_jna_macos_assertion(line));
        assert!(!is_jna_macos_assertion(
            "Assertion failed: something else, file other.c, line 1."
        ));
        assert!(!is_jna_macos_assertion("regular log output"));
    }

    #[test]
    fn classifies_eula_refusal_from_output_tail() {
        let eula_line = "[15:26:09] [main/INFO]: You need to agree to the EULA in order to run the server. Go to eula.txt for more info.".to_string();
        let mut lines = vec![
            "[15:26:09] [main/INFO]: Starting minecraft server version 26.2"
                .to_string(),
            eula_line.clone(),
        ];
        assert_eq!(analyze_exit_reason(&lines), Some(ExitReason::Eula));

        // Detected even when buried under later shutdown chatter.
        lines.push(
            "[16:44:23] [Server thread/INFO]: Stopped IO worker!".to_string(),
        );
        assert_eq!(analyze_exit_reason(&lines), Some(ExitReason::Eula));

        // A normal shutdown matches nothing and stays unclassified.
        let normal = vec![
            "[16:44:18] [Server thread/INFO]: Stopping the server".to_string(),
            "[16:44:23] [Server thread/INFO]: Stopped IO worker!".to_string(),
        ];
        assert_eq!(analyze_exit_reason(&normal), None);
        assert_eq!(analyze_exit_reason(&[]), None);

        // Only the tail is inspected; ancient history does not classify a
        // much-later exit.
        let mut old = vec![eula_line];
        old.resize(EXIT_ANALYSIS_TAIL_LINES + 10, "noise".to_string());
        assert_eq!(analyze_exit_reason(&old), None);
    }
}
