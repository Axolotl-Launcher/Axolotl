//! Integrity hashing and content validation for downloaded files.

use super::super::io::IOError;
use crate::ErrorKind;
use crate::util::download::constants::{
    DOWNLOAD_BUFFER_BYTES, INTEGRITY_HASH_FOR_DOWNLOAD, INTEGRITY_INCORRECT,
    INTEGRITY_MD5, INTEGRITY_SHA1, INTEGRITY_SHA256, INTEGRITY_SHA512,
};
use crate::util::fetch::{
    ComputedIntegrity, ContentValidation, Integrity, IntegrityHashers,
    acquire_native_validation_permit,
};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub(crate) async fn compute_file_integrity(
    path: &Path,
    integrity: &Integrity,
) -> crate::Result<ComputedIntegrity> {
    let started = std::time::Instant::now();
    let _permit = acquire_native_validation_permit().await?;
    let mut file = File::open(path)
        .await
        .map_err(|error| IOError::with_path(error, path))?;
    let mut hashers = IntegrityHashers::new_integrity_hashers(integrity);
    let mut size = 0;
    let mut buffer = vec![0_u8; DOWNLOAD_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| IOError::with_path(error, path))?;
        if read == 0 {
            break;
        }
        hashers.update(&buffer[..read]);
        size += read as u64;
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), cpu_percent = super::local_resources::pressure(path).cpu_percent, "Completed download hash verification");
    Ok(hashers.finish(size))
}

pub(crate) fn verify_computed_integrity(
    expected: &Integrity,
    actual: &ComputedIntegrity,
) -> crate::Result<()> {
    if let Some(size) = expected.size
        && actual.size != size
    {
        if !expected.has_hash() {
            return Err(ErrorKind::OtherError(format!(
                "Incorrect size for download: {size} != {}",
                actual.size
            ))
            .into());
        }
        tracing::warn!(
            expected_size = size,
            actual_size = actual.size,
            "Downloaded size differs from the expected size; relying on content hash verification"
        );
    }

    let checks = [
        (INTEGRITY_SHA1, expected.sha1.as_ref(), actual.sha1.as_ref()),
        (
            INTEGRITY_SHA512,
            expected.sha512.as_ref(),
            actual.sha512.as_ref(),
        ),
        (
            INTEGRITY_SHA256,
            expected.sha256.as_ref(),
            actual.sha256.as_ref(),
        ),
        (INTEGRITY_MD5, expected.md5.as_ref(), actual.md5.as_ref()),
    ];
    for (algorithm, expected, actual) in checks {
        if let Some(expected) = expected
            && actual
                .is_none_or(|actual| !actual.eq_ignore_ascii_case(expected))
        {
            return Err(ErrorKind::OtherError(format!(
                "Incorrect {algorithm} hash for download: {expected} != {}",
                actual.map(String::as_str).unwrap_or("not computed")
            ))
            .into());
        }
    }
    Ok(())
}

pub(crate) fn is_integrity_error(error: &crate::Error) -> bool {
    match error.raw.as_ref() {
        ErrorKind::HashError(..) => true,
        ErrorKind::OtherError(message) => {
            message.starts_with(INTEGRITY_INCORRECT)
                && message.contains(INTEGRITY_HASH_FOR_DOWNLOAD)
        }
        _ => false,
    }
}

pub(crate) async fn validate_file_content(
    path: &Path,
    validation: ContentValidation,
) -> crate::Result<()> {
    if validation == ContentValidation::None {
        return Ok(());
    }
    let started = std::time::Instant::now();
    let _permit = acquire_native_validation_permit().await?;
    let log_path = path.to_path_buf();
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> crate::Result<()> {
        let file = std::fs::File::open(&path)
            .map_err(|error| IOError::with_path(error, &path))?;
        match validation {
            ContentValidation::None => {}
            ContentValidation::Json => {
                serde_json::from_reader::<_, serde_json::Value>(file)?;
            }
            ContentValidation::Jar => {
                zip::ZipArchive::new(file).map_err(|error| {
                    ErrorKind::OtherError(format!(
                        "Invalid JAR archive {}: {error}",
                        path.display()
                    ))
                })?;
            }
        }
        Ok(())
    })
    .await??;
    tracing::debug!(path = %log_path.display(), validation_ms = started.elapsed().as_millis(), "Completed download content validation");
    Ok(())
}

pub(crate) async fn verify_file(
    path: &Path,
    integrity: &Integrity,
) -> crate::Result<u64> {
    let computed = compute_file_integrity(path, integrity).await?;
    verify_computed_integrity(integrity, &computed)?;
    validate_file_content(path, integrity.content).await?;
    Ok(computed.size)
}
