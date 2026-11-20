use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{ImportLauncherType, generic, hmcl, instance_json, pcl};
use crate::launcher::pcl_ce_json;
use crate::state::ModLoader;

const TEMP_IMPORT_DIR: &str = "axolotl-launcher-import";
const PCL_PRODUCT: &str = "Plain Craft Launcher";
const PCL_CE_PRODUCT: &str = "Plain Craft Launcher Community Edition";

#[derive(Clone, Debug)]
pub(crate) struct ResolvedDirectLink {
    pub launcher: ImportLauncherType,
    pub launcher_root: PathBuf,
    pub dot_minecraft: PathBuf,
    /// Resolved `versions/<id>` directory of the linked installation; carried
    /// as part of the resolution contract even though current consumers
    /// derive their paths from `dot_minecraft`/`version_json` directly.
    #[allow(dead_code)]
    pub version_dir: PathBuf,
    pub version_json: PathBuf,
    pub version_id: String,
    pub game_version: String,
    pub loader: ModLoader,
    /// Detected loader version of the linked document. Directly associated
    /// instances display no loader version (the loader is managed by the
    /// external launcher), so this is not persisted anywhere yet.
    #[allow(dead_code)]
    pub loader_version: Option<String>,
}

/// The launcher's dialect and root used to resolve a traditional
/// `.minecraft/versions/<id>` directory. The root is intentionally separate
/// from the game directory: PCL stores its settings beside the executable,
/// while its `.minecraft` may be a sibling directory.
#[derive(Clone, Debug)]
pub(crate) struct DirectLinkSource {
    pub launcher: ImportLauncherType,
    pub launcher_root: PathBuf,
}

/// Identifies the launcher that owns an externally selected version folder.
///
/// Direct-link Settings receives `.minecraft` roots rather than launcher
/// executables, so the normal import scanner cannot infer PCL from its
/// executable alone. Probe the selected root and its parent, where portable
/// PCL/PCL-CE installations keep their executable and `PCL` configuration.
/// HMCL is only selected when its configuration actually references the
/// supplied game directory; an unrelated `.hmcl` folder must not relabel a
/// generic Minecraft installation.
pub(crate) fn detect_direct_link_source(
    dot_minecraft: &Path,
    version_dir: &Path,
) -> DirectLinkSource {
    if let Some(launcher_root) = detect_pcl_ce_root(dot_minecraft, version_dir)
    {
        return DirectLinkSource {
            launcher: ImportLauncherType::PCL2CE,
            launcher_root,
        };
    }
    for launcher_root in [
        dot_minecraft.parent().map(Path::to_path_buf),
        Some(dot_minecraft.to_path_buf()),
    ]
    .into_iter()
    .flatten()
    {
        if super::pe_info::folder_has_product(&launcher_root, PCL_PRODUCT) {
            return DirectLinkSource {
                launcher: ImportLauncherType::PCL2,
                launcher_root,
            };
        }

        if super::hmcl::config_exists(&launcher_root)
            && super::hmcl::configured_game_dir(
                &launcher_root,
                dot_minecraft,
                version_dir,
            )
            .is_some()
        {
            return DirectLinkSource {
                launcher: ImportLauncherType::HMCL,
                launcher_root,
            };
        }
    }

    DirectLinkSource {
        launcher: ImportLauncherType::Generic,
        launcher_root: dot_minecraft.to_path_buf(),
    }
}

pub(crate) fn detect_pcl_ce_root(
    dot_minecraft: &Path,
    version_dir: &Path,
) -> Option<PathBuf> {
    detect_pcl_ce_root_with(
        dot_minecraft,
        version_dir,
        super::pe_info::folder_has_product,
        || {
            let matches_root = |sources: Vec<(String, String)>| {
                sources.iter().any(|(_, path)| {
                    paths_match(Path::new(path), dot_minecraft)
                })
            };
            matches_root(pcl::get_pclce_instances())
                && !matches_root(pcl::get_pcl_instances())
        },
    )
}

