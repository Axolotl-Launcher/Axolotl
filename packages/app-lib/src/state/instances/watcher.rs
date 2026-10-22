use crate::State;
use crate::event::InstancePayloadType;
use crate::event::emit::{emit_instance, emit_minecraft_crash_warning};
use crate::state::{
    DirectoryInfo, InstanceInstallStage, ProjectType, attached_world_data,
};
use crate::worlds::WorldType;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{RwLock, mpsc::channel};

use super::adapters::sqlite::instance_rows;
use super::config_sync::{CONFIG_FILE_NAME, CONFIG_FILE_TEMP_NAME};

pub struct FileWatcher {
    watcher: RwLock<Debouncer<RecommendedWatcher>>,
    instance_ids: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    content_changes: Arc<RwLock<HashMap<String, InstanceContentChangeState>>>,
    manual_import_directory: Arc<RwLock<Option<PathBuf>>>,
    manual_import_generation: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceContentWatchSnapshot {
    pub epoch: u64,
    pub generation: u64,
    pub dirty_paths: HashSet<String>,
    pub directory_dirty: bool,
}

#[derive(Default)]
struct InstanceContentChangeState {
    epoch: u64,
    generation: u64,
    dirty_paths: HashSet<String>,
    directory_dirty: bool,
    tracked_paths: HashSet<String>,
}

static NEXT_CONTENT_EPOCH: AtomicU64 = AtomicU64::new(1);

pub async fn init_watcher() -> crate::Result<FileWatcher> {
    let (tx, mut rx) = channel(1);
    let instance_ids =
        Arc::new(RwLock::new(HashMap::<String, HashSet<String>>::new()));
    let event_instance_ids = instance_ids.clone();
    let content_changes = Arc::new(RwLock::new(HashMap::<
        String,
        InstanceContentChangeState,
    >::new()));
    let event_content_changes = content_changes.clone();
    let manual_import_directory = Arc::new(RwLock::new(None::<PathBuf>));
    let event_manual_import_directory = manual_import_directory.clone();
    let manual_import_generation = Arc::new(AtomicU64::new(0));

    let file_watcher = new_debouncer(
        Duration::from_secs_f32(1.0),
        move |res: DebounceEventResult| {
            tx.blocking_send(res).ok();
        },
    )?;

    tokio::task::spawn(async move {
        let span = tracing::span!(tracing::Level::INFO, "init_watcher");
        tracing::info!(parent: &span, "Initing watcher");
        while let Some(res) = rx.recv().await {
            let _span = span.enter();

            match res {
                Ok(events) => {
                    let instance_ids = event_instance_ids.read().await;
                    let manual_import_directory =
                        event_manual_import_directory.read().await.clone();
                    let mut visited_instances = Vec::new();
                    let mut synced_option_files =
                        HashMap::<String, HashSet<String>>::new();
                    let mut scan_manual_downloads = false;

                    for e in &events {
                        let direct_instances = instance_ids
                            .iter()
                            .filter_map(|(root, instance_ids)| {
                                let root = Path::new(root);
                                root.is_absolute()
                                    .then_some((root, instance_ids))
                                    .filter(|(root, _)| {
                                        e.path.starts_with(root)
                                    })
                            })
                            .max_by_key(|(root, _)| root.components().count())
                            .map(|(root, instance_ids)| {
                                (root.to_path_buf(), instance_ids.clone())
                            });
                        let mut instance_path = direct_instances
                            .as_ref()
                            .and_then(|(root, _)| root.file_name());

                        let mut found = false;
                        for component in e.path.components() {
                            if found {
                                instance_path = Some(component.as_os_str());
                                break;
                            }

                            if component.as_os_str()
                                == crate::state::dirs::INSTANCES_FOLDER_NAME
                            {
                                found = true;
                            }
                        }

                        if let Some(instance_path) = instance_path {
                            let instance_path_str =
                                instance_path.to_string_lossy().to_string();
                            let Some(instance_ids) = direct_instances
                                .as_ref()
                                .map(|(_, instance_ids)| instance_ids.clone())
                                .or_else(|| {
                                    instance_ids
                                        .get(&instance_path_str)
                                        .cloned()
                                })
                            else {
                                continue;
                            };
                            for instance_id in instance_ids {
                                let first_file_name = e
                                    .path
                                    .components()
                                    .skip_while(|x| {
                                        x.as_os_str() != instance_path
                                    })
                                    .nth(1)
                                    .map(|x| x.as_os_str());
                                if let Some(file_name) = first_file_name
                                    .as_ref()
                                    .and_then(|name| name.to_str())
                                    .filter(|name| {
                                        matches!(
                                            *name,
                                            "command_history.txt"
                                                | "hotbar.nbt"
                                                | "options.txt"
                                                | "servers.dat"
                                                | "resourcepacks"
                                                | "datapacks"
                                        )
                                    })
                                {
                                    synced_option_files
                                        .entry(instance_id.clone())
                                        .or_default()
                                        .insert(file_name.to_owned());
                                }
                                let relative_path = e
                                    .path
                                    .components()
                                    .skip_while(|x| {
                                        x.as_os_str() != instance_path
                                    })
                                    .skip(1)
                                    .map(|component| {
                                        component.as_os_str().to_string_lossy()
                                    })
                                    .collect::<Vec<_>>()
                                    .join("/");
                                if first_file_name
                                    .as_ref()
                                    .is_some_and(|x| *x == "screenshots")
                                {
                                    let screenshot_instance_id =
                                        instance_id.clone();
                                    tokio::spawn(async move {
                                        if let Err(error) = crate::api::instance::reconcile_screenshots(&screenshot_instance_id).await {
                                        tracing::debug!(%error, "Screenshot index reconciliation failed");
                                    }
                                    });
                                }
                                if !relative_path.is_empty() {
                                    record_upgrade_content_change(
                                        &event_content_changes,
                                        &instance_id,
                                        &relative_path,
                                    )
                                    .await;
                                }
                                if first_file_name
                                    .is_some_and(is_config_sync_file_name)
                                {
                                    continue;
                                }
                                let is_crash_report = first_file_name
                                    .as_ref()
                                    .is_some_and(|x| *x == "crash-reports")
                                    && e.path
                                        .extension()
                                        .as_ref()
                                        .is_some_and(|x| *x == "txt");
                                let is_jvm_crash =
                                    first_file_name.as_ref().is_some_and(|x| {
                                        x.to_string_lossy()
                                            .starts_with("hs_err_pid")
                                    }) && e
                                        .path
                                        .extension()
                                        .as_ref()
                                        .is_some_and(|x| *x == "log");
                                if is_crash_report || is_jvm_crash {
                                    crash_task(instance_id);
                                } else if !visited_instances
                                    .contains(&instance_id)
                                {
                                    let event = if first_file_name
                                        .as_ref()
                                        .is_some_and(|x| *x == "servers.dat")
                                    {
                                        Some(
                                            InstancePayloadType::ServersUpdated,
                                        )
                                    } else if first_file_name
                                        .as_ref()
                                        .is_some_and(|x| *x == "screenshots")
                                    {
                                        Some(
                                        InstancePayloadType::ScreenshotsUpdated,
                                    )
                                    } else if first_file_name
                                        .as_ref()
                                        .is_some_and(|x| {
                                            *x == "saves"
                                                && e.path
                                                    .file_name()
                                                    .as_ref()
                                                    .is_some_and(|x| {
                                                        *x == "level.dat"
                                                    })
                                        })
                                    {
                                        tracing::info!(
                                            "World updated: {}",
                                            e.path.display()
                                        );
                                        let world = e
                                            .path
                                            .parent()
                                            .unwrap()
                                            .file_name()
                                            .unwrap()
                                            .to_string_lossy()
                                            .to_string();
                                        if !e.path.is_file() {
                                            let instance_id =
                                                instance_id.clone();
                                            let world = world.clone();
                                            tokio::spawn(async move {
                                                if let Ok(state) = State::get().await
												&& let Err(e) = attached_world_data::AttachedWorldData::remove_for_world(
													&instance_id,
													WorldType::Singleplayer,
													&world,
													&state.pool
												).await {
													tracing::warn!("Failed to remove AttachedWorldData for '{world}': {e}")
												}
                                            });
                                        }
                                        Some(
                                            InstancePayloadType::WorldUpdated {
                                                world,
                                            },
                                        )
                                    } else if first_file_name
                                        .as_ref()
                                        .is_none_or(|x| *x != "saves")
                                    {
                                        Some(InstancePayloadType::Synced)
                                    } else {
                                        None
                                    };
                                    if let Some(event) = event {
                                        let emit_instance_id =
                                            instance_id.clone();
                                        tokio::spawn(async move {
                                            let _ = emit_instance(
                                                &emit_instance_id,
                                                event,
                                            )
                                            .await;
                                        });
                                        visited_instances.push(instance_id);
                                    }
                                }
                            }
                        } else if manual_import_directory.as_ref().is_some_and(
                            |directory| e.path.starts_with(directory),
                        ) {
                            scan_manual_downloads = true;
                        }
                    }
                    for (instance_id, file_names) in synced_option_files {
                        tokio::spawn(async move {
                            for file_name in file_names {
                                if let Err(error) =
                                    crate::api::instance::reconcile_synced_option_file(
                                        &instance_id,
                                        &file_name,
                                    )
                                    .await
                                {
                                    tracing::error!(
                                        "Failed to reconcile {file_name} after filesystem change: {error}"
                                    );
                                }
                            }
                        });
                    }
                    if scan_manual_downloads
                        && let Some(directory) = manual_import_directory
                    {
                        tokio::spawn(async move {
                            if let Err(error) =
                                crate::api::curseforge::scan_pending_manual_downloads_in(
                                    &directory,
                                )
                                .await
                            {
                                tracing::warn!(
                                    "Unable to scan pending manual downloads: {error}"
                                );
                            }
                        });
                    }
                }
                Err(error) => tracing::warn!("Unable to watch file: {error}"),
            }
        }
    });

    Ok(FileWatcher {
        watcher: RwLock::new(file_watcher),
        instance_ids,
        content_changes,
        manual_import_directory,
        manual_import_generation,
    })
}

impl FileWatcher {
    pub(crate) async fn track_upgrade_source(
        &self,
        instance_id: &str,
        paths: impl IntoIterator<Item = String>,
    ) -> Option<InstanceContentWatchSnapshot> {
        let mut changes = self.content_changes.write().await;
        let change = changes.get_mut(instance_id)?;
        change.tracked_paths.extend(paths);
        Some(change.snapshot())
    }

