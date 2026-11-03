use std::{
    fmt,
    path::{Path, PathBuf},
};

use futures::stream::{FuturesUnordered, StreamExt};
use io::IOError;
use serde::{Deserialize, Serialize};
use tempfile::Builder as TempFileBuilder;

use crate::{
    install::{
        InstallPhaseDetails, InstallPhaseId, InstallProgress,
        InstallProgressReporter,
    },
    launcher::download::LocalRuntimeSource,
    state::{
        ModLoader, State,
        instances::{
            adapters::sqlite::instance_rows,
            watcher::{unwatch_instance_folder, watch_instance_folder},
        },
    },
    util::{
        fetch::{self, IoSemaphore},
        io,
    },
};

pub mod atlauncher;
mod axolotl;
pub mod curseforge;
pub(crate) mod direct_link;
pub mod gamedir_resolve;
pub mod gdlauncher;
pub(crate) mod generic;
pub mod hmcl;
pub mod hmcl_config;
pub(crate) mod instance_json;
pub mod mmc;
mod modrinth_app;
mod pcl;
pub use pcl::config_exists;
pub use pcl::read_pcl_registry;
pub mod pe_info;

/// A scanned importable instance with its resolved filesystem path.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportableInstance {
    pub name: String,
    pub path: String,
    /// Whether this instance qualifies for compatible mode import (single
    /// version with a shared `mods/` directory and no version isolation).
    #[serde(default)]
    pub compatible_mode: bool,
    /// For compatible mode: the original version subfolder path containing
    /// the version JSON.  `path` is set to the GameDir, and `version_path`
    /// records `<GameDir>/versions/<name>` so the importer can locate the JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportLauncherType {
    Axolotl,
    MultiMC,
    PrismLauncher,
    ATLauncher,
    GDLauncher,
    Curseforge,
    ModrinthApp,
    PCL2,
    PCL2CE,
    HMCL,
    Generic,
    #[serde(other)]
    Unknown,
}
// impl display
impl fmt::Display for ImportLauncherType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportLauncherType::Axolotl => write!(f, "Axolotl"),
            ImportLauncherType::MultiMC => write!(f, "MultiMC"),
            ImportLauncherType::PrismLauncher => write!(f, "PrismLauncher"),
            ImportLauncherType::ATLauncher => write!(f, "ATLauncher"),
            ImportLauncherType::GDLauncher => write!(f, "GDLauncher"),
            ImportLauncherType::Curseforge => write!(f, "Curseforge"),
            ImportLauncherType::ModrinthApp => {
                write!(f, "Modrinth source installation")
            }
            ImportLauncherType::PCL2 => write!(f, "PCL2"),
            ImportLauncherType::PCL2CE => write!(f, "PCL2CE"),
            ImportLauncherType::HMCL => write!(f, "HMCL"),
            ImportLauncherType::Generic => write!(f, "Generic"),
            ImportLauncherType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Return a list of importable instances from a launcher type and base path,
/// by iterating through the folder and checking each candidate.
pub async fn get_importable_instances(
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
) -> crate::Result<Vec<ImportableInstance>> {
    let instances = match launcher_type {
        ImportLauncherType::Axolotl => get_axolotl_instances(&base_path).await,
        ImportLauncherType::ModrinthApp => {
            get_modrinth_app_instances(&base_path).await
        }
        ImportLauncherType::GDLauncher | ImportLauncherType::ATLauncher => {
            get_instances_subfolder_scan(&base_path, "instances", launcher_type)
                .await
        }
        ImportLauncherType::Curseforge => {
            get_instances_subfolder_scan(&base_path, "Instances", launcher_type)
                .await
        }
        ImportLauncherType::MultiMC => {
            let subpath =
                mmc::get_instances_subpath(base_path.join("multimc.cfg"))
                    .await
                    .unwrap_or_else(|| "instances".to_string());
            get_instances_subfolder_scan(&base_path, &subpath, launcher_type)
                .await
        }
        ImportLauncherType::PrismLauncher => {
            let subpath =
                mmc::get_instances_subpath(base_path.join("prismlauncher.cfg"))
                    .await
                    .unwrap_or_else(|| "instances".to_string());
            get_instances_subfolder_scan(&base_path, &subpath, launcher_type)
                .await
        }
        ImportLauncherType::PCL2 | ImportLauncherType::PCL2CE => {
            get_pcl_instances(&base_path).await
        }
        ImportLauncherType::HMCL => get_hmcl_instances(&base_path).await,
        ImportLauncherType::Generic => get_generic_instances(&base_path).await,
        ImportLauncherType::Unknown => {
            get_unknown_launcher_instances(&base_path).await
        }
    }?;

    // For Generic, compatible mode is already checked inside
    // `get_generic_instances`.  For launchers that use
    // `collect_launcher_instances` (PCL2/PCL2CE, HMCL, MultiMC, etc.),
    // compatible mode is checked per-GameDir inside that function.
    // No global check needed here.

    Ok(instances)
}

async fn check_compatible_mode(instance: &mut ImportableInstance) {
    let path = PathBuf::from(&instance.path);
    let Some(version_name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if parent.file_name().and_then(|n| n.to_str()) != Some("versions") {
        return;
    }
    let Some(game_dir) = parent.parent() else {
        return;
    };
    let mods_dir = game_dir.join("mods");
    let version_dir = game_dir.join("versions").join(version_name);

    let has_mods = mods_dir.is_dir();

    let no_resourcepacks = !version_dir.join("resourcepacks").is_dir();
    let has_valid_json = instance_json::detect(&version_dir).is_some();

    if has_mods && no_resourcepacks && has_valid_json {
        instance.compatible_mode = true;
    }
}

fn finalize_compatible_instance(instance: &mut ImportableInstance) {
    instance.version_path = Some(instance.path.clone());
    if let Some(pos) = instance.name.rfind(':') {
        instance.name = instance.name[pos + 1..].to_string();
    }
    if let Some(stripped) = instance.name.strip_prefix("versions/") {
        instance.name = stripped.to_string();
    }
    let p = PathBuf::from(&instance.path);
    if let Some(game_dir) = p.parent().and_then(|v| v.parent()) {
        instance.path = game_dir.to_string_lossy().to_string();
    }
}

/// Scans a launcher's instances subfolder, validating each candidate with the
/// launcher-specific check.
async fn get_instances_subfolder_scan(
    base_path: &Path,
    instances_subfolder: &str,
    launcher_type: ImportLauncherType,
) -> crate::Result<Vec<ImportableInstance>> {
    let instances_folder = base_path.join(instances_subfolder);
    let mut result = Vec::new();
    let mut dir = io::read_dir(&instances_folder).await.map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "Invalid {launcher_type} launcher path, could not find '{instances_subfolder}' subfolder."
        ))
    })?;
    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e| IOError::with_path(e, &instances_folder))?
    {
        let path = entry.path();
        if path.is_dir()
            && is_valid_importable_instance(path.clone(), launcher_type).await
        {
            let name = path.file_name();
            if let Some(name) = name {
                let name = name.to_string_lossy().to_string();
                result.push(ImportableInstance {
                    path: path.to_string_lossy().to_string(),
                    name,
                    compatible_mode: false,
                    version_path: None,
                });
            }
        }
    }
    Ok(result)
}