fn detect_pcl_ce_root_with(
    dot_minecraft: &Path,
    version_dir: &Path,
    has_product: impl Fn(&Path, &str) -> bool,
    exclusively_registered: impl FnOnce() -> bool,
) -> Option<PathBuf> {
    let parent_root = dot_minecraft.parent();
    let candidates = [parent_root, Some(dot_minecraft)];
    if let Some(root) = candidates.into_iter().flatten().find(|root| {
        has_product(root, PCL_CE_PRODUCT)
            && (root == &dot_minecraft
                || dot_minecraft.file_name().is_some_and(|name| {
                    name.eq_ignore_ascii_case(".minecraft")
                })
                || has_pcl_ce_local_config(root))
    }) {
        return Some(root.to_path_buf());
    }
    let classic_instance = version_dir.join("PCL").join("Setup.ini").is_file();
    let nearby_classic = candidates.into_iter().flatten().any(|root| {
        has_product(root, PCL_PRODUCT) && !has_product(root, PCL_CE_PRODUCT)
    });
    if classic_instance || nearby_classic {
        return None;
    }
    if has_pcl_ce_local_config(dot_minecraft) {
        return Some(dot_minecraft.to_path_buf());
    }
    let adjacent_ce = parent_root.filter(|root| {
        has_pcl_ce_local_config(root)
            && dot_minecraft
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(".minecraft"))
    });
    if let Some(root) = adjacent_ce {
        return Some(root.to_path_buf());
    }
    if has_pcl_ce_local_config(version_dir) || exclusively_registered() {
        return Some(dot_minecraft.to_path_buf());
    }
    None
}

pub(super) fn has_pcl_ce_local_config(launcher_root: &Path) -> bool {
    let config = launcher_root.join("PCL");
    config.join("config.v1.yml").is_file()
        || config.join("config.v1.json").is_file()
}

impl ResolvedDirectLink {
    pub(crate) fn launcher_key(&self) -> &'static str {
        launcher_key(self.launcher)
            .expect("resolved direct links always use a supported launcher")
    }
}

/// Resolves the same launcher identity used by the existing import flow into
/// persistent paths for a read-only direct association.
///
/// The conventional repository layout follows HMCL
/// `DefaultGameRepositoryLayout.getInstanceRoot/getInstanceJson` at commit
/// `083dbb18ade1c935e2e56d0bdefcd718be1e2ed6`: shared root plus
/// `versions/<id>/<id>.json`. PCL's fallback follows
/// `ModMinecraft.McInstance.GetJsonPath` at commit
/// `639de1b48a44326cbd5465579295cecf23d9056a`: prefer the same-name JSON,
/// otherwise inspect JSON files in the version directory. PCL-CE keeps the
/// same version-folder model in `Modules/Minecraft/McInstance.cs` at commit
/// `aa3b81c6afb3cd1896dda271578b002066512177`.
pub(crate) async fn resolve_direct_link(
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
    instance_path: Option<String>,
) -> crate::Result<ResolvedDirectLink> {
    if launcher_key(launcher_type).is_none()
        && launcher_type != ImportLauncherType::Unknown
    {
        return Err(unsupported_launcher(launcher_type));
    }

    reject_temporary_import_path(&base_path)?;
    if let Some(instance_path) = instance_path.as_deref() {
        reject_temporary_import_path(Path::new(instance_path))?;
    }

    if launcher_type == ImportLauncherType::Unknown {
        return resolve_unknown(
            base_path,
            instance_folder,
            instance_path.as_deref(),
        )
        .await;
    }

    resolve_known(
        launcher_type,
        base_path,
        &instance_folder,
        instance_path.as_deref(),
    )
}

