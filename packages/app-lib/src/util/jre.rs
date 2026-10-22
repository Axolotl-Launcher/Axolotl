use super::{io, java_environment};
use crate::state::JavaVersion;
use futures::prelude::*;
use std::collections::VecDeque;
use std::env;
use std::path::PathBuf;
use std::{collections::HashSet, path::Path};
use tokio::task::JoinError;

use crate::{State, get_resource_file};
#[cfg(target_os = "windows")]
use winreg::{
    RegKey,
    enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
};

/// Maximum directory levels below a search root that the keyword BFS
/// descends into.
const BFS_MAX_DEPTH: usize = 5;
/// Upper bound of directories examined per search root, as a guard against
/// pathological directory trees.
const BFS_MAX_DIRS_PER_ROOT: usize = 10_000;
/// How many blocking collection jobs (registry, BFS roots, ...) run at once.
const COLLECT_CONCURRENCY: usize = 4;
const JAVA_INSTALL_STAGING_SUFFIX: &str = ".installing";

/// Directory names (lowercase, substring match) that make the BFS descend
/// into a top-level directory of a search root.
const DIR_NAME_KEYWORDS: &[&str] = &[
    "java",
    "jdk",
    "jre",
    "dragonwell",
    "azul",
    "zulu",
    "oracle",
    "open",
    "amazon",
    "corretto",
    "eclipse",
    "temurin",
    "hotspot",
    "semeru",
    "kona",
    "bellsoft",
    "liberica",
    "graal",
    "sdkman",
    "environment",
    "env",
    "runtime",
    "x86_64",
    "amd64",
    "arm64",
    "minecraft",
    "launcher",
    "hmcl",
];

/// Directory names (lowercase, substring match) that the BFS never descends
/// into, at any depth.
const EXCLUDED_DIR_NAMES: &[&str] = &[
    "javapath",
    "java8path",
    "common files",
    "netease",
    "node_modules",
    "assets",
    "libraries",
    "resourcepacks",
    "shaderpacks",
    "screenshots",
    "saves",
    "logs",
    "crash-reports",
    "cache",
    "mods",
    "versions",
    ".git",
];

// Entrypoint function
// Returns a Vec of unique JavaVersions collected from the PATH, JAVA_HOME,
// OS-specific locations (registry keys, common install directories), other
// launchers' bundled runtimes and a bounded keyword search of likely
// directories
#[tracing::instrument]
pub async fn get_all_jre(
    full_scan: bool,
    exhaustive: bool,
) -> Result<Vec<JavaVersion>, JREError> {
    let jre_paths = collect_candidate_paths(full_scan, exhaustive).await?;

    // Get JRE versions from potential paths concurrently
    Ok(check_java_at_filepaths(jre_paths)
        .await
        .into_iter()
        .collect())
}

// Gathers candidate paths from every source; cheap sources run inline while
// filesystem-heavy sources run on blocking threads with bounded concurrency
async fn collect_candidate_paths(
    full_scan: bool,
    exhaustive: bool,
) -> Result<HashSet<PathBuf>, JREError> {
    let mut jre_paths = HashSet::new();

    jre_paths.extend(get_all_jre_path().await);
    jre_paths.extend(get_all_autoinstalled_jre_path().await?);
    jre_paths.extend(get_java_home_paths());

    let common_paths = tokio::task::spawn_blocking(get_common_install_paths);
    let runtime_paths =
        tokio::task::spawn_blocking(get_official_launcher_runtime_paths);
    #[cfg(target_os = "windows")]
    let registry_paths = tokio::task::spawn_blocking(get_registry_paths);

    if let Ok(set) = common_paths.await {
        jre_paths.extend(set);
    }
    if let Ok(set) = runtime_paths.await {
        jre_paths.extend(set);
    }
    #[cfg(target_os = "windows")]
    if let Ok(set) = registry_paths.await {
        jre_paths.extend(set);
    }

    if full_scan || exhaustive {
        let found: Vec<HashSet<PathBuf>> = stream::iter(bfs_search_roots())
            .map(|root| {
                let root = root;
                tokio::task::spawn_blocking(move || {
                    if exhaustive {
                        bfs_exhaustive_scan(&root)
                    } else {
                        bfs_keyword_scan(&root)
                    }
                })
            })
            .buffer_unordered(COLLECT_CONCURRENCY)
            .filter_map(|res| async move { res.ok() })
            .collect()
            .await;

        for set in found {
            jre_paths.extend(set);
        }
    }

    Ok(jre_paths)
}

