//! Parsing and resolving local Minecraft version JSONs for directly linked
//! launcher installations.
//!
//! HMCL and PCL/PCL-CE intentionally use different resolvers. The fixed
//! upstream references are:
//! - HMCL `DefaultGameRepositorySnapshot.resolve`, `GameInstanceManifest.merge`,
//!   `GameInstancePatch.merge`, and `Arguments.merge` at commit
//!   `083dbb18ade1c935e2e56d0bdefcd718be1e2ed6`;
//! - PCL `McInstance.JsonObject` in `ModMinecraft.vb` at commit
//!   `639de1b48a44326cbd5465579295cecf23d9056a`;
//! - PCL-CE `McInstance.JsonObject` and `JsonCompat.Merge` at commit
//!   `aa3b81c6afb3cd1896dda271578b002066512177`.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use daedalus::minecraft::{
    Argument, ArgumentType, AssetIndex, Download, DownloadType, JavaVersion,
    Library, LoggingConfiguration, LoggingSide, VersionType,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::direct_link::LinkedLauncherDialect;
use super::version_json::{parse_merged_version_json, parse_version_json};
use super::{pcl_ce_arguments, pcl_ce_json};

/// A library plus community-launcher fields which daedalus intentionally does
/// not model. Keeping these fields is required for direct-link path resolution.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkedLibrary {
    #[serde(flatten)]
    pub library: Library,
    #[serde(default, alias = "MMC-hint")]
    pub hint: Option<String>,
    #[serde(default, alias = "MMC-filename")]
    pub filename: Option<String>,
}

impl LinkedLibrary {
    pub fn classpath_relative_path(&self) -> crate::Result<PathBuf> {
        if let Some(path) = self
            .library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.artifact.as_ref())
            .and_then(|artifact| artifact.path.as_deref())
        {
            return Ok(PathBuf::from(path));
        }

        // HMCL's community fields preserve an explicit local filename when a
        // manifest does not carry downloads.artifact.path.
        if self.hint.as_deref() == Some("local")
            && let Some(filename) = self.filename.as_deref()
        {
            return Ok(PathBuf::from(filename));
        }

        Ok(daedalus::get_path_from_artifact(&self.library.name)?.into())
    }
}

fn clone_model<T>(value: &T) -> crate::Result<T>
where
    T: Serialize + DeserializeOwned,
{
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RawVersionDocument {
    id: String,
    #[serde(default)]
    inherits_from: Option<String>,
    #[serde(default)]
    jar: Option<String>,
    #[serde(default)]
    main_class: Option<String>,
    #[serde(default)]
    minecraft_arguments: Option<String>,
    #[serde(default)]
    arguments: Option<HashMap<ArgumentType, Vec<Argument>>>,
    #[serde(default)]
    assets: Option<String>,
    #[serde(default)]
    asset_index: Option<RawAssetIndex>,
    #[serde(default)]
    java_version: Option<JavaVersion>,
    #[serde(default)]
    libraries: Vec<LinkedLibrary>,
    #[serde(default)]
    logging: Option<HashMap<LoggingSide, LoggingConfiguration>>,
    #[serde(default)]
    downloads: Option<HashMap<DownloadType, Download>>,
    #[serde(
        rename = "type",
        default,
        deserialize_with = "deserialize_version_type"
    )]
    type_: Option<VersionType>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    release_time: Option<String>,
    #[serde(default)]
    minimum_launcher_version: Option<u32>,
    #[serde(default)]
    compliance_level: Option<u32>,
    #[serde(default)]
    root: Option<bool>,
    #[serde(default)]
    patches: Option<Vec<RawVersionPatch>>,

    // HMCL private per-version settings written into version JSONs.
    #[serde(default)]
    java_args: Option<String>,
    #[serde(default)]
    min_memory: Option<i64>,
    #[serde(default)]
    max_memory: Option<i64>,
    #[serde(default)]
    perm_size: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    server_ip: Option<String>,
    #[serde(default)]
    server_port: Option<u16>,
    #[serde(default)]
    uses_global: Option<bool>,
    #[serde(default)]
    java_dir: Option<String>,
    #[serde(default)]
    default_java_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct RawVersionPatch {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    minecraft_arguments: Option<String>,
    #[serde(default)]
    arguments: Option<HashMap<ArgumentType, Vec<Argument>>>,
    #[serde(default)]
    main_class: Option<String>,
    #[serde(default)]
    inherits_from: Option<String>,
    #[serde(default)]
    jar: Option<String>,
    #[serde(default)]
    asset_index: Option<RawAssetIndex>,
    #[serde(default)]
    assets: Option<String>,
    #[serde(default)]
    compliance_level: Option<u32>,
    #[serde(default)]
    java_version: Option<JavaVersion>,
    #[serde(default)]
    libraries: Vec<LinkedLibrary>,
    #[serde(default)]
    downloads: Option<HashMap<DownloadType, Download>>,
    #[serde(default)]
    logging: Option<HashMap<LoggingSide, LoggingConfiguration>>,
    #[serde(
        rename = "type",
        default,
        deserialize_with = "deserialize_version_type"
    )]
    type_: Option<VersionType>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    release_time: Option<String>,
    #[serde(default)]
    minimum_launcher_version: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RawAssetIndex {
    id: String,
    sha1: Option<String>,
    size: Option<u32>,
    total_size: Option<u32>,
    url: Option<String>,
}

impl From<RawAssetIndex> for AssetIndex {
    fn from(raw: RawAssetIndex) -> Self {
        Self {
            id: raw.id,
            sha1: raw.sha1.unwrap_or_default(),
            size: raw.size.unwrap_or_default(),
            total_size: raw.total_size.unwrap_or_default(),
            url: raw.url.unwrap_or_default(),
        }
    }
}

fn deserialize_version_type<'de, D>(
    deserializer: D,
) -> Result<Option<VersionType>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(|raw| match raw.to_ascii_lowercase().as_str() {
        "release" => Some(VersionType::Release),
        "snapshot" | "fool" => Some(VersionType::Snapshot),
        "old_alpha" | "old-alpha" => Some(VersionType::OldAlpha),
        "old_beta" | "old-beta" => Some(VersionType::OldBeta),
        _ => None,
    }))
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct HmclVersionSettings {
    pub java_args: Option<String>,
    pub min_memory: Option<i64>,
    pub max_memory: Option<i64>,
    pub perm_size: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub server_ip: Option<String>,
    pub server_port: Option<u16>,
    pub uses_global: Option<bool>,
    pub java_dir: Option<String>,
    pub default_java_path: Option<String>,
}

