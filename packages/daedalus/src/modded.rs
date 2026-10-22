use crate::minecraft::{
    Argument, ArgumentType, JavaVersion, Library, LibraryDownloads,
    VersionInfo, VersionType,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// The latest version of the format the fabric model structs deserialize to
pub const CURRENT_FABRIC_FORMAT_VERSION: usize = 1;
/// The latest version of the format the fabric model structs deserialize to
pub const CURRENT_FORGE_FORMAT_VERSION: usize = 3;
/// The latest version of the format the quilt model structs deserialize to
pub const CURRENT_QUILT_FORMAT_VERSION: usize = 2;
/// The latest version of the format the neoforge model structs deserialize to
pub const CURRENT_NEOFORGE_FORMAT_VERSION: usize = 1;
/// Current OptiFine manifest cache format.
pub const CURRENT_OPTIFINE_FORMAT_VERSION: usize = 1;
/// Current Cleanroom manifest cache format.
pub const CURRENT_CLEANROOM_FORMAT_VERSION: usize = 1;
/// Current LiteLoader manifest cache format.
pub const CURRENT_LITELOADER_FORMAT_VERSION: usize = 2;
/// Current Legacy Fabric manifest cache format.
pub const CURRENT_LEGACY_FABRIC_FORMAT_VERSION: usize = 1;
/// Current Babric manifest cache format.
pub const CURRENT_BABRIC_FORMAT_VERSION: usize = 2;

/// Metadata for locating and caching a loader manifest.
#[derive(Debug, Clone)]
pub struct LoaderManifestMetadata {
    /// The canonical loader name used in launcher-meta paths.
    pub loader: String,
    /// The latest manifest format version for this loader.
    pub format_version: usize,
    /// The cache key that includes the loader format version.
    pub cache_key: String,
    /// Optional Minecraft version for game-scoped loader metadata.
    pub game_version: Option<String>,
    /// The fallback launcher-meta path to the manifest.
    pub path: String,
}

/// Returns metadata for the latest manifest format for the provided loader.
pub fn loader_manifest_metadata(loader: &str) -> LoaderManifestMetadata {
    let format_version = current_loader_manifest_format_version(loader);
    let cache_key = format!("{loader}-v{format_version}");
    let path = fallback_loader_manifest_path(loader);

    LoaderManifestMetadata {
        loader: loader.to_string(),
        format_version,
        cache_key,
        game_version: None,
        path,
    }
}

/// Returns metadata for a loader manifest scoped to one Minecraft version.
pub fn loader_manifest_metadata_for_game(
    loader: &str,
    game_version: &str,
) -> LoaderManifestMetadata {
    let mut metadata = loader_manifest_metadata(loader);
    metadata.cache_key = format!("{}:{game_version}", metadata.cache_key);
    metadata.game_version = Some(game_version.to_string());
    metadata
}

/// Returns loader manifest metadata from a versioned cache key.
pub fn loader_manifest_metadata_from_cache_key(
    cache_key: &str,
) -> LoaderManifestMetadata {
    let (manifest_key, game_version) = cache_key
        .split_once(':')
        .map_or((cache_key, None), |(key, game_version)| {
            (key, Some(game_version.to_string()))
        });
    if let Some((loader, format_version)) = manifest_key
        .rsplit_once("-v")
        .and_then(|(loader, version)| {
            version
                .parse::<usize>()
                .ok()
                .map(|version| (loader, version))
        })
    {
        let path = fallback_loader_manifest_path(loader);

        LoaderManifestMetadata {
            loader: loader.to_string(),
            format_version,
            cache_key: cache_key.to_string(),
            game_version,
            path,
        }
    } else {
        loader_manifest_metadata(cache_key)
    }
}

fn current_loader_manifest_format_version(loader: &str) -> usize {
    match loader {
        "fabric" => CURRENT_FABRIC_FORMAT_VERSION,
        "forge" => CURRENT_FORGE_FORMAT_VERSION,
        "quilt" => CURRENT_QUILT_FORMAT_VERSION,
        "neo" => CURRENT_NEOFORGE_FORMAT_VERSION,
        "optifine" => CURRENT_OPTIFINE_FORMAT_VERSION,
        "cleanroom" => CURRENT_CLEANROOM_FORMAT_VERSION,
        "lite_loader" => CURRENT_LITELOADER_FORMAT_VERSION,
        "legacy_fabric" => CURRENT_LEGACY_FABRIC_FORMAT_VERSION,
        "babric" => CURRENT_BABRIC_FORMAT_VERSION,
        _ => 0,
    }
}

fn fallback_loader_manifest_path(loader: &str) -> String {
    let format_version = match loader {
        "quilt" => 1,
        _ => 0,
    };
    format!("{loader}/v{format_version}/manifest.json")
}

/// The dummy replace string library names, inheritsFrom, and version names should be replaced with
pub const DUMMY_REPLACE_STRING: &str = "${modrinth.gameVersion}";

/// Returns whether a Minecraft version uses the unobfuscated distribution.
pub fn uses_unobfuscated_minecraft(game_version: &str) -> bool {
    if let Some((year, snapshot)) = game_version.split_once('w') {
        let snapshot = snapshot.as_bytes();
        return year.parse::<u32>().is_ok_and(|year| year >= 26)
            && snapshot.len() >= 3
            && snapshot[..2].iter().all(u8::is_ascii_digit)
            && snapshot[2..].iter().all(u8::is_ascii_alphabetic);
    }

    let mut components = game_version.split('.');
    let Some(major) = components
        .next()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return false;
    };
    let Some(minor) = components
        .next()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return false;
    };

    if components.any(|value| value.parse::<u32>().is_err()) {
        return false;
    }

    major > 26 || (major == 26 && minor >= 1)
}

