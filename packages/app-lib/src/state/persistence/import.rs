//! A document is published and verified before its durable handover marker.
//! Neither an interrupted import nor a damaged new authority revives old rows.

use super::Persistence;
use super::document::{self, Document, ReadResult, Snapshot, Writer};
use super::status::Availability;
use serde_json::{Map, Value, json};
use std::future::Future;
use std::path::Path;

const PROTOCOL: &str = "axolotl-import";

pub(crate) struct Import {
    marker: Document,
    target: Document,
    name: String,
    relative_target: String,
}

impl Import {
    pub(crate) fn new(
        persistence: &Persistence,
        name: &str,
        target: Document,
    ) -> crate::Result<Self> {
        if name.is_empty()
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || byte == b'-'
                    || byte == b'_'
            })
        {
            return Err(crate::ErrorKind::InputError(
                "Invalid import name".into(),
            )
            .into());
        }
        let relative_target = target
            .path()
            .strip_prefix(persistence.root())
            .map_err(|_| {
                crate::ErrorKind::InputError(
                    "Import target is outside its data root".into(),
                )
            })?
            .to_str()
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Import target is not UTF-8".into(),
                )
            })?
            .replace('\\', "/");
        let marker = persistence.document(marker_path(name), PROTOCOL, 1)?;
        if relative_target
            .to_ascii_lowercase()
            .starts_with("state/persistence/")
        {
            return Err(crate::ErrorKind::InputError(
                "Import targets cannot overwrite persistence receipts".into(),
            )
            .into());
        }
        Ok(Self {
            marker,
            target,
            name: name.to_owned(),
            relative_target,
        })
    }

    /// The loader runs only while both the marker and the new authority are
    /// absent. SQL adapters can read a closed legacy snapshot asynchronously;
    /// the worker owns both file locks across validation and publication.
    /// Loaders read only the legacy snapshot; they must not acquire document
    /// locks or write through this target while its import owns those locks.
    pub(crate) async fn run<F, Fut, V>(
        &self,
        load: F,
        validate: V,
    ) -> crate::Result<ReadResult>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<Map<String, Value>>> + Send,
        V: Fn(&Snapshot) -> crate::Result<()> + Send + 'static,
    {
        let marker = self.marker.clone();
        let target = self.target.clone();
        let name = self.name.clone();
        let relative_target = self.relative_target.clone();
        let runtime = tokio::runtime::Handle::current();
        document::blocking(move || {
            let _marker_lock = Writer::acquire(marker.path())?;
            let _target_lock = Writer::acquire(target.path())?;
            let state = marker.read_locked()?;
            if state.availability != Availability::Missing {
                // Presence is irrevocable, including an opaque or damaged
                // marker. Only a valid marker grants a usable new authority.
                if !complete(&state, &name, &relative_target) {
                    return Ok(ReadResult {
                        availability: Availability::Unavailable("Import handover marker is unreadable or unsupported".into()),
                        snapshot: None,
                    });
                }
                let current = target.read_locked()?;
                if let Some(snapshot) = current.snapshot.as_ref()
                    && matches!(current.availability, Availability::Ready | Availability::Recovered) {
                    validate(snapshot)?;
                }
                return Ok(current);
            }
            let mut current = target.read_locked()?;
            if current.availability == Availability::Missing {
                let snapshot = target.initial(runtime.block_on(load())?);
                validate(&snapshot)?;
                target.write_locked(&snapshot)?;
                current = target.read_locked()?;
            }
            if !matches!(current.availability, Availability::Ready | Availability::Recovered) {
                return Ok(current);
            }
            validate(current.snapshot.as_ref().expect("readable document"))?;
            let receipt = marker.initial(Map::from_iter([
                ("name".into(), json!(name)),
                ("target".into(), json!(relative_target)),
                ("complete".into(), json!(true)),
            ]));
            marker.write_locked(&receipt)?;
            if !complete(&marker.read_locked()?, &name, &relative_target) {
                return Err(crate::ErrorKind::FSError("Import marker failed verification".into()).into());
            }
            Ok(current)
        }).await
    }
}

fn complete(result: &ReadResult, name: &str, target: &str) -> bool {
    matches!(
        result.availability,
        Availability::Ready | Availability::Recovered
    ) && result
        .snapshot
        .as_ref()
        .and_then(Snapshot::data)
        .is_some_and(|data| {
            data.get("name").and_then(Value::as_str) == Some(name)
                && data.get("target").and_then(Value::as_str) == Some(target)
                && data.get("complete").and_then(Value::as_bool) == Some(true)
        })
}

fn marker_path(name: &str) -> String {
    format!("state/persistence/imports/{name}.json")
}

/// These names are reserved for whole-domain handovers. A marker is written
/// only after that domain's adapter has validated its complete new authority.
/// An unreadable marker also retires the old source: reattaching it would make
/// previously deleted rows writable again. Its service reports unavailable.
pub(crate) async fn retired_database_domains(
    root: &Path,
) -> crate::Result<Vec<String>> {
    let persistence = Persistence::new(root);
    let mut retired = Vec::new();
    for name in ["accounts", "telemetry", "media", "sync"] {
        let marker = persistence.document(marker_path(name), PROTOCOL, 1)?;
        match marker.read().await {
            Ok(result) if result.availability == Availability::Missing => {}
            Ok(_) => retired.push(name.to_owned()),
            Err(error) => {
                tracing::warn!(domain = name, %error, "Handover receipt unavailable; leaving its legacy database retired");
                retired.push(name.to_owned());
            }
        }
    }
    Ok(retired)
}

/// Freeze a receipt and its authority under the same lock order as import.
/// A backup must not advertise completion while omitting a retired authority.
pub(crate) async fn snapshot(
    persistence: &Persistence,
    name: &str,
) -> crate::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
    let marker = persistence.document(marker_path(name), PROTOCOL, 1)?;
    let persistence = persistence.clone();
    let name = name.to_owned();
    document::blocking(move || {
        let _marker_lock = Writer::acquire(marker.path())?;
        let read = marker.read_locked()?;
        let target = read
            .snapshot
            .as_ref()
            .and_then(Snapshot::data)
            .and_then(|data| data.get("target"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                crate::ErrorKind::FSError(
                    "Cannot snapshot an unreadable handover receipt".into(),
                )
            })?;
        if !complete(&read, &name, target) {
            return Err(crate::ErrorKind::FSError(
                "Cannot snapshot an unsupported handover receipt".into(),
            )
            .into());
        }
        let target_path =
            crate::util::io::safe_relative_join(persistence.root(), target)?;
        if target_path.extension().and_then(|value| value.to_str())
            != Some("json")
            || target
                .to_ascii_lowercase()
                .starts_with("state/persistence/")
        {
            return Err(crate::ErrorKind::InputError(
                "Invalid authority in handover receipt".into(),
            )
            .into());
        }
        let _target_lock = Writer::acquire(&target_path)?;
        let bytes = document::read_bytes(&target_path)?.ok_or_else(|| {
            crate::ErrorKind::FSError(
                "Cannot snapshot a missing document authority".into(),
            )
        })?;
        if !serde_json::from_slice::<Value>(&bytes)?.is_object() {
            return Err(crate::ErrorKind::FSError(
                "Cannot snapshot an invalid document envelope".into(),
            )
            .into());
        }
        let receipt =
            document::read_bytes(marker.path())?.ok_or_else(|| {
                crate::ErrorKind::FSError("Handover receipt disappeared".into())
            })?;
        Ok(vec![
            (marker_path(&name).into(), receipt),
            (target.into(), bytes),
        ])
    })
    .await
}
