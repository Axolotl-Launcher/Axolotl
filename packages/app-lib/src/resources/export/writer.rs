use super::{
    formats,
    plan::{invalid, snapshot},
};
use crate::resources::*;
use async_zip::{
    Compression, DeflateOption, ZipEntryBuilder, tokio::write::ZipFileWriter,
};
use futures::io::AsyncWriteExt;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

async fn await_output<T, E: std::fmt::Display>(
    cancel: &CancellationToken,
    output: impl std::future::Future<Output = Result<T, E>>,
) -> ResourceResult<T> {
    tokio::select! { biased; _ = cancel.cancelled() => Err(ResourceError::Canceled), result = output => result.map_err(|error| invalid(error.to_string())) }
}

fn canceled(cancel: &CancellationToken) -> ResourceResult<()> {
    if cancel.is_cancelled() {
        Err(ResourceError::Canceled)
    } else {
        Ok(())
    }
}

pub fn temporary_path(
    plan: &ResourceExportPlan,
    output: &Path,
) -> ResourceResult<PathBuf> {
    let id = uuid::Uuid::parse_str(plan.id.as_str())
        .map_err(|e| invalid(e.to_string()))?;
    Ok(output
        .parent()
        .ok_or_else(|| invalid("output has no parent directory"))?
        .join(format!(".axolotl-export-{id}.tmp")))
}

