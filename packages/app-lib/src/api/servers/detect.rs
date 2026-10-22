//! Read-only, bounded inspection of external dedicated-server packages.
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use crate::api::drop_classifier::{
    MAX_ZIP_EXTRACTION_ENTRIES as MAX_ENTRIES, ZipEntrySet,
    is_noise_entry as noise,
};
use crate::mod_metadata::manifest::manifest_attribute as attribute;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_METADATA: u64 = 2 * 1024 * 1024;
const MAX_JAR: u64 = 128 * 1024 * 1024;
const MAX_ROOT_JARS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerImportPreview {
    pub inner_base: String,
    pub server_type: String,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub jar_name: Option<String>,
    pub needs_install: bool,
    /// Stable reason code; translated by the desktop frontend.
    pub problem: Option<String>,
}

pub(crate) fn read_bounded(
    mut reader: impl Read,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("metadata-too-large".into());
    }
    Ok(bytes)
}

pub(crate) fn safe_source_path(
    root: &Path,
    relative: &str,
) -> Result<PathBuf, String> {
    let path = super::files::safe_relative_join(root, relative)
        .map_err(|e| e.to_string())?;
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        let meta =
            std::fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("symbolic-links".into());
        }
    }
    Ok(path)
}

enum Source {
    Folder(PathBuf),
    Zip(zip::ZipArchive<std::fs::File>, ZipEntrySet),
}
impl Source {
    fn list(&mut self, base: &str) -> Result<Vec<String>, String> {
        match self {
            Self::Folder(root) => {
                let dir = if base.is_empty() {
                    root.clone()
                } else {
                    safe_source_path(root, base)?
                };
                let mut files = Vec::new();
                for entry in
                    std::fs::read_dir(dir).map_err(|e| e.to_string())?
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let ty = entry.file_type().map_err(|e| e.to_string())?;
                    let mut name =
                        entry.file_name().to_string_lossy().into_owned();
                    if noise(&name) {
                        continue;
                    }
                    if ty.is_symlink() {
                        continue;
                    }
                    if ty.is_dir() {
                        name.push('/');
                    }
                    files.push(name);
                    if files.len() > MAX_ENTRIES {
                        return Err("too-many-files".into());
                    }
                }
                Ok(files)
            }
            Self::Zip(_, entries) => {
                let prefix = if base.is_empty() {
                    String::new()
                } else {
                    format!("{base}/")
                };
                let mut names: Vec<String> = entries
                    .child_folders(&prefix)
                    .into_iter()
                    .map(|folder| format!("{folder}/"))
                    .collect();
                names.extend(
                    entries
                        .files
                        .iter()
                        .filter_map(|path| path.strip_prefix(&prefix))
                        .filter(|name| !name.contains('/') && !noise(name))
                        .map(str::to_string),
                );
                Ok(names)
            }
        }
    }
    fn read(&mut self, path: &str, limit: u64) -> Result<Vec<u8>, String> {
        match self {
            Self::Folder(root) => read_bounded(
                std::fs::File::open(safe_source_path(root, path)?)
                    .map_err(|e| e.to_string())?,
                limit,
            ),
            Self::Zip(archive, _) => {
                // Match decoded names just like the shared extractor, including Windows separators.
                let index = (0..archive.len())
                    .find(|&i| {
                        archive.by_index_raw(i).ok().is_some_and(|e| {
                            crate::api::pack::detect::decode_zip_entry_name(
                                e.name_raw(),
                            )
                            .replace('\\', "/")
                                == path
                        })
                    })
                    .ok_or("missing-file")?;
                read_bounded(
                    archive.by_index(index).map_err(|e| e.to_string())?,
                    limit,
                )
            }
        }
    }
}

