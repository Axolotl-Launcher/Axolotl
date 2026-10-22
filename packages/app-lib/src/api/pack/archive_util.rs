//! Shared helpers for extracting content from local modpack archives.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::detect::decode_zip_entry_name;
use crate::util::io;
use tokio_util::sync::CancellationToken;

pub(crate) const EXTRACTION_SIZE_LIMIT: u64 = 8 * 1024 * 1024 * 1024;

fn archive_error(error: zip::result::ZipError) -> crate::Error {
    crate::ErrorKind::InputError(format!("Modpack archive is invalid: {error}"))
        .into()
}

/// Extracts every file under `prefix` in the archive into `target_dir`,
/// preserving the directory structure below the prefix. Returns the number of
/// files written.
pub(crate) async fn extract_archive_subdir(
    archive_path: PathBuf,
    prefix: String,
    target_dir: PathBuf,
) -> crate::Result<u32> {
    tokio::task::spawn_blocking(move || {
        extract_archive_subdir_sync(&archive_path, &prefix, &target_dir, None)
    })
    .await?
}

pub(crate) async fn materialize_archive_subdir_for_instance(
    instance_id: String,
    cancellation: CancellationToken,
    archive_path: PathBuf,
    prefix: String,
    target_dir: PathBuf,
) -> crate::Result<(u32, StagedArchiveReplacements)> {
    run_cancellable_blocking_instance_write(
        instance_id,
        cancellation,
        move |cancellation| {
            materialize_archive_subdir_sync(
                &archive_path,
                &prefix,
                &target_dir,
                Some(cancellation),
            )
        },
    )
    .await
}

pub(crate) async fn run_blocking_instance_write<T, F>(
    instance_id: String,
    cancellation: CancellationToken,
    operation: F,
) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&CancellationToken) -> crate::Result<T> + Send + 'static,
{
    run_blocking_instance_write_inner(
        instance_id,
        cancellation,
        operation,
        false,
    )
    .await
}

/// Cancellation may stop the lock wait, but a started writer must be joined
/// before rollback. Finalization and rollback use the non-cancellable helper.
pub(crate) async fn run_cancellable_blocking_instance_write<T, F>(
    instance_id: String,
    cancellation: CancellationToken,
    operation: F,
) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&CancellationToken) -> crate::Result<T> + Send + 'static,
{
    run_blocking_instance_write_inner(
        instance_id,
        cancellation,
        operation,
        true,
    )
    .await
}

async fn run_blocking_instance_write_inner<T, F>(
    instance_id: String,
    cancellation: CancellationToken,
    operation: F,
    cancellable: bool,
) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&CancellationToken) -> crate::Result<T> + Send + 'static,
{
    let state = crate::State::get().await?;
    let started = std::time::Instant::now();
    tracing::debug!(%instance_id, cancellable, "Waiting for modpack archive content write lock");
    let instance_lock = if cancellable {
        state
            .instance_locks
            .lock_exclusive_cancellable(&instance_id, &cancellation)
            .await?
    } else {
        state.lock_instance_content_exclusive(&instance_id).await
    };
    tracing::debug!(%instance_id, wait_ms = started.elapsed().as_millis(), "Acquired modpack archive content write lock");
    tokio::task::spawn_blocking(move || {
        let _instance_lock = instance_lock;
        operation(&cancellation)
    })
    .await?
}