async fn resolve_unknown(
    base_path: PathBuf,
    instance_folder: String,
    instance_path: Option<&str>,
) -> crate::Result<ResolvedDirectLink> {
    let selected = instance_path.map(Path::new).unwrap_or(&base_path);
    if let Ok((dot_minecraft, version_dir)) =
        resolve_repository_paths(selected, &instance_folder)
        && let Some(launcher_root) =
            detect_pcl_ce_root(&dot_minecraft, &version_dir)
    {
        return resolve_known(
            ImportLauncherType::PCL2CE,
            launcher_root,
            &instance_folder,
            version_dir.to_str(),
        );
    }

    // Match the selected scan result before assigning a dialect. Merely having
    // HMCL/PCL configuration beside a Generic version must not relabel that
    // version and route it through the wrong launch merger.
    if let Ok(instances) = Box::pin(super::get_importable_instances(
        ImportLauncherType::HMCL,
        base_path.clone(),
    ))
    .await
        && selection_matches(&instances, &instance_folder, instance_path)
        && let Ok(resolved) = resolve_known(
            ImportLauncherType::HMCL,
            base_path.clone(),
            &instance_folder,
            instance_path,
        )
    {
        return Ok(resolved);
    }

    // The existing PCL scanner intentionally merges legacy PCL and PCL-CE
    // config sources. Recover the source dialect after matching the selected
    // scan item so PCL-CE is not always mislabeled as the first PCL variant.
    if let Ok(instances) = Box::pin(super::get_importable_instances(
        ImportLauncherType::PCL2,
        base_path.clone(),
    ))
    .await
        && selection_matches(&instances, &instance_folder, instance_path)
    {
        let launcher_type = if (has_pcl_ce_local_config(&base_path)
            || super::pe_info::folder_has_product(&base_path, PCL_CE_PRODUCT))
            && (split_config_name(&instance_folder).0 == ".minecraft"
                || instance_path.is_some_and(|path| {
                    path_contains(
                        &base_path.join(".minecraft"),
                        Path::new(path),
                    )
                })) {
            ImportLauncherType::PCL2CE
        } else {
            pcl_dialect(&instance_folder, instance_path)
        };
        if let Ok(resolved) = resolve_known(
            launcher_type,
            base_path.clone(),
            &instance_folder,
            instance_path,
        ) {
            return Ok(resolved);
        }
    }

    resolve_known(
        ImportLauncherType::Generic,
        base_path,
        &instance_folder,
        instance_path,
    )
    .map_err(|_| {
        crate::ErrorKind::InputError(
            "Could not resolve a traditional .minecraft instance as HMCL, PCL2, PCL2CE, or Generic"
                .to_string(),
        )
        .into()
    })
}

fn selection_matches(
    instances: &[super::ImportableInstance],
    instance_folder: &str,
    instance_path: Option<&str>,
) -> bool {
    instances.iter().any(|candidate| {
        if let Some(selected) = instance_path {
            paths_match(Path::new(selected), Path::new(&candidate.path))
                || candidate.version_path.as_deref().is_some_and(
                    |version_path| {
                        paths_match(
                            Path::new(selected),
                            Path::new(version_path),
                        )
                    },
                )
        } else {
            candidate.name == instance_folder
        }
    })
}

fn pcl_dialect(
    instance_folder: &str,
    instance_path: Option<&str>,
) -> ImportLauncherType {
    let pcl_sources = pcl::get_pcl_instances();
    let pcl_ce_sources = pcl::get_pclce_instances();
    pcl_dialect_from_sources(
        instance_folder,
        instance_path,
        &pcl_sources,
        &pcl_ce_sources,
    )
}

fn pcl_dialect_from_sources(
    instance_folder: &str,
    instance_path: Option<&str>,
    pcl_sources: &[(String, String)],
    pcl_ce_sources: &[(String, String)],
) -> ImportLauncherType {
    let config_name = split_config_name(instance_folder).0;
    let source_matches = |sources: &[(String, String)]| {
        sources.iter().any(|(name, path)| {
            if let Some(selected) = instance_path {
                path_contains(Path::new(path), Path::new(selected))
            } else {
                name == config_name
            }
        })
    };
    let pcl_matches = source_matches(pcl_sources);
    let pcl_ce_matches = source_matches(pcl_ce_sources);

    if pcl_ce_matches && !pcl_matches {
        ImportLauncherType::PCL2CE
    } else {
        // Preserve the existing importer's legacy-PCL-first precedence for
        // duplicate config names and the launcher's local `.minecraft` entry.
        ImportLauncherType::PCL2
    }
}

fn resolve_known(
    launcher_type: ImportLauncherType,
    base_path: PathBuf,
    instance_folder: &str,
    instance_path: Option<&str>,
) -> crate::Result<ResolvedDirectLink> {
    let launcher_root = canonicalize_checked(&base_path)?;
    let (dot_minecraft, version_dir) = if let Some(instance_path) =
        instance_path
    {
        let version_dir = canonicalize_checked(Path::new(instance_path))?;
        let dot_minecraft = compatible_game_dir(&base_path, &version_dir)?;
        (canonicalize_checked(&dot_minecraft)?, version_dir)
    } else {
        let source =
            resolve_source_path(launcher_type, &base_path, instance_folder)?;
        resolve_repository_paths(&source, instance_folder)?
    };

    reject_temporary_import_path(&launcher_root)?;
    reject_temporary_import_path(&dot_minecraft)?;
    reject_temporary_import_path(&version_dir)?;

    let is_pcl_ce = launcher_type == ImportLauncherType::PCL2CE;
    let version_json = if is_pcl_ce {
        pcl_ce_json::discover(&version_dir)?
    } else {
        discover_version_json(&version_dir)?
    };
    let version_json = canonicalize_checked(&version_json)?;
    let version_id = version_json
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Version JSON has no usable file stem: {}",
                version_json.display()
            ))
            .as_error()
        })?
        .to_string();
    let info = if is_pcl_ce {
        instance_json::detect_pcl_ce_manifest(&version_json)?
    } else {
        instance_json::detect(&version_dir)
    };
    let info = info.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Could not detect Minecraft version from {}",
            version_json.display()
        ))
        .as_error()
    })?;
    let loader = info
        .loader
        .as_deref()
        .map(ModLoader::try_from_string)
        .transpose()?
        .unwrap_or(ModLoader::Vanilla);

    Ok(ResolvedDirectLink {
        launcher: launcher_type,
        launcher_root,
        dot_minecraft,
        version_dir,
        version_json,
        version_id,
        game_version: info.vanilla_name,
        loader,
        loader_version: info.loader_version,
    })
}

