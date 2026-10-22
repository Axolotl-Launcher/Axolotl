//! Launcher storage snapshots and reference-aware metadata cleanup.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::state::DirectoryInfo;

pub const STORAGE_CACHE_VERSION: u32 = 5;
const STORAGE_CACHE_FILE: &str = "launcher-storage.cbor";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageNodeType {
    Instances,
    Backups,
    Servers,
    SyncedOptions,
    Components,
    LauncherLogs,
    LauncherData,
    Icons,
    InstallRollbacks,
    ContentStore,
    ResourceBackups,
    ResourceCache,
    Staging,
    Folder,
    JavaVersions,
    Versions,
    Libraries,
    Assets,
    Natives,
    LogConfigs,
    GameLocales,
    Resources,
    Logs,
    CrashReports,
    Cache,
    Meta,
    Database,
    Other,
    Instance,
    Mods,
    Replay,
    Resourcepacks,
    Saves,
    World,
    Schematics,
    Screenshots,
    Shaderpacks,
    Minimap,
    #[serde(rename = "distant-horizons")]
    DistantHorizons,
    #[serde(rename = "db-file")]
    DbFile,
    #[serde(rename = "db-backup")]
    DbBackup,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoragePathKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePath {
    pub path: String,
    pub kind: StoragePathKind,
}

#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq,
)]
pub struct StorageSize {
    pub actual: u64,
    pub symlink: u64,
}

impl StorageSize {
    pub fn total(self) -> u64 {
        self.actual.saturating_add(self.symlink)
    }
}

impl std::ops::Add for StorageSize {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            actual: self.actual.saturating_add(rhs.actual),
            symlink: self.symlink.saturating_add(rhs.symlink),
        }
    }
}