fn extract_archive_subdir_sync(
    archive_path: &Path,
    prefix: &str,
    target_dir: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<u32> {
    let (files_written, replacements) = materialize_archive_subdir_sync(
        archive_path,
        prefix,
        target_dir,
        cancellation,
    )?;
    replacements.finalize()?;
    Ok(files_written)
}

fn materialize_archive_subdir_sync(
    archive_path: &Path,
    prefix: &str,
    target_dir: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<(u32, StagedArchiveReplacements)> {
    let mut replacements = HashMap::<PathBuf, Option<PathBuf>>::new();
    let mut replacement_order = Vec::<PathBuf>::new();
    let result = extract_archive_subdir_entries_sync(
        archive_path,
        prefix,
        target_dir,
        cancellation,
        &mut replacements,
        &mut replacement_order,
    );
    match result {
        Ok(files_written) => Ok((
            files_written,
            StagedArchiveReplacements {
                replacements,
                replacement_order,
            },
        )),
        Err(error) => {
            if let Err(rollback_error) =
                rollback_archive_replacements(&replacements, &replacement_order)
            {
                return Err(crate::ErrorKind::OtherError(format!(
                    "{error}; failed to restore archive extraction: {rollback_error}"
                ))
                .into());
            }
            Err(error)
        }
    }
}

fn extract_archive_subdir_entries_sync(
    archive_path: &Path,
    prefix: &str,
    target_dir: &Path,
    cancellation: Option<&CancellationToken>,
    replacements: &mut HashMap<PathBuf, Option<PathBuf>>,
    replacement_order: &mut Vec<PathBuf>,
) -> crate::Result<u32> {
    let file = std::fs::File::open(archive_path)
        .map_err(|error| io::IOError::with_path(error, archive_path))?;
    let mut archive = zip::ZipArchive::new(file).map_err(archive_error)?;
    let mut files_written = 0_u32;
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        check_cancellation(cancellation)?;
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        let entry_name = decode_zip_entry_name(entry.name_raw());
        if entry.is_dir() || !entry_name.starts_with(prefix) {
            continue;
        }
        let relative = &entry_name[prefix.len()..];
        if relative.is_empty() {
            continue;
        }
        total_size = total_size.saturating_add(entry.size());
        if total_size > EXTRACTION_SIZE_LIMIT {
            return Err(crate::ErrorKind::InputError(
                "Modpack archive contents exceed the extraction limit"
                    .to_string(),
            )
            .into());
        }
        let target =
            io::safe_relative_join(target_dir, &relative.replace('\\', "/"))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| io::IOError::with_path(error, parent))?;
        }
        write_archive_entry_atomically(
            &mut entry,
            &target,
            cancellation,
            replacements,
            replacement_order,
        )?;
        files_written = files_written.saturating_add(1);
    }
    Ok(files_written)
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sibling = path.as_os_str().to_os_string();
    sibling.push(suffix);
    PathBuf::from(sibling)
}

fn remove_file_if_exists(path: &Path) -> crate::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::IOError::with_path(error, path).into()),
    }
}

fn write_archive_entry_atomically<R: std::io::Read>(
    reader: &mut R,
    target: &Path,
    cancellation: Option<&CancellationToken>,
    replacements: &mut HashMap<PathBuf, Option<PathBuf>>,
    replacement_order: &mut Vec<PathBuf>,
) -> crate::Result<u64> {
    let written = write_archive_entry_to_staging(reader, target, cancellation)?;
    if let Err(error) = materialize_staged_archive_entry(
        target,
        replacements,
        replacement_order,
    ) {
        let _ = remove_file_if_exists(&sibling_path(target, ".installing"));
        return Err(error);
    }
    Ok(written)
}

/// Writes one archive entry completely to a sibling staging file. This is
/// safe to call from parallel extraction workers because it never mutates the
/// live target.
pub(crate) fn write_archive_entry_to_staging<R: std::io::Read>(
    reader: &mut R,
    target: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<u64> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| io::IOError::with_path(error, parent))?;
    }
    let temporary = sibling_path(target, ".installing");
    remove_file_if_exists(&temporary)?;
    let write_result = (|| {
        let mut output = std::fs::File::create(&temporary)
            .map_err(|error| io::IOError::with_path(error, &temporary))?;
        let written = copy_with_cancellation(
            reader,
            &mut output,
            cancellation,
            &temporary,
        )?;
        Ok::<_, crate::Error>(written)
    })();
    let written = match write_result {
        Ok(written) => written,
        Err(error) => {
            let _ = remove_file_if_exists(&temporary);
            return Err(error);
        }
    };
    Ok(written)
}

