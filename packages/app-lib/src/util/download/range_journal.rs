//! Validated checkpoints for sparse range downloads.

use crate::util::download::constants::{
    DOWNLOAD_BUFFER_BYTES, H2_RANGE_MAX_WORKERS, RANGE_JOURNAL_MAX_BYTES,
    RANGE_JOURNAL_RANGE_CHECKPOINT_EXCEEDS_ITS_BOUNDS, RANGE_JOURNAL_SUFFIX,
};
use crate::util::fetch::{self, Integrity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Checkpoint {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) written: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct Journal {
    version: u32,
    size: u64,
    integrity: Integrity,
    ranges: Vec<Checkpoint>,
}

pub(crate) struct RangeJournal {
    path: PathBuf,
    state: Mutex<Journal>,
    resumable: bool,
    prefix_hashers: Mutex<HashMap<usize, Sha256>>,
}

pub(crate) fn path(part: &Path) -> PathBuf {
    fetch::suffixed_path(part, RANGE_JOURNAL_SUFFIX)
}

impl Journal {
    fn valid(&self, integrity: &Integrity, size: u64) -> bool {
        if self.version != 1
            || self.integrity != *integrity
            || self.size != size
            || self.ranges.is_empty()
            || self.ranges.len() > 8
        {
            return false;
        }
        let mut offset = 0;
        for range in &self.ranges {
            if range.start != offset
                || range.end <= range.start
                || range.end > size
                || range.written > range.end - range.start
                || range.written > 0
                    && (range.sha256.len() != 64
                        || !range
                            .sha256
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit()))
            {
                return false;
            }
            offset = range.end;
        }
        offset == size
    }
}

