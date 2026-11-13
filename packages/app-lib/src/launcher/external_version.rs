//! Publication of portable versions after internal loader installation completes.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use daedalus::minecraft::{
    DownloadType, Library, LibraryDownload, LibraryDownloads, VersionInfo,
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::direct_ensure::{self, LinkedFilePlan};
use super::direct_link::{
    DirectLinkedLaunch, classified_artifact_path, is_native_only_library,
};
use super::download::MinecraftDownloadProgress;
use super::instance_runtime::InstanceRuntimeAdapter;
use super::local_version::LinkedLibrary;
use crate::instance::QuickPlayType;
use crate::state::{Instance, State};
use crate::util::{fetch, io};

pub(crate) struct PortableVersion {
    pub(crate) document: Value,
    pub(crate) libraries: Vec<LinkedLibrary>,
}

fn uses_fml_runtime(info: &VersionInfo) -> bool {
    matches!(
        info.main_class.as_str(),
        "cpw.mods.bootstraplauncher.BootstrapLauncher"
            | "cpw.mods.modlauncher.Launcher"
            | "net.minecraftforge.bootstrap.BootstrapLauncher"
    )
}

fn is_fml_runtime_library(library: &Library) -> bool {
    let mut coordinate = library.name.split(':');
    let group = coordinate.next();
    let artifact = coordinate.next();
    let _version = coordinate.next();
    let classifier = coordinate.next();
    matches!(
        (group, artifact),
        (
            Some("net.minecraftforge"),
            Some(
                "fmlcore"
                    | "javafmllanguage"
                    | "lowcodelanguage"
                    | "mclanguage"
            )
        )
    ) || (classifier == Some("universal")
        && matches!(
            (group, artifact),
            (Some("net.minecraftforge"), Some("forge"))
                | (Some("net.neoforged"), Some("neoforge"))
        ))
}

fn library_identity(name: &str) -> Option<(String, String)> {
    let mut coordinates = name.split(':');
    Some((
        coordinates.next()?.to_string(),
        coordinates.next()?.to_string(),
    ))
}

fn library_applies_to_host(library: &LinkedLibrary) -> bool {
    library.library.rules.as_deref().is_none_or(|rules| {
        super::parse_rules(
            rules,
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true,
        )
    })
}

/// PCL resolves duplicate Maven identities from the first matching entry.
/// Put the host-applicable entry first so a platform-specific duplicate cannot
/// shadow the runtime artifact needed by this installation.
fn order_duplicate_libraries_for_portable_launch(
    libraries: &mut [LinkedLibrary],
) {
    let mut counts = HashMap::<(String, String), usize>::new();
    for library in libraries.iter() {
        if let Some(identity) = library_identity(&library.library.name) {
            *counts.entry(identity).or_default() += 1;
        }
    }
    let duplicate_identities = counts
        .into_iter()
        .filter_map(|(identity, count)| (count > 1).then_some(identity))
        .collect::<HashSet<_>>();
    let mut positions = HashMap::<(String, String), Vec<usize>>::new();
    for (index, library) in libraries.iter().enumerate() {
        if let Some(identity) = library_identity(&library.library.name)
            && duplicate_identities.contains(&identity)
        {
            positions.entry(identity).or_default().push(index);
        }
    }
    for indices in positions.values() {
        let mut entries = indices
            .iter()
            .map(|&index| libraries[index].clone())
            .collect::<Vec<_>>();
        entries.sort_by_key(|library| !library_applies_to_host(library));
        for (index, library) in indices.iter().copied().zip(entries) {
            libraries[index] = library;
        }
    }
}

fn expand_native_artifact_library(
    library: &LinkedLibrary,
) -> Vec<LinkedLibrary> {
    let Some(downloads) = library.library.downloads.as_ref() else {
        return vec![library.clone()];
    };
    if !library.library.include_in_classpath
        || library.library.natives.is_none()
        || downloads.artifact.is_none()
        || is_native_only_library(&library.library)
    {
        return vec![library.clone()];
    }

    let mut classpath = library.clone();
    classpath.library.natives = None;
    classpath.library.extract = None;
    if let Some(downloads) = classpath.library.downloads.as_mut() {
        downloads.classifiers = None;
    }
    vec![classpath, library.clone()]
}

/// FML discovers game modules through Maven paths, including outputs absent from its classpath.
fn generated_fml_runtime_libraries(
    info: &VersionInfo,
) -> crate::Result<Vec<Library>> {
    let mut libraries = Vec::new();
    if !uses_fml_runtime(info) {
        return Ok(libraries);
    }
    let Some(data) = &info.data else {
        return Ok(libraries);
    };
    for key in ["PATCHED", "MC_SRG", "MC_EXTRA"] {
        let Some(entry) = data.get(key) else {
            continue;
        };
        let coordinate = entry.client.trim().trim_matches(['\'', '"']);
        let coordinate = coordinate.strip_prefix('[').and_then(|coordinate| coordinate.strip_suffix(']')).ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!("FML client runtime artifact {key} has no Maven coordinate: {}", entry.client))
        })?;
        let path = daedalus::get_path_from_artifact(coordinate)?;
        if coordinate.starts_with("com.axolotl.loader-installer:embedded:")
            || !direct_ensure::safe_maven_relative_path(&path)
            || !path.ends_with(".jar")
        {
            return Err(crate::ErrorKind::LauncherError(format!("Invalid FML client runtime artifact {key}: {coordinate} at {path}")).into());
        }
        let sha1 = data
            .get(&format!("{key}_SHA"))
            .and_then(|entry| super::processor_output_sha1(&entry.client))
            .unwrap_or_default()
            .to_string();
        libraries.push(Library {
            name: coordinate.to_string(),
            downloads: Some(LibraryDownloads {
                artifact: Some(LibraryDownload {
                    path: Some(path),
                    sha1,
                    size: 0,
                    url: String::new(),
                }),
                classifiers: None,
            }),
            url: None,
            extract: None,
            natives: None,
            rules: None,
            checksums: None,
            include_in_classpath: true,
            downloadable: false,
        });
    }
    Ok(libraries)
}