fn materialize_staged_archive_entry(
    target: &Path,
    replacements: &mut HashMap<PathBuf, Option<PathBuf>>,
    replacement_order: &mut Vec<PathBuf>,
) -> crate::Result<()> {
    let temporary = sibling_path(target, ".installing");
    let backup = sibling_path(target, ".installing.previous");
    let created = sibling_path(target, ".installing.created");
    let has_marker = backup.exists() || created.exists();
    // On case-insensitive filesystems different ZIP spellings can identify
    // the same file. Resolve the actual on-disk paths only on a marker
    // collision so large archives keep constant-time transaction lookups in
    // the usual case. Preserve the first spelling and its original backup.
    let already_replaced = replacements.contains_key(target)
        || (has_marker
            && std::fs::canonicalize(target).ok().is_some_and(|canonical| {
                replacements.keys().any(|previous| {
                    std::fs::canonicalize(previous)
                        .is_ok_and(|previous| previous == canonical)
                })
            }));
    if already_replaced {
        // Duplicate archive entries for the same normalized path replace the
        // previous entry from this transaction, while the original pre-install
        // backup remains untouched for rollback.
        remove_file_if_exists(target)?;
        if let Err(error) = std::fs::rename(&temporary, target) {
            let _ = remove_file_if_exists(&temporary);
            return Err(io::IOError::with_path(error, target).into());
        }
        return Ok(());
    }

    if has_marker {
        return Err(crate::ErrorKind::FSError(format!(
            "Archive target has an unfinished replacement that must be recovered first: {}",
            target.display()
        ))
        .into());
    }
    let previous = if target.exists() {
        std::fs::rename(target, &backup)
            .map_err(|error| io::IOError::with_path(error, target))?;
        Some(backup)
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&created)
            .map_err(|error| io::IOError::with_path(error, &created))?;
        None
    };
    if let Err(error) = std::fs::rename(&temporary, target) {
        if let Some(previous) = previous.as_deref() {
            let _ = std::fs::rename(previous, target);
        } else {
            let _ = remove_file_if_exists(&created);
        }
        let _ = remove_file_if_exists(&temporary);
        return Err(io::IOError::with_path(error, target).into());
    }
    replacements.insert(target.to_path_buf(), previous);
    replacement_order.push(target.to_path_buf());
    Ok(())
}

/// A published archive batch whose pre-install files are still retained.
/// Callers use the database transaction as their commit point and then either
/// finalize this batch or roll it back.
#[must_use = "published archive replacements must be finalized or rolled back"]
#[derive(Debug)]
pub(crate) struct StagedArchiveReplacements {
    replacements: HashMap<PathBuf, Option<PathBuf>>,
    replacement_order: Vec<PathBuf>,
}

impl StagedArchiveReplacements {
    pub(crate) fn finalize(self) -> crate::Result<()> {
        finalize_archive_replacements(&self.replacements)
    }

    pub(crate) fn rollback(self) -> crate::Result<()> {
        rollback_archive_replacements(
            &self.replacements,
            &self.replacement_order,
        )
    }
}

pub(crate) async fn settle_staged_archive_install(
    instance_id: String,
    cancellation: CancellationToken,
    replacements: StagedArchiveReplacements,
    result: crate::Result<()>,
    description: &str,
) -> crate::Result<()> {
    match result {
        Ok(()) => {
            run_blocking_instance_write(instance_id, cancellation, move |_| {
                replacements.finalize()
            })
            .await
        }
        Err(error) => {
            let rollback_result = run_blocking_instance_write(
                instance_id,
                cancellation,
                move |_| replacements.rollback(),
            )
            .await;
            if let Err(rollback_error) = rollback_result {
                return Err(crate::ErrorKind::OtherError(format!(
                    "{error}; failed to restore {description}: {rollback_error}"
                ))
                .into());
            }
            Err(error)
        }
    }
}

/// Publishes fully-written staging files while retaining the pre-install
/// versions for a later database-backed commit decision. Targets must be
/// unique. Cancellation before or during the short rename phase restores
/// every target already replaced by this call.
pub(crate) fn materialize_staged_archive_entries(
    targets: &[PathBuf],
    cancellation: Option<&CancellationToken>,
) -> crate::Result<StagedArchiveReplacements> {
    let mut replacements = HashMap::<PathBuf, Option<PathBuf>>::new();
    let mut replacement_order = Vec::<PathBuf>::new();
    let result = (|| {
        for target in targets {
            check_cancellation(cancellation)?;
            materialize_staged_archive_entry(
                target,
                &mut replacements,
                &mut replacement_order,
            )?;
        }
        check_cancellation(cancellation)
    })();
    if let Err(error) = result {
        if let Err(rollback_error) =
            rollback_archive_replacements(&replacements, &replacement_order)
        {
            return Err(crate::ErrorKind::OtherError(format!(
                "{error}; failed to restore staged archive entries: {rollback_error}"
            ))
            .into());
        }
        let _ = discard_staged_archive_entries(targets);
        return Err(error);
    }
    Ok(StagedArchiveReplacements {
        replacements,
        replacement_order,
    })
}

/// Publishes a set of fully-written staging files as one rollback-capable
/// batch and immediately finalizes their pre-install backups. Use
/// [`materialize_staged_archive_entries`] when a later database operation is
/// the actual commit point.
#[cfg(test)]
pub(crate) fn commit_staged_archive_entries(
    targets: &[PathBuf],
    cancellation: Option<&CancellationToken>,
) -> crate::Result<()> {
    let replacements =
        materialize_staged_archive_entries(targets, cancellation)?;
    // Backup deletion happens after the transactional commit point. A cleanup
    // failure must not invoke rollback because earlier backups may already be
    // gone; the caller's install-level recovery can handle the reported error.
    replacements.finalize()
}