pub async fn cleanup_partial(
    plan: &ResourceExportPlan,
    output: &Path,
) -> ResourceResult<()> {
    let temporary = temporary_path(plan, output)?;
    match tokio::fs::symlink_metadata(&temporary).await {
        Ok(meta) if meta.is_file() && !super::scan::linked(&meta) => {
            tokio::fs::remove_file(&temporary)
                .await
                .map_err(|e| invalid(e.to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(invalid("invalid temporary export file")),
        Err(e) => Err(invalid(e.to_string())),
    }
}

pub async fn validate_output(
    root: &Path,
    plan: &ResourceExportPlan,
    output: &Path,
) -> ResourceResult<PathBuf> {
    let parent = output
        .parent()
        .ok_or_else(|| invalid("output has no parent directory"))?;
    let parent = tokio::fs::canonicalize(parent)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let mut output = parent.join(
        output
            .file_name()
            .ok_or_else(|| invalid("output has no filename"))?,
    );
    if let Ok(meta) = tokio::fs::symlink_metadata(&output).await {
        if !meta.is_file() || super::scan::linked(&meta) {
            return Err(invalid("output must be a regular file"));
        }
        output = tokio::fs::canonicalize(&output)
            .await
            .map_err(|e| invalid(e.to_string()))?;
    }
    for action in &plan.files {
        if tokio::fs::canonicalize(root.join(&snapshot(action).relative_path))
            .await
            .is_ok_and(|source| source == output)
        {
            return Err(invalid(
                "the output cannot replace a selected source file",
            ));
        }
    }
    Ok(output)
}

pub fn work_size(plan: &ResourceExportPlan) -> u64 {
    plan.files.iter().fold(1u64, |total, action| {
        let size = snapshot(action).integrity.size.unwrap_or(0);
        total.saturating_add(size).saturating_add(
            if matches!(action, ExportFileAction::Embed { .. }) {
                size
            } else {
                0
            },
        )
    })
}

async fn checked_metadata(
    root: &Path,
    root_canonical: &Path,
    snapshot: &ExportFileSnapshot,
) -> ResourceResult<std::fs::Metadata> {
    let safe = path_util::SafeRelativeUtf8UnixPathBuf::try_from(
        snapshot.relative_path.clone(),
    )
    .map_err(|e| invalid(e.to_string()))?;
    if super::categories::forbidden(safe.as_str()) {
        return Err(invalid("protected export path"));
    }
    let mut path = root.to_path_buf();
    for segment in safe.as_str().split('/') {
        path.push(segment);
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(|e| invalid(e.to_string()))?;
        if super::scan::linked(&metadata) {
            return Err(invalid(
                "export path contains a symbolic link or junction",
            ));
        }
    }
    if !tokio::fs::canonicalize(&path)
        .await
        .map_err(|e| invalid(e.to_string()))?
        .starts_with(root_canonical)
    {
        return Err(invalid("export file escaped the game directory"));
    }
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    if !metadata.is_file() || snapshot.integrity.size != Some(metadata.len()) {
        return Err(invalid(format!(
            "{} changed after export analysis; refresh again",
            snapshot.relative_path
        )));
    }
    Ok(metadata)
}

fn same_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn matches_bytes(
    snapshot: &ExportFileSnapshot,
    size: u64,
    hasher: Sha256,
) -> bool {
    let hash = format!("{:x}", hasher.finalize());
    snapshot.integrity.size == Some(size)
        && snapshot.integrity.hashes.iter().any(|h| {
            h.algorithm == HashAlgorithm::Sha256
                && h.value.eq_ignore_ascii_case(&hash)
        })
}

async fn verify_files(
    root: &Path,
    root_canonical: &Path,
    plan: &ResourceExportPlan,
    initial: &[std::fs::Metadata],
    cancel: &CancellationToken,
    completed: &mut u64,
    total: u64,
    progress: &(impl Fn(u64, u64) + Send + Sync),
) -> ResourceResult<()> {
    let mut buffer = vec![0; 256 * 1024];
    for (action, initial) in plan.files.iter().zip(initial) {
        canceled(cancel)?;
        let snapshot = snapshot(action);
        let before = checked_metadata(root, root_canonical, snapshot).await?;
        if !same_metadata(&before, initial) {
            return Err(invalid(format!(
                "{} changed during export; refresh again",
                snapshot.relative_path
            )));
        }
        let mut file = super::files::open_async(root, &snapshot.relative_path)
            .await
            .map_err(|e| invalid(e.to_string()))?;
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        loop {
            let read = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), r = file.read(&mut buffer) => r.map_err(|e| invalid(e.to_string()))? };
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            size += read as u64;
            if size > before.len() {
                return Err(invalid(format!(
                    "{} grew during export; refresh again",
                    snapshot.relative_path
                )));
            }
            *completed = completed.saturating_add(read as u64);
            progress(*completed, total);
        }
        if !matches_bytes(snapshot, size, hasher)
            || !same_metadata(
                &before,
                &file.metadata().await.map_err(|e| invalid(e.to_string()))?,
            )
        {
            return Err(invalid(format!(
                "{} changed after export analysis; refresh again",
                snapshot.relative_path
            )));
        }
    }
    for (action, initial) in plan.files.iter().zip(initial) {
        canceled(cancel)?;
        if !same_metadata(
            initial,
            &checked_metadata(root, root_canonical, snapshot(action)).await?,
        ) {
            return Err(invalid(format!(
                "{} changed during export; refresh again",
                snapshot(action).relative_path
            )));
        }
    }
    Ok(())
}

/// Writes only frozen actions, checks the bytes written, then atomically publishes.
/// The temporary file guard removes incomplete output on failure or cancellation.
pub async fn write(
    root: &Path,
    plan: &ResourceExportPlan,
    output: &Path,
    cancel: &CancellationToken,
    progress: &(impl Fn(u64, u64) + Send + Sync),
) -> ResourceResult<()> {
    write_checked(root, plan, output, cancel, progress, || async { Ok(()) })
        .await
}

pub async fn write_checked<F, Fut>(
    root: &Path,
    plan: &ResourceExportPlan,
    output: &Path,
    cancel: &CancellationToken,
    progress: &(impl Fn(u64, u64) + Send + Sync),
    validate: F,
) -> ResourceResult<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ResourceResult<()>>,
{
    validate().await?;
    if plan.package_info.name.trim().is_empty()
        || plan.package_info.version.trim().is_empty()
        || plan.requirements.game_version.is_empty()
    {
        return Err(invalid("invalid export package information"));
    }
    let (manifest_name, manifest) = formats::manifest(plan)?;
    let output = validate_output(root, plan, output).await?;
    let started = std::time::Instant::now();
    let total = work_size(plan);
    progress(0, total);
    let root_canonical = tokio::fs::canonicalize(root)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let mut initial = Vec::with_capacity(plan.files.len());
    let mut paths = HashSet::new();
    for action in &plan.files {
        canceled(cancel)?;
        if !paths.insert(&snapshot(action).relative_path) {
            return Err(invalid("duplicate export path"));
        }
        if !snapshot(action)
            .integrity
            .hashes
            .iter()
            .any(|h| h.algorithm == HashAlgorithm::Sha256)
        {
            return Err(invalid("export snapshot has no SHA-256 hash"));
        }
        initial.push(
            checked_metadata(root, &root_canonical, snapshot(action)).await?,
        );
    }
    canceled(cancel)?;
    let temporary_file = temporary_path(plan, &output)?;
    if plan
        .files
        .iter()
        .any(|a| root.join(&snapshot(a).relative_path) == temporary_file)
    {
        return Err(invalid("temporary output is part of the selected files"));
    }
    let opened = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_file)
        .await
        .map_err(|e| invalid(e.to_string()))?;
    let temporary = tempfile::TempPath::try_from_path(temporary_file)
        .map_err(|e| invalid(e.to_string()))?;
    let mut file = opened;
    let mut zip = ZipFileWriter::with_tokio(&mut file);
    let mut completed = 0;
    let mut buffer = vec![0; 256 * 1024];
    for action in &plan.files {
        canceled(cancel)?;
        let ExportFileAction::Embed { snapshot } = action else {
            continue;
        };
        let path = root.join(&snapshot.relative_path);
        let compression = match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "jar" | "zip" | "rar" | "7z" | "png" | "jpg" | "jpeg" | "ogg"
            | "mp4" | "gz" => Compression::Stored,
            _ => Compression::Deflate,
        };
        let builder = ZipEntryBuilder::new(
            format!("overrides/{}", snapshot.relative_path).into(),
            compression,
        )
        .deflate_option(DeflateOption::Other(1));
        let mut entry =
            await_output(cancel, zip.write_entry_stream(builder)).await?;
        let mut input = super::files::open_async(root, &snapshot.relative_path)
            .await
            .map_err(|e| invalid(e.to_string()))?;
        let mut sha256 = Sha256::new();
        let mut size = 0;
        loop {
            let read = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), r = input.read(&mut buffer) => r.map_err(|e| invalid(e.to_string()))? };
            if read == 0 {
                break;
            }
            await_output(cancel, entry.write_all(&buffer[..read])).await?;
            sha256.update(&buffer[..read]);
            size += read as u64;
            if snapshot
                .integrity
                .size
                .is_none_or(|expected| size > expected)
            {
                return Err(invalid(format!(
                    "{} grew during export; refresh again",
                    snapshot.relative_path
                )));
            }
            completed += read as u64;
            progress(completed, total);
        }
        if !matches_bytes(snapshot, size, sha256) {
            return Err(invalid(format!(
                "{} changed while being exported; refresh again",
                snapshot.relative_path
            )));
        }
        await_output(cancel, entry.close()).await?;
    }
    canceled(cancel)?;
    let manifest = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| invalid(e.to_string()))?;
    await_output(
        cancel,
        zip.write_entry_whole(
            ZipEntryBuilder::new(manifest_name.into(), Compression::Deflate),
            &manifest,
        ),
    )
    .await?;
    await_output(cancel, zip.close()).await?;
    await_output(cancel, file.sync_all()).await?;
    drop(file);
    tracing::debug!(
        plan_id = plan.id.as_str(),
        elapsed_ms = started.elapsed().as_millis(),
        "Finished writing export entries; verifying frozen bytes"
    );
    verify_files(
        root,
        &root_canonical,
        plan,
        &initial,
        cancel,
        &mut completed,
        total,
        progress,
    )
    .await?;
    validate_output(root, plan, &output).await?;
    validate().await?;
    canceled(cancel)?;
    temporary
        .persist(&output)
        .map_err(|e| invalid(e.to_string()))?;
    progress(total, total);
    tracing::debug!(
        plan_id = plan.id.as_str(),
        bytes_processed = completed,
        elapsed_ms = started.elapsed().as_millis(),
        "Published verified export archive"
    );
    Ok(())
}

#[cfg(test)]
mod output_tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_interrupts_a_blocked_target_write_and_removes_temporary_output()
     {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("out.zip");
        std::fs::write(&output, b"previous archive").unwrap();
        let temporary = root.path().join("out.zip.tmp");
        std::fs::write(&temporary, b"partial archive").unwrap();
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        let task = tokio::spawn(async move {
            let _temporary =
                tempfile::TempPath::try_from_path(temporary).unwrap();
            await_output(
                &cancel,
                std::future::pending::<Result<(), std::io::Error>>(),
            )
            .await
        });
        trigger.cancel();
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Err(ResourceError::Canceled)
        ));
        assert!(!root.path().join("out.zip.tmp").exists());
        assert_eq!(std::fs::read(output).unwrap(), b"previous archive");
    }
}