fn resolve_source_path(
    launcher_type: ImportLauncherType,
    base_path: &Path,
    instance_folder: &str,
) -> crate::Result<PathBuf> {
    let (config_name, rest) = split_config_name(instance_folder);
    let target = if rest.is_empty() { config_name } else { rest };

    let game_dir = match launcher_type {
        ImportLauncherType::HMCL => {
            hmcl::get_instance_path(base_path, config_name)
                .map(PathBuf::from)
                .map(|path| {
                    if path.is_absolute() {
                        path
                    } else {
                        base_path.join(path)
                    }
                })
                .unwrap_or_else(|| base_path.to_path_buf())
        }
        ImportLauncherType::PCL2 => {
            find_pcl_source(config_name, &pcl::get_pcl_instances())
                .or_else(|| {
                    find_pcl_source(config_name, &pcl::get_pclce_instances())
                })
                .or_else(|| {
                    (config_name == ".minecraft")
                        .then(|| base_path.join(".minecraft"))
                        .filter(|path| path.is_dir())
                })
                .unwrap_or_else(|| base_path.to_path_buf())
        }
        ImportLauncherType::PCL2CE => {
            find_pcl_source(config_name, &pcl::get_pclce_instances())
                .or_else(|| {
                    (config_name == ".minecraft")
                        .then(|| base_path.join(".minecraft"))
                        .filter(|path| path.is_dir())
                })
                .or_else(|| {
                    find_pcl_source(config_name, &pcl::get_pcl_instances())
                })
                .unwrap_or_else(|| base_path.to_path_buf())
        }
        ImportLauncherType::Generic => base_path.to_path_buf(),
        _ => return Err(unsupported_launcher(launcher_type)),
    };

    Ok(resolve_instance_path(&game_dir, target))
}

fn resolve_repository_paths(
    source: &Path,
    instance_folder: &str,
) -> crate::Result<(PathBuf, PathBuf)> {
    let source = canonicalize_checked(source)?;
    if source
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        let dot_minecraft = source
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| invalid_version_directory(&source))?;
        return Ok((canonicalize_checked(dot_minecraft)?, source));
    }

    let (_, dot_minecraft) = generic::resolve_dotminecraft(&source);
    let dot_minecraft = canonicalize_checked(&dot_minecraft)?;
    let target = split_config_name(instance_folder).1;
    let target = if target.is_empty() {
        Path::new(instance_folder)
            .strip_prefix("versions")
            .unwrap_or_else(|_| Path::new(instance_folder))
    } else {
        Path::new(target)
            .strip_prefix("versions")
            .unwrap_or_else(|_| Path::new(target))
    };
    let version_dir = dot_minecraft.join("versions").join(target);
    if !version_dir.is_dir() {
        return Err(invalid_version_directory(&version_dir));
    }

    Ok((dot_minecraft, canonicalize_checked(&version_dir)?))
}

fn compatible_game_dir(
    base_path: &Path,
    version_dir: &Path,
) -> crate::Result<PathBuf> {
    if version_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("versions"))
    {
        return version_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| invalid_version_directory(version_dir));
    }

    let (_, dot_minecraft) = generic::resolve_dotminecraft(base_path);
    Ok(dot_minecraft)
}

/// Returns whether a file is a Minecraft version manifest rather than a JSON
/// sidecar produced by the game or a launcher. Version folders often contain
/// files such as `usercache.json`; those must not make a copied PCL instance
/// appear ambiguous.
fn is_minecraft_version_manifest(path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&contents) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };

    object
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.trim().is_empty())
        && [
            "arguments",
            "assetIndex",
            "assets",
            "clientVersion",
            "downloads",
            "inheritsFrom",
            "jar",
            "libraries",
            "mainClass",
            "minecraftArguments",
        ]
        .iter()
        .any(|field| object.contains_key(*field))
}

