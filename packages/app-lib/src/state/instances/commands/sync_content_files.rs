use crate::State;
use crate::state::instances::adapters::filesystem;
use crate::state::instances::{Instance, InstanceFile};
use crate::state::store::content_file_fingerprints::ContentFileFingerprints;
use crate::state::store::content_refresh_scan_failures::{
    ContentRefreshScanFailures, ScanFailureKey, ScanFailureRecord,
};
use crate::state::store::{Collection as _, instance_content_files};
use crate::state::{
    ContentProviderRef, DirectoryInfo, ModrinthVersionId, ProjectType,
};
use crate::util::fetch;
use crate::util::io;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Resolves the directory whose game content (`mods`, `resourcepacks`, ...)
/// belongs to this instance.
///
/// Ordinary instances own their profile directory under Axolotl's instances
/// folder, honouring a per-instance `game_dir_override`
/// (`DirectoryInfo::instance_game_dir`). Directly associated instances have no
/// profile directory: their content lives inside the externally managed
/// installation, resolved through the launcher dialect so PCL version
/// isolation (`versions/<id>` gameDir) is honored; the shared linked
/// `.minecraft` root is the fallback when the dialect resolution cannot be
/// completed (see `launcher::linked_game_dir`).
pub(crate) fn instance_content_root(
    directories: &DirectoryInfo,
    instance: &Instance,
) -> crate::Result<PathBuf> {
    if let Some(game_dir) =
        crate::launcher::linked_game_dir(instance).or_else(|| {
            instance
                .linked_dot_minecraft
                .as_deref()
                .map(str::trim)
                .filter(|linked| !linked.is_empty())
                .map(PathBuf::from)
        })
    {
        return Ok(io::canonicalize(game_dir)?);
    }

    Ok(io::canonicalize(directories.instance_game_dir(instance))?)
}

/// Joins a stored logical relative path using native path components. Content
/// rows intentionally use `/` as their cross-platform serialization format;
/// filesystem access must not concatenate that representation directly.
pub(crate) fn join_content_path(root: &Path, relative: &str) -> PathBuf {
    relative
        .split(['/', '\\'])
        .filter(|component| !component.is_empty() && *component != ".")
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

pub(crate) async fn sync_content_files(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceFile>> {
    sync_content_files_with_verification(
        instance_id,
        state,
        false,
        ScanConsistency::RequireStable,
    )
    .await
    .map(|outcome| outcome.files)
}

pub(crate) async fn sync_content_files_for_display(
    instance_id: &str,
    state: &State,
) -> crate::Result<ScanOutcome> {
    sync_content_files_with_verification(
        instance_id,
        state,
        false,
        ScanConsistency::AllowCommittedSnapshot,
    )
    .await
}

pub(crate) async fn sync_content_files_for_operation(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<InstanceFile>> {
    sync_content_files_with_verification(
        instance_id,
        state,
        true,
        ScanConsistency::RequireStable,
    )
    .await
    .map(|outcome| outcome.files)
}

async fn sync_content_files_with_verification(
    instance_id: &str,
    state: &State,
    verify_all: bool,
    consistency: ScanConsistency,
) -> crate::Result<ScanOutcome> {
    let instance =
        instance_content_files::get_instance(&state.data, instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError("Unknown instance".to_string())
            })?;

    sync_instance_content_files_with_verification(
        &instance,
        state,
        verify_all,
        consistency,
    )
    .await
}

pub(crate) async fn sync_instance_content_files(
    instance: &Instance,
    state: &State,
) -> crate::Result<Vec<InstanceFile>> {
    sync_instance_content_files_with_verification(
        instance,
        state,
        false,
        ScanConsistency::RequireStable,
    )
    .await
    .map(|outcome| outcome.files)
}

async fn sync_instance_content_files_with_verification(
    instance: &Instance,
    state: &State,
    verify_all: bool,
    consistency: ScanConsistency,
) -> crate::Result<ScanOutcome> {
    Box::pin(sync_with_retry_progress(
        instance,
        state,
        verify_all,
        consistency,
        || std::future::ready(()),
        None,
    ))
    .await
}

enum ScanAttempt {
    Committed(Vec<InstanceFile>),
    Retry(HashSet<String>),
}

#[derive(Debug)]
pub(crate) struct ScanOutcome {
    pub files: Vec<InstanceFile>,
    pub stable: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScanConsistency {
    RequireStable,
    AllowCommittedSnapshot,
}

#[cfg(test)]
async fn sync_with_retry<F, Fut>(
    instance: &Instance,
    state: &State,
    verify_all: bool,
    before_commit: F,
) -> crate::Result<Vec<InstanceFile>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    sync_with_retry_progress(
        instance,
        state,
        verify_all,
        ScanConsistency::RequireStable,
        before_commit,
        None,
    )
    .await
    .map(|outcome| outcome.files)
}

#[derive(Clone, Debug)]
pub(crate) enum ScanProgress {
    Files(Vec<InstanceFile>),
    Failed { path: String, error: String },
}

pub(crate) struct ScanPublication<'a> {
    pub sender: &'a tokio::sync::mpsc::Sender<ScanProgress>,
    pub cancel: &'a tokio_util::sync::CancellationToken,
    pub mode: crate::resources::refresh::RefreshMode,
}

pub(crate) async fn sync_content_files_progressively(
    instance_id: &str,
    state: &State,
    sender: &tokio::sync::mpsc::Sender<ScanProgress>,
    cancel: &tokio_util::sync::CancellationToken,
    mode: crate::resources::refresh::RefreshMode,
) -> crate::Result<ScanOutcome> {
    let instance =
        instance_content_files::get_instance(&state.data, instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError("Unknown instance".into())
            })?;
    sync_with_retry_progress(
        &instance,
        state,
        false,
        ScanConsistency::AllowCommittedSnapshot,
        || std::future::ready(()),
        Some(ScanPublication {
            sender,
            cancel,
            mode,
        }),
    )
    .await
}

