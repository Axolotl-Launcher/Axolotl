use super::operations::{
    InstanceScreenshot, has_png_extension, sort_screenshots,
    source_screenshots_dir,
};
use crate::State;
use crate::state::instances::adapters::sqlite::screenshot_sources::{
    self, InstanceScreenshotSource,
};
use crate::state::media::{
    Media,
    manifest::{ScreenshotIdentity, unique_renames},
};
use crate::util::fetch::sha1_file_async;
use crate::util::io::IOError;
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::Mutex;

#[derive(Clone)]
pub(super) struct ScannedScreenshot {
    file_name: String,
    created_at: DateTime<Utc>,
    modified_at: i64,
    file_size: i64,
    path: PathBuf,
}

pub(super) async fn list_source_screenshots(
    state: &State,
    source: InstanceScreenshotSource,
) -> crate::Result<Vec<InstanceScreenshot>> {
    let _lock = state.lock_instance_screenshots(&source.id).await;
    let scanned = scan_source_screenshots(state, &source).await?;
    reconcile_source_screenshots(state, &source, scanned).await
}

pub(crate) async fn reconcile_screenshots(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    reconcile_screenshots_with_state(instance_id, &state).await
}

pub(crate) async fn reconcile_screenshots_with_state(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    if let Some(source) = screenshot_sources::get_instance_screenshot_source(
        instance_id,
        &state.pool,
    )
    .await?
    {
        list_source_screenshots(state, source).await?;
    }
    Ok(())
}

pub(super) async fn scan_source_screenshots(
    state: &State,
    source: &InstanceScreenshotSource,
) -> crate::Result<Vec<ScannedScreenshot>> {
    let screenshots_dir = source_screenshots_dir(state, source).await?;
    tokio::task::spawn_blocking(move || scan_screenshots_dir(&screenshots_dir))
        .await?
}

fn scan_screenshots_dir(
    screenshots_dir: &Path,
) -> crate::Result<Vec<ScannedScreenshot>> {
    let entries = match std::fs::read_dir(screenshots_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error.into()),
    };
    let mut screenshots = Vec::new();

    for entry in entries {
        let entry = entry
            .map_err(|error| IOError::with_path(error, screenshots_dir))?;
        let file_type = entry
            .file_type()
            .map_err(|error| IOError::with_path(error, entry.path()))?;
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        if !has_png_extension(&path) {
            continue;
        }

        let Some(file_name) = entry.file_name().to_str().map(str::to_owned)
        else {
            continue;
        };
        let metadata = entry
            .metadata()
            .map_err(|error| IOError::with_path(error, &path))?;
        let created_at = metadata
            .created()
            .or_else(|_| metadata.modified())
            .map(DateTime::<Utc>::from)
            .map_err(|error| IOError::with_path(error, &path))?;
        let modified_at = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .map(|value| value.timestamp_millis())
            .map_err(|error| IOError::with_path(error, &path))?;
        let file_size = i64::try_from(metadata.len()).map_err(|_| {
            crate::ErrorKind::InputError(
                "Screenshot is too large to index".to_string(),
            )
        })?;

        screenshots.push(ScannedScreenshot {
            file_name,
            created_at,
            modified_at,
            file_size,
            path,
        });
    }

    Ok(screenshots)
}