/// Returns whether a version directory contains at least one usable Minecraft
/// version manifest. This lets a root scan ignore launcher bookkeeping folders
/// that are not launchable instances.
pub(crate) fn has_minecraft_version_manifest(version_dir: &Path) -> bool {
    std::fs::read_dir(version_dir).is_ok_and(|entries| {
        entries.flatten().map(|entry| entry.path()).any(|path| {
            path.is_file()
                && path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("json")
                })
                && is_minecraft_version_manifest(&path)
        })
    })
}

pub(crate) fn has_linked_version_manifest(
    version_dir: &Path,
    launcher_type: ImportLauncherType,
) -> bool {
    if launcher_type == ImportLauncherType::PCL2CE {
        instance_json::detect_pcl_ce(version_dir).is_some()
    } else {
        has_minecraft_version_manifest(version_dir)
    }
}

/// Finds the actual manifest selected by upstream launchers: a valid
/// same-name manifest first, then the sole valid manifest in the version
/// directory. Ambiguous folders are rejected instead of guessing which
/// manifest the UI intended.
pub(crate) fn discover_version_json(
    version_dir: &Path,
) -> crate::Result<PathBuf> {
    let folder_name = version_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_version_directory(version_dir))?;
    let same_name = version_dir.join(format!("{folder_name}.json"));
    if same_name.is_file() && is_minecraft_version_manifest(&same_name) {
        return Ok(same_name);
    }

    let mut json_files = std::fs::read_dir(version_dir)
        .map_err(|error| {
            crate::ErrorKind::FSError(format!(
                "Failed to inspect version directory {}: {error}",
                version_dir.display()
            ))
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("json")
                })
                && is_minecraft_version_manifest(path)
        })
        .collect::<Vec<_>>();
    json_files.sort();

    match json_files.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(crate::ErrorKind::InputError(format!(
            "No Minecraft version JSON found in {}",
            version_dir.display()
        ))
        .into()),
        _ => Err(crate::ErrorKind::InputError(format!(
            "Multiple Minecraft version JSON files found in {}; expected a same-name JSON or one unique fallback",
            version_dir.display()
        ))
        .into()),
    }
}

fn launcher_key(launcher_type: ImportLauncherType) -> Option<&'static str> {
    match launcher_type {
        ImportLauncherType::HMCL => Some("hmcl"),
        ImportLauncherType::PCL2 => Some("pcl2"),
        ImportLauncherType::PCL2CE => Some("pcl2_ce"),
        ImportLauncherType::Generic => Some("generic"),
        _ => None,
    }
}