/// Collects Modrinth launcher profiles from its internal database.
async fn get_modrinth_app_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    let names =
        modrinth_app::get_importable_instances(base_path.to_path_buf()).await?;
    Ok(names
        .into_iter()
        .map(|n| ImportableInstance {
            name: n.clone(),
            path: n,
            compatible_mode: false,
            version_path: None,
        })
        .collect())
}

/// Collects PCL2 / PCL2CE instances from the registry and CE config,
/// de-duplicating paths that appear in both sources.
async fn get_pcl_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    if !pe_info::folder_has_product(base_path, "Plain Craft Launcher") {
        return Ok(Vec::new());
    }
    let mut collector = InstanceCollector::new();
    // Try both PCL2 registry and PCLCE config — the user may have instances
    // registered in either or both sources.
    if pcl::read_pcl_registry().is_some() {
        collect_launcher_instances(&mut collector, pcl::get_pcl_instances())
            .await;
    }
    if pcl::config_exists() {
        collect_launcher_instances(&mut collector, pcl::get_pclce_instances())
            .await;
    }
    // Also check for a .minecraft folder next to the launcher executable.
    if let Some(entry) = pcl::get_local_dotminecraft(base_path) {
        collect_launcher_instances(&mut collector, vec![entry]).await;
    }
    Ok(collector.instances)
}

/// Collects HMCL instances from its launcher config.
async fn get_hmcl_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    if !hmcl::config_exists(base_path) {
        return Ok(Vec::new());
    }
    let mut collector = InstanceCollector::new();
    collect_launcher_instances(&mut collector, hmcl::get_instances(base_path))
        .await;
    Ok(collector.instances)
}

/// Collects Axolotl instances from a base path. A base path holding
/// `axolotl_config.json` is itself an instance; otherwise direct child
/// folders with their own config are treated as instances.
async fn get_axolotl_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    if base_path.join("axolotl_config.json").is_file() {
        let name = base_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "imported".to_string());
        return Ok(vec![ImportableInstance {
            name,
            path: base_path.to_string_lossy().to_string(),
            compatible_mode: false,
            version_path: None,
        }]);
    }

    let mut instances = Vec::new();
    let mut dir = io::read_dir(base_path).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.is_dir()
            && path.join("axolotl_config.json").is_file()
            && let Some(name) = path.file_name()
        {
            instances.push(ImportableInstance {
                name: name.to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                compatible_mode: false,
                version_path: None,
            });
        }
    }
    Ok(instances)
}

/// Scans a generic launcher folder for instance JSONs.
async fn get_generic_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    let mut scanned = scan_instances_at(base_path, None).await;
    // A folder import commonly targets a launcher or pack directory whose
    // game files live one level below it in `.minecraft`. Treat that nested
    // root exactly like a directly selected `.minecraft` folder, preserving
    // the version-directory path so the importer can resolve its metadata.
    if base_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !name.eq_ignore_ascii_case(".minecraft"))
    {
        scanned.extend(
            scan_instances_at(&base_path.join(".minecraft"), None).await,
        );
    }
    let mut collector = InstanceCollector::new();
    for (name, path) in scanned {
        collector.push(name, path);
    }
    let mut instances = collector.instances;

    if instances.is_empty() && base_path.is_dir() {
        let mut dir = io::read_dir(base_path).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir()
                && instance_json::detect(&path).is_some()
                && let Some(name) = path.file_name()
            {
                instances.push(ImportableInstance {
                    name: name.to_string_lossy().to_string(),
                    path: path.to_string_lossy().to_string(),
                    compatible_mode: false,
                    version_path: None,
                });
            }
        }
    }

    for instance in &mut instances {
        check_compatible_mode(instance).await;
        if instance.compatible_mode {
            finalize_compatible_instance(instance);
        }
    }

    Ok(instances)
}

/// Probes every known launcher type in a folder and merges all found
/// instances, de-duplicating by resolved path.
async fn get_unknown_launcher_instances(
    base_path: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    let mut collector = InstanceCollector::new();

    // PCL2
    if pe_info::folder_has_product(base_path, "Plain Craft Launcher")
        && pcl::read_pcl_registry().is_some()
    {
        collect_launcher_instances(&mut collector, pcl::get_pcl_instances())
            .await;
    }

    // PCL2CE
    if pe_info::folder_has_product(base_path, "Plain Craft Launcher")
        && pcl::config_exists()
    {
        collect_launcher_instances(&mut collector, pcl::get_pclce_instances())
            .await;
    }

    // Also check for a .minecraft folder next to the launcher executable.
    if pe_info::folder_has_product(base_path, "Plain Craft Launcher")
        && let Some(entry) = pcl::get_local_dotminecraft(base_path)
    {
        collect_launcher_instances(&mut collector, vec![entry]).await;
    }

    // HMCL
    if hmcl::config_exists(base_path) {
        collect_launcher_instances(
            &mut collector,
            hmcl::get_instances(base_path),
        )
        .await;
    }

    // ModrinthApp: uses its internal SQLite database; query real physical
    // profile paths for accurate dedup.
    for (iname, ipath) in modrinth_app::get_importable_instances_with_paths(
        base_path.to_path_buf(),
    )
    .await
    .unwrap_or_default()
    {
        collector.push(iname, ipath);
    }

    collect_subfolder_launcher_instances(&mut collector, base_path).await;

    // Generic fallback: scan versions/ subdirectory and base path for
    // instance.json files (handles .minecraft and other unrecognized launchers).
    if collector.instances.is_empty() {
        for (name, path) in scan_instances_at(base_path, None).await {
            collector.instances.push(ImportableInstance {
                name,
                path: path.to_string_lossy().to_string(),
                compatible_mode: false,
                version_path: None,
            });
        }
    }

    collector.instances.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(collector.instances)
}