fn extend_bootstrap_ignore_argument(
    value: &mut Value,
    filenames: &[String],
) -> bool {
    match value {
        Value::String(argument) => {
            let Some(ignored) = argument.strip_prefix("-DignoreList=") else {
                return false;
            };
            let mut ignored = ignored
                .split(',')
                .filter(|prefix| !prefix.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            for filename in filenames {
                if !ignored.iter().any(|prefix| filename.starts_with(prefix)) {
                    ignored.push(filename.clone());
                }
            }
            *argument = format!("-DignoreList={}", ignored.join(","));
            true
        }
        Value::Array(arguments) => {
            arguments.iter_mut().fold(false, |changed, argument| {
                extend_bootstrap_ignore_argument(argument, filenames) || changed
            })
        }
        Value::Object(argument) => {
            argument.get_mut("value").is_some_and(|value| {
                extend_bootstrap_ignore_argument(value, filenames)
            })
        }
        _ => false,
    }
}

/// Declared FML game JARs must remain outside BootstrapLauncher's parent module layer.
fn exclude_fml_game_modules_from_bootstrap(
    document: &mut Value,
    libraries: &[LinkedLibrary],
    generated: &[Library],
) -> crate::Result<()> {
    let mut filenames = Vec::new();
    for library in libraries {
        if is_fml_runtime_library(&library.library)
            || generated
                .iter()
                .any(|generated| generated.name == library.library.name)
        {
            let path = library.classpath_relative_path()?;
            if let Some(filename) =
                path.file_name().and_then(|name| name.to_str())
            {
                filenames.push(filename.to_string());
            }
        }
    }
    if filenames.is_empty() {
        return Ok(());
    }
    let arguments = document
        .as_object_mut()
        .unwrap()
        .entry("arguments")
        .or_insert_with(|| serde_json::json!({}));
    let jvm = arguments
        .as_object_mut()
        .unwrap()
        .entry("jvm")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .unwrap();
    let mut changed = false;
    for argument in jvm.iter_mut() {
        changed |= extend_bootstrap_ignore_argument(argument, &filenames);
    }
    if !changed {
        let mut argument =
            Value::String("-DignoreList=asm,securejarhandler".to_string());
        extend_bootstrap_ignore_argument(&mut argument, &filenames);
        jvm.push(argument);
    }
    Ok(())
}

pub(crate) fn project_manifest(
    info: &VersionInfo,
    version_name: &str,
    game_version: &str,
) -> crate::Result<PortableVersion> {
    let generated = generated_fml_runtime_libraries(info)?;
    let mut libraries = info
        .libraries
        .iter()
        .filter(|library| {
            !library
                .name
                .starts_with("com.axolotl.loader-installer:embedded:")
                && (library.include_in_classpath
                    || library.natives.is_some()
                    || is_native_only_library(library)
                    || (uses_fml_runtime(info)
                        && is_fml_runtime_library(library)))
        })
        .map(|library| LinkedLibrary {
            library: {
                let mut runtime_library = library.clone();
                if uses_fml_runtime(info) && is_fml_runtime_library(library) {
                    runtime_library.include_in_classpath = true;
                }
                runtime_library
            },
            hint: None,
            filename: None,
        })
        .collect::<Vec<_>>();
    for generated_library in &generated {
        if let Some(existing) = libraries
            .iter_mut()
            .find(|library| library.library.name == generated_library.name)
        {
            existing.library = generated_library.clone();
        } else {
            libraries.push(LinkedLibrary {
                library: generated_library.clone(),
                hint: None,
                filename: None,
            });
        }
    }
    let mut libraries = libraries
        .iter()
        .flat_map(expand_native_artifact_library)
        .collect::<Vec<_>>();
    order_duplicate_libraries_for_portable_launch(&mut libraries);
    let mut document = serde_json::to_value(info)?;
    document["id"] = version_name.into();
    document["clientVersion"] = game_version.into();
    if uses_fml_runtime(info) {
        exclude_fml_game_modules_from_bootstrap(
            &mut document,
            &libraries,
            &generated,
        )?;
    }
    let object = document
        .as_object_mut()
        .expect("VersionInfo serializes as an object");
    object.remove("data");
    object.remove("processors");
    let portable_libraries = libraries
        .iter()
        .map(|library| {
            let mut value = serde_json::to_value(&library.library)?;
            let object = value
                .as_object_mut()
                .expect("Library serializes as an object");
            object.remove("include_in_classpath");
            object.remove("downloadable");
            if !library.library.include_in_classpath
                && let Some(downloads) =
                    object.get_mut("downloads").and_then(Value::as_object_mut)
            {
                downloads.remove("artifact");
            }
            if let Some(classifiers) = object
                .get_mut("downloads")
                .and_then(Value::as_object_mut)
                .and_then(|downloads| {
                    downloads
                        .get_mut("classifiers")
                        .and_then(Value::as_object_mut)
                })
            {
                classifiers.retain(|_, classifier| {
                    classifier
                        .get("path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| !path.is_empty())
                });
                if classifiers.is_empty() {
                    object
                        .get_mut("downloads")
                        .and_then(Value::as_object_mut)
                        .map(|downloads| downloads.remove("classifiers"));
                }
            }
            Ok(value)
        })
        .collect::<crate::Result<Vec<_>>>()?;
    document["libraries"] = portable_libraries.into();
    Ok(PortableVersion {
        document,
        libraries,
    })
}

fn check_cancellation(cancellation: &CancellationToken) -> crate::Result<()> {
    if cancellation.is_cancelled() {
        return Err(crate::ErrorKind::LauncherError(
            "External version installation canceled".to_string(),
        )
        .into());
    }
    Ok(())
}

fn applicable(
    library: &LinkedLibrary,
    java_arch: &str,
    minecraft_updated: bool,
) -> bool {
    library.library.rules.as_deref().is_none_or(|rules| {
        super::parse_rules(
            rules,
            java_arch,
            &QuickPlayType::None,
            minecraft_updated,
        )
    })
}

async fn copy_runtime_artifact(
    state: &State,
    source: &Path,
    plan: &LinkedFilePlan,
    required: bool,
) -> crate::Result<()> {
    if !source.is_file() {
        if required {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Missing generated runtime artifact {} at {} (external destination {})",
                plan.label, source.display(), plan.destination.display()
            )).into());
        }
        return Ok(());
    }
    if let Err(error) =
        fetch::validate_file_content(source, plan.validation).await
    {
        if required {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Invalid required runtime artifact {} at {}: {error}",
                plan.label,
                source.display()
            ))
            .into());
        }
        return Ok(());
    }
    let sha1 = match &plan.sha1 {
        Some(sha1) => sha1.clone(),
        None => fetch::sha1_file_async(source).await?.1,
    };
    if direct_ensure::file_is_current(&plan.destination, Some(&sha1), plan.size)
        .await
    {
        return Ok(());
    }
    let copied = super::local_artifact::copy_verified(
        source,
        &plan.destination,
        Some(&sha1),
        plan.size,
        &state.io_semaphore,
    )
    .await
    .map_err(|error| {
        crate::ErrorKind::LauncherError(format!(
            "Could not copy runtime artifact {} from {} to {}: {error}",
            plan.label,
            source.display(),
            plan.destination.display()
        ))
    })?;
    if !copied && required {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Invalid generated runtime artifact {} at {} (external destination {})",
            plan.label, source.display(), plan.destination.display()
        )).into());
    }
    Ok(())
}