// Gets candidate paths from the JAVA_HOME environment variable, including
// sibling installations next to it (users commonly keep all their Javas in
// the same parent directory)
fn get_java_home_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    let Ok(java_home) = env::var("JAVA_HOME") else {
        return jre_paths;
    };
    if java_home.trim().is_empty() {
        return jre_paths;
    }

    let java_home = PathBuf::from(java_home);
    jre_paths.insert(java_home.join("bin"));

    if let Some(parent) = java_home.parent()
        && let Ok(siblings) = std::fs::read_dir(parent)
    {
        for sibling in siblings.flatten() {
            jre_paths.insert(sibling.path().join("bin"));
        }
    }

    jre_paths
}

// Hard paths for locations of commonly installed Java (Windows)
#[cfg(target_os = "windows")]
fn get_common_install_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    let java_paths = [
        r"C:\Program Files\Java",
        r"C:\Program Files (x86)\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files (x86)\Eclipse Adoptium",
        r"C:\Program Files\Microsoft",
    ];
    for java_path in java_paths {
        let Ok(java_subpaths) = std::fs::read_dir(java_path) else {
            continue;
        };
        for java_subpath in java_subpaths.flatten() {
            jre_paths.insert(java_subpath.path().join("bin"));
        }
    }

    // Scan all subdirectories of Program Files for any Java installations
    let program_files_dirs = [r"C:\Program Files", r"C:\Program Files (x86)"];
    for pf_dir in program_files_dirs {
        let Ok(subdirs) = std::fs::read_dir(pf_dir) else {
            continue;
        };
        for entry in subdirs.flatten() {
            let subdir = entry.path();
            jre_paths.insert(subdir.join("bin"));
            jre_paths.insert(subdir.join("jre").join("bin"));
        }
    }

    // LocalAppData programs (e.g. Eclipse Adoptium installs here)
    if let Some(local_app_data) = dirs::data_local_dir() {
        let programs = local_app_data.join("Programs");
        if programs.is_dir()
            && let Ok(dir) = std::fs::read_dir(programs)
        {
            for entry in dir.flatten() {
                jre_paths.insert(entry.path().join("bin"));
            }
        }
    }

    // Roaming AppData — check for jdk/jre directories
    if let Some(app_data) = dirs::data_dir()
        && let Ok(dir) = std::fs::read_dir(app_data)
    {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("java")
                || name.contains("jdk")
                || name.contains("jre")
            {
                jre_paths.insert(entry.path().join("bin"));
            }
        }
    }

    jre_paths
}

// Hard paths for locations of commonly installed Java (Mac)
#[cfg(target_os = "macos")]
fn get_common_install_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    let java_paths = [
        r"/Applications/Xcode.app/Contents/Applications/Application Loader.app/Contents/MacOS/itms/java",
        r"/Library/Internet Plug-Ins/JavaAppletPlugin.plugin/Contents/Home",
        r"/System/Library/Frameworks/JavaVM.framework/Versions/Current/Commands",
    ];
    for path in java_paths {
        jre_paths.insert(PathBuf::from(path));
    }

    // Iterate over JavaVirtualMachines/(something)/Contents/Home/bin
    let base_path = PathBuf::from("/Library/Java/JavaVirtualMachines/");
    if let Ok(dir) = std::fs::read_dir(base_path) {
        for entry in dir.flatten() {
            jre_paths.insert(entry.path().join("Contents/Home/bin"));
        }
    }

    // User-local JavaVirtualMachines
    if let Some(home) = dirs::home_dir() {
        let user_jvm = home
            .join("Library")
            .join("Java")
            .join("JavaVirtualMachines");
        if user_jvm.is_dir() {
            if let Ok(dir) = std::fs::read_dir(user_jvm) {
                for entry in dir.flatten() {
                    jre_paths.insert(entry.path().join("Contents/Home/bin"));
                }
            }
        }

        // sdkman candidates
        let sdkman_path = home.join(".sdkman").join("candidates").join("java");
        if sdkman_path.is_dir() {
            if let Ok(dir) = std::fs::read_dir(sdkman_path) {
                for entry in dir.flatten() {
                    jre_paths.insert(entry.path().join("bin"));
                }
            }
        }
    }

    jre_paths
}