async fn sync_with_retry_progress<F, Fut>(
    instance: &Instance,
    state: &State,
    verify_all: bool,
    consistency: ScanConsistency,
    mut before_commit: F,
    publication: Option<ScanPublication<'_>>,
) -> crate::Result<ScanOutcome>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    {
        let _lock = state
            .lock_instance_content_with_timeout(
                &instance.id,
                std::time::Duration::from_secs(20),
            )
            .await?;
        let root = instance_content_root(&state.directories, instance)?;
        super::content_file_operations::recover(state, &instance.id, &root)
            .await?;
    }
    let mut changed = HashSet::new();
    for _ in 0..3 {
        match scan_and_commit(
            instance,
            state,
            verify_all,
            &changed,
            &mut before_commit,
            publication.as_ref(),
        )
        .await?
        {
            ScanAttempt::Committed(files) => {
                return Ok(ScanOutcome {
                    files,
                    stable: true,
                });
            }
            ScanAttempt::Retry(paths) => changed.extend(paths),
        }
    }
    if publication
        .as_ref()
        .is_some_and(|publication| publication.cancel.is_cancelled())
    {
        return Err(crate::resources::ResourceError::Canceled.into());
    }
    if consistency == ScanConsistency::AllowCommittedSnapshot {
        ensure_scan_location(instance, state).await?;
        let files =
            instance_content_files::get_files(&state.data, &instance.id)
                .await?
                .into_iter()
                .filter(|file| !file.missing)
                .collect::<Vec<_>>();
        tracing::warn!(instance_id = %instance.id, files = files.len(),
            "Content remained unstable after three scans; using the last committed snapshot");
        // The coordinator already has the last committed snapshot. Do not
        // publish the fallback file list: doing so would schedule recognition
        // against stale hashes while the physical directory is still changing.
        return Ok(ScanOutcome {
            files,
            stable: false,
        });
    }
    Err(crate::ErrorKind::InputError(
        "Content changed repeatedly during scanning; retry after file operations finish".into(),
    ).into())
}