/// Collects instances from launchers with a dedicated instances subfolder
/// (MultiMC, PrismLauncher, ATLauncher, GDLauncher, CurseForge).
async fn collect_subfolder_launcher_instances(
    collector: &mut InstanceCollector,
    base_path: &Path,
) {
    let other_types = [
        (ImportLauncherType::MultiMC, "multimc.cfg"),
        (ImportLauncherType::PrismLauncher, "prismlauncher.cfg"),
        (ImportLauncherType::ATLauncher, "instances"),
        (ImportLauncherType::GDLauncher, "instances"),
        (ImportLauncherType::Curseforge, "Instances"),
    ];
    for (lt, marker) in other_types {
        let subpath = match lt {
            ImportLauncherType::MultiMC | ImportLauncherType::PrismLauncher => {
                mmc::get_instances_subpath(base_path.join(marker))
                    .await
                    .unwrap_or_else(|| "instances".to_string())
            }
            ImportLauncherType::ATLauncher | ImportLauncherType::GDLauncher => {
                "instances".to_string()
            }
            ImportLauncherType::Curseforge => "Instances".to_string(),
            _ => unreachable!(),
        };
        if let Ok(found) =
            get_instances_subfolder_scan(base_path, &subpath, lt).await
        {
            for inst in found {
                collector.push(inst.name, PathBuf::from(inst.path));
            }
        }
    }
}

/// Runs `scan_instances_at` over every source folder and pushes de-duplicated
/// results into the collector.
async fn collect_launcher_instances(
    collector: &mut InstanceCollector,
    sources: Vec<(String, String)>,
) {
    for (name, dir) in sources {
        let game_dir = PathBuf::from(&dir);
        let mut game_dir_instances = Vec::new();
        for (iname, ipath) in scan_instances_at(&game_dir, Some(&name)).await {
            game_dir_instances.push(ImportableInstance {
                name: iname,
                path: ipath.to_string_lossy().to_string(),
                compatible_mode: false,
                version_path: None,
            });
        }
        for mut inst in game_dir_instances {
            check_compatible_mode(&mut inst).await;
            if inst.compatible_mode {
                finalize_compatible_instance(&mut inst);
            }
            collector.push_instance(inst);
        }
    }
}

/// Collects importable instances while de-duplicating by resolved path.
struct InstanceCollector {
    instances: Vec<ImportableInstance>,
    seen: std::collections::HashSet<PathBuf>,
}

impl InstanceCollector {
    fn new() -> Self {
        Self {
            instances: Vec::new(),
            seen: std::collections::HashSet::new(),
        }
    }

    /// Pushes an instance unless a path with the same resolved path was
    /// already collected.
    fn push(&mut self, name: String, path: PathBuf) {
        self.push_instance(ImportableInstance {
            name,
            path: path.to_string_lossy().to_string(),
            compatible_mode: false,
            version_path: None,
        });
    }

    fn push_instance(&mut self, instance: ImportableInstance) {
        let key = instance
            .version_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&instance.path));
        if self.seen.insert(key) {
            self.instances.push(instance);
        }
    }
}

async fn scan_instances_at(
    path: &Path,
    prefix: Option<&str>,
) -> Vec<(String, PathBuf)> {
    if !path.is_dir() {
        return Vec::new();
    }
    let mut instances = Vec::new();

    // A `versions` container lists its direct children as instances. Keep
    // the name as the plain child folder name so re-resolution does not turn
    // it into `versions/versions/<id>`.
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        collect_child_instances(&mut instances, path, prefix, false).await;
        return instances;
    }

    // A folder that hosts a `versions` directory is a shared game directory
    // (a container), not an instance: its loose JSON files (launcher configs,
    // mod-manager metadata) must not shadow the real version folders as the
    // detected instance. Only fall back to the folder itself when no version
    // folder could be collected.
    let versions_dir = path.join("versions");
    let has_versions_dir = versions_dir.is_dir();
    if has_versions_dir {
        collect_child_instances(&mut instances, &versions_dir, prefix, true)
            .await;
    }
    if instances.is_empty() && instance_json::detect(path).is_some() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "imported".to_string());
        instances.push((
            if let Some(pre) = prefix {
                format!("{pre}:{name}")
            } else {
                name
            },
            path.to_path_buf(),
        ));
    }
    tracing::debug!(
        "scan_instances_at: path={} prefix={:?} found={}",
        path.display(),
        prefix,
        instances.len()
    );
    instances
}

/// Scans direct child folders of `dir` for importable instances and appends
/// them to `instances`. Child names are optionally prefixed with the launcher
/// name and, for a nested `versions/` folder, the `versions/` path segment.
async fn collect_child_instances(
    instances: &mut Vec<(String, PathBuf)>,
    dir: &Path,
    prefix: Option<&str>,
    is_versions_subdir: bool,
) {
    let Ok(mut entries) = io::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir()
            && instance_json::detect(&path).is_some()
            && let Some(name) = path.file_name()
        {
            let name = name.to_string_lossy().to_string();
            let instance_name = if let Some(pre) = prefix {
                if is_versions_subdir {
                    format!("{pre}:versions/{name}")
                } else {
                    format!("{pre}:{name}")
                }
            } else if is_versions_subdir {
                format!("versions/{name}")
            } else {
                name
            };
            instances.push((instance_name, path));
        }
    }
}

fn split_generic_game_dir_and_version(
    path: &Path,
) -> (PathBuf, Option<PathBuf>) {
    let path_buf = path.to_path_buf();
    if let Some(versions_dir) = path_buf.parent()
        && versions_dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
        && let Some(game_dir) = versions_dir.parent()
    {
        return (game_dir.to_path_buf(), Some(path_buf));
    }
    (path_buf, None)
}

