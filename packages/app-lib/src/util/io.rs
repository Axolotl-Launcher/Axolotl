// IO error
// A wrapper around the tokio IO functions that adds the path to the error message, instead of the uninformative std::io::Error.

use eyre::{Context, ContextCompat, Result, eyre};
use std::{
    future::Future,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use tokio::task::spawn_blocking;
use tracing::warn;

use crate::util::file_lock::get_locking_processes;

#[derive(Debug, thiserror::Error)]
pub enum IOError {
    #[error("{source}, path: {path}")]
    IOPathError {
        #[source]
        source: std::io::Error,
        path: String,
    },
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

impl IOError {
    pub fn from(source: std::io::Error) -> Self {
        Self::IOError(source)
    }
    pub fn with_path(
        source: std::io::Error,
        path: impl AsRef<std::path::Path>,
    ) -> Self {
        let path = path.as_ref();

        Self::IOPathError {
            source,
            path: path.to_string_lossy().to_string(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        match self {
            IOError::IOPathError { source, .. } => source.kind(),
            IOError::IOError(source) => source.kind(),
        }
    }
}

/// Check if an `std::io::Error` is a permission / sharing-violation that may
/// indicate another process holds a file lock. If so, attempt to detect the
/// locking processes and return a richer `IOError` with their details appended.
pub(crate) fn io_error_with_lock_info(
    source: std::io::Error,
    path: impl AsRef<std::path::Path>,
) -> IOError {
    let path = path.as_ref();
    io_error_with_lock_info_for_paths(source, path, &[path])
}

/// Makes a remote file name portable while rejecting paths and traversal.
pub(crate) fn sanitize_download_file_name(name: &str) -> crate::Result<String> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\'])
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid download file name".to_string(),
        )
        .into());
    }
    let trimmed_len = name.trim_end_matches(['.', ' ']).len();
    let name: String = name
        .char_indices()
        .map(|(index, character)| {
            if index >= trimmed_len
                || character.is_control()
                || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    safe_relative_join(Path::new(""), &name)?;
    Ok(name)
}

/// Joins an untrusted Unix-style relative file path, rejecting traversal and
/// Windows aliases on every platform. This is a lexical check, not a symlink
/// sandbox.
pub(crate) fn safe_relative_join(
    root: &Path,
    relative: &str,
) -> crate::Result<PathBuf> {
    path_util::SafeRelativeUtf8UnixPathBuf::try_from(relative.to_string())
        .map_err(|error| {
            crate::ErrorKind::InputError(format!(
                "Invalid file path {relative}: {error}"
            ))
        })?;
    if relative.split('/').any(|segment| {
        segment.is_empty()
            || segment == "."
            || segment.ends_with(['.', ' '])
            || segment.contains(':')
            || segment.chars().any(char::is_control)
    }) {
        return Err(crate::ErrorKind::InputError(format!(
            "Invalid file path: {relative}"
        ))
        .into());
    }
    Ok(root.join(relative))
}

/// Join a user-provided relative path onto `root`, rejecting traversal.
pub(crate) fn join_within_root(
    root: &Path,
    relative: &str,
) -> crate::Result<PathBuf> {
    let normalized = relative.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return Err(crate::ErrorKind::InputError(
            "Inner path must be relative".to_string(),
        )
        .into());
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(crate::ErrorKind::InputError(
                    "Inner path escapes the source root".to_string(),
                )
                .into());
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Inner path must not be empty".to_string(),
        )
        .into());
    }
    safe_relative_join(root, &safe.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn io_error_with_lock_info_for_paths(
    source: std::io::Error,
    error_path: &Path,
    lock_paths: &[&Path],
) -> IOError {
    let raw_os_error = source.raw_os_error();
    let is_lock_error = source.kind() == ErrorKind::PermissionDenied
        || (cfg!(windows) && raw_os_error == Some(32))
        || (cfg!(target_os = "linux") && raw_os_error == Some(26));

    if is_lock_error
        && let Some((locked_path, processes)) =
            lock_paths.iter().find_map(|path| {
                let processes = get_locking_processes(path);
                (!processes.is_empty()).then_some((*path, processes))
            })
    {
        warn!(
            "File lock detected on {} — {} holding process(es)",
            locked_path.display(),
            processes.len()
        );
        let detail = processes
            .iter()
            .map(|p| {
                format!(
                    "  PID {} - {} (locked path: {})",
                    p.pid, p.name, p.path
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let enhanced = std::io::Error::new(
            source.kind(),
            format!("{source}\n\nFile locked by:\n{detail}"),
        );
        return IOError::IOPathError {
            source: enhanced,
            path: error_path.to_string_lossy().to_string(),
        };
    }

    IOError::IOPathError {
        source,
        path: error_path.to_string_lossy().to_string(),
    }
}

#[cfg(windows)]
const WINDOWS_SHARING_VIOLATION_RETRY_DELAYS: [std::time::Duration; 5] = [
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(250),
    std::time::Duration::from_millis(500),
    std::time::Duration::from_secs(1),
    std::time::Duration::from_secs(2),
];

#[cfg(windows)]
pub(crate) async fn retry_windows_sharing_violation<T, F, Fut>(
    path: &Path,
    operation: &str,
    action: F,
) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    retry_windows_sharing_violation_with_delays(
        path,
        operation,
        &WINDOWS_SHARING_VIOLATION_RETRY_DELAYS,
        action,
    )
    .await
}

#[cfg(windows)]
async fn retry_windows_sharing_violation_with_delays<T, F, Fut>(
    path: &Path,
    operation: &str,
    delays: &[std::time::Duration],
    mut action: F,
) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    for (retry_index, delay) in delays.iter().enumerate() {
        match action().await {
            Err(error) if error.raw_os_error() == Some(32) => {
                warn!(
                    "Windows sharing violation while {operation} {}. Retry {}/{} in {} ms",
                    path.display(),
                    retry_index + 1,
                    delays.len(),
                    delay.as_millis()
                );
                tokio::time::sleep(*delay).await;
            }
            result => return result,
        }
    }

    action().await
}

#[cfg(not(windows))]
pub(crate) async fn retry_windows_sharing_violation<T, F, Fut>(
    _path: &Path,
    _operation: &str,
    mut action: F,
) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    action().await
}

pub fn canonicalize(
    path: impl AsRef<std::path::Path>,
) -> Result<std::path::PathBuf, IOError> {
    let path = path.as_ref();
    dunce::canonicalize(path).map_err(|e| IOError::IOPathError {
        source: e,
        path: path.to_string_lossy().to_string(),
    })
}

pub async fn read_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<tokio::fs::ReadDir, IOError> {
    let path = path.as_ref();
    tokio::fs::read_dir(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn create_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::create_dir(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn create_dir_all(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub(crate) fn is_symlink_or_reparse(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub async fn remove_dir_all(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        fn remove_safe(path: &Path) -> Result<(), IOError> {
            let meta = match std::fs::symlink_metadata(path) {
                Ok(m) => m,
                Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(io_error_with_lock_info(e, path)),
            };

            if is_symlink_or_reparse(&meta) {
                return std::fs::remove_file(path)
                    .or_else(|_| std::fs::remove_dir(path))
                    .map_err(|e| io_error_with_lock_info(e, path));
            }

            if meta.is_file() {
                return std::fs::remove_file(path)
                    .map_err(|e| io_error_with_lock_info(e, path));
            }

            let rd = match std::fs::read_dir(path) {
                Ok(d) => d,
                Err(e) => return Err(io_error_with_lock_info(e, path)),
            };
            for entry in rd {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => return Err(io_error_with_lock_info(e, path)),
                };
                remove_safe(&entry.path())?;
            }

            std::fs::remove_dir(path)
                .map_err(|e| io_error_with_lock_info(e, path))
        }

        remove_safe(&path)
    })
    .await
    .map_err(|e| {
        IOError::IOError(std::io::Error::other(format!(
            "background task failed: {e}"
        )))
    })?
}

/// Reads a text file to a string, automatically detecting its encoding and
/// substituting any invalid characters with the Unicode replacement character.
///
/// This function is best suited for reading Minecraft instance files, whose
/// encoding may vary depending on the platform, launchers, client versions
/// (older Minecraft versions tended to rely on the system's default codepage
/// more on Windows platforms), and mods used, while not being highly sensitive
/// to occasional occurrences of mojibake or character replacements.
pub async fn read_any_encoding_to_string(
    path: impl AsRef<std::path::Path>,
) -> Result<(String, &'static encoding_rs::Encoding), IOError> {
    let path = path.as_ref();
    let file_bytes =
        tokio::fs::read(path)
            .await
            .map_err(|e| IOError::IOPathError {
                source: e,
                path: path.to_string_lossy().to_string(),
            })?;

    let file_encoding = {
        let mut encoding_detector = chardetng::EncodingDetector::new();
        encoding_detector.feed(&file_bytes, true);
        encoding_detector.guess(None, true)
    };

    let (file_string, actual_file_encoding, _) =
        file_encoding.decode(&file_bytes);
    Ok((file_string.to_string(), actual_file_encoding))
}

pub async fn read(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<u8>, IOError> {
    let path = path.as_ref();
    tokio::fs::read(path)
        .await
        .map_err(|e| io_error_with_lock_info(e, path))
}

pub async fn write(
    path: impl AsRef<std::path::Path>,
    data: impl AsRef<[u8]>,
) -> Result<(), IOError> {
    let path = path.as_ref().to_owned();
    let data = data.as_ref().to_owned();
    spawn_blocking(move || {
        let cloned_path = path.clone();
        sync_write(data, path)
            .map_err(|e| io_error_with_lock_info(e, &cloned_path))
    })
    .await
    .map_err(|_| std::io::Error::other("background task failed"))??;

    Ok(())
}

fn sync_write(
    data: impl AsRef<[u8]>,
    path: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    let mut tempfile =
        NamedTempFile::new_in(path.as_ref().parent().ok_or_else(|| {
            std::io::Error::other(
                "could not get parent directory for temporary file",
            )
        })?)?;
    tempfile.write_all(data.as_ref())?;
    let tmp_path = tempfile.into_temp_path();
    let path = path.as_ref();
    tmp_path.persist(path)?;
    std::io::Result::Ok(())
}

pub fn is_same_disk(old_dir: &Path, new_dir: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        use eyre::eyre;

        // we need to use `symlink_metadata` instead of `metadata`, because
        // if this file is a symlink, we need to query the symlink file itself,
        // rather than the target.
        // downloaded JREs use symlinks to point to certain stuff like LICENSE
        // files.
        // this fixes moving JRE dirs.

        let old_meta = std::fs::symlink_metadata(old_dir)
            .wrap_err_with(|| eyre!("getting meta of old dir {old_dir:?}"))?;
        let new_meta = std::fs::symlink_metadata(new_dir)
            .wrap_err_with(|| eyre!("getting meta of new dir {new_dir:?}"))?;

        Ok(old_meta.dev() == new_meta.dev())
    }

    #[cfg(windows)]
    {
        // Extract the volume prefix from the raw path without following
        // symlinks/reparse points.  `canonicalize` would follow them and
        // fail on dangling links (target already deleted), so we compare
        // drive letters directly from the path components first.
        let old_prefix = old_dir.components().next();
        let new_prefix = new_dir.components().next();

        let same = match (old_prefix, new_prefix) {
            (
                Some(std::path::Component::Prefix(old)),
                Some(std::path::Component::Prefix(new)),
            ) => old.as_os_str() == new.as_os_str(),
            _ => {
                // Fall back to canonicalization for relative paths, UNC, etc.
                let old_dir = canonicalize(old_dir)?;
                let new_dir = canonicalize(new_dir)?;
                match (old_dir.components().next(), new_dir.components().next())
                {
                    (
                        Some(std::path::Component::Prefix(old)),
                        Some(std::path::Component::Prefix(new)),
                    ) => old.as_os_str() == new.as_os_str(),
                    _ => false,
                }
            }
        };
        Ok(same)
    }
}

/// Replaces `destination` with `source` in a single step where the platform
/// supports it. A destination that already exists is replaced, and the
/// previous contents stay in place when the operation fails, so callers never
/// have to remove the destination first.
#[cfg(not(target_os = "windows"))]
pub(crate) fn atomically_replace_file(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
pub(crate) fn atomically_replace_file(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| {
            std::io::Error::from_raw_os_error(win32_from_hresult(
                error.code().0,
            ))
        })
    }
}

/// The `windows` crate reports a failed Win32 call as an `HRESULT` built by
/// `HRESULT_FROM_WIN32`, so the original error code sits in the low word under
/// the `0x8007` facility. `std::io::Error` and the sharing-violation retry
/// below compare against real Win32 codes, so unwrap it here: a value left as
/// an `HRESULT` would never match `ERROR_SHARING_VIOLATION` and would also
/// report the wrong `ErrorKind`.
#[cfg(target_os = "windows")]
pub(crate) fn win32_from_hresult(code: i32) -> i32 {
    const FACILITY_WIN32: u32 = 0x0007;
    let bits = code as u32;
    if (bits >> 16) & 0x1FFF == FACILITY_WIN32 {
        (bits & 0xFFFF) as i32
    } else {
        code
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn win32_from_hresult(code: i32) -> i32 {
    code
}

/// Flushes a file's contents to stable storage. Used before an atomic replace
/// so a crash after the rename cannot leave an empty destination behind.
///
/// Opened for writing on purpose: `FlushFileBuffers` requires a handle with
/// write access, so a read-only handle makes this fail with access denied on
/// Windows.
pub(crate) async fn fsync_file(path: &std::path::Path) -> std::io::Result<()> {
    let file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
    file.sync_all().await
}

pub async fn rename_or_move(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();

    let to_parent = to
        .parent()
        .wrap_err_with(|| eyre!("getting parent of `to` dir {to:?}"))?;
    let same_disk = is_same_disk(from, to_parent).wrap_err_with(|| {
        eyre!("checking if `to_parent` ({to_parent:?}) and `from` ({from:?}) are on the same disk")
    })?;

    if same_disk {
        tokio::fs::rename(from, to)
            .await
            .map_err(|e| io_error_with_lock_info(e, from))
            .wrap_err_with(|| eyre!("moving {from:?} to {to:?} on same disk"))
    } else {
        move_recursive(from, to).await.with_context(|| {
            eyre!("moving {from:?} to {to:?} on different disks")
        })
    }
}

#[async_recursion::async_recursion]
async fn move_recursive(from: &Path, to: &Path) -> Result<()> {
    let meta = tokio::fs::symlink_metadata(from)
        .await
        .wrap_err_with(|| eyre!("getting metadata of {from:?}"))?;

    // Symlink / reparse point: recreate the link at the destination and
    // remove the source link.  Never follow/copy the target.
    if is_symlink_or_reparse(&meta) {
        let target = tokio::fs::read_link(from)
            .await
            .wrap_err_with(|| eyre!("reading link target of {from:?}"))?;
        create_symlink(&target, to).await.wrap_err_with(|| {
            eyre!("recreating symlink at {to:?} (target: {target:?})")
        })?;
        // Remove the source link.  On Unix, remove_file works for all
        // symlinks.  On Windows, directory reparse points (junctions)
        // may need remove_dir as fallback.
        if tokio::fs::remove_file(from).await.is_err() {
            tokio::fs::remove_dir(from)
                .await
                .map_err(|e| io_error_with_lock_info(e, from))
                .wrap_err_with(|| eyre!("removing source link {from:?}"))?;
        }
        return Ok(());
    }

    if meta.is_file() {
        copy(from, to)
            .await
            .wrap_err_with(|| eyre!("copying {from:?} to {to:?}"))?;
        remove_file(from).await.wrap_err_with(|| {
            eyre!("removing {from:?} after copying to {to:?}")
        })?;
        return Ok(());
    }

    create_dir(to)
        .await
        .wrap_err_with(|| eyre!("creating dir for {to:?}"))?;

    let mut dir = read_dir(from)
        .await
        .wrap_err_with(|| eyre!("reading dir {from:?}"))?;
    while let Some(entry) = dir
        .next_entry()
        .await
        .wrap_err_with(|| eyre!("reading dir entry in {from:?}"))?
    {
        let new_path = to.join(entry.file_name());
        move_recursive(&entry.path(), &new_path)
            .await
            .with_context(|| {
                eyre!("moving {:?} to {new_path:?}", entry.path())
            })?;
    }

    remove_dir(from)
        .await
        .wrap_err_with(|| eyre!("removing source dir {from:?}"))?;

    Ok(())
}

pub async fn copy(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> Result<u64, IOError> {
    let from = from.as_ref().to_path_buf();
    let to = to.as_ref().to_path_buf();

    // Never follow symlinks / reparse points — recreate the link at the
    // destination instead of copying the target's content.
    let meta = tokio::fs::symlink_metadata(&from)
        .await
        .map_err(|e| io_error_with_lock_info(e, &from))?;

    if is_symlink_or_reparse(&meta) {
        let target = tokio::fs::read_link(&from)
            .await
            .map_err(|e| io_error_with_lock_info(e, &from))?;
        create_symlink(&target, &to).await?;
        return Ok(0);
    }

    retry_windows_sharing_violation(&to, "copying", || {
        let from = from.clone();
        let to = to.clone();
        async move { tokio::fs::copy(from, to).await }
    })
    .await
    .map_err(|e| io_error_with_lock_info(e, &from))
}

/// Recursively copy a directory from `from` to `to`.
///
/// Creates the target directory and recursively copies all files and
/// subdirectories. Directory symlinks / junctions are followed and their
/// target subtree is materialized as a real directory; file links are copied
/// as their target's content. Broken links and link cycles fail with a
/// descriptive error instead of a generic read failure.
pub async fn copy_dir(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let from = from.as_ref().to_path_buf();
    let to = to.as_ref().to_path_buf();
    copy_dir_inner(&from, &to, &mut Vec::new()).await
}

#[async_recursion::async_recursion]
async fn copy_dir_inner(
    from: &Path,
    to: &Path,
    dir_stack: &mut Vec<PathBuf>,
) -> Result<(), IOError> {
    use async_walkdir::WalkDir;
    use futures::StreamExt as _;

    create_dir_all(to).await?;

    // Track canonicalized directories on the current recursion path so a
    // symlink pointing back to an ancestor fails cleanly instead of
    // recursing forever.
    let canonical = canonicalize(from)?;
    if dir_stack.contains(&canonical) {
        return Err(IOError::with_path(
            std::io::Error::other(format!(
                "symlink cycle detected while copying {from:?}"
            )),
            from,
        ));
    }
    dir_stack.push(canonical);

    let mut entries = WalkDir::new(from);
    while let Some(entry) = entries.next().await {
        let entry = entry.map_err(|e| {
            IOError::with_path(std::io::Error::other(e.to_string()), from)
        })?;

        // Skip macOS resource forks.
        if entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with("__MACOSX"))
            .unwrap_or(false)
        {
            continue;
        }

        let entry_path = entry.path().to_path_buf();
        let relative = entry_path.strip_prefix(from).map_err(|_| {
            IOError::with_path(
                std::io::Error::other("path prefix mismatch"),
                &entry_path,
            )
        })?;
        let target = to.join(relative);

        let meta = tokio::fs::symlink_metadata(&entry_path)
            .await
            .map_err(|e| IOError::with_path(e, &entry_path))?;

        if is_symlink_or_reparse(&meta) {
            let raw_target = tokio::fs::read_link(&entry_path)
                .await
                .map_err(|e| IOError::with_path(e, &entry_path))?;
            // Relative link targets resolve against the link's parent
            // directory, not the process working directory.
            let resolved = if raw_target.is_absolute() {
                raw_target
            } else {
                entry_path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(raw_target)
            };
            let target_meta = match tokio::fs::metadata(&resolved).await {
                Ok(meta) => meta,
                Err(_) => {
                    return Err(IOError::with_path(
                        std::io::Error::other(format!(
                            "broken symlink {entry_path:?} points to missing target {resolved:?}"
                        )),
                        &entry_path,
                    ));
                }
            };
            if target_meta.is_dir() {
                // Materialize the directory link: copy the target subtree as
                // a real directory instead of recreating the link.
                copy_dir_inner(&resolved, &target, dir_stack).await?;
            } else {
                // Materialize file links as their target's content.
                let bytes = read(&entry_path).await?;
                write(&target, bytes).await?;
            }
        } else if meta.is_dir() {
            create_dir_all(&target).await?;
        } else {
            let bytes = read(&entry_path).await?;
            write(&target, bytes).await?;
        }
    }

    dir_stack.pop();
    Ok(())
}

pub async fn remove_file(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::remove_file(path)
        .await
        .map_err(|e| io_error_with_lock_info(e, path))
}

pub async fn remove_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::remove_dir(path)
        .await
        .map_err(|e| io_error_with_lock_info(e, path))
}

pub async fn metadata(
    path: impl AsRef<std::path::Path>,
) -> Result<std::fs::Metadata, IOError> {
    let path = path.as_ref();
    tokio::fs::metadata(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

/// Gets a resource file from the executable. Returns `theseus::Result<(TempDir, PathBuf)>`.
#[macro_export]
macro_rules! get_resource_file {
    (directory: $relative_dir:expr, file: $file_name:expr) => {
        'get_resource_file: {
            let dir = match tempfile::tempdir() {
                Ok(dir) => dir,
                Err(e) => {
                    break 'get_resource_file $crate::Result::Err(
                        $crate::util::io::IOError::from(e).into(),
                    );
                }
            };
            let path = dir.path().join($file_name);
            if let Err(e) = $crate::util::io::write(
                &path,
                include_bytes!(concat!($relative_dir, "/", $file_name)),
            )
            .await
            {
                break 'get_resource_file $crate::Result::Err(e.into());
            }
            let path = match $crate::util::io::canonicalize(path) {
                Ok(path) => path,
                Err(e) => {
                    break 'get_resource_file $crate::Result::Err(e.into());
                }
            };
            $crate::Result::Ok((dir, path))
        }
    };

    ($relative_dir:literal / $file_name:literal) => {
        get_resource_file!(directory: $relative_dir, file: $file_name)
    };

    (env $dir_env_name:literal / $file_name:literal) => {
        get_resource_file!(directory: env!($dir_env_name), file: $file_name)
    };
}

pub async fn create_symlink(
    target: impl AsRef<std::path::Path>,
    link: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let target = target.as_ref().to_path_buf();
    let link = link.as_ref().to_path_buf();
    let link_for_error = link.clone();

    if let Some(parent) = link.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| IOError::with_path(e, parent))?;
    }

    if !target.exists() {
        return Err(IOError::with_path(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Target path does not exist: {}", target.display()),
            ),
            &target,
        ));
    }

    let link_target = target.clone();
    let link_path = link.clone();
    let result = spawn_blocking(move || {
        super::symlink::create_link_blocking(&link_target, &link_path)
    })
    .await
    .map_err(|e| {
        IOError::with_path(
            std::io::Error::other(format!("symlink task panicked: {e}")),
            &link_for_error,
        )
    })?;

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            #[cfg(target_os = "windows")]
            if error.kind() == ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314)
            {
                // Symbolic links need administrator privileges when Windows
                // Developer Mode is off. Run the creation in a short-lived
                // elevated helper process (UAC prompt) instead of elevating
                // the launcher itself, which would break drag-and-drop.
                return super::symlink::create_link_elevated(&target, &link)
                    .await
                    .map_err(|e| IOError::with_path(e, &link_for_error));
            }
            Err(IOError::with_path(error, &link_for_error))
        }
    }
}