    pub(crate) async fn content_watch_snapshot(
        &self,
        instance_id: &str,
    ) -> Option<InstanceContentWatchSnapshot> {
        self.content_changes
            .read()
            .await
            .get(instance_id)
            .map(InstanceContentChangeState::snapshot)
    }

    pub(crate) async fn clear_content_changes_if_generation(
        &self,
        instance_id: &str,
        generation: u64,
    ) {
        let mut changes = self.content_changes.write().await;
        let Some(change) = changes.get_mut(instance_id) else {
            return;
        };
        if change.generation == generation {
            change.dirty_paths.clear();
            change.directory_dirty = false;
        }
    }

    #[cfg(test)]
    pub(crate) async fn record_upgrade_content_change(
        &self,
        instance_id: &str,
        relative_path: &str,
    ) {
        record_upgrade_content_change(
            &self.content_changes,
            instance_id,
            relative_path,
        )
        .await;
    }

    pub(crate) async fn configure_manual_import_directory(
        &self,
        directory: Option<PathBuf>,
    ) -> crate::Result<()> {
        let current = self.manual_import_directory.read().await.clone();
        if current == directory {
            return Ok(());
        }

        let mut debouncer = self.watcher.write().await;
        if let Some(directory) = directory.as_ref() {
            debouncer
                .watcher()
                .watch(directory, RecursiveMode::NonRecursive)?;
        }
        if let Some(current) = current.as_ref() {
            let _ = debouncer.watcher().unwatch(current);
        }
        *self.manual_import_directory.write().await = directory;
        let generation = self
            .manual_import_generation
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        let Some(directory) = self.manual_import_directory.read().await.clone()
        else {
            return Ok(());
        };
        let active_generation = self.manual_import_generation.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            interval.set_missed_tick_behavior(
                tokio::time::MissedTickBehavior::Skip,
            );
            loop {
                interval.tick().await;
                if active_generation.load(Ordering::Relaxed) != generation {
                    break;
                }
                if let Err(error) =
                    crate::api::curseforge::scan_pending_manual_downloads_in(
                        &directory,
                    )
                    .await
                {
                    tracing::warn!(
                        "Unable to poll pending manual downloads: {error}"
                    );
                }
            }
        });
        Ok(())
    }
}