/// Everything needed to import one instance, grouped so launcher-specific
/// importers don't need to thread a dozen parameters through every call.
struct ImportJob {
    instance_id: String,
    base_path: PathBuf,
    instance_folder: String,
    instance_path: PathBuf,
    reporter: InstallProgressReporter,
    symlink: bool,
    overrides: ImportOverrides,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ImportOverrides {
    pub game_version: Option<String>,
    pub loader: Option<ModLoader>,
    pub loader_version: Option<String>,
}

pub(crate) async fn import_instance_with_reporter(
    instance_id: &str,
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
    instance_path: Option<String>,
    overrides: ImportOverrides,
    reporter: InstallProgressReporter,
    symlink: bool,
) -> crate::Result<()> {
    let instance_path = PathBuf::from(instance_path.ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Import source path is required".to_string(),
        )
    })?);
    if !instance_path.is_dir() {
        return Err(crate::ErrorKind::InputError(format!(
            "Import source path does not exist: {}",
            instance_path.display()
        ))
        .into());
    }
    import_instance_inner(
        ImportJob {
            instance_id: instance_id.to_string(),
            base_path,
            instance_folder,
            instance_path,
            reporter,
            symlink,
            overrides,
        },
        launcher_type,
    )
    .await
}

async fn import_instance_inner(
    job: ImportJob,
    launcher_type: ImportLauncherType,
) -> crate::Result<()> {
    tracing::debug!(
        "Importing instance from {} (symlink={}, launcher_type={launcher_type})",
        job.instance_folder,
        job.symlink
    );
    let details = InstallPhaseDetails::Import {
        launcher_type,
        instance_folder: job.instance_folder.clone(),
    };
    if launcher_type == ImportLauncherType::Unknown {
        import_unknown_launcher(job).await
    } else {
        import_via_launcher(launcher_type, &job, details).await
    }?;

    tracing::debug!("Completed import.");
    Ok(())
}

/// Runs the launcher-specific import step for a known launcher type.
async fn import_via_launcher(
    launcher_type: ImportLauncherType,
    job: &ImportJob,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let ImportJob {
        instance_id,
        base_path,
        instance_folder,
        instance_path,
        reporter,
        symlink,
        overrides,
    } = job;

    match launcher_type {
        ImportLauncherType::MultiMC | ImportLauncherType::PrismLauncher => {
            mmc::import_mmc_instance_dir(
                instance_path.clone(),
                Some(base_path.join("icons")),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
            )
            .await
        }
        ImportLauncherType::ATLauncher => {
            atlauncher::import_atlauncher_dir(
                base_path.clone(),
                instance_path.clone(),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
            )
            .await
        }
        ImportLauncherType::GDLauncher => {
            gdlauncher::import_gdlauncher(
                instance_path.clone(),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
            )
            .await
        }
        ImportLauncherType::Curseforge => {
            curseforge::import_curseforge(
                instance_path.clone(),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
            )
            .await
        }
        ImportLauncherType::ModrinthApp => {
            modrinth_app::import_instance(
                base_path.clone(),
                instance_folder.clone(),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
            )
            .await
        }
        ImportLauncherType::PCL2 | ImportLauncherType::PCL2CE => {
            let (game_dir, version_path) =
                split_generic_game_dir_and_version(instance_path);
            generic::import_generic(
                game_dir,
                instance_id,
                reporter.clone(),
                details,
                *symlink,
                overrides,
                version_path,
            )
            .await
        }
        ImportLauncherType::HMCL => {
            let (game_dir, version_path) =
                split_generic_game_dir_and_version(instance_path);
            generic::import_generic(
                game_dir,
                instance_id,
                reporter.clone(),
                details,
                *symlink,
                overrides,
                version_path,
            )
            .await
        }
        ImportLauncherType::Axolotl => {
            axolotl::import_axolotl(
                instance_path.clone(),
                instance_id,
                reporter.clone(),
                details,
                *symlink,
                overrides,
            )
            .await
        }
        ImportLauncherType::Generic => {
            let (game_dir, version_dir) =
                split_generic_game_dir_and_version(instance_path);
            generic::import_generic(
                game_dir,
                instance_id,
                reporter.clone(),
                details,
                *symlink,
                overrides,
                version_dir,
            )
            .await
        }
        ImportLauncherType::Unknown => {
            unreachable!("handled by import_instance_inner")
        }
    }
}

/// Tries to identify an unknown launcher folder by probing each known
/// launcher type, then dispatches the matching import.
async fn import_unknown_launcher(job: ImportJob) -> crate::Result<()> {
    let (game_dir, version_path) =
        split_generic_game_dir_and_version(&job.instance_path);
    generic::import_generic(
        game_dir,
        &job.instance_id,
        job.reporter.clone(),
        InstallPhaseDetails::Import {
            launcher_type: ImportLauncherType::Unknown,
            instance_folder: job.instance_folder,
        },
        job.symlink,
        &job.overrides,
        version_path,
    )
    .await
}

/// Returns the default path for the given launcher type
/// None if it can't be found or doesn't exist
pub fn get_default_launcher_path(
    r#type: ImportLauncherType,
) -> Option<PathBuf> {
    let path = match r#type {
        ImportLauncherType::MultiMC => {
            return find_multimc_path();
        }
        ImportLauncherType::PrismLauncher => {
            Some(dirs::data_dir()?.join("PrismLauncher"))
        }
        ImportLauncherType::ATLauncher => {
            Some(dirs::data_dir()?.join("ATLauncher"))
        }
        ImportLauncherType::GDLauncher => {
            Some(dirs::data_dir()?.join("gdlauncher_next"))
        }
        ImportLauncherType::Curseforge => {
            let home = dirs::home_dir()?;
            let primary = home.join("curseforge").join("minecraft");
            if primary.exists() {
                return Some(primary);
            }
            Some(dirs::document_dir()?.join("curseforge").join("minecraft"))
        }
        ImportLauncherType::ModrinthApp => {
            Some(dirs::data_dir()?.join("ModrinthApp"))
        }
        ImportLauncherType::PCL2 => {
            if pcl::read_pcl_registry().is_some() {
                dirs::data_dir()
            } else {
                None
            }
        }
        ImportLauncherType::PCL2CE => {
            if pcl::config_exists() {
                dirs::data_dir()
            } else {
                None
            }
        }
        ImportLauncherType::Axolotl => None,
        ImportLauncherType::HMCL => None,
        ImportLauncherType::Generic => None,
        ImportLauncherType::Unknown => None,
    };
    let path = path?;
    if path.exists() { Some(path) } else { None }
}

