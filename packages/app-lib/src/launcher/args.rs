//! Minecraft CLI argument logic
use crate::instance::QuickPlayType;
use crate::launcher::direct_link::{
    DirectLinkedLaunch, is_native_only_library,
};
use crate::launcher::local_version::LinkedLibrary;
use crate::launcher::pcl_ce_command;
use crate::launcher::quick_play_version::{
    QuickPlayServerVersion, QuickPlaySingleplayerVersion,
};
use crate::launcher::{QuickPlayVersion, parse_rules};
use crate::state::Credentials;
use crate::{
    state::{MemorySettings, WindowSize},
    util::{
        io::IOError, mojang::add_fallen_discovery_java_arg,
        platform::classpath_separator,
    },
};
use daedalus::minecraft::LoggingConfiguration;
use daedalus::{
    get_path_from_artifact,
    minecraft::{Argument, ArgumentValue, Library, VersionType},
    modded::SidedDataEntry,
};
use dunce::canonicalize;
use itertools::Itertools;
use std::io::{BufRead, BufReader, ErrorKind};
use std::net::SocketAddr;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use uuid::Uuid;

// Replaces the space separator with a newline character, as to not split the arguments
const TEMPORARY_REPLACE_CHAR: &str = "\n";

pub fn get_class_paths(
    libraries_path: &Path,
    libraries: &[Library],
    launcher_class_path: &[&Path],
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<String> {
    // A version manifest can contain platform-specific revisions of the same
    // Maven module (for example LWJGL 3.2.1 for macOS and 3.2.2 elsewhere).
    // Rules normally leave only one revision enabled. Keep the last enabled
    // revision as a safety net for stale or partially merged metadata; putting
    // both JARs on the classpath lets the older one shadow the native ABI.
    let mut seen_artifacts = HashSet::new();
    let libraries = libraries
        .iter()
        .rev()
        .filter(|library| {
            if let Some(rules) = &library.rules
                && !parse_rules(
                    rules,
                    java_arch,
                    &QuickPlayType::None,
                    minecraft_updated,
                )
            {
                return false;
            }
            if !library.include_in_classpath {
                return false;
            }
            // Modern Mojang manifests express native archives as separate
            // classifier coordinates. A classifier must not displace the
            // unclassified JAR that contains the Java classes.
            let mut coordinates = library.name.split(':');
            let group = coordinates.next().unwrap_or_default();
            let artifact = coordinates.next().unwrap_or_default();
            let _version = coordinates.next();
            let classifier = coordinates.next().unwrap_or_default();
            let key = format!("{group}:{artifact}:{classifier}");
            seen_artifacts.insert(key)
        })
        .collect::<Vec<_>>();

    launcher_class_path
        .iter()
        .map(|path| {
            Ok(canonicalize(path)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified class path {} does not exist",
                        path.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy()
                .to_string())
        })
        .chain(libraries.into_iter().rev().map(|library| {
            // A merged library carrying both a Java artifact and native
            // classifiers (e.g. LWJGL) must have its JAR present on disk.
            // Pure native libraries (no Java artifact) remain tolerated
            // when absent, matching the pre-existing classpath behaviour.
            let allow_not_exist =
                crate::launcher::download::is_native_library(library)
                    && !crate::launcher::download::needs_java_artifact(library);
            get_lib_path(libraries_path, &library.name, allow_not_exist)
        }))
        .process_results(|iter| {
            iter.unique().join(classpath_separator(java_arch))
        })
}

pub fn get_linked_class_paths(
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    launcher_class_path: &[&Path],
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<String> {
    launcher_class_path
        .iter()
        .map(|path| {
            Ok(canonicalize(path)
                .map_err(|error| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified class path {} does not exist: {error}",
                        path.display()
                    ))
                    .as_error()
                })?
                .to_string_lossy()
                .to_string())
        })
        .chain(libraries.iter().filter_map(|library| {
            if let Some(rules) = library.library.rules.as_deref()
                && !parse_rules(
                    rules,
                    java_arch,
                    &QuickPlayType::None,
                    minecraft_updated,
                )
            {
                return None;
            }
            if !library.library.include_in_classpath {
                return None;
            }
            // A natives-only declaration (e.g. the 1.12.2
            // `net.java.jinput:jinput-platform:2.0.5`) publishes no plain jar
            // anywhere, so it must never become a classpath entry. This
            // mirrors HMCL's `DefaultLauncher.getClasspath`, which contributes
            // natives libraries only through native extraction; the same
            // predicate keeps the ensure stage from planning the plain jar.
            if is_native_only_library(&library.library) {
                return None;
            }

            Some(direct.library_path(library).and_then(|path| {
                canonicalize(&path)
                    .map(|path| path.to_string_lossy().to_string())
                    .map_err(|error| {
                        crate::ErrorKind::LauncherError(format!(
                            "Could not resolve linked library {} at {}: {error}",
                            library.library.name,
                            path.display()
                        ))
                        .as_error()
                    })
            }))
        }))
        .process_results(|paths| {
            paths.unique().join(classpath_separator(java_arch))
        })
}

