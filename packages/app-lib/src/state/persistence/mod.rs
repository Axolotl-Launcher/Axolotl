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