impl std::ops::AddAssign for StorageSize {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub for StorageSize {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            actual: self.actual.saturating_sub(rhs.actual),
            symlink: self.symlink.saturating_sub(rhs.symlink),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: StorageNodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "instance_id", skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    pub size: StorageSize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    pub paths: Vec<StoragePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<StorageNode>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageTree {
    pub version: u32,
    #[serde(rename = "scannedAt")]
    pub scanned_at: DateTime<Utc>,
    pub total: StorageSize,
    pub categories: Vec<StorageNode>,
    #[serde(rename = "rootOther")]
    pub root_other: Option<StorageNode>,
    pub external: Vec<StorageNode>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StorageScanEvent {
    Cached {
        tree: StorageTree,
    },
    Progress {
        completed: usize,
        total: usize,
        category: StorageNodeType,
    },
}

mod cache;
mod meta;
#[cfg(test)]
mod tests;
pub(crate) mod usage;

pub use cache::{CacheCleanupResult, clear_cache};
pub use meta::{MetaAnalysis, MetaCleanupResult, analyze_meta, clean_meta};

static SCAN_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StorageCacheFile {
    version: u32,
    roots: String,
    tree: StorageTree,
}

fn decode_snapshot(bytes: &[u8], roots: &str) -> Option<StorageTree> {
    let cache: StorageCacheFile = serde_cbor::from_slice(bytes).ok()?;
    (cache.version == STORAGE_CACHE_VERSION && cache.roots == roots)
        .then_some(cache.tree)
}

fn snapshot_is_fresh(tree: &StorageTree) -> bool {
    let age = Utc::now()
        .signed_duration_since(tree.scanned_at)
        .num_seconds();
    (0..300).contains(&age)
}

fn dirs() -> crate::Result<DirectoryInfo> {
    DirectoryInfo::global_handle_if_ready()
        .map(|directory| DirectoryInfo {
            settings_dir: identity(&directory.settings_dir),
            config_dir: identity(&directory.config_dir),
            app_identifier: directory.app_identifier.clone(),
        })
        .ok_or_else(|| {
            crate::ErrorKind::FSError(
                "Launcher state is not ready for storage scanning".into(),
            )
            .into()
        })
}

fn folder(path: &Path, kind: StorageNodeType) -> StorageNode {
    StorageNode {
        id: path.to_string_lossy().into_owned(),
        node_type: kind,
        name: None,
        instance_id: None,
        size: StorageSize::default(),
        count: None,
        paths: vec![StoragePath {
            path: path.to_string_lossy().into_owned(),
            kind: StoragePathKind::Directory,
        }],
        children: None,
    }
}

fn aggregate(node: &mut StorageNode, children: Vec<StorageNode>) {
    node.size = children
        .iter()
        .fold(StorageSize::default(), |sum, child| sum + child.size);
    node.children = Some(children);
}

fn identity(path: &Path) -> PathBuf {
    crate::util::io::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn ownership_key(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}

fn minimal_roots(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut paths: Vec<_> =
        paths.into_iter().map(|path| identity(&path)).collect();
    paths.sort_by_key(|path| path.components().count());
    let mut roots: Vec<PathBuf> = Vec::new();
    for path in paths {
        if !roots
            .iter()
            .any(|root| ownership_key(&path).starts_with(ownership_key(root)))
        {
            roots.push(path);
        }
    }
    roots
}

/// Traverses each owned subtree once. Links are measured as references and never
/// contribute to the owned byte count, including when the scan starts at a link.
#[async_recursion]
async fn scan_entry(
    path: &Path,
    depth: usize,
    excluded: &HashSet<PathBuf>,
    reference: bool,
    visited: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
) -> Option<StorageNode> {
    if excluded.contains(&ownership_key(path)) {
        return None;
    }
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return None;
        }
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return None;
        }
    };
    let mut node = folder(path, StorageNodeType::Folder);
    node.name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    if crate::util::io::is_symlink_or_reparse(&metadata) {
        let target = match fs::canonicalize(path).await {
            Ok(target) => target,
            Err(error) => {
                warnings.push(format!("{}: {error}", path.display()));
                return Some(node);
            }
        };
        if !visited.insert(ownership_key(&target)) {
            return Some(node);
        }
        if let Some(target_node) =
            scan_entry(&target, 0, excluded, true, visited, warnings).await
        {
            node.size.symlink = target_node.size.total();
        }
        visited.remove(&ownership_key(&target));
        return Some(node);
    }
    if metadata.is_file() {
        node.paths[0].kind = StoragePathKind::File;
        if reference {
            node.size.symlink = metadata.len();
        } else {
            node.size.actual = metadata.len();
        }
        node.count = Some(1);
        return Some(node);
    }
    if !metadata.is_dir() {
        return None;
    }
    let mut entries = match fs::read_dir(path).await {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return Some(node);
        }
    };
    let mut children = Vec::new();
    let mut count = 0;
    loop {
        match entries.next_entry().await {
            Ok(Some(entry)) => {
                if let Some(child) = scan_entry(
                    &entry.path(),
                    depth.saturating_sub(1),
                    excluded,
                    reference,
                    visited,
                    warnings,
                )
                .await
                {
                    node.size += child.size;
                    count += child.count.unwrap_or(0);
                    if depth > 0 && child.size.total() > 0 {
                        children.push(child);
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                warnings.push(format!("{}: {error}", path.display()));
                break;
            }
        }
    }
    node.count = Some(count);
    if !children.is_empty() {
        node.children = Some(children);
    }
    Some(node)
}

async fn scan(
    path: &Path,
    depth: usize,
    excluded: &HashSet<PathBuf>,
    warnings: &mut Vec<String>,
) -> Option<StorageNode> {
    let excluded = excluded.iter().map(|path| ownership_key(path)).collect();
    scan_entry(path, depth, &excluded, false, &mut HashSet::new(), warnings)
        .await
}

fn classify_children(node: &mut StorageNode, category: &StorageNodeType) {
    for child in node.children.as_mut().into_iter().flatten() {
        let name = child.name.as_deref().unwrap_or_default();
        let kind = match (category, name) {
            (StorageNodeType::Meta, "versions") => StorageNodeType::Versions,
            (StorageNodeType::Meta, "libraries") => StorageNodeType::Libraries,
            (StorageNodeType::Meta, "assets") => StorageNodeType::Assets,
            (StorageNodeType::Meta, "natives") => StorageNodeType::Natives,
            (StorageNodeType::Meta, "java_versions") => {
                StorageNodeType::JavaVersions
            }
            (StorageNodeType::Meta, "log_configs") => {
                StorageNodeType::LogConfigs
            }
            (StorageNodeType::Meta, "game-locales") => {
                StorageNodeType::GameLocales
            }
            (StorageNodeType::Meta, "resources") => StorageNodeType::Resources,
            (StorageNodeType::Cache, "content-store") => {
                StorageNodeType::ContentStore
            }
            (StorageNodeType::Cache, "resources") => {
                StorageNodeType::ResourceCache
            }
            (StorageNodeType::Cache, "resource-staging" | "modpack-import") => {
                StorageNodeType::Staging
            }
            (StorageNodeType::Instance, "mods") => StorageNodeType::Mods,
            (StorageNodeType::Instance, "resourcepacks") => {
                StorageNodeType::Resourcepacks
            }
            (StorageNodeType::Instance, "shaderpacks") => {
                StorageNodeType::Shaderpacks
            }
            (StorageNodeType::Instance, "saves") => StorageNodeType::Saves,
            (StorageNodeType::Instance, "schematics") => {
                StorageNodeType::Schematics
            }
            (StorageNodeType::Instance, "screenshots") => {
                StorageNodeType::Screenshots
            }
            (StorageNodeType::Instance, "flashback" | "replay_recordings") => {
                StorageNodeType::Replay
            }
            (
                StorageNodeType::Instance,
                "voxelmap" | "xaero" | "XaeroWaypoints_BACKUP",
            ) => StorageNodeType::Minimap,
            (
                StorageNodeType::Instance,
                ".voxy" | "Distant_Horizons_server_data",
            ) => StorageNodeType::DistantHorizons,
            (_, "logs") => StorageNodeType::Logs,
            (_, "crash-reports") => StorageNodeType::CrashReports,
            _ => StorageNodeType::Folder,
        };
        if kind != StorageNodeType::Folder {
            child.name = None;
        }
        child.node_type = kind;
        child.instance_id.clone_from(&node.instance_id);
        if matches!(
            child.node_type,
            StorageNodeType::Mods
                | StorageNodeType::Resourcepacks
                | StorageNodeType::Shaderpacks
        ) && let Some(entries) = child.children.take()
        {
            let (previous, current): (Vec<_>, Vec<_>) =
                entries.into_iter().partition(|entry| {
                    entry
                        .paths
                        .first()
                        .is_some_and(|path| path.kind == StoragePathKind::File)
                        && entry.name.as_deref().is_some_and(
                            crate::resources::history_cleanup::recognizable,
                        )
                });
            let mut entries = current;
            if !previous.is_empty() {
                let mut backups = folder(
                    Path::new(&child.paths[0].path),
                    StorageNodeType::ResourceBackups,
                );
                backups.id = format!("{}-previous", child.id);
                backups.paths = previous
                    .iter()
                    .flat_map(|entry| entry.paths.clone())
                    .collect();
                aggregate(&mut backups, previous);
                entries.push(backups);
            }
            child.children = Some(entries);
        }
        if child.node_type == StorageNodeType::Saves {
            for world in child.children.as_mut().into_iter().flatten() {
                world.node_type = StorageNodeType::World;
                world.instance_id.clone_from(&node.instance_id);
            }
        }
    }
}

pub async fn scan_storage(force: bool) -> crate::Result<StorageTree> {
    scan_storage_with_progress(force, &|_| {}).await
}

pub async fn scan_storage_with_progress(
    force: bool,
    progress: &(dyn Fn(StorageScanEvent) + Send + Sync),
) -> crate::Result<StorageTree> {
    let _scan = SCAN_LOCK.lock().await;
    let directories = dirs()?;
    let settings =
        crate::state::Settings::get(&crate::State::get().await?.pool).await?;
    let backup = settings
        .backup_repository_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| directories.config_dir.join("Backups/instances"));
    let roots = format!(
        "{}\u{1f}{}\u{1f}{}",
        identity(&directories.settings_dir).display(),
        identity(&directories.config_dir).display(),
        identity(&backup).display()
    );
    let cache_path = directories.caches_dir().join(STORAGE_CACHE_FILE);
    if !force
        && let Ok(bytes) = fs::read(&cache_path).await
        && let Some(tree) = decode_snapshot(&bytes, &roots)
    {
        progress(StorageScanEvent::Cached { tree: tree.clone() });
        if snapshot_is_fresh(&tree) {
            return Ok(tree);
        }
    }
    let tree = scan_with_directories_progress(
        &directories,
        &backup,
        crate::api::instance::list().await?,
        progress,
    )
    .await?;
    let cache = StorageCacheFile {
        version: STORAGE_CACHE_VERSION,
        roots,
        tree: tree.clone(),
    };
    if let Ok(bytes) = serde_cbor::to_vec(&cache)
        && let Err(error) = async {
            fs::create_dir_all(directories.caches_dir()).await?;
            let temp = cache_path
                .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            fs::write(&temp, bytes).await?;
            let result = fs::rename(&temp, &cache_path).await;
            if result.is_err() {
                let _ = fs::remove_file(&temp).await;
            }
            result
        }
        .await
    {
        tracing::warn!(%error, "Storage snapshot could not be cached");
    }
    Ok(tree)
}

#[cfg(test)]
async fn scan_with_directories(
    directories: &DirectoryInfo,
    backup: &Path,
    instances: Vec<crate::state::InstanceMetadata>,
) -> crate::Result<StorageTree> {
    scan_with_directories_progress(directories, backup, instances, &|_| {})
        .await
}

async fn scan_with_directories_progress(
    directories: &DirectoryInfo,
    backup: &Path,
    instances: Vec<crate::state::InstanceMetadata>,
    progress: &(dyn Fn(StorageScanEvent) + Send + Sync),
) -> crate::Result<StorageTree> {
    let mut warnings = Vec::new();
    let mut targets = vec![
        (directories.instances_dir(), StorageNodeType::Instances, 3),
        (directories.caches_dir(), StorageNodeType::Cache, 2),
        (directories.metadata_dir(), StorageNodeType::Meta, 1),
        (backup.to_path_buf(), StorageNodeType::Backups, 1),
        (directories.servers_dir(), StorageNodeType::Servers, 2),
        (
            directories.synced_options_dir(),
            StorageNodeType::SyncedOptions,
            1,
        ),
        (
            directories.install_rollbacks_dir(),
            StorageNodeType::InstallRollbacks,
            1,
        ),
        (directories.icon_dir(), StorageNodeType::Icons, 0),
        (
            directories.config_dir.join("hongshi"),
            StorageNodeType::Components,
            1,
        ),
        (
            directories.config_dir.join("terracotta"),
            StorageNodeType::Components,
            1,
        ),
        (
            directories.settings_dir.join("launcher_logs"),
            StorageNodeType::LauncherLogs,
            0,
        ),
        (
            directories.settings_dir.join("release"),
            StorageNodeType::Database,
            1,
        ),
        (
            directories.settings_dir.join("beta"),
            StorageNodeType::Database,
            1,
        ),
        (
            directories.settings_dir.join("Backups/app-db"),
            StorageNodeType::DbBackup,
            1,
        ),
    ];
    for root in [&directories.settings_dir, &directories.config_dir] {
        for (name, kind, depth) in [
            ("caches", StorageNodeType::Cache, 2),
            ("hongshi", StorageNodeType::Components, 1),
            ("terracotta", StorageNodeType::Components, 1),
            ("install-rollbacks", StorageNodeType::InstallRollbacks, 1),
            ("icons", StorageNodeType::Icons, 0),
            ("instance-shortcut-icons", StorageNodeType::Icons, 0),
            ("backgrounds", StorageNodeType::LauncherData, 1),
            ("app-window-state.json", StorageNodeType::LauncherData, 0),
            ("instance_groups.json", StorageNodeType::LauncherData, 0),
            (
                "native-download-reputation.json",
                StorageNodeType::LauncherData,
                0,
            ),
            ("update-channel.json", StorageNodeType::LauncherData, 0),
            ("download.log", StorageNodeType::LauncherLogs, 0),
        ] {
            let path = root.join(name);
            if !targets.iter().any(|(existing, _, _)| {
                ownership_key(existing) == ownership_key(&path)
            }) {
                targets.push((path, kind, depth));
            }
        }
        for candidate in
            [root.join("Backups"), root.join("Backups").join("instances")]
        {
            if !targets.iter().any(|(path, _, _)| {
                ownership_key(path) == ownership_key(&candidate)
            }) && is_backup_repository(&candidate).await
            {
                targets.push((candidate, StorageNodeType::Backups, 1));
            }
        }
    }
    let mut categories: Vec<StorageNode> = Vec::new();
    let mut claimed = HashSet::new();
    let total_sections = targets.len() + 2;
    for (index, (path, kind, depth)) in targets.iter().enumerate() {
        progress(StorageScanEvent::Progress {
            completed: index,
            total: total_sections,
            category: kind.clone(),
        });
        if !claimed.insert(ownership_key(path)) {
            continue;
        }
        let excluded = targets
            .iter()
            .filter(|(other, _, _)| {
                ownership_key(other) != ownership_key(path)
                    && ownership_key(other).starts_with(ownership_key(path))
            })
            .map(|(other, _, _)| other.clone())
            .collect();
        if let Some(mut node) =
            scan(path, *depth, &excluded, &mut warnings).await
        {
            node.node_type = kind.clone();
            if !matches!(
                kind,
                StorageNodeType::Components | StorageNodeType::Database
            ) {
                node.name = None;
            }
            classify_children(&mut node, kind);
            if *kind == StorageNodeType::Instances {
                for child in node.children.as_mut().into_iter().flatten() {
                    if let Some(instance) = instances.iter().find(|instance| {
                        directories
                            .instances_dir()
                            .join(&instance.instance.path)
                            .to_string_lossy()
                            == child.id
                    }) {
                        child.name = Some(instance.instance.name.clone());
                        child.node_type = StorageNodeType::Instance;
                        child.instance_id = Some(instance.instance.id.clone());
                        classify_children(child, &StorageNodeType::Instance);
                    }
                }
            }
            if node.size.total() == 0 {
                continue;
            }
            if let Some(existing) = categories
                .iter_mut()
                .find(|existing| existing.node_type == *kind)
            {
                if existing.id.starts_with("category-") {
                    existing.paths.extend(node.paths.clone());
                    if node.name.is_none() {
                        node.name = Some(path.to_string_lossy().into_owned());
                    }
                    let mut children =
                        existing.children.take().unwrap_or_default();
                    children.push(node);
                    aggregate(existing, children);
                } else {
                    let mut old = existing.clone();
                    if old.name.is_none() {
                        old.name =
                            old.paths.first().map(|path| path.path.clone());
                    }
                    if node.name.is_none() {
                        node.name = Some(path.to_string_lossy().into_owned());
                    }
                    let mut parent = folder(path, kind.clone());
                    parent.id = format!("category-{kind:?}");
                    parent.paths =
                        old.paths.iter().chain(&node.paths).cloned().collect();
                    aggregate(&mut parent, vec![old, node]);
                    *existing = parent;
                }
            } else {
                categories.push(node);
            }
        }
    }

    let mut remainder = Vec::new();
    progress(StorageScanEvent::Progress {
        completed: targets.len(),
        total: total_sections,
        category: StorageNodeType::Other,
    });
    for root in minimal_roots(vec![
        directories.settings_dir.clone(),
        directories.config_dir.clone(),
        backup.to_path_buf(),
    ]) {
        if let Some(node) = scan(&root, 2, &claimed, &mut warnings).await
            && node.size.total() > 0
        {
            remainder.extend(node.children.unwrap_or_default());
        }
    }
    let root_other = if remainder.is_empty() {
        None
    } else {
        let mut node = folder(&directories.config_dir, StorageNodeType::Other);
        node.id = "root-other".into();
        node.paths = remainder
            .iter()
            .flat_map(|child| child.paths.clone())
            .collect();
        aggregate(&mut node, remainder);
        Some(node)
    };
    let mut external = Vec::new();
    progress(StorageScanEvent::Progress {
        completed: targets.len() + 1,
        total: total_sections,
        category: StorageNodeType::Instance,
    });
    let owned = minimal_roots(vec![
        directories.settings_dir.clone(),
        directories.config_dir.clone(),
        backup.to_path_buf(),
    ]);
    let mut external_roots = Vec::new();
    for instance in &instances {
        let runtime = crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(&instance.instance, directories)?;
        let path = identity(&runtime.game_dir());
        if !owned.iter().any(|root| path.starts_with(root)) {
            external_roots.push(path);
        }
    }
    for path in minimal_roots(external_roots) {
        if let Some(mut node) =
            scan(&path, 2, &HashSet::new(), &mut warnings).await
        {
            let matching: Vec<_> = instances.iter().filter(|instance| {
                crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(&instance.instance, directories).is_ok_and(|runtime| identity(&runtime.game_dir()).starts_with(&path))
            }).collect();
            node.name = Some(
                matching
                    .iter()
                    .map(|instance| instance.instance.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            node.node_type = StorageNodeType::Instance;
            node.instance_id = matching
                .first()
                .map(|instance| instance.instance.id.clone());
            classify_children(&mut node, &StorageNodeType::Instance);
            external.push(node);
        }
    }
    let mut total = categories
        .iter()
        .fold(StorageSize::default(), |sum, node| sum + node.size);
    if let Some(other) = &root_other {
        total += other.size;
    }
    warnings.truncate(100);
    progress(StorageScanEvent::Progress {
        completed: total_sections,
        total: total_sections,
        category: StorageNodeType::Other,
    });
    Ok(StorageTree {
        version: STORAGE_CACHE_VERSION,
        scanned_at: Utc::now(),
        total,
        categories,
        root_other,
        external,
        warnings,
    })
}

async fn is_backup_repository(root: &Path) -> bool {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let path = root.join("backup.db");
    if !fs::try_exists(&path).await.unwrap_or(false) {
        return false;
    }
    let Ok(pool) = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .create_if_missing(false),
        )
        .await
    else {
        return false;
    };
    let result: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('repository_meta', 'backup_objects', 'backup_snapshots')").fetch_one(&pool).await;
    pool.close().await;
    matches!(result, Ok(3))
}