impl HmclVersionSettings {
    fn overlay(&mut self, child: &RawVersionDocument) {
        self.java_args = child.java_args.clone().or(self.java_args.take());
        self.min_memory = child.min_memory.or(self.min_memory);
        self.max_memory = child.max_memory.or(self.max_memory);
        self.perm_size = child.perm_size.clone().or(self.perm_size.take());
        self.width = child.width.or(self.width);
        self.height = child.height.or(self.height);
        self.fullscreen = child.fullscreen.or(self.fullscreen);
        self.server_ip = child.server_ip.clone().or(self.server_ip.take());
        self.server_port = child.server_port.or(self.server_port);
        self.uses_global = child.uses_global.or(self.uses_global);
        self.java_dir = child.java_dir.clone().or(self.java_dir.take());
        self.default_java_path = child
            .default_java_path
            .clone()
            .or(self.default_java_path.take());
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MergedVersion {
    pub id: String,
    pub main_class: Option<String>,
    pub jar: Option<String>,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndex>,
    pub java_version: Option<JavaVersion>,
    pub libraries: Vec<LinkedLibrary>,
    pub arguments: Option<HashMap<ArgumentType, Vec<Argument>>>,
    pub minecraft_arguments: Option<String>,
    pub logging: Option<HashMap<LoggingSide, LoggingConfiguration>>,
    pub downloads: Option<HashMap<DownloadType, Download>>,
    pub type_: Option<VersionType>,
    pub release_time: Option<DateTime<Utc>>,
    pub time: Option<DateTime<Utc>>,
    pub minimum_launcher_version: Option<u32>,
    pub compliance_level: Option<u32>,
    pub hmcl_settings: HmclVersionSettings,
}

/// Checks whether a JSON file is a Minecraft version document rather than
/// runtime state such as `usercache.json`. Version directories commonly keep
/// both kinds of JSON side by side.
fn is_minecraft_version_document(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<Value>(&contents).ok())
        .is_some_and(|value| {
            value
                .as_object()
                .and_then(|object| object.get("id"))
                .and_then(Value::as_str)
                .is_some_and(|id| !id.trim().is_empty())
        })
}

/// Finds the version JSON used by both import validation and launch resolution.
/// The conventional same-name manifest wins; otherwise exactly one Minecraft
/// version manifest in the version directory is accepted.
pub fn discover_version_json(
    root: &Path,
    version_id: &str,
) -> crate::Result<PathBuf> {
    let version_dir = root.join("versions").join(version_id);
    let conventional = version_dir.join(format!("{version_id}.json"));
    if conventional.is_file() && is_minecraft_version_document(&conventional) {
        return Ok(conventional);
    }

    let mut json_files = std::fs::read_dir(&version_dir)
        .map_err(|error| {
            crate::ErrorKind::FSError(format!(
                "Failed to read local version directory {}: {error}",
                version_dir.display()
            ))
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && is_minecraft_version_document(path)
        })
        .collect::<Vec<_>>();
    json_files.sort();

    match json_files.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(crate::ErrorKind::InputError(format!(
            "No version JSON found in {}",
            version_dir.display()
        ))
        .into()),
        _ => Err(crate::ErrorKind::InputError(format!(
            "Multiple version JSON files found in {}; expected {} or one unique fallback",
            version_dir.display(),
            conventional.display()
        ))
        .into()),
    }
}

/// Compatibility entry point used by slice A validation. Generic local chains
/// follow standard Mojang inheritance semantics and do not consume HMCL patches.
/// Currently exercised by the direct-link resolution tests only, so the
/// non-test build treats it as dead.
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_version_chain(
    root: &Path,
    version_id: &str,
) -> crate::Result<MergedVersion> {
    load_generic_version_chain_from(root, version_id, None)
}

pub fn load_version_for_dialect(
    root: &Path,
    version_id: &str,
    version_json: Option<&Path>,
    dialect: LinkedLauncherDialect,
) -> crate::Result<MergedVersion> {
    match dialect {
        LinkedLauncherDialect::Hmcl => {
            load_hmcl_version_chain_from(root, version_id, version_json)
        }
        LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe => {
            load_pcl_version_chain(root, version_id, version_json, dialect)
        }
        LinkedLauncherDialect::Generic => {
            load_generic_version_chain_from(root, version_id, version_json)
        }
    }
}

fn load_generic_version_chain_from(
    root: &Path,
    version_id: &str,
    version_json: Option<&Path>,
) -> crate::Result<MergedVersion> {
    let mut requested = match version_json {
        Some(path) => read_document_path(path)?,
        None => read_document(root, version_id)?,
    };
    requested.id = version_id.to_string();
    let mut visited = HashSet::new();
    let mut merged = resolve_generic_document(root, requested, &mut visited)?;
    merged.id = version_id.to_string();
    Ok(merged)
}

fn resolve_generic_document(
    root: &Path,
    document: RawVersionDocument,
    visited: &mut HashSet<String>,
) -> crate::Result<MergedVersion> {
    if !visited.insert(document.id.clone()) {
        return Err(crate::ErrorKind::InputError(format!(
            "Circular inheritsFrom chain detected at \"{}\"",
            document.id
        ))
        .into());
    }

    let mut merged = if let Some(parent_id) = document.inherits_from.as_deref()
    {
        let parent = read_document(root, parent_id)?;
        let parent = resolve_generic_document(root, parent, visited)?;
        merge_hmcl_document(parent, &document)?
    } else {
        merged_from_document(&document)?
    };
    if merged.jar.is_none() {
        merged.jar =
            Some(document.jar.clone().unwrap_or_else(|| document.id.clone()));
    }
    merged.id = document.id.clone();
    visited.remove(&document.id);
    Ok(merged)
}

fn load_hmcl_version_chain_from(
    root: &Path,
    version_id: &str,
    version_json: Option<&Path>,
) -> crate::Result<MergedVersion> {
    let mut requested = match version_json {
        Some(path) => read_document_path(path)?,
        None => read_document(root, version_id)?,
    };
    requested.id = version_id.to_string();
    let requested_id = version_id.to_string();
    let mut visited = HashSet::new();
    let mut merged = resolve_hmcl_document(root, requested, &mut visited)?;
    merged.id = requested_id;
    merged.libraries = hmcl_unique_libraries(merged.libraries);
    Ok(merged)
}

