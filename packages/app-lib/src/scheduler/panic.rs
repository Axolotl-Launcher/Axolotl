//! Process-wide panic hook. Every panic, on any thread or task, is appended to
//! `panic.log` in the launcher log folder before the previous hook runs. The
//! record bypasses the tracing writer: a panic raised while that writer holds
//! its lock would otherwise deadlock inside the hook.

use std::io::Write;
use std::path::{Path, PathBuf};

pub const PANIC_LOG_FILE: &str = "panic.log";
/// The launcher's log pruning only removes `session_*` files, so the panic log
/// bounds itself: it never grows past this size and rotates into
/// `panic.old.log`.
pub const PANIC_LOG_MAX_BYTES: u64 = 1024 * 1024;
const PANIC_LOG_PREVIOUS_FILE: &str = "panic.old.log";
const TRUNCATED: &str = " [truncated]\n";
/// Serializes the size check, rotation and append of threads that panic at
/// the same time.
static PANIC_LOG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Installs the hook; call it once, as early in `main` as the log folder is known.
pub fn install_panic_hook(app_identifier: &str) {
    let log_dir =
        crate::state::DirectoryInfo::launcher_logs_dir_path(app_identifier);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = &log_dir {
            let thread = std::thread::current();
            let location = info
                .location()
                .map(|location| {
                    format!("{}:{}", location.file(), location.line())
                })
                .unwrap_or_default();
            let message = super::supervisor::panic_message(info.payload());
            let _ = append_panic_record(
                dir,
                PANIC_LOG_MAX_BYTES,
                thread.name().unwrap_or("unnamed"),
                &location,
                &message,
            );
        }
        previous(info);
    }));
}

/// Appends one line: RFC 3339 time, thread, location and message. Neither
/// `panic.log` nor `panic.old.log` grows past `max_bytes`: an oversized record
/// is cut to fit, and the log rotates before a write would overflow it.
pub(crate) fn append_panic_record(
    dir: &Path,
    max_bytes: u64,
    thread: &str,
    location: &str,
    message: &str,
) -> std::io::Result<PathBuf> {
    let _serial = PANIC_LOG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::fs::create_dir_all(dir)?;
    let path = dir.join(PANIC_LOG_FILE);
    let mut line = format!(
        "{} panic thread={thread} at={location}: {}\n",
        chrono::Local::now().to_rfc3339(),
        message.replace('\n', "\\n"),
    );
    let limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    if line.len() > limit {
        let mut cut = limit.saturating_sub(TRUNCATED.len());
        while !line.is_char_boundary(cut) {
            cut -= 1;
        }
        line.truncate(cut);
        line.push_str(TRUNCATED);
    }
    let current = std::fs::metadata(&path).map_or(0, |meta| meta.len());
    if current > 0 && current + line.len() as u64 > max_bytes {
        std::fs::rename(&path, dir.join(PANIC_LOG_PREVIOUS_FILE))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?
        .write_all(line.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_records_are_appended_one_per_line() {
        let dir = tempfile::tempdir().unwrap();
        append_panic_record(dir.path(), 4096, "main", "src/a.rs:1", "first")
            .unwrap();
        let path = append_panic_record(
            dir.path(),
            4096,
            "worker",
            "src/b.rs:2",
            "second\nline",
        )
        .unwrap();
        let log = std::fs::read_to_string(path).unwrap();
        let lines: Vec<_> = log.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("panic thread=main at=src/a.rs:1: first"));
        assert!(
            lines[1]
                .ends_with("panic thread=worker at=src/b.rs:2: second\\nline")
        );
    }

    #[test]
    fn a_full_log_is_rotated_once() {
        // Two records fit under the cap, a third does not.
        const CAP: u64 = 180;
        let dir = tempfile::tempdir().unwrap();
        for message in ["first", "second", "third", "fourth", "fifth"] {
            append_panic_record(dir.path(), CAP, "main", "src/a.rs:1", message)
                .unwrap();
        }
        let current =
            std::fs::read_to_string(dir.path().join(PANIC_LOG_FILE)).unwrap();
        let previous =
            std::fs::read_to_string(dir.path().join(PANIC_LOG_PREVIOUS_FILE))
                .unwrap();
        assert_eq!(current.lines().count(), 1);
        assert!(current.ends_with(": fifth\n"));
        assert!(previous.contains(": third") && previous.contains(": fourth"));
        assert!(!previous.contains(": first"), "only one old log is kept");
        assert!(current.len() as u64 <= CAP && previous.len() as u64 <= CAP);
    }

    #[test]
    fn concurrent_records_rotate_without_failing() {
        // Each record is about 60 bytes, so the log rotates every few writes.
        const CAP: u64 = 200;
        let dir = tempfile::tempdir().unwrap();
        let failures: usize = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|worker| {
                    let dir = dir.path();
                    scope.spawn(move || {
                        (0..40)
                            .filter(|record| {
                                append_panic_record(
                                    dir,
                                    CAP,
                                    &format!("w{worker}"),
                                    "src/a.rs:1",
                                    &format!("r{record}"),
                                )
                                .is_err()
                            })
                            .count()
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .sum()
        });
        assert_eq!(failures, 0, "no record may be dropped by a rotation race");
        for file in [PANIC_LOG_FILE, PANIC_LOG_PREVIOUS_FILE] {
            let log = std::fs::read_to_string(dir.path().join(file)).unwrap();
            assert!(log.len() as u64 <= CAP, "{file} outgrew the cap");
            assert!(
                log.lines().all(|line| line.contains(" panic thread=w")),
                "{file} holds a torn record: {log:?}"
            );
        }
    }

    #[test]
    fn an_oversized_record_is_cut_to_the_cap() {
        const CAP: u64 = 512;
        let dir = tempfile::tempdir().unwrap();
        let path = append_panic_record(
            dir.path(),
            CAP,
            "main",
            "src/a.rs:1",
            &"x".repeat(10_000),
        )
        .unwrap();
        let log = std::fs::read_to_string(path).unwrap();
        assert!(log.len() as u64 <= CAP);
        assert!(log.ends_with(TRUNCATED));
    }
}