async fn copy_runtime_libraries(
    state: &State,
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    java_arch: &str,
    minecraft_updated: bool,
    cancellation: &CancellationToken,
) -> crate::Result<()> {
    let cache = state.directories.libraries_dir();
    for library in libraries {
        check_cancellation(cancellation)?;
        if !applicable(library, java_arch, minecraft_updated) {
            continue;
        }
        let lib = &library.library;
        let mut plans = Vec::new();
        if lib.include_in_classpath
            && let Some(mut plan) =
                direct_ensure::linked_classpath_plan(direct, library)?
        {
            if !lib.downloadable
                && plan.destination.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("jar")
                })
            {
                plan.validation = fetch::ContentValidation::Jar;
            }
            let relative = library.classpath_relative_path()?;
            let declared_source = cache.join(&relative);
            let source = if declared_source.is_file() {
                declared_source
            } else {
                cache.join(daedalus::get_path_from_artifact(&lib.name)?)
            };
            plans.push((source, plan, !lib.downloadable));
        }
        if let Some(plan) =
            direct_ensure::linked_native_plan(direct, library, java_arch)?
        {
            let relative = plan
                .destination
                .strip_prefix(direct.libraries_dir())
                .map_err(|error| {
                    crate::ErrorKind::LauncherError(format!(
                        "Invalid external native path: {error}"
                    ))
                })?
                .to_path_buf();
            let mut source = cache.join(&relative);
            if !source.is_file()
                && let Some(sha1) = plan.sha1.as_deref().filter(|sha1| {
                    sha1.len() == 40
                        && sha1.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            {
                let cached_native = state
                    .directories
                    .caches_dir()
                    .join("minecraft-natives")
                    .join(format!("{sha1}.jar"));
                if cached_native.is_file() {
                    source = cached_native;
                }
            }
            plans.push((source, plan, !lib.downloadable));
        }
        if let Some(classifiers) = lib
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.as_ref())
        {
            for (classifier, download) in classifiers {
                if !classifier.starts_with("native")
                    && !lib.natives.as_ref().is_some_and(|natives| {
                        natives.values().any(|pattern| {
                            ["32", "64"].iter().any(|width| {
                                pattern.replace("${arch}", width) == *classifier
                            })
                        })
                    })
                {
                    continue;
                }
                let relative = download.path.clone().map_or_else(
                    || classified_artifact_path(&lib.name, classifier),
                    Ok,
                )?;
                if !direct_ensure::safe_maven_relative_path(&relative) {
                    return Err(crate::ErrorKind::LauncherError(format!(
                        "Refusing unsafe classifier path {relative:?} for {}",
                        lib.name
                    ))
                    .into());
                }
                let destination = direct.libraries_dir().join(&relative);
                if plans
                    .iter()
                    .any(|(_, plan, _)| plan.destination == destination)
                {
                    continue;
                }
                plans.push((
                    cache.join(&relative),
                    LinkedFilePlan {
                        label: format!("{}:{classifier}", lib.name),
                        urls: Vec::new(),
                        destination,
                        sha1: (!download.sha1.trim().is_empty())
                            .then(|| download.sha1.clone()),
                        size: (download.size > 0)
                            .then_some(download.size as u64),
                        validation: fetch::ContentValidation::Jar,
                    },
                    false,
                ));
            }
        }
        for (source, plan, required) in plans {
            check_cancellation(cancellation)?;
            copy_runtime_artifact(state, &source, &plan, required).await?;
        }
    }
    Ok(())
}