fn resolve_hmcl_document(
    root: &Path,
    document: RawVersionDocument,
    visited: &mut HashSet<String>,
) -> crate::Result<MergedVersion> {
    if !visited.insert(document.id.clone()) {
        return Err(crate::ErrorKind::InputError(format!(
            "Circular inheritsFrom chain detected at \"{}\"",
            document.id
        ))
        .into());
    }

    // HMCL DefaultGameRepositorySnapshot.resolve: root manifests with patches
    // start from an empty manifest; ordinary manifests retain their own fields.
    let mut merged = if let Some(parent_id) = document.inherits_from.as_deref()
    {
        let parent = read_document(root, parent_id)?;
        let parent = resolve_hmcl_document(root, parent, visited)?;
        merge_hmcl_document(parent, &document)?
    } else if document.root == Some(true) && document.patches.is_some() {
        let mut merged = empty_merged(document.id.clone());
        merged.hmcl_settings.overlay(&document);
        merged
    } else {
        merged_from_document(&document)?
    };

    let mut patches: Vec<RawVersionPatch> =
        clone_model(&document.patches)?.unwrap_or_default();
    patches.sort_by_key(|patch| patch.priority.unwrap_or(i32::MIN));
    for patch in patches {
        apply_hmcl_patch(&mut merged, patch);
    }

    if merged.jar.is_none() {
        merged.jar =
            Some(document.jar.clone().unwrap_or_else(|| document.id.clone()));
    }
    merged.id = document.id.clone();
    visited.remove(&document.id);
    Ok(merged)
}

fn merge_hmcl_document(
    mut parent: MergedVersion,
    child: &RawVersionDocument,
) -> crate::Result<MergedVersion> {
    overlay_document_scalars(&mut parent, child)?;
    parent.arguments = merge_arguments(
        parent.arguments.take(),
        clone_model(&child.arguments)?,
    );
    let mut libraries = child.libraries.clone();
    libraries.append(&mut parent.libraries);
    parent.libraries = libraries;
    parent.minimum_launcher_version = parent
        .minimum_launcher_version
        .max(child.minimum_launcher_version);
    // HMCL carries this nullable field directly from the child rather than
    // inheriting the parent's value when it is absent.
    parent.compliance_level = child.compliance_level;
    parent.hmcl_settings.overlay(child);
    parent.id = child.id.clone();
    Ok(parent)
}

fn apply_hmcl_patch(merged: &mut MergedVersion, patch: RawVersionPatch) {
    if patch.minecraft_arguments.is_some() {
        merged.minecraft_arguments = patch.minecraft_arguments;
    }
    merged.arguments =
        merge_arguments(merged.arguments.take(), patch.arguments);
    if patch.main_class.is_some() {
        merged.main_class = patch.main_class;
    }
    // HMCL GameInstancePatch.merge deliberately keeps parent.jar().
    if patch.asset_index.is_some() {
        merged.asset_index = patch.asset_index.map(Into::into);
    }
    if patch.assets.is_some() {
        merged.assets = patch.assets;
    }
    merged.compliance_level = patch.compliance_level;
    if patch.java_version.is_some() {
        merged.java_version = patch.java_version;
    }
    if !patch.libraries.is_empty() {
        let mut libraries = patch.libraries;
        libraries.append(&mut merged.libraries);
        merged.libraries = libraries;
    }
    if patch.downloads.is_some() {
        merged.downloads = patch.downloads;
    }
    if patch.logging.is_some() {
        merged.logging = patch.logging;
    }
    if patch.type_.is_some() {
        merged.type_ = patch.type_;
    }
    if patch.time.is_some() {
        merged.time = parse_date(&patch.time);
    }
    if patch.release_time.is_some() {
        merged.release_time = parse_date(&patch.release_time);
    }
    merged.minimum_launcher_version = merged
        .minimum_launcher_version
        .max(patch.minimum_launcher_version);
}

fn load_pcl_version_chain(
    root: &Path,
    version_id: &str,
    version_json: Option<&Path>,
    dialect: LinkedLauncherDialect,
) -> crate::Result<MergedVersion> {
    let mut visited = HashSet::new();
    let mut argument_layers = Vec::new();
    let (path, mut value) = resolve_pcl_json(
        root,
        version_id,
        version_json,
        dialect,
        &mut visited,
        &mut argument_layers,
    )?;
    if dialect == LinkedLauncherDialect::PclCe {
        collect_pcl_ce_arguments(&mut value, argument_layers, &path)?;
        pcl_ce_arguments::normalize(&mut value, &path)?;
        value["id"] = Value::String(version_id.to_string());
        if let Some(object) = value.as_object_mut() {
            object.remove("patches");
        }
    }
    let mut document: RawVersionDocument =
        parse_merged_version_json(&path, &value)?;
    document.id = version_id.to_string();
    merged_from_document(&document)
}

const MAX_PCL_CE_INHERITANCE_DEPTH: usize = 64;
const PCL_CE_ARGUMENT_INHERITANCE_BUDGET: usize = 4 * 1024 * 1024;

