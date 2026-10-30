//! Murmur2 fingerprinting and sha1 hashing of CurseForge files.

use crate::ErrorKind;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

async fn read_curseforge_verification_chunk(
    file: &mut tokio::fs::File,
    buffer: &mut [u8],
    cancellation: Option<&CancellationToken>,
) -> crate::Result<usize> {
    match cancellation {
        Some(cancellation) => tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(ErrorKind::OtherError(
                "CurseForge fingerprint verification canceled".to_string(),
            ).into()),
            result = file.read(buffer) => Ok(result?),
        },
        None => Ok(file.read(buffer).await?),
    }
}

pub(crate) async fn fingerprint_and_sha1_file(
    path: &Path,
    cancellation: Option<&CancellationToken>,
) -> crate::Result<(u64, String, u32)> {
    #[cfg(test)]
    crate::util::download::verified_file::record_scan();
    let started = std::time::Instant::now();
    let acquire = crate::util::fetch::acquire_native_validation_permit();
    let _permit = if let Some(cancellation) = cancellation {
        tokio::select! { biased; _ = cancellation.cancelled() => return Err(ErrorKind::OtherError("fingerprint verification canceled while waiting for resources".into()).into()), permit = acquire => permit? }
    } else {
        acquire.await?
    };

    use crate::util::download::constants::CURSEFORGE_VERIFY_BUFFER_SIZE as BUFFER_SIZE;
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut sha1 = sha1_smol::Sha1::new();
    let mut size = 0_u64;
    let mut normalized_size = 0_u64;
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        let bytes = &buffer[..read];
        sha1.update(bytes);
        size += read as u64;
        normalized_size += bytes
            .iter()
            .filter(|byte| !is_curseforge_fingerprint_whitespace(**byte))
            .count() as u64;
    }

    let mut file = tokio::fs::File::open(path).await?;
    let mut fingerprint =
        CurseForgeFingerprintHasher::new(normalized_size as u32);
    loop {
        let read = read_curseforge_verification_chunk(
            &mut file,
            &mut buffer,
            cancellation,
        )
        .await?;
        if read == 0 {
            break;
        }
        fingerprint.update(&buffer[..read]);
    }
    tracing::debug!(path = %path.display(), bytes = size, verification_ms = started.elapsed().as_millis(), "Completed CurseForge fingerprint verification");
    Ok((size, sha1.digest().to_string(), fingerprint.finish()))
}

pub(crate) use crate::util::fingerprint::compute_fingerprint;
pub(crate) use crate::util::fingerprint::{
    Murmur2Hasher as CurseForgeFingerprintHasher,
    is_fingerprint_whitespace as is_curseforge_fingerprint_whitespace,
};