pub(crate) async fn watch_instances_init(
    watcher: &FileWatcher,
    dirs: &DirectoryInfo,
    pool: &sqlx::SqlitePool,
) {
    let Ok(instances) = instance_rows::list_instances(pool).await else {
        return;
    };

    for instance in instances {
        let Ok(content_root) =
            crate::state::instances::commands::instance_content_root(
                dirs, &instance,
            )
        else {
            tracing::warn!(
                instance_id = instance.id,
                "Unable to resolve instance content root for watcher"
            );
            continue;
        };
        watch_instance_folder(
            &instance.id,
            &instance.path,
            &content_root,
            watcher,
        )
        .await;
        let screenshot_instance_id = instance.id.clone();
        tokio::spawn(async move {
            if let Err(error) = crate::api::instance::reconcile_screenshots(
                &screenshot_instance_id,
            )
            .await
            {
                tracing::debug!(%error, "Initial screenshot index reconciliation failed");
            }
        });
        let synced_instance_id = instance.id.clone();
        tokio::spawn(async move {
            if let Err(error) =
                crate::api::instance::synced_options::reconcile_instance(
                    &synced_instance_id,
                )
                .await
            {
                tracing::debug!(%error, "Initial synced option reconciliation failed");
            }
        });
    }
}

pub(crate) async fn watch_instance_folder(
    instance_id: &str,
    instance_path: &str,
    full_instance_path: &Path,
    watcher: &FileWatcher,
) {
    let Ok(metadata) = tokio::fs::metadata(full_instance_path).await else {
        return;
    };

    if !metadata.is_dir() {
        return;
    }

    let mut to_watch = Vec::new();
    for full_path in instance_watch_paths(full_instance_path) {
        if full_path == full_instance_path {
            // The root is watched non-recursively after the subfolders.
            continue;
        }
        let meta = tokio::fs::symlink_metadata(&full_path).await;
        let exists = meta.is_ok();
        let is_symlink = meta.ok().is_some_and(|m| m.file_type().is_symlink());
        let sub_path = full_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if !exists
            && !is_symlink
            && !sub_path.contains('.')
            && let Err(e) = crate::util::io::create_dir_all(&full_path).await
        {
            tracing::error!(
                "Failed to create directory for watcher {full_path:?}: {e}"
            );
            return;
        }

        to_watch.push(full_path);
    }

    let full_instance_path_key =
        full_instance_path.to_string_lossy().to_string();
    let mut debouncer = watcher.watcher.write().await;
    let is_watched = watcher
        .instance_ids
        .read()
        .await
        .contains_key(&full_instance_path_key);
    if !is_watched {
        for full_path in &to_watch {
            if let Err(e) = debouncer
                .watcher()
                .watch(full_path, RecursiveMode::Recursive)
            {
                tracing::error!(
                    "Failed to watch directory for watcher {full_path:?}: {e}"
                );
                return;
            }
        }

        if let Err(e) = debouncer
            .watcher()
            .watch(full_instance_path, RecursiveMode::NonRecursive)
        {
            tracing::error!(
                "Failed to watch root instance directory for watcher {full_instance_path:?}: {e}"
            );
        }
    }

    let mut instance_ids = watcher.instance_ids.write().await;
    instance_ids
        .entry(instance_path.to_string())
        .or_default()
        .insert(instance_id.to_string());
    instance_ids
        .entry(full_instance_path_key)
        .or_default()
        .insert(instance_id.to_string());
    drop(instance_ids);
    drop(debouncer);
    watcher
        .content_changes
        .write()
        .await
        .insert(instance_id.to_string(), new_instance_content_change_state());
}