/// Searches common locations for a MultiMC installation.
/// MultiMC stores data in its own application directory (not a standard data dir)
fn find_multimc_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Linux/macOS: ~/.local/share/multimc is the typical location
    if let Some(data_dir) = dirs::data_dir() {
        candidates.push(data_dir.join("multimc"));
        candidates.push(data_dir.join("MultiMC"));
    }

    // Windows: check common extraction locations
    #[cfg(target_os = "windows")]
    {
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("MultiMC"));
            candidates.push(home.join("Desktop").join("MultiMC"));
            candidates.push(home.join("Downloads").join("MultiMC"));
        }
        candidates.push(PathBuf::from("C:\\MultiMC"));
        if let Some(program_files) =
            std::env::var_os("ProgramFiles").map(PathBuf::from)
        {
            candidates.push(program_files.join("MultiMC"));
        }
        if let Some(program_files_x86) =
            std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)
        {
            candidates.push(program_files_x86.join("MultiMC"));
        }
    }

    // macOS: MultiMC is a .app bundle with data inside MultiMC.app/Data/
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/Applications/MultiMC.app/Data"));
        if let Some(home) = dirs::home_dir() {
            candidates.push(
                home.join("Applications").join("MultiMC.app").join("Data"),
            );
        }
    }

    candidates
        .into_iter()
        .find(|p| p.join("multimc.cfg").exists())
}

/// Checks if this PathBuf is a valid instance for the given launcher type

#[tracing::instrument]
pub async fn is_valid_importable_instance(
    instance_path: PathBuf,
    r#type: ImportLauncherType,
) -> bool {
    match r#type {
        ImportLauncherType::MultiMC | ImportLauncherType::PrismLauncher => {
            mmc::is_valid_mmc(instance_path).await
        }
        ImportLauncherType::ATLauncher => {
            atlauncher::is_valid_atlauncher(instance_path).await
        }
        ImportLauncherType::GDLauncher => {
            gdlauncher::is_valid_gdlauncher(instance_path).await
        }
        ImportLauncherType::Curseforge => {
            curseforge::is_valid_curseforge(instance_path).await
        }
        ImportLauncherType::ModrinthApp => instance_path.is_dir(),
        ImportLauncherType::Axolotl => {
            instance_path.join("axolotl_config.json").is_file()
        }
        ImportLauncherType::PCL2
        | ImportLauncherType::PCL2CE
        | ImportLauncherType::HMCL
        | ImportLauncherType::Generic => instance_path.is_dir(),
        ImportLauncherType::Unknown => false,
    }
}

/// Caches an image file in the filesystem into the cache directory, and returns the path to the cached file.

#[tracing::instrument]
pub async fn recache_icon(
    icon_path: PathBuf,
) -> crate::Result<Option<PathBuf>> {
    let state = crate::State::get().await?;

    let bytes = tokio::fs::read(&icon_path).await;
    if let Ok(bytes) = bytes {
        let bytes = bytes::Bytes::from(bytes);
        let cache_dir = &state.directories.caches_dir();
        let semaphore = &state.io_semaphore;
        Ok(Some(
            fetch::write_cached_icon(
                &icon_path.to_string_lossy(),
                cache_dir,
                bytes,
                semaphore,
            )
            .await?,
        ))
    } else {
        // could not find icon (for instance, prism default icon, etc)
        Ok(None)
    }
}

pub(crate) async fn copy_dotminecraft_with_reporter(
    instance_id: &str,
    dotminecraft: PathBuf,
    io_semaphore: &IoSemaphore,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;

    let files = collect_dotminecraft_files(&dotminecraft).await?;
    tracing::debug!(
        source = %dotminecraft.display(),
        destination = %instance_path.display(),
        files = files.len(),
        "Copying imported instance content"
    );

    let total = files.len() as u64;
    if total == 0 {
        reporter
            .update(
                InstallPhaseId::PreparingInstance,
                Some(InstallProgress {
                    current: 0,
                    total: 0,
                    secondary: None,
                }),
                details,
            )
            .await?;
        return Ok(());
    }

    copy_files_with_progress(
        &instance_path,
        files,
        io_semaphore,
        reporter,
        details,
    )
    .await
}

/// Collects all files under `.minecraft`, excluding launcher metadata files
/// at the source root (`<dirname>.json` and `<dirname>.jar`).
async fn collect_dotminecraft_files(
    dotminecraft: &Path,
) -> crate::Result<Vec<(PathBuf, PathBuf)>> {
    // Collect all files recursively
    let files = get_all_subfiles(dotminecraft, false).await?;

    // Filter out launcher metadata files at the source root:
    // <dirname>.json (instance config), <dirname>.jar (custom jar override)
    let dirname = dotminecraft
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let skip_json = format!("{dirname}.json");
    let skip_jar = format!("{dirname}.jar");

    let mut collected = Vec::new();
    for abs_path in files {
        let metadata = tokio::fs::symlink_metadata(&abs_path)
            .await
            .map_err(|error| IOError::with_path(error, &abs_path))?;
        if crate::util::io::is_symlink_or_reparse(&metadata) {
            tracing::warn!(
                path = %abs_path.display(),
                "Skipping nested symlink or reparse point while copying an instance"
            );
            continue;
        }
        let Some(rel) = abs_path
            .strip_prefix(dotminecraft)
            .ok()
            .map(Path::to_path_buf)
        else {
            continue;
        };
        if rel
            .parent()
            .is_some_and(|path| !path.as_os_str().is_empty())
        {
            collected.push((abs_path, rel));
            continue;
        }
        let name = rel.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if name != skip_json && name != skip_jar {
            collected.push((abs_path, rel));
        }
    }
    Ok(collected)
}

async fn copy_import_file_atomically(
    src: &Path,
    dest: &Path,
    io_semaphore: &IoSemaphore,
) -> crate::Result<()> {
    let _permit = io_semaphore.0.acquire().await?;
    let parent = dest.parent().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Import destination has no parent directory: {}",
            dest.display()
        ))
    })?;
    io::create_dir_all(parent).await?;

    let temporary = create_import_temp_path(parent, ".axolotl-import-").await?;
    io::copy(src, &temporary).await?;

    let backup = if tokio::fs::try_exists(dest).await? {
        let backup =
            create_import_temp_path(parent, ".axolotl-import-backup-").await?;
        tokio::fs::remove_file(&backup).await?;
        tokio::fs::rename(dest, &backup).await.map_err(|error| {
            crate::ErrorKind::IOError(IOError::with_path(error, dest))
        })?;
        Some(backup)
    } else {
        None
    };

    if let Err(error) = tokio::fs::rename(&temporary, dest).await {
        if let Some(backup) = backup {
            let backup_path = backup.to_path_buf();
            if let Err(recovery_error) = tokio::fs::rename(&backup, dest).await
            {
                let retained_path = match backup.keep() {
                    Ok(path) => path,
                    Err(keep_error) => {
                        return Err(crate::ErrorKind::FSError(format!(
                            "Failed to replace '{}' ({error}) and restore the original file ({recovery_error}); preserving backup '{}' also failed ({keep_error})",
                            dest.display(),
                            backup_path.display(),
                        ))
                        .into());
                    }
                };
                return Err(crate::ErrorKind::FSError(format!(
                    "Failed to replace '{}' ({error}) and restore the original file ({recovery_error}); backup retained at '{}'",
                    dest.display(),
                    retained_path.display(),
                ))
                .into());
            }
        }
        return Err(
            crate::ErrorKind::IOError(IOError::with_path(error, dest)).into()
        );
    }

    if let Some(backup) = backup {
        let _ = tokio::fs::remove_file(backup).await;
    }
    Ok(())
}