async fn scan_and_commit<F, Fut>(
    instance: &Instance,
    state: &State,
    verify_all: bool,
    _changed: &HashSet<String>,
    before_commit: &mut F,
    publication: Option<&ScanPublication<'_>>,
) -> crate::Result<ScanAttempt>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut content_set =
        instance_content_files::get_content_set(&state.data, &instance.id)
            .await?;
    let watch_snapshot = state
        .file_watcher
        .content_watch_snapshot(&instance.id)
        .await;
    let content_root = instance_content_root(&state.directories, instance)?;
    let instance_files_root = content_root.clone();
    let scan_root = content_root.clone();
    let scan_started = std::time::Instant::now();
    let scanned = tokio::task::spawn_blocking(move || {
        filesystem::scan_content_files_from(&scan_root, "")
    })
    .await??;
    tracing::debug!(instance_id = %instance.id, files = scanned.len(),
        elapsed_us = scan_started.elapsed().as_micros() as u64,
        "Scanned content file attributes");
    let scanned_state = scan_state(&scanned);
    let scanned_paths = scanned
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<HashSet<_>>();
    let failure_location = serde_json::to_string(&(
        crate::state::store::content_refresh_checks::location_key(instance),
        content_root.to_string_lossy(),
    ))?;
    let mut saved_failures = HashMap::new();
    if publication.is_some() {
        for (key, record) in
            ContentRefreshScanFailures::failed(&state.derived, &instance.id)
                .await?
        {
            if key.location_key != failure_location
                || !scanned_paths.contains(&key.relative_path)
            {
                ContentRefreshScanFailures::remove(&state.derived, &key)
                    .await?;
            } else {
                saved_failures.insert(key.relative_path, record);
            }
        }
    }
    let existing_files =
        instance_content_files::get_files(&state.data, &instance.id).await?;
    let mut existing_files_by_path = HashMap::new();
    let mut existing_files_by_sha1: HashMap<String, Vec<InstanceFile>> =
        HashMap::new();
    for file in existing_files {
        existing_files_by_sha1
            .entry(file.sha1.clone())
            .or_default()
            .push(file.clone());
        existing_files_by_path.insert(file.relative_path.clone(), file);
    }
    let saved_fingerprints =
        ContentFileFingerprints::get_scoped(&state.data, &instance.id)
            .await?
            .into_iter()
            .map(|(key, record)| (key.file_id, record))
            .collect::<HashMap<_, _>>();
    let entry_file_ids = match content_set.as_ref() {
        Some(content_set) => {
            instance_content_files::get_entry_file_ids(
                &state.data,
                &content_set.id,
            )
            .await?
        }
        None => HashSet::new(),
    };
    let mut fingerprint_records = HashMap::new();

    let now = Utc::now();
    let mut files: Vec<InstanceFile> = Vec::new();
    let mut reclaims: HashMap<String, String> = HashMap::new();
    let mut merges: HashMap<String, String> = HashMap::new();
    let mut claimed_reclaim_ids = HashSet::new();
    let mut externally_changed_file_ids = HashSet::new();
    let mut refreshed_metadata_file_ids = HashSet::new();
    let mut published_count = 0;
    let mut failed_files = Vec::new();
    let mut last_publish = std::time::Instant::now();
    let no_cancel = tokio_util::sync::CancellationToken::new();
    let cancel = publication
        .map(|publication| publication.cancel)
        .unwrap_or(&no_cancel);

    let mut scanned = scanned;
    scanned.sort_by_key(|file| {
        let known = existing_files_by_path.get(&file.relative_path)
            .and_then(|file| saved_fingerprints.get(&file.id))
            .is_some_and(|record| record.matches(&instance_files_root.to_string_lossy(), &file.relative_path, file.size,
                file.modified.and_then(crate::state::store::content_file_fingerprints::FileModifiedTime::from_system_time).as_ref()));
        !known
    });
    let mut scanned = scanned.into_iter().peekable();
    while let Some(file) = scanned.next() {
        if cancel.is_cancelled() {
            return Err(crate::ErrorKind::ResourceError(
                crate::resources::ResourceError::Canceled,
            )
            .into());
        }
        let existing_file = existing_files_by_path.get(&file.relative_path);
        let failure_key = ScanFailureKey {
            instance_id: instance.id.clone(),
            location_key: failure_location.clone(),
            relative_path: file.relative_path.clone(),
        };
        if let Some(publication) = publication
            && publication.mode
                == crate::resources::refresh::RefreshMode::Normal
            && let Some(failed) = saved_failures.get(&file.relative_path)
            && Utc::now().timestamp().saturating_sub(failed.attempted_at) < 30
        {
            let _ = publication
                .sender
                .send(ScanProgress::Failed {
                    path: file.relative_path.clone(),
                    error: failed.error.clone(),
                })
                .await;
            if let Some(existing) = existing_file {
                failed_files.push(existing.clone());
            }
            continue;
        }
        let previous =
            existing_file.and_then(|file| saved_fingerprints.get(&file.id));
        let attributes_known = previous.is_some_and(|record| record.matches(&instance_files_root.to_string_lossy(), &file.relative_path, file.size,
            file.modified.and_then(crate::state::store::content_file_fingerprints::FileModifiedTime::from_system_time).as_ref()));
        let resolved = state
            .file_fingerprints
            .resolve_scan(
                &instance_files_root,
                &file.relative_path,
                previous,
                verify_all,
                cancel,
            )
            .await;
        let resolved = resolved.and_then(|resolved| {
            if publication.is_some() && resolved.record.is_none() {
                Err(crate::resources::ResourceError::InvalidRequest(format!(
                    "{}: file modification time is unavailable",
                    file.relative_path
                )))
            } else {
                Ok(resolved)
            }
        });
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error)
                if publication.is_some()
                    && error != crate::resources::ResourceError::Canceled =>
            {
                ContentRefreshScanFailures::upsert(
                    &state.derived,
                    &failure_key,
                    &ScanFailureRecord {
                        attempted_at: Utc::now().timestamp(),
                        error: error.to_string(),
                    },
                )
                .await?;
                let _ = publication
                    .unwrap()
                    .sender
                    .send(ScanProgress::Failed {
                        path: file.relative_path.clone(),
                        error: error.to_string(),
                    })
                    .await;
                if let Some(existing) = existing_file {
                    failed_files.push(existing.clone());
                }
                continue;
            }
            Err(error) => {
                return Err(crate::ErrorKind::ResourceError(error).into());
            }
        };
        if publication.is_some()
            && saved_failures.contains_key(&file.relative_path)
        {
            ContentRefreshScanFailures::remove(&state.derived, &failure_key)
                .await?;
        }
        let scanned_size =
            resolved.identity.integrity.size.unwrap_or(file.size);
        let scanned_sha1 = resolved
            .identity
            .integrity
            .hashes
            .iter()
            .find(|hash| {
                hash.algorithm == crate::resources::HashAlgorithm::Sha1
            })
            .expect("fingerprint service always supplies SHA-1")
            .value
            .clone();
        if let Some(record) = resolved.record {
            fingerprint_records.insert(file.relative_path.clone(), record);
        }
        let reclaim_candidate = if existing_file.is_some() {
            None
        } else {
            reclaimable_existing_file(
                &scanned_sha1,
                &file.relative_path,
                &existing_files_by_sha1,
                &scanned_paths,
                &claimed_reclaim_ids,
            )
        };
        let merge_candidate = existing_file
            .filter(|file| !entry_file_ids.contains(&file.id))
            .and_then(|file| {
                mergeable_tracked_file(
                    &scanned_sha1,
                    &file.relative_path,
                    &file.id,
                    &existing_files_by_sha1,
                    &entry_file_ids,
                    &scanned_paths,
                    &claimed_reclaim_ids,
                )
            });
        let source_file = existing_file.or(reclaim_candidate);
        if let Some(existing_file) = existing_file
            && physical_file_identity_changed(
                existing_file,
                &scanned_sha1,
                scanned_size,
            )
        {
            externally_changed_file_ids.insert(existing_file.id.clone());
        }
        if let Some(candidate) = reclaim_candidate {
            claimed_reclaim_ids.insert(candidate.id.clone());
            reclaims.insert(
                file.relative_path.clone(),
                candidate.relative_path.clone(),
            );
        }
        if let Some(candidate) = merge_candidate {
            claimed_reclaim_ids.insert(candidate.id.clone());
            merges.insert(file.relative_path.clone(), candidate.id.clone());
        }

        let metadata_source = source_file.filter(|file| {
            !physical_file_identity_changed(file, &scanned_sha1, scanned_size)
        });
        files.push(InstanceFile {
            id: source_file
                .map(|file| file.id.clone())
                .unwrap_or_else(instance_file_id),
            instance_id: instance.id.clone(),
            relative_path: file.relative_path,
            file_name: file.file_name,
            enabled: file.enabled,
            sha1: scanned_sha1,
            size: scanned_size,
            missing: false,
            added_at: source_file.map(|file| file.added_at).unwrap_or(now),
            modified_at: now,
            local_mod_data: metadata_source
                .and_then(|file| file.local_mod_data.clone()),
            icon_path: metadata_source.and_then(|file| file.icon_path.clone()),
        });
        if publication.is_some() {
            let current = files.last_mut().unwrap();
            let previous = current.local_mod_data.clone();
            enrich_local_file(current, &instance_files_root, state).await;
            if previous != current.local_mod_data {
                refreshed_metadata_file_ids.insert(current.id.clone());
            }
            let next_needs_hash = scanned.peek().is_some_and(|next| {
                existing_files_by_path.get(&next.relative_path).and_then(|file| saved_fingerprints.get(&file.id)).is_none_or(|record| {
                    !record.matches(&instance_files_root.to_string_lossy(), &next.relative_path, next.size,
                        next.modified.and_then(crate::state::store::content_file_fingerprints::FileModifiedTime::from_system_time).as_ref())
                })
            });
            if published_count == 0
                || (attributes_known && next_needs_hash)
                || files.len() - published_count >= 32
                || last_publish.elapsed()
                    >= std::time::Duration::from_millis(50)
            {
                let batch = &files[published_count..];
                let _lock =
                    state.lock_instance_content_exclusive(&instance.id).await;
                ensure_scan_location(instance, state).await?;
                let latest = instance_content_files::get_content_set(
                    &state.data,
                    &instance.id,
                )
                .await?;
                if latest.as_ref().map(|set| (&set.id, set.revision))
                    != content_set.as_ref().map(|set| (&set.id, set.revision))
                {
                    return Ok(ScanAttempt::Retry(HashSet::new()));
                }
                let valid = batch.iter().all(|file| {
                    let path = join_content_path(
                        &instance_files_root,
                        &file.relative_path,
                    );
                    std::fs::metadata(path).ok().is_some_and(|metadata| {
                        metadata.len() == file.size
                            && metadata.modified().ok()
                                == scanned_state
                                    .get(&file.relative_path)
                                    .and_then(|state| state.1)
                    })
                });
                if !valid {
                    return Ok(ScanAttempt::Retry(HashSet::new()));
                }
                if let Some(committed) = instance_content_files::set_scanned(
                    &state.data,
                    instance_content_files::ScannedFiles {
                        instance_id: &instance.id,
                        content_set: &content_set,
                        files: batch,
                        reclaims: &reclaims,
                        merges: &merges,
                        externally_changed_file_ids:
                            &externally_changed_file_ids,
                        refreshed_metadata_file_ids:
                            &refreshed_metadata_file_ids,
                        fingerprints: &fingerprint_records,
                        complete: false,
                        create_entries: true,
                    },
                    || std::future::ready(!cancel.is_cancelled()),
                )
                .await?
                {
                    content_set = instance_content_files::get_content_set(
                        &state.data,
                        &instance.id,
                    )
                    .await?;
                    published_count = files.len();
                    last_publish = std::time::Instant::now();
                    externally_changed_file_ids.clear();
                    refreshed_metadata_file_ids.clear();
                    drop(_lock);
                    let _ = publication
                        .unwrap()
                        .sender
                        .send(ScanProgress::Files(committed))
                        .await;
                }
            }
        }
    }

    if publication.is_none() {
        for file in &mut files {
            let old = file.local_mod_data.clone();
            enrich_local_file(file, &content_root, state).await;
            if old != file.local_mod_data {
                refreshed_metadata_file_ids.insert(file.id.clone());
            }
        }
    }
    files.extend(failed_files);

    // Hashing is deliberately outside the instance lock. The lock only
    // protects the short database reconciliation transaction, so a large
    // modpack cannot block disable/delete operations for its entire scan.
    if cancel.is_cancelled() {
        return Err(crate::ErrorKind::ResourceError(
            crate::resources::ResourceError::Canceled,
        )
        .into());
    }
    before_commit().await;
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            &instance.id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    super::content_file_operations::recover(
        state,
        &instance.id,
        &instance_files_root,
    )
    .await?;
    ensure_scan_location(instance, state).await?;
    let latest_set =
        instance_content_files::get_content_set(&state.data, &instance.id)
            .await?;
    let latest_watch = state
        .file_watcher
        .content_watch_snapshot(&instance.id)
        .await;
    let verify_root = instance_files_root.clone();
    let latest_files = tokio::task::spawn_blocking(move || {
        filesystem::scan_content_files_from(&verify_root, "")
    })
    .await??;
    let latest_state = scan_state(&latest_files);
    let changed_paths = changed_scan_paths(&scanned_state, &latest_state);
    let revision = |set: &Option<crate::state::ContentSet>| {
        set.as_ref().map(|set| (set.id.clone(), set.revision))
    };
    if !changed_paths.is_empty()
        || watch_snapshot != latest_watch
        || revision(&content_set) != revision(&latest_set)
    {
        return Ok(ScanAttempt::Retry(changed_paths));
    }
    let Some(synced_files) =
        crate::state::store::instance_content_files::set_scanned(
            &state.data,
            crate::state::store::instance_content_files::ScannedFiles {
                instance_id: &instance.id,
                content_set: &content_set,
                files: &files,
                reclaims: &reclaims,
                merges: &merges,
                externally_changed_file_ids: &externally_changed_file_ids,
                refreshed_metadata_file_ids: &refreshed_metadata_file_ids,
                fingerprints: &fingerprint_records,
                complete: true,
                create_entries: publication.is_some(),
            },
            || async {
                state
                    .file_watcher
                    .content_watch_snapshot(&instance.id)
                    .await
                    == watch_snapshot
            },
        )
        .await?
    else {
        return Ok(ScanAttempt::Retry(HashSet::new()));
    };

    if let Some(snapshot) = watch_snapshot {
        state
            .file_watcher
            .clear_content_changes_if_generation(
                &instance.id,
                snapshot.generation,
            )
            .await;
    }

    if let Some(publication) = publication {
        drop(_instance_lock);
        let _ = publication
            .sender
            .send(ScanProgress::Files(synced_files.clone()))
            .await;
    }
    Ok(ScanAttempt::Committed(synced_files))
}