fn reject_temporary_import_path(path: &Path) -> crate::Result<()> {
    let temporary_root = std::env::temp_dir().join(TEMP_IMPORT_DIR);
    if path.starts_with(&temporary_root) {
        return Err(crate::ErrorKind::InputError(
            "Direct association is unavailable for extracted launcher archives because the temporary folder is deleted after import"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

fn canonicalize_checked(path: &Path) -> crate::Result<PathBuf> {
    let canonical = crate::util::io::canonicalize(path)?;
    reject_temporary_import_path(&canonical)?;
    Ok(canonical)
}

fn paths_match(left: &Path, right: &Path) -> bool {
    match (
        crate::util::io::canonicalize(left),
        crate::util::io::canonicalize(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn path_contains(root: &Path, selected: &Path) -> bool {
    match (
        crate::util::io::canonicalize(root),
        crate::util::io::canonicalize(selected),
    ) {
        (Ok(root), Ok(selected)) => selected.starts_with(root),
        _ => selected.starts_with(root),
    }
}

fn unsupported_launcher(launcher_type: ImportLauncherType) -> crate::Error {
    crate::ErrorKind::InputError(format!(
        "Direct association does not support launcher {launcher_type}; expected HMCL, PCL2, PCL2CE, Generic, or Unknown"
    ))
    .into()
}

fn invalid_version_directory(path: &Path) -> crate::Error {
    crate::ErrorKind::InputError(format!(
        "Expected a traditional .minecraft version directory at {}",
        path.display()
    ))
    .into()
}

/// Splits an instance folder identity like `name` or `Version:1.12.2` into
/// (config name, version part); PCL and HMCL instance identities use this
/// shape.
fn split_config_name(name: &str) -> (&str, &str) {
    name.split_once(':').unwrap_or((name, ""))
}

/// Resolves the folder of an instance from a base path and its scan identity.
fn resolve_instance_path(base_path: &Path, instance_folder: &str) -> PathBuf {
    if let Ok(rest) = Path::new(instance_folder).strip_prefix("versions") {
        return base_path.join("versions").join(rest);
    }
    if base_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .as_deref()
        == Some(instance_folder)
    {
        base_path.to_path_buf()
    } else {
        base_path.join(instance_folder)
    }
}

/// Finds the game directory of a PCL/PCL-CE instance from its scan sources
/// (registry entries or CE config).
fn find_pcl_source(
    instance_name: &str,
    sources: &[(String, String)],
) -> Option<PathBuf> {
    sources
        .iter()
        .find(|(name, _)| name == instance_name)
        .map(|(_, path)| PathBuf::from(path))
        .filter(|path| path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_json(version_dir: &Path, file_stem: &str) {
        std::fs::create_dir_all(version_dir).unwrap();
        std::fs::write(
            version_dir.join(format!("{file_stem}.json")),
            serde_json::to_vec_pretty(&json!({
                "id": file_stem,
                "mainClass": "net.minecraft.client.main.Main",
                "type": "release"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn resolves_normal_versions_layout() {
        let root = TempDir::new().unwrap();
        let version_dir = root.path().join("versions/1.20.1");
        write_json(&version_dir, "1.20.1");

        let resolved = resolve_direct_link(
            ImportLauncherType::Generic,
            root.path().to_path_buf(),
            "versions/1.20.1".to_string(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(resolved.launcher, ImportLauncherType::Generic);
        assert_eq!(
            resolved.dot_minecraft,
            crate::util::io::canonicalize(root.path()).unwrap()
        );
        assert_eq!(resolved.version_id, "1.20.1");
        assert_eq!(
            resolved.version_dir,
            crate::util::io::canonicalize(&version_dir).unwrap()
        );
    }

    #[test]
    fn identifies_hmcl_only_when_it_owns_the_game_directory() {
        let root = TempDir::new().unwrap();
        let game_dir = root.path().join("game");
        std::fs::create_dir_all(game_dir.join("versions")).unwrap();
        let hmcl_dir = root.path().join(".hmcl");
        std::fs::create_dir_all(&hmcl_dir).unwrap();
        std::fs::write(
            hmcl_dir.join("hmcl.json"),
            serde_json::to_vec(&json!({
                "configurations": {
                    "default": { "gameDir": game_dir.to_string_lossy() }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let source = detect_direct_link_source(
            &game_dir,
            &game_dir.join("versions").join("demo"),
        );
        assert_eq!(source.launcher, ImportLauncherType::HMCL);
        assert_eq!(source.launcher_root, root.path());
    }

    #[test]
    fn recognizes_pcl_ce_by_owned_config_games_or_specific_product() {
        let root = TempDir::new().unwrap();
        let game_dir = root.path().join(".minecraft");
        let version_dir = game_dir.join("versions/demo");
        std::fs::create_dir_all(&version_dir).unwrap();
        let no_products = |_: &Path, _: &str| false;
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                no_products,
                || false
            ),
            None
        );
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                no_products,
                || true
            ),
            Some(game_dir.clone())
        );
        let ce_product = |path: &Path, product: &str| {
            path == root.path() && product == PCL_CE_PRODUCT
        };
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                ce_product,
                || false
            ),
            Some(root.path().to_path_buf())
        );

        std::fs::create_dir_all(root.path().join("PCL")).unwrap();
        std::fs::write(root.path().join("PCL/config.v1.json"), "{}").unwrap();
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                no_products,
                || false
            ),
            Some(root.path().to_path_buf())
        );
        let unrelated = root.path().join("unrelated");
        assert_eq!(
            detect_pcl_ce_root_with(
                &unrelated,
                &unrelated.join("versions/demo"),
                no_products,
                || false
            ),
            None
        );
        let classic_product = |path: &Path, product: &str| {
            path == root.path() && product == PCL_PRODUCT
        };
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                classic_product,
                || true
            ),
            None
        );
        std::fs::create_dir_all(version_dir.join("PCL")).unwrap();
        std::fs::write(version_dir.join("PCL/Setup.ini"), "").unwrap();
        assert_eq!(
            detect_pcl_ce_root_with(
                &game_dir,
                &version_dir,
                no_products,
                || true
            ),
            None
        );
    }

    #[tokio::test]
    async fn unknown_direct_link_uses_pcl_ce_reader_without_executable() {
        let root = TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("PCL")).unwrap();
        std::fs::write(
            root.path().join("PCL/config.v1.yml"),
            "LaunchArgumentInfo: CE\n",
        )
        .unwrap();
        let game_dir = root.path().join(".minecraft");
        let version_dir = game_dir.join("versions/1.21.1");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("1.21.1.json"),
            r#"{/* CE */"id":"1.21.1","mainClass":"game.Main",}"#,
        )
        .unwrap();

        let resolved = resolve_direct_link(
            ImportLauncherType::Unknown,
            game_dir,
            "versions/1.21.1".into(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(resolved.launcher, ImportLauncherType::PCL2CE);
        assert_eq!(
            resolved.launcher_root,
            crate::util::io::canonicalize(root.path()).unwrap()
        );
    }

    #[tokio::test]
    async fn resolves_compatible_mode_from_game_dir_and_version_path() {
        let root = TempDir::new().unwrap();
        let version_dir = root.path().join("versions/1.12.2-forge");
        write_json(&version_dir, "1.12.2-forge");

        let resolved = resolve_direct_link(
            ImportLauncherType::PCL2,
            root.path().to_path_buf(),
            "Friendly PCL Name".to_string(),
            Some(version_dir.to_string_lossy().to_string()),
        )
        .await
        .unwrap();

        assert_eq!(resolved.launcher_key(), "pcl2");
        assert_eq!(
            resolved.dot_minecraft,
            crate::util::io::canonicalize(root.path()).unwrap()
        );
        assert_eq!(resolved.version_id, "1.12.2-forge");
    }

    #[tokio::test]
    async fn unique_json_fallback_uses_actual_stem_not_display_name() {
        let root = TempDir::new().unwrap();
        let version_dir = root.path().join("versions/ui-folder");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("actual-version-id.json"),
            serde_json::to_vec_pretty(&json!({
                "id": "actual-version-id",
                "clientVersion": "1.20.1",
                "mainClass": "net.minecraft.client.main.Main",
                "type": "release"
            }))
            .unwrap(),
        )
        .unwrap();

        let resolved = resolve_direct_link(
            ImportLauncherType::Generic,
            root.path().to_path_buf(),
            "versions/ui-folder".to_string(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(resolved.version_id, "actual-version-id");
        assert!(resolved.version_json.ends_with("actual-version-id.json"));
    }

    #[test]
    fn ignores_runtime_json_when_finding_a_copied_pcl_instance() {
        let root = TempDir::new().unwrap();
        let version_dir =
            root.path().join("versions").join("1.19.2 - 64bit - copy");
        write_json(&version_dir, "1.19.2 - 64bit");
        std::fs::write(
            version_dir.join("usercache.json"),
            r#"[{"name":"player","uuid":"example"}]"#,
        )
        .unwrap();

        assert!(has_minecraft_version_manifest(&version_dir));
        assert_eq!(
            discover_version_json(&version_dir).unwrap(),
            version_dir.join("1.19.2 - 64bit.json")
        );
    }

    #[tokio::test]
    async fn pcl_ce_direct_link_reads_the_selected_lenient_manifest() {
        let root = TempDir::new().unwrap();
        let version_dir = root.path().join("versions/custom-profile");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(
            version_dir.join("custom-profile.json"),
            r#"{
                // PCL-CE accepts mixed field casing and comments.
                "ClientVersion":"1.21.1", "MAINCLASS":"game.Main",
                "Arguments":{"Game":[{"Rules":null,"Value":"--demo"}],},
            }"#,
        )
        .unwrap();
        std::fs::write(version_dir.join("usercache.json"), "[]").unwrap();
        let resolved = resolve_direct_link(
            ImportLauncherType::PCL2CE,
            root.path().to_path_buf(),
            "versions/custom-profile".to_string(),
            Some(version_dir.to_string_lossy().to_string()),
        )
        .await
        .unwrap();
        assert_eq!(resolved.launcher, ImportLauncherType::PCL2CE);
        assert!(resolved.version_json.ends_with("custom-profile.json"));
        assert_eq!(resolved.game_version, "1.21.1");
        assert!(
            resolve_direct_link(
                ImportLauncherType::Generic,
                root.path().to_path_buf(),
                "versions/custom-profile".to_string(),
                Some(version_dir.to_string_lossy().to_string()),
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn rejects_a_pcl_bookkeeping_directory_without_a_version_manifest() {
        let root = TempDir::new().unwrap();
        let version_dir = root.path().join("versions").join("Sodium Plus");
        std::fs::create_dir_all(version_dir.join("PCL")).unwrap();
        std::fs::write(
            version_dir.join("PCL").join("config.v1.yml"),
            "VersionVanilla: 1.21.1\n",
        )
        .unwrap();

        assert!(!has_minecraft_version_manifest(&version_dir));
    }

    #[tokio::test]
    async fn rejects_temporary_launcher_import_directory() {
        let error = resolve_direct_link(
            ImportLauncherType::Generic,
            std::env::temp_dir().join(TEMP_IMPORT_DIR).join("extracted"),
            "versions/1.20.1".to_string(),
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("temporary"));
    }

    #[tokio::test]
    async fn rejects_unsupported_launcher_before_touching_paths() {
        let error = resolve_direct_link(
            ImportLauncherType::MultiMC,
            PathBuf::from("missing"),
            "instance".to_string(),
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("does not support"));
    }

    #[test]
    fn maps_pcl_and_hmcl_to_distinct_persistent_dialects() {
        assert_eq!(launcher_key(ImportLauncherType::HMCL), Some("hmcl"));
        assert_eq!(launcher_key(ImportLauncherType::PCL2), Some("pcl2"));
        assert_eq!(launcher_key(ImportLauncherType::PCL2CE), Some("pcl2_ce"));
    }

    #[test]
    fn unknown_recovers_pcl_ce_dialect_from_selected_source_path() {
        let root = TempDir::new().unwrap();
        let pcl_root = root.path().join("pcl");
        let pcl_ce_root = root.path().join("pcl-ce");
        let selected = pcl_ce_root.join("versions/1.21.1");
        std::fs::create_dir_all(&pcl_root).unwrap();
        std::fs::create_dir_all(&selected).unwrap();
        let pcl_sources = vec![(
            "Legacy".to_string(),
            pcl_root.to_string_lossy().to_string(),
        )];
        let pcl_ce_sources = vec![(
            "Community".to_string(),
            pcl_ce_root.to_string_lossy().to_string(),
        )];

        assert_eq!(
            pcl_dialect_from_sources(
                "1.21.1",
                Some(selected.to_string_lossy().as_ref()),
                &pcl_sources,
                &pcl_ce_sources,
            ),
            ImportLauncherType::PCL2CE
        );
    }

    #[tokio::test]
    async fn unknown_prefers_detected_hmcl_over_generic() {
        let root = TempDir::new().unwrap();
        let game_dir = root.path().join("game");
        let version_dir = game_dir.join("versions/1.20.4");
        write_json(&version_dir, "1.20.4");
        std::fs::create_dir_all(root.path().join(".hmcl")).unwrap();
        std::fs::write(
            root.path().join(".hmcl/hmcl.json"),
            serde_json::to_vec_pretty(&json!({
                "configurations": {
                    "HMCL Profile": { "gameDir": game_dir }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let resolved = resolve_direct_link(
            ImportLauncherType::Unknown,
            root.path().to_path_buf(),
            "HMCL Profile:versions/1.20.4".to_string(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(resolved.launcher, ImportLauncherType::HMCL);
        assert_eq!(resolved.launcher_key(), "hmcl");
    }

    #[tokio::test]
    async fn unknown_does_not_relabel_unmatched_generic_as_hmcl() {
        let root = TempDir::new().unwrap();
        let hmcl_game = root.path().join("hmcl-game");
        write_json(&hmcl_game.join("versions/hmcl"), "1.20.1");
        std::fs::create_dir_all(root.path().join(".hmcl")).unwrap();
        std::fs::write(
            root.path().join(".hmcl/hmcl.json"),
            serde_json::to_vec_pretty(&json!({
                "configurations": {
                    "HMCL Profile": { "gameDir": hmcl_game }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let generic_version = root.path().join("versions/generic");
        write_json(&generic_version, "1.20.4");
        let resolved = resolve_direct_link(
            ImportLauncherType::Unknown,
            root.path().to_path_buf(),
            // Deliberately collide with the HMCL candidate's display identity;
            // the explicitly selected path must take precedence.
            "HMCL Profile:versions/hmcl".to_string(),
            Some(generic_version.to_string_lossy().to_string()),
        )
        .await
        .unwrap();

        assert_eq!(resolved.launcher, ImportLauncherType::Generic);
        assert_eq!(resolved.launcher_key(), "generic");
    }
}