pub(crate) fn discard_staged_archive_entries(
    targets: &[PathBuf],
) -> crate::Result<()> {
    for target in targets {
        remove_file_if_exists(&sibling_path(target, ".installing"))?;
    }
    Ok(())
}

fn finalize_archive_replacements(
    replacements: &HashMap<PathBuf, Option<PathBuf>>,
) -> crate::Result<()> {
    for (target, previous) in replacements {
        if let Some(previous) = previous {
            remove_file_if_exists(previous)?;
        } else {
            remove_file_if_exists(&sibling_path(
                target,
                ".installing.created",
            ))?;
        }
    }
    Ok(())
}

fn rollback_archive_replacements(
    replacements: &HashMap<PathBuf, Option<PathBuf>>,
    replacement_order: &[PathBuf],
) -> crate::Result<()> {
    for target in replacement_order.iter().rev() {
        remove_file_if_exists(target)?;
        match replacements.get(target).and_then(Option::as_ref) {
            Some(previous) => {
                if !previous.exists() {
                    return Err(crate::ErrorKind::FSError(format!(
                        "Archive replacement backup is missing: {}",
                        previous.display()
                    ))
                    .into());
                }
                std::fs::rename(previous, target)
                    .map_err(|error| io::IOError::with_path(error, target))?;
            }
            None => {
                remove_file_if_exists(&sibling_path(
                    target,
                    ".installing.created",
                ))?;
            }
        }
        let temporary = sibling_path(target, ".installing");
        remove_file_if_exists(&temporary)?;
    }
    Ok(())
}

/// Restores archive replacements left armed when the process stopped during
/// an install. The sidecars are private implementation details and are only
/// consumed while rolling back a known interrupted instance job.
pub(crate) fn recover_interrupted_archive_replacements(
    instance_root: &Path,
) -> crate::Result<()> {
    fn collect_files(
        directory: &Path,
        files: &mut Vec<PathBuf>,
    ) -> crate::Result<()> {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => {
                return Err(io::IOError::with_path(error, directory).into());
            }
        };
        for entry in entries {
            let entry = entry
                .map_err(|error| io::IOError::with_path(error, directory))?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| io::IOError::with_path(error, &path))?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                collect_files(&path, files)?;
            } else if metadata.is_file() {
                files.push(path);
            }
        }
        Ok(())
    }

    fn target_for_sidecar(path: &Path, suffix: &str) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        let target_name = name.strip_suffix(suffix)?;
        Some(path.with_file_name(target_name))
    }

    let mut files = Vec::new();
    collect_files(instance_root, &mut files)?;

    // Restore backups before processing created markers. The two marker kinds
    // are mutually exclusive for one target, but this order is conservative
    // if a damaged install happens to contain both.
    for previous in files.iter().filter(|path| {
        target_for_sidecar(path, ".installing.previous").is_some()
    }) {
        if !previous.exists() {
            continue;
        }
        let target = target_for_sidecar(previous, ".installing.previous")
            .expect("filtered archive backup path");
        remove_file_if_exists(&target)?;
        std::fs::rename(previous, &target)
            .map_err(|error| io::IOError::with_path(error, &target))?;
        remove_file_if_exists(&sibling_path(&target, ".installing.created"))?;
    }
    for marker in files.iter().filter(|path| {
        target_for_sidecar(path, ".installing.created").is_some()
    }) {
        if !marker.exists() {
            continue;
        }
        let target = target_for_sidecar(marker, ".installing.created")
            .expect("filtered archive created marker path");
        remove_file_if_exists(&target)?;
        remove_file_if_exists(marker)?;
    }
    for staging in files.iter().filter(|path| {
        target_for_sidecar(path, ".installing").is_some()
            || target_for_sidecar(path, ".installing.download").is_some()
    }) {
        remove_file_if_exists(staging)?;
    }
    Ok(())
}

pub(crate) fn check_cancellation(
    cancellation: Option<&CancellationToken>,
) -> crate::Result<()> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(crate::ErrorKind::OtherError(
            "Install was canceled".to_string(),
        )
        .into());
    }
    Ok(())
}

