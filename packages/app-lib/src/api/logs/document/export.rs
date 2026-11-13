use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path};

use cap_fs_ext::{FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, Metadata, OpenOptions};
use regex::bytes::Regex;

const CHUNK: usize = 64 * 1024;

trait FinishWrite: Write {
    fn finish(&mut self) -> io::Result<()>;
}

struct Sink<'a>(&'a mut dyn Write);
impl Write for Sink<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl FinishWrite for Sink<'_> {
    fn finish(&mut self) -> io::Result<()> {
        self.flush()
    }
}

/// One ordered literal replacement. Retain the suffix that may begin a match
/// in the next chunk; completed matches may consume beyond that suffix.
struct ReplaceWriter<'a> {
    inner: Box<dyn FinishWrite + 'a>,
    pattern: Regex,
    overlap: usize,
    replacement: &'a [u8],
    pending: Vec<u8>,
}
impl ReplaceWriter<'_> {
    fn emit(&mut self, last: bool) -> io::Result<()> {
        let limit = if last {
            self.pending.len()
        } else {
            self.pending.len().saturating_sub(self.overlap)
        };
        let mut consumed = 0;
        for found in self.pattern.find_iter(&self.pending) {
            if found.start() >= limit {
                break;
            }
            self.inner
                .write_all(&self.pending[consumed..found.start()])?;
            self.inner.write_all(self.replacement)?;
            consumed = found.end();
        }
        let end = limit.max(consumed);
        self.inner.write_all(&self.pending[consumed..end])?;
        self.pending.drain(..end);
        Ok(())
    }
}
impl Write for ReplaceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Also bound each downstream stage if a replacement expands its input.
        for chunk in bytes.chunks(CHUNK) {
            self.pending.extend_from_slice(chunk);
            self.emit(false)?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl FinishWrite for ReplaceWriter<'_> {
    fn finish(&mut self) -> io::Result<()> {
        self.emit(true)?;
        self.inner.finish()
    }
}