/// Stops watching an instance folder and forgets its instance-id mapping.
///
/// Used when the instance folder is about to be renamed or replaced. On
/// Windows an active watch keeps an open directory handle, which blocks
/// renaming the folder with `ERROR_ACCESS_DENIED`; the folder must be
/// unwatched first and re-registered afterwards.
pub(crate) async fn unwatch_instance_folder(
    instance_path: &str,
    full_instance_path: &Path,
    watcher: &FileWatcher,
) {
    let full_instance_path_key =
        full_instance_path.to_string_lossy().to_string();
    let mut debouncer = watcher.watcher.write().await;
    let (removed_instance_ids, should_unwatch) = {
        let mut mappings = watcher.instance_ids.write().await;
        let removed_instance_ids =
            mappings.remove(instance_path).unwrap_or_default();
        let mut should_unwatch = false;
        if let Some(shared_ids) = mappings.get_mut(&full_instance_path_key) {
            for instance_id in &removed_instance_ids {
                shared_ids.remove(instance_id);
            }
            if shared_ids.is_empty() {
                mappings.remove(&full_instance_path_key);
                should_unwatch = true;
            }
        }
        (removed_instance_ids, should_unwatch)
    };
    if should_unwatch {
        for full_path in instance_watch_paths(full_instance_path) {
            let _ = debouncer.watcher().unwatch(&full_path);
        }
    }
    drop(debouncer);
    for instance_id in removed_instance_ids {
        watcher.content_changes.write().await.remove(&instance_id);
    }
}

