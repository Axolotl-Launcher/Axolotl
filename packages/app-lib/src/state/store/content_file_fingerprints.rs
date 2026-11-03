//! File byte evidence with a trustworthy filesystem attribute baseline.

use crate::resources::{ContentHash, HashAlgorithm};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use super::Collection;

pub(crate) struct ContentFileFingerprints;

impl ContentFileFingerprints {
    pub(crate) async fn transfer(
        handle: &super::Handle,
        instance_id: &str,
        original: &str,
        destination: &str,
        root: &std::path::Path,
        integrity: &crate::resources::ArtifactIntegrity,
    ) -> crate::Result<()> {
        let mut unit = super::atom(handle).await?;
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT file.id, fingerprint.payload FROM instance_files file
             JOIN content_file_fingerprints fingerprint ON fingerprint.instance_id = file.instance_id AND fingerprint.file_id = file.id
             WHERE file.instance_id = ? AND file.relative_path = ? AND file.missing = 0"
        ).bind(instance_id).bind(destination).fetch_optional(unit.connection()).await?;
        if let Some((file_id, payload)) = row
            && let Some(mut record) = decode(&payload)
            && record.relative_path == original
            && record.content_root == root.to_string_lossy()
            && integrity.size == Some(record.size)
            && integrity.hashes.iter().any(|hash| {
                hash.algorithm == HashAlgorithm::Sha1
                    && hash.value.eq_ignore_ascii_case(record.sha1())
            })
        {
            record.relative_path = destination.into();
            Self::atom_set(
                &mut unit,
                &FingerprintKey {
                    instance_id: instance_id.into(),
                    file_id,
                },
                Some(&record),
            )
            .await?;
        }
        unit.commit().await
    }

    pub(crate) async fn get_current(
        handle: &super::Handle,
        instance_id: &str,
        relative_path: &str,
    ) -> crate::Result<Option<FileFingerprintRecord>> {
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT fingerprint.payload FROM content_file_fingerprints fingerprint
             JOIN instance_files file ON file.instance_id = fingerprint.instance_id AND file.id = fingerprint.file_id
             WHERE file.instance_id = ? AND file.relative_path = ? AND file.missing = 0"
        ).bind(instance_id).bind(relative_path).fetch_optional(handle.pool()).await?;
        Ok(payload.and_then(|payload| decode(&payload)))
    }

    /// Recognition can finish after another operation replaced the tracked file.
    pub(crate) async fn set_current(
        handle: &super::Handle,
        instance_id: &str,
        record: &FileFingerprintRecord,
    ) -> crate::Result<()> {
        if !record.valid() {
            return Err(crate::ErrorKind::InputError(
                "Invalid fingerprint".into(),
            )
            .into());
        }
        let mut unit = super::atom(handle).await?;
        sqlx::query(
            "INSERT INTO content_file_fingerprints (instance_id, file_id, payload)
             SELECT instance_id, id, ? FROM instance_files
             WHERE instance_id = ? AND relative_path = ? AND sha1 = ? AND size = ? AND missing = 0
             ON CONFLICT (instance_id, file_id) DO UPDATE SET payload = excluded.payload"
        ).bind(serde_json::to_string(record)?).bind(instance_id).bind(&record.relative_path)
            .bind(record.sha1()).bind(record.size as i64).execute(unit.connection()).await?;
        unit.commit().await
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