fn resolve_pcl_json(
    root: &Path,
    version_id: &str,
    version_json: Option<&Path>,
    dialect: LinkedLauncherDialect,
    visited: &mut HashSet<String>,
    argument_layers: &mut Vec<Option<Value>>,
) -> crate::Result<(PathBuf, Value)> {
    if !visited.insert(version_id.to_string()) {
        return Err(crate::ErrorKind::InputError(format!(
            "Circular inheritsFrom chain detected at \"{version_id}\""
        ))
        .into());
    }
    if dialect == LinkedLauncherDialect::PclCe
        && visited.len() > MAX_PCL_CE_INHERITANCE_DEPTH
    {
        return Err(crate::ErrorKind::InputError(
            "PCL-CE version inheritance chain is too deep".into(),
        )
        .into());
    }

    let path = match version_json {
        Some(path) => path.to_path_buf(),
        None if dialect == LinkedLauncherDialect::PclCe => {
            pcl_ce_json::discover(&root.join("versions").join(version_id))?
        }
        None => discover_version_json(root, version_id)?,
    };
    let mut child = if dialect == LinkedLauncherDialect::PclCe {
        pcl_ce_json::read(&path)?
    } else {
        read_document_path(&path)?
    };

    // PCL McInstance.JsonObject converts HMCL-format documents by sorting
    // patches ascending and repeatedly applying JObject.Merge. PCL-CE's
    // JsonCompat.Merge defines the same recursive-object/concatenated-array
    // behavior explicitly.
    let hmcl_format =
        child.get("patches").is_some() && child.get("time").is_none();
    if hmcl_format {
        let mut patches = child
            .get_mut("patches")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default();
        patches.sort_by_key(|patch| {
            patch.get("priority").and_then(Value::as_i64).unwrap_or(0)
        });
        let mut current = Value::Object(Map::new());
        for patch in patches {
            // PCL ignores HMCL-format patches whose id is absent or null.
            if patch.get("id").is_some_and(|id| !id.is_null()) {
                pcl_merge(&mut current, &patch);
            }
        }
        child = current;
        child["id"] = Value::String(version_id.to_string());
        if let Some(object) = child.as_object_mut() {
            object.remove("inheritsFrom");
        }
    }

    if dialect == LinkedLauncherDialect::PclCe {
        argument_layers.push(child.get("arguments").map(|arguments| {
            if let Some(groups) = arguments.as_object() {
                Value::Object(
                    ["game", "jvm"]
                        .into_iter()
                        .filter_map(|group| {
                            groups
                                .get(group)
                                .map(|value| (group.to_string(), value.clone()))
                        })
                        .collect(),
                )
            } else if arguments.is_null() {
                Value::Null
            } else {
                Value::Bool(false)
            }
        }));
    }
    let parent_id = child
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .filter(|parent| *parent != version_id)
        .map(str::to_string);
    let result = if let Some(parent_id) = parent_id {
        let (_, mut parent) = resolve_pcl_json(
            root,
            &parent_id,
            None,
            dialect,
            visited,
            argument_layers,
        )?;
        let inherited_jar = parent
            .get("jar")
            .and_then(Value::as_str)
            .or_else(|| parent.get("id").and_then(Value::as_str))
            .map(str::to_string);

        if dialect == LinkedLauncherDialect::Pcl {
            // PCL classic explicitly restores libraries as child-first,
            // parent-last after JObject.Merge. PCL-CE uses JsonCompat.Merge
            // directly, so its arrays remain parent-first, child-last.
            let mut libraries = child
                .get("libraries")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            libraries.extend(
                parent
                    .get("libraries")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            pcl_merge(&mut parent, &child);
            if !libraries.is_empty() {
                parent["libraries"] = Value::Array(libraries);
            }
        } else {
            pcl_merge(&mut parent, &child);
        }
        if parent.get("jar").is_none()
            && let Some(inherited_jar) = inherited_jar
        {
            parent["jar"] = Value::String(inherited_jar);
        }
        parent
    } else {
        child
    };
    visited.remove(version_id);
    Ok((path, result))
}

/// PCL-CE merges each ancestor's already resolved argument groups into the
/// requested document, then collects that document followed by each resolved
/// ancestor. For A <- B <- C, this yields AABC, AB, A before grouped dedup.
fn collect_pcl_ce_arguments(
    document: &mut Value,
    layers: Vec<Option<Value>>,
    path: &Path,
) -> crate::Result<()> {
    if layers.len() < 2 {
        return Ok(());
    }

    let mut budget = PCL_CE_ARGUMENT_INHERITANCE_BUDGET;
    let mut resolved: Vec<Option<Value>> = Vec::with_capacity(layers.len());
    for mut arguments in layers.into_iter().rev() {
        for parent in resolved.iter().rev().flatten() {
            match &arguments {
                None => {
                    take_argument_budget(
                        &mut budget,
                        argument_weight(parent),
                        path,
                    )?;
                    arguments = Some(parent.clone());
                }
                Some(child) if child.is_object() && parent.is_object() => {
                    take_argument_budget(
                        &mut budget,
                        argument_weight(parent)
                            .saturating_add(argument_weight(child)),
                        path,
                    )?;
                    let mut merged = parent.clone();
                    pcl_merge(&mut merged, child);
                    arguments = Some(merged);
                }
                _ => {}
            }
        }
        resolved.push(arguments);
    }

    let Some(Some(arguments)) = resolved.pop() else {
        return Ok(());
    };
    let Value::Object(mut groups) = arguments else {
        document["arguments"] = arguments;
        return Ok(());
    };
    for group in ["game", "jvm"] {
        let Some(Value::Array(entries)) = groups.get_mut(group) else {
            continue;
        };
        for arguments in resolved.iter().rev().flatten() {
            if arguments.is_null() {
                continue;
            }
            let Some(groups) = arguments.as_object() else {
                return Err(crate::ErrorKind::InputError(format!(
                    "Invalid inherited arguments while resolving {}: expected an argument group object",
                    path.display()
                )).into());
            };
            let Some(values) =
                groups.get(group).filter(|value| !value.is_null())
            else {
                continue;
            };
            let Some(values) = values.as_array() else {
                return Err(crate::ErrorKind::InputError(format!(
                    "Invalid inherited arguments.{group} while resolving {}: expected an argument array",
                    path.display()
                )).into());
            };
            for value in values {
                take_argument_budget(
                    &mut budget,
                    argument_weight(value),
                    path,
                )?;
                entries.push(value.clone());
            }
        }
    }
    document["arguments"] = Value::Object(groups);
    Ok(())
}

fn argument_weight(value: &Value) -> usize {
    size_of::<Value>()
        + match value {
            Value::Array(items) => items.iter().map(argument_weight).sum(),
            Value::Object(object) => object
                .iter()
                .map(|(key, value)| key.len() + argument_weight(value))
                .sum(),
            Value::String(value) => value.len(),
            _ => 0,
        }
}

fn take_argument_budget(
    budget: &mut usize,
    weight: usize,
    path: &Path,
) -> crate::Result<()> {
    *budget = budget.checked_sub(weight).ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "PCL-CE inherited arguments expand too far in {}",
            path.display()
        ))
    })?;
    Ok(())
}

/// Newtonsoft JObject.Merge / PCL-CE JsonCompat.Merge semantics used by PCL:
/// object members recurse, arrays append, and all other child values replace.
fn pcl_merge(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, source_value) in source {
                if let Some(target_value) = target.get_mut(key) {
                    pcl_merge(target_value, source_value);
                } else {
                    target.insert(key.clone(), source_value.clone());
                }
            }
        }
        (Value::Array(target), Value::Array(source)) => {
            target.extend(source.iter().cloned());
        }
        (target, source) => *target = source.clone(),
    }
}

fn read_document(
    root: &Path,
    version_id: &str,
) -> crate::Result<RawVersionDocument> {
    read_document_path(&discover_version_json(root, version_id)?)
}

