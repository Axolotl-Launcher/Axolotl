//! Bounded JSON documents with serialized, cancellation-safe publication.

use super::status::Availability;
use fs4::tokio::AsyncFileExt;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 16 << 20;

#[derive(Clone)]
pub(crate) struct Document {
    path: PathBuf,
    protocol: &'static str,
    schema_version: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub(crate) availability: Availability,
    /// Retain the envelope as well as the payload, including unknown fields.
    raw: Value,
}

impl Snapshot {
    pub(crate) fn data(&self) -> Option<&Map<String, Value>> {
        self.raw.get("data").and_then(Value::as_object)
    }

    pub(crate) fn decode<T: DeserializeOwned>(&self) -> crate::Result<T> {
        Ok(serde_json::from_value(
            self.raw.get("data").cloned().unwrap_or(Value::Null),
        )?)
    }

    pub(crate) fn revision(&self) -> Option<u64> {
        self.raw.get("revision").and_then(Value::as_u64)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReadResult {
    pub(crate) availability: Availability,
    pub(crate) snapshot: Option<Snapshot>,
}

impl Document {
    pub(super) fn new(
        path: PathBuf,
        protocol: &'static str,
        schema_version: u32,
    ) -> Self {
        Self {
            path,
            protocol,
            schema_version,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) async fn read(&self) -> crate::Result<ReadResult> {
        let document = self.clone();
        blocking(move || {
            let _lock = Writer::acquire(&document.path)?;
            document.read_locked()
        })
        .await
    }

    /// Keep a service's secret resolution or validation in the same worker and
    /// file lock as its metadata read. The callback must not reenter this file.
    pub(crate) async fn inspect<T, F>(&self, inspect: F) -> crate::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(ReadResult) -> crate::Result<T> + Send + 'static,
    {
        let document = self.clone();
        blocking(move || {
            let _lock = Writer::acquire(&document.path)?;
            inspect(document.read_locked()?)
        })
        .await
    }

    /// Mutate named payload keys under the file lock. A typed whole-document
    /// roundtrip is deliberately not used: it would discard unknown fields.
    pub(crate) async fn update<F>(&self, change: F) -> crate::Result<Snapshot>
    where
        F: FnOnce(&mut Map<String, Value>) -> crate::Result<()>
            + Send
            + 'static,
    {
        self.update_and_then(change, |_| Ok(())).await
    }

    /// The completion callback runs after durable publication, before the
    /// worker releases the file lock, including when metadata did not change.
    pub(crate) async fn update_and_then<F, C>(
        &self,
        change: F,
        committed: C,
    ) -> crate::Result<Snapshot>
    where
        F: FnOnce(&mut Map<String, Value>) -> crate::Result<()>
            + Send
            + 'static,
        C: FnOnce(&Snapshot) -> crate::Result<()> + Send + 'static,
    {
        let document = self.clone();
        blocking(move || {
            let _lock = Writer::acquire(&document.path)?;
            let read = document.read_locked()?;
            if !read.availability.writable() {
                return Err(unavailable(&document.path, &read.availability));
            }
            let mut snapshot =
                read.snapshot.unwrap_or_else(|| document.empty());
            let previous = snapshot.raw.clone();
            let data = snapshot
                .raw
                .get_mut("data")
                .and_then(Value::as_object_mut)
                .expect("validated payload");
            change(data)?;
            if snapshot.raw == previous
                && read.availability != Availability::Missing
            {
                committed(&snapshot)?;
                return Ok(snapshot);
            }
            let revision =
                snapshot.revision().unwrap_or(0).checked_add(1).ok_or_else(
                    || {
                        crate::ErrorKind::InputError(
                            "Document revision overflow".into(),
                        )
                    },
                )?;
            snapshot.raw["revision"] = Value::from(revision);
            document.write_locked(&snapshot)?;
            snapshot.availability = Availability::Ready;
            committed(&snapshot)?;
            Ok(snapshot)
        })
        .await
    }

    pub(super) fn empty(&self) -> Snapshot {
        Snapshot {
            availability: Availability::Ready,
            raw: json!({
                "protocol": self.protocol,
                "schema_version": self.schema_version,
                "revision": 0,
                "data": {},
            }),
        }
    }

    pub(super) fn initial(&self, data: Map<String, Value>) -> Snapshot {
        let mut snapshot = self.empty();
        snapshot.raw["data"] = Value::Object(data);
        snapshot.raw["revision"] = Value::from(1);
        snapshot
    }

    fn parse(&self, bytes: &[u8]) -> Result<Snapshot, String> {
        let raw: Value =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let Some(object) = raw.as_object() else {
            return Err("Document envelope is not an object".into());
        };
        let protocol = object.get("protocol").and_then(Value::as_str);
        let version = object.get("schema_version").and_then(Value::as_u64);
        if protocol.is_some_and(|protocol| protocol != self.protocol)
            || version
                .is_some_and(|version| version > u64::from(self.schema_version))
        {
            return Ok(Snapshot {
                availability: Availability::ReadOnly(
                    "Unsupported document protocol or schema".into(),
                ),
                raw,
            });
        }
        if protocol != Some(self.protocol)
            || version != Some(u64::from(self.schema_version))
            || !object.get("data").is_some_and(Value::is_object)
            || object.get("revision").and_then(Value::as_u64).is_none()
        {
            return Err("Invalid document envelope".into());
        }
        Ok(Snapshot {
            availability: Availability::Ready,
            raw,
        })
    }

    pub(super) fn read_locked(&self) -> crate::Result<ReadResult> {
        let published = read_bytes(&self.path);
        match &published {
            Ok(Some(bytes)) => match self.parse(bytes) {
                Ok(snapshot) => {
                    return Ok(ReadResult {
                        availability: snapshot.availability.clone(),
                        snapshot: Some(snapshot),
                    });
                }
                Err(_) => {}
            },
            Ok(None) => {}
            Err(error) => {
                return Ok(ReadResult {
                    availability: Availability::Unavailable(error.to_string()),
                    snapshot: None,
                });
            }
        }
        let backup = sidecar(&self.path, ".previous");
        let backup_bytes = read_bytes(&backup);
        let backup_present = !matches!(&backup_bytes, Ok(None));
        let candidate = backup_bytes.ok().flatten();
        if let Some((bytes, mut snapshot)) = candidate.and_then(|bytes| {
            self.parse(&bytes).ok().map(|snapshot| (bytes, snapshot))
        }) {
            // A newer backup is opaque too. Publish its bytes unchanged and
            // retain its read-only status instead of lowering its schema.
            if let Ok(Some(bytes)) = &published {
                preserve_corrupt(&self.path, bytes)?;
            }
            replace_bytes(&self.path, &bytes)?;
            if snapshot.availability == Availability::Ready {
                snapshot.availability = Availability::Recovered;
            }
            return Ok(ReadResult {
                availability: snapshot.availability.clone(),
                snapshot: Some(snapshot),
            });
        }
        match published {
            Ok(None) if !backup_present => Ok(ReadResult {
                availability: Availability::Missing,
                snapshot: None,
            }),
            Ok(None) => Ok(ReadResult {
                availability: Availability::Unavailable(
                    "Previous document copy is unreadable".into(),
                ),
                snapshot: None,
            }),
            Ok(Some(bytes)) => {
                preserve_corrupt(&self.path, &bytes)?;
                Ok(ReadResult {
                    availability: Availability::Unavailable(
                        "No readable document or previous good copy".into(),
                    ),
                    snapshot: None,
                })
            }
            Err(_) => unreachable!(),
        }
    }

    pub(super) fn write_locked(
        &self,
        snapshot: &Snapshot,
    ) -> crate::Result<()> {
        let bytes = serde_json::to_vec_pretty(&snapshot.raw)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(crate::ErrorKind::InputError(
                "State document exceeds its size limit".into(),
            )
            .into());
        }
        if let Some(previous) = read_bytes(&self.path)? {
            if self.parse(&previous).is_ok() {
                replace_bytes(&sidecar(&self.path, ".previous"), &previous)?;
            } else {
                preserve_corrupt(&self.path, &previous)?;
            }
        }
        replace_bytes(&self.path, &bytes)
    }
}

fn unavailable(path: &Path, availability: &Availability) -> crate::Error {
    crate::ErrorKind::FSError(format!(
        "State document {} cannot be changed: {availability:?}",
        path.display(),
    ))
    .into()
}

/// All synchronous I/O, lock waits and publication stay in one owned worker.
/// Cancellation of the awaiting future cannot drop its lock mid-publication.
pub(super) async fn blocking<T, F>(operation: F) -> crate::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> crate::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Document worker failed: {error}"
            ))
        })?
}