/// Removes loader libraries that are incompatible with the selected game version.
///
/// For Cleanroom the vanilla LWJGL 2 family is dropped: the
/// `org.lwjgl.lwjgl:lwjgl` binding (shares the `org.lwjgl` package with the
/// Cleanroom LWJGL 3 line), the natives-only carrier
/// `org.lwjgl.lwjgl:lwjgl-platform` (whose root-level extracted `lwjgl.dll` /
/// `liblwjgl.so` / `liblwjgl.dylib` would be what `System.loadLibrary("lwjgl")`
/// resolves first in the natives directory, while the Cleanroom LWJGL 3
/// natives are nested), and `org.lwjgl.lwjgl:lwjgl_util` at exactly
/// `2.9.4-nightly-20150209` (LWJGL 2 utility classes the LWJGL 3 port does
/// not provide; the version condition mirrors HMCL's `DefaultLauncher`
/// Cleanroom handling, which removes every classpath entry containing
/// `2.9.4-nightly-20150209`). HMCL itself never classpaths native jars and
/// its first-wins native extraction over self-contained official Cleanroom
/// manifests keeps the old LWJGL 2 native out of the natives directory, so
/// dropping the carrier is an Axolotl-side strengthening for merged
/// direct links, not an HMCL library rule. The vanilla JNA platform and the
/// Mojang ICU bundle are removed by the pre-existing Axolotl Cleanroom rule
/// (not HMCL logic): with Cleanroom's newer JNA line on the classpath the
/// 1.12.2-era `net.java.dev.jna:platform` and `com.ibm.icu:icu4j-core-mojang`
/// carry conflicting runtime-class versions. The Cleanroom LWJGL 3 line
/// (`org.lwjgl:lwjgl:3.4.1-unsafe`), `lwjglxx`, and the vanilla JNA core are
/// untouched.
pub fn normalize_loader_libraries(
    loader: &str,
    game_version: &str,
    libraries: &mut Vec<Library>,
) -> Vec<String> {
    let remove_fabric_intermediary =
        loader == "fabric" && uses_unobfuscated_minecraft(game_version);
    let remove_cleanroom_conflicts = loader == "cleanroom";
    if !remove_fabric_intermediary && !remove_cleanroom_conflicts {
        return Vec::new();
    }

    let mut removed = Vec::new();
    libraries.retain(|library| {
        let mut coordinates = library.name.split(':');
        let group = coordinates.next();
        let artifact = coordinates.next();
        let version = coordinates.next();
        let has_version = version.is_some();
        let is_intermediary = remove_fabric_intermediary
            && group == Some("net.fabricmc")
            && artifact == Some("intermediary")
            && has_version;
        let conflicts_with_cleanroom = remove_cleanroom_conflicts
            && has_version
            && (matches!(
                (group, artifact),
                (Some("org.lwjgl.lwjgl"), Some("lwjgl" | "lwjgl-platform"))
                    | (Some("net.java.dev.jna"), Some("platform"))
                    | (Some("com.ibm.icu"), Some("icu4j-core-mojang"))
            ) || (group == Some("org.lwjgl.lwjgl")
                && artifact == Some("lwjgl_util")
                && version == Some("2.9.4-nightly-20150209")));

        if is_intermediary || conflicts_with_cleanroom {
            removed.push(library.name.clone());
        }

        !is_intermediary && !conflicts_with_cleanroom
    });
    removed
}

/// A data variable entry that depends on the side of the installation
#[derive(Serialize, Deserialize, Debug)]
pub struct SidedDataEntry {
    /// The value on the client
    pub client: String,
    /// The value on the server
    pub server: String,
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    serde_json::from_str::<DateTime<Utc>>(&format!("\"{s}\""))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|date| date.and_utc())
        })
        .map_err(serde::de::Error::custom)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// A partial version returned by fabric meta
pub struct PartialVersionInfo {
    /// The version ID of the version
    pub id: String,
    /// The version ID this partial version inherits from
    pub inherits_from: String,
    /// The time that the version was released
    #[serde(deserialize_with = "deserialize_date")]
    pub release_time: DateTime<Utc>,
    /// The latest time a file in this version was updated
    #[serde(deserialize_with = "deserialize_date")]
    pub time: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The classpath to the main class to launch the game
    pub main_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// (Legacy) Arguments passed to the game
    pub minecraft_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Arguments passed to the game or JVM
    pub arguments: Option<HashMap<ArgumentType, Vec<Argument>>>,
    /// Libraries that the version depends on
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Java runtime required by the loader profile, when it overrides Minecraft.
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
    /// The type of version
    pub type_: VersionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// (Forge-only)
    pub data: Option<HashMap<String, SidedDataEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// (Forge-only) The list of processors to run after downloading the files
    pub processors: Option<Vec<Processor>>,
}

/// A processor to be ran after downloading the files
#[derive(Serialize, Deserialize, Debug)]
pub struct Processor {
    /// Maven coordinates for the JAR library of this processor.
    pub jar: String,
    /// Maven coordinates for all the libraries that must be included in classpath when running this processor.
    pub classpath: Vec<String>,
    /// Arguments for this processor.
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Represents a map of outputs. Keys and values can be data values
    pub outputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Which sides this processor shall be ran on.
    /// Valid values: client, server, extract
    pub sides: Option<Vec<String>>,
}

/// Identifies one library artifact. Entries that differ in version, classifier
/// or extension are different artifacts: their download URLs and hashes are
/// not interchangeable. The classifier matters because 1.19 and newer declare
/// natives as separate `group:artifact:version:natives-<platform>` entries,
/// each with its own artifact URL and operating-system rules, so those entries
/// must never be merged into one another.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ArtifactKey<'a> {
    group: &'a str,
    artifact: &'a str,
    version: &'a str,
    classifier: Option<&'a str>,
    extension: Option<&'a str>,
}

impl<'a> ArtifactKey<'a> {
    fn from_name(name: &'a str) -> Option<Self> {
        let (coordinate, extension) = match name.split_once('@') {
            Some((coordinate, extension)) => (coordinate, Some(extension)),
            None => (name, None),
        };
        let mut parts = coordinate.split(':');
        let group = parts.next()?;
        let artifact = parts.next()?;
        let version = parts.next()?;
        Some(Self {
            group,
            artifact,
            version,
            classifier: parts.next(),
            extension,
        })
    }
}

fn merge_downloads(target: &mut LibraryDownloads, incoming: &LibraryDownloads) {
    match (&mut target.artifact, &incoming.artifact) {
        (Some(existing), Some(candidate)) => {
            if existing.sha1.is_empty() {
                existing.sha1 = candidate.sha1.clone();
            }
            if existing.url.is_empty() {
                existing.url = candidate.url.clone();
            }
            if existing.size == 0 {
                existing.size = candidate.size;
            }
            if existing.path.is_none() {
                existing.path = candidate.path.clone();
            }
        }
        (target @ None, Some(candidate)) => *target = Some(candidate.clone()),
        _ => {}
    }
    if let Some(incoming_classifiers) = &incoming.classifiers {
        let target_classifiers =
            target.classifiers.get_or_insert_with(HashMap::new);
        for (classifier, download) in incoming_classifiers {
            match target_classifiers.get_mut(classifier) {
                Some(existing) => {
                    if existing.sha1.is_empty() {
                        existing.sha1 = download.sha1.clone();
                    }
                    if existing.url.is_empty() {
                        existing.url = download.url.clone();
                    }
                    if existing.size == 0 {
                        existing.size = download.size;
                    }
                    if existing.path.is_none() {
                        existing.path = download.path.clone();
                    }
                }
                None => {
                    target_classifiers
                        .insert(classifier.clone(), download.clone());
                }
            }
        }
    }
}