pub(crate) fn copy_with_cancellation<R, W>(
    reader: &mut R,
    writer: &mut W,
    cancellation: Option<&CancellationToken>,
    target: &Path,
) -> crate::Result<u64>
where
    R: std::io::Read,
    W: std::io::Write,
{
    let mut buffer = [0_u8; 64 * 1024];
    let mut written = 0_u64;
    loop {
        check_cancellation(cancellation)?;
        let count = reader
            .read(&mut buffer)
            .map_err(|error| io::IOError::with_path(error, target))?;
        if count == 0 {
            return Ok(written);
        }
        check_cancellation(cancellation)?;
        writer
            .write_all(&buffer[..count])
            .map_err(|error| io::IOError::with_path(error, target))?;
        written = written.saturating_add(count as u64);
    }
}

/// Extracts a single archive entry to the given target file path.
pub(crate) async fn extract_archive_entry_to_file(
    archive_path: PathBuf,
    entry_name: String,
    target: PathBuf,
) -> crate::Result<()> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path)
            .map_err(|error| io::IOError::with_path(error, &archive_path))?;
        let mut archive = zip::ZipArchive::new(file).map_err(archive_error)?;
        let index = (0..archive.len())
            .find(|&index| {
                archive
                    .by_index_raw(index)
                    .map(|entry| {
                        decode_zip_entry_name(entry.name_raw()) == entry_name
                    })
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Modpack archive is missing {entry_name}"
                ))
            })?;
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| io::IOError::with_path(error, parent))?;
        }
        let mut output = std::fs::File::create(&target)
            .map_err(|error| io::IOError::with_path(error, &target))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| io::IOError::with_path(error, &target))?;
        Ok(())
    })
    .await?
}

/// Reads a single archive entry into a string, tolerating GB18030-encoded
/// file contents produced by Chinese packaging tools.
pub(crate) async fn read_archive_entry_to_string(
    archive_path: PathBuf,
    entry_name: String,
) -> crate::Result<String> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path)
            .map_err(|error| io::IOError::with_path(error, &archive_path))?;
        let mut archive = zip::ZipArchive::new(file).map_err(archive_error)?;
        let index = super::detect::find_entry_index(&mut archive, &entry_name)?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Modpack archive is missing {entry_name}"
                ))
            })?;
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut contents)?;
        Ok(match String::from_utf8(contents) {
            Ok(value) => value,
            Err(error) => {
                let (decoded, _, _) =
                    encoding_rs::GB18030.decode(error.as_bytes());
                decoded.into_owned()
            }
        })
    })
    .await?
}

