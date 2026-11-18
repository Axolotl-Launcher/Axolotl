//! Bounded immutable recovery records and exact backup reconstruction.
use crate::state::persistence::document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
const SHARD_BYTES: usize = 1024 * 1024;
const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    name: String,
    bytes: usize,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    protocol: String,
    schema_version: u32,
    kind: String,
    key: String,
    instance_id: Option<String>,
    revision: i64,
    parts: Vec<Part>,
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn invalid(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}
fn previous(path: &Path) -> std::path::PathBuf {
    let mut path = path.as_os_str().to_owned();
    path.push(".previous");
    path.into()
}
pub(crate) async fn write(
    path: &Path,
    kind: &str,
    key: &str,
    instance_id: Option<&str>,
    revision: i64,
    value: Value,
) -> crate::Result<String> {
    let bytes = serde_json::to_vec(&value)?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid("Recovery record exceeds its size limit"));
    }
    let folder = path
        .parent()
        .ok_or_else(|| invalid("Recovery path has no parent"))?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| invalid("Recovery filename is not UTF-8"))?;
    let mut parts = Vec::new();
    for (index, bytes) in bytes.chunks(SHARD_BYTES).enumerate() {
        let name = format!("{stem}-{index}.part");
        let target = folder.join(&name);
        document::write_blob(&previous(&target), bytes.to_vec()).await?;
        document::write_blob(&target, bytes.to_vec()).await?;
        parts.push(Part {
            name,
            bytes: bytes.len(),
            sha256: digest(bytes),
        });
    }
    let envelope = serde_json::to_vec(&Envelope {
        protocol: "axolotl-operation".into(),
        schema_version: 1,
        kind: kind.into(),
        key: key.into(),
        instance_id: instance_id.map(str::to_owned),
        revision,
        parts,
    })?;
    let hash = digest(&envelope);
    document::write_blob(&previous(path), envelope.clone()).await?;
    document::write_blob(path, envelope).await?;
    Ok(hash)
}
async fn exact(
    path: &Path,
    hash: &str,
    limit: usize,
) -> crate::Result<Vec<u8>> {
    if let Ok(bytes) = bounded(path, limit).await
        && digest(&bytes) == hash
    {
        return Ok(bytes);
    }
    let bytes = bounded(&previous(path), limit).await?;
    if digest(&bytes) != hash {
        return Err(invalid(
            "Recovery record and backup failed integrity validation",
        ));
    }
    Ok(bytes)
}
pub(crate) async fn read(
    path: &Path,
    hash: &str,
    kind: &str,
    key: &str,
    instance_id: Option<&str>,
    revision: i64,
) -> crate::Result<Value> {
    let bytes = exact(path, hash, 64 * 1024).await?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    if envelope.protocol != "axolotl-operation"
        || envelope.schema_version != 1
        || envelope.kind != kind
        || envelope.key != key
        || envelope.instance_id.as_deref() != instance_id
        || envelope.revision != revision
        || envelope.parts.len() > MAX_BYTES / SHARD_BYTES
    {
        return Err(invalid(
            "Unsupported or mismatched operation envelope retained",
        ));
    }
    let folder = path
        .parent()
        .ok_or_else(|| invalid("Recovery envelope has no parent"))?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| invalid("Recovery envelope filename invalid"))?;
    let mut payload = Vec::new();
    for (index, part) in envelope.parts.into_iter().enumerate() {
        if part.name != format!("{stem}-{index}.part")
            || part.bytes > SHARD_BYTES
            || payload.len().saturating_add(part.bytes) > MAX_BYTES
        {
            return Err(invalid(
                "Recovery shard escapes its operation or size bound",
            ));
        }
        let bytes =
            exact(&folder.join(part.name), &part.sha256, part.bytes).await?;
        if bytes.len() != part.bytes {
            return Err(invalid("Recovery shard size mismatch"));
        }
        payload.extend_from_slice(&bytes);
    }
    Ok(serde_json::from_slice(&payload)?)
}