pub(crate) async fn finalize_external_version(
    instance: &Instance,
    version_id: &str,
    game_version: &str,
    info: &VersionInfo,
    state: &State,
    java_arch: &str,
    minecraft_updated: bool,
    progress: Option<&MinecraftDownloadProgress>,
    cancellation: &CancellationToken,
) -> crate::Result<()> {
    let runtime =
        InstanceRuntimeAdapter::for_instance(instance, &state.directories)?;
    let Some(direct) = runtime.direct_link() else {
        return Ok(());
    };
    finalize_version(
        direct,
        version_id,
        game_version,
        info,
        state,
        java_arch,
        minecraft_updated,
        progress,
        cancellation,
    )
    .await
}

async fn finalize_version(
    direct: &DirectLinkedLaunch,
    version_id: &str,
    game_version: &str,
    info: &VersionInfo,
    state: &State,
    java_arch: &str,
    minecraft_updated: bool,
    progress: Option<&MinecraftDownloadProgress>,
    cancellation: &CancellationToken,
) -> crate::Result<()> {
    check_cancellation(cancellation)?;
    let portable = project_manifest(info, &direct.version_id, game_version)?;
    copy_runtime_libraries(
        state,
        direct,
        &portable.libraries,
        java_arch,
        minecraft_updated,
        cancellation,
    )
    .await?;
    let client = info.downloads.get(&DownloadType::Client);
    let source = state
        .directories
        .version_dir(version_id)
        .join(format!("{version_id}.jar"));
    let plan = LinkedFilePlan {
        label: format!("Minecraft client {game_version}"),
        urls: Vec::new(),
        destination: direct
            .version_dir()
            .join(format!("{}.jar", direct.version_id)),
        sha1: client
            .filter(|client| !client.sha1.is_empty())
            .map(|client| client.sha1.clone()),
        size: client
            .filter(|client| client.size > 0)
            .map(|client| client.size as u64),
        validation: fetch::ContentValidation::Jar,
    };
    check_cancellation(cancellation)?;
    copy_runtime_artifact(state, &source, &plan, true).await?;
    direct_ensure::ensure_direct_launch_dependencies_with_progress(
        state,
        direct,
        &portable.libraries,
        info,
        java_arch,
        minecraft_updated,
        progress,
    )
    .await?;
    for library in &portable.libraries {
        if applicable(library, java_arch, minecraft_updated) {
            direct_ensure::validate_local_runtime_library(
                direct, library, java_arch,
            )
            .await?;
        }
    }
    check_cancellation(cancellation)?;
    let json = direct
        .version_dir()
        .join(format!("{}.json", direct.version_id));
    let lock = fetch::destination_download_lock(&json);
    let _guard = lock.lock().await;
    check_cancellation(cancellation)?;
    io::write(json, serde_json::to_vec(&portable.document)?).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::{ExternalGameDirMode, LinkedLauncherDialect};
    use crate::state::{DirectoryInfo, test_state};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn version_info() -> VersionInfo {
        serde_json::from_value(json!({
            "id": "1.20.1-forge", "assets": "1.20",
            "assetIndex": {"id": "", "sha1": "", "size": 0, "totalSize": 0, "url": ""},
            "downloads": {}, "libraries": [],
            "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
            "arguments": {"game": ["--fml.mcVersion", "1.20.1"]},
            "minimumLauncherVersion": 21,
            "releaseTime": "2023-06-12T13:25:51Z", "time": "2023-06-13T11:08:00Z",
            "type": "release",
            "data": {"BINPATCH": {"client": "private-cache-path", "server": ""}},
            "processors": [{"jar": "processor:tool:1", "classpath": [], "args": []}]
        })).unwrap()
    }

    fn runtime_libraries() -> Vec<daedalus::minecraft::Library> {
        serde_json::from_value(json!([
            {"name": "net.minecraftforge:forge:1.20.1-47.4.0:client", "downloadable": false,
                "downloads": {"artifact": {"path": "custom/forge-client.jar", "sha1": "", "size": 0, "url": ""}}},
            {"name": "optifine:OptiFine:1.20.1_HD_U_I6", "downloadable": false},
            {"name": "example:runtime:1.0"},
            {"name": "processor:tool:1", "include_in_classpath": false},
            {"name": "com.axolotl.loader-installer:embedded:1:client@lzma", "include_in_classpath": false, "downloadable": false}
        ])).unwrap()
    }

    fn fml_version_info(neoforge: bool) -> VersionInfo {
        let mut info = version_info();
        let (game, loader, patched, extra, universal) = if neoforge {
            (
                "1.21.1",
                "21.1.251",
                "net.neoforged:neoforge:21.1.251:client",
                "net.minecraft:client:1.21.1-20240808.144430:extra",
                "net.neoforged:neoforge:21.1.251:universal",
            )
        } else {
            (
                "1.20.1",
                "47.4.20",
                "net.minecraftforge:forge:1.20.1-47.4.20:client",
                "net.minecraft:client:1.20.1-20230612.114412:extra",
                "net.minecraftforge:forge:1.20.1-47.4.20:universal",
            )
        };
        info.id = format!("{game}-{loader}");
        info.arguments.as_mut().unwrap().insert(
            daedalus::minecraft::ArgumentType::Jvm,
            vec![daedalus::minecraft::Argument::Normal(
                "-DignoreList=bootstraplauncher,asm".to_string(),
            )],
        );
        info.libraries = serde_json::from_value(json!([
            {"name": "cpw.mods:bootstraplauncher:1.1.2"},
            {"name": universal, "include_in_classpath": false},
            {"name": "net.minecraftforge:installertools:1.3.0", "include_in_classpath": false},
            {"name": "net.minecraftforge:ForgeAutoRenamingTool:0.1.22:all", "include_in_classpath": false}
        ])).unwrap();
        if !neoforge {
            for module in [
                "fmlcore",
                "javafmllanguage",
                "lowcodelanguage",
                "mclanguage",
            ] {
                info.libraries.push(serde_json::from_value(json!({
                    "name": format!("net.minecraftforge:{module}:1.20.1-47.4.20"),
                    "include_in_classpath": false
                })).unwrap());
            }
        }
        let srg = format!("{}:srg", extra.strip_suffix(":extra").unwrap());
        for (key, coordinate) in [
            ("PATCHED", patched),
            ("MC_EXTRA", extra),
            ("MC_SRG", srg.as_str()),
            (
                "MC_SLIM",
                "net.minecraft:client:1.20.1-20230612.114412:slim",
            ),
        ] {
            info.data.as_mut().unwrap().insert(
                key.to_string(),
                daedalus::modded::SidedDataEntry {
                    client: format!("[{coordinate}]"),
                    server: "[example:server-only:1]".to_string(),
                },
            );
        }
        info
    }

    async fn fixture() -> (TempDir, Arc<State>, DirectLinkedLaunch) {
        let temp = TempDir::new().unwrap();
        let dirs = DirectoryInfo {
            settings_dir: temp.path().join("settings"),
            config_dir: temp.path().join("config"),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(dirs.instances_dir()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        let state = test_state(dirs, pool).await.unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: temp.path().join(".minecraft"),
            launcher_root: None,
            version_id: "Survival".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::PclCe,
            game_dir_mode: Some(ExternalGameDirMode::Isolated),
        };
        (temp, state, direct)
    }

    fn write(path: impl AsRef<Path>, bytes: &[u8]) {
        let path = path.as_ref();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn jar_bytes(payload: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file("fixture", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(payload).unwrap();
        archive.finish().unwrap().into_inner()
    }

    fn seed_cache(state: &State, info: &VersionInfo) {
        write(
            state
                .directories
                .version_dir(&info.id)
                .join(format!("{}.jar", info.id)),
            &jar_bytes(b"client jar"),
        );
        for library in project_manifest(info, "Survival", "1.20.1")
            .unwrap()
            .libraries
            .iter()
            .filter(|library| library.library.include_in_classpath)
        {
            write(
                state.directories.libraries_dir().join(
                    daedalus::get_path_from_artifact(&library.library.name)
                        .unwrap(),
                ),
                &jar_bytes(library.library.name.as_bytes()),
            );
        }
    }

    async fn finalize(
        state: &State,
        direct: &DirectLinkedLaunch,
        info: &VersionInfo,
        token: &CancellationToken,
    ) -> crate::Result<()> {
        finalize_version(
            direct,
            &info.id,
            "1.20.1",
            info,
            state,
            std::env::consts::ARCH,
            true,
            None,
            token,
        )
        .await
    }

    #[test]
    fn projection_keeps_portable_runtime_and_native_metadata() {
        let mut info = version_info();
        info.libraries = runtime_libraries();
        info.libraries.push(serde_json::from_value(json!({
            "name": "example:natives:1", "include_in_classpath": false,
            "natives": {"windows": "natives-windows"},
            "extract": {"exclude": ["META-INF/"]},
            "rules": [{"action": "allow", "os": {"name": "windows"}}],
            "downloads": {
                "artifact": {"path": "processor-only.jar", "sha1": "", "size": 0, "url": ""},
                "classifiers": {"natives-windows": {"path": "native.jar", "sha1": "", "size": 0, "url": ""}}
            }
        })).unwrap());
        let before = serde_json::to_value(&info).unwrap();
        let portable = project_manifest(&info, "Survival", "24w14a").unwrap();
        let document = &portable.document;
        assert_eq!(document["id"], "Survival");
        assert_eq!(document["clientVersion"], "24w14a");
        assert_eq!(document["releaseTime"], before["releaseTime"]);
        assert_eq!(document["time"], before["time"]);
        assert_eq!(document["arguments"], before["arguments"]);
        assert_eq!(document["mainClass"], before["mainClass"]);
        assert!(document.get("data").is_none());
        assert!(document.get("processors").is_none());
        let libraries = document["libraries"].as_array().unwrap();
        assert_eq!(libraries.len(), 4);
        for library in libraries {
            assert!(library.get("include_in_classpath").is_none());
            assert!(library.get("downloadable").is_none());
        }
        assert!(!portable.libraries[0].library.downloadable);
        assert!(!portable.libraries[3].library.include_in_classpath);
        assert!(libraries[3]["downloads"].get("artifact").is_none());
        assert_eq!(libraries[3]["natives"]["windows"], "natives-windows");
        assert_eq!(
            libraries[3]["downloads"]["classifiers"]["natives-windows"]["path"],
            "native.jar"
        );
        assert_eq!(libraries[3]["extract"]["exclude"][0], "META-INF/");
        assert_eq!(libraries[3]["rules"][0]["os"]["name"], "windows");
        assert_eq!(serde_json::to_value(&info).unwrap(), before);
    }

    #[test]
    fn projection_splits_classpath_artifact_from_native_metadata() {
        let mut info = version_info();
        info.libraries = serde_json::from_value(json!([{
            "name": "com.mojang:text2speech:1.10.3",
            "natives": {"windows": "natives-windows"},
            "extract": {"exclude": ["META-INF/"]},
            "downloads": {
                "artifact": {"path": "com/mojang/text2speech/1.10.3/text2speech-1.10.3.jar", "sha1": "main", "size": 1, "url": "https://libraries.minecraft.net/main.jar"},
                "classifiers": {"natives-windows": {"path": "com/mojang/text2speech/1.10.3/text2speech-1.10.3-natives-windows.jar", "sha1": "native", "size": 1, "url": "https://libraries.minecraft.net/native.jar"}}
            }
        }])).unwrap();

        let portable = project_manifest(&info, "Legacy", "1.12.2").unwrap();
        let libraries = portable.document["libraries"].as_array().unwrap();
        assert_eq!(libraries.len(), 2);
        assert!(libraries[0]["downloads"].get("artifact").is_some());
        assert!(libraries[0].get("natives").is_none());
        assert!(libraries[0]["downloads"].get("classifiers").is_none());
        assert!(libraries[1].get("natives").is_some());
        assert!(libraries[1]["downloads"].get("classifiers").is_some());
        assert_eq!(portable.libraries.len(), 2);
    }

    #[test]
    fn projection_keeps_duplicate_library_positions_stable() {
        let mut info = version_info();
        info.libraries = serde_json::from_value(json!([
            {"name": "first:library:1"},
            {"name": "com.mojang:text2speech:1.11.3", "rules": [{"action": "allow", "os": {"name": "osx-arm64"}}]},
            {"name": "com.mojang:text2speech:1.10.3"},
            {"name": "last:library:1"}
        ])).unwrap();

        let portable = project_manifest(&info, "Legacy", "1.12.2").unwrap();
        let names = portable
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "first:library:1",
                "com.mojang:text2speech:1.10.3",
                "com.mojang:text2speech:1.11.3",
                "last:library:1"
            ]
        );
    }

    #[test]
    fn portable_projection_puts_host_applicable_duplicate_library_first() {
        let mut info = version_info();
        info.libraries = serde_json::from_value(json!([
            {
                "name": "com.mojang:text2speech:1.11.3",
                "rules": [{"action": "allow", "os": {"name": "osx-arm64"}}],
                "downloads": {"artifact": {"path": "com/mojang/text2speech/1.11.3/text2speech-1.11.3.jar", "sha1": "", "size": 0, "url": ""}}
            },
            {
                "name": "com.mojang:text2speech:1.10.3",
                "downloads": {"artifact": {"path": "com/mojang/text2speech/1.10.3/text2speech-1.10.3.jar", "sha1": "", "size": 0, "url": ""}}
            }
        ])).unwrap();
        let portable = project_manifest(&info, "Legacy", "1.12.2").unwrap();
        let names = portable
            .libraries
            .iter()
            .map(|library| library.library.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "com.mojang:text2speech:1.10.3",
                "com.mojang:text2speech:1.11.3"
            ]
        );
    }

    #[tokio::test]
    async fn native_classifier_is_published_from_the_internal_native_cache() {
        let (_temp, state, direct) = fixture().await;
        // Native classifiers are resolved for the host OS, so the fixture has
        // to name the classifier this test binary will actually look up.
        let classifier = if cfg!(windows) {
            "natives-windows"
        } else if cfg!(target_os = "macos") {
            "natives-osx"
        } else {
            "natives-linux"
        };
        let os_key = classifier.trim_start_matches("natives-");
        let native_bytes = jar_bytes(b"narrator native");
        let native_sha1 = sha1_smol::Sha1::from(&native_bytes[..]).hexdigest();
        let native_cache = state
            .directories
            .caches_dir()
            .join("minecraft-natives")
            .join(format!("{native_sha1}.jar"));
        write(&native_cache, &native_bytes);
        let path = format!(
            "com/mojang/text2speech/1.10.3/text2speech-1.10.3-{classifier}.jar"
        );
        let mut info = version_info();
        info.libraries = serde_json::from_value(json!([{
            "name": "com.mojang:text2speech:1.10.3",
            "natives": {os_key: classifier},
            "downloads": {
                "artifact": {"path": "com/mojang/text2speech/1.10.3/text2speech-1.10.3.jar", "sha1": "", "size": 0, "url": ""},
                "classifiers": {classifier: {"path": path, "sha1": native_sha1, "size": native_bytes.len(), "url": format!("https://libraries.minecraft.net/{path}")}}
            }
        }])).unwrap();
        seed_cache(&state, &info);
        finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(direct.libraries_dir().join(path)).unwrap(),
            native_bytes
        );
    }

    #[tokio::test]
    async fn generated_forge_and_optifine_artifacts_resolve_after_idempotent_repair()
     {
        let (_temp, state, mut direct) = fixture().await;
        let mut info = version_info();
        info.libraries = runtime_libraries();
        seed_cache(&state, &info);
        write(
            direct.libraries_dir().join("custom/forge-client.jar"),
            b"obsolete forge",
        );
        write(
            direct.libraries_dir().join("unrelated/shared.jar"),
            b"unrelated",
        );
        write(direct.version_dir().join("saves/world/level.dat"), b"world");
        for mode in [ExternalGameDirMode::Isolated, ExternalGameDirMode::Shared]
        {
            direct.game_dir_mode = Some(mode);
            for _ in 0..2 {
                finalize(&state, &direct, &info, &CancellationToken::new())
                    .await
                    .unwrap();
                let resolved = direct.resolve().unwrap();
                let classpath = super::super::args::get_linked_class_paths(
                    &direct,
                    &resolved.merged.libraries,
                    &[&direct.client_jar("Survival")],
                    std::env::consts::ARCH,
                    true,
                )
                .unwrap();
                assert!(classpath.contains("forge-client.jar"), "{classpath}");
                assert!(
                    classpath.contains("OptiFine-1.20.1_HD_U_I6.jar"),
                    "{classpath}"
                );
                assert!(!classpath.contains("processor"), "{classpath}");
                assert_eq!(
                    std::fs::read(
                        direct.libraries_dir().join("custom/forge-client.jar")
                    )
                    .unwrap(),
                    jar_bytes(info.libraries[0].name.as_bytes())
                );
                assert_eq!(
                    std::fs::read(direct.client_jar("Survival")).unwrap(),
                    jar_bytes(b"client jar")
                );
            }
        }
        assert_eq!(
            std::fs::read(direct.libraries_dir().join("unrelated/shared.jar"))
                .unwrap(),
            b"unrelated"
        );
        assert_eq!(
            std::fs::read(direct.version_dir().join("saves/world/level.dat"))
                .unwrap(),
            b"world"
        );
        assert!(!direct.libraries_dir().join("processor").exists());
    }

    #[tokio::test]
    async fn fml_runtime_modules_and_generated_outputs_are_exported_for_forge_and_neoforge()
     {
        let (_temp, state, direct) = fixture().await;
        for neoforge in [false, true] {
            let info = fml_version_info(neoforge);
            let game_version = if neoforge { "1.21.1" } else { "1.20.1" };
            let before = serde_json::to_value(&info).unwrap();
            let projected =
                project_manifest(&info, "Survival", game_version).unwrap();
            assert_eq!(projected.libraries.len(), if neoforge { 5 } else { 9 });
            assert!(
                projected
                    .libraries
                    .iter()
                    .all(|library| library.library.include_in_classpath)
            );
            assert!(projected.libraries.iter().all(|library| {
                !library.library.name.contains("installertools")
                    && !library.library.name.contains("AutoRenamingTool")
            }));
            let ignored =
                projected.document["arguments"]["jvm"][0].as_str().unwrap();
            assert!(ignored.starts_with("-DignoreList=bootstraplauncher,asm,"));
            for key in ["PATCHED", "MC_SRG", "MC_EXTRA"] {
                let coordinate = info.data.as_ref().unwrap()[key]
                    .client
                    .trim_start_matches('[')
                    .trim_end_matches(']');
                let library = projected
                    .libraries
                    .iter()
                    .find(|library| library.library.name == coordinate)
                    .unwrap();
                assert!(!library.library.downloadable);
                let filename = library
                    .classpath_relative_path()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                assert!(ignored.contains(&filename), "{ignored}");
            }
            seed_cache(&state, &info);
            finalize_version(
                &direct,
                &info.id,
                game_version,
                &info,
                &state,
                std::env::consts::ARCH,
                true,
                None,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            for library in &projected.libraries {
                assert!(
                    direct.library_path(library).unwrap().is_file(),
                    "{}",
                    library.library.name
                );
            }
            let resolved = direct.resolve().unwrap();
            let classpath = super::super::args::get_linked_class_paths(
                &direct,
                &resolved.merged.libraries,
                &[&direct.client_jar("Survival")],
                std::env::consts::ARCH,
                true,
            )
            .unwrap();
            assert!(classpath.contains("-client.jar"), "{classpath}");
            assert!(classpath.contains("-extra.jar"), "{classpath}");
            assert!(classpath.contains("-srg.jar"), "{classpath}");
            assert!(classpath.contains("-universal.jar"), "{classpath}");
            assert!(
                !classpath.contains("-slim.jar")
                    && !classpath.contains("AutoRenamingTool")
                    && !classpath.contains("installertools"),
                "{classpath}"
            );
            assert_eq!(serde_json::to_value(&info).unwrap(), before);
        }
    }

    #[tokio::test]
    async fn missing_fml_output_absent_from_the_classpath_blocks_publication() {
        let (_temp, state, direct) = fixture().await;
        let info = fml_version_info(false);
        seed_cache(&state, &info);
        let patched = "net.minecraftforge:forge:1.20.1-47.4.20:client";
        std::fs::remove_file(
            state
                .directories
                .libraries_dir()
                .join(daedalus::get_path_from_artifact(patched).unwrap()),
        )
        .unwrap();
        let json = direct.version_dir().join("Survival.json");
        write(&json, b"previous manifest");
        let error = finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(patched), "{error}");
        assert_eq!(std::fs::read(json).unwrap(), b"previous manifest");
    }

    #[test]
    fn fml_generated_output_paths_are_validated_before_projection() {
        let mut info = fml_version_info(false);
        for coordinate in [
            "[example:../../escape:1]",
            "[com.axolotl.loader-installer:embedded:1:patched@jar]",
        ] {
            info.data
                .as_mut()
                .unwrap()
                .get_mut("PATCHED")
                .unwrap()
                .client = coordinate.to_string();
            assert!(
                project_manifest(&info, "Survival", "1.20.1")
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("Invalid FML client runtime artifact")
            );
        }
    }

    #[test]
    fn modern_forge_bootstrap_launcher_projects_generated_runtime_artifacts() {
        let mut info = fml_version_info(false);
        info.main_class =
            "net.minecraftforge.bootstrap.BootstrapLauncher".to_string();
        let portable = project_manifest(&info, "Survival", "1.20.2").unwrap();
        for key in ["PATCHED", "MC_SRG", "MC_EXTRA"] {
            let coordinate = info.data.as_ref().unwrap()[key]
                .client
                .trim_start_matches('[')
                .trim_end_matches(']');
            assert!(
                portable
                    .libraries
                    .iter()
                    .any(|library| library.library.name == coordinate)
            );
        }
        assert!(
            portable.document["arguments"]["jvm"]
                .as_array()
                .unwrap()
                .iter()
                .any(|argument| {
                    argument.as_str().is_some_and(|argument| {
                        argument
                            .contains("client-1.20.1-20230612.114412-srg.jar")
                    })
                })
        );
    }

    #[tokio::test]
    async fn missing_generated_output_preserves_previous_manifest() {
        let (_temp, state, direct) = fixture().await;
        let mut info = version_info();
        info.libraries = runtime_libraries();
        let json = direct.version_dir().join("Survival.json");
        write(&json, b"previous manifest");
        let error = finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(&info.libraries[0].name), "{error}");
        assert!(error.contains("custom"), "{error}");
        assert_eq!(std::fs::read(json).unwrap(), b"previous manifest");
        assert!(!direct.client_jar("Survival").exists());
    }

    #[tokio::test]
    async fn copy_failure_preserves_previous_manifest() {
        let (_temp, state, direct) = fixture().await;
        let mut info = version_info();
        info.libraries = runtime_libraries();
        seed_cache(&state, &info);
        let json = direct.version_dir().join("Survival.json");
        write(&json, b"previous manifest");
        write(direct.libraries_dir(), b"not a directory");
        let error = finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("forge"), "{error}");
        assert_eq!(std::fs::read(json).unwrap(), b"previous manifest");
    }

    #[tokio::test]
    async fn corrupt_generated_jar_blocks_publication_even_without_a_checksum()
    {
        let (_temp, state, direct) = fixture().await;
        let mut info = version_info();
        info.libraries = runtime_libraries();
        seed_cache(&state, &info);
        let source = state.directories.libraries_dir().join(
            daedalus::get_path_from_artifact(&info.libraries[0].name).unwrap(),
        );
        write(&source, b"interrupted patch output");
        let json = direct.version_dir().join("Survival.json");
        write(&json, b"previous manifest");
        let error = finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(&info.libraries[0].name), "{error}");
        assert!(error.contains("Invalid"), "{error}");
        assert_eq!(std::fs::read(json).unwrap(), b"previous manifest");
        assert!(
            !direct
                .libraries_dir()
                .join("custom/forge-client.jar")
                .exists()
        );
    }

    #[tokio::test]
    async fn cancellation_while_waiting_to_publish_preserves_previous_manifest()
    {
        let (_temp, state, direct) = fixture().await;
        let info = version_info();
        seed_cache(&state, &info);
        let json = direct.version_dir().join("Survival.json");
        write(&json, b"previous manifest");
        let lock = fetch::destination_download_lock(&json);
        let guard = lock.lock().await;
        let token = CancellationToken::new();
        let finalization = finalize(&state, &direct, &info, &token);
        let cancel = async {
            for _ in 0..100 {
                if direct.client_jar("Survival").is_file() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            token.cancel();
            drop(guard);
        };
        let (result, ()) = tokio::join!(finalization, cancel);
        assert!(result.unwrap_err().to_string().contains("canceled"));
        assert_eq!(std::fs::read(json).unwrap(), b"previous manifest");
    }

    #[tokio::test]
    async fn required_native_is_copied_and_missing_native_blocks_publication() {
        let (_temp, state, direct) = fixture().await;
        let mut info = version_info();
        let os =
            serde_json::to_value(daedalus::minecraft::Os::native().get_os())
                .unwrap();
        let classifier = format!("natives-{}", os.as_str().unwrap());
        info.libraries = serde_json::from_value(json!([{
            "name": "example:natives:1", "include_in_classpath": false, "downloadable": false,
            "natives": {os.as_str().unwrap(): classifier},
            "downloads": {"classifiers": {
                classifier: {"path": "native/current.jar", "sha1": "", "size": 0, "url": ""},
                "natives-other": {"path": "native/other.jar", "sha1": "", "size": 0, "url": ""}
            }}
        }])).unwrap();
        seed_cache(&state, &info);
        let error = finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("native/current.jar")
                || error.contains("native\\current.jar"),
            "{error}"
        );
        assert!(!direct.version_dir().join("Survival.json").exists());
        write(
            state.directories.libraries_dir().join("native/current.jar"),
            &jar_bytes(b"current native"),
        );
        write(
            state.directories.libraries_dir().join("native/other.jar"),
            &jar_bytes(b"other native"),
        );
        finalize(&state, &direct, &info, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(direct.libraries_dir().join("native/current.jar"))
                .unwrap(),
            jar_bytes(b"current native")
        );
        assert_eq!(
            std::fs::read(direct.libraries_dir().join("native/other.jar"))
                .unwrap(),
            jar_bytes(b"other native")
        );
        let resolved = direct.resolve().unwrap();
        let classpath = super::super::args::get_linked_class_paths(
            &direct,
            &resolved.merged.libraries,
            &[&direct.client_jar("Survival")],
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        assert!(!classpath.contains("native"), "{classpath}");
    }
}