pub(super) fn get_pcl_ce_class_paths<'a>(
    direct: &DirectLinkedLaunch,
    libraries: &[LinkedLibrary],
    bridge: &Path,
    client: &Path,
    game_directory: &Path,
    classpath_head: impl Iterator<Item = &'a str>,
    java_arch: &str,
) -> crate::Result<String> {
    let mut ordered = classpath_head
        .map(|entry| {
            let path = game_directory.join(entry);
            canonicalize(&path).unwrap_or(path)
        })
        .collect::<Vec<_>>();
    ordered.reverse();
    let mut optifine = None;
    for library in libraries {
        if !library.library.include_in_classpath
            || is_native_only_library(&library.library)
        {
            continue;
        }
        let path = direct.library_path(library)?;
        let path = canonicalize(&path).map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Could not resolve linked library {} at {}: {error}",
                library.library.name,
                path.display(),
            ))
        })?;
        if library.library.name.starts_with("optifine:OptiFine:") {
            optifine = Some(path);
        } else {
            ordered.push(path);
        }
    }
    let launcher_path = |path: &Path| {
        canonicalize(path).map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Specified class path {} does not exist: {error}",
                path.display(),
            ))
        })
    };
    ordered.push(launcher_path(client)?);
    if let Some(optifine) = optifine {
        ordered.insert(ordered.len().saturating_sub(2), optifine);
    }
    let bridge = launcher_path(bridge)?;
    Ok(std::iter::once(&bridge)
        .chain(ordered.iter())
        .map(|path| path.to_string_lossy())
        .join(classpath_separator(java_arch)))
}

pub fn get_class_paths_jar<T: AsRef<str>>(
    libraries_path: &Path,
    libraries: &[T],
    java_arch: &str,
) -> crate::Result<String> {
    let cps = libraries
        .iter()
        .map(|library| get_lib_path(libraries_path, library.as_ref(), false))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(cps.join(classpath_separator(java_arch)))
}

pub fn get_lib_path(
    libraries_path: &Path,
    lib: &str,
    allow_not_exist: bool,
) -> crate::Result<String> {
    let path = libraries_path.join(get_path_from_artifact(lib)?);

    let path = match canonicalize(&path) {
        Ok(p) => p,
        Err(err) if err.kind() == ErrorKind::NotFound && allow_not_exist => {
            path
        }
        Err(err) => {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Could not canonicalize library path {}: {err}",
                path.display()
            ))
            .as_error());
        }
    };

    Ok(path.to_string_lossy().to_string())
}

#[allow(clippy::too_many_arguments)]
pub fn get_jvm_arguments(
    arguments: Option<&[Argument]>,
    natives_path: &Path,
    libraries_path: &Path,
    log_configs_path: &Path,
    class_paths: &str,
    agent_path: &Path,
    version_name: &str,
    memory: MemorySettings,
    custom_args: Vec<String>,
    java_arch: &str,
    quick_play_type: &QuickPlayType,
    quick_play_version: QuickPlayVersion,
    log_config: Option<&LoggingConfiguration>,
    ipc_addr: SocketAddr,
    pcl_ce_replacements: Option<&[(&str, &str)]>,
) -> crate::Result<Vec<String>> {
    let mut parsed_arguments = Vec::new();

    if let Some(replacements) = pcl_ce_replacements {
        parsed_arguments = pcl_ce_command::jvm_arguments(
            arguments.unwrap_or_default(),
            &custom_args,
            replacements,
        );
    }
    if let Some(args) = arguments {
        if pcl_ce_replacements.is_none() {
            parse_arguments(
                args,
                &mut parsed_arguments,
                |arg| {
                    parse_jvm_argument(
                        arg.to_string(),
                        natives_path,
                        libraries_path,
                        class_paths,
                        version_name,
                        java_arch,
                    )
                },
                java_arch,
                quick_play_type,
            )?;
        }
    } else {
        parsed_arguments.push(format!(
            "-Djava.library.path={}",
            canonicalize(natives_path)
                .map_err(|_| crate::ErrorKind::LauncherError(format!(
                    "Specified natives path {} does not exist",
                    natives_path.to_string_lossy()
                ))
                .as_error())?
                .to_string_lossy()
        ));
        parsed_arguments.push("-cp".to_string());
        parsed_arguments.push(class_paths.to_string());
    }

    parsed_arguments.push(format!("-Xmx{}M", memory.maximum));

    if let Some(LoggingConfiguration::Log4j2Xml { argument, file }) = log_config
    {
        let full_path = log_configs_path.join(&file.id);
        let full_path = full_path.to_string_lossy();
        parsed_arguments.push(argument.replace("${path}", &full_path));
    }

    parsed_arguments.push(format!(
        "-javaagent:{}",
        canonicalize(agent_path)
            .map_err(|_| {
                crate::ErrorKind::LauncherError(format!(
                    "Specified Java Agent path {} does not exist",
                    libraries_path.to_string_lossy()
                ))
                .as_error()
            })?
            .to_string_lossy()
    ));

    parsed_arguments
        .push(format!("-Dmodrinth.internal.ipc.host={}", ipc_addr.ip()));
    parsed_arguments
        .push(format!("-Dmodrinth.internal.ipc.port={}", ipc_addr.port()));

    parsed_arguments.push(format!(
        "-Dmodrinth.internal.quickPlay.serverVersion={}",
        serde_json::to_value(quick_play_version.server)?
            .as_str()
            .unwrap()
    ));
    if let QuickPlayType::Server(server) = quick_play_type
        && quick_play_version.server == QuickPlayServerVersion::Injected
    {
        let (host, port) = server.require_resolved()?;
        parsed_arguments.extend_from_slice(&[
            format!("-Dmodrinth.internal.quickPlay.host={host}"),
            format!("-Dmodrinth.internal.quickPlay.port={port}"),
        ]);
    }

    if pcl_ce_replacements.is_none() {
        parsed_arguments
            .extend(custom_args.into_iter().filter(|arg| !arg.is_empty()));
    }

    add_fallen_discovery_java_arg(&mut parsed_arguments);
    Ok(parsed_arguments)
}