/// Preserve an exact immutable generation without interpreting its payload.
/// The destination keeps the filename because shard names belong to it.
pub(crate) async fn copy_exact(
    path: &Path,
    destination: &Path,
    hash: &str,
    kind: &str,
    key: &str,
    instance_id: Option<&str>,
    revision: i64,
) -> crate::Result<()> {
    let bytes = exact(path, hash, 64 * 1024).await?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    if envelope.protocol != "axolotl-operation"
        || envelope.schema_version != 1
        || envelope.kind != kind
        || envelope.key != key
        || envelope.instance_id.as_deref() != instance_id
        || envelope.revision != revision
        || envelope.parts.len() > MAX_BYTES / SHARD_BYTES
        || path.file_name() != destination.file_name()
    {
        return Err(invalid(
            "Unsupported operation cannot be relocated safely",
        ));
    }
    let folder = path
        .parent()
        .ok_or_else(|| invalid("Operation has no parent"))?;
    let target = destination
        .parent()
        .ok_or_else(|| invalid("Operation destination has no parent"))?;
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| invalid("Invalid operation filename"))?;
    let mut total = 0usize;
    for (index, part) in envelope.parts.iter().enumerate() {
        total = total.saturating_add(part.bytes);
        if part.name != format!("{stem}-{index}.part")
            || part.bytes > SHARD_BYTES
            || total > MAX_BYTES
        {
            return Err(invalid("Operation shard cannot be relocated safely"));
        }
        let payload =
            exact(&folder.join(&part.name), &part.sha256, part.bytes).await?;
        if payload.len() != part.bytes {
            return Err(invalid("Operation shard size mismatch"));
        }
        document::write_blob(
            &previous(&target.join(&part.name)),
            payload.clone(),
        )
        .await?;
        document::write_blob(&target.join(&part.name), payload).await?;
    }
    document::write_blob(&previous(destination), bytes.clone()).await?;
    document::write_blob(destination, bytes).await
}
pub(crate) async fn bounded(
    path: &Path,
    limit: usize,
) -> crate::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > limit as u64
    {
        return Err(invalid(
            "Recovery file exceeds its bound or is not a regular file",
        ));
    }
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit as u64 + 1).read_to_end(&mut bytes).await?;
    if bytes.len() > limit {
        return Err(invalid("Recovery file grew beyond its bound"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn corrupt_primary_recovers_exact_backup_without_removing_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("record.json");
        let hash = write(
            &path,
            "moves",
            "operation",
            Some("instance"),
            4,
            json!({"path":"mods/a.jar"}),
        )
        .await
        .unwrap();
        tokio::fs::write(&path, b"broken original").await.unwrap();
        assert_eq!(
            read(&path, &hash, "moves", "operation", Some("instance"), 4)
                .await
                .unwrap()["path"],
            "mods/a.jar"
        );
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"broken original");
    }
    #[tokio::test]
    async fn future_envelope_and_wrong_owner_cannot_authorize_replay() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("record.json");
        let hash =
            write(&path, "moves", "operation", Some("first"), 4, json!({}))
                .await
                .unwrap();
        assert!(
            read(&path, &hash, "moves", "operation", Some("second"), 4)
                .await
                .is_err()
        );
        let mut future: Value =
            serde_json::from_slice(&tokio::fs::read(&path).await.unwrap())
                .unwrap();
        future["futureReplayMode"] = json!(true);
        let bytes = serde_json::to_vec(&future).unwrap();
        tokio::fs::write(&path, &bytes).await.unwrap();
        assert!(
            read(
                &path,
                &digest(&bytes),
                "moves",
                "operation",
                Some("first"),
                4
            )
            .await
            .is_err()
        );
    }
    #[tokio::test]
    async fn oversized_envelope_is_rejected_before_allocation_and_bad_record_is_local()
     {
        let directory = tempfile::tempdir().unwrap();
        let bad = directory.path().join("bad.json");
        let good = directory.path().join("good.json");
        tokio::fs::write(&bad, vec![0; 65 * 1024]).await.unwrap();
        assert!(bounded(&bad, 64 * 1024).await.is_err());
        let hash =
            write(&good, "moves", "other", None, 1, json!({"safe":true}))
                .await
                .unwrap();
        assert_eq!(
            read(&good, &hash, "moves", "other", None, 1).await.unwrap()["safe"],
            true
        );
    }
}
