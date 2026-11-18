//! File authorities independent of a database connection or SQL transaction.
//!
//! Services migrate individually. A completed handover is durable even if its
//! new document later disappears: that is a local availability problem, never
//! permission to resurrect the retired database snapshot.

pub(crate) mod document;
pub(crate) mod import;
pub(crate) mod status;

#[cfg(test)]
mod tests;

use std::path::{Component, Path, PathBuf};

/// Only a missing group authority may start with an empty document.
pub(super) fn read_group_file<T: serde::de::DeserializeOwned>(
    path: &Path,
    version: u32,
) -> crate::Result<Option<T>> {
    let read = (|| -> crate::Result<Option<T>> {
        let Some(bytes) = document::snapshot_json(path)? else {
            if document::snapshot_json(&document::sidecar(path, ".previous"))?
                .is_some()
            {
                return Err(crate::ErrorKind::FSError(
                    "Restore the previous group document before editing".into(),
                )
                .into());
            }
            return Ok(None);
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        if value["schema_version"]
            .as_u64()
            .is_some_and(|stored| stored > u64::from(version))
        {
            return Err(crate::ErrorKind::FSError(
                "Group document uses a newer schema".into(),
            )
            .into());
        }
        Ok(Some(serde_json::from_slice(&bytes)?))
    })();
    read.map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Group data at {} is unavailable; editing is disabled until the file is repaired: {error}",
            path.display(),
        )).into()
    })
}

pub(super) fn edit_group_file<T, R>(
    cache: &std::sync::RwLock<Option<T>>,
    file_access: &std::sync::Mutex<()>,
    path: Option<&Path>,
    load: impl FnOnce(&Path) -> crate::Result<Option<T>>,
    normalize: impl FnOnce(&mut T),
    edit: impl FnOnce(&mut T) -> crate::Result<R>,
) -> crate::Result<R>
where
    T: Default + Clone + PartialEq + serde::Serialize,
{
    let mut cached = cache.write().unwrap_or_else(|error| error.into_inner());
    let _file_guard = file_access
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // Check again at the point of mutation: the file may have been damaged
    // or repaired since it was read into the cache.
    let current = match path.map(load).transpose() {
        Ok(file) => file.flatten().unwrap_or_default(),
        Err(error) => {
            *cached = None;
            return Err(error);
        }
    };
    let mut updated = current.clone();
    *cached = Some(current);
    let result = edit(&mut updated)?;
    normalize(&mut updated);
    if cached.as_ref() != Some(&updated) {
        let path = path.ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Group store is not initialized".into(),
            )
        })?;
        document::replace_bytes(path, &serde_json::to_vec_pretty(&updated)?)?;
        *cached = Some(updated);
    }
    Ok(result)
}

#[derive(Clone)]
pub(crate) struct Persistence {
    root: PathBuf,
}

impl Persistence {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn document(
        &self,
        relative: impl AsRef<Path>,
        protocol: &'static str,
        schema_version: u32,
    ) -> crate::Result<document::Document> {
        let relative = relative.as_ref();
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(crate::ErrorKind::InputError(
                "A state document needs a relative path inside its data root"
                    .into(),
            )
            .into());
        }
        let text = relative.to_str().ok_or_else(|| {
            crate::ErrorKind::InputError("A document path must be UTF-8".into())
        })?;
        if relative.extension().and_then(|value| value.to_str()) != Some("json")
            || protocol.is_empty()
            || schema_version == 0
        {
            return Err(crate::ErrorKind::InputError("State documents need a JSON filename, nonempty protocol and positive schema version".into()).into());
        }
        crate::util::io::safe_relative_join(
            &self.root,
            &text.replace('\\', "/"),
        )?;
        Ok(document::Document::new(
            self.root.join(relative),
            protocol,
            schema_version,
        ))
    }
}
