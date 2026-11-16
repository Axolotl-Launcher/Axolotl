//! Durable prepare/decision/publish. Only a committed coordinator token decides.
use super::manifest::{self, Collections, Head};
use crate::state::persistence::document;
use fs4::tokio::AsyncFileExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Serialize, Deserialize)]
pub(crate) struct Intent {
    pub protocol: String,
    pub schema_version: u32,
    pub expected: u64,
    pub desired: Head,
}
pub(crate) struct Transaction {
    pub root: PathBuf,
    pub head: Head,
    pub collections: Collections,
    pub changed: bool,
    _lock: tokio::fs::File,
}
async fn lock(root: &Path) -> crate::Result<tokio::fs::File> {
    tokio::fs::create_dir_all(root.join("state")).await?;
    let lock = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(root.join("state/transaction.lock"))
        .await?;
    while !lock.try_lock_exclusive()? {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    Ok(lock)
}
pub(crate) async fn initial_lock(
    root: &Path,
) -> crate::Result<tokio::fs::File> {
    lock(root).await
}
impl Transaction {
    pub(crate) async fn begin(
        root: PathBuf,
        committed: Option<String>,
    ) -> crate::Result<Self> {
        let lock = lock(&root).await?;
        if let Some(token) = committed {
            replay(&root, &token).await?;
        }
        let (head, collections) = manifest::load(&root).await?;
        Ok(Self {
            root,
            head,
            collections,
            changed: false,
            _lock: lock,
        })
    }
    pub(crate) async fn prepare(&self) -> crate::Result<Option<String>> {
        if !self.changed {
            return Ok(None);
        }
        if !self.head.extra.is_empty()
            || self
                .head
                .native
                .values()
                .any(|native| !native.extra.is_empty())
            || self.head.shards.iter().any(|shard| !shard.extra.is_empty())
        {
            return Err(crate::ErrorKind::InputError(
                "Newer sync generation metadata retained; mutation paused"
                    .into(),
            )
            .into());
        }
        let token = uuid::Uuid::new_v4().to_string();
        let revision = self.head.revision.checked_add(1).ok_or_else(|| {
            crate::ErrorKind::InputError("Sync revision exhausted".into())
        })?;
        let desired = manifest::prepare(
            &self.root,
            revision,
            token.clone(),
            &self.collections,
            self.head.legacy_extensions_present,
        )
        .await?;
        let intent = Intent {
            protocol: "axolotl-sync-commit".into(),
            schema_version: 1,
            expected: self.head.revision,
            desired,
        };
        let bytes = serde_json::to_vec(&intent)?;
        if bytes.len() > 1 << 20 {
            return Err(crate::ErrorKind::InputError(
                "Sync decision exceeds the recoverable journal size".into(),
            )
            .into());
        }
        document::write_json(
            &self.root.join(format!("state/journals/{token}.json")),
            bytes,
        )
        .await?;
        Ok(Some(token))
    }
    pub(crate) async fn finish(&self, token: &str) -> crate::Result<()> {
        replay(&self.root, token).await
    }
}
pub(crate) async fn replay(root: &Path, token: &str) -> crate::Result<()> {
    uuid::Uuid::parse_str(token)?;
    let bytes = manifest::read_bounded(
        &root.join(format!("state/journals/{token}.json")),
        1 << 20,
    )
    .await?;
    if bytes.len() > 1 << 20 {
        return Err(crate::ErrorKind::InputError(
            "Sync journal exceeds bound".into(),
        )
        .into());
    }
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    if raw.get("protocol").and_then(serde_json::Value::as_str)
        != Some("axolotl-sync-commit")
        || raw
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(1)
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported sync journal retained".into(),
        )
        .into());
    }
    let intent: Intent = serde_json::from_value(raw)?;
    if intent.desired.transaction != token {
        return Err(crate::ErrorKind::InputError(
            "Sync journal decision mismatch".into(),
        )
        .into());
    }
    if intent.expected.checked_add(1) != Some(intent.desired.revision) {
        return Err(crate::ErrorKind::InputError(
            "Invalid sync decision revision".into(),
        )
        .into());
    }
    manifest::publish(root, intent.desired, intent.expected).await
}