// Hard paths for locations of commonly installed Java (Linux)
#[cfg(target_os = "linux")]
fn get_common_install_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    let java_paths = [
        r"/usr",
        r"/usr/java",
        r"/usr/lib/jvm",
        r"/usr/lib64/jvm",
        r"/opt/jdk",
        r"/opt/jdks",
    ];
    for path in java_paths {
        let path = PathBuf::from(path);
        jre_paths.insert(PathBuf::from(&path).join("jre").join("bin"));
        jre_paths.insert(PathBuf::from(&path).join("bin"));
        if let Ok(dir) = std::fs::read_dir(path) {
            for entry in dir.flatten() {
                let entry_path = entry.path();
                jre_paths.insert(entry_path.join("jre").join("bin"));
                jre_paths.insert(entry_path.join("bin"));
            }
        }
    }

    // Additional Java version manager directories
    if let Some(home) = dirs::home_dir() {
        // sdkman candidates
        let sdkman_path = home.join(".sdkman").join("candidates").join("java");
        if sdkman_path.is_dir() {
            if let Ok(dir) = std::fs::read_dir(sdkman_path) {
                for entry in dir.flatten() {
                    jre_paths.insert(entry.path().join("bin"));
                }
            }
        }

        // asdf installs
        let asdf_path = home.join(".asdf").join("installs").join("java");
        if asdf_path.is_dir() {
            if let Ok(dir) = std::fs::read_dir(asdf_path) {
                for entry in dir.flatten() {
                    jre_paths.insert(entry.path().join("bin"));
                }
            }
        }

        // jabba jdk
        let jabba_path = home.join(".jabba").join("jdk");
        if jabba_path.is_dir() {
            if let Ok(dir) = std::fs::read_dir(jabba_path) {
                for entry in dir.flatten() {
                    jre_paths.insert(entry.path().join("bin"));
                }
            }
        }
    }

    // Snap packages
    let snap_path = PathBuf::from("/snap");
    if snap_path.is_dir() {
        if let Ok(dir) = std::fs::read_dir(snap_path) {
            for entry in dir.flatten() {
                jre_paths.insert(entry.path().join("bin"));
            }
        }
    }

    jre_paths
}

// Runtimes bundled by the official Minecraft launcher, laid out as
// runtime/<component>/<platform>/<component>/bin (with an extra
// jre.bundle/Contents/Home level on macOS)
fn get_official_launcher_runtime_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    for root in official_launcher_runtime_roots() {
        let Ok(components) = std::fs::read_dir(root) else {
            continue;
        };
        for component in components.flatten() {
            let Ok(platforms) = std::fs::read_dir(component.path()) else {
                continue;
            };
            for platform in platforms.flatten() {
                let Ok(installs) = std::fs::read_dir(platform.path()) else {
                    continue;
                };
                for install in installs.flatten() {
                    let install = install.path();
                    jre_paths.insert(install.join("bin"));
                    jre_paths
                        .insert(install.join("jre.bundle/Contents/Home/bin"));
                }
            }
        }
    }

    jre_paths
}

fn official_launcher_runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(data) = dirs::data_dir() {
            roots.push(data.join(".minecraft").join("runtime"));
        }
        // Microsoft Store distribution of the official launcher
        if let Some(local) = dirs::data_local_dir() {
            roots.push(
                local
                    .join("Packages")
                    .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                    .join("LocalCache")
                    .join("Local")
                    .join("runtime"),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(data) = dirs::data_dir() {
            roots.push(data.join("minecraft").join("runtime"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".minecraft").join("runtime"));
        }
    }

    roots
}