pub(super) async fn reconcile_source_screenshots(
    state: &State,
    source: &InstanceScreenshotSource,
    scanned: Vec<ScannedScreenshot>,
) -> crate::Result<Vec<InstanceScreenshot>> {
    let media = Media::new(&state.persistence);
    let mut existing = match media.screenshots().await {
        Ok(rows) => rows
            .into_iter()
            .filter(|row| row.instance_id == source.id)
            .collect::<Vec<_>>(),
        Err(error) => {
            tracing::warn!(%error, "Screenshot organization unavailable; showing native files");
            Vec::new()
        }
    };
    let present = scanned
        .iter()
        .map(|file| file.file_name.clone())
        .collect::<HashSet<_>>();
    static OBSERVED: OnceLock<Mutex<HashMap<String, HashSet<String>>>> =
        OnceLock::new();
    let observed = OBSERVED.get_or_init(|| Mutex::new(HashMap::new()));
    let previous = {
        let mut inventory = observed.lock().await;
        inventory.insert(
            format!("{}\0{}", state.persistence.root().display(), source.id),
            present.clone(),
        )
    };
    let known = existing
        .iter()
        .map(|row| row.file_name.clone())
        .collect::<HashSet<_>>();
    // Only a missing anchored identity can justify reading candidate bodies.
    // New screenshots and changed physical attributes alone never trigger it.
    if existing.iter().any(|row| {
        row.fingerprint.is_some() && !present.contains(&row.file_name)
    }) {
        let candidates = scanned
            .iter()
            .filter(|file| {
                !known.contains(&file.file_name)
                    && newly_observed(previous.as_ref(), &file.file_name)
            })
            .cloned()
            .collect::<Vec<_>>();
        let hashes = stream::iter(candidates)
            .map(|file| async move {
                match candidate_hash(&file).await {
                    Ok(hash) => Some((file.file_name, hash)),
                    Err(error) => {
                        tracing::warn!(%error, "Screenshot rename candidate unavailable; retaining native listing");
                        None
                    }
                }
            })
            .buffer_unordered(4)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for (id, name) in unique_renames(&existing, &present, &hashes) {
            match media.relocate(&id, &source.id, &name).await {
                Ok(()) => {
                    if let Some(row) =
                        existing.iter_mut().find(|row| row.id == id)
                    {
                        row.file_name = name;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error,"Screenshot rename relationship could not be published")
                }
            }
        }
    }
    let by_name = existing
        .into_iter()
        .map(|row| (row.file_name.clone(), row))
        .collect::<HashMap<_, _>>();
    let mut result = scanned
        .into_iter()
        .map(|file| {
            let identity =
                by_name.get(&file.file_name).cloned().unwrap_or_else(|| {
                    ScreenshotIdentity::for_file(&source.id, &file.file_name)
                });
            InstanceScreenshot {
                id: identity.id,
                instance_id: source.id.clone(),
                instance_name: source.name.clone(),
                file_name: file.file_name,
                created_at: identity
                    .authored_at
                    .and_then(DateTime::from_timestamp_millis)
                    .unwrap_or(file.created_at),
                modified_at: file.modified_at,
                group_id: identity.group_id,
                path: file.path,
            }
        })
        .collect::<Vec<_>>();
    sort_screenshots(&mut result);
    Ok(result)
}

// Session-only hints avoid rehashing the same unmatched files on every list.
// Neither file attributes nor negative matches become persistent authority.
async fn candidate_hash(file: &ScannedScreenshot) -> crate::Result<String> {
    type Hints = HashMap<PathBuf, (i64, i64, String)>;
    static HINTS: OnceLock<Mutex<Hints>> = OnceLock::new();
    let hints = HINTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((size, modified, hash)) =
        hints.lock().await.get(&file.path).cloned()
        && size == file.file_size
        && modified == file.modified_at
    {
        return Ok(hash);
    }
    let (_, hash) = sha1_file_async(&file.path).await?;
    let mut cache = hints.lock().await;
    if cache.len() >= 8192 {
        cache.clear();
    }
    cache.insert(
        file.path.clone(),
        (file.file_size, file.modified_at, hash.clone()),
    );
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_listing_reads_attributes_without_decoding_png_contents() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("image.png"), b"not a PNG body")
            .unwrap();
        std::fs::write(directory.path().join("ignored.txt"), b"text").unwrap();
        std::fs::create_dir(directory.path().join("directory.png")).unwrap();
        let files = scan_screenshots_dir(directory.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "image.png");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
    }
}

fn newly_observed(previous: Option<&HashSet<String>>, filename: &str) -> bool {
    previous.is_some_and(|old| !old.contains(filename))
}
#[cfg(test)]
mod rename_tests {
    use super::*;
    #[test]
    fn preexisting_unanchored_duplicate_and_first_scan_cannot_receive_a_missing_identity()
     {
        let previous = HashSet::from(["A.png".to_owned(), "B.png".to_owned()]);
        assert!(!newly_observed(Some(&previous), "B.png"));
        assert!(!newly_observed(None, "renamed.png"));
        assert!(newly_observed(Some(&previous), "renamed.png"));
    }
}