#[cfg(all(test, windows))]
mod windows_sharing_violation_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn retries_sharing_violation_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let action_attempts = attempts.clone();
        let delays = [std::time::Duration::ZERO; 2];

        retry_windows_sharing_violation_with_delays(
            Path::new("locked.test"),
            "testing",
            &delays,
            move || {
                let attempt = action_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        Err(std::io::Error::from_raw_os_error(32))
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .expect("third attempt succeeds");

        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_other_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let action_attempts = attempts.clone();
        let delays = [std::time::Duration::ZERO; 2];

        let error = retry_windows_sharing_violation_with_delays(
            Path::new("failed.test"),
            "testing",
            &delays,
            move || {
                action_attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(std::io::Error::from_raw_os_error(5)) }
            },
        )
        .await
        .expect_err("access denied must not be retried");

        assert_eq!(error.raw_os_error(), Some(5));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn sharing_violation_retries_are_bounded() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let action_attempts = attempts.clone();
        let delays = [std::time::Duration::ZERO; 2];

        let error = retry_windows_sharing_violation_with_delays(
            Path::new("locked.test"),
            "testing",
            &delays,
            move || {
                action_attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>(std::io::Error::from_raw_os_error(32)) }
            },
        )
        .await
        .expect_err("sharing violation remains after retry budget");

        assert_eq!(error.raw_os_error(), Some(32));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}

