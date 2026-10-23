//! Read-only, bounded inspection of external dedicated-server packages.
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use super::source::{
    MAX_ARCHIVE_LAYERS, Source, read_bounded, safe_source_path,
};
use crate::api::drop_classifier::MAX_ZIP_EXTRACTION_ENTRIES as MAX_ENTRIES;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_METADATA: u64 = 2 * 1024 * 1024;
const MAX_JAR: u64 = 128 * 1024 * 1024;
const MAX_ROOT_JARS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerImportPreview {
    pub inner_base: String,
    #[serde(default)]
    pub archives: Vec<String>,
    pub server_type: String,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub jar_name: Option<String>,
    pub needs_install: bool,
    /// Stable reason code; translated by the desktop frontend.
    pub problem: Option<String>,
}

fn at(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.into()
    } else {
        format!("{base}/{name}")
    }
}
/// Reads server JAR metadata without altering the existing mod metadata parser.
pub(super) fn manifest_attribute(content: &str, key: &str) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(previous) = lines.last_mut() {
                previous.push_str(rest);
            }
        } else {
            lines.push(line.to_string());
        }
    }
    lines.iter().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case(key)
            .then(|| value.trim().to_string())
    })
}

fn text<R: Read + Seek>(
    jar: &mut zip::ZipArchive<R>,
    path: &str,
) -> Option<String> {
    String::from_utf8(read_bounded(jar.by_name(path).ok()?, MAX_METADATA).ok()?)
        .ok()
}
fn json<R: Read + Seek>(
    jar: &mut zip::ZipArchive<R>,
    path: &str,
) -> Option<Value> {
    serde_json::from_str(&text(jar, path)?).ok()
}
fn property(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| v.trim().into())
    })
}
pub(super) fn version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')
        })
}
fn candidate(
    kind: &str,
    game: String,
    loader: Option<String>,
    jar: Option<String>,
    install: bool,
) -> ServerImportPreview {
    let problem =
        if !version(&game) || loader.as_deref().is_some_and(|v| !version(v)) {
            Some("missing-version".into())
        } else {
            None
        };
    ServerImportPreview {
        inner_base: String::new(),
        archives: Vec::new(),
        server_type: kind.into(),
        game_version: game,
        loader_version: loader,
        jar_name: jar,
        needs_install: install,
        problem,
    }
}