// Windows Registry keys of known Java distributions
#[cfg(target_os = "windows")]
fn get_registry_paths() -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    let key_paths = [
        r"SOFTWARE\JavaSoft\Java Runtime Environment", // Oracle
        r"SOFTWARE\JavaSoft\Java Development Kit",
        r"SOFTWARE\JavaSoft\JRE", // Oracle
        r"SOFTWARE\JavaSoft\JDK",
        r"SOFTWARE\Eclipse Foundation\JDK", // Eclipse
        r"SOFTWARE\Eclipse Adoptium\JRE",   // Eclipse
        r"SOFTWARE\Eclipse Adoptium\JDK",
        r"SOFTWARE\Microsoft\JDK",     // Microsoft
        r"SOFTWARE\Azul Systems\Zulu", // Azul
        r"SOFTWARE\BellSoft\Liberica", // BellSoft
    ];

    for key in key_paths {
        for flag in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(jre_key) = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(key, KEY_READ | flag)
            {
                jre_paths.extend(get_paths_from_jre_winregkey(jre_key));
            }
        }
    }

    jre_paths
}

// Gets paths rather than search directly as RegKeys should not be passed asynchronously (do not impl Send)
#[cfg(target_os = "windows")]
#[tracing::instrument]
pub fn get_paths_from_jre_winregkey(jre_key: RegKey) -> HashSet<PathBuf> {
    let mut jre_paths = HashSet::new();

    for subkey in jre_key.enum_keys().flatten() {
        let Ok(subkey) = jre_key.open_subkey(subkey) else {
            continue;
        };

        for subkey_value in ["JavaHome", "InstallationPath"] {
            let path: Result<String, std::io::Error> =
                subkey.get_value(subkey_value);
            let Ok(path) = path else { continue };

            jre_paths.insert(PathBuf::from(path).join("bin"));
        }

        // Eclipse Adoptium stores the install path in a nested subkey
        if let Ok(msi_key) = subkey.open_subkey(r"hotspot\MSI") {
            let path: Result<String, std::io::Error> =
                msi_key.get_value("Path");
            if let Ok(path) = path {
                jre_paths.insert(PathBuf::from(path).join("bin"));
            }
        }
    }
    jre_paths
}

// Roots for the bounded keyword BFS, per platform
fn bfs_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(home) = dirs::home_dir() {
        roots.push(home);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(data) = dirs::data_dir() {
            roots.push(data);
        }
        if let Some(local) = dirs::data_local_dir() {
            roots.push(local);
        }

        let disks = sysinfo::Disks::new_with_refreshed_list();
        for disk in disks.list() {
            if disk.is_removable() {
                continue;
            }
            let mount = disk.mount_point().to_path_buf();
            roots.push(mount.join("Program Files"));
            roots.push(mount.join("Program Files (x86)"));
            roots.push(mount);
        }
    }

    #[cfg(target_os = "macos")]
    {
        roots.push(PathBuf::from("/opt"));
        roots.push(PathBuf::from("/usr/local"));
        // Homebrew formula link farms (Apple Silicon and Intel)
        roots.push(PathBuf::from("/opt/homebrew/opt"));
        roots.push(PathBuf::from("/usr/local/opt"));
    }

    #[cfg(target_os = "linux")]
    {
        roots.push(PathBuf::from("/opt"));
        roots.push(PathBuf::from("/usr/local"));

        // Scan all mounted non-removable partitions for Java installations
        let disks = sysinfo::Disks::new_with_refreshed_list();
        for disk in disks.list() {
            if disk.is_removable() {
                continue;
            }
            let mount = disk.mount_point().to_path_buf();
            if mount != PathBuf::from("/") && mount.as_os_str() != "/" {
                roots.push(mount);
            }
        }

        // Also scan common mount directories
        for common in &["/mnt", "/media", "/run/media"] {
            if std::path::Path::new(common).is_dir() {
                if let Ok(entries) = std::fs::read_dir(common) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            roots.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    roots.retain(|root| root.is_dir());
    roots.sort();
    roots.dedup();
    roots
}

fn dir_name_matches_keywords(name: &str) -> bool {
    DIR_NAME_KEYWORDS
        .iter()
        .any(|keyword| name.contains(keyword))
}

fn dir_name_excluded(name: &str) -> bool {
    EXCLUDED_DIR_NAMES
        .iter()
        .any(|excluded| name.contains(excluded))
}

// Breadth-first search below `root` for directories containing a Java
// executable. Only keyword-matching directories are entered at the top
// level; the exclusion list applies at every level, as do the depth and
// directory-count bounds
fn bfs_keyword_scan(root: &Path) -> HashSet<PathBuf> {
    let mut found = HashSet::new();
    let mut scanned_dirs = 0usize;
    let mut queue = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0usize));

    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_lowercase();
            if dir_name_excluded(&name) {
                continue;
            }
            if depth == 0 && !dir_name_matches_keywords(&name) {
                continue;
            }

            scanned_dirs += 1;
            if scanned_dirs > BFS_MAX_DIRS_PER_ROOT {
                return found;
            }

            let java_bin = path.join(JAVA_BIN);
            if java_bin.is_file() {
                found.insert(java_bin);
            } else if depth + 1 < BFS_MAX_DEPTH {
                queue.push_back((path, depth + 1));
            }
        }
    }

    found
}