impl InstanceContentChangeState {
    fn snapshot(&self) -> InstanceContentWatchSnapshot {
        InstanceContentWatchSnapshot {
            epoch: self.epoch,
            generation: self.generation,
            dirty_paths: self.dirty_paths.clone(),
            directory_dirty: self.directory_dirty,
        }
    }
}

fn new_instance_content_change_state() -> InstanceContentChangeState {
    InstanceContentChangeState {
        epoch: NEXT_CONTENT_EPOCH.fetch_add(1, Ordering::Relaxed),
        ..InstanceContentChangeState::default()
    }
}

fn is_upgrade_content_change(
    relative_path: &str,
    tracked_paths: &HashSet<String>,
) -> bool {
    let normalized = relative_path.replace('\\', "/");
    let top_level = normalized.split('/').next().unwrap_or_default();
    matches!(
        top_level,
        "mods" | "resourcepacks" | "shaderpacks" | "datapacks"
    ) || tracked_paths.contains(&normalized)
}

async fn record_upgrade_content_change(
    content_changes: &RwLock<HashMap<String, InstanceContentChangeState>>,
    instance_id: &str,
    relative_path: &str,
) {
    let mut content_changes = content_changes.write().await;
    if let Some(change) = content_changes.get_mut(instance_id)
        && is_upgrade_content_change(relative_path, &change.tracked_paths)
    {
        change.generation = change.generation.wrapping_add(1);
        change.dirty_paths.insert(relative_path.replace('\\', "/"));
        change.directory_dirty = true;
    }
}

