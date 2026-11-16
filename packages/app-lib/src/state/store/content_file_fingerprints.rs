//! File byte evidence with a trustworthy filesystem attribute baseline.

use crate::resources::{ContentHash, HashAlgorithm};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use sqlx::SqliteConnection;

#[cfg(test)]
use super::Collection;

pub(crate) struct ContentFileFingerprints;

impl ContentFileFingerprints {
    #[cfg(test)]
    pub(crate) async fn set(
        handle: &super::Handle,
        key: &FingerprintKey,
        record: Option<&FileFingerprintRecord>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            super::content_evidence::remove(
                handle,
                "fingerprint",
                &key.instance_id,
                &key.file_id,
            )
            .await;
            return Ok(());
        };
        let guards =
            super::content_evidence::current(handle, &key.instance_id).await?;
        let guard = guards.get(&key.file_id);
        if let Some(guard) = guard {
            super::content_evidence::write(
                handle,
                "fingerprint",
                &key.file_id,
                guard,
                record,
            )
            .await;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn set_scoped(
        handle: &super::Handle,
        instance: &String,
        records: &[(FingerprintKey, FileFingerprintRecord)],
    ) -> crate::Result<()> {
        for (key, _) in Self::get_scoped(handle, instance).await? {
            Self::set(handle, &key, None).await?;
        }
        for (key, record) in records {
            Self::set(handle, key, Some(record)).await?;
        }
        Ok(())
    }

    pub(crate) async fn get(
        handle: &super::Handle,
        key: &FingerprintKey,
    ) -> crate::Result<Option<FileFingerprintRecord>> {
        let guards =
            super::content_evidence::current(handle, &key.instance_id).await?;
        let Some(guard) = guards.get(&key.file_id) else {
            return Ok(None);
        };
        Ok(super::content_evidence::read(
            handle,
            "fingerprint",
            &key.instance_id,
            &key.file_id,
            guard,
        )
        .await)
    }
    pub(crate) async fn get_scoped(
        handle: &super::Handle,
        instance: &String,
    ) -> crate::Result<Vec<(FingerprintKey, FileFingerprintRecord)>> {
        let guards = super::content_evidence::current(handle, instance).await?;
        Ok(super::content_evidence::read_scoped(
            handle,
            "fingerprint",
            instance,
            &guards,
            None,
        )
        .await
        .into_iter()
        .map(|(file_id, record)| {
            (
                FingerprintKey {
                    instance_id: instance.clone(),
                    file_id,
                },
                record,
            )
        })
        .collect())
    }
    pub(crate) async fn set_current(
        handle: &super::Handle,
        instance: &str,
        record: &FileFingerprintRecord,
    ) -> crate::Result<()> {
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid fingerprint".into(),
            )
            .into());
        }
        let guards = super::content_evidence::current(handle, instance).await?;
        if let Some(guard) = guards.values().find(|guard| {
            guard.relative_path == record.relative_path
                && guard.sha1.eq_ignore_ascii_case(record.sha1())
                && u64::try_from(guard.size).ok() == Some(record.size)
        }) {
            super::content_evidence::write(
                handle,
                "fingerprint",
                &guard.file_id,
                guard,
                record,
            )
            .await;
        }
        Ok(())
    }
    pub(crate) async fn get_current(
        handle: &super::Handle,
        instance: &str,
        path: &str,
    ) -> crate::Result<Option<FileFingerprintRecord>> {
        let guards = super::content_evidence::current(handle, instance).await?;
        let Some(guard) =
            guards.values().find(|guard| guard.relative_path == path)
        else {
            return Ok(None);
        };
        Ok(super::content_evidence::read(
            handle,
            "fingerprint",
            instance,
            &guard.file_id,
            guard,
        )
        .await)
    }
    pub(crate) async fn transfer(
        handle: &super::Handle,
        instance: &str,
        original: &str,
        destination: &str,
        root: &std::path::Path,
        integrity: &crate::resources::ArtifactIntegrity,
    ) -> crate::Result<()> {
        let guards = super::content_evidence::current(handle, instance).await?;
        if let Some(guard) = guards
            .values()
            .find(|guard| guard.relative_path == destination)
        {
            if let Some(mut record) =
                super::content_evidence::read::<FileFingerprintRecord>(
                    handle,
                    "fingerprint",
                    instance,
                    &guard.file_id,
                    guard,
                )
                .await
            {
                if record.relative_path == original
                    && record.content_root == root.to_string_lossy()
                    && integrity.size == Some(record.size)
                    && integrity.hashes.iter().any(|hash| {
                        hash.algorithm == HashAlgorithm::Sha1
                            && hash.value.eq_ignore_ascii_case(record.sha1())
                    })
                {
                    record.relative_path = destination.into();
                    super::content_evidence::write(
                        handle,
                        "fingerprint",
                        &guard.file_id,
                        guard,
                        &record,
                    )
                    .await;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FingerprintKey {
    pub instance_id: String,
    pub file_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileModifiedTime {
    pub seconds: i64,
    pub nanos: u32,
}

impl FileModifiedTime {
    pub(crate) fn from_system_time(
        time: std::time::SystemTime,
    ) -> Option<Self> {
        match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => Some(Self {
                seconds: duration.as_secs().try_into().ok()?,
                nanos: duration.subsec_nanos(),
            }),
            Err(error) => {
                let duration = error.duration();
                let seconds =
                    i64::try_from(duration.as_secs()).ok()?.checked_neg()?;
                let nanos = duration.subsec_nanos();
                Some(Self {
                    seconds: if nanos == 0 {
                        seconds
                    } else {
                        seconds.checked_sub(1)?
                    },
                    nanos: if nanos == 0 { 0 } else { 1_000_000_000 - nanos },
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileFingerprintRecord {
    pub content_root: String,
    pub relative_path: String,
    pub size: u64,
    pub modified: FileModifiedTime,
    pub hashes: Vec<ContentHash>,
    pub curseforge_fingerprint: Option<u32>,
}

impl FileFingerprintRecord {
    pub(crate) fn sha1(&self) -> &str {
        self.hashes
            .iter()
            .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
            .map(|hash| hash.value.as_str())
            .unwrap_or_default()
    }

    pub(crate) fn matches(
        &self,
        root: &str,
        path: &str,
        size: u64,
        modified: Option<&FileModifiedTime>,
    ) -> bool {
        self.content_root == root
            && self.relative_path == path
            && self.size == size
            && modified == Some(&self.modified)
    }

    fn valid(&self) -> bool {
        !self.content_root.is_empty()
            && !self.relative_path.is_empty()
            && self.modified.nanos < 1_000_000_000
            && self.size <= i64::MAX as u64
            && self
                .hashes
                .iter()
                .any(|hash| hash.algorithm == HashAlgorithm::Sha1)
            && self.hashes.iter().enumerate().all(|(index, hash)| {
                let length = match hash.algorithm {
                    HashAlgorithm::Sha1 => 40,
                    HashAlgorithm::Sha256 => 64,
                    HashAlgorithm::Sha512 => 128,
                    HashAlgorithm::Md5 => 32,
                };
                hash.value.len() == length
                    && hash.value.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && !self.hashes[..index]
                        .iter()
                        .any(|previous| previous.algorithm == hash.algorithm)
            })
    }
}

#[cfg(test)]
impl Collection for ContentFileFingerprints {
    type Key = FingerprintKey;
    type Scope = String;
    type Record = FileFingerprintRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == &key.instance_id
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM content_file_fingerprints WHERE instance_id = ? AND file_id = ?"
        ).bind(&key.instance_id).bind(&key.file_id).fetch_optional(&mut *conn).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM content_file_fingerprints WHERE instance_id = ? AND file_id = ?")
                .bind(&key.instance_id).bind(&key.file_id).execute(&mut *conn).await?;
            return Ok(());
        };
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid content_file_fingerprints record".into(),
            )
            .into());
        }
        sqlx::query("INSERT INTO content_file_fingerprints (instance_id, file_id, payload) VALUES (?, ?, ?) ON CONFLICT (instance_id, file_id) DO UPDATE SET payload = excluded.payload")
            .bind(&key.instance_id).bind(&key.file_id).bind(serde_json::to_string(record)?)
            .execute(&mut *conn).await?;
        Ok(())
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT instance_id, file_id, payload FROM content_file_fingerprints"
        ).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, file_id, payload)| {
                Some((
                    FingerprintKey {
                        instance_id,
                        file_id,
                    },
                    decode(&payload)?,
                ))
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM content_file_fingerprints")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT file_id, payload FROM content_file_fingerprints WHERE instance_id = ?"
        ).bind(scope).fetch_all(&mut *conn).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(file_id, payload)| {
                Some((
                    FingerprintKey {
                        instance_id: scope.clone(),
                        file_id,
                    },
                    decode(&payload)?,
                ))
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query(
            "DELETE FROM content_file_fingerprints WHERE instance_id = ?",
        )
        .bind(scope)
        .execute(&mut *conn)
        .await?;
        for (key, record) in records {
            Self::write(conn, key, Some(record)).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
fn decode(payload: &str) -> Option<FileFingerprintRecord> {
    match serde_json::from_str::<FileFingerprintRecord>(payload) {
        Ok(record) if record.valid() => Some(record),
        _ => {
            tracing::warn!(
                "Skipping an invalid content_file_fingerprints record"
            );
            None
        }
    }
}