#[cfg(test)]
mod atomic_replace_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn download_file_names_replace_invalid_characters() {
        for (original, expected) in [
            (
                "Yet Another Bingo: Ultimate-214.0.mrpack",
                "Yet Another Bingo_ Ultimate-214.0.mrpack",
            ),
            ("mod<>:\"|?*\0\n.jar", "mod_________.jar"),
            ("[中文] mod-1.2+forge.jar", "[中文] mod-1.2+forge.jar"),
            ("pack.zip. ", "pack.zip__"),
        ] {
            assert_eq!(
                sanitize_download_file_name(original).unwrap(),
                expected
            );
        }
        for invalid in [
            "",
            ".",
            "..",
            "../mod.jar",
            "nested/mod.jar",
            r"..\mod.jar",
            "C:/mod.jar",
            "NUL.jar",
        ] {
            assert!(
                sanitize_download_file_name(invalid).is_err(),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn join_within_root_uses_portable_path_validation() {
        let root = Path::new("root");
        assert_eq!(
            join_within_root(root, r"config\example.toml").unwrap(),
            root.join("config/example.toml")
        );
        assert_eq!(
            join_within_root(root, "./config/./example.toml").unwrap(),
            root.join("config/example.toml")
        );
        for relative in [
            "../outside",
            r"..\outside",
            "/outside",
            "C:/outside",
            "C:outside",
            r"\\server\share",
            "config/.. /outside",
            "config/file:stream",
            "config/NUL",
            "config/file.",
            "config/file\0",
        ] {
            assert!(join_within_root(root, relative).is_err(), "{relative:?}");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_win32_hresult_is_unwrapped_to_its_error_code() {
        // HRESULT_FROM_WIN32(ERROR_SHARING_VIOLATION) as the `windows` crate
        // reports it.
        assert_eq!(win32_from_hresult(0x8007_0020u32 as i32), 32);
        assert_eq!(win32_from_hresult(0x8007_0005u32 as i32), 5);
        // A plain Win32 code and an unrelated HRESULT both pass through.
        assert_eq!(win32_from_hresult(2), 2);
        assert_eq!(
            win32_from_hresult(0x8000_4005u32 as i32),
            0x8000_4005u32 as i32
        );
    }

    #[tokio::test]
    async fn atomically_replace_file_swaps_contents_in_place() {
        let directory = tempdir().expect("temp dir");
        let source = directory.path().join("source.tmp");
        let destination = directory.path().join("destination.bin");
        tokio::fs::write(&source, b"new")
            .await
            .expect("write source");
        tokio::fs::write(&destination, b"old")
            .await
            .expect("write destination");

        atomically_replace_file(&source, &destination).expect("replace");

        assert_eq!(tokio::fs::read(&destination).await.expect("read"), b"new");
        assert!(!source.exists(), "the source file is consumed");
    }

    #[tokio::test]
    async fn fsync_file_accepts_a_partial_file_and_reports_missing_ones() {
        let directory = tempdir().expect("temp dir");
        let part = directory.path().join("download.part");
        tokio::fs::write(&part, b"partial")
            .await
            .expect("write part");

        fsync_file(&part).await.expect("flush an existing file");

        let missing = directory.path().join("missing.part");
        let error = fsync_file(&missing).await.expect_err("missing file");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}

#[cfg(test)]
mod copy_dir_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn copy_dir_copies_plain_tree() {
        let src = tempdir().expect("temp src");
        std::fs::create_dir(src.path().join("sub")).expect("sub dir");
        std::fs::write(src.path().join("root.txt"), b"root")
            .expect("write root");
        std::fs::write(src.path().join("sub/nested.txt"), b"nested")
            .expect("write nested");

        let dst = tempdir().expect("temp dst");
        copy_dir(src.path(), dst.path().join("out"))
            .await
            .expect("copy");

        assert_eq!(
            std::fs::read(dst.path().join("out/root.txt")).expect("read root"),
            b"root"
        );
        assert_eq!(
            std::fs::read(dst.path().join("out/sub/nested.txt"))
                .expect("read nested"),
            b"nested"
        );
    }

    #[cfg(unix)]
    mod unix_symlinks {
        use super::*;
        use std::os::unix::fs::symlink;

        #[tokio::test]
        async fn copy_dir_materializes_directory_symlink() {
            let src = tempdir().expect("temp src");
            let real = src.path().join("real");
            std::fs::create_dir(&real).expect("real dir");
            std::fs::write(real.join("file.txt"), b"content")
                .expect("write file");
            symlink(&real, src.path().join("link")).expect("create link");

            let dst = tempdir().expect("temp dst");
            copy_dir(src.path(), dst.path().join("out"))
                .await
                .expect("copy");

            let copied = dst.path().join("out/link/file.txt");
            assert!(
                copied.is_file(),
                "directory symlink should be materialized"
            );
            assert_eq!(
                std::fs::read(&copied).expect("read copied file"),
                b"content"
            );
            assert!(
                std::fs::symlink_metadata(dst.path().join("out/link"))
                    .expect("meta")
                    .file_type()
                    .is_dir()
            );
        }

        #[tokio::test]
        async fn copy_dir_reports_broken_symlink() {
            let src = tempdir().expect("temp src");
            symlink(src.path().join("missing"), src.path().join("broken"))
                .expect("create broken link");

            let dst = tempdir().expect("temp dst");
            let err = copy_dir(src.path(), dst.path().join("out"))
                .await
                .expect_err("broken link should fail");
            let text = format!("{err}");
            assert!(
                text.contains("broken symlink"),
                "error should mention the broken link: {text}"
            );
        }

        #[tokio::test]
        async fn copy_dir_rejects_symlink_cycle() {
            let src = tempdir().expect("temp src");
            std::fs::create_dir(src.path().join("a")).expect("a dir");
            symlink(src.path().join("a"), src.path().join("a/back"))
                .expect("create cycle link");

            let dst = tempdir().expect("temp dst");
            let err = copy_dir(src.path(), dst.path().join("out"))
                .await
                .expect_err("cycle should fail");
            let text = format!("{err}");
            assert!(
                text.contains("symlink cycle"),
                "error should mention the cycle: {text}"
            );
        }
    }
}