/// All paths `watch_instance_folder` registers for a single instance,
/// including the root, so `unwatch_instance_folder` can release them again.
fn instance_watch_paths(full_instance_path: &Path) -> Vec<PathBuf> {
    // `saves` is both a ProjectType folder and part of the crash-report
    // extras; deduplicate so watch/unwatch stay symmetric (a leftover watch
    // handle on a subfolder keeps Windows from renaming the instance root).
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for sub in ProjectType::iterator().map(|x| x.get_folder()).chain([
        "crash-reports",
        "saves",
        "screenshots",
    ]) {
        let full_path = full_instance_path.join(sub);
        if seen.insert(full_path.clone()) {
            paths.push(full_path);
        }
    }
    paths.push(full_instance_path.to_path_buf());
    paths
}

fn crash_task(instance_id: String) {
    tokio::task::spawn(async move {
        let res = async {
            let state = State::get().await?;
            let Some(instance) =
                instance_rows::get_instance_by_id(&instance_id, &state.pool)
                    .await?
            else {
                return Ok(());
            };

            if instance.install_stage == InstanceInstallStage::Installed {
                emit_minecraft_crash_warning(&instance_id, &instance.name)
                    .await?;
            }

            Ok::<(), crate::Error>(())
        }
        .await;

        match res {
            Ok(()) => {}
            Err(err) => {
                tracing::warn!("Unable to send crash report to frontend: {err}")
            }
        };
    });
}

fn is_config_sync_file_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name == CONFIG_FILE_NAME || name == CONFIG_FILE_TEMP_NAME
}

#[cfg(test)]
mod shared_root_tests {
    use super::*;

    #[tokio::test]
    async fn shared_root_keeps_remaining_instance_watched() {
        let root = tempfile::tempdir().unwrap();
        let watcher = init_watcher().await.unwrap();
        let root_key = root.path().to_string_lossy().to_string();

        watch_instance_folder("instance-one", "one", root.path(), &watcher)
            .await;
        watch_instance_folder("instance-two", "two", root.path(), &watcher)
            .await;

        assert_eq!(
            watcher
                .instance_ids
                .read()
                .await
                .get(&root_key)
                .cloned()
                .unwrap_or_default(),
            HashSet::from([
                "instance-one".to_string(),
                "instance-two".to_string(),
            ])
        );

        unwatch_instance_folder("one", root.path(), &watcher).await;

        assert_eq!(
            watcher
                .instance_ids
                .read()
                .await
                .get(&root_key)
                .cloned()
                .unwrap_or_default(),
            HashSet::from(["instance-two".to_string()])
        );
        assert!(
            watcher
                .content_watch_snapshot("instance-one")
                .await
                .is_none()
        );
        assert!(
            watcher
                .content_watch_snapshot("instance-two")
                .await
                .is_some()
        );

        unwatch_instance_folder("two", root.path(), &watcher).await;
        assert!(watcher.instance_ids.read().await.get(&root_key).is_none());
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn watched_instance_folder_cannot_be_renamed_on_windows() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DirectoryInfo {
            settings_dir: temp.path().to_path_buf(),
            config_dir: temp.path().to_path_buf(),
            app_identifier: "test".to_string(),
        };
        let watcher = init_watcher().await.unwrap();
        let instance_path = "watched-instance";
        let full_path = dirs.instances_dir().join(instance_path);
        std::fs::create_dir_all(&full_path).unwrap();

        watch_instance_folder(
            "instance-1",
            instance_path,
            &full_path,
            &watcher,
        )
        .await;

        // On Windows, an active watch keeps a directory handle open and blocks
        // renaming the instance folder (ERROR_ACCESS_DENIED). This is the
        // failure the symlink import used to hit.
        let rename_result = std::fs::rename(
            &full_path,
            temp.path().join("watched-instance.bak"),
        );
        assert!(
            rename_result.is_err(),
            "a watched folder must not be renameable on Windows"
        );

        // The import flow unwatches the folder first; after that the rename
        // must succeed (the watcher closes its handles asynchronously, so a
        // short retry window is needed).
        unwatch_instance_folder(instance_path, &full_path, &watcher).await;

        let mut renamed = false;
        for _ in 0..20 {
            match std::fs::rename(
                &full_path,
                temp.path().join("watched-instance.bak"),
            ) {
                Ok(()) => {
                    renamed = true;
                    break;
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => panic!("unexpected rename error: {error:?}"),
            }
        }
        assert!(
            renamed,
            "rename should succeed after the folder is unwatched"
        );

        drop(watcher);
    }
}

#[cfg(test)]
mod config_file_name_tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn recognizes_sync_config_files_but_not_other_instance_events() {
        assert!(is_config_sync_file_name(OsStr::new("axolotl_config.json")));
        assert!(is_config_sync_file_name(OsStr::new(
            "axolotl_config.json.tmp"
        )));
        assert!(!is_config_sync_file_name(OsStr::new("mods")));
        assert!(!is_config_sync_file_name(OsStr::new("servers.dat")));
    }