fn bfs_exhaustive_scan(root: &Path) -> HashSet<PathBuf> {
    let mut found = HashSet::new();
    let mut scanned_dirs = 0usize;
    let mut queue = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0usize));

    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // No keyword filter at any depth — scan ALL directories

            scanned_dirs += 1;
            if scanned_dirs > 100_000 {
                return found;
            }

            let java_bin = path.join(JAVA_BIN);
            if java_bin.is_file() {
                found.insert(java_bin);
            }
            // No depth limit — keep going
            queue.push_back((path, depth + 1));
        }
    }

    found
}

pub(crate) fn is_java_install_staging_path(path: &Path) -> bool {
    path.components().any(|component| {
        let std::path::Component::Normal(name) = component else {
            return false;
        };
        let name = name.to_string_lossy().to_ascii_lowercase();
        name.starts_with('.') && name.ends_with(JAVA_INSTALL_STAGING_SUFFIX)
    })
}

// Gets all JREs from the launcher's own auto-installed Java directory
#[tracing::instrument]
async fn get_all_autoinstalled_jre_path() -> Result<HashSet<PathBuf>, JREError>
{
    Box::pin(async move {
        let state = State::get().await.map_err(|_| JREError::StateError)?;

        let mut jre_paths = HashSet::new();
        let base_path = state.directories.java_versions_dir();

        if base_path.is_dir()
            && let Ok(dir) = std::fs::read_dir(base_path)
        {
            for entry in dir.flatten() {
                let root = entry.path();

                #[cfg(target_os = "macos")]
                {
                    jre_paths.insert(root.join("Contents/Home/bin/java"));
                    jre_paths
                        .insert(root.join("jre.bundle/Contents/Home/bin/java"));
                    continue;
                }

                #[cfg(not(target_os = "macos"))]
                {
                    let bin = root.join("bin");
                    if let Ok(contents) = std::fs::read_to_string(bin.clone()) {
                        jre_paths.insert(root.join(contents.trim()));
                    } else {
                        jre_paths.insert(bin.join(JAVA_BIN));
                    }
                }
            }
        }

        Ok(jre_paths)
    })
    .await
}

// Gets all JREs from the PATH env variable
#[tracing::instrument]
async fn get_all_jre_path() -> HashSet<PathBuf> {
    // Iterate over values in PATH variable, where accessible JREs are referenced
    let paths =
        env::var("PATH").map(|x| env::split_paths(&x).collect::<HashSet<_>>());
    paths.unwrap_or_else(|_| HashSet::new())
}

pub const JAVA_BIN: &str = if cfg!(target_os = "windows") {
    "javaw.exe"
} else {
    "java"
};