fn at(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.into()
    } else {
        format!("{base}/{name}")
    }
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
fn version(value: &str) -> bool {
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
    let main = attribute(&manifest, "Main-Class")?;
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
                let cp = attribute(&manifest, "Class-Path")?;
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

fn inspect_root(
    source: &mut Source,
    base: &str,
    names: &[String],
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
    if candidates.is_empty() {
        if names.iter().any(|n| n == "axolotl-server.json")
            || (names.iter().any(|n| n == "server.properties")
                && names
                    .iter()
                    .any(|n| n.to_ascii_lowercase().ends_with(".jar")))
        {
            let mut result =
                candidate("unknown", String::new(), None, None, false);
            result.inner_base = base.into();
            result.problem = Some("unsupported-type".into());
            return Ok(Some(result));
        }
        return Ok(None);
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
    let mut result = candidates.remove(0);
    if result.server_type == "fabric" && result.loader_version.is_none() {
        result.problem = Some("missing-version".into());
    }
    for other in candidates {
        if other.problem.is_some() {
            result.problem = other.problem.clone();
        }
        if result.server_type != other.server_type
            || result.game_version != other.game_version
            || result.loader_version != other.loader_version
        {
            result.problem = Some("conflicting-versions".into());
        } else if result.jar_name != other.jar_name
            && result.jar_name.is_some()
            && other.jar_name.is_some()
        {
            result.problem = Some("multiple-launchers".into());
        } else if !other.needs_install {
            result.needs_install = false;
        }
    }
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

/// Descends only single-directory wrappers; never searches worlds/mods recursively.
pub fn inspect_server_import(
    path: &Path,
) -> Result<Option<ServerImportPreview>, String> {
    let mut source = if path.is_dir() {
        if std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("symbolic-links".into());
        }
        Source::Folder(path.into())
    } else if path
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("zip"))
    {
        let mut archive = zip::ZipArchive::new(
            std::fs::File::open(path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let entries =
            ZipEntrySet::from_archive_with_cancellation(&mut archive, None)?;
        Source::Zip(archive, entries)
    } else {
        return Ok(None);
    };
    let mut base = String::new();
    for _ in 0..16 {
        let mut names = source.list(&base)?;
        names.sort();
        if let Some(result) = inspect_root(&mut source, &base, &names)? {
            return Ok(Some(result));
        }
        if names.len() == 1 && names[0].ends_with('/') {
            base = at(&base, names[0].trim_end_matches('/'));
        } else {
            return Ok(None);
        }
    }
    Err("wrapping-too-deep".into())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::io::Write;
    pub(crate) fn jar(main: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        out.start_file("META-INF/MANIFEST.MF", options).unwrap();
        write!(out, "Manifest-Version: 1.0\nMain-Class: {main}\n").unwrap();
        for (name, value) in files {
            out.start_file(*name, options).unwrap();
            out.write_all(value.as_bytes()).unwrap();
        }
        out.finish().unwrap().into_inner()
    }
    pub(crate) fn forge() -> Vec<u8> {
        jar(
            "net.minecraftforge.installer.SimpleInstaller",
            &[
                (
                    "install_profile.json",
                    r#"{"spec":1,"minecraft":"1.20.1","version":"1.20.1-forge-47.4.16","json":"/version.json","libraries":[{"name":"net.minecraftforge:forge:1.20.1-47.4.16:universal"}]}"#,
                ),
                (
                    "version.json",
                    r#"{"id":"1.20.1-forge-47.4.16","inheritsFrom":"1.20.1"}"#,
                ),
            ],
        )
    }
    pub(crate) fn vanilla() -> Vec<u8> {
        jar(
            "net.minecraft.bundler.Main",
            &[("version.json", r#"{"id":"1.20.1"}"#)],
        )
    }
    #[test]
    fn external_server_installers_precede_generic_modded_clients() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("arbitrary-name.jar"), forge())
            .unwrap();
        std::fs::create_dir(root.path().join("mods")).unwrap();
        std::fs::write(root.path().join("mods/example.jar"), b"mod").unwrap();
        assert!(
            matches!(crate::api::drop_classifier::classify_dropped_item_with_candidates(root.path(), false), crate::api::drop_classifier::DroppedItemType::Server { preview, .. } if preview.game_version == "1.20.1" && preview.loader_version.as_deref() == Some("47.4.16") && preview.needs_install)
        );
    }
    #[test]
    fn external_server_client_mods_are_not_server_evidence() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("mods")).unwrap();
        std::fs::write(root.path().join("mods/server-mod.jar"), forge())
            .unwrap();
        std::fs::write(
            root.path().join("server.jar"),
            jar(
                "net.minecraft.client.main.Main",
                &[("version.json", r#"{"id":"1.20.1"}"#)],
            ),
        )
        .unwrap();
        assert!(inspect_server_import(root.path()).unwrap().is_none());
        assert!(!matches!(
            crate::api::drop_classifier::classify_dropped_item(root.path()),
            crate::api::drop_classifier::DroppedItemType::Server { .. }
        ));
    }
    #[test]
    fn external_server_zip_wrapper_detection_preserves_root() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("backup.zip");
        let mut out =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        out.start_file("outer/inner/server/renamed.jar", opts)
            .unwrap();
        out.write_all(&forge()).unwrap();
        out.start_file("outer/inner/server/mods/a.jar", opts)
            .unwrap();
        out.write_all(b"mod").unwrap();
        out.start_file("__MACOSX/noise", opts).unwrap();
        out.write_all(b"noise").unwrap();
        out.finish().unwrap();
        let preview = inspect_server_import(&path).unwrap().unwrap();
        assert_eq!(preview.inner_base, "outer/inner/server");
        assert!(preview.needs_install);
    }
    #[test]
    fn external_server_fabric_and_paper_and_vanilla_use_content() {
        for (kind, bytes) in [
            ("vanilla", vanilla()),
            (
                "paper",
                jar(
                    "io.papermc.paperclip.Main",
                    &[("version.json", r#"{"id":"1.20.1"}"#)],
                ),
            ),
            (
                "fabric",
                jar(
                    "net.fabricmc.installer.ServerLauncher",
                    &[(
                        "install.properties",
                        "game-version=1.20.1\nfabric-loader-version=0.16.14\n",
                    )],
                ),
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("renamed.jar"), bytes).unwrap();
            let preview = inspect_server_import(root.path()).unwrap().unwrap();
            assert_eq!(preview.server_type, kind);
            assert_eq!(preview.game_version, "1.20.1");
            assert_eq!(preview.problem, None);
            assert_eq!(preview.jar_name.as_deref(), Some("renamed.jar"));
        }
    }
    #[test]
    fn external_server_conflicts_and_missing_versions_are_blocked() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("first.jar"), vanilla()).unwrap();
        std::fs::write(root.path().join("second.jar"), vanilla()).unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("multiple-launchers")
        );
        std::fs::write(
            root.path().join("second.jar"),
            jar(
                "net.minecraft.bundler.Main",
                &[("version.json", r#"{"id":"1.21.4"}"#)],
            ),
        )
        .unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("conflicting-versions")
        );
        std::fs::remove_file(root.path().join("second.jar")).unwrap();
        std::fs::write(
            root.path().join("first.jar"),
            jar("net.minecraft.bundler.Main", &[]),
        )
        .unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("missing-version")
        );
    }
    #[test]
    fn external_server_installed_forge_combines_installer_metadata() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("forge.jar"), forge()).unwrap();
        let libs = root
            .path()
            .join("libraries/net/minecraftforge/forge/1.20.1-47.4.16");
        std::fs::create_dir_all(&libs).unwrap();
        std::fs::write(libs.join("unix_args.txt"),"--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.4.16").unwrap();
        let result = inspect_server_import(root.path()).unwrap().unwrap();
        assert!(!result.needs_install);
        assert_eq!(result.problem, None);
        std::fs::write(libs.join("win_args.txt"),"--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.0.0").unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("conflicting-versions")
        );
    }
    #[test]
    fn external_server_installed_conflict_survives_matching_installer() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("forge.jar"), forge()).unwrap();
        let libs = root
            .path()
            .join("libraries/net/minecraftforge/forge/1.20.1-47.4.16");
        std::fs::create_dir_all(&libs).unwrap();
        std::fs::write(libs.join("unix_args.txt"), "--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.0.0").unwrap();
        std::fs::write(libs.join("win_args.txt"), "--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.4.16").unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("conflicting-versions")
        );
    }
    #[test]
    fn external_server_forge_metadata_is_cross_checked() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("installer.jar"), forge()).unwrap();
        let result = inspect_server_import(root.path()).unwrap().unwrap();
        assert_eq!(result.loader_version.as_deref(), Some("47.4.16"));
        assert_eq!(result.problem, None);
        std::fs::write(root.path().join("installer.jar"),jar("net.minecraftforge.installer.SimpleInstaller",&[
            ("install_profile.json",r#"{"minecraft":"1.20.1","version":"1.20.1-forge-47.4.16"}"#),
            ("version.json",r#"{"inheritsFrom":"1.20.1","id":"1.20.1-forge-47.0.0"}"#),
        ])).unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("conflicting-versions")
        );
        std::fs::write(root.path().join("installer.jar"),jar("net.minecraftforge.installer.SimpleInstaller",&[("install_profile.json",r#"{"minecraft":"1.20.1","path":"net.minecraftforge:forge:1.20.1-47.4.16:universal"}"#)])).unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .loader_version
                .as_deref(),
            Some("47.4.16")
        );
    }
    #[test]
    fn external_server_bundler_cache_is_not_a_client_instance() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("server.jar"), vanilla()).unwrap();
        std::fs::create_dir_all(root.path().join("versions/1.20.1")).unwrap();
        std::fs::write(
            root.path().join("versions/1.20.1/server.jar"),
            b"bundled-server",
        )
        .unwrap();
        assert_eq!(
            inspect_server_import(root.path()).unwrap().unwrap().problem,
            None
        );
        std::fs::write(root.path().join("versions/1.20.1/1.20.1.json"), b"{}")
            .unwrap();
        assert_eq!(
            inspect_server_import(root.path())
                .unwrap()
                .unwrap()
                .problem
                .as_deref(),
            Some("client-server-conflict")
        );
    }
}