async fn create_import_temp_path(
    parent: &Path,
    prefix: &str,
) -> crate::Result<tempfile::TempPath> {
    let parent = parent.to_path_buf();
    let prefix = prefix.to_string();
    Ok(tokio::task::spawn_blocking(move || {
        TempFileBuilder::new()
            .prefix(&prefix)
            .tempfile_in(&parent)
            .map(|file| file.into_temp_path())
            .map_err(|error| {
                crate::ErrorKind::IOError(IOError::with_path(error, &parent))
            })
    })
    .await
    .map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Import temp-file task failed: {error}"
        ))
    })??)
}

/// Copies the collected files into the instance profile concurrently, bounded
/// by the I/O semaphore, reporting progress after every completed file.
async fn copy_files_with_progress(
    instance_path: &Path,
    files: Vec<(PathBuf, PathBuf)>,
    io_semaphore: &IoSemaphore,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let total = files.len() as u64;

    // Build (src, dst) pairs, then copy concurrently bounded by IoSemaphore
    let mut copy_tasks: FuturesUnordered<_> = files
        .into_iter()
        .map(|(src, rel)| {
            let dst = instance_path.join(&rel);
            (src, dst)
        })
        .map(|(src, dst)| {
            async move {
                // Always copy the source file. Size and modification time are not
                // a content identity, and the temporary-file swap prevents a
                // failed copy from leaving a partial destination behind.
                copy_import_file_atomically(&src, &dst, io_semaphore).await?;
                Ok::<(), crate::Error>(())
            }
        })
        .collect();

    let mut completed: u64 = 0;
    let mut first_error = None;
    while let Some(result) = copy_tasks.next().await {
        match result {
            Ok(()) => {
                completed += 1;
                if first_error.is_none()
                    && let Err(error) = reporter
                        .update(
                            InstallPhaseId::PreparingInstance,
                            Some(InstallProgress {
                                current: completed,
                                total,
                                secondary: None,
                            }),
                            details.clone(),
                        )
                        .await
                {
                    first_error = Some(error);
                }
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    // Final 100% report (ensures the bar fills even if reporter throttles the last update)
    reporter
        .update(
            InstallPhaseId::PreparingInstance,
            Some(InstallProgress {
                current: total,
                total,
                secondary: None,
            }),
            details,
        )
        .await?;

    Ok(())
}

#[cfg(test)]
mod import_copy_tests {
    use super::{IoSemaphore, copy_import_file_atomically};
    use std::fs;
    use tempfile::tempdir;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn replaces_existing_file_without_leaving_staging_files() {
        let root = tempdir().unwrap();
        let source = root.path().join("source.bin");
        let destination = root.path().join("instance").join("config.bin");
        fs::write(&source, b"new contents").unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, b"stale contents").unwrap();

        copy_import_file_atomically(
            &source,
            &destination,
            &IoSemaphore(Semaphore::new(1)),
        )
        .await
        .unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"new contents");
        let leftovers: Vec<_> = fs::read_dir(destination.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| {
                name.to_string_lossy().starts_with(".axolotl-import-")
            })
            .collect();
        assert!(leftovers.is_empty(), "staging files remain: {leftovers:?}");
    }
}

/// Determines the real game working directory for an import whose source is a
/// `versions/<name>` folder.
///
/// The game body jar always lives at `<root>/versions/<name>/<name>.jar`
/// regardless of version isolation (that layout has been stable since 1.6), so
/// the jar alone cannot tell the two layouts apart — `dir_has_game_body` only
/// proves "this is a Minecraft game root". The discriminator is where the game
/// *content* (mods/saves/config/…) actually sits:
///
/// - inside the source folder → version-isolated: the version folder itself is
///   the game dir (`<root>/versions/<name>` with its own mods/saves/config);
/// - at an ancestor (normally the `.minecraft` root) → shared install: the
///   `.minecraft` root is the game dir.
///
/// A source with no content anywhere (a fresh, never-launched instance) falls
/// back to the source folder itself: the game creates the content folders
/// there on first run, and for imports the user's explicit game-dir choice
/// (or no override, i.e. the managed symlink) decides the rest.
fn resolve_import_game_root(source: &Path) -> PathBuf {
    // The source is itself the game root: either a whole Minecraft folder that
    // carries a game body, or any folder that already holds game content
    // (a version-isolated `versions/<name>` with mods/saves/config inside).
    if source.is_dir()
        && (dir_has_game_content(source) || dir_has_game_body(source))
    {
        return source.to_path_buf();
    }

    // Otherwise the source is a `versions/<name>` subfolder of a shared
    // install: resolve to the nearest ancestor holding the game content
    // (normally the `.minecraft` root). Never climb past `.minecraft`.
    let mut cursor = source.parent();
    while let Some(dir) = cursor {
        if dir_has_game_content(dir) {
            return dir.to_path_buf();
        }
        if dir.file_name().and_then(|n| n.to_str()) == Some(".minecraft") {
            break;
        }
        cursor = dir.parent();
    }

    source.to_path_buf()
}

/// True if `root` is a Minecraft game root: it has at least one `.jar` game body
/// under `versions/<name>/` (e.g. `versions/1.20.1/1.20.1.jar`).
fn dir_has_game_body(root: &Path) -> bool {
    let versions = root.join("versions");
    let Ok(entries) = std::fs::read_dir(&versions) else {
        return false;
    };
    for version_entry in entries.flatten() {
        if !version_entry.path().is_dir() {
            continue;
        }
        let Ok(version_files) = std::fs::read_dir(version_entry.path()) else {
            continue;
        };
        for file in version_files.flatten() {
            if file.path().extension().is_some_and(|ext| ext == "jar") {
                return true;
            }
        }
    }
    false
}

/// True if `root` directly holds game *content* — the folders/files the running
/// game creates and writes in its working directory (mods, saves, config,
/// resourcepacks, …). Unlike the game body jar (which sits at
/// `<root>/versions/<name>/<name>.jar` in every layout), content appears only in
/// the folder that actually acts as the game dir, so this is what distinguishes
/// a version-isolated `versions/<name>` folder from a shared `.minecraft` root.
fn dir_has_game_content(root: &Path) -> bool {
    [
        "mods",
        "saves",
        "config",
        "resourcepacks",
        "datapacks",
        "shaderpacks",
        "logs",
        "crash-reports",
    ]
    .iter()
    .any(|dir| root.join(dir).is_dir())
        || root.join("options.txt").is_file()
        || root.join("servers.dat").is_file()
}

pub(crate) async fn finish_import(
    instance_id: &str,
    dotminecraft: PathBuf,
    io_semaphore: &IoSemaphore,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
    symlink: bool,
) -> crate::Result<()> {
    let local_source = LocalRuntimeSource::discover(&dotminecraft);

    // Respect an explicitly chosen game-dir override (the user's isolated /
    // not-isolated selection, already stored on the instance row at creation).
    // Only fall back to auto-detection for symlink imports that did not carry
    // an explicit override, so copy imports always stay built-in (no override)
    // and the frontend's choice is never clobbered.
    let state = crate::state::State::get().await?;
    let pool = &state.pool;
    let existing_override =
        instance_rows::get_instance_path_and_game_dir_override_by_id(
            instance_id,
            pool,
        )
        .await?
        .map(|(_, override_dir)| override_dir)
        .unwrap_or(None);
    if existing_override.is_none() && symlink {
        // For a non-version-isolated import the game content (mods, saves, config)
        // lives in the `.minecraft` root, not in the detected `versions/<name>`
        // subfolder. Detect that and record the override so the instance uses the
        // real game root directly instead of an empty version subfolder.
        let game_root = resolve_import_game_root(&dotminecraft);
        if game_root != dotminecraft {
            crate::state::edit_instance(
                instance_id,
                crate::state::EditInstance {
                    game_dir_override: Some(Some(
                        game_root.to_string_lossy().to_string(),
                    )),
                    ..Default::default()
                },
                pool,
            )
            .await?;
        }
    }

    if symlink {
        let state = State::get().await?;
        let relative_path =
            instance_rows::get_instance_path_by_id(instance_id, &state.pool)
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError("Unknown instance".to_string())
                })?;
        // The instance's managed folder lives at instances_dir/<path>. This is
        // where the symlink is created; it must NOT go through the game-dir
        // override (which points at the external .minecraft root).
        let instance_path =
            state.directories.instances_dir().join(&relative_path);

        if instance_path.exists() {
            // The instance folder is registered with the file watcher as soon
            // as the instance row is created. On Windows an active watch keeps
            // an open directory handle, so renaming the folder fails with
            // ERROR_ACCESS_DENIED. Unwatch it first, then re-register once the
            // symlink is in place (or the backup has been restored).
            unwatch_instance_folder(
                &relative_path,
                &instance_path,
                &state.file_watcher,
            )
            .await;

            // Never delete the existing instance directory before the new
            // symlink is in place: a failure between the two would lose the
            // original data. Move it aside, create the link, then clean up
            // the backup; on failure the backup is moved back.
            let name = instance_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("instance");
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            let backup_path = instance_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .join(format!("{name}.bak-{timestamp}"));

            if let Err(error) =
                rename_instance_dir_for_symlink(&instance_path, &backup_path)
                    .await
            {
                watch_instance_folder(
                    instance_id,
                    &relative_path,
                    &instance_path,
                    &state.file_watcher,
                )
                .await;
                return Err(error.into());
            }
            if let Err(error) =
                io::create_symlink(&dotminecraft, &instance_path).await
            {
                let _ = io::rename_or_move(&backup_path, &instance_path).await;
                watch_instance_folder(
                    instance_id,
                    &relative_path,
                    &instance_path,
                    &state.file_watcher,
                )
                .await;
                return Err(error.into());
            }
            let _ = io::remove_dir_all(&backup_path).await;
            watch_instance_folder(
                instance_id,
                &relative_path,
                &instance_path,
                &state.file_watcher,
            )
            .await;
        } else {
            io::create_symlink(&dotminecraft, &instance_path).await?;
        }

        crate::state::edit_instance(
            instance_id,
            crate::state::EditInstance {
                symlink_target: Some(Some(
                    dotminecraft.to_string_lossy().to_string(),
                )),
                ..Default::default()
            },
            &crate::state::State::get().await?.pool,
        )
        .await?;
    } else {
        copy_dotminecraft_with_reporter(
            instance_id,
            dotminecraft,
            io_semaphore,
            reporter.clone(),
            details,
        )
        .await?;
    }

    crate::launcher::install_minecraft_for_instance_id_with_local_source(
        instance_id,
        local_source,
        false,
        Some(reporter),
        crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
    )
    .await?;

    let recognition_instance_id = instance_id.to_string();
    tokio::spawn(async move {
        if let Err(error) = crate::api::curseforge::recognize_instance_files(
            &recognition_instance_id,
        )
        .await
        {
            tracing::warn!(
                instance_id = %recognition_instance_id,
                %error,
                "CurseForge file recognition after import failed"
            );
        }
    });

    Ok(())
}