fn read_document_path<T: DeserializeOwned>(path: &Path) -> crate::Result<T> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Failed to read local version JSON {}: {error}",
            path.display()
        ))
    })?;
    parse_version_json(path, content.as_bytes(), true)
}

fn merged_from_document(
    document: &RawVersionDocument,
) -> crate::Result<MergedVersion> {
    let mut settings = HmclVersionSettings::default();
    settings.overlay(document);
    Ok(MergedVersion {
        id: document.id.clone(),
        main_class: document.main_class.clone(),
        jar: document.jar.clone().or_else(|| Some(document.id.clone())),
        assets: document.assets.clone(),
        asset_index: document.asset_index.clone().map(Into::into),
        java_version: clone_model(&document.java_version)?,
        libraries: document.libraries.clone(),
        arguments: clone_model(&document.arguments)?,
        minecraft_arguments: document.minecraft_arguments.clone(),
        logging: clone_model(&document.logging)?,
        downloads: clone_model(&document.downloads)?,
        type_: document.type_.clone(),
        release_time: parse_date(&document.release_time),
        time: parse_date(&document.time),
        minimum_launcher_version: document.minimum_launcher_version,
        compliance_level: document.compliance_level,
        hmcl_settings: settings,
    })
}

fn empty_merged(id: String) -> MergedVersion {
    MergedVersion {
        id,
        main_class: None,
        jar: None,
        assets: None,
        asset_index: None,
        java_version: None,
        libraries: Vec::new(),
        arguments: None,
        minecraft_arguments: None,
        logging: None,
        downloads: None,
        type_: None,
        release_time: None,
        time: None,
        minimum_launcher_version: None,
        compliance_level: None,
        hmcl_settings: HmclVersionSettings::default(),
    }
}

fn overlay_document_scalars(
    merged: &mut MergedVersion,
    child: &RawVersionDocument,
) -> crate::Result<()> {
    if child.main_class.is_some() {
        merged.main_class = child.main_class.clone();
    }
    if child.jar.is_some() {
        merged.jar = child.jar.clone();
    }
    if child.minecraft_arguments.is_some() {
        merged.minecraft_arguments = child.minecraft_arguments.clone();
    }
    if child.assets.is_some() {
        merged.assets = child.assets.clone();
    }
    if child.asset_index.is_some() {
        merged.asset_index = child.asset_index.clone().map(Into::into);
    }
    if child.java_version.is_some() {
        merged.java_version = clone_model(&child.java_version)?;
    }
    if child.logging.is_some() {
        merged.logging = clone_model(&child.logging)?;
    }
    if child.downloads.is_some() {
        merged.downloads = clone_model(&child.downloads)?;
    }
    if child.type_.is_some() {
        merged.type_ = child.type_.clone();
    }
    if child.release_time.is_some() {
        merged.release_time = parse_date(&child.release_time);
    }
    if child.time.is_some() {
        merged.time = parse_date(&child.time);
    }
    if child.compliance_level.is_some() {
        merged.compliance_level = child.compliance_level;
    }
    Ok(())
}

fn merge_arguments(
    parent: Option<HashMap<ArgumentType, Vec<Argument>>>,
    child: Option<HashMap<ArgumentType, Vec<Argument>>>,
) -> Option<HashMap<ArgumentType, Vec<Argument>>> {
    match (parent, child) {
        (None, None) => None,
        (Some(merged), None) | (None, Some(merged)) => Some(merged),
        (Some(mut parent), Some(child)) => {
            for (type_, values) in child {
                parent.entry(type_).or_default().extend(values);
            }
            Some(parent)
        }
    }
}

fn hmcl_unique_libraries(libraries: Vec<LinkedLibrary>) -> Vec<LinkedLibrary> {
    let mut result: Vec<LinkedLibrary> = Vec::new();
    for library in libraries {
        let Some((group, artifact, version)) =
            coordinate_parts(&library.library.name)
        else {
            result.push(library);
            continue;
        };
        let rules = serde_json::to_value(&library.library.rules).ok();
        let mut duplicate = false;
        for existing in &mut result {
            let Some((other_group, other_artifact, other_version)) =
                coordinate_parts(&existing.library.name)
            else {
                continue;
            };
            if group != other_group
                || artifact != other_artifact
                || rules != serde_json::to_value(&existing.library.rules).ok()
            {
                continue;
            }

            match compare_versions(version, other_version) {
                Ordering::Greater => *existing = library.clone(),
                Ordering::Equal => {
                    // HMCL keeps same-coordinate entries with distinct native
                    // payloads, but collapses true duplicates and prefers the
                    // richer serialized declaration.
                    let left = serde_json::to_value(&library).ok();
                    let right = serde_json::to_value(&*existing).ok();
                    if left == right {
                        duplicate = true;
                        break;
                    }
                    let library_is_native = hmcl_library_is_native(&library);
                    let existing_is_native = hmcl_library_is_native(existing);
                    if library.library.name != existing.library.name
                        || library_is_native != existing_is_native
                    {
                        continue;
                    }
                    if left.as_ref().map(|value| value.to_string().len())
                        > right.as_ref().map(|value| value.to_string().len())
                    {
                        *existing = library.clone();
                    }
                }
                Ordering::Less => {}
            }
            duplicate = true;
            break;
        }
        if !duplicate {
            result.push(library);
        }
    }
    result
}

fn hmcl_library_is_native(library: &LinkedLibrary) -> bool {
    library.library.natives.is_some()
        || library
            .library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.as_ref())
            .is_some_and(|classifiers| {
                classifiers
                    .keys()
                    .any(|classifier| classifier.starts_with("native"))
            })
}

