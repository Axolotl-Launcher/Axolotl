use super::model::{DownloadRoute, IN_FLIGHT_DOWNLOADS, Integrity};
use super::policy::range_splitting_allowed;
#[cfg(test)]
use super::routes;
#[cfg(test)]
use super::routes::route;
use super::routes::sanitize_url_for_log;
use super::throttle::{IoSemaphore, acquire_native_validation_permit};
use crate::util::download::constants::{
    DOWNLOAD_BUFFER_BYTES, FETCH_BYTES, FETCH_CREATING_DOWNLOAD_FILE,
    FETCH_FINALIZING_DOWNLOAD, FETCH_ICONS, FETCH_OPENING_DOWNLOAD_FILE,
    FETCH_PART_RANGES_JSON, FETCH_PART_RANGES_JSON_2, FETCH_REMOVING,
    FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES,
    FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE, FETCH_SEGMENT,
    HTTPS_SCHEME, PARTIAL_FILE_SUFFIX, STALE_PARTIAL_DOWNLOAD_MAX_AGE,
};
#[cfg(test)]
use crate::util::download::constants::{
    FETCH_127_0_0_1, FETCH_LOCALHOST, HTTP_SCHEME,
};
use crate::util::download::retired_sources;
use crate::util::io::{self, IOError};
use crate::{ErrorKind, LabrinthError};
use bytes::Bytes;
#[cfg(test)]
use eyre::Context;
use parking_lot::Mutex;
use reqwest::{Method, header};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256, Sha512};
#[cfg(test)]
use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(test)]
use std::time;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

#[derive(Default)]
pub(crate) struct IntegrityHashers {
    sha1: Option<sha1_smol::Sha1>,
    sha512: Option<Sha512>,
    sha256: Option<Sha256>,
    md5: Option<md5::Context>,
}

#[derive(Default)]
pub(crate) struct ComputedIntegrity {
    pub(crate) size: u64,
    pub(crate) sha1: Option<String>,
    pub(crate) sha512: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) md5: Option<String>,
}

impl IntegrityHashers {
    pub(crate) fn new_integrity_hashers(integrity: &Integrity) -> Self {
        Self {
            sha1: integrity.sha1.as_ref().map(|_| sha1_smol::Sha1::new()),
            sha512: integrity.sha512.as_ref().map(|_| Sha512::new()),
            sha256: integrity.sha256.as_ref().map(|_| Sha256::new()),
            md5: integrity.md5.as_ref().map(|_| md5::Context::new()),
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        if let Some(hasher) = &mut self.sha1 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.sha512 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.sha256 {
            hasher.update(bytes);
        }
        if let Some(hasher) = &mut self.md5 {
            hasher.consume(bytes);
        }
    }

    pub(crate) fn finish(self, size: u64) -> ComputedIntegrity {
        ComputedIntegrity {
            size,
            sha1: self.sha1.map(|hasher| hasher.digest().to_string()),
            sha512: self
                .sha512
                .map(|hasher| format!("{:x}", hasher.finalize())),
            sha256: self
                .sha256
                .map(|hasher| format!("{:x}", hasher.finalize())),
            md5: self.md5.map(|hasher| format!("{:x}", hasher.finalize())),
        }
    }
}

pub(crate) fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

pub(crate) async fn remove_if_exists(path: &Path) -> crate::Result<()> {
    match io::retry_windows_sharing_violation(path, FETCH_REMOVING, || {
        tokio::fs::remove_file(path)
    })
    .await
    {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::io_error_with_lock_info(error, path).into()),
    }
}

pub(crate) async fn create_download_file(path: &Path) -> Result<File, IOError> {
    io::retry_windows_sharing_violation(
        path,
        FETCH_CREATING_DOWNLOAD_FILE,
        || File::create(path),
    )
    .await
    .map_err(|error| io::io_error_with_lock_info(error, path))
}

pub(crate) async fn open_download_file_for_append(
    path: &Path,
) -> Result<File, IOError> {
    io::retry_windows_sharing_violation(
        path,
        FETCH_OPENING_DOWNLOAD_FILE,
        || async {
            tokio::fs::OpenOptions::new().append(true).open(path).await
        },
    )
    .await
    .map_err(|error| io::io_error_with_lock_info(error, path))
}