type ScanState = HashMap<String, (u64, Option<std::time::SystemTime>)>;

async fn ensure_scan_location(
    instance: &Instance,
    state: &State,
) -> crate::Result<()> {
    let current =
        instance_content_files::get_instance(&state.data, &instance.id).await?;
    if current.as_ref().is_none_or(|current| {
        crate::state::store::content_refresh_checks::location_key(current)
            != crate::state::store::content_refresh_checks::location_key(
                instance,
            )
    }) {
        return Err(crate::ErrorKind::ResourceError(
            crate::resources::ResourceError::Canceled,
        )
        .into());
    }
    Ok(())
}

fn scan_state(files: &[filesystem::ScannedContentFile]) -> ScanState {
    files
        .iter()
        .map(|file| (file.relative_path.clone(), (file.size, file.modified)))
        .collect()
}

fn changed_scan_paths(
    before: &ScanState,
    after: &ScanState,
) -> HashSet<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

pub(crate) async fn enrich_local_file(
    file: &mut InstanceFile,
    instance_dir: &Path,
    state: &State,
) {
    let icon_cache_dir = state.directories.caches_dir().join("icons");
    let Some(project_type) = project_type_for_file(file) else {
        return;
    };
    let extract_metadata = project_type == ProjectType::Mod
        && file.local_mod_data.as_ref().is_none_or(|json| {
            serde_json::from_str::<crate::mod_metadata::LocalModMetadata>(json)
                .ok()
                .map_or(!json.is_empty(), |metadata| metadata.needs_refresh())
        });
    let extract_icon = file.icon_path.is_none()
        && matches!(project_type, ProjectType::Mod | ProjectType::ResourcePack);
    if !extract_metadata && !extract_icon {
        return;
    }

    let path = join_content_path(instance_dir, &file.relative_path);

    // Resource packs are read entry-wise so large archives are not
    // materialized in memory just to fetch `pack.png`.
    if extract_icon && project_type == ProjectType::ResourcePack {
        let icon = crate::mod_metadata::icon::extract_resource_pack_icon(&path);
        file.icon_path = Some(
            cache_extracted_icon(icon, &file.sha1, &icon_cache_dir, state)
                .await,
        );
        return;
    }

    // Mods: one in-memory read serves both metadata and icon extraction.
    let bytes = match tokio::fs::read(&path).await {
        Ok(data) => bytes::Bytes::from(data),
        Err(_) => {
            // File temporarily inaccessible; skip silently.
            return;
        }
    };

    if extract_metadata {
        file.local_mod_data = Some(
            crate::mod_metadata::extract_mod_metadata(&bytes)
                .and_then(|metadata| serde_json::to_string(&metadata).ok())
                .unwrap_or_default(),
        );
    }

    if extract_icon {
        let meta = file.local_mod_data.as_ref().and_then(|json| {
            serde_json::from_str::<crate::mod_metadata::LocalModMetadata>(json)
                .ok()
        });
        let icon =
            crate::mod_metadata::icon::extract_mod_icon(&bytes, meta.as_ref());
        file.icon_path = Some(
            cache_extracted_icon(icon, &file.sha1, &icon_cache_dir, state)
                .await,
        );
    }
}

