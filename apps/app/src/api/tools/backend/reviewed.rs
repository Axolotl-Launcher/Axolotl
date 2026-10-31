use super::{business_error, resource_history};
use launcher_tools::ToolError;
use sha2::{Digest, Sha256};
use theseus::data::InstanceMetadata;

pub(super) fn linked(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn archive_review_detects_changed_bytes_without_exposing_them() {
        let path = std::env::temp_dir()
            .join(format!("mcp-archive-{}.zip", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"original").await.unwrap();
        let first = archive(&path).await.unwrap();
        tokio::fs::write(&path, b"modified").await.unwrap();
        let second = archive(&path).await.unwrap();
        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
        tokio::fs::remove_file(&path).await.unwrap();
        assert!(archive(&path).await.is_err());
    }
}

pub(super) async fn instance(id: &str) -> Result<InstanceMetadata, ToolError> {
    resource_history::idle(id).await?;
    let metadata = theseus::instance::get(id)
        .await
        .map_err(business_error)?
        .ok_or_else(|| ToolError::new("not_found", "Instance not found"))?;
    if metadata.instance.is_direct_linked()
        || metadata.instance.symlink_target.is_some()
        || metadata.instance.game_dir_override.is_some()
    {
        return Err(ToolError::new(
            "external_instance_layout",
            "Use an ordinary launcher-owned instance for this operation",
        ));
    }
    Ok(metadata)
}

pub(super) fn fingerprint(
    value: &impl serde::Serialize,
) -> Result<String, ToolError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        ToolError::new(
            "serialization_failed",
            "Cannot fingerprint reviewed state",
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) async fn unchanged(
    id: &str,
    expected: &str,
) -> Result<InstanceMetadata, ToolError> {
    let metadata = instance(id).await?;
    if fingerprint(&metadata)? != expected {
        return Err(ToolError::new(
            "stale_instance",
            "Instance changed after review; prepare a fresh plan",
        ));
    }
    Ok(metadata)
}

pub(super) async fn archive(
    path: &std::path::Path,
) -> Result<String, ToolError> {
    use tokio::io::AsyncReadExt;
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|_| {
        ToolError::new("archive_unavailable", "Archive is unavailable")
    })?;
    if !path.is_absolute()
        || !metadata.is_file()
        || linked(&metadata)
        || metadata.len() > 4 * 1024 * 1024 * 1024
    {
        return Err(ToolError::new(
            "invalid_archive",
            "Use an absolute regular archive file up to 4 GiB",
        ));
    }
    let mut file = tokio::fs::File::open(path).await.map_err(|_| {
        ToolError::new("archive_unavailable", "Cannot read archive")
    })?;
    let mut hash = Sha256::new();
    let mut bytes = vec![0; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut bytes).await.map_err(|_| {
            ToolError::new("archive_unavailable", "Cannot read archive")
        })?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > 4 * 1024 * 1024 * 1024 {
            return Err(ToolError::new(
                "archive_changed",
                "Archive grew beyond the size limit",
            ));
        }
        hash.update(&bytes[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