// For each example filepath in 'paths', perform check_java_at_filepath, checking each one concurrently
// and returning a JavaVersion for every valid path that points to a java bin
#[tracing::instrument]
pub async fn check_java_at_filepaths(
    paths: HashSet<PathBuf>,
) -> HashSet<JavaVersion> {
    stream::iter(paths.into_iter())
        .map(|p: PathBuf| {
            tokio::task::spawn(async move { check_java_at_filepath(&p).await })
        })
        .buffer_unordered(64)
        .filter_map(async |x| x.ok().and_then(Result::ok))
        .collect()
        .await
}

// For example filepath 'path', attempt to resolve it and get a Java version at this path
// If no such path exists, or no such valid java at this path exists, returns None
#[tracing::instrument]
pub async fn check_java_at_filepath(path: &Path) -> crate::Result<JavaVersion> {
    // Attempt to canonicalize the potential java filepath
    // If it fails, this path does not exist and None is returned (no Java here)
    let path = io::canonicalize(path)?;

    // Checks for existence of Java at this filepath
    // Adds JAVA_BIN to the end of the path if it is not already there
    let file_name = path.file_name().and_then(|x| x.to_str());
    let java = if file_name.is_some_and(|x| x == JAVA_BIN) {
        path
    } else if cfg!(target_os = "windows")
        && file_name.is_some_and(|x| x.eq_ignore_ascii_case("java.exe"))
    {
        path.with_file_name(JAVA_BIN)
    } else {
        path.join(JAVA_BIN)
    };

    if !java.exists() {
        return Err(JREError::NoExecutable(java).into());
    };

    if let Some(java_version) = java_version_from_release_file(&java) {
        return Ok(java_version);
    }

    let (_temp, file_path) =
        get_resource_file!(env "JAVA_JARS_DIR" / "theseus.jar")?;

    let output = java_environment::command(&java)
        .arg("-cp")
        .arg(file_path)
        .arg("com.modrinth.theseus.JavaInfo")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut java_version = None;
    let mut java_arch = None;

    for line in stdout.lines() {
        let mut parts = line.split('=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();

        if key == "os.arch" {
            java_arch = Some(value);
        } else if key == "java.version" {
            java_version = Some(value);
        }
    }

    // Extract version info from it
    if let Some(arch) = java_arch
        && let Some(version) = java_version
    {
        if let Ok(version) = extract_java_version(version) {
            let path = java.to_string_lossy().to_string();
            return Ok(JavaVersion {
                parsed_version: version,
                path,
                version: version.to_string(),
                architecture: arch.to_string(),
                distribution: None,
            });
        }

        return Err(JREError::InvalidJREVersion(version.to_owned()).into());
    }

    Err(JREError::FailedJavaCheck(java).into())
}

/// Reads a Java installation's `release` file (present in Java 8+ distributions
/// on Windows, macOS and Linux) to resolve its version and architecture without
/// starting a JVM. Returns `None` when the file is missing or lacks a field we
/// can trust, so the caller falls back to executing the runtime.
fn java_version_from_release_file(java: &Path) -> Option<JavaVersion> {
    let root = java.parent()?.parent()?;
    let contents = std::fs::read_to_string(root.join("release")).ok()?;

    let mut version = None;
    let mut os_arch = None;
    let mut implementor = None;
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "JAVA_VERSION" => version = Some(value.to_string()),
            "OS_ARCH" => os_arch = Some(value.to_string()),
            "IMPLEMENTOR" => implementor = Some(value.to_string()),
            _ => {}
        }
    }

    let parsed_version = extract_java_version(&version?).ok()?;
    let architecture = normalize_release_arch(&os_arch?)?;

    Some(JavaVersion {
        parsed_version,
        path: java.to_string_lossy().to_string(),
        version: parsed_version.to_string(),
        architecture,
        distribution: implementor,
    })
}

/// Maps a `release` file `OS_ARCH` value to the `os.arch` form the JVM reports,
/// which the launcher's natives and rule matching expect. Returns `None` for
/// unrecognised values so the caller re-checks by running the JVM instead.
fn normalize_release_arch(os_arch: &str) -> Option<String> {
    let normalized = match os_arch.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "aarch64",
        "x86" | "i386" | "i486" | "i586" | "i686" => "x86",
        "arm" => "arm",
        _ => return None,
    };
    Some(normalized.to_string())
}