pub(super) struct PclCeArgumentContext<'a> {
    pub(super) version_name: &'a str,
    pub(super) version_type: &'a str,
    pub(super) natives: &'a Path,
    pub(super) libraries: &'a Path,
    pub(super) classpath: &'a str,
    pub(super) game_directory: &'a Path,
    pub(super) assets: &'a Path,
    pub(super) game_assets: &'a Path,
    pub(super) asset_index: &'a str,
    pub(super) resolution: WindowSize,
    pub(super) java_arch: &'a str,
}

impl PclCeArgumentContext<'_> {
    pub(super) async fn replacements(
        &self,
        credentials: &Credentials,
    ) -> Vec<(&'static str, String)> {
        let profile = credentials.maybe_online_profile().await;
        let directory = |path: &Path| {
            canonicalize(path)
                .unwrap_or_else(|_| path.to_path_buf())
                .to_string_lossy()
                .into_owned()
        };
        let libraries = directory(self.libraries);
        vec![
            (
                "${classpath_separator}",
                classpath_separator(self.java_arch).into(),
            ),
            ("${natives_directory}", directory(self.natives)),
            ("${library_directory}", libraries.clone()),
            ("${libraries_directory}", libraries),
            ("${classpath}", self.classpath.into()),
            ("${launcher_name}", "PCLCE".into()),
            ("${launcher_version}", env!("CARGO_PKG_VERSION").into()),
            ("${version_name}", self.version_name.into()),
            ("${version_type}", self.version_type.trim().into()),
            ("${game_directory}", directory(self.game_directory)),
            ("${assets_root}", directory(self.assets)),
            ("${game_assets}", directory(self.game_assets)),
            ("${assets_index_name}", self.asset_index.into()),
            ("${resolution_width}", self.resolution.0.to_string()),
            ("${resolution_height}", self.resolution.1.to_string()),
            ("${auth_player_name}", profile.name.clone()),
            ("${auth_uuid}", profile.id.simple().to_string()),
            ("${auth_access_token}", credentials.access_token.clone()),
            ("${access_token}", credentials.access_token.clone()),
            ("${auth_session}", credentials.access_token.clone()),
            ("${user_type}", "msa".into()),
            ("${user_properties}", "{}".into()),
            ("${accessToken}", credentials.access_token.clone()),
            ("${uuid}", profile.id.simple().to_string()),
            ("${auth_xuid}", "0".into()),
            ("${clientid}", "c4502edb-87c6-40cb-b595-64a280cf8906".into()),
        ]
    }
}

pub(super) fn append_pcl_ce_quick_play(
    arguments: &mut Vec<String>,
    target: &QuickPlayType,
    version: QuickPlayVersion,
) -> crate::Result<()> {
    match target {
        QuickPlayType::Singleplayer(world)
            if version.singleplayer
                == QuickPlaySingleplayerVersion::Builtin =>
        {
            arguments.extend(["--quickPlaySingleplayer".into(), world.clone()]);
        }
        QuickPlayType::Server(server)
            if version.server == QuickPlayServerVersion::Builtin =>
        {
            let address = match server {
                crate::server_address::ServerAddress::Unresolved(address) => {
                    address.clone()
                }
                crate::server_address::ServerAddress::Resolved {
                    original_host,
                    original_port,
                    ..
                } => {
                    if original_host.contains(':') {
                        format!("[{original_host}]:{original_port}")
                    } else {
                        format!("{original_host}:{original_port}")
                    }
                }
            };
            arguments.extend(["--quickPlayMultiplayer".into(), address]);
        }
        QuickPlayType::Server(server)
            if version.server == QuickPlayServerVersion::BuiltinLegacy =>
        {
            let (host, port) = server.require_resolved()?;
            arguments.extend([
                "--server".into(),
                host.to_string(),
                "--port".into(),
                port.to_string(),
            ]);
        }
        _ => {}
    }
    Ok(())
}

fn parse_jvm_argument(
    mut argument: String,
    natives_path: &Path,
    libraries_path: &Path,
    class_paths: &str,
    version_name: &str,
    java_arch: &str,
) -> crate::Result<String> {
    argument.retain(|c| !c.is_whitespace());
    Ok(argument
        .replace(
            "${natives_directory}",
            &canonicalize(natives_path)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified natives path {} does not exist",
                        natives_path.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy(),
        )
        .replace(
            "${library_directory}",
            &canonicalize(libraries_path)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified libraries path {} does not exist",
                        libraries_path.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy(),
        )
        .replace("${classpath_separator}", classpath_separator(java_arch))
        .replace("${launcher_name}", "theseus")
        .replace("${launcher_version}", env!("CARGO_PKG_VERSION"))
        .replace("${version_name}", version_name)
        .replace("${classpath}", class_paths))
}