/// Fills the gaps of `base` from another entry carrying the same coordinate.
/// Metadata is only ever completed: a field that is already present keeps its
/// value, so a URL, hash or native condition is never taken from a different
/// version of the same artifact. Classpath participation is unioned, because
/// dropping download metadata would make a required file invisible to the
/// downloader even when another entry asks for it on the classpath.
fn merge_library_metadata(base: &mut Library, incoming: &Library) {
    if base.url.is_none() {
        base.url = incoming.url.clone();
    }
    if base.natives.is_none() {
        base.natives = incoming.natives.clone();
    }
    if base.rules.is_none() {
        base.rules = incoming.rules.clone();
    }
    if base.extract.is_none() {
        base.extract = incoming.extract.clone();
    }
    match (&base.checksums, &incoming.checksums) {
        (None, Some(checksums)) => base.checksums = Some(checksums.clone()),
        (Some(existing), Some(candidate)) if existing != candidate => {
            base.checksums = None;
        }
        _ => {}
    }
    if let Some(downloads) = &incoming.downloads {
        merge_downloads(
            base.downloads.get_or_insert(LibraryDownloads {
                artifact: None,
                classifiers: None,
            }),
            downloads,
        );
    }
    base.include_in_classpath |= incoming.include_in_classpath;
    base.downloadable |= incoming.downloadable;
}

/// Merges a partial version into a complete one
pub fn merge_partial_version(
    partial: PartialVersionInfo,
    merge: VersionInfo,
) -> VersionInfo {
    let merge_id = merge.id.clone();

    // Every library is kept here. Duplicates are merged below, keyed by the
    // exact coordinate, so metadata carried by only one of the entries (the
    // `natives` and `downloads.classifiers` of a native carrier, for example)
    // survives instead of being dropped with the duplicate.
    let libraries = merge.libraries;

    let arguments = if let Some(partial_args) = partial.arguments {
        if let Some(merge_args) = merge.arguments {
            let mut new_map = HashMap::new();

            fn argument_key(argument: &Argument) -> String {
                serde_json::to_string(argument)
                    .unwrap_or_else(|_| format!("{argument:?}"))
            }

            fn add_keys(
                new_map: &mut HashMap<ArgumentType, Vec<Argument>>,
                args: HashMap<ArgumentType, Vec<Argument>>,
            ) {
                for (type_, arguments) in args {
                    let existing = new_map.entry(type_).or_default();
                    let max_overlap = existing.len().min(arguments.len());
                    let overlap = (1..=max_overlap)
                        .rev()
                        .find(|&length| {
                            existing[existing.len() - length..]
                                .iter()
                                .map(argument_key)
                                .eq(arguments[..length]
                                    .iter()
                                    .map(argument_key))
                        })
                        .unwrap_or(0);
                    existing.extend(arguments.into_iter().skip(overlap));
                }
            }

            add_keys(&mut new_map, merge_args);
            add_keys(&mut new_map, partial_args);

            Some(new_map)
        } else {
            Some(partial_args)
        }
    } else {
        merge.arguments
    };
    let mut libraries = libraries
        .into_iter()
        .chain(partial.libraries)
        .map(|mut library| {
            library.name =
                library.name.replace(DUMMY_REPLACE_STRING, &merge_id);
            library
        })
        .collect::<Vec<_>>();
    // Loader profiles may repeat a vanilla coordinate with a reduced
    // description (Forge 1.16.x is a notable example: its duplicate LWJGL
    // entry omits `natives` and `downloads.classifiers`). Merge metadata from
    // duplicates before dropping them so native archives remain discoverable,
    // while entries that differ in version or extension stay separate.
    let mut merged_libraries: Vec<Library> =
        Vec::with_capacity(libraries.len());
    for library in libraries {
        let key = ArtifactKey::from_name(&library.name);
        let existing = key.as_ref().and_then(|key| {
            merged_libraries.iter_mut().find(|existing| {
                ArtifactKey::from_name(&existing.name).as_ref() == Some(key)
            })
        });
        match existing {
            Some(existing) => merge_library_metadata(existing, &library),
            None => merged_libraries.push(library),
        }
    }
    libraries = merged_libraries;

    let minecraft_arguments =
        match (merge.minecraft_arguments, partial.minecraft_arguments) {
            (Some(base), Some(loader)) => {
                Some(merge_legacy_arguments(&base, &loader))
            }
            (base, loader) => loader.or(base),
        };

    VersionInfo {
        arguments,
        asset_index: merge.asset_index,
        assets: merge.assets,
        downloads: merge.downloads,
        id: partial.id.replace(DUMMY_REPLACE_STRING, &merge_id),
        java_version: partial.java_version.or(merge.java_version),
        libraries,
        logging: merge.logging,
        main_class: if let Some(main_class) = partial.main_class {
            main_class
        } else {
            merge.main_class
        },
        minecraft_arguments,
        minimum_launcher_version: merge.minimum_launcher_version,
        release_time: merge.release_time,
        time: merge.time,
        type_: partial.type_,
        data: partial.data,
        processors: partial.processors,
    }
}

fn merge_legacy_arguments(base: &str, loader: &str) -> String {
    let base_tokens = base.split_whitespace().collect::<Vec<_>>();
    let loader_tokens = loader.split_whitespace().collect::<Vec<_>>();
    let base_pairs = argument_pairs(&base_tokens);
    let loader_pairs = argument_pairs(&loader_tokens);
    let mut merged = base_tokens.join(" ");
    for (start, end) in loader_pairs {
        let pair = &loader_tokens[start..end];
        if !base_pairs.iter().any(|(base_start, base_end)| {
            base_tokens[*base_start..*base_end] == *pair
        }) {
            if !merged.is_empty() {
                merged.push(' ');
            }
            merged.push_str(&pair.join(" "));
        }
    }
    merged
}