async fn cache_extracted_icon(
    icon: Option<(String, Vec<u8>)>,
    sha1: &str,
    icon_cache_dir: &Path,
    state: &State,
) -> String {
    let Some((entry_name, icon_bytes)) = icon else {
        return String::new();
    };

    let extension = icon_extension(&entry_name);
    let cache_path = icon_cache_dir.join(format!("{sha1}.{extension}"));
    match fetch::write(&cache_path, &icon_bytes, &state.io_semaphore).await {
        Ok(()) => crate::util::io::canonicalize(&cache_path)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| cache_path.to_string_lossy().into_owned()),
        Err(_) => String::new(),
    }
}

fn icon_extension(entry_name: &str) -> &str {
    let extension = Path::new(entry_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if matches!(
        extension.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg"
    ) {
        extension
    } else {
        "png"
    }
}

pub(crate) fn project_type_for_file(
    file: &InstanceFile,
) -> Option<ProjectType> {
    filesystem::project_type_from_relative_path(&file.relative_path)
}

pub(crate) fn installed_modrinth_version_id(
    provider_refs: &[ContentProviderRef],
) -> Option<ModrinthVersionId> {
    provider_refs.iter().find_map(|reference| match reference {
        ContentProviderRef::Modrinth { version_id, .. } => {
            version_id.as_ref().cloned()
        }
        ContentProviderRef::CurseForge { .. }
        | ContentProviderRef::McArchive { .. } => None,
    })
}

fn instance_file_id() -> String {
    format!("instance-file:{}", Uuid::new_v4())
}

fn physical_file_identity_changed(
    existing: &InstanceFile,
    scanned_sha1: &str,
    scanned_size: u64,
) -> bool {
    existing.sha1 != scanned_sha1 || existing.size != scanned_size
}

/// Finds the single missing file row that can safely inherit a scanned file's
/// installed identity. A rename should keep its provider refs, entry, and
/// install history; ambiguous duplicates and cross-type moves stay untracked.
fn reclaimable_existing_file<'a>(
    sha1: &str,
    new_relative_path: &str,
    existing_files_by_sha1: &'a HashMap<String, Vec<InstanceFile>>,
    scanned_paths: &HashSet<String>,
    claimed_ids: &HashSet<String>,
) -> Option<&'a InstanceFile> {
    let candidates = existing_files_by_sha1.get(sha1)?;
    if candidates
        .iter()
        .any(|file| scanned_paths.contains(file.relative_path.as_str()))
    {
        return None;
    }
    let missing = candidates
        .iter()
        .filter(|file| {
            file.missing || !scanned_paths.contains(&file.relative_path)
        })
        .collect::<Vec<_>>();
    if missing.len() != 1 || claimed_ids.contains(&missing[0].id) {
        return None;
    }
    let candidate = missing[0];
    let new_type =
        filesystem::project_type_from_relative_path(new_relative_path)?;
    let old_type =
        filesystem::project_type_from_relative_path(&candidate.relative_path)?;
    (new_type == old_type).then_some(candidate)
}