impl RangeJournal {
    pub(crate) async fn load(
        part: &Path,
        integrity: &Integrity,
        size: u64,
    ) -> crate::Result<Option<Arc<Self>>> {
        if !integrity.supports_resume() {
            return Ok(None);
        }
        let journal_path = path(part);
        let metadata = match tokio::fs::metadata(&journal_path).await {
            Ok(metadata) if metadata.len() <= RANGE_JOURNAL_MAX_BYTES => {
                metadata
            }
            _ => return Ok(None),
        };
        if !metadata.is_file() {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        tokio::fs::File::open(&journal_path)
            .await?
            .take(RANGE_JOURNAL_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > RANGE_JOURNAL_MAX_BYTES as usize {
            return Ok(None);
        }
        let mut state = match serde_json::from_slice::<Journal>(&bytes) {
            Ok(state) if state.valid(integrity, size) => state,
            _ => return Ok(None),
        };
        if !tokio::fs::metadata(part)
            .await
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == size)
        {
            return Ok(None);
        }
        let mut prefix_hashers = HashMap::new();
        for (index, range) in state.ranges.iter_mut().enumerate() {
            if range.written > 0 {
                let hasher = hash_prefix(part, range).await?;
                if format!("{:x}", hasher.clone().finalize()) != range.sha256 {
                    range.written = 0;
                    range.sha256.clear();
                } else {
                    prefix_hashers.insert(index, hasher);
                }
            }
        }
        Ok(Some(Arc::new(Self {
            path: journal_path,
            state: Mutex::new(state),
            resumable: true,
            prefix_hashers: Mutex::new(prefix_hashers),
        })))
    }

    pub(crate) fn new(
        part: &Path,
        integrity: &Integrity,
        size: u64,
        count: usize,
    ) -> Arc<Self> {
        let count = count
            .clamp(1, H2_RANGE_MAX_WORKERS)
            .min(usize::try_from(size.max(1)).unwrap_or(usize::MAX));
        let base = size / count as u64;
        let remainder = size % count as u64;
        let mut start = 0;
        let ranges = (0..count)
            .map(|index| {
                let end = start + base + u64::from((index as u64) < remainder);
                let checkpoint = Checkpoint {
                    start,
                    end,
                    written: 0,
                    sha256: String::new(),
                };
                start = end;
                checkpoint
            })
            .collect();
        Arc::new(Self {
            path: path(part),
            state: Mutex::new(Journal {
                version: 1,
                size,
                integrity: integrity.clone(),
                ranges,
            }),
            resumable: integrity.supports_resume(),
            prefix_hashers: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn hash_prefix(
        &self,
        index: usize,
        part: &Path,
        checkpoint: &Checkpoint,
    ) -> crate::Result<Sha256> {
        if let Some(hasher) = self.prefix_hashers.lock().await.remove(&index) {
            return Ok(hasher);
        }
        hash_prefix(part, checkpoint).await
    }

    pub(crate) async fn ranges(&self) -> Vec<Checkpoint> {
        self.state.lock().await.ranges.clone()
    }

    pub(crate) async fn checkpoint(
        &self,
        index: usize,
        written: u64,
        hasher: &Sha256,
    ) -> crate::Result<()> {
        let mut state = self.state.lock().await;
        let range = &mut state.ranges[index];
        if written > range.end - range.start {
            return Err(crate::ErrorKind::InputError(
                RANGE_JOURNAL_RANGE_CHECKPOINT_EXCEEDS_ITS_BOUNDS.into(),
            )
            .into());
        }
        range.written = written;
        range.sha256 = format!("{:x}", hasher.clone().finalize());
        if self.resumable {
            self.save(&state).await?;
        }
        Ok(())
    }

    pub(crate) async fn persist(&self) -> crate::Result<()> {
        if self.resumable {
            self.save(&*self.state.lock().await).await?;
        }
        Ok(())
    }

    async fn save(&self, state: &Journal) -> crate::Result<()> {
        let temporary = fetch::suffixed_path(
            &self.path,
            &format!(".{}", uuid::Uuid::new_v4()),
        );
        let bytes = serde_json::to_vec(state)?;
        let result = async {
            super::local_resources::write(
                &temporary,
                bytes.len() as u64,
                tokio::fs::write(&temporary, bytes),
            )
            .await?;
            fetch::finalize_download(&temporary, &self.path).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }
}

pub(crate) async fn hash_prefix(
    part: &Path,
    checkpoint: &Checkpoint,
) -> crate::Result<Sha256> {
    let mut hasher = Sha256::new();
    if checkpoint.written == 0 {
        return Ok(hasher);
    }
    let _permit = fetch::acquire_native_validation_permit().await?;
    let mut file = tokio::fs::File::open(part).await?;
    file.seek(std::io::SeekFrom::Start(checkpoint.start))
        .await?;
    let mut remaining = checkpoint.written;
    let mut buffer = vec![0; DOWNLOAD_BUFFER_BYTES];
    while remaining > 0 {
        let wanted = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..wanted]).await?;
        hasher.update(&buffer[..wanted]);
        remaining -= wanted as u64;
    }
    Ok(hasher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completed_ranges_survive_restart_and_corruption_is_local() {
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("artifact.part");
        tokio::fs::write(&part, b"abcdefghijkl").await.unwrap();
        let integrity = Integrity::sha1("identity").with_size(12);
        let journal = RangeJournal::new(&part, &integrity, 12, 3);
        journal
            .checkpoint(0, 4, &Sha256::new_with_prefix(b"abcd"))
            .await
            .unwrap();
        journal
            .checkpoint(1, 2, &Sha256::new_with_prefix(b"ef"))
            .await
            .unwrap();
        let restored = RangeJournal::load(&part, &integrity, 12)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            restored
                .ranges()
                .await
                .iter()
                .map(|range| range.written)
                .collect::<Vec<_>>(),
            vec![4, 2, 0]
        );
        tokio::fs::write(&part, b"abcdXXghijkl").await.unwrap();
        let restored = RangeJournal::load(&part, &integrity, 12)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            restored
                .ranges()
                .await
                .iter()
                .map(|range| range.written)
                .collect::<Vec<_>>(),
            vec![4, 0, 0]
        );
        assert!(
            RangeJournal::load(
                &part,
                &Integrity::sha1("other").with_size(12),
                12
            )
            .await
            .unwrap()
            .is_none()
        );
    }

    #[tokio::test]
    async fn malformed_journal_never_reuses_sparse_output() {
        let directory = tempfile::tempdir().unwrap();
        let part = directory.path().join("artifact.part");
        tokio::fs::write(&part, b"1234").await.unwrap();
        tokio::fs::write(path(&part), br#"{"version":1,"size":4,"ranges":[]}"#)
            .await
            .unwrap();
        assert!(
            RangeJournal::load(&part, &Integrity::sha1("hash").with_size(4), 4)
                .await
                .unwrap()
                .is_none()
        );
    }
}