    #[tokio::test]
    async fn content_change_tracking_ignores_unrelated_paths() {
        let watcher = init_watcher().await.unwrap();
        watcher.content_changes.write().await.insert(
            "instance".to_string(),
            new_instance_content_change_state(),
        );
        watcher
            .track_upgrade_source(
                "instance",
                ["schematics/existing.schem".to_string()],
            )
            .await;

        watcher
            .record_upgrade_content_change("instance", "config/options.txt")
            .await;
        watcher
            .record_upgrade_content_change(
                "instance",
                "schematics/existing.schem",
            )
            .await;
        watcher
            .record_upgrade_content_change("instance", "mods/new.jar")
            .await;

        let snapshot =
            watcher.content_watch_snapshot("instance").await.unwrap();
        assert_eq!(snapshot.generation, 2);
        assert!(snapshot.dirty_paths.contains("mods/new.jar"));
        assert!(snapshot.dirty_paths.contains("schematics/existing.schem"));
        assert!(!snapshot.dirty_paths.contains("config/options.txt"));
    }

    #[tokio::test]
    async fn concurrent_content_notifications_do_not_lose_generation() {
        let watcher = Arc::new(init_watcher().await.unwrap());
        watcher.content_changes.write().await.insert(
            "instance".to_string(),
            new_instance_content_change_state(),
        );
        let mut tasks = Vec::new();
        for index in 0..32 {
            let watcher = Arc::clone(&watcher);
            tasks.push(tokio::spawn(async move {
                watcher
                    .record_upgrade_content_change(
                        "instance",
                        &format!("mods/{index}.jar"),
                    )
                    .await;
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }

        let snapshot =
            watcher.content_watch_snapshot("instance").await.unwrap();
        assert_eq!(snapshot.generation, 32);
        assert_eq!(snapshot.dirty_paths.len(), 32);
    }

    #[tokio::test]
    async fn watcher_reinitialization_changes_content_epoch() {
        let first = init_watcher().await.unwrap();
        let second = init_watcher().await.unwrap();
        first.content_changes.write().await.insert(
            "instance".to_string(),
            new_instance_content_change_state(),
        );
        second.content_changes.write().await.insert(
            "instance".to_string(),
            new_instance_content_change_state(),
        );
        assert_ne!(
            first
                .content_watch_snapshot("instance")
                .await
                .unwrap()
                .epoch,
            second
                .content_watch_snapshot("instance")
                .await
                .unwrap()
                .epoch
        );
    }
}
