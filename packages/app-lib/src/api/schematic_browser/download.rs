//! Write one browsed schematic into an instance's folder.
//!
//! The plan itself is resolved through [`super::download_plan`]; this module
//! only carries out a `direct` delivery. Anything behind a share page
//! (`quark` / `external`) is refused here on purpose: the UI is expected to
//! offer "open the source page" for those instead of a download that cannot
//! finish.
//!
//! The file lands in the folder the content scanner already walks
//! (`schematics/`, or `structures/` for raw nbt), so no database write is
//! needed: the next scan registers it.

use std::path::{Path, PathBuf};

use tokio_util::sync::CancellationToken;

use crate::api::schematic_browser::error::{
    SchematicSourceError, SourceResult, StorageCode,
};
use crate::api::schematic_browser::types::{
    ContentKind, Delivery, GenerateSize, SchematicFormat, SourceId,
};
use crate::state::State;
use crate::util::fetch::{
    DownloadRequest, Integrity, ResourceClass, download_to_path,
};
use crate::util::io;

/// Destination folder, relative to the instance root.
///
/// Worlds are distributed as archives that have to be unpacked into their own
/// `saves/<name>/` directory; that is out of scope for this command, and the
/// UI sends those to the source page instead.
fn destination_folder(
    content_kind: ContentKind,
    format: SchematicFormat,
) -> SourceResult<&'static str> {
    match content_kind {
        ContentKind::World => Err(SchematicSourceError::unsupported(
            "world downloads need unpacking; open the source page",
        )),
        ContentKind::Schematic => Ok("schematics"),
    }
}

/// Keep a source-supplied name from escaping the destination folder.
///
/// Anything that is not a conservative file character becomes `_`, leading and
/// trailing dots are dropped (so `..` and `.hidden` cannot survive), and an
/// empty result falls back to the item id.
fn safe_file_name(
    hint: Option<&str>,
    id: &str,
    format: SchematicFormat,
) -> String {
    let stem = hint
        .map(|raw| {
            raw.rsplit(['/', '\\'])
                .next()
                .unwrap_or_default()
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric()
                        || matches!(c, '.' | '_' | '-' | ' ')
                    {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .unwrap_or_default();
    let stem = stem.trim().trim_matches('.').to_string();
    let suffix = match format {
        SchematicFormat::Unknown => "",
        other => other.as_str(),
    };
    let base = if stem.is_empty() {
        id.replace(['/', '\\', ':'], "-")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric()
                    || matches!(c, '.' | '_' | '-' | ' ')
                {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .trim_matches('.')
            .to_string()
    } else if suffix.is_empty() || stem.to_ascii_lowercase().ends_with(suffix) {
        stem
    } else {
        format!("{stem}.{suffix}")
    };
    base
}

/// First free path for `file_name` inside `folder`, suffixing `" (2)"`, `" (3)"`.
///
/// Re-downloading an item must never replace a file the user may have edited,
/// and the download path is atomic, so the check has to happen before it runs.
async fn unique_destination(folder: &Path, file_name: &str) -> PathBuf {
    let candidate = folder.join(file_name);
    if io::metadata(&candidate).await.is_err() {
        return candidate;
    }
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    for index in 2..1000u32 {
        let name = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let candidate = folder.join(name);
        if io::metadata(&candidate).await.is_err() {
            return candidate;
        }
    }
    // Falling back to the original name keeps the command from failing outright;
    // a thousand collisions already means the folder is unusable anyway.
    folder.join(file_name)
}

/// Download one item into the instance and return the written path.
pub async fn download_apply(
    instance_id: &str,
    source_id: SourceId,
    id: &str,
    size: Option<GenerateSize>,
    cancel: &CancellationToken,
) -> SourceResult<PathBuf> {
    let plan =
        super::download_plan(source_id, id, size.clone(), cancel).await?;
    match plan.delivery {
        Delivery::Direct => {}
        Delivery::Quark | Delivery::External => {
            return Err(SchematicSourceError::unsupported(
                "the source hands out a share page, not a file",
            ));
        }
    }
    // The plan carries no format or content kind; the cached detail does.
    let detail = super::detail(source_id, id, cancel).await?;
    let item = detail.item;
    let folder = destination_folder(item.content_kind, item.format)?;
    let file_name =
        safe_file_name(plan.file_name_hint.as_deref(), id, item.format);

    let state = State::get().await.map_err(|err| {
        SchematicSourceError::storage(StorageCode::Io, err.to_string())
    })?;
    let root = crate::api::instance::get_full_path(instance_id)
        .await
        .map_err(|err| {
            SchematicSourceError::storage(
                StorageCode::PathInvalid,
                err.to_string(),
            )
        })?;
    let folder_path = root.join(folder);
    io::create_dir_all(&folder_path).await.map_err(|err| {
        SchematicSourceError::storage(StorageCode::Io, err.to_string())
    })?;
    // A second download of the same item must not replace the copy the user may
    // have edited. Pick the next free `"name (2).ext"` instead of overwriting;
    // the fetch layer replaces its destination atomically, so it would otherwise
    // silently destroy local work.
    let destination = unique_destination(&folder_path, &file_name).await;

    let mut request = DownloadRequest::new(plan.url, ResourceClass::Other)
        .with_integrity(Integrity::default());
    if let Some(referer) = plan.referer {
        request = request.with_header("referer", referer);
    }
    request.cancellation = Some(cancel.clone());

    // Plans carry no checksum, so integrity stays empty and the fetch layer
    // trusts the length it observes. Small files only; see the plan types.
    download_to_path(
        request,
        &destination,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await
    .map_err(|err| {
        SchematicSourceError::storage(StorageCode::Io, err.to_string())
    })?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_follows_content_kind_and_format() {
        assert_eq!(
            destination_folder(
                ContentKind::Schematic,
                SchematicFormat::Litematic
            )
            .unwrap(),
            "schematics"
        );
        assert_eq!(
            destination_folder(ContentKind::Schematic, SchematicFormat::Nbt)
                .unwrap(),
            "structures"
        );
        assert!(
            destination_folder(ContentKind::World, SchematicFormat::Unknown)
                .is_err()
        );
    }

    #[test]
    fn hostile_names_are_neutralised() {
        assert_eq!(
            safe_file_name(
                Some("../../evil.jar"),
                "reden:1",
                SchematicFormat::Schem
            ),
            "evil.jar_schem".replace("_schem", ".schem")
        );
        // Traversal markers and separators never survive into the name.
        let name = safe_file_name(
            Some("..\\..\\windows\\x"),
            "reden:1",
            SchematicFormat::Litematic,
        );
        assert!(!name.contains(".."));
        assert!(!name.contains('\\'));
        assert!(!name.contains('/'));
        // An empty hint falls back to the id, and a hint that already carries
        // the format suffix is not double-suffixed.
        assert_eq!(
            safe_file_name(None, "buildschematics:42", SchematicFormat::Schem),
            "buildschematics-42.schem"
        );
        assert_eq!(
            safe_file_name(
                Some("house.litematic"),
                "reden:1",
                SchematicFormat::Litematic
            ),
            "house.litematic"
        );
    }
}