fn coordinate_parts(name: &str) -> Option<(&str, &str, &str)> {
    let mut parts = name.split(':');
    Some((parts.next()?, parts.next()?, parts.next()?))
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let split = |value: &str| {
        value
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
            .map(|part| part.to_ascii_lowercase())
            .collect::<Vec<_>>()
    };
    let left = split(left);
    let right = split(right);
    for (left, right) in left.iter().zip(&right) {
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn parse_date(raw: &Option<String>) -> Option<DateTime<Utc>> {
    let raw = raw.as_deref()?;
    DateTime::parse_from_rfc3339(raw)
        .map(|date| date.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|date| date.and_utc())
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn write_json(root: &Path, id: &str, filename: &str, value: Value) {
        let directory = root.join("versions").join(id);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(filename),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn linked_version_rules_keep_existing_semantics() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "demo",
            "client.json",
            json!({
                "id": "demo",
                "arguments": {"game": [
                    "--plain",
                    {"value": "--implicit-allow"},
                    {"value": ["--explicit-allow"], "rules": []},
                    {"value": "--denied", "rules": [
                        {"action": "allow"}, {"action": "disallow"}
                    ]}
                ]}
            }),
        );
        let version = load_version_chain(dir.path(), "demo").unwrap();
        let arguments = version.arguments.unwrap();
        let game = &arguments[&ArgumentType::Game];
        assert!(
            matches!(&game[0], Argument::Normal(value) if value == "--plain")
        );
        for (argument, expected) in game[1..].iter().zip([true, true, false]) {
            let Argument::Ruled { rules, .. } = argument else {
                panic!("expected a ruled argument");
            };
            assert_eq!(
                crate::launcher::parse_rules(
                    rules,
                    "x86_64",
                    &crate::instance::QuickPlayType::None,
                    false,
                ),
                expected,
            );
        }
    }

    #[test]
    fn inherited_hmcl_patch_reports_bad_rules_without_values() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "child",
            "child.json",
            json!({"id": "child", "inheritsFrom": "base"}),
        );
        let path = dir.path().join("versions").join("base").join("client.json");
        for (rules, location, expected) in [
            (Value::Null, "rules", "an array of rules"),
            (
                json!([{"action": "private-test-action"}]),
                "rules[0].action",
                "allow or disallow",
            ),
            (
                json!([{"action": "allow", "features": {"is_demo_user": "private-test-feature"}}]),
                "rules[0].features",
                "an object with boolean feature flags",
            ),
        ] {
            write_json(
                dir.path(),
                "base",
                "client.json",
                json!({
                    "id": "base",
                    "patches": [{}, {"arguments": {"jvm": [
                        "-Xmx1G", {"rules": rules, "value": "private-test-token"}
                    ]}}]
                }),
            );
            let error = load_version_for_dialect(
                dir.path(),
                "child",
                None,
                LinkedLauncherDialect::Hmcl,
            )
            .unwrap_err();
            let message = error.to_string();
            assert!(message.contains(&path.display().to_string()), "{message}");
            assert!(
                message.contains(&format!(
                    "patches[1].arguments.jvm[1].{location}"
                )),
                "{message}"
            );
            assert!(
                message.contains(&format!("expected {expected}")),
                "{message}"
            );
            assert!(!message.contains("private-test-"));
        }
    }

    #[test]
    fn pcl_reports_invalid_argument_at_merged_index() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({"id": "base", "arguments": {"game": ["--plain"]}}),
        );
        write_json(
            dir.path(),
            "child",
            "client.json",
            json!({
                "id": "child",
                "inheritsFrom": "base",
                "arguments": {"game": [{"value": ["private-test-token", 10]}]}
            }),
        );
        let path = dir.path().join("versions/child/client.json");
        let error = load_version_for_dialect(
            dir.path(),
            "child",
            Some(&path),
            LinkedLauncherDialect::Pcl,
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("merged version JSON"));
        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains("arguments.game[1].value"));
        assert!(message.contains("expected a string or an array of strings"));
        assert!(!message.contains("private-test-token"));
    }

    #[test]
    fn pcl_ce_merges_lenient_parent_and_child_before_interpreting_arguments() {
        let dir = tempdir().unwrap();
        let versions = dir.path().join("versions");
        for (folder, filename, contents) in [
            (
                "base",
                "base.json",
                r#"{
                // Base metadata from another launcher
                "ID":"base", "ARGUMENTS":{"GAME":["--base"], "JVM":["-Dbase=true"],},
            }"#,
            ),
            (
                "child",
                "profile.json",
                r#"{
                "InheritsFrom":"base", "Arguments": {
                    "game":[{"Rules":null,"Value":["--numbers",23,true]}, {"rules":[]}],
                    "jvm":null, "future-arguments":{},
                }
            }"#,
            ),
        ] {
            let path = versions.join(folder);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join(filename), contents).unwrap();
        }

        let version = load_version_for_dialect(
            dir.path(),
            "child",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        assert_eq!(version.id, "child");
        assert_eq!(version.jar.as_deref(), Some("base"));
        let arguments = version.arguments.unwrap();
        assert!(!arguments.contains_key(&ArgumentType::Jvm));
        let game = &arguments[&ArgumentType::Game];
        assert_eq!(game.len(), 3);
        assert!(
            matches!(&game[0], Argument::Normal(value) if value == "--base")
        );
        assert!(matches!(&game[1], Argument::Ruled {
            rules,
            value: daedalus::minecraft::ArgumentValue::Many(values),
        } if rules.is_empty() && values == &["--numbers", "23", "true"]));
        assert!(
            matches!(&game[2], Argument::Normal(value) if value == "--base")
        );
        assert!(
            load_version_for_dialect(
                dir.path(),
                "child",
                None,
                LinkedLauncherDialect::Generic
            )
            .is_err()
        );
    }

    #[test]
    fn pcl_ce_collects_already_merged_arguments_from_three_levels() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({
                "id": "base", "arguments": {"game": ["--base", "a"], "jvm": ["-Dbase=1"]},
                "minecraftArguments": "--legacy base",
            }),
        );
        write_json(
            dir.path(),
            "middle",
            "middle.json",
            json!({
                "id": "middle", "inheritsFrom": "base",
                "arguments": {"game": ["--middle", "b"], "jvm": ["-Dmiddle=1"]},
            }),
        );
        write_json(
            dir.path(),
            "leaf",
            "leaf.json",
            json!({
                "id": "leaf", "inheritsFrom": "middle",
                "arguments": {"game": ["--leaf", "c"], "jvm": ["-Dleaf=1"]},
            }),
        );

        let version = load_version_for_dialect(
            dir.path(),
            "leaf",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        assert_eq!(
            version.minecraft_arguments.as_deref(),
            Some("--legacy base")
        );
        let arguments = version.arguments.as_ref().unwrap();
        assert_eq!(
            crate::launcher::pcl_ce_command::game_arguments(
                version.minecraft_arguments.as_deref(),
                Some(&arguments[&ArgumentType::Game]),
                Some("--custom ${version_name}"),
                &[("${version_name}", "Leaf Profile")],
            ),
            [
                "--legacy",
                "base",
                "--base",
                "a",
                "--middle",
                "b",
                "--leaf",
                "c",
                "--custom",
                "Leaf Profile"
            ],
        );
        assert_eq!(
            serde_json::to_value(version.arguments.unwrap()).unwrap(),
            json!({
                "game": ["--base", "a", "--base", "a", "--middle", "b", "--leaf", "c", "--base", "a", "--middle", "b", "--base", "a"],
                "jvm": ["-Dbase=1", "-Dbase=1", "-Dmiddle=1", "-Dleaf=1", "-Dbase=1", "-Dmiddle=1", "-Dbase=1"],
            })
        );

        let other = load_version_for_dialect(
            dir.path(),
            "leaf",
            None,
            LinkedLauncherDialect::Pcl,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(other.arguments.unwrap()).unwrap(),
            json!({
                "game": ["--base", "a", "--middle", "b", "--leaf", "c"],
                "jvm": ["-Dbase=1", "-Dmiddle=1", "-Dleaf=1"],
            })
        );
    }

    #[test]
    fn pcl_ce_gates_collection_on_resolved_null_and_empty_groups() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({
                "id": "base", "arguments": {"game": ["--base"], "jvm": ["-Dbase=1"]},
                "minecraftArguments": "--legacy",
            }),
        );
        write_json(
            dir.path(),
            "middle",
            "middle.json",
            json!({
                "id": "middle", "inheritsFrom": "base",
                "arguments": {"game": 12, "jvm": ["-Dmiddle=1"]},
            }),
        );
        write_json(
            dir.path(),
            "leaf",
            "leaf.json",
            json!({
                "id": "leaf", "inheritsFrom": "middle", "arguments": {"game": null, "jvm": []},
            }),
        );

        let version = load_version_for_dialect(
            dir.path(),
            "leaf",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        assert_eq!(version.minecraft_arguments.as_deref(), Some("--legacy"));
        assert_eq!(
            serde_json::to_value(version.arguments.unwrap()).unwrap(),
            json!({
                "jvm": ["-Dbase=1", "-Dbase=1", "-Dmiddle=1", "-Dbase=1", "-Dmiddle=1", "-Dbase=1"],
            })
        );

        write_json(
            dir.path(),
            "middle",
            "middle.json",
            json!({
                "id": "middle", "inheritsFrom": "base", "arguments": null,
            }),
        );
        write_json(
            dir.path(),
            "leaf",
            "leaf.json",
            json!({
                "id": "leaf", "inheritsFrom": "middle", "arguments": {"game": []},
            }),
        );
        let version = load_version_for_dialect(
            dir.path(),
            "leaf",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(version.arguments.unwrap()).unwrap(),
            json!({
                "game": ["--base", "--base"], "jvm": ["-Dbase=1", "-Dbase=1"],
            })
        );
    }

    #[test]
    fn pcl_ce_limits_expansion_of_pathological_argument_chains() {
        let layers = vec![Some(json!({"game": ["private-test-token"]})); 24];
        let error = collect_pcl_ce_arguments(
            &mut json!({}),
            layers,
            Path::new("leaf.json"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("inherited arguments expand too far"),
            "{error}"
        );
        assert!(error.contains("leaf.json"));
        assert!(!error.contains("private-test-token"));
    }

    #[test]
    fn pcl_ce_reports_invalid_active_values_without_logging_argument_contents()
    {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "test",
            "test.json",
            json!({
                "id": "test", "arguments": {"jvm": ["private-test-token", {"value": 10}]}
            }),
        );
        let err = load_version_for_dialect(
            dir.path(),
            "test",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("arguments.jvm[1].value"), "{err}");
        assert!(!err.contains("private-test-token"));
    }

    #[test]
    fn discovers_same_name_then_unique_json() {
        let dir = tempdir().unwrap();
        write_json(dir.path(), "same", "same.json", json!({"id":"same"}));
        write_json(dir.path(), "same", "other.json", json!({"id":"other"}));
        assert!(
            discover_version_json(dir.path(), "same")
                .unwrap()
                .ends_with("same.json")
        );

        write_json(
            dir.path(),
            "fallback",
            "actual.json",
            json!({"id":"actual"}),
        );
        assert!(
            discover_version_json(dir.path(), "fallback")
                .unwrap()
                .ends_with("actual.json")
        );
    }

    #[test]
    fn ignores_non_version_json_alongside_a_manifest() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "copied-version",
            "actual-version.json",
            json!({"id":"actual-version"}),
        );
        write_json(
            dir.path(),
            "copied-version",
            "usercache.json",
            json!([{ "name": "Player", "uuid": "example" }]),
        );

        assert!(
            discover_version_json(dir.path(), "copied-version")
                .unwrap()
                .ends_with("actual-version.json")
        );
    }

    #[test]
    fn generic_inherits_but_does_not_apply_hmcl_patches() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({"id":"base", "mainClass":"base.Main"}),
        );
        write_json(
            dir.path(),
            "generic",
            "generic.json",
            json!({
                "id":"generic", "inheritsFrom":"base", "mainClass":"child.Main",
                "patches":[{"id":"hmcl-only", "priority":10, "mainClass":"patched.Main"}]
            }),
        );

        let merged = load_version_for_dialect(
            dir.path(),
            "generic",
            None,
            LinkedLauncherDialect::Generic,
        )
        .unwrap();
        assert_eq!(merged.main_class.as_deref(), Some("child.Main"));
    }

    #[test]
    fn hmcl_root_and_patches_follow_priority() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "hmcl",
            "hmcl.json",
            json!({
                "id":"hmcl", "root":true,
                "mainClass":"ignored.Root",
                "patches":[
                    {"id":"loader", "priority":30000, "mainClass":"loader.Main", "libraries":[{"name":"x:loader:1"}]},
                    {"id":"game", "priority":0, "mainClass":"game.Main", "libraries":[{"name":"x:game:1"}]}
                ]
            }),
        );
        let merged = load_version_for_dialect(
            dir.path(),
            "hmcl",
            None,
            LinkedLauncherDialect::Hmcl,
        )
        .unwrap();
        assert_eq!(merged.main_class.as_deref(), Some("loader.Main"));
        assert_eq!(
            merged
                .libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:loader:1", "x:game:1"]
        );
    }

    #[test]
    fn hmcl_inheritance_then_patches_preserve_merge_order_and_parent_jar() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({
                "id":"base", "jar":"vanilla", "mainClass":"base.Main",
                "arguments":{"game":["--base"]},
                "libraries":[{"name":"x:base:1"}]
            }),
        );
        write_json(
            dir.path(),
            "child",
            "child.json",
            json!({
                "id":"child", "inheritsFrom":"base", "mainClass":"child.Main",
                "arguments":{"game":["--child"]},
                "libraries":[{"name":"x:child:1"}],
                "patches":[{
                    "id":"loader", "priority":10, "jar":"ignored-loader",
                    "mainClass":"loader.Main", "arguments":{"game":["--loader"]},
                    "libraries":[{"name":"x:loader:1"}]
                }]
            }),
        );

        let merged = load_version_for_dialect(
            dir.path(),
            "child",
            None,
            LinkedLauncherDialect::Hmcl,
        )
        .unwrap();
        assert_eq!(merged.main_class.as_deref(), Some("loader.Main"));
        assert_eq!(merged.jar.as_deref(), Some("vanilla"));
        assert_eq!(
            merged
                .libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:loader:1", "x:child:1", "x:base:1"]
        );
        let arguments = merged.arguments.unwrap();
        assert_eq!(
            serde_json::to_value(&arguments[&ArgumentType::Game]).unwrap(),
            json!(["--base", "--child", "--loader"])
        );
    }

    #[test]
    fn hmcl_library_dedup_keeps_rule_distinct_entries() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "hmcl",
            "hmcl.json",
            json!({
                "id":"hmcl",
                "libraries":[
                    {"name":"x:library:1", "rules":[{"action":"allow", "os":{"name":"linux"}}]},
                    {"name":"x:library:2", "rules":[{"action":"allow", "os":{"name":"windows"}}]},
                    {"name":"x:plain:1"},
                    {"name":"x:plain:2"}
                ]
            }),
        );

        let merged = load_version_for_dialect(
            dir.path(),
            "hmcl",
            None,
            LinkedLauncherDialect::Hmcl,
        )
        .unwrap();
        assert_eq!(
            merged
                .libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:library:1", "x:library:2", "x:plain:2"]
        );
    }

    #[test]
    fn pcl_hmcl_format_sorts_patches_and_ignores_null_ids() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "pcl",
            "pcl.json",
            json!({
                "id":"pcl", "patches":[
                    {"id":"loader", "priority":20, "mainClass":"loader.Main", "libraries":[{"name":"x:loader:1"}]},
                    {"id":null, "priority":30, "mainClass":"ignored.Main", "libraries":[{"name":"x:ignored:1"}]},
                    {"id":"game", "priority":0, "mainClass":"game.Main", "libraries":[{"name":"x:game:1"}]}
                ]
            }),
        );

        let merged = load_version_for_dialect(
            dir.path(),
            "pcl",
            None,
            LinkedLauncherDialect::Pcl,
        )
        .unwrap();
        assert_eq!(merged.id, "pcl");
        assert_eq!(merged.main_class.as_deref(), Some("loader.Main"));
        assert_eq!(
            merged
                .libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:game:1", "x:loader:1"]
        );
    }

    #[test]
    fn explicit_json_path_sets_requested_identity_and_keeps_parent_discovery() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({"id":"base", "mainClass":"base.Main"}),
        );
        write_json(
            dir.path(),
            "folder",
            "actual.json",
            json!({"id":"internal", "inheritsFrom":"base", "mainClass":"child.Main"}),
        );
        write_json(
            dir.path(),
            "folder",
            "unrelated.json",
            json!({"id":"unrelated"}),
        );
        let explicit = dir.path().join("versions/folder/actual.json");

        let merged = load_version_for_dialect(
            dir.path(),
            "logical-id",
            Some(&explicit),
            LinkedLauncherDialect::Hmcl,
        )
        .unwrap();
        assert_eq!(merged.id, "logical-id");
        assert_eq!(merged.main_class.as_deref(), Some("child.Main"));
    }

    #[test]
    fn pcl_recursively_merges_objects_and_appends_arrays() {
        let dir = tempdir().unwrap();
        write_json(
            dir.path(),
            "base",
            "base.json",
            json!({
                "id":"base", "mainClass":"base.Main",
                "arguments":{"game":["--base"], "jvm":["-Dbase=1"]},
                "libraries":[{"name":"x:base:1"}],
                "assetIndex":{"id":"base", "url":"base"}
            }),
        );
        write_json(
            dir.path(),
            "child",
            "child.json",
            json!({
                "id":"child", "inheritsFrom":"base", "mainClass":"child.Main",
                "arguments":{"game":["--child"]},
                "libraries":[{"name":"x:child:1"}],
                "assetIndex":{"id":"child"}
            }),
        );
        let pcl = load_version_for_dialect(
            dir.path(),
            "child",
            None,
            LinkedLauncherDialect::Pcl,
        )
        .unwrap();
        assert_eq!(pcl.main_class.as_deref(), Some("child.Main"));
        assert_eq!(pcl.jar.as_deref(), Some("base"));
        assert_eq!(
            pcl.asset_index.as_ref().map(|index| index.id.as_str()),
            Some("child")
        );
        assert_eq!(
            pcl.asset_index.as_ref().map(|index| index.url.as_str()),
            Some("base")
        );
        assert_eq!(
            pcl.libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:child:1", "x:base:1"]
        );
        let game = &pcl.arguments.unwrap()[&ArgumentType::Game];
        assert_eq!(game.len(), 2);

        let pcl_ce = load_version_for_dialect(
            dir.path(),
            "child",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        assert_eq!(pcl_ce.jar.as_deref(), Some("base"));
        assert_eq!(
            pcl_ce
                .libraries
                .iter()
                .map(|library| library.library.name.as_str())
                .collect::<Vec<_>>(),
            ["x:base:1", "x:child:1"]
        );
    }

    #[test]
    fn artifact_path_and_local_hint_survive_parsing() {
        let artifact: LinkedLibrary = serde_json::from_value(json!({
            "name":"x:y:1", "downloads":{"artifact":{"path":"custom/y.jar", "sha1":"", "size":0, "url":""}}
        })).unwrap();
        assert_eq!(
            artifact.classpath_relative_path().unwrap(),
            PathBuf::from("custom/y.jar")
        );

        let local: LinkedLibrary = serde_json::from_value(json!({
            "name":"x:local:1", "MMC-hint":"local", "MMC-filename":"local/custom.jar"
        })).unwrap();
        assert_eq!(local.hint.as_deref(), Some("local"));
        assert_eq!(
            local.classpath_relative_path().unwrap(),
            PathBuf::from("local/custom.jar")
        );
    }
}