pub(super) struct Writer {
    _file: tokio::fs::File,
}

impl Writer {
    pub(super) fn acquire(path: &Path) -> crate::Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Document has no parent directory".into(),
            )
        })?;
        std::fs::create_dir_all(parent)?;
        // The OS file lock identifies a canonical directory across separate
        // Persistence handles and processes; no in-memory registry can diverge.
        let parent = std::fs::canonicalize(parent)?;
        let name = path.file_name().ok_or_else(|| {
            crate::ErrorKind::InputError("Document has no filename".into())
        })?;
        let lock = sidecar(&parent.join(name), ".writer.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock)?;
        let file = tokio::fs::File::from_std(file);
        while !file.try_lock_exclusive()? {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(Self { _file: file })
    }
}

pub(super) fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub(super) fn read_bytes(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(std::io::Error::other(
            "State document is not a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(std::io::Error::other(
            "State document grew beyond its size limit",
        ));
    }
    Ok(Some(bytes))
}

fn preserve_corrupt(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    // A repeated failed read of identical bytes need not create more copies.
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(bytes));
    let destination = sidecar(path, &format!(".corrupt-{digest}"));
    if !destination.try_exists()? {
        replace_bytes(&destination, bytes)?;
    }
    Ok(())
}

fn replace_bytes(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        crate::ErrorKind::InputError("Document has no parent directory".into())
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    crate::util::io::atomically_replace_file(temporary.path(), path)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

/// Shared publication primitive for existing settings documents. Their own
/// composition/version rules and database commit journal remain with callers.
pub(crate) async fn write_json(
    path: &Path,
    bytes: Vec<u8>,
) -> crate::Result<()> {
    let path = path.to_owned();
    blocking(move || {
        serde_json::from_slice::<Value>(&bytes)?;
        let _lock = Writer::acquire(&path)?;
        if let Some(previous) = read_bytes(&path)? {
            if serde_json::from_slice::<Value>(&previous).is_ok() {
                replace_bytes(&sidecar(&path, ".previous"), &previous)?;
            } else {
                preserve_corrupt(&path, &previous)?;
            }
        }
        replace_bytes(&path, &bytes)
    })
    .await
}

/// Publish immutable content before a manifest may reference it. A corrupt
/// existing object is archived; readers never observe a partially written body.
pub(crate) async fn write_blob(
    path: &Path,
    bytes: Vec<u8>,
) -> crate::Result<()> {
    let path = path.to_owned();
    blocking(move || {
        if bytes.len() as u64 > MAX_BYTES {
            return Err(crate::ErrorKind::InputError(
                "Media content exceeds the supported size".into(),
            )
            .into());
        }
        let _lock = Writer::acquire(&path)?;
        if let Some(previous) = read_bytes(&path)? {
            if previous == bytes {
                return Ok(());
            }
            preserve_corrupt(&path, &previous)?;
        }
        replace_bytes(&path, &bytes)
    })
    .await
}

/// Replace a writable native projection with a fresh inode even for equal
/// bytes. This deliberately breaks any legacy hard/symbolic shared alias.
pub(crate) async fn write_detached_blob(
    path: &Path,
    bytes: Vec<u8>,
) -> crate::Result<()> {
    let path = path.to_owned();
    blocking(move || {
        if bytes.len() as u64 > MAX_BYTES {
            return Err(crate::ErrorKind::InputError(
                "Native projection exceeds size limit".into(),
            )
            .into());
        }
        let _lock = Writer::acquire(&path)?;
        replace_bytes(&path, &bytes)
    })
    .await
}