fn inspect_jar<R: Read + Seek>(
    mut jar: zip::ZipArchive<R>,
    name: &str,
) -> Option<ServerImportPreview> {
    if jar.len() > MAX_ENTRIES {
        return None;
    }
    let manifest = text(&mut jar, "META-INF/MANIFEST.MF")?;
    let main = manifest_attribute(&manifest, "Main-Class")?;
    if main == "net.minecraftforge.installer.SimpleInstaller" {
        let profile =
            json(&mut jar, "install_profile.json").unwrap_or(Value::Null);
        let embedded = json(&mut jar, "version.json").unwrap_or(Value::Null);
        let mut games: Vec<String> = [
            profile["minecraft"].as_str(),
            embedded["inheritsFrom"].as_str(),
        ]
        .into_iter()
        .flatten()
        .map(str::to_string)
        .collect();
        let mut pairs = Vec::new();
        for id in [profile["version"].as_str(), embedded["id"].as_str()]
            .into_iter()
            .flatten()
        {
            if let Some((mc, build)) = id.split_once("-forge-") {
                games.push(mc.into());
                pairs.push((mc.to_string(), build.to_string()));
            }
        }
        let coordinates: Vec<&str> = profile["path"]
            .as_str()
            .into_iter()
            .chain(
                profile["libraries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|library| library["name"].as_str()),
            )
            .collect();
        if coordinates
            .iter()
            .any(|coordinate| coordinate.starts_with("net.neoforged:"))
        {
            let mut result = candidate(
                "neoforge",
                games.first().cloned().unwrap_or_default(),
                None,
                None,
                true,
            );
            result.problem = Some("unsupported-type".into());
            return Some(result);
        }
        games.sort();
        games.dedup();
        let game = games.first().cloned().unwrap_or_default();
        let mut coordinate_conflict = false;
        for coordinate in coordinates {
            if let Some(artifact) =
                coordinate.strip_prefix("net.minecraftforge:forge:")
            {
                let artifact = artifact.split(':').next().unwrap_or_default();
                if let Some(build) = artifact.strip_prefix(&format!("{game}-"))
                {
                    pairs.push((game.clone(), build.to_string()));
                } else {
                    coordinate_conflict = true;
                }
            }
        }
        pairs.sort();
        pairs.dedup();
        let loader = pairs.first().map(|(_, build)| build.clone());
        let mut result = candidate("forge", game, loader, None, true);
        if result.loader_version.is_none() {
            result.problem = Some("missing-version".into());
        }
        if games.len() > 1 || pairs.len() > 1 || coordinate_conflict {
            result.problem = Some("conflicting-versions".into());
        }
        return Some(result);
    }

    if main == "net.minecraftforge.fml.relauncher.ServerLaunchWrapper" {
        let classpath =
            manifest_attribute(&manifest, "Class-Path").unwrap_or_default();
        let mut games: Vec<_> = classpath
            .split_whitespace()
            .filter_map(|path| {
                path.strip_prefix("minecraft_server.")?
                    .strip_suffix(".jar")
                    .map(str::to_string)
            })
            .collect();
        games.sort();
        games.dedup();
        let normalized = manifest.replace("\r\n", "\n");
        let loader = normalized
            .split("\n\n")
            .find(|section| {
                manifest_attribute(section, "Name").as_deref()
                    == Some("net/minecraftforge/common/")
            })
            .and_then(|section| {
                manifest_attribute(section, "Implementation-Version")
            });
        let mut result = candidate(
            "forge",
            games.first().cloned().unwrap_or_default(),
            loader,
            Some(name.into()),
            false,
        );
        if games.len() > 1 {
            result.problem = Some("conflicting-versions".into());
        }
        return Some(result);
    }

    if main.contains("neoforged") || main.contains("quiltmc") {
        let mut result = candidate(
            if main.contains("neoforged") {
                "neoforge"
            } else {
                "quilt"
            },
            String::new(),
            None,
            None,
            false,
        );
        result.problem = Some("unsupported-type".into());
        return Some(result);
    }
    if main.starts_with("net.fabricmc.")
        && (main.ends_with("ServerLauncher")
            || main.ends_with("FabricServerLauncher"))
    {
        let properties = text(&mut jar, "install.properties")
            .or_else(|| text(&mut jar, "fabric-server-launch.properties"))
            .unwrap_or_default();
        let game = property(&properties, "game-version")
            .or_else(|| property(&properties, "gameVersion"))
            .or_else(|| property(&properties, "minecraftVersion"))
            .unwrap_or_default();
        let loader = property(&properties, "fabric-loader-version")
            .or_else(|| property(&properties, "loaderVersion"))
            .or_else(|| {
                let cp = manifest_attribute(&manifest, "Class-Path")?;
                cp.split_whitespace().find_map(|p| {
                    let rest = p.strip_prefix(
                        "libraries/net/fabricmc/fabric-loader/",
                    )?;
                    Some(rest.split('/').next()?.to_string())
                })
            });
        return Some(candidate(
            "fabric",
            game,
            loader,
            Some(name.into()),
            false,
        ));
    }
    if matches!(
        main.as_str(),
        "net.minecraft.bundler.Main"
            | "net.minecraft.server.Main"
            | "net.minecraft.server.MinecraftServer"
            | "io.papermc.paperclip.Main"
            | "io.papermc.paperclip.Paperclip"
            | "com.destroystokyo.paperclip.Paperclip"
    ) {
        let kind = if main.contains("paperclip") {
            "paper"
        } else {
            "vanilla"
        };
        let metadata = json(&mut jar, "version.json").unwrap_or(Value::Null);
        let mut game = metadata["id"].as_str().unwrap_or_default().to_string();
        if game.is_empty()
            && let Some(list) = text(&mut jar, "META-INF/versions.list")
        {
            let rows: Vec<_> = list
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            if rows.len() == 1 {
                game = rows[0]
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .into();
            }
        }
        return Some(candidate(kind, game, None, Some(name.into()), false));
    }
    None
}

/// A declaration is evidence only when it identifies a server-specific format.
/// Ordinary CurseForge client manifests and mods folders are not declarations.
fn server_declaration(
    source: &mut Source,
    base: &str,
    names: &[String],
) -> Result<Option<ServerImportPreview>, String> {
    if !names.iter().any(|name| name == "manifest.json") {
        return Ok(None);
    }
    let bytes = match source.read(&at(base, "manifest.json"), MAX_METADATA) {
        Ok(bytes) => bytes,
        Err(error) if error == "metadata-too-large" => return Ok(None),
        Err(error) => return Err(error),
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    if value.get("serverPackCreatorVersion").is_none() {
        return Ok(None);
    }
    let kind = value["modloader"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let game = value["minecraftVersion"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let loader = value["modloaderVersion"].as_str().map(str::to_string);
    let mut result = candidate(&kind, game, loader, None, true);
    if !matches!(kind.as_str(), "forge" | "fabric") {
        result.problem = Some("unsupported-type".into());
    } else if result.loader_version.is_none() {
        result.problem = Some("missing-version".into());
    }
    // Treat this as data, never source/evaluate it as a shell script. Only the
    // three identity fields corroborate the declaration; hooks/URLs/JVM args do not.
    if names.iter().any(|name| name == "variables.txt") {
        let variables = String::from_utf8(
            source.read(&at(base, "variables.txt"), MAX_METADATA)?,
        )
        .map_err(|e| e.to_string())?;
        for line in variables
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
        {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches(['\'', '"']);
            let expected = match key.trim() {
                "MINECRAFT_VERSION" => Some(result.game_version.as_str()),
                "MODLOADER" => Some(result.server_type.as_str()),
                "MODLOADER_VERSION" => result.loader_version.as_deref(),
                _ => None,
            };
            if expected
                .is_some_and(|expected| !value.eq_ignore_ascii_case(expected))
            {
                result.problem = Some("conflicting-versions".into());
            }
        }
    }
    Ok(Some(result))
}

/// Reconcile independent declarations and installed launch evidence. Missing
/// fields can be filled, but conflicting identities or launchers never win by order.
fn reconcile(mut evidence: Vec<ServerImportPreview>) -> ServerImportPreview {
    let mut result = evidence.remove(0);
    for other in evidence {
        if result.server_type != other.server_type
            || (!result.game_version.is_empty()
                && !other.game_version.is_empty()
                && result.game_version != other.game_version)
            || (result.loader_version.is_some()
                && other.loader_version.is_some()
                && result.loader_version != other.loader_version)
        {
            result.problem = Some("conflicting-versions".into());
            continue;
        }
        if result.jar_name.is_some()
            && other.jar_name.is_some()
            && result.jar_name != other.jar_name
        {
            result.problem = Some("multiple-launchers".into());
        }
        if result.game_version.is_empty() {
            result.game_version = other.game_version;
        }
        if result.loader_version.is_none() {
            result.loader_version = other.loader_version;
        }
        if result.jar_name.is_none() {
            result.jar_name = other.jar_name;
        }
        result.needs_install &= other.needs_install;
        if other
            .problem
            .as_deref()
            .is_some_and(|problem| problem != "missing-version")
        {
            result.problem = other.problem;
        }
    }
    let complete = version(&result.game_version)
        && (!matches!(result.server_type.as_str(), "forge" | "fabric")
            || result.loader_version.as_deref().is_some_and(version));
    if !complete && result.problem.is_none() {
        result.problem = Some("missing-version".into());
    }
    if complete && result.problem.as_deref() == Some("missing-version") {
        result.problem = None;
    }
    result
}

fn inspect_root(
    source: &mut Source,
    base: &str,
    names: &[String],
    allow_configuration: bool,
) -> Result<Option<ServerImportPreview>, String> {
    let mut candidates = Vec::new();
    let jars: Vec<_> = names
        .iter()
        .filter(|name| {
            !name.contains('/') && name.to_ascii_lowercase().ends_with(".jar")
        })
        .collect();
    if jars.len() > MAX_ROOT_JARS {
        return Err("too-many-jars".into());
    }
    for name in jars {
        let relative = at(base, name);
        let found = match source {
            Source::Folder(root) => {
                let file =
                    std::fs::File::open(safe_source_path(root, &relative)?)
                        .map_err(|e| e.to_string())?;
                zip::ZipArchive::new(file)
                    .ok()
                    .and_then(|jar| inspect_jar(jar, name))
            }
            Source::Zip(_, _) => zip::ZipArchive::new(Cursor::new(
                source.read(&relative, MAX_JAR)?,
            ))
            .ok()
            .and_then(|jar| inspect_jar(jar, name)),
        };
        if let Some(found) = found {
            candidates.push(found);
        }
    }
    for (kind, library) in [
        ("forge", "libraries/net/minecraftforge/forge"),
        ("neoforge", "libraries/net/neoforged/neoforge"),
    ] {
        if !names.iter().any(|n| n == "libraries/") {
            continue;
        }
        let library_base = at(base, library);
        let Ok(versions) = source.list(&library_base) else {
            continue;
        };
        for v in versions.iter().filter(|v| v.ends_with('/')) {
            let args_base = at(&library_base, v.trim_end_matches('/'));
            let mut found: Option<ServerImportPreview> = None;
            for args_name in ["unix_args.txt", "win_args.txt"] {
                let Ok(bytes) =
                    source.read(&at(&args_base, args_name), MAX_METADATA)
                else {
                    continue;
                };
                let args =
                    String::from_utf8(bytes).map_err(|e| e.to_string())?;
                let tokens: Vec<_> = args.split_whitespace().collect();
                let get = |key: &str| {
                    tokens
                        .windows(2)
                        .find(|w| w[0] == key)
                        .map(|w| w[1].trim_matches(['\'', '"']).to_string())
                };
                if !get("--launchTarget")
                    .is_some_and(|v| v.to_ascii_lowercase().contains("server"))
                {
                    continue;
                }
                let mut result = candidate(
                    kind,
                    get("--fml.mcVersion").unwrap_or_default(),
                    get("--fml.forgeVersion"),
                    None,
                    false,
                );
                if kind == "neoforge" {
                    result.problem = Some("unsupported-type".into());
                }
                if found.as_ref().is_some_and(|previous| previous != &result) {
                    result.problem = Some("conflicting-versions".into());
                }
                found = Some(result);
            }
            if let Some(result) = found {
                candidates.push(result);
            }
        }
    }
    if let Some(declaration) = server_declaration(source, base, names)? {
        candidates.push(declaration);
    }
    if candidates.is_empty() {
        let properties = if names.iter().any(|name| name == "server.properties")
        {
            String::from_utf8(
                source.read(&at(base, "server.properties"), MAX_METADATA)?,
            )
            .ok()
        } else {
            None
        };
        let server_settings = properties.as_deref().is_some_and(|text| {
            property(text, "server-port")
                .is_some_and(|value| value.parse::<u16>().is_ok())
                && property(text, "level-name").is_some()
        });
        if allow_configuration
            || server_settings
            || names.iter().any(|name| name == "axolotl-server.json")
        {
            let mut result =
                candidate("unknown", String::new(), None, None, true);
            result.problem = Some("configuration-required".into());
            candidates.push(result);
        } else {
            return Ok(None);
        }
    }
    // A vanilla jar beside a loader launcher is a dependency, not a second server.
    if candidates
        .iter()
        .any(|c| matches!(c.server_type.as_str(), "forge" | "fabric"))
    {
        let vanilla: Vec<_> = candidates
            .iter()
            .filter(|c| c.server_type == "vanilla")
            .cloned()
            .collect();
        candidates.retain(|c| c.server_type != "vanilla");
        for c in &mut candidates {
            if c.game_version.is_empty()
                && c.server_type == "fabric"
                && vanilla.len() == 1
            {
                c.game_version = vanilla[0].game_version.clone();
                if version(&c.game_version) {
                    c.problem = None;
                }
            }
            if vanilla.iter().any(|v| {
                !v.game_version.is_empty() && v.game_version != c.game_version
            }) {
                c.problem = Some("conflicting-versions".into());
            }
        }
    }
    let mut result = reconcile(candidates);
    let client_versions = if names.iter().any(|name| name == "versions/") {
        let versions = at(base, "versions");
        source
            .list(&versions)
            .unwrap_or_default()
            .into_iter()
            .filter(|name| name.ends_with('/'))
            .any(|name| {
                let id = name.trim_end_matches('/');
                source.list(&at(&versions, id)).is_ok_and(|files| {
                    files.iter().any(|file| file == &format!("{id}.json"))
                })
            })
    } else {
        false
    };
    if client_versions
        || names.iter().any(|name| {
            matches!(name.as_str(), "instance.cfg" | "axolotl_config.json")
        })
    {
        result.problem = Some("client-server-conflict".into());
    }
    result.inner_base = base.into();
    Ok(Some(result))
}

fn wrapped_archive(names: &[String]) -> Option<&str> {
    let mut archives = names
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".zip"));
    let archive = archives.next()?;
    if archives.next().is_some() {
        return None;
    }
    names
        .iter()
        .all(|name| {
            name == archive
                || matches!(
                    Path::new(name)
                        .extension()
                        .and_then(|v| v.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("txt" | "md")
                )
        })
        .then_some(archive.as_str())
}

/// Descends only single-directory wrappers; never searches worlds/mods recursively.
pub fn inspect_server_import(
    path: &Path,
) -> Result<Option<ServerImportPreview>, String> {
    inspect(path, false, true)
}

/// The explicit Server action may configure a resource-only package. The
/// automatic classifier never treats that user choice as detected identity.
pub fn inspect_configurable_server_import(
    path: &Path,
) -> Result<Option<ServerImportPreview>, String> {
    inspect(path, true, true)
}

/// The global drop classifier must preserve its existing nested-unpack consent.
pub(super) fn inspect_server_drop(
    path: &Path,
    allow_nested_unpack: bool,
) -> Result<Option<ServerImportPreview>, String> {
    inspect(path, false, allow_nested_unpack)
}

fn inspect(
    path: &Path,
    allow_configuration: bool,
    allow_nested_unpack: bool,
) -> Result<Option<ServerImportPreview>, String> {
    let Some(mut source) = Source::open(path)? else {
        return Ok(None);
    };
    let mut archives = Vec::new();
    let mut nested_bytes = 0;
    let mut base = String::new();
    for _ in 0..16 {
        let mut names = source.list(&base)?;
        names.sort();
        if let Some(mut result) =
            inspect_root(&mut source, &base, &names, false)?
        {
            result.archives = archives;
            return Ok(Some(result));
        }
        if names.len() == 1
            && names[0].ends_with('/')
            && !matches!(
                names[0].as_str(),
                "mods/" | "config/" | "libraries/" | "world/" | "versions/"
            )
        {
            base = at(&base, names[0].trim_end_matches('/'));
        } else if let Some(nested) = wrapped_archive(&names) {
            if !allow_nested_unpack || archives.len() >= MAX_ARCHIVE_LAYERS {
                return Ok(None);
            }
            let relative = at(&base, nested);
            let Some(next) = source.nested(&relative, &mut nested_bytes)?
            else {
                return Ok(None);
            };
            source = next;
            archives.push(relative);
            base.clear();
        } else {
            let mut result = if allow_configuration {
                inspect_root(&mut source, &base, &names, true)?
            } else {
                None
            };
            if let Some(preview) = result.as_mut() {
                preview.archives = archives;
            }
            return Ok(result);
        }
    }
    Err("wrapping-too-deep".into())
}

#[cfg(test)]
pub(super) mod tests;