/// Moves a pre-existing instance directory aside so a symlink can take its
/// place.
///
/// The folder is unwatched just before this runs, but the watcher closes its
/// directory handles asynchronously; on Windows a rename attempted in that
/// window fails with `ERROR_ACCESS_DENIED`. Retry briefly before failing.
async fn rename_instance_dir_for_symlink(
    from: &Path,
    to: &Path,
) -> eyre::Result<()> {
    let mut last_error = None;
    for attempt in 1..=8 {
        match io::rename_or_move(from, to).await {
            Ok(()) => return Ok(()),
            Err(error) if is_busy_dir_error(&error) => {
                tracing::debug!(
                    "Instance directory {from:?} still busy, retrying rename ({attempt}/8)"
                );
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| eyre::eyre!("Instance directory rename failed")))
}

/// Whether a rename error means the destination directory is still busy and
/// the operation may succeed shortly (Windows `ERROR_ACCESS_DENIED` and
/// `ERROR_SHARING_VIOLATION`).
fn is_busy_dir_error(error: &eyre::Report) -> bool {
    #[cfg(windows)]
    {
        error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io_error| {
                    matches!(io_error.raw_os_error(), Some(5 | 32))
                })
        })
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
}

#[async_recursion::async_recursion]
#[tracing::instrument]
pub async fn get_all_subfiles(
    src: &Path,
    include_empty_dirs: bool,
) -> crate::Result<Vec<PathBuf>> {
    let meta = tokio::fs::symlink_metadata(src)
        .await
        .map_err(|e| IOError::with_path(e, src))?;

    // Symlink / reparse point: never follow, treat as leaf file.
    if crate::util::io::is_symlink_or_reparse(&meta) {
        return Ok(vec![src.to_path_buf()]);
    }

    if meta.is_file() {
        return Ok(vec![src.to_path_buf()]);
    }

    let mut files = Vec::new();
    let mut dir = io::read_dir(&src).await?;

    let mut has_files = false;
    while let Some(child) = dir
        .next_entry()
        .await
        .map_err(|e| IOError::with_path(e, src))?
    {
        has_files = true;
        let src_child = child.path();
        files.append(
            &mut get_all_subfiles(&src_child, include_empty_dirs).await?,
        );
    }

    if !has_files && include_empty_dirs {
        files.push(src.to_path_buf());
    }

    Ok(files)
}