#[allow(clippy::too_many_arguments)]
pub async fn get_minecraft_arguments(
    arguments: Option<&[Argument]>,
    legacy_arguments: Option<&str>,
    credentials: &Credentials,
    version: &str,
    asset_index_name: &str,
    game_directory: &Path,
    assets_directory: &Path,
    game_assets_directory: &Path,
    version_type: &VersionType,
    resolution: WindowSize,
    java_arch: &str,
    quick_play_type: &QuickPlayType,
    quick_play_version: QuickPlayVersion,
) -> crate::Result<Vec<String>> {
    let access_token = credentials.access_token.clone();
    let user_type = if credentials.is_microsoft() {
        "msa"
    } else {
        "legacy"
    };
    let profile = credentials.maybe_online_profile().await;
    let mut parsed_arguments = Vec::new();

    // Legacy-format loader profiles (Forge 1.6.x era and friends) repeat
    // their `minecraftArguments` verbatim inside `arguments.game`. Emitting
    // both duplicates every option, which older launch wrappers reject
    // (joptsimple throws on repeated options like `--gameDir`), so options
    // already provided by the modern argument list are dropped from the
    // legacy emission together with their inline values.
    let modern_options: HashSet<&str> = arguments
        .into_iter()
        .flatten()
        .filter_map(|argument| match argument {
            Argument::Normal(argument) => Some(argument.as_str()),
            Argument::Ruled { value, .. } => match value {
                ArgumentValue::Single(argument) => Some(argument.as_str()),
                ArgumentValue::Many(_) => None,
            },
        })
        .filter(|argument| argument.starts_with('-'))
        .collect();

    if let Some(legacy_arguments) = legacy_arguments {
        let mut legacy = legacy_arguments.split(' ').peekable();
        while let Some(x) = legacy.next() {
            if modern_options.contains(x) {
                if let Some(next) = legacy.peek()
                    && !next.starts_with('-')
                {
                    legacy.next();
                }
                continue;
            }
            parsed_arguments.push(parse_minecraft_argument(
                &x.replace(' ', TEMPORARY_REPLACE_CHAR),
                &access_token,
                &profile.name,
                profile.id,
                user_type,
                version,
                asset_index_name,
                game_directory,
                assets_directory,
                game_assets_directory,
                version_type,
                resolution,
                quick_play_type,
            )?);
        }
    }

    if let Some(arguments) = arguments {
        parse_arguments(
            arguments,
            &mut parsed_arguments,
            |arg| {
                parse_minecraft_argument(
                    arg,
                    &access_token,
                    &profile.name,
                    profile.id,
                    user_type,
                    version,
                    asset_index_name,
                    game_directory,
                    assets_directory,
                    game_assets_directory,
                    version_type,
                    resolution,
                    quick_play_type,
                )
            },
            java_arch,
            quick_play_type,
        )?;
    }

    if let QuickPlayType::Server(server) = quick_play_type
        && quick_play_version.server == QuickPlayServerVersion::BuiltinLegacy
    {
        let (host, port) = server.require_resolved()?;
        parsed_arguments.extend_from_slice(&[
            "--server".to_string(),
            host.to_string(),
            "--port".to_string(),
            port.to_string(),
        ]);
    }

    Ok(parsed_arguments)
}

#[allow(clippy::too_many_arguments)]
fn parse_minecraft_argument(
    argument: &str,
    access_token: &str,
    username: &str,
    uuid: Uuid,
    user_type: &str,
    version: &str,
    asset_index_name: &str,
    game_directory: &Path,
    assets_directory: &Path,
    game_assets_directory: &Path,
    version_type: &VersionType,
    resolution: WindowSize,
    quick_play_type: &QuickPlayType,
) -> crate::Result<String> {
    Ok(argument
        .replace("${accessToken}", access_token)
        .replace("${auth_access_token}", access_token)
        .replace("${auth_session}", access_token)
        .replace("${auth_player_name}", username)
        // TODO: add auth xuid eventually
        .replace("${auth_xuid}", "0")
        .replace("${auth_uuid}", &uuid.simple().to_string())
        .replace("${uuid}", &uuid.simple().to_string())
        .replace("${clientid}", "c4502edb-87c6-40cb-b595-64a280cf8906")
        .replace("${user_properties}", "{}")
        .replace("${user_type}", user_type)
        .replace("${version_name}", version)
        .replace("${assets_index_name}", asset_index_name)
        .replace(
            "${game_directory}",
            &canonicalize(game_directory)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified game directory {} does not exist",
                        game_directory.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy(),
        )
        .replace(
            "${assets_root}",
            &canonicalize(assets_directory)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified assets directory {} does not exist",
                        assets_directory.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy(),
        )
        .replace(
            "${game_assets}",
            &canonicalize(game_assets_directory)
                .map_err(|_| {
                    crate::ErrorKind::LauncherError(format!(
                        "Specified assets directory {} does not exist",
                        game_assets_directory.to_string_lossy()
                    ))
                    .as_error()
                })?
                .to_string_lossy(),
        )
        .replace("${version_type}", version_type.as_str())
        .replace("${resolution_width}", &resolution.0.to_string())
        .replace("${resolution_height}", &resolution.1.to_string())
        .replace(
            "${quickPlaySingleplayer}",
            match quick_play_type {
                QuickPlayType::Singleplayer(world) => world,
                _ => "",
            },
        )
        .replace(
            "${quickPlayMultiplayer}",
            &match quick_play_type {
                QuickPlayType::Server(address) => address.to_string(),
                _ => "".to_string(),
            },
        ))
}