fn argument_pairs(tokens: &[&str]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let end = if tokens[index].starts_with('-') && index + 1 < tokens.len()
        {
            index + 2
        } else {
            index + 1
        };
        pairs.push((index, end));
        index = end;
    }
    pairs
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::minecraft::{
        LibraryDownload, LibraryDownloads, Os, OsRule, Rule, RuleAction,
    };
    use chrono::Utc;
    use std::collections::HashMap;

    fn library(name: &str) -> Library {
        Library {
            downloads: None,
            extract: None,
            name: name.to_string(),
            url: None,
            natives: None,
            rules: None,
            checksums: None,
            include_in_classpath: true,
            downloadable: true,
        }
    }

    fn version_info() -> VersionInfo {
        VersionInfo {
            arguments: Some(HashMap::from([(
                ArgumentType::Game,
                vec![Argument::Normal("--demo".to_string())],
            )])),
            asset_index: serde_json::from_value(serde_json::json!({
                "id": "legacy",
                "sha1": "test",
                "size": 0,
                "totalSize": 0,
                "url": "https://example.com/index.json"
            }))
            .unwrap(),
            assets: "legacy".to_string(),
            downloads: HashMap::new(),
            id: "1.12.2-forge".to_string(),
            java_version: None,
            libraries: vec![
                library("example:shared:1.0"),
                library("example:base:1.0"),
            ],
            logging: None,
            main_class: "example.Main".to_string(),
            minecraft_arguments: Some(
                "--username ${auth_player_name} --gameDir ${game_directory}"
                    .to_string(),
            ),
            minimum_launcher_version: 0,
            release_time: Utc::now(),
            time: Utc::now(),
            type_: VersionType::Release,
            data: None,
            processors: None,
        }
    }

    #[test]
    fn merge_partial_version_keeps_loader_order_and_removes_exact_duplicates() {
        let now = Utc::now();
        let mut minecraft = version_info();
        minecraft.release_time = "2017-09-18T08:39:46Z".parse().unwrap();
        minecraft.time = "2021-12-15T15:04:05Z".parse().unwrap();
        let release_time = minecraft.release_time;
        let time = minecraft.time;
        let partial = PartialVersionInfo {
            id: "1.12.2-liteloader".to_string(),
            inherits_from: "1.12.2".to_string(),
            release_time: now,
            time: now,
            main_class: Some("net.minecraft.launchwrapper.Launch".to_string()),
            minecraft_arguments: None,
            arguments: Some(HashMap::from([(
                ArgumentType::Game,
                vec![
                    Argument::Normal("--demo".to_string()),
                    Argument::Normal("--tweakClass".to_string()),
                    Argument::Normal("example.Tweaker".to_string()),
                ],
            )])),
            libraries: vec![
                library("example:shared:1.0"),
                library("example:lite:1.0"),
            ],
            java_version: None,
            type_: VersionType::Release,
            data: None,
            processors: None,
        };

        let merged = merge_partial_version(partial, minecraft);
        assert_eq!(merged.release_time, release_time);
        assert_eq!(merged.time, time);
        assert_eq!(merged.id, "1.12.2-liteloader");
        assert_eq!(
            merged
                .libraries
                .iter()
                .map(|library| library.name.as_str())
                .collect::<Vec<_>>(),
            ["example:shared:1.0", "example:base:1.0", "example:lite:1.0"]
        );
        assert_eq!(merged.main_class, "net.minecraft.launchwrapper.Launch");
        assert_eq!(
            merged.minecraft_arguments.as_deref(),
            Some("--username ${auth_player_name} --gameDir ${game_directory}")
        );
        assert_eq!(
            merged.arguments.unwrap()[&ArgumentType::Game]
                .iter()
                .filter_map(|argument| match argument {
                    Argument::Normal(value) => Some(value.as_str()),
                    Argument::Ruled { .. } => None,
                })
                .collect::<Vec<_>>(),
            ["--demo", "--tweakClass", "example.Tweaker"]
        );
    }

    #[test]
    fn merge_partial_version_preserves_repeated_jvm_options() {
        let now = Utc::now();
        let mut minecraft = version_info();
        minecraft.arguments = Some(HashMap::from([(
            ArgumentType::Jvm,
            vec![Argument::Normal("-cp".to_string())],
        )]));
        let partial = PartialVersionInfo {
            id: "1.20.1-forge".to_string(),
            inherits_from: "1.20.1".to_string(),
            release_time: now,
            time: now,
            main_class: Some(
                "cpw.mods.bootstraplauncher.BootstrapLauncher".to_string(),
            ),
            minecraft_arguments: None,
            arguments: Some(HashMap::from([(
                ArgumentType::Jvm,
                vec![
                    Argument::Normal("--add-opens".to_string()),
                    Argument::Normal(
                        "java.base/java.util.jar=cpw.mods.securejarhandler"
                            .to_string(),
                    ),
                    Argument::Normal("--add-opens".to_string()),
                    Argument::Normal(
                        "java.base/java.lang.invoke=cpw.mods.securejarhandler"
                            .to_string(),
                    ),
                ],
            )])),
            libraries: Vec::new(),
            java_version: None,
            type_: VersionType::Release,
            data: None,
            processors: None,
        };

        let merged = merge_partial_version(partial, minecraft);
        let arguments = &merged.arguments.unwrap()[&ArgumentType::Jvm];
        assert_eq!(
            arguments
                .iter()
                .filter(|argument| {
                    matches!(argument, Argument::Normal(value) if value == "--add-opens")
                })
                .count(),
            2
        );
    }
    fn download(
        url: &str,
        sha1: &str,
        size: u32,
        path: Option<&str>,
    ) -> LibraryDownload {
        LibraryDownload {
            path: path.map(str::to_string),
            sha1: sha1.to_string(),
            size,
            url: url.to_string(),
        }
    }

    fn natives_entry(classifier: &str) -> Library {
        let mut native = library("org.lwjgl:lwjgl:3.2.2");
        native.natives = Some(HashMap::from([(
            Os::Windows,
            format!("natives-windows{classifier}"),
        )]));
        native.downloads = Some(LibraryDownloads {
            artifact: Some(download(
                "https://example.com/lwjgl.jar",
                "aaa111",
                100,
                Some("org/lwjgl/lwjgl.jar"),
            )),
            classifiers: Some(HashMap::from([(
                format!("natives-windows{classifier}"),
                download(
                    "https://example.com/lwjgl-natives.jar",
                    "bbb222",
                    50,
                    None,
                ),
            )])),
        });
        native
    }

    fn partial_with_libraries(libraries: Vec<Library>) -> PartialVersionInfo {
        let now = Utc::now();
        PartialVersionInfo {
            id: "1.16.4-forge".to_string(),
            inherits_from: "1.16.4".to_string(),
            release_time: now,
            time: now,
            main_class: Some("example.Main".to_string()),
            minecraft_arguments: None,
            arguments: None,
            libraries,
            java_version: None,
            type_: VersionType::Release,
            data: None,
            processors: None,
        }
    }
    #[test]
    fn merged_library_keeps_natives_artifact_and_classifier_metadata() {
        let mut minecraft = version_info();
        let mut reduced = library("org.lwjgl:lwjgl:3.2.2");
        reduced.downloads = Some(LibraryDownloads {
            artifact: Some(download(
                "https://example.com/lwjgl.jar",
                "aaa111",
                100,
                None,
            )),
            classifiers: None,
        });
        minecraft.libraries = vec![reduced];

        let merged = merge_partial_version(
            partial_with_libraries(vec![natives_entry("")]),
            minecraft,
        );

        let lwjgl = merged
            .libraries
            .iter()
            .find(|library| library.name == "org.lwjgl:lwjgl:3.2.2")
            .expect("merged lwjgl library");
        assert!(
            lwjgl.natives.is_some(),
            "the natives declaration must survive the merge"
        );
        let downloads = lwjgl.downloads.as_ref().expect("merged downloads");
        let artifact = downloads.artifact.as_ref().expect("artifact");
        assert_eq!(artifact.url, "https://example.com/lwjgl.jar");
        assert_eq!(artifact.sha1, "aaa111");
        let classifiers = downloads.classifiers.as_ref().expect("classifiers");
        let native = classifiers
            .get("natives-windows")
            .expect("native classifier kept from the partial entry");
        assert_eq!(native.sha1, "bbb222");
        assert!(lwjgl.include_in_classpath);
    }

    #[test]
    fn merged_library_does_not_take_metadata_from_another_version() {
        let mut minecraft = version_info();
        let mut other_version = library("org.lwjgl:lwjgl:3.2.1");
        other_version.downloads = Some(LibraryDownloads {
            artifact: Some(download(
                "https://example.com/lwjgl-3.2.1.jar",
                "oldsha1",
                90,
                None,
            )),
            classifiers: None,
        });
        minecraft.libraries = vec![other_version];

        let merged = merge_partial_version(
            partial_with_libraries(vec![natives_entry("")]),
            minecraft,
        );

        let urls = merged
            .libraries
            .iter()
            .filter(|library| library.name.starts_with("org.lwjgl:lwjgl:"))
            .map(|library| {
                library
                    .downloads
                    .as_ref()
                    .and_then(|downloads| downloads.artifact.as_ref())
                    .map(|artifact| artifact.url.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        assert!(
            urls.contains(&"https://example.com/lwjgl-3.2.1.jar"),
            "the other version keeps its own URL: {urls:?}"
        );
        assert!(
            urls.contains(&"https://example.com/lwjgl.jar"),
            "the native carrier keeps its own URL: {urls:?}"
        );
    }

    fn allow_os(target: Os) -> Rule {
        Rule {
            action: RuleAction::Allow,
            os: Some(OsRule {
                name: Some(target),
                version: None,
                arch: None,
            }),
            features: None,
        }
    }

    fn rule_os(library: &Library) -> Option<&Os> {
        library
            .rules
            .as_ref()
            .and_then(|rules| rules.first())
            .and_then(|rule| rule.os.as_ref())
            .and_then(|os| os.name.as_ref())
    }

    /// One `group:artifact:version:natives-<platform>` entry as 1.19 and newer
    /// declare natives: its own artifact URL, hash and operating-system rule.
    fn platform_native_entry(classifier: &str, os: Os, url: &str) -> Library {
        let mut native =
            library(&format!("org.lwjgl:lwjgl:3.3.1:{classifier}"));
        native.downloads = Some(LibraryDownloads {
            artifact: Some(download(url, "natives-sha1", 42, None)),
            classifiers: None,
        });
        native.rules = Some(vec![allow_os(os)]);
        native
    }

    #[test]
    fn platform_native_entries_are_never_merged_into_one() {
        let mut minecraft = version_info();
        minecraft.libraries = vec![
            platform_native_entry(
                "natives-windows",
                Os::Windows,
                "https://example.com/lwjgl-natives-windows.jar",
            ),
            platform_native_entry(
                "natives-linux",
                Os::Linux,
                "https://example.com/lwjgl-natives-linux.jar",
            ),
        ];

        let merged = merge_partial_version(
            partial_with_libraries(vec![library("org.lwjgl:lwjgl:3.3.1")]),
            minecraft,
        );

        let natives = merged
            .libraries
            .iter()
            .filter(|library| library.name.contains(":natives-"))
            .collect::<Vec<_>>();
        assert_eq!(
            natives.len(),
            2,
            "each platform native is its own entry: {:?}",
            merged
                .libraries
                .iter()
                .map(|library| library.name.as_str())
                .collect::<Vec<_>>()
        );
        let windows = natives
            .iter()
            .find(|library| library.name.ends_with("natives-windows"))
            .expect("windows native entry");
        assert_eq!(
            windows
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
                .map(|artifact| artifact.url.as_str()),
            Some("https://example.com/lwjgl-natives-windows.jar"),
            "the native keeps its own artifact URL"
        );
        assert_eq!(
            rule_os(windows),
            Some(&Os::Windows),
            "the native keeps its own operating-system rule"
        );
        let linux = natives
            .iter()
            .find(|library| library.name.ends_with("natives-linux"))
            .expect("linux native entry");
        assert_eq!(
            rule_os(linux),
            Some(&Os::Linux),
            "the linux native must not inherit the windows rule"
        );
    }

    #[test]
    fn the_same_artifact_stays_one_entry_even_with_a_classifier() {
        let mut minecraft = version_info();
        minecraft.libraries = vec![natives_entry("")];

        let merged = merge_partial_version(
            partial_with_libraries(vec![natives_entry("-arm64")]),
            minecraft,
        );

        let names = merged
            .libraries
            .iter()
            .map(|library| library.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "org.lwjgl:lwjgl:3.2.2")
                .count(),
            1,
            "one coordinate stays one entry: {names:?}"
        );
        let lwjgl = merged
            .libraries
            .iter()
            .find(|library| library.name == "org.lwjgl:lwjgl:3.2.2")
            .expect("merged lwjgl library");
        let classifiers = lwjgl
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.as_ref())
            .expect("merged native classifiers");
        assert!(
            classifiers.contains_key("natives-windows")
                && classifiers.contains_key("natives-windows-arm64"),
            "a classifier map entry still merges into its coordinate: {classifiers:?}"
        );
    }

    #[test]
    fn a_conflicting_checksum_is_dropped_instead_of_mixed() {
        let mut minecraft = version_info();
        let mut carrier = library("example:native:1.0");
        carrier.checksums = Some(vec!["checksum-a".to_string()]);
        minecraft.libraries = vec![carrier];

        let mut other = library("example:native:1.0");
        other.checksums = Some(vec!["checksum-b".to_string()]);

        let merged = merge_partial_version(
            partial_with_libraries(vec![other]),
            minecraft,
        );
        let native = merged
            .libraries
            .iter()
            .find(|library| library.name == "example:native:1.0")
            .expect("merged library");
        assert_eq!(
            native.checksums, None,
            "unrelated checksums must not be unioned into one unverifiable list"
        );
    }

    #[test]
    fn libraries_without_a_version_are_never_merged_together() {
        let mut minecraft = version_info();
        minecraft.libraries =
            vec![library("example:plain"), library("example:other")];

        let merged = merge_partial_version(
            partial_with_libraries(Vec::new()),
            minecraft,
        );

        assert_eq!(merged.libraries.len(), 2);
    }

    #[test]
    fn a_different_version_of_the_same_artifact_is_not_dropped() {
        let mut minecraft = version_info();
        let mut newer = library("example:shared:2.0");
        newer.include_in_classpath = false;
        minecraft.libraries = vec![newer];

        let merged = merge_partial_version(
            partial_with_libraries(vec![library("example:shared:1.0")]),
            minecraft,
        );

        let names = merged
            .libraries
            .iter()
            .map(|library| library.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            names.contains(&"example:shared:2.0"),
            "another version of the same artifact is a different library: {names:?}"
        );
        assert!(
            names.contains(&"example:shared:1.0"),
            "the partial entry keeps its own version: {names:?}"
        );
    }

    #[test]
    fn merge_unions_rules_and_downloadable_without_losing_metadata() {
        let mut minecraft = version_info();
        let mut reduced = library("example:dual:1.0");
        reduced.downloadable = false;
        reduced.downloads = Some(LibraryDownloads {
            artifact: Some(download(
                "https://example.com/dual.jar",
                "carrier-sha1",
                7,
                None,
            )),
            classifiers: None,
        });
        minecraft.libraries = vec![reduced];

        let mut richer = library("example:dual:1.0");
        richer.include_in_classpath = false;
        richer.rules = Some(vec![Rule {
            action: RuleAction::Allow,
            os: None,
            features: None,
        }]);

        let merged = merge_partial_version(
            partial_with_libraries(vec![richer]),
            minecraft,
        );

        let dual = merged
            .libraries
            .iter()
            .find(|library| library.name == "example:dual:1.0")
            .expect("merged library");
        assert!(
            dual.rules.is_some(),
            "rules carried by only one entry must survive"
        );
        assert!(
            dual.include_in_classpath,
            "classpath participation is unioned, not overwritten"
        );
        assert_eq!(
            dual.downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
                .map(|artifact| artifact.sha1.as_str()),
            Some("carrier-sha1"),
            "download metadata must not be dropped with the duplicate"
        );
    }

    #[test]
    fn merge_partial_version_combines_legacy_game_arguments() {
        let now = Utc::now();
        let mut minecraft = version_info();
        minecraft.minecraft_arguments = Some(
            "--username ${auth_player_name} --version ${version_name} \
             --versionType ${version_type}"
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        let partial = PartialVersionInfo {
            id: "1.12.2-forge".to_string(),
            inherits_from: "1.12.2".to_string(),
            release_time: now,
            time: now,
            main_class: Some("net.minecraft.launchwrapper.Launch".to_string()),
            minecraft_arguments: Some(
                "--username ${auth_player_name} --version ${version_name} \
                 --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker \
                 --versionType Forge"
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            arguments: None,
            libraries: Vec::new(),
            java_version: None,
            type_: VersionType::Release,
            data: None,
            processors: None,
        };

        let merged = merge_partial_version(partial, minecraft);
        assert_eq!(
            merged.minecraft_arguments.as_deref(),
            Some(
                "--username ${auth_player_name} --version ${version_name} \
                 --versionType ${version_type} \
                 --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker \
                 --versionType Forge"
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .as_str()
            )
        );
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A manifest containing information about a mod loader's versions
pub struct Manifest {
    /// The game versions the mod loader supports
    pub game_versions: Vec<Version>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Groups of game versions that share compatible loader version profiles
    pub version_groups: Vec<VersionGroup>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
///  A game version of Minecraft
pub struct Version {
    /// The minecraft version ID
    pub id: String,
    /// Whether the release is stable or not
    pub stable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The loader profile group for this Minecraft version
    pub version_group: Option<String>,
    /// A map that contains loader versions for the game version
    pub loaders: Vec<LoaderVersion>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A group of Minecraft versions that share loader version profiles
pub struct VersionGroup {
    /// The version group ID
    pub id: String,
    /// The loader versions for this version group
    pub loaders: Vec<LoaderVersion>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// A version of a Minecraft mod loader
pub struct LoaderVersion {
    /// The version ID of the loader
    pub id: String,
    /// The URL of the version's manifest
    pub url: String,
    /// Whether the loader is stable or not
    pub stable: bool,
    /// How the version profile at `url` should be resolved.
    #[serde(default, skip_serializing_if = "is_json_profile_source")]
    pub profile_source: LoaderProfileSource,
    /// JSON profile used if the official source cannot be resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_url: Option<String>,
}

/// Source format used to resolve a loader version profile.
#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq,
)]
#[serde(rename_all = "snake_case")]
pub enum LoaderProfileSource {
    /// A partial Minecraft version JSON document.
    #[default]
    Json,
    /// An official Forge-compatible installer JAR.
    Installer,
    /// A profile synthesized from the LiteLoader versions manifest.
    LiteLoader,
    /// A profile synthesized from Babric's Glass Maven metadata and legacy profile.
    Babric,
}

fn is_json_profile_source(source: &LoaderProfileSource) -> bool {
    *source == LoaderProfileSource::Json
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cleanroom_and_forge_profiles_accept_additional_lwjgl_platforms() {
        for (id, minecraft) in [
            ("1.12.2-cleanroom-0.7.5", "1.12.2"),
            ("1.7.10-forge", "1.7.10"),
        ] {
            let input = json!({
                "id": id,
                "inheritsFrom": minecraft,
                "releaseTime": "2026-10-15T00:00:00Z",
                "time": "2026-10-15T00:00:00Z",
                "type": "release",
                "arguments": {"jvm": [{
                    "rules": [{"action": "allow", "os": {"name": "freebsd"}}],
                    "value": "-Dorg.lwjgl.librarypath=natives/freebsd"
                }]},
                "libraries": [{
                    "name": "org.lwjgl:lwjgl:3.3.6:natives-freebsd",
                    "rules": [{"action": "allow", "os": {"name": "freebsd"}}],
                    "natives": {"freebsd": "natives-freebsd"}
                }, {
                    "name": "org.lwjgl:lwjgl-natives-linux-ppc64le:3.4.3",
                    "rules": [{"action": "allow", "os": {"name": "linux-ppc64le"}}]
                }]
            });
            let profile: PartialVersionInfo =
                serde_json::from_value(input).unwrap();
            let library = &profile.libraries[0];
            assert_eq!(
                library.rules.as_ref().unwrap()[0].os.as_ref().unwrap().name,
                Some(crate::minecraft::Os::Freebsd)
            );
            assert_eq!(
                library.natives.as_ref().unwrap()
                    [&crate::minecraft::Os::Freebsd],
                "natives-freebsd"
            );
            assert_eq!(
                profile.libraries[1].rules.as_ref().unwrap()[0]
                    .os
                    .as_ref()
                    .unwrap()
                    .name,
                Some(crate::minecraft::Os::LinuxPpc64le)
            );
            let restored: PartialVersionInfo =
                serde_json::from_value(serde_json::to_value(&profile).unwrap())
                    .unwrap();
            assert_eq!(restored.id, id);
            assert_eq!(restored.inherits_from, minecraft);
            assert!(
                restored.arguments.unwrap().contains_key(&ArgumentType::Jvm)
            );
        }
    }

    fn library(name: &str) -> Library {
        Library {
            downloads: None,
            extract: None,
            name: name.to_string(),
            url: None,
            natives: None,
            rules: None,
            checksums: None,
            include_in_classpath: true,
            downloadable: true,
        }
    }

    #[test]
    fn loader_manifest_metadata_uses_independent_format_versions() {
        let fabric = loader_manifest_metadata("fabric");
        assert_eq!(fabric.format_version, 1);
        assert_eq!(fabric.cache_key, "fabric-v1");
        assert_eq!(fabric.game_version, None);
        assert_eq!(fabric.path, "fabric/v0/manifest.json");

        assert_eq!(loader_manifest_metadata("quilt").format_version, 2);
        assert_eq!(loader_manifest_metadata("forge").format_version, 3);
        assert_eq!(loader_manifest_metadata("neo").format_version, 1);
        assert_eq!(loader_manifest_metadata("babric").format_version, 2);
        assert_eq!(loader_manifest_metadata("babric").cache_key, "babric-v2");
        assert_eq!(
            loader_manifest_metadata("quilt").path,
            "quilt/v1/manifest.json"
        );

        let forge_1201 = loader_manifest_metadata_for_game("forge", "1.20.1");
        let forge_262 = loader_manifest_metadata_for_game("forge", "26.2");
        assert_eq!(forge_1201.cache_key, "forge-v3:1.20.1");
        assert_eq!(forge_262.cache_key, "forge-v3:26.2");
        assert_ne!(forge_1201.cache_key, forge_262.cache_key);
        assert_eq!(forge_1201.game_version.as_deref(), Some("1.20.1"));
        assert_eq!(
            loader_manifest_metadata_from_cache_key(&forge_1201.cache_key)
                .game_version
                .as_deref(),
            Some("1.20.1")
        );
    }

    #[test]
    fn unobfuscated_version_boundary_is_conservative() {
        for version in ["26.1", "26.2", "26.1.1", "27.0", "26w14a"] {
            assert!(uses_unobfuscated_minecraft(version), "{version}");
        }

        for version in [
            "26.0", "26", "1.21", "1.21.11", "25w46a", "26w非", "invalid",
        ] {
            assert!(!uses_unobfuscated_minecraft(version), "{version}");
        }
    }

    #[test]
    fn normalization_only_removes_exact_fabric_intermediary_coordinates() {
        let retained = [
            "net.fabricmc:fabric-loader:0.19.3",
            "net.fabricmc:tiny-mappings-parser:0.3.0",
            "example:intermediary-helper:1.0",
            "example:contains-intermediary:1.0",
        ];
        let mut libraries =
            std::iter::once(library("net.fabricmc:intermediary:26.2"))
                .chain(retained.iter().map(|name| library(name)))
                .collect();

        assert_eq!(
            normalize_loader_libraries("fabric", "26.2", &mut libraries),
            vec!["net.fabricmc:intermediary:26.2"]
        );
        assert_eq!(
            libraries
                .iter()
                .map(|library| library.name.as_str())
                .collect::<Vec<_>>(),
            retained
        );
        assert!(
            normalize_loader_libraries("fabric", "26.2", &mut libraries)
                .is_empty()
        );
        let paths = libraries
            .iter()
            .map(|library| {
                crate::get_path_from_artifact(&library.name).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(!paths.iter().any(|path| {
            path == "net/fabricmc/intermediary/26.2/intermediary-26.2.jar"
        }));
        assert!(paths.iter().any(|path| {
            path == "net/fabricmc/fabric-loader/0.19.3/fabric-loader-0.19.3.jar"
        }));
    }

    #[test]
    fn normalization_removes_intermediary_at_26_1_boundary() {
        let mut libraries = vec![library("net.fabricmc:intermediary:26.1")];
        assert_eq!(
            normalize_loader_libraries("fabric", "26.1", &mut libraries),
            vec!["net.fabricmc:intermediary:26.1"]
        );
        assert!(libraries.is_empty());
    }

    #[test]
    fn normalization_preserves_legacy_fabric_and_other_loaders() {
        for (loader, version) in [
            ("fabric", "1.21"),
            ("quilt", "26.2"),
            ("vanilla", "26.2"),
            ("forge", "26.2"),
            ("neo", "26.2"),
        ] {
            let mut libraries = vec![library("net.fabricmc:intermediary:26.2")];
            assert!(
                normalize_loader_libraries(loader, version, &mut libraries)
                    .is_empty()
            );
            assert_eq!(libraries.len(), 1);
        }
    }

    #[test]
    fn cleanroom_normalization_removes_lwjgl2_family_and_legacy_mojang_conflicts()
     {
        // The vanilla LWJGL 2 family (binding, util at 2.9.4-nightly-20150209,
        // natives carrier at any version) plus the pre-existing Axolotl
        // Cleanroom rule targets (vanilla JNA platform, Mojang ICU) are
        // removed. The Cleanroom LWJGL 3 line, lwjglxx, the version-scoped
        // lwjgl_util at other versions, and the Cleanroom JNA core stay.
        let retained = [
            "org.lwjgl:lwjgl:3.4.1-unsafe",
            "org.lwjgl.lwjgl:lwjgl_util:2.9.2-nightly-20140822",
            "net.java.dev.jna:jna:5.19.1",
            "net.java.dev.jna:jna-platform:5.19.1",
            "com.ibm.icu:icu4j:78.3",
            "com.cleanroommc:lwjglxx:1.1.22",
        ];
        let removed = [
            "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
            "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209",
            "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
            "net.java.dev.jna:platform:3.4.0",
            "com.ibm.icu:icu4j-core-mojang:51.2",
            "org.lwjgl.lwjgl:lwjgl-platform:2.9.2-nightly-20140822",
        ];
        let mut libraries = removed
            .iter()
            .chain(retained.iter())
            .map(|name| library(name))
            .collect();

        assert_eq!(
            normalize_loader_libraries("cleanroom", "1.12.2", &mut libraries),
            removed
        );
        assert_eq!(
            libraries
                .iter()
                .map(|library| library.name.as_str())
                .collect::<Vec<_>>(),
            retained
        );
    }

    #[test]
    fn cleanroom_normalization_matches_lwjgl2_coordinates_by_version_scope() {
        // lwjgl and lwjgl-platform keep the pre-existing artifact match (any
        // version); lwjgl_util only matches at 2.9.4-nightly-20150209,
        // mirroring HMCL's classpath filter for Cleanroom. The LWJGL 3 line
        // and lwjglxx never match.
        let mut libraries = vec![
            library("org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"),
            library("org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209"),
            library("org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209"),
            library("org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822"),
            library("org.lwjgl.lwjgl:lwjgl-platform:2.9.2-nightly-20140822"),
            library("org.lwjgl.lwjgl:lwjgl_util:2.9.2-nightly-20140822"),
            library("org.lwjgl:lwjgl:3.4.1-unsafe"),
            library("org.lwjgl:lwjgl-platform:9.9.9"),
            library("com.cleanroommc:lwjglxx:1.1.22"),
        ];

        assert_eq!(
            normalize_loader_libraries("cleanroom", "1.12.2", &mut libraries),
            [
                "org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl_util:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                "org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.2-nightly-20140822",
            ]
        );
        assert_eq!(
            libraries
                .iter()
                .map(|library| library.name.as_str())
                .collect::<Vec<_>>(),
            [
                "org.lwjgl.lwjgl:lwjgl_util:2.9.2-nightly-20140822",
                "org.lwjgl:lwjgl:3.4.1-unsafe",
                "org.lwjgl:lwjgl-platform:9.9.9",
                "com.cleanroommc:lwjglxx:1.1.22",
            ]
        );
    }

    #[test]
    fn cleanroom_runtime_conflicts_remain_for_other_loaders() {
        for loader in ["vanilla", "forge", "fabric", "quilt", "neo"] {
            let mut libraries = vec![
                library("org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"),
                library(
                    "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
                ),
                library("net.java.dev.jna:platform:3.4.0"),
                library("com.ibm.icu:icu4j-core-mojang:51.2"),
            ];

            assert!(
                normalize_loader_libraries(loader, "1.12.2", &mut libraries)
                    .is_empty()
            );
            assert_eq!(libraries.len(), 4);
        }
    }

    fn merged_fabric_version(game_version: &str) -> VersionInfo {
        let partial: PartialVersionInfo = serde_json::from_value(json!({
            "id": "fabric-loader-0.19.3-${modrinth.gameVersion}",
            "inheritsFrom": "${modrinth.gameVersion}",
            "releaseTime": "2026-07-22T00:00:00Z",
            "time": "2026-07-22T00:00:00Z",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [
                { "name": "net.fabricmc:intermediary:${modrinth.gameVersion}" },
                { "name": "net.fabricmc:fabric-loader:0.19.3" },
                { "name": "org.ow2.asm:asm:9.9" }
            ],
            "type": "release"
        }))
        .unwrap();
        let minecraft: VersionInfo = serde_json::from_value(json!({
            "arguments": {},
            "assetIndex": {
                "id": game_version,
                "sha1": "asset-sha1",
                "size": 1,
                "totalSize": 1,
                "url": "https://example.com/assets.json"
            },
            "assets": game_version,
            "downloads": {},
            "id": game_version,
            "libraries": [],
            "mainClass": "net.minecraft.client.main.Main",
            "minimumLauncherVersion": 0,
            "releaseTime": "2026-07-22T00:00:00Z",
            "time": "2026-07-22T00:00:00Z",
            "type": "release"
        }))
        .unwrap();

        merge_partial_version(partial, minecraft)
    }

    #[test]
    fn merged_fabric_v0_profile_is_normalized_before_consumers() {
        let mut modern = merged_fabric_version("26.2");
        assert!(modern
            .libraries
            .iter()
            .any(|library| library.name == "net.fabricmc:intermediary:26.2"));

        assert_eq!(
            normalize_loader_libraries("fabric", "26.2", &mut modern.libraries),
            vec!["net.fabricmc:intermediary:26.2"]
        );
        let serialized = serde_json::to_string(&modern).unwrap();
        assert!(!serialized.contains("net.fabricmc:intermediary"));
        assert!(serialized.contains("net.fabricmc:fabric-loader:0.19.3"));
        assert!(serialized.contains("org.ow2.asm:asm:9.9"));

        let mut legacy = merged_fabric_version("1.21");
        assert!(
            normalize_loader_libraries("fabric", "1.21", &mut legacy.libraries)
                .is_empty()
        );
        assert!(legacy
            .libraries
            .iter()
            .any(|library| library.name == "net.fabricmc:intermediary:1.21"));
    }
}
