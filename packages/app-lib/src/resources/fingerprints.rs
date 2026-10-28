//! Attribute-validated byte evidence shared by scans and recognition.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use super::{
    ArtifactIntegrity, ContentHash, HashAlgorithm, ResourceError,
    ResourceResult,
};
use crate::state::store::content_file_fingerprints::{
    FileFingerprintRecord, FileModifiedTime,
};

pub(crate) struct FileFingerprints {
    reads: Semaphore,
    paths: Mutex<HashMap<PathBuf, Weak<Mutex<Option<FileFingerprintRecord>>>>>,
    #[cfg(test)]
    pub(crate) read_count: std::sync::atomic::AtomicU64,
    #[cfg(test)]
    after_read: std::sync::Mutex<Option<Box<dyn FnMut() + Send>>>,
}

impl Default for FileFingerprints {
    fn default() -> Self {
        Self {
            reads: Semaphore::new(2),
            paths: Mutex::new(HashMap::new()),
            #[cfg(test)]
            read_count: std::sync::atomic::AtomicU64::new(0),
            #[cfg(test)]
            after_read: std::sync::Mutex::new(None),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FingerprintResult {
    pub identity: super::identity::LocalArtifactIdentity,
    /// A missing modification time permits this attempt but never cache reuse.
    pub record: Option<FileFingerprintRecord>,
}

impl FileFingerprints {
    pub(crate) async fn seed_verified(
        &self,
        state: &crate::State,
        instance_id: &str,
        root: &Path,
        relative_path: &str,
        integrity: ArtifactIntegrity,
        modified: Option<FileModifiedTime>,
    ) -> ResourceResult<()> {
        use crate::state::store::content_file_fingerprints::ContentFileFingerprints;
        let Some(modified) = modified else {
            return Ok(());
        };
        let root =
            crate::util::io::canonicalize(root).map_err(crate::Error::from)?;
        let path = super::execution::checked_path(&root, relative_path)?;
        let (size, current) = attributes(&path).await?;
        if integrity.size != Some(size) || current.as_ref() != Some(&modified) {
            return Ok(());
        }
        if ContentFileFingerprints::get_current(
            &state.data,
            instance_id,
            relative_path,
        )
        .await?
        .is_some_and(|saved| {
            saved.matches(
                &root.to_string_lossy(),
                relative_path,
                size,
                Some(&modified),
            )
        }) {
            return Ok(());
        }
        ContentFileFingerprints::set_current(
            &state.data,
            instance_id,
            &FileFingerprintRecord {
                content_root: root.to_string_lossy().into_owned(),
                relative_path: relative_path.into(),
                size,
                modified,
                hashes: integrity.hashes,
                curseforge_fingerprint: None,
            },
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn resolve_scan(
        &self,
        root: &Path,
        relative_path: &str,
        previous: Option<&FileFingerprintRecord>,
        verify_bytes: bool,
        cancel: &CancellationToken,
    ) -> ResourceResult<FingerprintResult> {
        self.resolve_inner(
            root,
            relative_path,
            previous,
            verify_bytes,
            false,
            cancel,
        )
        .await
    }

    pub(crate) async fn resolve(
        &self,
        root: &Path,
        relative_path: &str,
        previous: Option<&FileFingerprintRecord>,
        verify_bytes: bool,
        cancel: &CancellationToken,
    ) -> ResourceResult<FingerprintResult> {
        self.resolve_inner(
            root,
            relative_path,
            previous,
            verify_bytes,
            true,
            cancel,
        )
        .await
    }

    async fn resolve_inner(
        &self,
        root: &Path,
        relative_path: &str,
        previous: Option<&FileFingerprintRecord>,
        verify_bytes: bool,
        require_identity: bool,
        cancel: &CancellationToken,
    ) -> ResourceResult<FingerprintResult> {
        let started = std::time::Instant::now();
        let path = super::execution::checked_path(root, relative_path)?;
        let root_name = root.to_string_lossy().into_owned();
        let flight = {
            let mut paths = self.paths.lock().await;
            paths.retain(|_, flight| flight.strong_count() > 0);
            match paths.get(&path).and_then(Weak::upgrade) {
                Some(flight) => flight,
                None => {
                    let flight = Arc::new(Mutex::new(None));
                    paths.insert(path.clone(), Arc::downgrade(&flight));
                    flight
                }
            }
        };
        let mut shared = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(ResourceError::Canceled),
            guard = flight.lock() => guard,
        };
        for attempt in 0..2 {
            let before = attributes(&path).await?;
            let mut reusable = None;
            if !verify_bytes {
                for record in [shared.as_ref(), previous].into_iter().flatten()
                {
                    if record.matches(
                        &root_name,
                        relative_path,
                        before.0,
                        before.1.as_ref(),
                    ) {
                        if !require_identity || complete(record) {
                            tracing::debug!(
                                relative_path,
                                elapsed_us =
                                    started.elapsed().as_micros() as u64,
                                bytes_read = 0u64,
                                "Reused content fingerprint"
                            );
                            return Ok(from_record(record.clone()));
                        }
                        reusable = Some(record.clone());
                        break;
                    }
                }
            }
            let permit = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(ResourceError::Canceled),
                permit = self.reads.acquire() => permit.map_err(|_| ResourceError::Canceled)?,
            };
            let read_path = path.clone();
            let read_cancel = cancel.clone();
            #[cfg(test)]
            self.read_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(test)]
            crate::util::download::verified_file::record_scan();
            let result = tokio::task::spawn_blocking(move || {
                hash_file(&read_path, reusable.as_ref(), &read_cancel)
            })
            .await
            .map_err(|error| {
                ResourceError::InvalidRequest(error.to_string())
            })??;
            drop(permit);
            #[cfg(test)]
            if let Some(hook) = self.after_read.lock().unwrap().as_mut() {
                hook();
            }
            let after = attributes(&path).await?;
            if before != after || result.integrity.size != Some(after.0) {
                if attempt == 0 {
                    continue;
                }
                return Err(ResourceError::InvalidRequest(format!(
                    "{relative_path}: file changed repeatedly during fingerprinting"
                )));
            }
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            let record = after.1.map(|modified| FileFingerprintRecord {
                content_root: root_name.clone(),
                relative_path: relative_path.into(),
                size: after.0,
                modified,
                hashes: result.integrity.hashes.clone(),
                curseforge_fingerprint: result.fingerprint,
            });
            if record.is_none() {
                tracing::warn!(
                    relative_path,
                    "Content modification time is unavailable; fingerprint cannot be reused across refreshes"
                );
            }
            tracing::debug!(
                relative_path,
                bytes_read = after.0,
                elapsed_us = started.elapsed().as_micros() as u64,
                attempt,
                "Computed content fingerprints"
            );
            *shared = record.clone();
            return Ok(FingerprintResult {
                identity: result,
                record,
            });
        }
        unreachable!()
    }
}

fn complete(record: &FileFingerprintRecord) -> bool {
    record.curseforge_fingerprint.is_some()
        && [
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
        ]
        .iter()
        .all(|algorithm| {
            record
                .hashes
                .iter()
                .any(|hash| &hash.algorithm == algorithm)
        })
}

fn from_record(record: FileFingerprintRecord) -> FingerprintResult {
    FingerprintResult {
        identity: super::identity::LocalArtifactIdentity {
            integrity: ArtifactIntegrity {
                size: Some(record.size),
                hashes: record.hashes.clone(),
            },
            fingerprint: record.curseforge_fingerprint,
        },
        record: Some(record),
    }
}

async fn attributes(
    path: &Path,
) -> ResourceResult<(u64, Option<FileModifiedTime>)> {
    let metadata = tokio::fs::metadata(path).await.map_err(|error| {
        ResourceError::InvalidRequest(format!("{}: {error}", path.display()))
    })?;
    if !metadata.is_file() {
        return Err(ResourceError::InvalidRequest(
            "Content path is not a file".into(),
        ));
    }
    Ok((
        metadata.len(),
        metadata
            .modified()
            .ok()
            .and_then(FileModifiedTime::from_system_time),
    ))
}

fn hash_file(
    path: &Path,
    reusable: Option<&FileFingerprintRecord>,
    cancel: &CancellationToken,
) -> ResourceResult<super::identity::LocalArtifactIdentity> {
    use sha2::Digest;
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|error| ResourceError::InvalidRequest(error.to_string()))?;
    let saved = reusable
        .map(|record| record.hashes.clone())
        .unwrap_or_default();
    let missing =
        |algorithm| !saved.iter().any(|hash| hash.algorithm == algorithm);
    let mut sha1 = missing(HashAlgorithm::Sha1).then(sha1_smol::Sha1::new);
    let mut sha256 = missing(HashAlgorithm::Sha256).then(sha2::Sha256::new);
    let mut sha512 = missing(HashAlgorithm::Sha512).then(sha2::Sha512::new);
    let mut normalized = Vec::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 256 * 1024];
    loop {
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let count = file.read(&mut buffer).map_err(|error| {
            ResourceError::InvalidRequest(error.to_string())
        })?;
        if count == 0 {
            break;
        }
        size += count as u64;
        let part = &buffer[..count];
        if let Some(hash) = &mut sha1 {
            hash.update(part);
        }
        if let Some(hash) = &mut sha256 {
            hash.update(part);
        }
        if let Some(hash) = &mut sha512 {
            hash.update(part);
        }
        normalized.extend(part.iter().copied().filter(|byte| {
            !crate::util::fingerprint::is_fingerprint_whitespace(*byte)
        }));
    }
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    tracing::debug!(
        path = %path.display(), bytes_read = size,
        sha1_computed = sha1.is_some(), sha256_computed = sha256.is_some(),
        sha512_computed = sha512.is_some(), curseforge_computed = true,
        "Read content bytes for fingerprints"
    );
    let mut murmur =
        crate::util::fingerprint::Murmur2Hasher::new(normalized.len() as u32);
    murmur.update(&normalized);
    let mut hashes = saved;
    if let Some(hash) = sha1 {
        hashes.push(ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: hash.digest().to_string(),
        });
    }
    if let Some(hash) = sha256 {
        hashes.push(ContentHash {
            algorithm: HashAlgorithm::Sha256,
            value: format!("{:x}", hash.finalize()),
        });
    }
    if let Some(hash) = sha512 {
        hashes.push(ContentHash {
            algorithm: HashAlgorithm::Sha512,
            value: format!("{:x}", hash.finalize()),
        });
    }
    Ok(super::identity::LocalArtifactIdentity {
        integrity: ArtifactIntegrity {
            size: Some(size),
            hashes,
        },
        fingerprint: Some(murmur.finish()),
    })
}

#[cfg(test)]
mod tests;