fn parse_arguments<F>(
    arguments: &[Argument],
    parsed_arguments: &mut Vec<String>,
    parse_function: F,
    java_arch: &str,
    quick_play_type: &QuickPlayType,
) -> crate::Result<()>
where
    F: Fn(&str) -> crate::Result<String>,
{
    for argument in arguments {
        match argument {
            Argument::Normal(arg) => {
                let parsed =
                    parse_function(&arg.replace(' ', TEMPORARY_REPLACE_CHAR))?;
                for arg in parsed.split(TEMPORARY_REPLACE_CHAR) {
                    parsed_arguments.push(arg.to_string());
                }
            }
            Argument::Ruled { rules, value } => {
                if parse_rules(rules, java_arch, quick_play_type, true) {
                    match value {
                        ArgumentValue::Single(arg) => {
                            parsed_arguments.push(parse_function(
                                &arg.replace(' ', TEMPORARY_REPLACE_CHAR),
                            )?);
                        }
                        ArgumentValue::Many(args) => {
                            for arg in args {
                                parsed_arguments.push(parse_function(
                                    &arg.replace(' ', TEMPORARY_REPLACE_CHAR),
                                )?);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn get_processor_arguments(
    libraries_path: &Path,
    arguments: &[impl AsRef<str>],
    data: &HashMap<String, SidedDataEntry>,
) -> crate::Result<Vec<String>> {
    // We use iterator combinators to make sure that 1 input argument maps
    // to exactly 1 output argument. Otherwise you might get issues that take
    // days to debug *sigh*
    //
    // Arguments can be enclosed in square brackets [] if they are not taken
    // literally, but are actually resolved to the path of a library we
    // previously downloaded.
    // For example, `[net.neoforged:neoform:1.21.10-20251010.172816:mappings@tsrg.lzma]`.
    //
    // Otherwise, arguments may contain `{KEY}` variable placeholders, which
    // must be replaced with the corresponding value from `data`.
    // Importantly, variables might not *just* be `{KEY}`, but may also be
    // e.g. `{KEY}/some more values`. For example, `{ROOT}/libraries/`.
    // Therefore, it is important that we don't just check if the variable is
    // enclosed in `{}`s, but actually do a find-and-replace with all variables.
    //
    // Currently, we do it in a naive way where we iterate over every `data`
    // entry and just `.replace()`, which is not efficient, but we shouldn't
    // have a lot of entries in `data`, and this code is not run often anyway.

    arguments
        .iter()
        .map(|arg| {
            let arg = arg.as_ref();
            if let Some(arg) = arg.strip_prefix('[')
                && let Some(lib_key) = arg.strip_suffix(']')
            {
                // this should resolve to the path of a library
                get_lib_path(libraries_path, lib_key, true)
            } else {
                let mut arg = arg.to_string();

                // replace variables like `{PATH}` to their real values
                for (key, entry) in data {
                    let replacement = if let Some(arg) =
                        entry.client.strip_prefix('[')
                        && let Some(lib_key) = arg.strip_suffix(']')
                    {
                        // if the value of `PATH` in `data` is also a library key,
                        // it'll be enclosed in `[]`s, and we resolve it to a real lib path
                        get_lib_path(libraries_path, lib_key, true)?
                    } else {
                        // otherwise we just take the value in `data` literally
                        entry.client.clone()
                    };

                    arg = arg.replace(&format!("{{{key}}}"), &replacement);
                }

                Ok(arg)
            }
        })
        .collect::<crate::Result<Vec<_>>>()
}

pub async fn get_processor_main_class(
    path: String,
) -> crate::Result<Option<String>> {
    let main_class = tokio::task::spawn_blocking(move || {
        let zipfile = std::fs::File::open(&path)
            .map_err(|e| IOError::with_path(e, &path))?;
        let mut archive = zip::ZipArchive::new(zipfile).map_err(|_| {
            crate::ErrorKind::LauncherError(format!(
                "Cannot read processor at {path}"
            ))
            .as_error()
        })?;

        let file = archive.by_name("META-INF/MANIFEST.MF").map_err(|_| {
            crate::ErrorKind::LauncherError(format!(
                "Cannot read processor manifest at {path}"
            ))
            .as_error()
        })?;

        let reader = BufReader::new(file);

        for line in reader.lines() {
            let mut line = line.map_err(IOError::from)?;
            line.retain(|c| !c.is_whitespace());

            if line.starts_with("Main-Class:")
                && let Some(class) = line.split(':').nth(1)
            {
                return Ok(Some(class.to_string()));
            }
        }

        Ok::<Option<String>, crate::Error>(None)
    })
    .await??;

    Ok(main_class)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::direct_link::LinkedLauncherDialect;
    use crate::launcher::quick_play_version::QuickPlaySingleplayerVersion;
    use serde_json::json;

    #[test]
    fn pcl_ce_jvm_arguments_expand_before_axolotl_runtime_options() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let manifest = [
            Argument::Normal("-Dprofile=${version_name}".into()),
            Argument::Normal("-Dprofile=${version_name}".into()),
        ];
        let parsed = get_jvm_arguments(
            Some(&manifest),
            path,
            path,
            path,
            "",
            path,
            "linked",
            MemorySettings {
                maximum: 2048,
                automatic: false,
                optimize_before_launch: false,
            },
            vec!["-Dcustom=${libraries_directory}".into()],
            "x86_64",
            &QuickPlayType::None,
            QuickPlayVersion::find_version(0, &[]),
            None,
            "127.0.0.1:12345".parse().unwrap(),
            Some(&[
                ("${version_name}", "Test Profile"),
                ("${libraries_directory}", r"C:\PCL Files\libraries"),
            ]),
        )
        .unwrap();
        assert_eq!(
            &parsed[..3],
            &[
                r"-Dcustom=C:\PCL Files\libraries",
                "-Dprofile=Test Profile",
                "-Xmx2048M"
            ]
        );
        assert!(
            parsed
                .iter()
                .any(|argument| argument.starts_with("-javaagent:"))
        );
    }

    #[test]
    fn pcl_ce_quick_play_is_appended_without_manifest_features() {
        let server = QuickPlayType::Server(
            crate::server_address::ServerAddress::Resolved {
                original_host: "play.example.test".into(),
                original_port: 25565,
                resolved_host: "127.0.0.1".into(),
                resolved_port: 25570,
            },
        );
        let version = QuickPlayVersion::find_version(0, &[]);
        let mut arguments = Vec::new();
        append_pcl_ce_quick_play(&mut arguments, &server, version).unwrap();
        assert_eq!(
            arguments,
            ["--quickPlayMultiplayer", "play.example.test:25565"]
        );
        arguments.clear();
        append_pcl_ce_quick_play(
            &mut arguments,
            &QuickPlayType::Singleplayer("My World".into()),
            version,
        )
        .unwrap();
        assert_eq!(arguments, ["--quickPlaySingleplayer", "My World"]);
    }

    #[test]
    fn legacy_fallen_args_get_discovery_without_replacing_custom_hosts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let legacy = [
            "-Dminecraft.api.auth.host=https://auth.msp.fallenbreath.me",
            "-Dminecraft.api.account.host=https://account.msp.fallenbreath.me",
            "-Dminecraft.api.session.host=https://session.msp.fallenbreath.me",
            "-Dminecraft.api.services.host=https://services.msp.fallenbreath.me",
            "-Dminecraft.api.profiles.host=https://profiles.msp.fallenbreath.me",
        ];
        let discovery = "-Dminecraft.api.discovery.host=https://discovery.msp.fallenbreath.me/minecraft/client";
        let custom_discovery =
            "-Dminecraft.api.discovery.host=https://example.test/client";
        let custom_session =
            "-Dminecraft.api.session.host=https://example.test";
        let launch = |custom_args: &[&str]| {
            get_jvm_arguments(
                Some(&[]),
                path,
                path,
                path,
                "",
                path,
                "26.3",
                MemorySettings {
                    maximum: 2048,
                    automatic: false,
                    optimize_before_launch: false,
                },
                custom_args.iter().map(ToString::to_string).collect(),
                "x86_64",
                &QuickPlayType::None,
                QuickPlayVersion::find_version(0, &[]),
                None,
                "127.0.0.1:12345".parse().unwrap(),
                None,
            )
            .unwrap()
            .into_iter()
            .filter(|arg| arg.starts_with("-Dminecraft.api."))
            .collect::<Vec<_>>()
        };

        assert_eq!(launch(&legacy), [legacy.as_slice(), &[discovery]].concat());
        assert_eq!(launch(&[]), Vec::<String>::new());
        for custom in [
            discovery,
            custom_discovery,
            custom_session,
            "-Dminecraft.api.session.host",
        ] {
            let overridden = [legacy.as_slice(), &[custom]].concat();
            assert_eq!(launch(&overridden), overridden);
        }
    }

    #[test]
    fn universal_os_arguments_still_require_the_declared_architecture() {
        let arguments: Vec<Argument> = serde_json::from_value(serde_json::json!([
            "-Dalways=true",
            {
                "rules": [{"action": "allow", "os": {"name": "universal"}}],
                "value": "-Duniversal=true"
            },
            {
                "rules": [{"action": "allow", "os": {"name": "universal", "arch": "x86"}}],
                "value": ["-Xss1M"]
            }
        ]))
        .unwrap();

        for (architecture, uses_x86_argument) in [
            ("x86", true),
            ("i386", true),
            ("i686", true),
            ("x86_64", false),
            ("amd64", false),
            ("aarch64", false),
            ("arm64", false),
        ] {
            let mut parsed = Vec::new();
            parse_arguments(
                &arguments,
                &mut parsed,
                |argument| Ok(argument.to_string()),
                architecture,
                &QuickPlayType::None,
            )
            .unwrap();
            let mut expected = vec!["-Dalways=true", "-Duniversal=true"];
            if uses_x86_argument {
                expected.push("-Xss1M");
            }
            assert_eq!(parsed, expected, "Java architecture: {architecture}");
        }
    }

    #[tokio::test]
    async fn mixed_legacy_and_modern_game_arguments_are_both_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let game_directory = directory.path().join("instance");
        let assets_directory = directory.path().join("assets");
        std::fs::create_dir_all(&game_directory).unwrap();
        std::fs::create_dir_all(&assets_directory).unwrap();
        let credentials = Credentials::offline("Player").unwrap();
        let modern = vec![
            Argument::Normal("--tweakClass".to_string()),
            Argument::Normal("example.LiteLoaderTweaker".to_string()),
        ];

        let parsed = get_minecraft_arguments(
            Some(&modern),
            Some("--username ${auth_player_name} --gameDir ${game_directory}"),
            &credentials,
            "1.12.2",
            "1.12",
            &game_directory,
            &assets_directory,
            &assets_directory,
            &VersionType::Release,
            WindowSize(854, 480),
            "x86_64",
            &QuickPlayType::None,
            QuickPlayVersion {
                server: QuickPlayServerVersion::Unsupported,
                singleplayer: QuickPlaySingleplayerVersion::Unsupported,
            },
        )
        .await
        .unwrap();

        assert_eq!(parsed[0], "--username");
        assert_eq!(parsed[1], "Player");
        assert_eq!(parsed[2], "--gameDir");
        assert_eq!(
            parsed[3],
            canonicalize(&game_directory).unwrap().to_string_lossy()
        );
        assert_eq!(&parsed[4..], ["--tweakClass", "example.LiteLoaderTweaker"]);
    }

    #[test]
    fn linked_classpath_uses_download_and_local_paths_and_skips_native_only() {
        let root = tempfile::tempdir().unwrap();
        let artifact = root.path().join("libraries/custom/a.jar");
        let local = root.path().join("versions/demo/libraries/local.jar");
        let main = root.path().join("main.jar");
        let client = root.path().join("client.jar");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&artifact, b"artifact").unwrap();
        std::fs::write(&local, b"local").unwrap();
        std::fs::write(&main, b"main").unwrap();
        std::fs::write(&client, b"client").unwrap();

        let libraries: Vec<LinkedLibrary> = serde_json::from_value(json!([
            {
                "name":"x:artifact:1",
                "downloads":{"artifact":{"path":"custom/a.jar", "sha1":"", "size":0, "url":""}}
            },
            {
                "name":"x:local:1",
                "MMC-hint":"local",
                "MMC-filename":"local.jar"
            },
            {
                "name":"x:native:1",
                "natives":{"linux":"natives-linux"},
                "downloads":{"classifiers":{"natives-linux":{"path":"unused.jar", "sha1":"", "size":0, "url":""}}}
            }
        ]))
        .unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: root.path().to_path_buf(),
            launcher_root: None,
            version_id: "demo".to_string(),
            version_json: None,
            dialect: LinkedLauncherDialect::Hmcl,
            game_dir_mode: None,
        };

        let classpath = get_linked_class_paths(
            &direct,
            &libraries,
            &[&main, &client],
            std::env::consts::ARCH,
            true,
        )
        .unwrap();
        let paths = classpath
            .split(classpath_separator(std::env::consts::ARCH))
            .collect::<Vec<_>>();
        assert!(
            paths.contains(
                &canonicalize(main).unwrap().to_string_lossy().as_ref()
            )
        );
        assert!(paths.contains(
            &canonicalize(client).unwrap().to_string_lossy().as_ref()
        ));
        assert!(paths.contains(
            &canonicalize(artifact).unwrap().to_string_lossy().as_ref()
        ));
        assert!(paths.contains(
            &canonicalize(local).unwrap().to_string_lossy().as_ref()
        ));
        assert_eq!(paths.len(), 4);
    }

    #[test]
    fn pcl_ce_classpath_keeps_the_bridge_then_pcl_ce_library_and_jar_order() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        let names = [
            "bridge.jar",
            "client.jar",
            "libraries/first.jar",
            "libraries/last.jar",
            "libraries/optifine.jar",
        ];
        std::fs::create_dir(path.join("libraries")).unwrap();
        for name in names {
            std::fs::write(path.join(name), b"test").unwrap();
        }
        let libraries: Vec<LinkedLibrary> = serde_json::from_value(json!([
            {"name":"test:first:1", "downloads":{"artifact":{"path":"first.jar", "sha1":"", "size":0, "url":""}}},
            {"name":"optifine:OptiFine:1", "downloads":{"artifact":{"path":"optifine.jar", "sha1":"", "size":0, "url":""}}},
            {"name":"test:last:1", "downloads":{"artifact":{"path":"last.jar", "sha1":"", "size":0, "url":""}}},
            {"name":"test:native:1", "natives":{"windows":"natives-windows"}}
        ])).unwrap();
        let direct = DirectLinkedLaunch {
            dot_minecraft: path.to_path_buf(),
            launcher_root: None,
            version_id: "demo".into(),
            version_json: None,
            dialect: LinkedLauncherDialect::PclCe,
            game_dir_mode: None,
        };
        let classpath = get_pcl_ce_class_paths(
            &direct,
            &libraries,
            &path.join("bridge.jar"),
            &path.join("client.jar"),
            path,
            ["first config.jar", "second config.jar"].into_iter(),
            std::env::consts::ARCH,
        )
        .unwrap();
        let expected = [
            "bridge.jar",
            "second config.jar",
            "first config.jar",
            "libraries/first.jar",
            "libraries/optifine.jar",
            "libraries/last.jar",
            "client.jar",
        ];
        let expected = expected
            .into_iter()
            .map(|name| {
                let path = path.join(name);
                canonicalize(&path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned()
            })
            .join(classpath_separator(std::env::consts::ARCH));
        assert_eq!(classpath, expected);
    }

    #[tokio::test]
    async fn legacy_options_duplicated_by_modern_arguments_are_dropped() {
        let directory = tempfile::tempdir().unwrap();
        let game_directory = directory.path().join("instance");
        let assets_directory = directory.path().join("assets");
        let game_assets_directory = directory.path().join("resources");
        std::fs::create_dir_all(&game_directory).unwrap();
        std::fs::create_dir_all(&assets_directory).unwrap();
        std::fs::create_dir_all(&game_assets_directory).unwrap();
        let credentials = Credentials::offline("Player").unwrap();

        // 1.6.4-era Forge profiles repeat `minecraftArguments` verbatim inside
        // `arguments.game`; emitting both made launchwrapper reject the
        // duplicate options.
        let modern = vec![
            Argument::Normal("--username".to_string()),
            Argument::Normal("${auth_player_name}".to_string()),
            Argument::Normal("--version".to_string()),
            Argument::Normal("${version_name}".to_string()),
            Argument::Normal("--gameDir".to_string()),
            Argument::Normal("${game_directory}".to_string()),
            Argument::Normal("--assetsDir".to_string()),
            Argument::Normal("${game_assets}".to_string()),
            Argument::Normal("--tweakClass".to_string()),
            Argument::Normal(
                "cpw.mods.fml.common.launcher.FMLTweaker".to_string(),
            ),
        ];
        let legacy = "--username ${auth_player_name} --version ${version_name} --gameDir ${game_directory} --assetsDir ${game_assets} --tweakClass cpw.mods.fml.common.launcher.FMLTweaker";

        let parsed = get_minecraft_arguments(
            Some(&modern),
            Some(legacy),
            &credentials,
            "1.6.4",
            "legacy",
            &game_directory,
            &assets_directory,
            &game_assets_directory,
            &VersionType::Release,
            WindowSize(854, 480),
            "x86_64",
            &QuickPlayType::None,
            QuickPlayVersion {
                server: QuickPlayServerVersion::Unsupported,
                singleplayer: QuickPlaySingleplayerVersion::Unsupported,
            },
        )
        .await
        .unwrap();

        for option in [
            "--username",
            "--version",
            "--gameDir",
            "--assetsDir",
            "--tweakClass",
        ] {
            assert_eq!(
                parsed.iter().filter(|arg| *arg == option).count(),
                1,
                "option {option} must appear exactly once, got: {parsed:?}"
            );
        }
        assert!(
            parsed.contains(
                &"cpw.mods.fml.common.launcher.FMLTweaker".to_string()
            )
        );
        let assets_dir_position = parsed
            .iter()
            .position(|argument| argument == "--assetsDir")
            .unwrap();
        assert_eq!(
            parsed[assets_dir_position + 1],
            canonicalize(&game_assets_directory)
                .unwrap()
                .to_string_lossy()
        );

        // Legacy-only options still pass through unchanged.
        let parsed = get_minecraft_arguments(
            Some(&modern),
            Some("--demo --gameDir ${game_directory}"),
            &credentials,
            "1.6.4",
            "legacy",
            &game_directory,
            &assets_directory,
            &game_assets_directory,
            &VersionType::Release,
            WindowSize(854, 480),
            "x86_64",
            &QuickPlayType::None,
            QuickPlayVersion {
                server: QuickPlayServerVersion::Unsupported,
                singleplayer: QuickPlaySingleplayerVersion::Unsupported,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            parsed.iter().filter(|arg| *arg == "--demo").count(),
            1,
            "legacy-only option must be preserved, got: {parsed:?}"
        );
        assert_eq!(
            parsed.iter().filter(|arg| *arg == "--gameDir").count(),
            1,
            "option already provided by modern arguments must not duplicate, got: {parsed:?}"
        );
    }

    #[test]
    fn native_libraries_missing_main_artifacts_are_tolerated_on_classpath() {
        let directory = tempfile::tempdir().unwrap();
        let libraries: Vec<Library> = vec![
            // Legacy (2013-era loader profile) shape: natives without a
            // downloads block, exactly like Forge 1.6.4's lwjgl-platform.
            serde_json::from_value(serde_json::json!({
                "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.0",
                "natives": {
                    "linux": "natives-linux",
                    "osx": "natives-osx",
                    "windows": "natives-windows"
                }
            }))
            .unwrap(),
            // Modern shape: natives together with a downloads.classifiers
            // block. Its main artifact is also legitimately absent on disk.
            serde_json::from_value(serde_json::json!({
                "name": "org.lwjgl:lwjgl-platform:3.2.1",
                "natives": { "windows": "natives-windows" },
                "downloads": {
                    "classifiers": {
                        "natives-windows": {
                            "sha1": "abc",
                            "size": 64,
                            "url": "https://example.com/natives.jar"
                        }
                    }
                }
            }))
            .unwrap(),
        ];

        // Neither main artifact exists on disk; both must be tolerated.
        let class_paths =
            get_class_paths(directory.path(), &libraries, &[], "x86_64", true)
                .unwrap();
        assert!(class_paths.contains("lwjgl-platform-2.9.0.jar"));
        assert!(class_paths.contains("lwjgl-platform-3.2.1.jar"));

        // Non-native libraries keep their strict existence requirement.
        let missing: Library = serde_json::from_value(serde_json::json!({
            "name": "net.sf.jopt-simple:jopt-simple:4.5"
        }))
        .unwrap();
        assert!(
            get_class_paths(directory.path(), &[missing], &[], "x86_64", true)
                .is_err()
        );
    }

    #[test]
    fn classpath_prefers_the_last_revision_of_a_duplicate_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let old = directory
            .path()
            .join("org/lwjgl/lwjgl/3.2.1/lwjgl-3.2.1.jar");
        let current = directory
            .path()
            .join("org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&current, b"current").unwrap();

        let libraries: Vec<Library> =
            ["org.lwjgl:lwjgl:3.2.1", "org.lwjgl:lwjgl:3.2.2"]
                .into_iter()
                .map(|name| {
                    serde_json::from_value(serde_json::json!({
                        "name": name,
                    }))
                    .unwrap()
                })
                .collect();
        let class_paths =
            get_class_paths(directory.path(), &libraries, &[], "x86_64", true)
                .unwrap();

        assert!(class_paths.contains("lwjgl-3.2.2.jar"));
        assert!(!class_paths.contains("lwjgl-3.2.1.jar"));
    }

    #[test]
    fn classpath_keeps_main_artifact_with_native_classifier() {
        let directory = tempfile::tempdir().unwrap();
        let main = directory
            .path()
            .join("org/lwjgl/lwjgl-glfw/3.3.3/lwjgl-glfw-3.3.3.jar");
        let native = directory.path().join(
            "org/lwjgl/lwjgl-glfw/3.3.3/lwjgl-glfw-3.3.3-natives-windows.jar",
        );
        std::fs::create_dir_all(main.parent().unwrap()).unwrap();
        std::fs::write(&main, b"main").unwrap();
        std::fs::write(&native, b"native").unwrap();

        let libraries: Vec<Library> = [
            "org.lwjgl:lwjgl-glfw:3.3.3",
            "org.lwjgl:lwjgl-glfw:3.3.3:natives-windows",
        ]
        .into_iter()
        .map(|name| {
            serde_json::from_value(serde_json::json!({ "name": name })).unwrap()
        })
        .collect();
        let class_paths =
            get_class_paths(directory.path(), &libraries, &[], "x86_64", true)
                .unwrap();

        assert!(class_paths.contains("lwjgl-glfw-3.3.3.jar"));
        assert!(class_paths.contains("lwjgl-glfw-3.3.3-natives-windows.jar"));
    }
}