/// Keeps a partial `.part` file for a later resume when the download can be
/// safely resumed, at least one route could actually serve a resume, and some
/// data has already arrived; removes it otherwise so unusable partial data
/// does not accumulate on disk.
pub(crate) async fn preserve_or_remove_partial(
    part_path: &Path,
    integrity: &Integrity,
    routes_can_resume: bool,
) -> crate::Result<()> {
    let resumable = routes_can_resume
        && integrity.supports_resume()
        && tokio::fs::metadata(part_path)
            .await
            .is_ok_and(|metadata| metadata.len() > 0);
    if !resumable {
        remove_if_exists(part_path).await?;
    }
    Ok(())
}

pub(crate) fn any_route_can_resume(routes: &[DownloadRoute]) -> bool {
    routes
        .iter()
        .any(|route| route.supports_range && range_splitting_allowed(route))
}

/// Whether a `.part` file with resume data exists at `part_path`. The
/// multiplexed path starts from scratch; resume is handled by the legacy
/// path so a partially downloaded file is never lost.
pub(crate) async fn part_resume_expected(part_path: &Path) -> bool {
    tokio::fs::metadata(part_path)
        .await
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false)
}

fn is_partial_download_file_name(name: &str) -> bool {
    name.ends_with(PARTIAL_FILE_SUFFIX)
        || name.ends_with(FETCH_PART_RANGES_JSON)
        || name
            .rsplit_once(FETCH_PART_RANGES_JSON_2)
            .is_some_and(|(_, suffix)| uuid::Uuid::parse_str(suffix).is_ok())
        || name
            .rsplit_once(FETCH_SEGMENT)
            .is_some_and(|(prefix, index)| {
                prefix.ends_with(PARTIAL_FILE_SUFFIX)
                    && !index.is_empty()
                    && index.bytes().all(|byte| byte.is_ascii_digit())
            })
}

/// Removes partial download files under launcher-managed directories that
/// have not been written to for a week. Partial data is preserved between
/// attempts so interrupted downloads can resume, but destinations that are
/// never requested again (for example superseded modpack versions) would
/// otherwise accumulate multi-gigabyte litter forever.
pub fn cleanup_stale_partial_downloads(directories: Vec<PathBuf>) {
    tokio::task::spawn_blocking(move || {
        let Some(cutoff) = std::time::SystemTime::now()
            .checked_sub(STALE_PARTIAL_DOWNLOAD_MAX_AGE)
        else {
            return;
        };
        let mut pending = directories;
        let mut removed = 0_u64;
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                if !file_type.is_file()
                    || !is_partial_download_file_name(
                        &entry.file_name().to_string_lossy(),
                    )
                {
                    continue;
                }
                let stale = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .is_ok_and(|modified| modified < cutoff);
                if stale && std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            tracing::info!(removed, "Removed stale partial download files");
        }
    });
}

/// Feeds an existing partial download into fresh integrity hashers so a
/// resumed transfer can continue hashing where the file left off. Returns
/// `None` when the file cannot be read back or its length changed.
pub(crate) async fn hash_existing_part_prefix(
    path: &Path,
    integrity: &Integrity,
    expected_len: u64,
) -> Option<IntegrityHashers> {
    let mut file = File::open(path).await.ok()?;
    let mut hashers = IntegrityHashers::new_integrity_hashers(integrity);
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; DOWNLOAD_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer).await.ok()?;
        if read == 0 {
            break;
        }
        hashers.update(&buffer[..read]);
        size += read as u64;
    }
    (size == expected_len).then_some(hashers)
}

/// Keys the in-flight download lock on the destination path, so concurrent
/// downloads writing the same file (and thus the same sibling `.part` file)
/// serialize even when they expect different content. Uppercasing mirrors
/// NTFS `$UpCase` comparison semantics; unlike lowercasing it has no
/// context-sensitive folds that could split one on-disk file into two keys.
fn download_lock_key(destination: &Path) -> String {
    let path = destination.display().to_string();
    if cfg!(windows) {
        path.to_uppercase()
    } else {
        path
    }
}

fn in_flight_download_lock(key: String) -> Arc<AsyncMutex<()>> {
    use dashmap::mapref::entry::Entry;

    if IN_FLIGHT_DOWNLOADS.len() > 4_096 {
        IN_FLIGHT_DOWNLOADS.retain(|_, lock| lock.strong_count() > 0);
    }
    match IN_FLIGHT_DOWNLOADS.entry(key) {
        Entry::Occupied(mut entry) => {
            if let Some(lock) = entry.get().upgrade() {
                lock
            } else {
                let lock = Arc::new(AsyncMutex::new(()));
                entry.insert(Arc::downgrade(&lock));
                lock
            }
        }
        Entry::Vacant(entry) => {
            let lock = Arc::new(AsyncMutex::new(()));
            entry.insert(Arc::downgrade(&lock));
            lock
        }
    }
}