fn censor_stream(
    mut reader: impl Read,
    writer: &mut dyn Write,
    rules: &[(String, &'static str)],
) -> io::Result<()> {
    let mut output: Box<dyn FinishWrite + '_> = Box::new(Sink(writer));
    // Preserve CensoredString's sequential replacement order, including matches
    // formed by an earlier replacement. No complete line or document is kept.
    for (pattern, replacement) in rules.iter().rev() {
        if pattern.is_empty() {
            continue;
        }
        output = Box::new(ReplaceWriter {
            inner: output,
            pattern: Regex::new(&regex::escape(pattern))
                .map_err(io::Error::other)?,
            overlap: pattern.len() - 1,
            replacement: replacement.as_bytes(),
            pending: Vec::with_capacity(CHUNK + pattern.len()),
        });
    }
    let mut decoder = encoding_rs::UTF_8.new_decoder_without_bom_handling();
    let mut input = [0; CHUNK];
    let mut decoded = [0; CHUNK];
    loop {
        let count = reader.read(&mut input)?;
        let last = count == 0;
        let mut position = 0;
        loop {
            let (result, read, written, _) = decoder.decode_to_utf8(
                &input[position..count],
                &mut decoded,
                last,
            );
            output.write_all(&decoded[..written])?;
            position += read;
            if result == encoding_rs::CoderResult::InputEmpty {
                break;
            }
        }
        if last {
            return output.finish();
        }
    }
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

struct PendingExport<'a> {
    directory: &'a Dir,
    name: String,
}
impl Drop for PendingExport<'_> {
    fn drop(&mut self) {
        let _ = self.directory.remove_file(&self.name);
    }
}

pub(super) fn export_file(
    directories: &[std::path::PathBuf],
    filename: &str,
    destination: &Path,
    rules: &[(String, &'static str)],
) -> io::Result<()> {
    for directory in directories {
        let source_dir = match Dir::open_ambient_dir(
            directory,
            cap_std::ambient_authority(),
        ) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let source = match source_dir.open_with(filename, &options) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if !source.metadata()?.is_file() {
            return Err(io::Error::other("Log source is not a regular file"));
        }
        return export_open_file(
            &source_dir,
            filename,
            source,
            destination,
            rules,
            || {},
        );
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Log file not found",
    ))
}

fn export_open_file(
    source_dir: &Dir,
    filename: &str,
    source: cap_std::fs::File,
    destination: &Path,
    rules: &[(String, &'static str)],
    before_publish: impl FnOnce(),
) -> io::Result<()> {
    let name = destination
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid export destination"))?;
    if !destination.is_absolute()
        || !matches!(
            Path::new(name).components().next(),
            Some(Component::Normal(_))
        )
        || name.to_string_lossy().contains([':', '\\'])
    {
        return Err(io::Error::other("Invalid export destination"));
    }
    // All subsequent operations use this open directory, even if a parent is
    // renamed or replaced. The final rename replaces an entry, never follows it.
    let output_dir = Dir::open_ambient_dir(
        destination.parent().unwrap(),
        cap_std::ambient_authority(),
    )?;
    let same_name = if cfg!(windows) {
        name.to_string_lossy().eq_ignore_ascii_case(filename)
    } else {
        name == filename
    };
    if same_name
        && same_file(&source_dir.dir_metadata()?, &output_dir.dir_metadata()?)
    {
        return Err(io::Error::other("Cannot overwrite the original log"));
    }
    let source_metadata = source.metadata()?;
    // Reject an existing hardlink as well as the original pathname. A link
    // substituted later is safe: rename replaces that link, not its target.
    match output_dir.open_with(
        name,
        OpenOptions::new().read(true).follow(FollowSymlinks::No),
    ) {
        Ok(existing) if same_file(&source_metadata, &existing.metadata()?) => {
            return Err(io::Error::other("Cannot overwrite the original log"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let pending = PendingExport {
        directory: &output_dir,
        name: format!(".log-export-{}.tmp", uuid::Uuid::new_v4()),
    };
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let file = output_dir.open_with(&pending.name, &options)?;
    let temporary_metadata = file.metadata()?;
    let mut writer = BufWriter::new(file);
    // Freeze the source length so an actively growing file cannot extend export indefinitely.
    let input = BufReader::new(source.take(source_metadata.len()));
    if filename.ends_with(".gz") {
        censor_stream(
            flate2::read::MultiGzDecoder::new(input),
            &mut writer,
            rules,
        )?;
    } else {
        censor_stream(input, &mut writer, rules)?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    before_publish();
    if !same_file(
        &temporary_metadata,
        &output_dir.open(&pending.name)?.metadata()?,
    ) {
        return Err(io::Error::other("Export temporary file changed"));
    }
    output_dir.rename(&pending.name, &output_dir, name)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BoundedReader {
        remaining: usize,
    }
    impl Read for BoundedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            assert!(buffer.len() <= CHUNK);
            let count = self.remaining.min(buffer.len());
            buffer[..count].fill(b'x');
            self.remaining -= count;
            Ok(count)
        }
    }
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            assert!(bytes.len() <= CHUNK + 1024);
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn huge_unbroken_stream_uses_bounded_reads_and_writes() {
        let length = 128 * 1024 * 1024;
        let mut counter = Counter(0);
        censor_stream(
            BoundedReader { remaining: length },
            &mut counter,
            &[("secret".into(), "REDACTED")],
        )
        .unwrap();
        assert_eq!(counter.0, length);
    }

    #[test]
    fn stream_matches_whole_text_censorship_at_every_boundary() {
        let rules = vec![
            ("TOKEN".into(), "{TOKEN}"),
            ("{TOKEN}".into(), "safe"),
            ("用户名".into(), "name"),
            ("aaaa".into(), "a"),
        ];
        for offset in 0..24 {
            let text = format!(
                "{}TOKEN用户名{}",
                "x".repeat(CHUNK - offset),
                "a".repeat(CHUNK + 12)
            );
            let mut expected = text.clone();
            for (pattern, replacement) in &rules {
                expected = expected.replace(pattern, replacement);
            }
            let mut actual = Vec::new();
            censor_stream(text.as_bytes(), &mut actual, &rules).unwrap();
            assert_eq!(actual, expected.as_bytes());
        }
        let mut bytes = vec![b'x'; CHUNK - 1];
        bytes.extend_from_slice(&[0xf0, 0x9f, 0x92, 0x96, 0xff, b'!']);
        let mut output = Vec::new();
        censor_stream(bytes.as_slice(), &mut output, &[]).unwrap();
        assert_eq!(output, String::from_utf8_lossy(&bytes).as_bytes());
    }

    #[test]
    fn account_rules_match_existing_censorship_across_chunk_boundaries() {
        let credentials: crate::prelude::Credentials = serde_json::from_value(serde_json::json!({
            "profile": { "id": "12345678-1234-5678-1234-567812345678", "name": "Player_Name", "skins": [], "capes": [] },
            "access_token": "token-with-secret-bytes", "refresh_token": "unused", "expires": "2030-01-01T00:00:00Z", "active": true,
        })).unwrap();
        let rules = super::super::CensoredString::replacements(
            std::slice::from_ref(&credentials),
        );
        let username = whoami::username();
        let body = format!(
            "/{} /{username}/ \\{username}\\ Player_Name token-with-secret-bytes 12345678123456781234567812345678 12345678-1234-5678-1234-567812345678",
            "x".repeat(CHUNK - 5)
        );
        let expected =
            super::super::CensoredString::censor(body.clone(), &[credentials]);
        let mut output = Vec::new();
        censor_stream(body.as_bytes(), &mut output, &rules).unwrap();
        assert_eq!(output, expected.as_str().as_bytes());
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("token-with-secret-bytes")
        );
    }

    #[test]
    fn export_is_complete_for_plain_gzip_and_concatenated_gzip() {
        let root = tempfile::tempdir().unwrap();
        let text = format!(
            "{}TOKEN\n{}",
            "x".repeat(CHUNK - 2),
            "same\n".repeat(600_000)
        );
        let source = root.path().join("source.log");
        std::fs::write(&source, &text).unwrap();
        let mut gzip = flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        );
        gzip.write_all(text.as_bytes()).unwrap();
        let compressed = gzip.finish().unwrap();
        std::fs::write(
            root.path().join("source.log.gz"),
            [&compressed[..], &compressed[..]].concat(),
        )
        .unwrap();
        for filename in ["source.log", "source.log.gz"] {
            let target = root.path().join("saved.log");
            export_file(
                &[root.path().to_owned()],
                filename,
                &target,
                &[("TOKEN".into(), "SAFE")],
            )
            .unwrap();
            let expected = text
                .replace("TOKEN", "SAFE")
                .repeat(if filename.ends_with(".gz") { 2 } else { 1 });
            assert_eq!(std::fs::read_to_string(&target).unwrap(), expected);
            assert_eq!(std::fs::read_to_string(&source).unwrap(), text);
        }
    }

    #[test]
    fn source_and_hardlink_cannot_be_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.log");
        let alias = root.path().join("alias.log");
        std::fs::write(&source, "TOKEN").unwrap();
        std::fs::hard_link(&source, &alias).unwrap();
        for target in [&source, &alias] {
            assert!(
                export_file(
                    &[root.path().to_owned()],
                    "source.log",
                    target,
                    &[("TOKEN".into(), "SAFE")]
                )
                .is_err()
            );
            assert_eq!(std::fs::read_to_string(&source).unwrap(), "TOKEN");
        }
    }

    #[test]
    fn link_substituted_after_validation_never_truncates_the_source() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.log");
        let destination = root.path().join("saved.log");
        std::fs::write(&source, "TOKEN").unwrap();
        std::fs::write(&destination, "old export").unwrap();
        let directory =
            Dir::open_ambient_dir(root.path(), cap_std::ambient_authority())
                .unwrap();
        let opened = directory.open("source.log").unwrap();
        let result = export_open_file(
            &directory,
            "source.log",
            opened,
            &destination,
            &[("TOKEN".into(), "SAFE")],
            || {
                std::fs::remove_file(&destination).unwrap();
                std::fs::hard_link(&source, &destination).unwrap();
            },
        );
        result.unwrap();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "TOKEN");
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "SAFE");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn failed_gzip_does_not_replace_destination_and_cleans_temporary_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("broken.log.gz"), b"not gzip").unwrap();
        let destination = root.path().join("saved.log");
        std::fs::write(&destination, "keep").unwrap();
        assert!(
            export_file(
                &[root.path().to_owned()],
                "broken.log.gz",
                &destination,
                &[]
            )
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(destination).unwrap(), "keep");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn renamed_parent_handle_cannot_redirect_export_over_source() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("source");
        let output_dir = root.path().join("out");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&output_dir).unwrap();
        std::fs::write(source_dir.join("latest.log"), "TOKEN").unwrap();
        let directory =
            Dir::open_ambient_dir(&source_dir, cap_std::ambient_authority())
                .unwrap();
        let source = directory.open("latest.log").unwrap();
        export_open_file(
            &directory,
            "latest.log",
            source,
            &output_dir.join("latest.log"),
            &[("TOKEN".into(), "SAFE")],
            || {
                std::fs::rename(&output_dir, root.path().join("original-out"))
                    .unwrap();
                std::os::unix::fs::symlink(&source_dir, &output_dir).unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(source_dir.join("latest.log")).unwrap(),
            "TOKEN"
        );
        assert_eq!(
            std::fs::read_to_string(
                root.path().join("original-out/latest.log")
            )
            .unwrap(),
            "SAFE"
        );
    }
}