/// Finds the single tracked row that should hand its installed identity to an
/// untracked row at a new path. This repairs instances that were already
/// broken by a rename before reclaim-on-move existed.
fn mergeable_tracked_file<'a>(
    sha1: &str,
    new_relative_path: &str,
    untracked_file_id: &str,
    existing_files_by_sha1: &'a HashMap<String, Vec<InstanceFile>>,
    entry_file_ids: &HashSet<String>,
    scanned_paths: &HashSet<String>,
    claimed_ids: &HashSet<String>,
) -> Option<&'a InstanceFile> {
    let candidates = existing_files_by_sha1.get(sha1)?;
    let tracked = candidates
        .iter()
        .filter(|file| entry_file_ids.contains(&file.id))
        .collect::<Vec<_>>();
    if tracked.is_empty()
        || tracked.iter().any(|file| {
            file.id != untracked_file_id
                && scanned_paths.contains(file.relative_path.as_str())
        })
    {
        return None;
    }
    let missing = tracked
        .iter()
        .filter(|file| {
            file.id != untracked_file_id
                && (file.missing
                    || !scanned_paths.contains(file.relative_path.as_str()))
        })
        .collect::<Vec<_>>();
    if missing.len() != 1 || claimed_ids.contains(&missing[0].id) {
        return None;
    }
    let candidate = missing[0];
    let new_type =
        filesystem::project_type_from_relative_path(new_relative_path)?;
    let old_type =
        filesystem::project_type_from_relative_path(&candidate.relative_path)?;
    (new_type == old_type).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::instances::adapters::sqlite;
    use std::fs;
    use std::sync::Arc;

    #[test]
    fn content_scan_preserves_install_temporary_files() {
        let root = tempfile::tempdir().unwrap();
        let mods = root.path().join("instance/mods");
        fs::create_dir_all(&mods).unwrap();
        let temporary_names = [
            "example.jar.installing.download",
            "example.jar.installing",
            "example.jar.installing.previous",
        ];
        for name in temporary_names {
            fs::write(mods.join(name), name).unwrap();
        }

        let scanned =
            filesystem::scan_content_files(root.path(), "instance").unwrap();

        assert!(scanned.is_empty());
        for name in temporary_names {
            assert!(mods.join(name).is_file());
        }
    }

    // -----------------------------------------------------------------------
    // Directly associated instances scan their linked installation
    // -----------------------------------------------------------------------

    /// The launcher state is a process-wide singleton; initialize it once and
    /// reuse it so `State::get()` (used by the hash cache) resolves inside
    /// these APIs. The state root is intentionally leaked (`.keep()`) because
    /// the shared state outlives this function.
    async fn global_state() -> Arc<State> {
        if !State::initialized() {
            let root = tempfile::TempDir::new().unwrap().keep();
            let _ =
                State::init_for_test(root.to_string_lossy().to_string()).await;
        }
        State::get().await.unwrap()
    }

    fn write_self_contained_version(minecraft: &Path, version_id: &str) {
        let version_dir = minecraft.join("versions").join(version_id);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            version_dir.join(format!("{version_id}.json")),
            serde_json::to_vec(&serde_json::json!({
                "id": version_id,
                "mainClass": "net.minecraft.client.main.Main"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn direct_link_refresh_scans_linked_dot_minecraft_content() {
        let state = global_state().await;
        let minecraft = tempfile::TempDir::new().unwrap();
        write_self_contained_version(minecraft.path(), "1.12.2-linked");
        fs::create_dir_all(minecraft.path().join("mods")).unwrap();
        fs::write(
            minecraft.path().join("mods/linked-mod.jar"),
            b"linked mod bytes",
        )
        .unwrap();

        let instance = crate::state::create_direct_link_instance(
            crate::state::CreateDirectLinkInstance {
                name: None,
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::Generic,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: "versions/1.12.2-linked".to_string(),
                instance_path: None,
                game_dir_mode: Some(
                    crate::launcher::ExternalGameDirMode::Shared,
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let files = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();

        assert!(
            files
                .iter()
                .any(|file| file.relative_path == "mods/linked-mod.jar"),
            "refresh must discover mods inside the linked `.minecraft`, got: {:?}",
            files
                .iter()
                .map(|file| &file.relative_path)
                .collect::<Vec<_>>(),
        );

        // A second refresh reuses the fingerprint baseline. It must continue to
        // use the linked external root rather than the Axolotl profile path.
        let refreshed = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();
        assert!(
            refreshed
                .iter()
                .any(|file| file.relative_path == "mods/linked-mod.jar")
        );
    }

    #[tokio::test]
    async fn pcl_isolated_direct_link_scans_version_isolated_content() {
        let state = global_state().await;
        let minecraft = tempfile::TempDir::new().unwrap();
        write_self_contained_version(minecraft.path(), "1.12.2-pcl");
        // Version isolation on: PCL resolves this version's gameDir to
        // versions/<id>, so its mods folder lives beside the version JSON.
        let version_dir = minecraft.path().join("versions/1.12.2-pcl");
        fs::create_dir_all(version_dir.join("PCL")).unwrap();
        fs::write(
            version_dir.join("PCL/Setup.ini"),
            "VersionArgumentIndieV2: true\n",
        )
        .unwrap();
        fs::create_dir_all(version_dir.join("mods")).unwrap();
        fs::write(
            version_dir.join("mods/isolated-mod.jar"),
            b"isolated mod bytes",
        )
        .unwrap();

        let instance = crate::state::create_direct_link_instance(
            crate::state::CreateDirectLinkInstance {
                name: None,
                launcher_type:
                    crate::api::pack::import::ImportLauncherType::PCL2,
                base_path: minecraft.path().to_path_buf(),
                instance_folder: "versions/1.12.2-pcl".to_string(),
                instance_path: None,
                game_dir_mode: None,
            },
            &state,
        )
        .await
        .unwrap();

        let files = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();

        assert!(
            files
                .iter()
                .any(|file| file.relative_path == "mods/isolated-mod.jar"),
            "refresh must resolve the PCL-isolated gameDir, got: {:?}",
            files
                .iter()
                .map(|file| &file.relative_path)
                .collect::<Vec<_>>(),
        );
    }

    #[tokio::test]
    async fn ordinary_instance_refresh_still_scans_its_profile_directory() {
        let state = global_state().await;
        let metadata = crate::api::instance::create(
            format!("sync-normal {}", uuid::Uuid::new_v4()),
            "1.20.1".to_string(),
            crate::state::ModLoader::Vanilla,
            None,
            None,
            crate::state::InstanceLink::Unmanaged,
            None,
            None,
        )
        .await
        .unwrap();

        let mods = state
            .directories
            .instances_dir()
            .join(&metadata.instance.path)
            .join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("profile-mod.jar"), b"profile mod bytes").unwrap();

        let files = sync_instance_content_files(&metadata.instance, &state)
            .await
            .unwrap();

        assert!(
            files
                .iter()
                .any(|file| file.relative_path == "mods/profile-mod.jar"),
            "ordinary instances must keep scanning their profile directory, \
             got: {:?}",
            files
                .iter()
                .map(|file| &file.relative_path)
                .collect::<Vec<_>>(),
        );
    }

    /// Regression probe: an instance whose `game_dir_override` points to an
    /// external `.minecraft` root must have its content scanned from that root,
    /// not from the (empty) managed instance folder. This is the split-brain
    /// the content page empty-state used to hit.
    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn content_scan_uses_game_dir_override() {
        crate::event::EventState::init().await.unwrap();
        let root = tempfile::tempdir().unwrap().keep();
        let state =
            crate::State::init_for_test(root.to_string_lossy().to_string())
                .await
                .unwrap();

        // Create an external .minecraft root with a mod, outside the managed
        // profiles dir.
        let mc_root = tempfile::tempdir().unwrap();
        let mods_dir = mc_root.path().join("mods");
        fs::create_dir_all(&mods_dir).unwrap();
        fs::write(mods_dir.join("my-mod.jar"), "mod").unwrap();

        let created = crate::api::instance::create(
            "Override Instance".to_string(),
            "1.20.1".to_string(),
            crate::state::ModLoader::Vanilla,
            None,
            None,
            crate::state::InstanceLink::Unmanaged,
            None,
            Some(mc_root.path().to_string_lossy().to_string()),
        )
        .await
        .unwrap();
        crate::state::instances::commands::set_instance_install_stage(
            &created.instance.id,
            crate::state::InstanceInstallStage::Installed,
            &state.pool,
        )
        .await
        .unwrap();

        let instance =
            crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_id(
                &created.instance.id,
                &state.pool,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            state.directories.instance_game_dir(&instance),
            mc_root.path(),
            "instance_game_dir must resolve to the override root"
        );
        let direct =
            filesystem::scan_content_files_from(mc_root.path(), &instance.path)
                .unwrap();
        assert_eq!(
            direct.len(),
            1,
            "direct scan of override root should find the mod"
        );
        let files = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();

        assert_eq!(
            files.len(),
            1,
            "expected the override-root mod to be scanned"
        );
        assert!(files[0].relative_path.ends_with("mods/my-mod.jar"));
        crate::resources::installed::refresh_snapshot(
            &state,
            &instance.id,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        let entries =
            crate::resources::plans::installed(&state.pool, &instance.id)
                .await
                .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_path, "mods/my-mod.jar");
        assert_eq!(
            entries[0].ownership,
            crate::resources::ResourceOwnership::UserAdded
        );
        assert!(entries[0].artifact.is_none());
        crate::resources::installed::refresh_snapshot(
            &state,
            &instance.id,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            crate::resources::plans::installed(&state.pool, &instance.id)
                .await
                .unwrap()[0]
                .entry_id,
            entries[0].entry_id
        );
        fs::write(mods_dir.join("my-mod.jar"), "updated mod bytes").unwrap();
        let updated = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].size, 17);
        assert_ne!(updated[0].sha1, files[0].sha1);
        assert_eq!(updated[0].id, files[0].id);
        fs::write(mods_dir.join("my-mod.jar"), "changed mod bytes").unwrap();
        crate::resources::installed::refresh_snapshot(
            &state,
            &instance.id,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        let refreshed =
            crate::resources::plans::installed(&state.pool, &instance.id)
                .await
                .unwrap();
        assert_eq!(refreshed[0].integrity.size, Some(updated[0].size));
        assert_ne!(refreshed[0].integrity.hashes[0].value, updated[0].sha1);
    }

    #[test]
    fn external_hash_or_size_change_invalidates_physical_identity() {
        let now = Utc::now();
        let file = InstanceFile {
            id: "file".to_string(),
            instance_id: "instance".to_string(),
            relative_path: "mods/lithium.jar".to_string(),
            file_name: "lithium.jar".to_string(),
            enabled: true,
            sha1: "official".to_string(),
            size: 10,
            missing: false,
            added_at: now,
            modified_at: now,
            local_mod_data: None,
            icon_path: None,
        };

        assert!(!physical_file_identity_changed(&file, "official", 10));
        assert!(physical_file_identity_changed(&file, "external", 10));
        assert!(physical_file_identity_changed(&file, "official", 11));
    }

    async fn isolated_scan_fixture()
    -> (tempfile::TempDir, Arc<State>, Instance, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let directories = DirectoryInfo {
            settings_dir: temp.path().to_owned(),
            config_dir: temp.path().to_owned(),
            app_identifier: "content-scan-test".into(),
        };
        let root = directories.instances_dir().join("example");
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/example.jar"), b"old bytes").unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temp.path().join("state.db"))
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::raw_sql("INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('instance', 'example', 'installed', '1', 'Example', 1, 1); INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Example', 'local', 'available', '1.20.1', 'fabric', 1, 1); UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';").execute(&pool).await.unwrap();
        let state = crate::state::test_state(directories, pool).await.unwrap();
        let instance =
            sqlite::instance_rows::get_instance_by_id("instance", &state.pool)
                .await
                .unwrap()
                .unwrap();
        sync_instance_content_files(&instance, &state)
            .await
            .unwrap();
        (temp, state, instance, root)
    }

    #[tokio::test]
    async fn stale_scans_cannot_overwrite_toggle_delete_or_replacement() {
        for operation in ["toggle", "delete", "replace"] {
            let (_temp, state, instance, root) = isolated_scan_fixture().await;
            let barrier = tokio::sync::Barrier::new(2);
            let mut attempts = 0;
            let scan = sync_with_retry(&instance, &state, false, || {
                attempts += 1;
                let pause = attempts == 1;
                let barrier = &barrier;
                async move {
                    if pause {
                        barrier.wait().await;
                        barrier.wait().await;
                    }
                }
            });
            let mutate = async {
                barrier.wait().await;
                match operation {
                    "toggle" => {
                        crate::state::instances::commands::toggle_disable_project("instance", "mods/example.jar", Some(false), &state).await.unwrap();
                    }
                    "delete" => {
                        crate::state::instances::commands::remove_project(
                            "instance",
                            "mods/example.jar",
                            &state,
                        )
                        .await
                        .unwrap();
                    }
                    _ => {
                        let _lock =
                            state.lock_instance_content("instance").await;
                        fs::write(
                            root.join("mods/example.jar"),
                            b"replacement bytes",
                        )
                        .unwrap();
                    }
                }
                barrier.wait().await;
            };
            let (files, _) = tokio::join!(scan, mutate);
            let files = files.unwrap();
            assert!(attempts >= 2);
            match operation {
                "toggle" => {
                    assert_eq!(files.len(), 1);
                    assert_eq!(
                        files[0].relative_path,
                        "mods/example.jar.disabled"
                    );
                    assert!(!files[0].enabled);
                    assert!(!root.join("mods/example.jar").exists());
                }
                "delete" => {
                    assert!(files.is_empty());
                    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_files WHERE instance_id = 'instance'").fetch_one(&state.pool).await.unwrap();
                    assert_eq!(count, 0);
                }
                _ => {
                    assert_eq!(files.len(), 1);
                    assert_eq!(
                        files[0].sha1,
                        sha1_smol::Sha1::from(b"replacement bytes").hexdigest()
                    );
                    assert_eq!(files[0].size, 17);
                }
            }
            assert!(
                sqlx::query("PRAGMA foreign_key_check")
                    .fetch_all(&state.pool)
                    .await
                    .unwrap()
                    .is_empty()
            );
            state.pool.close().await;
        }
    }

    #[tokio::test]
    async fn repeatedly_changing_scan_leaves_database_untouched_and_is_retryable()
     {
        let (_temp, state, instance, root) = isolated_scan_fixture().await;
        let before = sqlite::files::get_instance_files("instance", &state.pool)
            .await
            .unwrap();
        let mut count = 0;
        let result = sync_with_retry(&instance, &state, false, || {
            count += 1;
            fs::write(root.join("mods/example.jar"), vec![b'x'; 20 + count])
                .unwrap();
            std::future::ready(())
        })
        .await;
        assert!(result.is_err());
        assert_eq!(count, 3);
        let after = sqlite::files::get_instance_files("instance", &state.pool)
            .await
            .unwrap();
        assert_eq!(after[0].sha1, before[0].sha1);
        let retried = sync_instance_content_files(&instance, &state)
            .await
            .unwrap();
        assert_eq!(retried[0].size, 23);
        state.pool.close().await;
    }

    #[tokio::test]
    async fn display_scan_falls_back_to_committed_files_and_preserves_pending_changes()
     {
        let (_temp, state, instance, root) = isolated_scan_fixture().await;
        let before = instance_content_files::get_files(&state.data, "instance")
            .await
            .unwrap();
        crate::state::instances::watcher::watch_instance_folder(
            "instance",
            "example",
            &root,
            &state.file_watcher,
        )
        .await;
        let mut count = 0;
        let outcome = sync_with_retry_progress(
            &instance,
            &state,
            false,
            ScanConsistency::AllowCommittedSnapshot,
            || {
                count += 1;
                fs::write(
                    root.join("mods/example.jar"),
                    vec![b'x'; 20 + count],
                )
                .unwrap();
                state.file_watcher.record_upgrade_content_change(
                    "instance",
                    "mods/example.jar",
                )
            },
            None,
        )
        .await
        .unwrap();
        assert!(!outcome.stable);
        assert_eq!(count, 3);
        assert_eq!(outcome.files[0].sha1, before[0].sha1);
        assert_eq!(outcome.files[0].size, before[0].size);
        let after = instance_content_files::get_files(&state.data, "instance")
            .await
            .unwrap();
        assert_eq!(after[0].sha1, before[0].sha1);
        let watch = state
            .file_watcher
            .content_watch_snapshot("instance")
            .await
            .unwrap();
        assert!(watch.dirty_paths.contains("mods/example.jar"));
        crate::state::instances::watcher::unwatch_instance_folder(
            "example",
            &root,
            &state.file_watcher,
        )
        .await;
        let stable = sync_content_files_for_display("instance", &state)
            .await
            .unwrap();
        assert!(stable.stable);
        assert_eq!(stable.files[0].size, 23);
        assert_ne!(stable.files[0].sha1, before[0].sha1);
        state.pool.close().await;
    }

    #[tokio::test]
    async fn progressive_fallback_does_not_publish_stale_files_and_cancellation_is_an_error()
     {
        let (_temp, state, instance, root) = isolated_scan_fixture().await;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut count = 0;
        let result = sync_with_retry_progress(
            &instance,
            &state,
            false,
            ScanConsistency::AllowCommittedSnapshot,
            || {
                count += 1;
                fs::write(
                    root.join("mods/example.jar"),
                    vec![b'x'; 20 + count],
                )
                .unwrap();
                std::future::ready(())
            },
            Some(ScanPublication {
                sender: &sender,
                cancel: &cancel,
                mode: crate::resources::refresh::RefreshMode::Normal,
            }),
        )
        .await
        .unwrap();
        assert!(!result.stable);
        assert_eq!(count, 3);
        let mut published = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if let ScanProgress::Files(files) = event {
                published.push(files);
            }
        }
        // Each attempt published its verified partial batch. The fallback
        // itself must not schedule an additional batch against old hashes.
        assert_eq!(published.len(), 3);
        assert_eq!(result.files[0].sha1, published.last().unwrap()[0].sha1);
        cancel.cancel();
        let error = sync_with_retry_progress(
            &instance,
            &state,
            false,
            ScanConsistency::AllowCommittedSnapshot,
            || std::future::ready(()),
            Some(ScanPublication {
                sender: &sender,
                cancel: &cancel,
                mode: crate::resources::refresh::RefreshMode::Normal,
            }),
        )
        .await;
        assert!(matches!(
            error.unwrap_err().raw.as_ref(),
            crate::ErrorKind::ResourceError(
                crate::resources::ResourceError::Canceled
            )
        ));
        state.pool.close().await;
    }

    #[tokio::test]
    async fn display_scan_does_not_hide_io_or_database_errors() {
        let (_temp, state, _instance, root) = isolated_scan_fixture().await;
        fs::rename(&root, root.with_file_name("moved")).unwrap();
        let result = sync_content_files_for_display("instance", &state).await;
        assert!(result.is_err());
        state.pool.close().await;
        let result = sync_content_files_for_display("instance", &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn display_fallback_can_return_an_empty_committed_snapshot() {
        let (_temp, state, instance, root) = isolated_scan_fixture().await;
        fs::remove_file(root.join("mods/example.jar")).unwrap();
        sync_instance_content_files(&instance, &state)
            .await
            .unwrap();
        let mut count = 0;
        let outcome = sync_with_retry_progress(
            &instance,
            &state,
            false,
            ScanConsistency::AllowCommittedSnapshot,
            || {
                count += 1;
                fs::write(root.join("mods/new.jar"), vec![b'x'; 20 + count])
                    .unwrap();
                std::future::ready(())
            },
            None,
        )
        .await
        .unwrap();
        assert!(!outcome.stable);
        assert_eq!(count, 3);
        assert!(outcome.files.is_empty());
        assert!(
            instance_content_files::get_files(&state.data, "instance")
                .await
                .unwrap()
                .into_iter()
                .all(|file| file.missing)
        );
        state.pool.close().await;
    }

    #[tokio::test]
    async fn cancellation_after_the_last_unstable_attempt_does_not_return_cached_files()
     {
        let (_temp, state, instance, root) = isolated_scan_fixture().await;
        let (sender, _receiver) = tokio::sync::mpsc::channel(32);
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut count = 0;
        let outcome = sync_with_retry_progress(
            &instance,
            &state,
            false,
            ScanConsistency::AllowCommittedSnapshot,
            || {
                count += 1;
                fs::write(
                    root.join("mods/example.jar"),
                    vec![b'x'; 20 + count],
                )
                .unwrap();
                if count == 3 {
                    cancel.cancel();
                }
                std::future::ready(())
            },
            Some(ScanPublication {
                sender: &sender,
                cancel: &cancel,
                mode: crate::resources::refresh::RefreshMode::Normal,
            }),
        )
        .await;
        assert_eq!(count, 3);
        assert!(matches!(
            outcome.unwrap_err().raw.as_ref(),
            crate::ErrorKind::ResourceError(
                crate::resources::ResourceError::Canceled
            )
        ));
        state.pool.close().await;
    }
}
