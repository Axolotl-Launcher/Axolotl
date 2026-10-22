//! Metadata reachability uses only local files and read-only channel databases.

use super::*;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde_json::Value;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::sync::Mutex;

static PLANS: LazyLock<Mutex<HashMap<String, CleanupPlan>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const PLAN_AGE: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, Serialize)]
pub struct MetaGroup {
    pub id: String,
    pub category: String,
    pub name: String,
    pub used: u64,
    pub unused: u64,
    pub unknown: u64,
    pub files: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetaAnalysis {
    pub id: String,
    pub scanned_at: DateTime<Utc>,
    pub groups: Vec<MetaGroup>,
    pub warnings: Vec<MetaWarning>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetaWarningKind {
    VersionUnavailable,
    ProcessorUnresolved,
    DatabaseUnavailable,
    ActiveOperation,
    CustomArguments,
    ExternalMetadata,
    UnreadablePath,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetaWarning {
    pub kind: MetaWarningKind,
    pub subject: String,
    pub detail: String,
}

impl MetaWarning {
    fn new(
        kind: MetaWarningKind,
        subject: impl Into<String>,
        detail: impl ToString,
    ) -> Self {
        Self {
            kind,
            subject: subject.into(),
            detail: detail.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MetaRemoved {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MetaCleanupResult {
    pub removed_files: u64,
    pub removed_bytes: u64,
    pub removed_groups: BTreeMap<String, MetaRemoved>,
    pub skipped_files: u64,
    pub failures: Vec<String>,
}

struct CleanupPlan {
    created: std::time::Instant,
    root: PathBuf,
    files: Vec<Candidate>,
}

struct Candidate {
    relative: String,
    group: String,
    size: u64,
    modified: Option<cap_std::time::SystemTime>,
    created: Option<cap_std::time::SystemTime>,
}

#[derive(Default)]
struct References {
    paths: HashSet<String>,
    versions: HashSet<String>,
    manifests: HashSet<String>,
    games: HashMap<String, String>,
    natives: HashSet<String>,
    legacy_candidates: HashMap<String, Vec<(u64, String)>>,
    blocked: bool,
    busy: bool,
    uncertain_categories: HashSet<&'static str>,
    warnings: Vec<MetaWarning>,
}

fn key(value: &str) -> String {
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value.to_owned()
    }
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn component(value: &str) -> bool {
    safe_relative(value) && !value.contains('/')
}

impl References {
    fn keep(&mut self, path: String) -> crate::Result<()> {
        if !safe_relative(&path) {
            return Err(invalid("Invalid path in local metadata"));
        }
        self.paths.insert(key(&path));
        Ok(())
    }

    fn used(&self, relative: &str) -> Option<bool> {
        let parts: Vec<_> = relative.split('/').collect();
        let referenced = match parts.as_slice() {
            ["versions", version, ..] => self.versions.contains(&key(version)),
            ["natives", version, ..] => self.natives.contains(&key(version)),
            _ => self.paths.contains(&key(relative)),
        };
        if referenced {
            return Some(true);
        }
        if self.blocked
            || self.busy
            || parts.first().is_some_and(|category| {
                self.uncertain_categories.contains(category)
            })
        {
            return None;
        }
        match parts.as_slice() {
            ["versions", version, name] => (name == &format!("{version}.jar")
                || name == &format!("{version}.json")
                || name == &format!("{version}.json.axolotl-format"))
                .then_some(false),
            ["natives", _, ..] => Some(false),
            ["libraries", ..]
                if parts.len() >= 5 && relative.ends_with(".jar") =>
            {
                Some(false)
            }
            ["assets", "objects", prefix, hash]
                if hash.len() == 40
                    && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && *prefix == &hash[..2] =>
            {
                Some(false)
            }
            ["assets", "indexes", name] if name.ends_with(".json") => {
                Some(false)
            }
            ["log_configs", name] if name.ends_with(".xml") => Some(false),
            ["resources", ..]
                if self.legacy_candidates.contains_key(&key(relative)) =>
            {
                Some(false)
            }
            _ => None,
        }
    }
}

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

async fn read_json(path: &Path) -> crate::Result<Value> {
    let metadata = fs::symlink_metadata(path).await?;
    if crate::util::io::is_symlink_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > 64 * 1024 * 1024
    {
        return Err(invalid(format!(
            "Cannot read local metadata: {}",
            path.display()
        )));
    }
    Ok(serde_json::from_slice(&fs::read(path).await?)?)
}

async fn channel_references(
    pool: &SqlitePool,
    refs: &mut References,
    directories: &DirectoryInfo,
) -> crate::Result<()> {
    let busy: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM install_jobs WHERE status IN ('queued', 'running', 'canceling', 'waiting_for_user') OR json_extract(state, '$.rollback') IS NOT NULL").fetch_one(pool).await?;
    refs.busy |= busy > 0;
    let overrides: Vec<String> = sqlx::query_scalar(
        "SELECT json(overrides) FROM instance_launch_overrides",
    )
    .fetch_all(pool)
    .await?;
    let settings: Vec<String> =
        sqlx::query_scalar("SELECT json(extra_launch_args) FROM settings")
            .fetch_all(pool)
            .await?;
    if overrides.iter().chain(settings.iter()).any(|value| {
        let value = value.to_lowercase();
        value.contains(".jar")
            || value.contains(".zip")
            || value.contains("meta/")
            || value.contains("meta\\")
    }) {
        refs.blocked = true;
        refs.warnings.push(MetaWarning::new(
            MetaWarningKind::CustomArguments,
            "",
            "",
        ));
    }
    let instances = crate::state::instances::adapters::sqlite::instance_rows::list_instances(pool).await?;
    for instance in instances {
        let runtime = crate::launcher::instance_runtime::InstanceRuntimeAdapter::for_instance(&instance, directories)?;
        if let Some(direct) = runtime.direct_link() {
            let meta_root = identity(&directories.metadata_dir());
            if [
                direct.libraries_dir(),
                direct.assets_dir(),
                direct.version_dir(),
            ]
            .iter()
            .any(|path| identity(path).starts_with(&meta_root))
            {
                refs.blocked = true;
                refs.warnings.push(MetaWarning::new(
                    MetaWarningKind::ExternalMetadata,
                    &instance.name,
                    "",
                ));
            }
            refs.natives.insert(key(&direct.natives_cache_key()));
            continue;
        }
        let rows = sqlx::query("SELECT game_version, loader, loader_version FROM instance_content_sets WHERE instance_id = ?").bind(&instance.id).fetch_all(pool).await?;
        if rows.is_empty() {
            return Err(invalid("An instance has no local content version"));
        }
        for row in rows {
            let game: String = row.try_get("game_version")?;
            let loader: String = row.try_get("loader")?;
            let version: Option<String> = row.try_get("loader_version")?;
            if !component(&game) {
                return Err(invalid("Unresolved game version"));
            }
            refs.versions.insert(key(&game));
            if loader != "vanilla" {
                let version = version
                    .filter(|version| {
                        component(version)
                            && version != "latest"
                            && version != "stable"
                    })
                    .ok_or_else(|| {
                        invalid("An instance has no exact loader version")
                    })?;
                let combined = format!("{game}-{version}");
                refs.versions.insert(key(&combined));
                refs.manifests.insert(key(&combined));
                refs.games.insert(key(&combined), game.clone());
                refs.natives.insert(key(&combined));
            } else {
                refs.natives.insert(key(&game));
                refs.manifests.insert(key(&game));
                refs.games.insert(key(&game), game.clone());
            }
        }
    }
    Ok(())
}

async fn references(directories: &DirectoryInfo) -> References {
    let mut refs = References::default();
    let mut databases = 0;
    for path in [
        directories.settings_dir.join("release/app.db"),
        directories.settings_dir.join("beta/app.db"),
        directories.settings_dir.join("app.db"),
    ] {
        match fs::try_exists(&path).await {
            Ok(false) => continue,
            Err(error) => {
                refs.blocked = true;
                refs.warnings.push(MetaWarning::new(
                    MetaWarningKind::DatabaseUnavailable,
                    path.display().to_string(),
                    error,
                ));
                continue;
            }
            Ok(true) => {}
        }
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .read_only(true)
            .create_if_missing(false);
        match SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
        {
            Ok(pool) => {
                databases += 1;
                if let Err(error) =
                    channel_references(&pool, &mut refs, directories).await
                {
                    refs.blocked = true;
                    refs.warnings.push(MetaWarning::new(
                        MetaWarningKind::DatabaseUnavailable,
                        path.display().to_string(),
                        error,
                    ));
                }
                pool.close().await;
            }
            Err(error) => {
                refs.blocked = true;
                refs.warnings.push(MetaWarning::new(
                    MetaWarningKind::DatabaseUnavailable,
                    path.display().to_string(),
                    error,
                ));
            }
        }
    }
    // A missing active database must never turn all metadata into orphans.
    if databases == 0 || crate::State::get().await.is_err() {
        refs.blocked = true;
    }
    resolve_manifests(&directories.metadata_dir(), &mut refs).await;
    if refs.busy {
        refs.warnings.push(MetaWarning::new(
            MetaWarningKind::ActiveOperation,
            "",
            "",
        ));
    }
    refs
}

/// Unknown loader libraries must not make unrelated legacy copies unclassifiable.
/// Standard releases from 1.7 onward use assets/objects; snapshots stay conservative.
fn uses_modern_resources(game: &str) -> bool {
    let parts: Vec<_> = game.split('.').map(str::parse::<u32>).collect();
    if parts.len() < 2 || parts.iter().any(Result::is_err) {
        return false;
    }
    let major = parts[0].as_ref().copied().unwrap_or(0);
    let minor = parts[1].as_ref().copied().unwrap_or(0);
    major > 1 || (major == 1 && minor >= 7)
}

async fn resolve_manifests(root: &Path, refs: &mut References) {
    let versions: Vec<_> = refs.manifests.iter().cloned().collect();
    for version in versions {
        if let Err(error) = version_references(root, &version, refs).await {
            refs.uncertain_categories.insert("libraries");
            let base = refs
                .games
                .get(&version)
                .filter(|game| **game != version)
                .cloned();
            let base_info = if let Some(game) = &base {
                read_json(
                    &root
                        .join("versions")
                        .join(game)
                        .join(format!("{game}.json")),
                )
                .await
                .ok()
                .and_then(|value| {
                    serde_json::from_value::<daedalus::minecraft::VersionInfo>(
                        value,
                    )
                    .ok()
                })
                .filter(|info| info.id == *game)
            } else {
                None
            };
            if let Some(info) = base_info {
                if asset_references(root, &info, refs).await.is_err() {
                    refs.uncertain_categories.extend(["assets", "log_configs"]);
                    if info.assets == "legacy" {
                        refs.uncertain_categories.insert("resources");
                    }
                }
            } else {
                refs.uncertain_categories.extend(["assets", "log_configs"]);
                if !refs
                    .games
                    .get(&version)
                    .is_some_and(|game| uses_modern_resources(game))
                {
                    refs.uncertain_categories.insert("resources");
                }
            }
            refs.warnings.push(MetaWarning::new(
                MetaWarningKind::VersionUnavailable,
                version,
                error,
            ));
        }
    }
    index_legacy_candidates(root, refs).await;
}

/// Indexes establish which resource paths are launcher-created copies. Unknown
/// files in the directory never become cleanup candidates merely by location.
async fn index_legacy_candidates(root: &Path, refs: &mut References) {
    let Ok(mut entries) = fs::read_dir(root.join("assets/indexes")).await
    else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            continue;
        }
        let Ok(value) = read_json(&entry.path()).await else {
            continue;
        };
        let Some(objects) = value.get("objects").and_then(Value::as_object)
        else {
            continue;
        };
        for (name, asset) in objects {
            let Some(size) = asset.get("size").and_then(Value::as_u64) else {
                continue;
            };
            let Some(hash) = asset.get("hash").and_then(Value::as_str) else {
                continue;
            };
            let relative = format!("resources/{name}");
            if safe_relative(&relative)
                && hash.len() == 40
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                refs.legacy_candidates
                    .entry(key(&relative))
                    .or_default()
                    .push((size, hash.to_lowercase()));
            }
        }
    }
}

async fn version_references(
    root: &Path,
    version: &str,
    refs: &mut References,
) -> crate::Result<()> {
    let path = root
        .join("versions")
        .join(version)
        .join(format!("{version}.json"));
    let value = read_json(&path).await?;
    if value
        .get("id")
        .and_then(Value::as_str)
        .is_none_or(|id| key(id) != key(version))
    {
        return Err(invalid("Local version identity mismatch"));
    }
    if value.get("inheritsFrom").is_some() {
        return Err(invalid("Unresolved local version inheritance"));
    }
    let info: daedalus::minecraft::VersionInfo =
        serde_json::from_value(value.clone())?;
    for library in &info.libraries {
        refs.keep(format!(
            "libraries/{}",
            daedalus::get_path_from_artifact(&library.name)?
        ))?;
        if let Some(natives) = &library.natives {
            for classifier in natives.values() {
                for width in ["32", "64"] {
                    let classifier = classifier.replace("${arch}", width);
                    refs.keep(format!("libraries/{}", crate::launcher::download::native_library_artifact_path(library, &classifier)?))?;
                }
            }
        }
        if let Some(downloads) = &library.downloads {
            if let Some(classifiers) = &downloads.classifiers {
                for classifier in classifiers.keys() {
                    refs.keep(format!("libraries/{}", crate::launcher::download::native_library_artifact_path(library, classifier)?))?;
                }
            }
            for artifact in downloads.artifact.iter().chain(
                downloads
                    .classifiers
                    .iter()
                    .flat_map(|classifiers| classifiers.values()),
            ) {
                if let Some(path) = &artifact.path {
                    refs.keep(format!("libraries/{path}"))?;
                }
            }
        }
    }
    if let Err(error) = processor_references(root, &info, refs) {
        refs.uncertain_categories.insert("libraries");
        refs.warnings.push(MetaWarning::new(
            MetaWarningKind::ProcessorUnresolved,
            version,
            error,
        ));
    }
    asset_references(root, &info, refs).await
}

async fn asset_references(
    root: &Path,
    info: &daedalus::minecraft::VersionInfo,
    refs: &mut References,
) -> crate::Result<()> {
    let index_id = &info.asset_index.id;
    if !component(index_id) {
        return Err(invalid("Invalid asset index ID"));
    }
    refs.keep(format!("assets/indexes/{index_id}.json"))?;
    let index = read_json(
        &root.join("assets/indexes").join(format!("{index_id}.json")),
    )
    .await?;
    let objects = index
        .get("objects")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Invalid asset index"))?;
    for (name, asset) in objects {
        let hash = asset
            .get("hash")
            .and_then(Value::as_str)
            .filter(|hash| {
                hash.len() == 40
                    && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| invalid("Invalid asset hash"))?;
        refs.keep(format!("assets/objects/{}/{hash}", &hash[..2]))?;
        if info.assets == "legacy" {
            refs.keep(format!("resources/{name}"))?;
        }
        if index.get("virtual").and_then(Value::as_bool) == Some(true) {
            refs.keep(format!("assets/virtual/{index_id}/{name}"))?;
        }
        if index.get("map_to_resources").and_then(Value::as_bool) == Some(true)
        {
            refs.keep(format!("resources/{name}"))?;
        }
    }
    if let Some(logging) = serde_json::to_value(info)?
        .get("logging")
        .and_then(Value::as_object)
    {
        for config in logging.values() {
            if let Some(id) = config.pointer("/file/id").and_then(Value::as_str)
            {
                if !component(id) {
                    return Err(invalid("Invalid logging filename"));
                }
                refs.keep(format!("log_configs/{id}"))?;
            }
        }
    }
    Ok(())
}

/// Resolve the same processor coordinates and data templates used by installation,
/// without downloading files or running a processor.
fn processor_references(
    root: &Path,
    info: &daedalus::minecraft::VersionInfo,
    refs: &mut References,
) -> crate::Result<()> {
    use daedalus::modded::SidedDataEntry;
    let Some(processors) = &info.processors else {
        return Ok(());
    };
    let root = identity(root);
    let libraries = root.join("libraries");
    let mut data: HashMap<_, _> = info
        .data
        .iter()
        .flat_map(|data| data.iter())
        .map(|(key, value)| {
            (
                key.clone(),
                SidedDataEntry {
                    client: value.client.clone(),
                    server: value.server.clone(),
                },
            )
        })
        .collect();
    let game = refs
        .games
        .get(&key(&info.id))
        .cloned()
        .unwrap_or_else(|| info.id.clone());
    for (name, value) in [
        ("SIDE", "client".into()),
        (
            "MINECRAFT_JAR",
            root.join("versions")
                .join(&info.id)
                .join(format!("{}.jar", info.id))
                .to_string_lossy()
                .into_owned(),
        ),
        ("MINECRAFT_VERSION", game),
        (
            "ROOT",
            root.parent()
                .unwrap_or(&root)
                .join("__instance_root__")
                .to_string_lossy()
                .into_owned(),
        ),
        ("LIBRARY_DIR", libraries.to_string_lossy().into_owned()),
    ] {
        data.insert(
            name.into(),
            SidedDataEntry {
                client: value,
                server: String::new(),
            },
        );
    }
    for value in data.values() {
        if let Some(coordinate) = value
            .client
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            refs.keep(format!(
                "libraries/{}",
                daedalus::get_path_from_artifact(coordinate)?
            ))?;
        }
    }
    for processor in processors {
        if processor
            .sides
            .as_ref()
            .is_some_and(|sides| !sides.iter().any(|side| side == "client"))
        {
            continue;
        }
        for coordinate in
            std::iter::once(&processor.jar).chain(&processor.classpath)
        {
            refs.keep(format!(
                "libraries/{}",
                daedalus::get_path_from_artifact(coordinate)?
            ))?;
        }
        let templates: Vec<_> = processor
            .args
            .iter()
            .chain(processor.outputs.iter().flat_map(|outputs| outputs.keys()))
            .collect();
        let resolved = crate::launcher::get_processor_arguments(
            &libraries, &templates, &data,
        )?;
        for argument in resolved {
            if argument.contains('{') || argument.contains('}') {
                return Err(invalid("Unresolved loader installation path"));
            }
            let value = argument
                .strip_prefix("--")
                .and_then(|option| option.split_once('='))
                .map_or(argument.as_str(), |(_, value)| value);
            let path = Path::new(value);
            if let Ok(relative) = path.strip_prefix(&root) {
                let relative = relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                refs.keep(relative)?;
            } else if argument.contains(root.to_string_lossy().as_ref()) {
                return Err(invalid("Ambiguous loader installation path"));
            }
        }
    }
    Ok(())
}

fn group_for(relative: &str) -> (String, String, String) {
    let mut parts = relative.split('/');
    let category = parts.next().unwrap_or("other").to_owned();
    let name = match category.as_str() {
        "versions" | "natives" | "java_versions" => {
            parts.next().unwrap_or_default().to_owned()
        }
        _ => String::new(),
    };
    let id = if name.is_empty() {
        category.clone()
    } else {
        format!("{category}/{name}")
    };
    (id, category, name)
}

fn directory(root: &Dir, relative: &str) -> std::io::Result<Dir> {
    let mut dir = root.try_clone()?;
    if relative.is_empty() {
        return Ok(dir);
    }
    if !safe_relative(relative) {
        return Err(std::io::Error::other("Invalid metadata path"));
    }
    for part in relative.split('/') {
        dir = dir.open_dir_nofollow(part)?;
    }
    Ok(dir)
}

fn opened_root(directories: &DirectoryInfo) -> std::io::Result<Dir> {
    Dir::open_ambient_dir(
        &directories.config_dir,
        cap_std::ambient_authority(),
    )?
    .open_dir_nofollow("meta")
}

fn collect(
    root: &Dir,
    relative: &str,
    refs: &References,
    groups: &mut BTreeMap<String, MetaGroup>,
    files: &mut Vec<Candidate>,
    warnings: &mut Vec<MetaWarning>,
) -> std::io::Result<()> {
    let dir = directory(root, relative)?;
    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if !safe_relative(&path) {
            warnings.push(MetaWarning::new(
                MetaWarningKind::UnreadablePath,
                path,
                "",
            ));
            continue;
        }
        let metadata = dir.symlink_metadata(entry.file_name())?;
        if metadata.is_symlink() {
            warnings.push(MetaWarning::new(
                MetaWarningKind::UnreadablePath,
                path,
                "",
            ));
            continue;
        }
        if metadata.is_dir() {
            if let Err(error) =
                collect(root, &path, refs, groups, files, warnings)
            {
                warnings.push(MetaWarning::new(
                    MetaWarningKind::UnreadablePath,
                    path,
                    error,
                ));
            }
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let (id, category, name) = group_for(&path);
        let group = groups.entry(id.clone()).or_insert_with(|| MetaGroup {
            id: id.clone(),
            category,
            name,
            used: 0,
            unused: 0,
            unknown: 0,
            files: 0,
        });
        group.files += 1;
        let size = metadata.len();
        let used = if path.starts_with("resources/")
            && refs.used(&path) == Some(false)
        {
            if verified_legacy_copy(root, &path, size, refs)? {
                Some(false)
            } else {
                None
            }
        } else {
            refs.used(&path)
        };
        match used {
            Some(true) => group.used += size,
            Some(false) => {
                group.unused += size;
                files.push(Candidate {
                    relative: path,
                    group: id,
                    size,
                    modified: metadata.modified().ok(),
                    created: metadata.created().ok(),
                });
            }
            None => group.unknown += size,
        }
    }
    Ok(())
}

fn verified_legacy_copy(
    root: &Dir,
    relative: &str,
    size: u64,
    refs: &References,
) -> std::io::Result<bool> {
    use sha1_smol::Sha1;
    use std::io::Read;
    let Some(expected) = refs.legacy_candidates.get(&key(relative)) else {
        return Ok(false);
    };
    if !expected.iter().any(|(length, _)| *length == size) {
        return Ok(false);
    }
    let (parent, name) = relative
        .rsplit_once('/')
        .ok_or_else(|| std::io::Error::other("Invalid resource path"))?;
    let dir = directory(root, parent)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir.open_with(name, &options)?;
    let mut digest = Sha1::new();
    let mut buffer = [0; 65536];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    let hash = digest.hexdigest();
    Ok(expected
        .iter()
        .any(|(length, expected)| *length == size && *expected == hash))
}

pub async fn analyze_meta() -> crate::Result<MetaAnalysis> {
    let _usage = usage::lease().await?;
    let directories = dirs()?;
    let refs = references(&directories).await;
    let root_path = identity(&directories.metadata_dir());
    let mut warnings = refs.warnings.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut groups = BTreeMap::new();
        let mut files = Vec::new();
        match opened_root(&directories) {
            Ok(root) => collect(
                &root,
                "",
                &refs,
                &mut groups,
                &mut files,
                &mut warnings,
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok::<_, std::io::Error>((
            groups.into_values().collect::<Vec<_>>(),
            files,
            warnings,
        ))
    })
    .await
    .map_err(|error| invalid(error.to_string()))??;
    let (groups, files, mut warnings) = result;
    warnings.truncate(100);
    let id = uuid::Uuid::new_v4().to_string();
    let mut plans = PLANS.lock().await;
    plans.retain(|_, plan| plan.created.elapsed() < PLAN_AGE);
    if plans.len() >= 4 {
        plans.clear();
    }
    plans.insert(
        id.clone(),
        CleanupPlan {
            created: std::time::Instant::now(),
            root: root_path,
            files,
        },
    );
    Ok(MetaAnalysis {
        id,
        scanned_at: Utc::now(),
        groups,
        warnings,
    })
}

pub async fn clean_meta(
    id: String,
    groups: Vec<String>,
) -> crate::Result<MetaCleanupResult> {
    let _scan = SCAN_LOCK.lock().await;
    let usage = usage::exclusive().await?;
    let state = crate::State::get().await?;
    if !state.process_manager.get_all().is_empty() {
        return Err(invalid("Stop running games before cleaning metadata"));
    }
    let directories = dirs()?;
    let refs = references(&directories).await;
    if refs.blocked || refs.busy {
        return Err(invalid(
            "Metadata references could not be verified or installation recovery is pending. Refresh the analysis after resolving it.",
        ));
    }
    let mut plans = PLANS.lock().await;
    let plan = plans
        .get(&id)
        .ok_or_else(|| invalid("Metadata analysis expired. Analyze again."))?;
    if plan.created.elapsed() >= PLAN_AGE
        || plan.root != identity(&directories.metadata_dir())
    {
        return Err(invalid("Metadata analysis expired. Analyze again."));
    }
    let selection: HashSet<_> = groups.into_iter().collect();
    if selection.is_empty()
        || selection
            .iter()
            .any(|group| !plan.files.iter().any(|file| &file.group == group))
    {
        return Err(invalid(
            "Select unused metadata from the current analysis",
        ));
    }
    let plan = plans.remove(&id).unwrap();
    drop(plans);
    let result = tokio::task::spawn_blocking(move || {
        let _usage = usage;
        remove_candidates(&directories, plan.files, &selection, &refs)
    })
    .await
    .map_err(|error| invalid(error.to_string()))??;
    let _ =
        fs::remove_file(dirs()?.caches_dir().join(STORAGE_CACHE_FILE)).await;
    Ok(result)
}

fn remove_candidates(
    directories: &DirectoryInfo,
    files: Vec<Candidate>,
    selection: &HashSet<String>,
    refs: &References,
) -> std::io::Result<MetaCleanupResult> {
    let root = opened_root(directories)?;
    let mut result = MetaCleanupResult::default();
    for file in files
        .into_iter()
        .filter(|file| selection.contains(&file.group))
    {
        if refs.used(&file.relative) != Some(false) {
            result.skipped_files += 1;
            continue;
        }
        let outcome = (|| {
            if file.relative.starts_with("resources/")
                && !verified_legacy_copy(
                    &root,
                    &file.relative,
                    file.size,
                    refs,
                )?
            {
                return Ok(false);
            }
            let (parent, name) = file
                .relative
                .rsplit_once('/')
                .ok_or_else(|| std::io::Error::other("Invalid candidate"))?;
            let dir = directory(&root, parent)?;
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let handle = dir.open_with(name, &options)?;
            let metadata = handle.metadata()?;
            if !metadata.is_file()
                || metadata.len() != file.size
                || metadata.modified().ok() != file.modified
                || metadata.created().ok() != file.created
            {
                return Ok(false);
            }
            drop(handle);
            dir.remove_file(name)?;
            drop(dir);
            prune_empty_parents(&root, parent);
            Ok::<_, std::io::Error>(true)
        })();
        match outcome {
            Ok(true) => {
                result.removed_files += 1;
                result.removed_bytes += file.size;
                let removed = result
                    .removed_groups
                    .entry(file.group.clone())
                    .or_default();
                removed.files += 1;
                removed.bytes += file.size;
            }
            Ok(false) => result.skipped_files += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                result.skipped_files += 1
            }
            Err(error) => {
                if result.failures.len() < 100 {
                    result.failures.push(format!("{}: {error}", file.relative));
                }
            }
        }
    }
    Ok::<_, std::io::Error>(result)
}

fn prune_empty_parents(root: &Dir, relative: &str) {
    let mut path = relative;
    while let Some((parent, name)) = path.rsplit_once('/') {
        let Ok(directory) = directory(root, parent) else {
            break;
        };
        if directory.remove_dir(name).is_err() {
            break;
        }
        path = parent;
    }
}

#[cfg(test)]
mod tests;
