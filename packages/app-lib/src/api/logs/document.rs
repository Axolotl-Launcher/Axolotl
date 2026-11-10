//! Bounded, noncompacting log reads for external inspection clients.
use super::{
    CensoredString, LogType, log_directories_for_type, resolve_instance_path,
};
use crate::{State, prelude::Credentials};
use std::{io::Read, path::Path};

const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

pub struct LogDocument {
    pub output: CensoredString,
    pub source_truncated: bool,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.ends_with([' ', '.'])
        && !name.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                )
        })
        && [".log", ".txt", ".log.gz", ".txt.gz"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

fn read_file(
    directory: &Path,
    filename: &str,
    maximum_bytes: u64,
) -> std::io::Result<Option<(String, bool)>> {
    let path = directory.join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let root = directory.canonicalize()?;
    let resolved = path.canonicalize()?;
    if resolved.parent() != Some(root.as_path()) || !resolved.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Log file resolves outside its log directory",
        ));
    }
    let file = std::fs::File::open(resolved)?;
    let reader: Box<dyn Read> = if filename.ends_with(".gz") {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    reader
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() as u64 > maximum_bytes;
    if truncated {
        bytes.truncate(maximum_bytes as usize);
    }
    // A byte limit may split an access token before the censor sees it. Drop
    // the incomplete final line rather than returning an unredacted prefix.
    if truncated && !bytes.ends_with(b"\n") {
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |end| end + 1);
        bytes.truncate(complete);
    }
    Ok(Some((
        String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    )))
}

/// Read supported instance logs without display compaction, notifications or
/// arbitrary paths. The whole bounded document is censored before pagination.
pub async fn read_log_document(
    instance_id: &str,
    log_type: LogType,
    filename: &str,
) -> crate::Result<LogDocument> {
    load_log_document(
        instance_id,
        log_type,
        filename,
        MAX_SOURCE_BYTES as u64,
        None,
    )
    .await
}

async fn load_log_document(
    instance_id: &str,
    log_type: LogType,
    filename: &str,
    maximum_bytes: u64,
    destination: Option<std::path::PathBuf>,
) -> crate::Result<LogDocument> {
    if !valid_name(filename) || !super::is_log_file_for_type(log_type, filename)
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid log filename".into(),
        )
        .into());
    }
    let state = State::get().await?;
    let (instance_path, override_dir) =
        resolve_instance_path(instance_id, &state).await?;
    let game_dir = state
        .directories
        .resolve_game_dir(&instance_path, override_dir.as_deref());
    let directories = log_directories_for_type(&state, &game_dir, log_type);
    let filename = filename.to_owned();
    let (text, source_truncated) = tokio::task::spawn_blocking(move || {
        for directory in directories {
            if let Some(output) = &destination {
                let source = directory.join(&filename);
                validate_export_destination(&source, output)?;
            }
            if let Some(document) =
                read_file(&directory, &filename, maximum_bytes)?
            {
                return Ok(document);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Log file not found",
        ))
    })
    .await
    .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))??;
    let credentials = Credentials::get_all(&state.data)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    Ok(LogDocument {
        output: CensoredString::censor(text, &credentials),
        source_truncated,
    })
}

fn validate_export_destination(
    source: &Path,
    output: &Path,
) -> std::io::Result<()> {
    if source.exists()
        && output.exists()
        && source.canonicalize()? == output.canonicalize()?
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Cannot overwrite the original log",
        ));
    }
    Ok(())
}

/// Save a complete, noncompacted and censored log without sending it over IPC.
pub async fn export_log_file(
    instance_id: &str,
    log_type: LogType,
    filename: &str,
    output_path: std::path::PathBuf,
) -> crate::Result<()> {
    let document = load_log_document(
        instance_id,
        log_type,
        filename,
        u64::MAX,
        Some(output_path.clone()),
    )
    .await?;
    tokio::fs::write(output_path, document.output.as_str()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_are_bounded_and_support_compressed_logs() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let content = "a\n".repeat(MAX_SOURCE_BYTES / 2 + 100);
        std::fs::write(root.path().join("latest.log"), &content).unwrap();
        let (text, truncated) =
            read_file(root.path(), "latest.log", MAX_SOURCE_BYTES as u64)
                .unwrap()
                .unwrap();
        assert_eq!(text.len(), MAX_SOURCE_BYTES);
        assert!(truncated);
        let mut gzip = flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        );
        gzip.write_all(content.as_bytes()).unwrap();
        std::fs::write(root.path().join("old.log.gz"), gzip.finish().unwrap())
            .unwrap();
        let (text, truncated) =
            read_file(root.path(), "old.log.gz", MAX_SOURCE_BYTES as u64)
                .unwrap()
                .unwrap();
        assert_eq!(text.len(), MAX_SOURCE_BYTES);
        assert!(truncated);
        assert!(
            read_file(root.path(), "absent.log", MAX_SOURCE_BYTES as u64)
                .unwrap()
                .is_none()
        );
        let (complete, truncated) =
            read_file(root.path(), "old.log.gz", u64::MAX)
                .unwrap()
                .unwrap();
        assert!(!truncated);
        assert_eq!(complete, content);
    }
    #[test]
    fn export_keeps_long_lines_and_repetitions_and_rejects_source_destination()
    {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("latest.log");
        let text = format!(
            "{}\n{}",
            "x".repeat(300_000),
            "repeated\n".repeat(300_000)
        );
        std::fs::write(&path, &text).unwrap();
        let (complete, truncated) =
            read_file(root.path(), "latest.log", u64::MAX)
                .unwrap()
                .unwrap();
        assert!(!truncated);
        assert_eq!(complete, text);
        assert!(validate_export_destination(&path, &path).is_err());
        assert!(
            validate_export_destination(&path, &root.path().join("saved.log"))
                .is_ok()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let username = whoami::username();
        let censored =
            CensoredString::censor(format!("prefix/{username}/suffix"), &[]);
        assert_eq!(censored.as_str(), "prefix/{COMPUTER_USERNAME}/suffix");
    }

    #[test]
    fn names_reject_paths_and_alternate_streams() {
        for name in [
            "../a.log",
            "..\\a.log",
            "a.log:secret",
            "C:\\a.log",
            "a.json",
            ".log",
        ] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("latest.log"));
    }
    #[test]
    fn canonical_paths_must_stay_inside_the_log_directory() {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        std::fs::write(root.path().join("outside.log"), "secret").unwrap();
        assert_eq!(
            read_file(&logs, "../outside.log", MAX_SOURCE_BYTES as u64)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    #[test]
    fn truncation_does_not_expose_a_partial_credential_line() {
        let root = tempfile::tempdir().unwrap();
        let content = format!("safe line\n{}", "s".repeat(MAX_SOURCE_BYTES));
        std::fs::write(root.path().join("latest.log"), content).unwrap();
        let (text, truncated) =
            read_file(root.path(), "latest.log", MAX_SOURCE_BYTES as u64)
                .unwrap()
                .unwrap();
        assert_eq!(text, "safe line\n");
        assert!(truncated);
    }
}