/// Serializes every writer for one destination, including download engines
/// that do not enter `download_to_path` (such as the H2 asset batch).
pub(crate) fn destination_download_lock(
    destination: &Path,
) -> Arc<AsyncMutex<()>> {
    in_flight_download_lock(download_lock_key(destination))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedContentRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
    /// `None` when the server reports an unknown complete length (`*`),
    /// which RFC 9110 permits on a 206 response.
    pub(crate) total: Option<u64>,
}

pub(crate) fn parse_content_range(
    response: &reqwest::Response,
) -> Option<ParsedContentRange> {
    let value = response
        .headers()
        .get(header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .strip_prefix(FETCH_BYTES)?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some(ParsedContentRange {
        start: start.parse().ok()?,
        end: end.parse().ok()?,
        total: if total == "*" {
            None
        } else {
            Some(total.parse().ok()?)
        },
    })
}

pub(crate) async fn response_status_error(
    response: reqwest::Response,
    method: &Method,
    request_url: &str,
) -> crate::Error {
    let status = response.status();
    if let Ok(mut error) = response.json::<LabrinthError>().await {
        error.status = Some(status.as_u16());
        error.method = Some(method.as_str().to_string());
        error.url = Some(sanitize_url_for_log(request_url));
        ErrorKind::LabrinthError(error).into()
    } else {
        ErrorKind::HttpError {
            status: status.as_u16(),
            method: method.as_str().to_string(),
            url: sanitize_url_for_log(request_url),
        }
        .into()
    }
}

/// Commits a fully downloaded `.part` file to `destination` without ever
/// leaving the destination missing: the part file is flushed first, then
/// replaced in a single step. A destination that already exists is only
/// replaced once the new file is complete, so a failure here keeps the
/// previous contents instead of deleting them.
pub(crate) async fn finalize_download(
    part_path: &Path,
    destination: &Path,
) -> crate::Result<()> {
    io::retry_windows_sharing_violation(
        destination,
        FETCH_FINALIZING_DOWNLOAD,
        || finalize_download_once(part_path, destination),
    )
    .await
    .map_err(|error| {
        io::io_error_with_lock_info_for_paths(
            error,
            destination,
            &[destination, part_path],
        )
    })?;
    Ok(())
}

async fn finalize_download_once(
    part_path: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    // Starting with the flush is what makes the commit meaningful: replacing
    // the destination without it could publish contents that a crash then
    // loses. A flush that fails for any reason therefore aborts the commit and
    // leaves the previous destination in place; the partial file stays for the
    // caller's retry, and a partial file that never existed reports `NotFound`.
    io::fsync_file(part_path).await?;
    io::atomically_replace_file(part_path, destination)
}

pub(crate) fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

pub(crate) fn is_allowed_download_redirect(url: &Url) -> bool {
    if retired_sources::is_retired(url) {
        return false;
    }
    if url.scheme() == HTTPS_SCHEME {
        return true;
    }
    #[cfg(test)]
    if url.scheme() == HTTP_SCHEME
        && url.host_str().is_some_and(|host| {
            host == FETCH_LOCALHOST || host == FETCH_127_0_0_1
        })
    {
        return true;
    }
    false
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceValidator {
    etag: Option<String>,
    last_modified: Option<String>,
}

pub(crate) fn response_validator(
    response: &reqwest::Response,
) -> ResourceValidator {
    ResourceValidator {
        etag: response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        last_modified: response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    }
}

pub(crate) fn resource_validator_changed(reason: &str) -> bool {
    matches!(
        reason,
        FETCH_RESOURCE_VALIDATOR_CHANGED_BETWEEN_RANGES
            | FETCH_RESOURCE_VALIDATOR_CHANGED_DURING_TAIL_HEDGE
    )
}

pub(crate) fn validate_resource_version(
    expected: &Mutex<Option<ResourceValidator>>,
    response: &reqwest::Response,
    integrity: &Integrity,
) -> bool {
    let candidate = response_validator(response);
    let mut expected = expected.lock();
    match expected.as_ref() {
        Some(expected) => {
            let matches = (expected.etag.is_none()
                || expected.etag == candidate.etag)
                && (expected.last_modified.is_none()
                    || expected.last_modified == candidate.last_modified);
            if !matches && integrity.supports_resume() {
                // CDN cache variants can have different validators. Every
                // range still needs valid bounds; the expected whole-file
                // hash, checked before publication, pins the actual content.
                tracing::debug!(
                    expected_validator = ?expected,
                    candidate_validator = ?candidate,
                    "Continuing ranges with different metadata; whole-file hash verification is required"
                );
                return true;
            }
            matches
        }
        None => {
            *expected = Some(candidate);
            true
        }
    }
}

pub async fn read_json<T>(
    path: &Path,
    semaphore: &IoSemaphore,
) -> crate::Result<T>
where
    T: DeserializeOwned,
{
    let _permit = semaphore.0.acquire().await?;

    let json = io::read(path).await?;
    let json = serde_json::from_slice::<T>(&json)?;

    Ok(json)
}

#[tracing::instrument(skip(bytes, semaphore))]
pub async fn write(
    path: &Path,
    bytes: &[u8],
    semaphore: &IoSemaphore,
) -> crate::Result<()> {
    let _permit = semaphore.0.acquire().await?;

    if let Some(parent) = path.parent() {
        io::create_dir_all(parent).await?;
    }

    let mut file = create_download_file(path).await?;
    file.write_all(bytes).await.map_err(|e| {
        crate::Error::from(io::io_error_with_lock_info(e, path))
    })?;
    tracing::trace!("Done writing file {}", path.display());
    Ok(())
}

pub async fn copy(
    src: impl AsRef<Path>,
    dest: impl AsRef<Path>,
    semaphore: &IoSemaphore,
) -> crate::Result<()> {
    let src: &Path = src.as_ref();
    let dest = dest.as_ref();

    let _permit = semaphore.0.acquire().await?;

    if let Some(parent) = dest.parent() {
        io::create_dir_all(parent).await?;
    }

    io::copy(src, dest).await?;
    tracing::trace!(
        "Done copying file {} to {}",
        src.display(),
        dest.display()
    );
    Ok(())
}

// Writes a icon to the cache and returns the absolute path of the icon within the cache directory

#[tracing::instrument(skip(bytes, semaphore))]
pub async fn write_cached_icon(
    icon_path: &str,
    cache_dir: &Path,
    bytes: Bytes,
    semaphore: &IoSemaphore,
) -> crate::Result<PathBuf> {
    let extension = Path::new(&icon_path).extension().and_then(OsStr::to_str);
    let hash = sha1_async(bytes.clone()).await?;
    let path = cache_dir
        .join(FETCH_ICONS)
        .join(if let Some(ext) = extension {
            format!("{hash}.{ext}")
        } else {
            hash
        });

    write(&path, &bytes, semaphore).await?;

    let path = io::canonicalize(path)?;
    Ok(path)
}

pub async fn sha1_async(bytes: Bytes) -> crate::Result<String> {
    let hash = tokio::task::spawn_blocking(move || {
        sha1_smol::Sha1::from(bytes).hexdigest()
    })
    .await?;

    Ok(hash)
}

pub async fn sha1_file_async(
    path: impl AsRef<Path>,
) -> crate::Result<(u64, String)> {
    let path = path.as_ref();
    #[cfg(test)]
    crate::util::download::verified_file::record_scan();
    let started = Instant::now();
    let _permit = acquire_native_validation_permit().await?;
    // Local files can be multi-gigabyte .mrpacks, so hash them without materializing bytes.
    let mut file = File::open(path)
        .await
        .map_err(|e| IOError::with_path(e, path))?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; DOWNLOAD_BUFFER_BYTES];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .map_err(|e| IOError::with_path(e, path))?;
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }

    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed SHA-1 file verification");
    Ok((size, hasher.digest().to_string()))
}

pub async fn sha1_file_cancellable(
    path: impl AsRef<Path>,
    cancellation: &tokio_util::sync::CancellationToken,
) -> crate::Result<(u64, String)> {
    let path = path.as_ref();
    #[cfg(test)]
    crate::util::download::verified_file::record_scan();
    let started = Instant::now();
    let _permit = tokio::select! { biased; _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("verification canceled while waiting for resources".into()).into()), permit = acquire_native_validation_permit() => permit? };
    let mut file = File::open(path)
        .await
        .map_err(|e| IOError::with_path(e, path))?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut size = 0;
    let mut buffer = vec![0; DOWNLOAD_BUFFER_BYTES];
    loop {
        let bytes_read = tokio::select! {
            _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("download canceled during verification".to_string()).into()),
            result = file.read(&mut buffer) => result.map_err(|e| IOError::with_path(e, path))?,
        };
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed cancellable SHA-1 file verification");
    Ok((size, hasher.digest().to_string()))
}