pub fn extract_java_version(version: &str) -> Result<u32, JREError> {
    let mut split = version.split('.');

    let version = split.next().unwrap();
    let version = version.split_once('-').map_or(version, |(x, _)| x);
    let mut version = version.parse::<u32>()?;
    if version == 1 {
        version = split.next().map_or(Ok(1), |x| x.parse::<u32>())?;
    }

    Ok(version)
}

#[derive(thiserror::Error, Debug)]
pub enum JREError {
    #[error("Command error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Env error: {0}")]
    EnvError(#[from] env::VarError),

    #[error("No executable found at {0}")]
    NoExecutable(PathBuf),

    #[error("Could not check Java version at path {0}")]
    FailedJavaCheck(PathBuf),

    #[error("Invalid JRE version string: {0}")]
    InvalidJREVersion(String),

    #[error("Parsing error: {0}")]
    ParseError(#[from] std::num::ParseIntError),

    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),

    #[error("No stored tag for Minecraft version {0}")]
    NoMinecraftVersionFound(String),

    #[error("Error getting launcher state")]
    StateError,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_java_bin(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(JAVA_BIN), b"").unwrap();
    }

    #[test]
    fn keyword_matching_accepts_vendor_and_generic_names() {
        for name in [
            "jdk-17.0.2",
            "my-java-installs",
            "zulu17.44.53",
            "amazon-corretto-21",
            "temurin",
            ".sdkman",
            ".minecraft",
            "openlogic-openjdk",
        ] {
            assert!(
                dir_name_matches_keywords(&name.to_lowercase()),
                "expected {name} to match"
            );
        }
    }

    #[test]
    fn keyword_matching_rejects_unrelated_names() {
        for name in ["documents", "photos", "projects", "windows"] {
            assert!(
                !dir_name_matches_keywords(&name.to_lowercase()),
                "expected {name} not to match"
            );
        }
    }

    #[test]
    fn exclusion_wins_over_keywords() {
        assert!(dir_name_excluded("javapath"));
        assert!(dir_name_excluded("java8path"));
        assert!(dir_name_excluded("node_modules"));
    }

    #[test]
    fn bfs_finds_java_within_depth_limit() {
        let root = tempfile::tempdir().unwrap();
        make_java_bin(&root.path().join("myjdk/a/b/c/bin"));

        let expected = root.path().join("myjdk/a/b/c/bin").join(JAVA_BIN);
        assert_eq!(bfs_keyword_scan(root.path()), HashSet::from([expected]));
    }

    #[test]
    fn bfs_ignores_java_beyond_depth_limit() {
        let root = tempfile::tempdir().unwrap();
        make_java_bin(&root.path().join("myjdk/a/b/c/d/bin"));

        assert!(bfs_keyword_scan(root.path()).is_empty());
    }

    #[test]
    fn bfs_skips_top_level_directories_without_keywords() {
        let root = tempfile::tempdir().unwrap();
        make_java_bin(&root.path().join("stuff/bin"));

        assert!(bfs_keyword_scan(root.path()).is_empty());
    }

    #[test]
    fn bfs_skips_excluded_directories() {
        let root = tempfile::tempdir().unwrap();
        make_java_bin(&root.path().join("javapath"));
        make_java_bin(&root.path().join("java/mods/bin"));

        assert!(bfs_keyword_scan(root.path()).is_empty());
    }

    #[test]
    fn bfs_stops_descending_once_java_is_found() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("jdk-21/bin");
        make_java_bin(&bin);
        make_java_bin(&bin.join("nested"));

        let found = bfs_keyword_scan(root.path());
        assert_eq!(found, HashSet::from([bin.join(JAVA_BIN)]));
    }

    #[test]
    fn extracts_major_versions_from_version_strings() {
        assert_eq!(extract_java_version("1.8.0_321").unwrap(), 8);
        assert_eq!(extract_java_version("17.0.1").unwrap(), 17);
        assert_eq!(extract_java_version("21-ea").unwrap(), 21);
        assert_eq!(extract_java_version("25").unwrap(), 25);
        assert!(extract_java_version("garbage").is_err());
    }
}