#[cfg(test)]
mod import_game_root_tests {
    use super::{dir_has_game_body, resolve_import_game_root};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_game_body(mc: &Path, name: &str) {
        let version_dir = mc.join("versions").join(name);
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(version_dir.join(format!("{name}.jar")), "game").unwrap();
    }

    #[test]
    fn resolves_minecraft_root_when_game_body_is_there() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        let version = mc.join("versions").join("My Pack");
        // Shared install, played as vanilla: game body under the `.minecraft`
        // root and content (saves) there too — a mod-less instance has no
        // `mods/*.jar`, so the content location is what resolves the root.
        write_game_body(&mc, "My Pack");
        fs::create_dir_all(mc.join("saves")).unwrap();

        let resolved = resolve_import_game_root(&version);
        assert_eq!(resolved, mc);
    }

    #[test]
    fn keeps_version_dir_when_it_has_the_content() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        let version = mc.join("versions").join("My Pack");
        // Version-isolated instance: the game body jar ALSO lives under the
        // `.minecraft` root, but the content (mods) is inside the version
        // folder — that folder is the real game dir.
        write_game_body(&mc, "My Pack");
        fs::create_dir_all(version.join("mods")).unwrap();

        let resolved = resolve_import_game_root(&version);
        assert_eq!(resolved, version);
    }

    #[test]
    fn resolves_minecraft_root_for_shared_mods() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        let version = mc.join("versions").join("My Pack");
        write_game_body(&mc, "My Pack");
        // Shared install with mods at the `.minecraft` root only.
        fs::create_dir_all(mc.join("mods")).unwrap();
        fs::write(mc.join("mods/mod-a.jar"), "a").unwrap();

        let resolved = resolve_import_game_root(&version);
        assert_eq!(resolved, mc);
    }

    #[test]
    fn keeps_version_dir_for_fresh_isolated_instance() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        let version = mc.join("versions").join("My Pack");
        // Fresh instance, never launched: only the game body exists, no content
        // folders anywhere. The jar's location is ambiguous (shared vs
        // isolated), so fall back to the innermost folder — the launcher will
        // create content there on first run.
        write_game_body(&mc, "My Pack");

        let resolved = resolve_import_game_root(&version);
        assert_eq!(resolved, version);
    }

    #[test]
    fn keeps_minecraft_root_when_source_is_root() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        write_game_body(&mc, "My Pack");

        let resolved = resolve_import_game_root(&mc);
        assert_eq!(resolved, mc);
    }

    #[test]
    fn no_game_body_falls_back_to_source() {
        let root = tempdir().unwrap();
        let version = root.path().join(".minecraft/versions/My Pack");
        fs::create_dir_all(&version).unwrap();

        let resolved = resolve_import_game_root(&version);
        assert_eq!(resolved, version);
    }

    #[test]
    fn detects_game_body_in_versions() {
        let root = tempdir().unwrap();
        let mc = root.path().join(".minecraft");
        assert!(!dir_has_game_body(&mc));

        write_game_body(&mc, "My Pack");
        assert!(dir_has_game_body(&mc));

        // A mods/ jar alone is not a game body signal.
        let empty = root.path().join("no-versions");
        fs::create_dir_all(empty.join("mods")).unwrap();
        fs::write(empty.join("mods/mod.jar"), "mod").unwrap();
        assert!(!dir_has_game_body(&empty));
    }
}

#[cfg(test)]
mod generic_instance_scan_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn finds_versions_inside_a_selected_folders_minecraft_directory() {
        let folder = tempdir().unwrap();
        let version_dir = folder
            .path()
            .join(".minecraft")
            .join("versions")
            .join("1.20.1-fabric");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("1.20.1-fabric.json"),
            r#"{"id":"1.20.1-fabric","inheritsFrom":"1.20.1"}"#,
        )
        .unwrap();

        let instances = get_generic_instances(folder.path()).await.unwrap();

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].name, "versions/1.20.1-fabric");
        assert_eq!(instances[0].path, version_dir.to_string_lossy());
    }

    #[tokio::test]
    async fn versions_children_shadow_loose_container_jsons() {
        // A shared game directory whose loose JSON files happen to embed
        // loader coordinates (launcher/manager metadata) is a container, not
        // an instance: the `versions` child must be the only candidate, and
        // the container's json must not be detected instead.
        let folder = tempdir().unwrap();
        let mc = folder.path().join(".minecraft");
        let version_dir = mc.join("versions").join("1.12.2-Cleanroom");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            mc.join("axolotl_config.json"),
            r#"{"lastVersion":"net.minecraftforge:forge:1.12.2-14.23.5.2864"}"#,
        )
        .unwrap();
        std::fs::write(
            version_dir.join("1.12.2-Cleanroom.json"),
            r#"{"id":"1.12.2-Cleanroom","libraries":[{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2864"}]}"#,
        )
        .unwrap();

        let instances = get_generic_instances(&mc).await.unwrap();

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].name, "versions/1.12.2-Cleanroom");
        assert_eq!(instances[0].path, version_dir.to_string_lossy());
    }
}