/// Allocates a unique scratch directory for extracting nested pack content.
pub(crate) async fn create_import_scratch_dir(
    state: &crate::State,
) -> crate::Result<PathBuf> {
    let dir = state
        .directories
        .caches_dir()
        .join("modpack-import")
        .join(uuid::Uuid::new_v4().to_string());
    io::create_dir_all(&dir).await?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::Write;

    #[test]
    fn unsafe_override_paths_fail_without_writing_outside_target() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let target_dir = root.path().join("server");
        std::fs::create_dir(&target_dir).unwrap();
        let outside = root.path().join("outside.txt");
        std::fs::write(&outside, b"unchanged").unwrap();
        for prefix in ["overrides/", "server/overrides/"] {
            for relative in [
                "../outside.txt",
                r"..\outside.txt",
                "config/../../outside.txt",
                "/outside.txt",
                "C:/outside.txt",
                "C:outside.txt",
                r"\\server\share\outside.txt",
                "config/.. /outside.txt",
                "config/mod.jar:payload",
                "config/NUL",
                "config/secret.",
            ] {
                let mut archive = zip::ZipWriter::new(
                    std::fs::File::create(&archive_path).unwrap(),
                );
                archive
                    .start_file(
                        format!("{prefix}{relative}"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .unwrap();
                archive.write_all(b"malicious").unwrap();
                archive.finish().unwrap();
                assert!(
                    extract_archive_subdir_sync(
                        &archive_path,
                        prefix,
                        &target_dir,
                        None,
                    )
                    .is_err(),
                    "{prefix}{relative}"
                );
                assert_eq!(std::fs::read(&outside).unwrap(), b"unchanged");
                assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn override_paths_with_backslashes_are_normalized() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let target_dir = root.path().join("server");
        let mut archive =
            zip::ZipWriter::new(std::fs::File::create(&archive_path).unwrap());
        archive
            .start_file(
                r"overrides/config\example.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"valid").unwrap();
        archive.finish().unwrap();
        assert_eq!(
            extract_archive_subdir_sync(
                &archive_path,
                "overrides/",
                &target_dir,
                None,
            )
            .unwrap(),
            1
        );
        assert_eq!(
            std::fs::read(target_dir.join("config/example.toml")).unwrap(),
            b"valid"
        );
    }

    struct CancelOnRead {
        cancellation: CancellationToken,
        read: bool,
    }

    impl std::io::Read for CancelOnRead {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.read {
                return Ok(0);
            }
            self.read = true;
            self.cancellation.cancel();
            let count = buffer.len().min(1024);
            buffer[..count].fill(1);
            Ok(count)
        }
    }

    #[test]
    fn blocking_copy_stops_before_writing_after_cancellation() {
        let cancellation = CancellationToken::new();
        let mut reader = CancelOnRead {
            cancellation: cancellation.clone(),
            read: false,
        };
        let mut output = Vec::new();
        let error = copy_with_cancellation(
            &mut reader,
            &mut output,
            Some(&cancellation),
            Path::new("override.bin"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("Install was canceled"));
        assert!(output.is_empty());
    }

    #[test]
    fn canceled_atomic_entry_keeps_existing_target() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("config/example.bin");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"previous").unwrap();
        let cancellation = CancellationToken::new();
        let mut reader = CancelOnRead {
            cancellation: cancellation.clone(),
            read: false,
        };
        let mut replacements = HashMap::new();
        let mut order = Vec::new();

        let error = write_archive_entry_atomically(
            &mut reader,
            &target,
            Some(&cancellation),
            &mut replacements,
            &mut order,
        )
        .unwrap_err();

        assert!(error.to_string().contains("Install was canceled"));
        assert_eq!(std::fs::read(&target).unwrap(), b"previous");
        assert!(!sibling_path(&target, ".installing").exists());
        assert!(replacements.is_empty());
    }

    #[test]
    fn archive_batch_rollback_restores_all_original_files() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("config/first.txt");
        let second = root.path().join("config/second.txt");
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"old-first").unwrap();
        std::fs::write(&second, b"old-second").unwrap();
        let mut replacements = HashMap::new();
        let mut order = Vec::new();

        write_archive_entry_atomically(
            &mut Cursor::new(b"new-first"),
            &first,
            None,
            &mut replacements,
            &mut order,
        )
        .unwrap();
        write_archive_entry_atomically(
            &mut Cursor::new(b"new-second"),
            &second,
            None,
            &mut replacements,
            &mut order,
        )
        .unwrap();
        rollback_archive_replacements(&replacements, &order).unwrap();

        assert_eq!(std::fs::read(&first).unwrap(), b"old-first");
        assert_eq!(std::fs::read(&second).unwrap(), b"old-second");
        assert!(!sibling_path(&first, ".installing.previous").exists());
        assert!(!sibling_path(&second, ".installing.previous").exists());
    }

    #[test]
    fn staged_archive_replacements_can_rollback_after_publish() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("config/existing.txt");
        let created = root.path().join("config/created.txt");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"old-existing").unwrap();
        write_archive_entry_to_staging(
            &mut Cursor::new(b"new-existing"),
            &existing,
            None,
        )
        .unwrap();
        write_archive_entry_to_staging(
            &mut Cursor::new(b"new-created"),
            &created,
            None,
        )
        .unwrap();

        let replacements = materialize_staged_archive_entries(
            &[existing.clone(), created.clone()],
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(&existing).unwrap(), b"new-existing");
        assert_eq!(std::fs::read(&created).unwrap(), b"new-created");
        assert!(sibling_path(&existing, ".installing.previous").exists());

        replacements.rollback().unwrap();

        assert_eq!(std::fs::read(&existing).unwrap(), b"old-existing");
        assert!(!created.exists());
        assert!(!sibling_path(&existing, ".installing.previous").exists());
    }

    #[test]
    fn materialized_zip_subdir_can_rollback_after_later_failure() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let target_dir = root.path().join("instance");
        let existing = target_dir.join("config/existing.txt");
        let created = target_dir.join("config/created.txt");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"old-existing").unwrap();
        {
            let file = std::fs::File::create(&archive_path).unwrap();
            let mut archive = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            archive
                .start_file("overrides/config/existing.txt", options)
                .unwrap();
            archive.write_all(b"new-existing").unwrap();
            archive
                .start_file("overrides/config/created.txt", options)
                .unwrap();
            archive.write_all(b"new-created").unwrap();
            archive.finish().unwrap();
        }

        let (written, replacements) = materialize_archive_subdir_sync(
            &archive_path,
            "overrides/",
            &target_dir,
            None,
        )
        .unwrap();
        assert_eq!(written, 2);
        assert_eq!(std::fs::read(&existing).unwrap(), b"new-existing");
        assert_eq!(std::fs::read(&created).unwrap(), b"new-created");

        replacements.rollback().unwrap();

        assert_eq!(std::fs::read(&existing).unwrap(), b"old-existing");
        assert!(!created.exists());
        assert!(!sibling_path(&created, ".installing.created").exists());
    }

    #[test]
    fn interrupted_archive_replacements_are_recovered_from_sidecars() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("config/existing.txt");
        let created = root.path().join("config/created.txt");
        let unfinished = root.path().join("config/unfinished.txt");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"old-existing").unwrap();
        write_archive_entry_to_staging(
            &mut Cursor::new(b"new-existing"),
            &existing,
            None,
        )
        .unwrap();
        write_archive_entry_to_staging(
            &mut Cursor::new(b"new-created"),
            &created,
            None,
        )
        .unwrap();
        let replacements = materialize_staged_archive_entries(
            &[existing.clone(), created.clone()],
            None,
        )
        .unwrap();
        std::mem::forget(replacements);
        // A damaged state may contain both marker kinds. The authoritative
        // previous backup must win and must not be deleted by the later
        // created-marker pass.
        std::fs::write(sibling_path(&existing, ".installing.created"), b"")
            .unwrap();
        std::fs::write(sibling_path(&unfinished, ".installing"), b"partial")
            .unwrap();

        recover_interrupted_archive_replacements(root.path()).unwrap();

        assert_eq!(std::fs::read(&existing).unwrap(), b"old-existing");
        assert!(!created.exists());
        assert!(!sibling_path(&existing, ".installing.previous").exists());
        assert!(!sibling_path(&created, ".installing.created").exists());
        assert!(!sibling_path(&unfinished, ".installing").exists());
    }

    #[test]
    fn stale_archive_backup_is_not_silently_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("config/example.txt");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"current").unwrap();
        let backup = sibling_path(&target, ".installing.previous");
        std::fs::write(&backup, b"recoverable-original").unwrap();
        write_archive_entry_to_staging(
            &mut Cursor::new(b"replacement"),
            &target,
            None,
        )
        .unwrap();

        let error = materialize_staged_archive_entries(
            std::slice::from_ref(&target),
            None,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unfinished replacement"));
        assert_eq!(std::fs::read(&backup).unwrap(), b"recoverable-original");
        assert_eq!(std::fs::read(&target).unwrap(), b"current");
    }

    #[test]
    fn duplicate_archive_entries_preserve_preinstall_backup() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("config/example.txt");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"original").unwrap();
        let mut replacements = HashMap::new();
        let mut order = Vec::new();

        write_archive_entry_atomically(
            &mut Cursor::new(b"first-entry"),
            &target,
            None,
            &mut replacements,
            &mut order,
        )
        .unwrap();
        write_archive_entry_atomically(
            &mut Cursor::new(b"second-entry"),
            &target,
            None,
            &mut replacements,
            &mut order,
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second-entry");

        rollback_archive_replacements(&replacements, &order).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn case_variant_zip_entries_finalize_and_rollback() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let first_name = "config/Thaumcraft Mob Aspects[ThaumcraftMobAspects]/lang/zh_CN.lang";
        let second_name = "config/Thaumcraft Mob Aspects[thaumcraftmobaspects]/lang/zh_cn.lang";
        {
            let mut archive = zip::ZipWriter::new(
                std::fs::File::create(&archive_path).unwrap(),
            );
            let options = zip::write::SimpleFileOptions::default();
            for (name, bytes) in [
                (first_name, b"first".as_slice()),
                (second_name, b"second".as_slice()),
            ] {
                archive.start_file(name, options).unwrap();
                archive.write_all(bytes).unwrap();
            }
            archive.finish().unwrap();
        }
        for existing in [false, true] {
            let target_dir =
                root.path()
                    .join(if existing { "existing" } else { "empty" });
            std::fs::create_dir_all(
                target_dir.join(first_name).parent().unwrap(),
            )
            .unwrap();
            let first = target_dir.join(first_name);
            let second = target_dir.join(second_name);
            if existing {
                std::fs::write(&first, b"original").unwrap();
            }
            let (written, replacements) = materialize_archive_subdir_sync(
                &archive_path,
                "",
                &target_dir,
                None,
            )
            .unwrap();
            assert_eq!(written, 2);
            assert_eq!(std::fs::read(&second).unwrap(), b"second");
            let same_file = std::fs::canonicalize(&first).unwrap()
                == std::fs::canonicalize(&second).unwrap();
            assert_eq!(
                replacements.replacements.len(),
                if same_file { 1 } else { 2 }
            );
            replacements.rollback().unwrap();
            if existing {
                assert_eq!(std::fs::read(&first).unwrap(), b"original");
            } else {
                assert!(!first.exists());
            }
            if !same_file || !existing {
                assert!(!second.exists());
            }
            extract_archive_subdir_sync(&archive_path, "", &target_dir, None)
                .unwrap();
            assert_eq!(std::fs::read(&second).unwrap(), b"second");
            for target in [&first, &second] {
                for suffix in [
                    ".installing",
                    ".installing.previous",
                    ".installing.created",
                ] {
                    assert!(!sibling_path(target, suffix).exists());
                }
            }
        }
    }

    #[test]
    #[ignore = "Set AXOLOTL_TEST_MULTIMC_ARCHIVE to a local GTNH MultiMC export"]
    fn real_gtnh_archive_extracts_all_files() {
        let archive_path = PathBuf::from(
            std::env::var_os("AXOLOTL_TEST_MULTIMC_ARCHIVE")
                .expect("local archive path"),
        );
        let root = tempfile::tempdir().unwrap();
        let written = extract_archive_subdir_sync(
            &archive_path,
            "GT New Horizons 2.8.4/",
            root.path(),
            None,
        )
        .unwrap();
        assert!(written > 1000);
        assert!(root.path().join("mmc-pack.json").is_file());
        assert!(
            root.path()
                .join("patches/net.minecraftforge.json")
                .is_file()
        );
        assert!(
            root.path()
                .join("libraries/lwjgl3ify-2.1.16-forgePatches.jar")
                .is_file()
        );
        let file = root.path().join(".minecraft/config/txloader/forceload/Thaumcraft Mob Aspects[thaumcraftmobaspects]/lang/zh_CN.lang");
        assert_eq!(std::fs::metadata(&file).unwrap().len(), 59);
        for suffix in
            [".installing", ".installing.created", ".installing.previous"]
        {
            assert!(!sibling_path(&file, suffix).exists());
        }
        println!("GTNH archive fully extracted: {written} files");
    }

    #[test]
    fn successful_archive_batch_removes_previous_backups() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("config/example.txt");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"original").unwrap();
        let mut replacements = HashMap::new();
        let mut order = Vec::new();

        write_archive_entry_atomically(
            &mut Cursor::new(b"replacement"),
            &target,
            None,
            &mut replacements,
            &mut order,
        )
        .unwrap();
        finalize_archive_replacements(&replacements).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"replacement");
        assert!(!sibling_path(&target, ".installing.previous").exists());
    }

    #[test]
    fn corrupt_later_zip_entry_rolls_back_earlier_replacements() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("pack.zip");
        let target_dir = root.path().join("instance");
        let first = target_dir.join("config/first.txt");
        let second = target_dir.join("config/second.txt");
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"old-first").unwrap();
        std::fs::write(&second, b"old-second").unwrap();

        let second_payload = b"unique-second-entry-payload";
        {
            let file = std::fs::File::create(&archive_path).unwrap();
            let mut archive = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            archive
                .start_file("overrides/config/first.txt", options)
                .unwrap();
            archive.write_all(b"new-first").unwrap();
            archive
                .start_file("overrides/config/second.txt", options)
                .unwrap();
            archive.write_all(second_payload).unwrap();
            archive.finish().unwrap();
        }
        let mut bytes = std::fs::read(&archive_path).unwrap();
        let payload_offset = bytes
            .windows(second_payload.len())
            .position(|window| window == second_payload)
            .expect("stored ZIP payload should be directly addressable");
        bytes[payload_offset] ^= 0xff;
        std::fs::write(&archive_path, bytes).unwrap();

        let error = extract_archive_subdir_sync(
            &archive_path,
            "overrides/",
            &target_dir,
            None,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("CRC")
                || error.to_string().contains("checksum")
        );
        assert_eq!(std::fs::read(&first).unwrap(), b"old-first");
        assert_eq!(std::fs::read(&second).unwrap(), b"old-second");
        assert!(!sibling_path(&first, ".installing").exists());
        assert!(!sibling_path(&first, ".installing.previous").exists());
        assert!(!sibling_path(&second, ".installing").exists());
        assert!(!sibling_path(&second, ".installing.previous").exists());
    }
}
