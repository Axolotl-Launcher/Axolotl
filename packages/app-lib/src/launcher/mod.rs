//! Logic for launching Minecraft
use crate::api::pack::import::direct_link::direct_link_group;
use crate::data::ModLoader;
use crate::event::emit::{emit_instance, emit_loading, init_loading};
use crate::event::{InstancePayloadType, LoadingBarType};
use crate::install::{
    InstallJavaStep, InstallPhaseDetails, InstallPhaseId, InstallProgress,
    InstallProgressReporter,
};
use crate::instance::QuickPlayType;
pub use crate::launcher::direct_link::ExternalGameDirMode;
pub(crate) use crate::launcher::direct_link::{
    DirectLinkedLaunch, LinkedLauncherDialect, apply_hmcl_settings,
    conservative_launch_facts, extract_linked_natives, hmcl_java_candidates,
    hmcl_with_global_fallback, merged_to_version_info,
    normalize_merged_loader_libraries, pcl_available_memory_gb,
    pcl_ram_profile,
};
use crate::launcher::download::{LocalRuntimeSource, download_log_config};
use crate::launcher::instance_runtime::InstanceRuntimeAdapter;
use crate::launcher::quick_play_version::{
    QuickPlayServerVersion, QuickPlayVersion,
};
use crate::server_address::{ServerAddress, parse_server_address};
use crate::state::store::Collection as _;
use crate::state::store::join_log::{JoinLog, JoinLogEntry, JoinLogKey};
use crate::state::{
    CacheBehaviour, Credentials, Instance, InstanceInstallStage,
    InstanceLaunchContext, InstanceLink, JavaVersion, MemorySettings,
    ProcessMetadata, WindowSize,
};
use crate::util::rpc::RpcServerBuilder;
use crate::util::{io, java_environment};
use crate::{State, get_resource_file, process};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use chrono::Utc;
use daedalus as d;
use daedalus::minecraft::{LoggingSide, RuleAction, VersionInfo};
use daedalus::modded::{LoaderVersion, Manifest};
use regex::Regex;
use serde::Deserialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

pub(crate) struct GameVersionMetadata {
    pub world_version: Option<u32>,
}

pub(crate) async fn read_game_version_metadata_from_jar(
    _path: &Path,
) -> crate::Result<Option<GameVersionMetadata>> {
    Ok(None)
}

#[cfg(target_os = "windows")]
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

mod args;
pub(crate) use args::get_processor_arguments;
mod direct_ensure;
mod direct_link;
pub(crate) mod external_version;
pub(crate) mod instance_runtime;
mod local_artifact;
mod natives;

pub mod download;
pub mod jvm_args;
pub mod language;
pub mod local_version;
pub(crate) mod multimc;
pub mod optifine;
pub mod quick_play_version;

const UTF8_GAME_ARGUMENT_PREFIX: &str = "__THESEUS_UTF8__:";

fn encode_game_argument(argument: String) -> String {
    if argument.is_ascii() && !argument.starts_with(UTF8_GAME_ARGUMENT_PREFIX) {
        argument
    } else {
        format!(
            "{UTF8_GAME_ARGUMENT_PREFIX}{}",
            BASE64_STANDARD.encode(argument.as_bytes())
        )
    }
}

#[cfg(target_os = "windows")]
fn set_high_performance_gpu_preference(
    executable: impl AsRef<std::path::Path>,
) -> crate::Result<()> {
    const REGISTRY_PATH: &str =
        "Software\\Microsoft\\DirectX\\UserGpuPreferences";
    const REGISTRY_VALUE: &str = "GpuPreference=2;";

    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = current_user.create_subkey(REGISTRY_PATH)?;
    let executable = executable.as_ref().to_string_lossy();

    if key
        .get_value::<String, _>(executable.as_ref())
        .ok()
        .as_deref()
        == Some(REGISTRY_VALUE)
    {
        return Ok(());
    }

    key.set_value(executable.as_ref(), &REGISTRY_VALUE)?;
    tracing::info!(%executable, "Set high-performance GPU preference");

    Ok(())
}

#[cfg(any(target_os = "linux", test))]
mod gpu_environment;

#[cfg(target_os = "linux")]
async fn high_performance_gpu_environment() -> Vec<(String, String)> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        switcheroo_gpu_environment(),
    )
    .await
    {
        Ok(Ok(environment)) => environment,
        result => {
            tracing::debug!(
                ?result,
                "GPU topology unavailable; preserving the system GPU selection"
            );
            Vec::new()
        }
    }
}

#[cfg(target_os = "linux")]
async fn switcheroo_gpu_environment() -> crate::Result<Vec<(String, String)>> {
    use std::collections::HashMap;
    use zbus::zvariant::OwnedValue;
    use zbus::{Connection, Proxy};
    let x11_session = std::env::var("XDG_SESSION_TYPE")
        .is_ok_and(|value| value == "x11")
        || (std::env::var_os("DISPLAY").is_some()
            && std::env::var_os("WAYLAND_DISPLAY").is_none());
    if x11_session {
        let mut command = Command::new("glxinfo");
        java_environment::configure_child_environment(command.as_std_mut());
        let output = command.arg("-B").kill_on_drop(true).output().await;
        let renderer_is_nvidia = output
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                gpu_environment::glx_renderer_is_nvidia(
                    &String::from_utf8_lossy(&output.stdout),
                )
            });
        if renderer_is_nvidia != Some(false) {
            tracing::debug!(
                ?renderer_is_nvidia,
                "Preserving the current X11 renderer without PRIME injection"
            );
            return Ok(Vec::new());
        }
    }

    let connection = Connection::system().await?;
    let proxy = Proxy::new(
        &connection,
        "net.hadess.SwitcherooControl",
        "/net/hadess/SwitcherooControl",
        "net.hadess.SwitcherooControl",
    )
    .await?;
    let gpus: Vec<HashMap<String, OwnedValue>> =
        proxy.get_property("GPUs").await?;
    let gpus = gpus
        .into_iter()
        .map(|mut gpu| gpu_environment::GpuEnvironment {
            is_default: gpu
                .get("Default")
                .and_then(|value| bool::try_from(value).ok()),
            name: gpu
                .remove("Name")
                .and_then(|value| String::try_from(value).ok())
                .unwrap_or_default(),
            environment: gpu
                .remove("Environment")
                .and_then(|value| Vec::<String>::try_from(value).ok())
                .unwrap_or_default(),
        })
        .collect();
    Ok(gpu_environment::select_offload_environment(gpus))
}

/// Mojang/HMCL ordered rule semantics: an empty list allows, a non-empty list
/// starts disallowed, and each matching rule replaces the previous decision.
#[tracing::instrument]
pub fn parse_rules(
    rules: &[d::minecraft::Rule],
    java_version: &str,
    quick_play_type: &QuickPlayType,
    minecraft_updated: bool,
) -> bool {
    if rules.is_empty() {
        return true;
    }

    rules.iter().fold(false, |allowed, rule| {
        parse_rule(rule, java_version, quick_play_type, minecraft_updated)
            .unwrap_or(allowed)
    })
}

/// Returns this rule's action only when every declared OS and feature
/// restriction matches. OS and features on the same rule are conjunctive.
#[tracing::instrument]
pub fn parse_rule(
    rule: &d::minecraft::Rule,
    java_version: &str,
    quick_play_type: &QuickPlayType,
    minecraft_updated: bool,
) -> Option<bool> {
    let os_matches = rule.os.as_ref().is_none_or(|os| {
        crate::util::platform::os_rule(os, java_version, minecraft_updated)
    });
    let features_match = rule.features.as_ref().is_none_or(|features| {
        let facts = [
            (features.is_demo_user, false),
            (features.has_custom_resolution, true),
            (features.has_quick_plays_support, true),
            (
                features.is_quick_play_singleplayer,
                matches!(quick_play_type, QuickPlayType::Singleplayer(_)),
            ),
            (
                features.is_quick_play_multiplayer,
                matches!(quick_play_type, QuickPlayType::Server(..)),
            ),
            (features.is_quick_play_realms, false),
        ];
        facts.into_iter().all(|(expected, actual)| {
            expected.is_none_or(|expected| expected == actual)
        })
    });

    (os_matches && features_match)
        .then_some(matches!(rule.action, RuleAction::Allow))
}

macro_rules! processor_rules {
    ($dest:expr; $($name:literal : client => $client:expr, server => $server:expr;)+) => {
        $(std::collections::HashMap::insert(
            $dest,
            String::from($name),
            daedalus::modded::SidedDataEntry {
                client: String::from($client),
                server: String::from($server),
            },
        );)+
    }
}

fn processor_output_sha1(value: &str) -> Option<&str> {
    let value = value.trim().trim_matches(['\'', '"']);
    let value = value
        .strip_prefix("sha1:")
        .or_else(|| value.strip_prefix("SHA1:"))
        .unwrap_or(value);
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(value)
}

async fn processor_outputs_are_current(
    processor: &d::modded::Processor,
    libraries_dir: &Path,
    data: &std::collections::HashMap<String, d::modded::SidedDataEntry>,
    allowed_roots: &[&Path],
) -> bool {
    let inferred_outputs: std::collections::HashMap<String, String>;
    let outputs = if let Some(outputs) = processor
        .outputs
        .as_ref()
        .filter(|outputs| !outputs.is_empty())
    {
        outputs
    } else {
        let is_binary_patcher =
            processor.jar.split(':').nth(1).is_some_and(|artifact| {
                artifact.eq_ignore_ascii_case("binarypatcher")
            });
        if !is_binary_patcher {
            return false;
        }
        let mut inferred = std::collections::HashMap::new();
        for (index, argument) in processor.args.iter().enumerate() {
            if argument != "--output" {
                continue;
            }
            let Some(path_template) = processor.args.get(index + 1) else {
                return false;
            };
            let Some(key) = path_template
                .strip_prefix('{')
                .and_then(|key| key.strip_suffix('}'))
            else {
                return false;
            };
            let hash_key = format!("{key}_SHA");
            if !data.contains_key(&hash_key) {
                return false;
            }
            inferred.insert(path_template.clone(), format!("{{{hash_key}}}"));
        }
        if inferred.is_empty() {
            return false;
        }
        inferred_outputs = inferred;
        &inferred_outputs
    };
    let mut canonical_roots = Vec::with_capacity(allowed_roots.len());
    for root in allowed_roots {
        if let Ok(root) = tokio::fs::canonicalize(root).await {
            canonical_roots.push(root);
        }
    }
    if canonical_roots.is_empty() {
        return false;
    }

    for (path_template, hash_template) in outputs {
        let resolved = match args::get_processor_arguments(
            libraries_dir,
            &[path_template, hash_template],
            data,
        ) {
            Ok(resolved) => resolved,
            Err(error) => {
                tracing::debug!(
                    processor = %processor.jar,
                    %error,
                    "Could not resolve processor output"
                );
                return false;
            }
        };
        let [resolved_path, resolved_hash] = resolved.as_slice() else {
            return false;
        };
        let Some(expected_sha1) = processor_output_sha1(resolved_hash) else {
            return false;
        };
        let path = PathBuf::from(resolved_path);
        if !path.is_absolute() {
            return false;
        }
        let Ok(path) = tokio::fs::canonicalize(path).await else {
            return false;
        };
        if !canonical_roots.iter().any(|root| path.starts_with(root)) {
            return false;
        }
        let Ok((_, actual_sha1)) =
            crate::util::fetch::sha1_file_async(&path).await
        else {
            return false;
        };
        if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
            return false;
        }
    }

    true
}

fn is_download_mojmaps_processor(processor: &d::modded::Processor) -> bool {
    processor
        .args
        .windows(2)
        .any(|args| args[0] == "--task" && args[1] == "DOWNLOAD_MOJMAPS")
}

async fn download_mojmaps_for_processor(
    state: &State,
    processor: &d::modded::Processor,
    client_mappings: Option<&(String, String, u32)>,
    libraries_dir: &Path,
    data: &std::collections::HashMap<String, d::modded::SidedDataEntry>,
) -> crate::Result<bool> {
    if !is_download_mojmaps_processor(processor) {
        return Ok(false);
    }

    let Some((download_url, download_sha1, download_size)) = client_mappings
    else {
        return Ok(false);
    };
    let arguments =
        args::get_processor_arguments(libraries_dir, &processor.args, data)?;
    let output = arguments
        .windows(2)
        .find(|args| args[0] == "--output")
        .map(|args| PathBuf::from(&args[1]))
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(
                "Forge DOWNLOAD_MOJMAPS processor did not declare an output path"
                    .to_string(),
            )
        })?;
    if !output.is_absolute() || !output.starts_with(libraries_dir) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Forge DOWNLOAD_MOJMAPS output is outside the libraries directory: {}",
            output.display()
        ))
        .as_error());
    }

    if output.exists()
        && let Ok((size, sha1)) =
            crate::util::fetch::sha1_file_async(&output).await
        && size == u64::from(*download_size)
        && sha1.eq_ignore_ascii_case(download_sha1)
    {
        return Ok(true);
    }

    let bytes = crate::util::fetch::fetch_official(
        download_url,
        Some(download_sha1),
        None,
        None,
        &state.download_semaphore,
    )
    .await?;
    if bytes.len() as u64 != u64::from(*download_size) {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Minecraft client mappings size mismatch: expected {}, got {}",
            download_size,
            bytes.len()
        ))
        .as_error());
    }
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    io::write(&output, bytes).await?;
    tracing::info!(
        processor = %processor.jar,
        output = %output.display(),
        "Downloaded Mojang mappings through the launcher HTTP client"
    );
    Ok(true)
}

pub async fn get_java_version_from_launch_context(
    context: &InstanceLaunchContext,
    version_info: &VersionInfo,
) -> crate::Result<Option<JavaVersion>> {
    if let Some(java) = context.launch_overrides.java_path.as_ref() {
        let java =
            crate::api::jre::check_jre(std::path::PathBuf::from(java)).await;

        if let Ok(java) = java {
            validate_loader_java_version(version_info, &java)?;
            return Ok(Some(java));
        }
    }

    let key = required_java_major(version_info);

    let java_version = crate::api::jre::find_java_for_version(key).await?;

    if let Some(java_version) = &java_version {
        validate_loader_java_version(version_info, java_version)?;
    }

    Ok(java_version)
}

fn required_java_major(version_info: &VersionInfo) -> u32 {
    if version_uses_liteloader(version_info) {
        8
    } else {
        version_info
            .java_version
            .as_ref()
            .map_or(8, |java| java.major_version)
    }
}

fn version_uses_liteloader(version_info: &VersionInfo) -> bool {
    version_info
        .libraries
        .iter()
        .any(|library| is_liteloader_library(&library.name))
}

fn has_client_processors(processors: Option<&[d::modded::Processor]>) -> bool {
    processors.is_some_and(|processors| {
        processors.iter().any(|processor| {
            processor
                .sides
                .as_ref()
                .is_none_or(|sides| sides.iter().any(|side| side == "client"))
        })
    })
}

fn is_liteloader_library(name: &str) -> bool {
    let mut coordinates = name.split(':');
    coordinates.next() == Some("com.mumfrey")
        && coordinates.next() == Some("liteloader")
        && coordinates.next().is_some()
}

fn validate_loader_java_version(
    version_info: &VersionInfo,
    java_version: &JavaVersion,
) -> crate::Result<()> {
    if version_uses_liteloader(version_info) && java_version.parsed_version != 8
    {
        return Err(crate::ErrorKind::LauncherError(format!(
            "LiteLoader requires Java 8, but Java {} is selected",
            java_version.parsed_version
        ))
        .into());
    }

    Ok(())
}

pub async fn get_loader_version_from_profile(
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
) -> crate::Result<Option<LoaderVersion>> {
    get_loader_version_from_profile_with_cache(
        game_version,
        loader,
        loader_version,
        None,
    )
    .await
}

pub async fn get_loader_version_from_profile_with_cache(
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<LoaderVersion>> {
    if loader == ModLoader::Vanilla {
        return Ok(None);
    }

    if loader == ModLoader::OptiFine {
        return optifine::resolve_loader_version(game_version, loader_version)
            .await;
    }

    let result = resolve_loader_version_with_refresh(
        game_version,
        loader_version,
        cache_behaviour,
        |behaviour| {
            crate::api::metadata::get_loader_versions_for_game_with_cache(
                loader.as_meta_str(),
                game_version,
                behaviour,
            )
        },
    )
    .await;
    if loader == ModLoader::Forge
        && cache_behaviour != Some(CacheBehaviour::CacheOnly)
        && let Some(requested) = explicit_loader_version(loader_version)
        && !matches!(&result, Ok(Some(_)))
    {
        return resolve_exact_loader_fallback(result, || {
            crate::api::loader_metadata::find_forge_version_in_fallback(
                game_version,
                requested,
            )
        })
        .await;
    }
    result
}

async fn resolve_exact_loader_fallback<F, Fut>(
    primary: crate::Result<Option<LoaderVersion>>,
    fallback: F,
) -> crate::Result<Option<LoaderVersion>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = crate::Result<Option<LoaderVersion>>>,
{
    if matches!(&primary, Ok(Some(_))) {
        return primary;
    }
    match fallback().await {
        Ok(Some(version)) => Ok(Some(version)),
        Ok(None) => primary,
        Err(error) => {
            tracing::warn!(%error, "Independent loader catalog lookup failed");
            match primary {
                Err(primary_error) => Err(primary_error),
                Ok(_) => Err(error),
            }
        }
    }
}

async fn resolve_loader_version_with_refresh<F, Fut>(
    game_version: &str,
    requested: Option<&str>,
    cache_behaviour: Option<CacheBehaviour>,
    mut fetch: F,
) -> crate::Result<Option<LoaderVersion>>
where
    F: FnMut(Option<CacheBehaviour>) -> Fut,
    Fut: std::future::Future<Output = crate::Result<Manifest>>,
{
    let select = |manifest: &Manifest| {
        loader_versions_for_game_version(manifest, game_version)
            .and_then(|loaders| select_loader_version(loaders, requested))
            .cloned()
    };
    let manifest = fetch(cache_behaviour).await?;
    if let Some(version) = select(&manifest) {
        return Ok(Some(version));
    }
    if matches!(
        cache_behaviour,
        Some(CacheBehaviour::CacheOnly | CacheBehaviour::Bypass)
    ) {
        return Ok(None);
    }

    tracing::info!(
        game_version,
        requested,
        "Loader version missing from catalog; forcing metadata refresh"
    );
    let refreshed = fetch(Some(CacheBehaviour::Bypass)).await?;
    Ok(select(&refreshed))
}

fn select_loader_version<'a>(
    loaders: &'a [LoaderVersion],
    requested: Option<&str>,
) -> Option<&'a LoaderVersion> {
    let requested = requested.unwrap_or("latest");
    loaders
        .iter()
        .find(|version| match requested {
            "latest" => true,
            "stable" => version.stable,
            id => version.id == id,
        })
        .or_else(|| (requested == "stable").then(|| loaders.first()).flatten())
}

fn explicit_loader_version(requested: Option<&str>) -> Option<&str> {
    requested.filter(|version| {
        !version.is_empty() && !matches!(*version, "latest" | "stable")
    })
}

fn missing_loader_version_fallback(
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
) -> Result<&'static str, String> {
    if let Some(requested) = explicit_loader_version(requested) {
        Err(format!(
            "Loader version {requested} is not available for {} {game_version}",
            loader.as_str()
        ))
    } else {
        Ok("stable")
    }
}

fn installed_offline_loader_version(
    loader: ModLoader,
    requested: Option<&str>,
) -> Option<LoaderVersion> {
    if !matches!(
        loader,
        ModLoader::Fabric
            | ModLoader::Forge
            | ModLoader::NeoForge
            | ModLoader::Quilt
            | ModLoader::Cleanroom
            | ModLoader::LiteLoader
            | ModLoader::LegacyFabric
            | ModLoader::Babric
    ) {
        return None;
    }
    let id = explicit_loader_version(requested)?;
    Some(LoaderVersion {
        id: id.to_string(),
        url: String::new(),
        stable: false,
        profile_source: Default::default(),
        fallback_url: None,
    })
}

async fn load_managed_launch_profile(
    state: &State,
    version: &d::minecraft::Version,
    loader: ModLoader,
    requested: Option<&str>,
    offline: bool,
) -> crate::Result<(Option<LoaderVersion>, d::minecraft::VersionInfo)> {
    let local_loader = if loader == ModLoader::OptiFine
        && explicit_loader_version(requested).is_some()
    {
        optifine::resolve_loader_version(&version.id, requested).await?
    } else {
        installed_offline_loader_version(loader, requested)
    };
    if offline {
        let loader_version =
            if loader == ModLoader::Vanilla || local_loader.is_some() {
                local_loader
            } else {
                get_loader_version_from_profile_with_cache(
                    &version.id,
                    loader,
                    requested,
                    Some(CacheBehaviour::CacheOnly),
                )
                .await?
            };
        ensure_launch_loader_selected(
            loader,
            &version.id,
            requested,
            loader_version.as_ref(),
        )?;
        let info = download::load_local_version_info(
            state,
            version,
            loader,
            loader_version.as_ref(),
        )
        .await?;
        return Ok((loader_version, info));
    }

    let local = if loader == ModLoader::Vanilla || local_loader.is_some() {
        download::try_load_launch_version_info(
            state,
            version,
            loader,
            local_loader.as_ref(),
        )
        .await
    } else {
        None
    };
    prefer_local_launch_profile(
        local.map(|info| (local_loader, info)),
        || async {
            let loader_version =
                get_loader_version_from_profile(&version.id, loader, requested)
                    .await?;
            ensure_launch_loader_selected(
                loader,
                &version.id,
                requested,
                loader_version.as_ref(),
            )?;
            // The local profile was absent, invalid or obsolete; do not read it again
            // through the download cache when recovering it.
            let info = download::download_version_info(
                state,
                version,
                loader,
                loader_version.as_ref(),
                Some(true),
                None,
                None,
            )
            .await?;
            Ok((loader_version, info))
        },
    )
    .await
}

async fn prefer_local_launch_profile<T, F, Fut>(
    local: Option<T>,
    fetch: F,
) -> crate::Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = crate::Result<T>>,
{
    match local {
        Some(profile) => Ok(profile),
        None => fetch().await,
    }
}

fn ensure_launch_loader_selected(
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
    selected: Option<&LoaderVersion>,
) -> crate::Result<()> {
    if loader != ModLoader::Vanilla && selected.is_none() {
        missing_loader_version_fallback(loader, game_version, requested)
            .map_err(crate::ErrorKind::LauncherError)?;
        return Err(crate::ErrorKind::LauncherError(format!(
            "No loader version selected for {}",
            loader.as_str()
        ))
        .into());
    }
    Ok(())
}

fn loader_versions_for_game_version<'a>(
    manifest: &'a Manifest,
    game_version: &str,
) -> Option<&'a [LoaderVersion]> {
    let version = manifest
        .game_versions
        .iter()
        .find(|version| version.id == game_version)
        .or_else(|| {
            manifest.game_versions.iter().find(|version| {
                version.id.replace(
                    daedalus::modded::DUMMY_REPLACE_STRING,
                    game_version,
                ) == game_version
            })
        })?;

    if let Some(version_group) = &version.version_group {
        manifest
            .version_groups
            .iter()
            .find(|group| group.id == *version_group)
            .map(|group| group.loaders.as_slice())
    } else {
        Some(version.loaders.as_slice())
    }
}

/// Resolves the Minecraft version manifest and finds the index for the given
/// game version. If the version isn't found in the cache, forces a manifest
/// refresh to pick up newly-released versions.
pub async fn resolve_minecraft_manifest(
    game_version: &str,
    state: &State,
) -> crate::Result<(d::minecraft::VersionManifest, usize)> {
    resolve_minecraft_manifest_with_cache(game_version, state, None).await
}

pub async fn resolve_minecraft_manifest_with_cache(
    game_version: &str,
    state: &State,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<(d::minecraft::VersionManifest, usize)> {
    let minecraft = crate::api::metadata::get_minecraft_versions_with_cache(
        cache_behaviour,
    )
    .await?;

    if let Some(idx) = minecraft
        .versions
        .iter()
        .position(|it| it.id == game_version)
    {
        return Ok((minecraft, idx));
    }

    // Version not found in the first manifest lookup. Online launches force a
    // refresh for newly released versions; offline launches repeat a cache-only
    // lookup so they never reach the network.
    let refreshed = crate::state::CachedEntry::get_minecraft_manifest(
        if cache_behaviour == Some(CacheBehaviour::CacheOnly) {
            Some(CacheBehaviour::CacheOnly)
        } else {
            Some(CacheBehaviour::MustRevalidate)
        },
        state.cache_pool(),
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor("minecraft versions".to_string())
    })?;

    let idx = refreshed
        .versions
        .iter()
        .position(|it| it.id == game_version)
        .ok_or(crate::ErrorKind::LauncherError(format!(
            "Invalid game version: {game_version}"
        )))?;

    Ok((refreshed, idx))
}

async fn get_instance_full_path(
    instance_path: &str,
    game_dir_override: Option<&str>,
) -> crate::Result<PathBuf> {
    let state = State::get().await?;
    let full_path = io::canonicalize(
        state
            .directories
            .resolve_game_dir(instance_path, game_dir_override),
    )?;
    Ok(full_path)
}

async fn promote_external_instance_link(
    instance: &Instance,
    state: &State,
) -> crate::Result<()> {
    // A symlink import can also carry a version-isolated game-dir override,
    // but it still owns a managed profile entry. Promoting it would make the
    // direct-link reconciler treat the external root as authoritative and
    // eventually delete the imported instance when that root is not listed in
    // Settings.
    if !external_link_promotion_allowed(
        instance.is_direct_linked(),
        instance.symlink_target.as_deref(),
    ) {
        return Ok(());
    }
    let Some(direct) = DirectLinkedLaunch::from_game_dir_override(instance)?
    else {
        return Ok(());
    };
    let game_dir_mode = direct.game_dir_mode.ok_or_else(|| {
        crate::ErrorKind::LauncherError(
            "External instance link has no game-directory mode".to_string(),
        )
    })?;
    let root = direct.dot_minecraft.to_string_lossy().to_string();
    let version_json = direct
        .version_json
        .as_deref()
        .map(io::canonicalize)
        .transpose()?
        .map(|path| path.to_string_lossy().to_string());
    let mut tx = state.pool.begin().await?;
    crate::state::instances::adapters::sqlite::instance_rows::set_direct_link_fields(
        &instance.id,
        &crate::state::instances::adapters::sqlite::instance_rows::DirectLinkFields {
            launcher: Some("generic".to_string()),
            launcher_root: Some(root.clone()),
            dot_minecraft: Some(root.clone()),
            version_id: Some(direct.version_id.clone()),
            version_json_path: version_json,
            game_dir_mode: Some(game_dir_mode.key().to_string()),
        },
        &mut tx,
    )
    .await?;
    if let Some(group) = direct_link_group(&direct.dot_minecraft) {
        crate::state::instances::adapters::sqlite::instance_rows::replace_instance_groups(
            &instance.id,
            &[group],
            &mut tx,
        )
        .await?;
    }
    tx.commit().await?;
    emit_instance(&instance.id, InstancePayloadType::Edited).await?;
    Ok(())
}

fn external_link_promotion_allowed(
    is_direct_linked: bool,
    symlink_target: Option<&str>,
) -> bool {
    !is_direct_linked && symlink_target.is_none()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceCompletionPolicy {
    FinalizeHere,
    DeferToInstallJob,
}

/// Serializes the small SQLite mutations performed by Minecraft core setup
/// with modpack content registration. Network transfers and loader
/// processors stay outside this guard. Cancellation can stop the semaphore
/// wait, but an operation that has started is driven to a definitive result
/// so callers never have to guess whether its transaction committed.
async fn run_install_database_write<T>(
    semaphore: &tokio::sync::Semaphore,
    cancellation: &CancellationToken,
    operation: impl std::future::Future<Output = crate::Result<T>>,
) -> crate::Result<T> {
    let _permit = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(crate::ErrorKind::OtherError(
                "Minecraft install database write canceled".to_string(),
            ).into());
        }
        permit = semaphore.acquire() => permit.map_err(|_| {
            crate::ErrorKind::OtherError(
                "install database semaphore closed".to_string(),
            )
        })?,
    };
    let result = operation.await;
    if result.is_ok() && cancellation.is_cancelled() {
        return Err(crate::ErrorKind::OtherError(
            "Minecraft install database write canceled".to_string(),
        )
        .into());
    }
    result
}

#[cfg(test)]
mod install_database_write_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn minecraft_database_operation_starts_only_after_permit() {
        let semaphore = Semaphore::new(0);
        let cancellation = CancellationToken::new();
        let operation_polled = Arc::new(AtomicBool::new(false));
        let operation_flag = Arc::clone(&operation_polled);

        let result = tokio::time::timeout(
            Duration::from_millis(50),
            run_install_database_write(&semaphore, &cancellation, async move {
                operation_flag.store(true, Ordering::SeqCst);
                Ok::<(), crate::Error>(())
            }),
        )
        .await;

        assert!(result.is_err());
        assert!(!operation_polled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn minecraft_database_wait_is_cancelable() {
        let semaphore = Semaphore::new(0);
        let cancellation = CancellationToken::new();
        let trigger = cancellation.clone();
        let cancel_task = tokio::spawn(async move {
            tokio::task::yield_now().await;
            trigger.cancel();
        });

        let error = tokio::time::timeout(
            Duration::from_millis(100),
            run_install_database_write(&semaphore, &cancellation, async {
                Ok::<(), crate::Error>(())
            }),
        )
        .await
        .expect("database semaphore wait should stop on cancellation")
        .unwrap_err();
        cancel_task.await.unwrap();

        assert!(error.to_string().contains("canceled"));
    }

    #[tokio::test]
    async fn minecraft_database_operation_runs_with_permit() {
        let semaphore = Semaphore::new(1);
        let cancellation = CancellationToken::new();
        let operation_polled = Arc::new(AtomicBool::new(false));
        let operation_flag = Arc::clone(&operation_polled);

        run_install_database_write(&semaphore, &cancellation, async move {
            operation_flag.store(true, Ordering::SeqCst);
            Ok::<(), crate::Error>(())
        })
        .await
        .unwrap();

        assert!(operation_polled.load(Ordering::SeqCst));
        assert_eq!(semaphore.available_permits(), 1);
    }

    #[tokio::test]
    async fn minecraft_database_cancellation_waits_for_started_operation() {
        let semaphore = Arc::new(Semaphore::new(1));
        let cancellation = CancellationToken::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
        let task_semaphore = Arc::clone(&semaphore);
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            run_install_database_write(
                &task_semaphore,
                &task_cancellation,
                async move {
                    let _ = started_tx.send(());
                    let _ = finish_rx.await;
                    Ok::<(), crate::Error>(())
                },
            )
            .await
        });

        started_rx.await.unwrap();
        cancellation.cancel();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!task.is_finished());

        finish_tx.send(()).unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("canceled"));
        assert_eq!(semaphore.available_permits(), 1);
    }
}

pub(crate) async fn run_instance_install_command(
    instance_id: String,
    cancellation: CancellationToken,
    mut command: Command,
) -> crate::Result<Output> {
    let state = State::get().await?;
    let instance_lock =
        state.lock_instance_content_exclusive(&instance_id).await;
    let task = tokio::spawn(async move {
        let _instance_lock = instance_lock;
        if cancellation.is_cancelled() {
            return Err(crate::ErrorKind::OtherError(
                "Install was canceled".to_string(),
            )
            .into());
        }
        command
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_task = tokio::spawn(async move {
            let mut output = Vec::new();
            if let Some(mut stdout) = stdout {
                stdout.read_to_end(&mut output).await?;
            }
            Ok::<_, std::io::Error>(output)
        });
        let stderr_task = tokio::spawn(async move {
            let mut output = Vec::new();
            if let Some(mut stderr) = stderr {
                stderr.read_to_end(&mut output).await?;
            }
            Ok::<_, std::io::Error>(output)
        });

        let (status, canceled) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                let _ = child.start_kill();
                (child.wait().await?, true)
            }
            status = child.wait() => (status?, false),
        };
        let stdout = stdout_task.await.map_err(std::io::Error::other)??;
        let stderr = stderr_task.await.map_err(std::io::Error::other)??;
        if canceled {
            return Err(crate::ErrorKind::OtherError(
                "Install was canceled".to_string(),
            )
            .into());
        }
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    });

    task.await?
}

pub async fn install_minecraft_with_reporter(
    context: &InstanceLaunchContext,
    repairing: bool,
    reporter: Option<InstallProgressReporter>,
    completion_policy: InstanceCompletionPolicy,
) -> crate::Result<()> {
    install_minecraft_with_local_source(
        context,
        repairing,
        reporter,
        None,
        completion_policy,
    )
    .await
}

async fn install_minecraft_with_local_source(
    context: &InstanceLaunchContext,
    repairing: bool,
    reporter: Option<InstallProgressReporter>,
    local_source: Option<&LocalRuntimeSource>,
    completion_policy: InstanceCompletionPolicy,
) -> crate::Result<()> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let instance = &context.instance;
    let content_set = &context.applied_content_set;
    let phase_details = InstallPhaseDetails::Minecraft {
        game_version: content_set.game_version.clone(),
        loader: content_set.loader,
    };
    let loading_bar = if reporter.is_none() {
        Some(
            init_loading(
                LoadingBarType::MinecraftDownload {
                    // If we are downloading minecraft for a profile, provide its name and uuid
                    instance_name: instance.name.clone(),
                    instance_id: instance.id.clone(),
                },
                100.0,
                "Downloading Minecraft",
            )
            .await?,
        )
    } else {
        None
    };

    let state = State::get().await?;
    let database_cancellation = reporter
        .as_ref()
        .map(InstallProgressReporter::cancellation_token)
        .unwrap_or_default();

    run_install_database_write(
        &state.install_db_semaphore,
        &database_cancellation,
        crate::state::instances::commands::set_instance_install_stage(
            &instance.id,
            InstanceInstallStage::MinecraftInstalling,
            &state.pool,
        ),
    )
    .await?;
    emit_instance(&instance.id, InstancePayloadType::Edited).await?;

    let instance_path = io::canonicalize(
        InstanceRuntimeAdapter::for_instance(instance, &state.directories)?
            .game_dir(),
    )?;
    if let Some(reporter) = &reporter {
        reporter
            .update(
                InstallPhaseId::ResolvingMinecraft,
                None,
                phase_details.clone(),
            )
            .await?;
    }
    let (minecraft, version_index) =
        resolve_minecraft_manifest(&content_set.game_version, &state).await?;
    let version = &minecraft.versions[version_index];
    let minecraft_updated = version_index
        <= minecraft
            .versions
            .iter()
            .position(|x| x.id == "22w16a")
            .unwrap_or(0);

    if content_set.loader != ModLoader::Vanilla
        && let Some(reporter) = &reporter
    {
        reporter
            .update(
                InstallPhaseId::ResolvingLoader,
                None,
                phase_details.clone(),
            )
            .await?;
    }

    let mmc_profile = multimc::MultiMcProfile::load(
        &instance_path,
        &content_set.game_version,
        content_set.loader,
        content_set.loader_version.as_deref(),
    )
    .await?;
    let metadata_loader =
        mmc_profile.as_ref().map_or(content_set.loader, |profile| {
            profile.metadata_loader(content_set.loader)
        });
    let mut loader_version = get_loader_version_from_profile(
        &content_set.game_version,
        metadata_loader,
        content_set.loader_version.as_deref(),
    )
    .await?;

    // If no loader version is selected, try to select the stable version!
    if metadata_loader != ModLoader::Vanilla && loader_version.is_none() {
        let fallback = missing_loader_version_fallback(
            content_set.loader,
            &content_set.game_version,
            content_set.loader_version.as_deref(),
        )
        .map_err(crate::ErrorKind::LauncherError)?;
        loader_version = get_loader_version_from_profile(
            &content_set.game_version,
            content_set.loader,
            Some(fallback),
        )
        .await?;

        run_install_database_write(
            &state.install_db_semaphore,
            &database_cancellation,
            crate::state::instances::commands::set_applied_content_set_loader_version(
                &instance.id,
                loader_version.as_ref().map(|x| x.id.as_str()),
                &state.pool,
            ),
        )
        .await?;
    }

    let version_jar =
        loader_version.as_ref().map_or(version.id.clone(), |it| {
            format!("{}-{}", version.id.clone(), it.id.clone())
        });

    // Download version info (5)
    let mut version_info = download::download_version_info(
        &state,
        version,
        metadata_loader,
        loader_version.as_ref(),
        Some(repairing),
        loading_bar.as_ref(),
        reporter.as_ref(),
    )
    .await?;

    if let Some(profile) = &mmc_profile {
        let vanilla = if profile.replaces_minecraft()
            && metadata_loader != ModLoader::Vanilla
        {
            Some(
                download::download_version_info(
                    &state,
                    version,
                    ModLoader::Vanilla,
                    None,
                    None,
                    None,
                    None,
                )
                .await?,
            )
        } else {
            None
        };
        profile
            .apply(
                &mut version_info,
                vanilla.as_ref(),
                &state.directories.libraries_dir(),
            )
            .await?;
    }
    let version_jar =
        mmc_profile.as_ref().map_or(version_jar.clone(), |profile| {
            profile.cache_key(&version_jar)
        });

    let key = required_java_major(&version_info);
    if let Some(reporter) = &reporter {
        reporter
            .update(
                InstallPhaseId::PreparingJava,
                Some(InstallProgress {
                    current: 0,
                    total: 4,
                    secondary: None,
                }),
                InstallPhaseDetails::Java {
                    major_version: key,
                    step: InstallJavaStep::Resolving,
                },
            )
            .await?;
    }
    let java_installation = if let Some(java_version) =
        get_java_version_from_launch_context(context, &version_info).await?
    {
        Some((std::path::PathBuf::from(java_version.path), false))
    } else if let Some(discovered) =
        crate::api::jre::find_java_for_version(key).await?
    {
        tracing::info!(
            "Reusing discovered Java {} at {}",
            discovered.version,
            discovered.path
        );
        Some((std::path::PathBuf::from(discovered.path), true))
    } else {
        if let Some(reporter) = &reporter {
            crate::api::jre::auto_install_java_with_reporter(
                key,
                reporter.clone(),
            )
            .await?
        } else {
            crate::api::jre::auto_install_java_with_loading(key, true).await?
        }
        .map(|path| (path, true))
    };

    let java_version = if let Some((java_path, set_java)) = java_installation {
        if let Some(reporter) = &reporter {
            reporter
                .update(
                    InstallPhaseId::PreparingJava,
                    Some(InstallProgress {
                        current: 4,
                        total: 4,
                        secondary: None,
                    }),
                    InstallPhaseDetails::Java {
                        major_version: key,
                        step: InstallJavaStep::Validating,
                    },
                )
                .await?;
        }
        let java_version = crate::api::jre::check_jre(java_path).await?;
        validate_loader_java_version(&version_info, &java_version)?;

        if set_java {
            let documents = crate::state::documents::Documents::new(&state);
            run_install_database_write(
                &state.install_db_semaphore,
                &database_cancellation,
                java_version.upsert(&documents),
            )
            .await?;
        }

        Some(java_version)
    } else {
        None
    };

    // Download minecraft (5-90)
    if let Some(reporter) = &reporter {
        reporter
            .update(
                InstallPhaseId::DownloadingMinecraft,
                None,
                phase_details.clone(),
            )
            .await?;
    }
    let minecraft_progress = download::download_minecraft(
        &state,
        local_source,
        &content_set.game_version,
        &version_info,
        loading_bar.as_ref(),
        java_version
            .as_ref()
            .map(|java| java.architecture.as_str())
            .unwrap_or(std::env::consts::ARCH),
        repairing,
        minecraft_updated,
        reporter.clone(),
        phase_details.clone(),
    )
    .await?;

    let mut client_path = state
        .directories
        .version_dir(&version_jar)
        .join(format!("{version_jar}.jar"));

    if let Some(profile) = &mmc_profile {
        client_path = profile
            .prepare_runtime(
                &state,
                &client_path,
                java_version
                    .as_ref()
                    .map(|java| java.architecture.as_str())
                    .unwrap_or(std::env::consts::ARCH),
                minecraft_updated,
                false,
            )
            .await?
            .assemble(&client_path, &instance_path)
            .await?;
    }

    let Some(java_version) = java_version else {
        if content_set.loader == ModLoader::OptiFine
            || has_client_processors(version_info.processors.as_deref())
        {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Java {key} is required to finish installing {}",
                content_set.loader.as_str()
            ))
            .into());
        }
        external_version::finalize_external_version(
            instance,
            &version_jar,
            &content_set.game_version,
            &version_info,
            &state,
            std::env::consts::ARCH,
            minecraft_updated,
            minecraft_progress.as_ref(),
            &database_cancellation,
        )
        .await?;
        if let Some(progress) = minecraft_progress {
            progress.finish().await?;
        }
        let protocol_version =
            read_protocol_version_from_jar(client_path).await?;
        run_install_database_write(
            &state.install_db_semaphore,
            &database_cancellation,
            crate::state::instances::commands::set_applied_content_set_protocol_version(
                &instance.id,
                protocol_version,
                &state.pool,
            ),
        )
        .await?;
        run_install_database_write(
            &state.install_db_semaphore,
            &database_cancellation,
            promote_external_instance_link(instance, &state),
        )
        .await?;
        if completion_policy == InstanceCompletionPolicy::FinalizeHere {
            run_install_database_write(
                &state.install_db_semaphore,
                &database_cancellation,
                crate::state::instances::commands::set_instance_install_stage(
                    &instance.id,
                    InstanceInstallStage::Installed,
                    &state.pool,
                ),
            )
            .await?;
            emit_instance(&instance.id, InstancePayloadType::Edited).await?;
        }
        if let Some(loading_bar) = &loading_bar {
            emit_loading(
                loading_bar,
                1.0,
                Some("Finished downloading Minecraft resources"),
            )?;
        }
        tracing::info!(
            java_version = key,
            instance_id = instance.id,
            "Postponed Java setup after downloading Minecraft resources"
        );
        return Ok(());
    };

    // Loader processors are separate JVMs. Pass the launcher's configured
    // custom proxy explicitly because child Java processes do not inherit
    // reqwest's proxy configuration.
    let java_proxy_args = state.proxy_config().await?.java_args();

    if content_set.loader == ModLoader::OptiFine
        && let Some(loader_version) = &loader_version
    {
        if let Some(reporter) = &reporter {
            reporter
                .update(
                    InstallPhaseId::RunningLoaderProcessors,
                    None,
                    phase_details.clone(),
                )
                .await?;
        }
        optifine::install_optifine_libraries(
            &state,
            std::path::Path::new(&java_version.path),
            &content_set.game_version,
            &loader_version.id,
            &client_path,
            repairing,
            &database_cancellation,
        )
        .await?;
    }

    if has_client_processors(version_info.processors.as_deref())
        && version_info.data.is_none()
    {
        version_info.data = Some(Default::default());
    }
    if let Some(processors) = &version_info.processors {
        let libraries_dir = state.directories.libraries_dir();
        let client_mappings = version_info
            .downloads
            .get(&d::minecraft::DownloadType::ClientMappings)
            .map(|download| {
                (download.url.clone(), download.sha1.clone(), download.size)
            });

        if let Some(ref mut data) = version_info.data {
            processor_rules! {
                data;
                "SIDE":
                    client => "client",
                    server => "";
                "MINECRAFT_JAR" :
                    client => client_path.to_string_lossy(),
                    server => "";
                "MINECRAFT_VERSION":
                    client => content_set.game_version.clone(),
                    server => "";
                "ROOT":
                    client => instance_path.to_string_lossy(),
                    server => "";
                "LIBRARY_DIR":
                    client => libraries_dir.to_string_lossy(),
                    server => "";
            }

            if let Some(loading_bar) = &loading_bar {
                emit_loading(
                    loading_bar,
                    0.0,
                    Some("Running forge processors"),
                )?;
            }
            let total_length = processors.len();
            if let Some(reporter) = &reporter {
                reporter
                    .update(
                        InstallPhaseId::RunningLoaderProcessors,
                        Some(InstallProgress {
                            current: 0,
                            total: total_length as u64,
                            secondary: None,
                        }),
                        phase_details.clone(),
                    )
                    .await?;
            }

            // Forge processors (90-100)
            for (index, processor) in processors.iter().enumerate() {
                let processor_started = Instant::now();
                if let Some(sides) = &processor.sides
                    && !sides.contains(&String::from("client"))
                {
                    if let Some(reporter) = &reporter {
                        reporter
                            .update(
                                InstallPhaseId::RunningLoaderProcessors,
                                Some(InstallProgress {
                                    current: (index + 1) as u64,
                                    total: total_length as u64,
                                    secondary: None,
                                }),
                                phase_details.clone(),
                            )
                            .await?;
                    }
                    continue;
                }
                if download_mojmaps_for_processor(
                    &state,
                    processor,
                    client_mappings.as_ref(),
                    &libraries_dir,
                    data,
                )
                .await?
                {
                    if let Some(loading_bar) = &loading_bar {
                        emit_loading(
                            loading_bar,
                            30.0 / total_length as f64,
                            Some(&format!(
                                "Running forge processor {index}/{total_length}"
                            )),
                        )?;
                    }
                    if let Some(reporter) = &reporter {
                        reporter
                            .update(
                                InstallPhaseId::RunningLoaderProcessors,
                                Some(InstallProgress {
                                    current: (index + 1) as u64,
                                    total: total_length as u64,
                                    secondary: None,
                                }),
                                phase_details.clone(),
                            )
                            .await?;
                    }
                    continue;
                }
                if processor_outputs_are_current(
                    processor,
                    &libraries_dir,
                    data,
                    &[&instance_path, &libraries_dir],
                )
                .await
                {
                    tracing::info!(
                        processor = %processor.jar,
                        index,
                        duration_ms = processor_started.elapsed().as_millis(),
                        "Skipped Forge processor because all declared outputs are valid"
                    );
                    if let Some(loading_bar) = &loading_bar {
                        emit_loading(
                            loading_bar,
                            30.0 / total_length as f64,
                            Some(&format!(
                                "Running forge processor {index}/{total_length}"
                            )),
                        )?;
                    }
                    if let Some(reporter) = &reporter {
                        reporter
                            .update(
                                InstallPhaseId::RunningLoaderProcessors,
                                Some(InstallProgress {
                                    current: (index + 1) as u64,
                                    total: total_length as u64,
                                    secondary: None,
                                }),
                                phase_details.clone(),
                            )
                            .await?;
                    }
                    continue;
                }

                let cp = {
                    let mut cp = processor.classpath.clone();
                    cp.push(processor.jar.clone());
                    cp
                };

                let mut command =
                    java_environment::tokio_command(&java_version.path);
                command
                    .args(&java_proxy_args)
                    .arg("-cp")
                    .arg(args::get_class_paths_jar(
                        &libraries_dir,
                        &cp,
                        &java_version.architecture,
                    )?)
                    .arg(
                        args::get_processor_main_class(args::get_lib_path(
                            &libraries_dir,
                            &processor.jar,
                            false,
                        )?)
                        .await?
                        .ok_or_else(|| {
                            crate::ErrorKind::LauncherError(format!(
                                "Could not find processor main class for {}",
                                processor.jar
                            ))
                        })?,
                    )
                    .args(args::get_processor_arguments(
                        &libraries_dir,
                        &processor.args,
                        data,
                    )?)
                    // Forge's installer processors resolve a few auxiliary
                    // paths relative to the game root. Running them from the
                    // launcher process directory can therefore exit
                    // successfully while leaving the client output jars in
                    // the wrong place (or not producing them at all).
                    .current_dir(&instance_path);
                let child = run_instance_install_command(
                    instance.id.clone(),
                    reporter
                        .as_ref()
                        .map(InstallProgressReporter::cancellation_token)
                        .unwrap_or_default(),
                    command,
                )
                .await
                .map_err(|err| {
                    crate::ErrorKind::LauncherError(format!(
                        "Error running processor: {err}",
                    ))
                })?;

                if !child.status.success() {
                    return Err(crate::ErrorKind::LauncherError(format!(
                        "Processor error: {}",
                        String::from_utf8_lossy(&child.stderr)
                    ))
                    .as_error());
                }
                tracing::info!(
                    processor = %processor.jar,
                    index,
                    duration_ms = processor_started.elapsed().as_millis(),
                    "Completed Forge processor"
                );

                if let Some(loading_bar) = &loading_bar {
                    emit_loading(
                        loading_bar,
                        30.0 / total_length as f64,
                        Some(&format!(
                            "Running forge processor {index}/{total_length}"
                        )),
                    )?;
                }
                if let Some(reporter) = &reporter {
                    reporter
                        .update(
                            InstallPhaseId::RunningLoaderProcessors,
                            Some(InstallProgress {
                                current: (index + 1) as u64,
                                total: total_length as u64,
                                secondary: None,
                            }),
                            phase_details.clone(),
                        )
                        .await?;
                }
            }
        }
    }

    external_version::finalize_external_version(
        instance,
        &version_jar,
        &content_set.game_version,
        &version_info,
        &state,
        &java_version.architecture,
        minecraft_updated,
        minecraft_progress.as_ref(),
        &database_cancellation,
    )
    .await?;
    if let Some(progress) = minecraft_progress {
        progress.finish().await?;
    }
    let protocol_version = read_protocol_version_from_jar(client_path).await?;

    run_install_database_write(
        &state.install_db_semaphore,
        &database_cancellation,
        crate::state::instances::commands::set_applied_content_set_protocol_version(
            &instance.id,
            protocol_version,
            &state.pool,
        ),
    )
    .await?;
    run_install_database_write(
        &state.install_db_semaphore,
        &database_cancellation,
        promote_external_instance_link(instance, &state),
    )
    .await?;
    if completion_policy == InstanceCompletionPolicy::FinalizeHere {
        run_install_database_write(
            &state.install_db_semaphore,
            &database_cancellation,
            crate::state::instances::commands::set_instance_install_stage(
                &instance.id,
                InstanceInstallStage::Installed,
                &state.pool,
            ),
        )
        .await?;
        emit_instance(&instance.id, InstancePayloadType::Edited).await?;
    }
    if let Some(loading_bar) = &loading_bar {
        emit_loading(loading_bar, 1.0, Some("Finished installing"))?;
    }

    Ok(())
}

pub async fn install_minecraft_for_instance_id_with_reporter(
    instance_id: &str,
    repairing: bool,
    reporter: Option<InstallProgressReporter>,
    completion_policy: InstanceCompletionPolicy,
) -> crate::Result<()> {
    let state = State::get().await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to install a nonexistent or unloaded instance {instance_id}!"
            ))
        })?;

    install_minecraft_with_reporter(
        &context,
        repairing,
        reporter,
        completion_policy,
    )
    .await
}

pub async fn install_minecraft_for_instance_id_with_local_source(
    instance_id: &str,
    local_source: Option<LocalRuntimeSource>,
    repairing: bool,
    reporter: Option<InstallProgressReporter>,
    completion_policy: InstanceCompletionPolicy,
) -> crate::Result<()> {
    let state = State::get().await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to install a nonexistent or unloaded instance {instance_id}!"
            ))
        })?;

    install_minecraft_with_local_source(
        &context,
        repairing,
        reporter,
        local_source.as_ref(),
        completion_policy,
    )
    .await
}

pub async fn read_protocol_version_from_jar(
    path: PathBuf,
) -> crate::Result<Option<u32>> {
    let zip = async_zip::tokio::read::fs::ZipFileReader::new(path).await?;
    let Some(entry_index) = zip
        .file()
        .entries()
        .iter()
        .position(|x| matches!(x.filename().as_str(), Ok("version.json")))
    else {
        return Ok(None);
    };

    #[derive(Deserialize, Debug)]
    struct VersionData {
        protocol_version: Option<u32>,
    }

    let mut data = vec![];
    zip.reader_with_entry(entry_index)
        .await?
        .read_to_end_checked(&mut data)
        .await?;
    let data: VersionData = serde_json::from_slice(&data)?;

    Ok(data.protocol_version)
}

/// Selects a Java runtime from the HMCL private settings of a directly
/// associated instance, if one is configured and usable.
async fn select_hmcl_java(
    direct: &DirectLinkedLaunch,
    settings: &crate::launcher::local_version::HmclVersionSettings,
) -> Option<JavaVersion> {
    select_linked_java(hmcl_java_candidates(settings, &direct.dot_minecraft))
        .await
}

/// Selects a Java runtime from the PCL/PCL-CE per-instance Java preference of
/// a directly associated instance (see
/// `PclLaunchSettings::java_candidates`).
async fn select_pcl_java(
    direct: &DirectLinkedLaunch,
    settings: &direct_link::PclLaunchSettings,
) -> Option<JavaVersion> {
    select_linked_java(settings.java_candidates(
        &direct.version_dir(),
        direct.launcher_root.as_deref(),
    ))
    .await
}

/// Verifies linked-launcher Java candidates in order and returns the first
/// usable one.
async fn select_linked_java(candidates: Vec<PathBuf>) -> Option<JavaVersion> {
    for candidate in candidates {
        match crate::api::jre::check_jre(candidate).await {
            Ok(java) => {
                tracing::info!(
                    java = %java.path,
                    "Using the Java runtime configured by the linked launcher"
                );
                return Some(java);
            }
            Err(error) => {
                tracing::debug!(
                    %error,
                    "Skipping unusable linked-launcher Java candidate"
                );
            }
        }
    }

    None
}

/// Resolves the game directory a directly associated instance actually plays
/// from. The selected runtime adapter preserves each external launcher's
/// shared or version-isolated path semantics. `None` when the instance is not
/// directly associated or its linked metadata is incomplete.
///
/// Content browsing (mods/worlds listing) uses this so it reads the same
/// folders the launch does. If a version chain cannot be resolved, the
/// selected runtime adapter supplies a mode-appropriate external fallback.
pub(crate) fn linked_game_dir(instance: &Instance) -> Option<PathBuf> {
    let adapter =
        InstanceRuntimeAdapter::external_for_instance(instance).ok()??;
    Some(adapter.game_dir())
}

fn link_project_and_version(
    link: &InstanceLink,
) -> (Option<&String>, Option<&String>) {
    match link {
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        }
        | InstanceLink::CurseForgeModpack {
            project_id,
            version_id,
        } => (Some(project_id), Some(version_id)),
        InstanceLink::ServerProject { project_id } => (Some(project_id), None),
        InstanceLink::ServerProjectModpack {
            server_project_id,
            content_version_id,
            ..
        } => (Some(server_project_id), Some(content_version_id)),
        InstanceLink::ImportedModpack {
            project_id,
            version_id,
            ..
        } => (project_id.as_ref(), version_id.as_ref()),
        InstanceLink::Unmanaged | InstanceLink::SharedInstance { .. } => {
            (None, None)
        }
    }
}

#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub async fn launch_minecraft(
    java_args: &[String],
    env_args: &[(String, String)],
    mc_set_options: &[(String, String)],
    wrapper: &Option<String>,
    memory: &MemorySettings,
    resolution: &WindowSize,
    maximize_window: bool,
    window_title: Option<String>,
    launch_preparation_timeout: u64,
    credentials: &Credentials,
    post_exit_hook: Option<String>,
    context: &InstanceLaunchContext,
    gc_intent: Option<crate::launcher::jvm_args::GcLaunchIntent>,
    gc_report: &mut Option<crate::launcher::jvm_args::GcLaunchReport>,
    mut quick_play_type: QuickPlayType,
    offline_mode: bool,
) -> crate::Result<ProcessMetadata> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let instance = &context.instance;
    let content_set = &context.applied_content_set;

    if instance.install_stage == InstanceInstallStage::PackInstalling
        || instance.install_stage == InstanceInstallStage::MinecraftInstalling
    {
        return Err(crate::ErrorKind::LauncherError(
            "Instance is still installing".to_string(),
        )
        .into());
    }

    if instance.install_stage != InstanceInstallStage::Installed {
        return Err(crate::ErrorKind::LauncherError(
            "Instance is not installed; start an install job first".to_string(),
        )
        .into());
    }

    let state = State::get().await?;

    let runtime =
        InstanceRuntimeAdapter::for_instance(instance, &state.directories)?;
    let direct_launch = runtime.direct_link().cloned();
    if direct_launch.is_some() && !instance.is_direct_linked() {
        // Instances created before direct-link metadata was introduced retain
        // their external `game_dir_override`. Their adapter above already
        // launches them in place; persist the resolved identity now so future
        // launches, content scans, and settings all use the same external
        // runtime. A persistence failure must not make an otherwise valid
        // existing instance unlaunchable.
        if let Err(error) =
            promote_external_instance_link(instance, &state).await
        {
            tracing::warn!(
                %error,
                instance = %instance.id,
                "Could not persist the migrated external instance link"
            );
        }
    }
    let mut resolved_linked = direct_launch
        .as_ref()
        .map(DirectLinkedLaunch::resolve)
        .transpose()?;
    // PCL/PCL-CE may isolate a version under versions/<id>; HMCL and generic
    // linked installations launch from the shared .minecraft root.
    let instance_path = if let Some(resolved) = &resolved_linked {
        io::canonicalize(&resolved.game_dir)?
    } else {
        get_instance_full_path(
            &instance.path,
            instance.game_dir_override.as_deref(),
        )
        .await?
    };

    let cache_behaviour = offline_mode.then_some(CacheBehaviour::CacheOnly);
    let mut linked_libraries = None;
    let mut mmc_profile = None;

    // PCL/PCL-CE expose per-instance launch preferences (Java, memory, custom
    // arguments) in their own config files; load them once for this launch.
    let pcl_launch = direct_launch
        .as_ref()
        .filter(|direct| {
            matches!(
                direct.dialect,
                LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe
            )
        })
        .map(DirectLinkedLaunch::pcl_launch_settings);

    let (
        version_info,
        minecraft_updated,
        version_jar,
        launch_version_id,
        launch_version_type,
        quick_play_version,
        hmcl_settings,
        launch_release_time,
    ) = if let Some(direct) = &direct_launch {
        // The dialect-specific resolver has already folded the linked manifest;
        // no version metadata is downloaded and no repair pass runs.
        let mut merged = resolved_linked
            .take()
            .expect("direct launch resolution accompanies direct metadata")
            .merged;
        // Same loader normalization the managed install path applies. The
        // merged list drives the ensure pass, native extraction, the launch
        // classpath, and the VersionInfo projection below, so dropping the
        // Cleanroom conflicts here — the whole vanilla LWJGL 2 family
        // (`org.lwjgl.lwjgl:lwjgl` binding, `lwjgl_util` at
        // 2.9.4-nightly-20150209, and the natives-only `lwjgl-platform`
        // carrier whose root-level `lwjgl.dll` would otherwise be what
        // `System.loadLibrary("lwjgl")` resolves first), plus the vanilla JNA
        // platform and Mojang ICU (pre-existing Axolotl Cleanroom rule) —
        // covers direct ensure, classpath, and launch at once; the Cleanroom
        // LWJGL 3 line, lwjglxx, and the vanilla JNA core are never affected.
        for removed_library in
            normalize_merged_loader_libraries(content_set.loader, &mut merged)
        {
            tracing::info!(
                loader = content_set.loader.as_str(),
                version = %merged.id,
                removed_library,
                "Removed loader-incompatible library from linked version profile"
            );
        }
        linked_libraries = Some(merged.libraries.clone());

        // Best-effort reuse of the shared metadata manifest for behaviors
        // keyed on the vanilla release order (Quick Play support, pre-1.13
        // library rules); unknown base versions fall back conservatively.
        let (minecraft_updated, quick_play_version) =
            match resolve_minecraft_manifest_with_cache(
                &content_set.game_version,
                &state,
                cache_behaviour,
            )
            .await
            {
                Ok((minecraft, version_index)) => (
                    version_index
                        <= minecraft
                            .versions
                            .iter()
                            .position(|x| x.id == "22w16a")
                            .unwrap_or(0),
                    QuickPlayVersion::find_version(
                        version_index,
                        &minecraft.versions,
                    ),
                ),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        version = %content_set.game_version,
                        "Linked version not found in Minecraft metadata; \
                         using conservative launch defaults"
                    );
                    conservative_launch_facts(&merged)
                }
            };

        let version_jar = merged
            .jar
            .clone()
            .unwrap_or_else(|| content_set.game_version.clone());
        let hmcl_settings = std::mem::take(&mut merged.hmcl_settings);
        let version_info = merged_to_version_info(merged)?;
        let launch_version_type = version_info.type_.clone();

        (
            version_info,
            minecraft_updated,
            version_jar,
            direct.version_id.clone(),
            launch_version_type,
            quick_play_version,
            hmcl_settings,
            // Unused: the options.txt language pass never runs for directly
            // associated instances.
            chrono::DateTime::<Utc>::MIN_UTC,
        )
    } else {
        let cached_minecraft = resolve_minecraft_manifest_with_cache(
            &content_set.game_version,
            &state,
            Some(CacheBehaviour::CacheOnly),
        )
        .await;
        let (minecraft, version_index) = match cached_minecraft {
            Ok(value) => value,
            Err(error) if offline_mode => return Err(error),
            Err(_) => {
                resolve_minecraft_manifest_with_cache(
                    &content_set.game_version,
                    &state,
                    cache_behaviour,
                )
                .await?
            }
        };
        let version = &minecraft.versions[version_index];
        let minecraft_updated = version_index
            <= minecraft
                .versions
                .iter()
                .position(|x| x.id == "22w16a")
                .unwrap_or(0);

        mmc_profile = multimc::MultiMcProfile::load(
            &instance_path,
            &content_set.game_version,
            content_set.loader,
            content_set.loader_version.as_deref(),
        )
        .await?;
        let metadata_loader =
            mmc_profile.as_ref().map_or(content_set.loader, |profile| {
                profile.metadata_loader(content_set.loader)
            });
        let (loader_version, mut version_info) = load_managed_launch_profile(
            &state,
            version,
            metadata_loader,
            content_set.loader_version.as_deref(),
            offline_mode,
        )
        .await?;

        let version_jar =
            loader_version.as_ref().map_or(version.id.clone(), |it| {
                format!("{}-{}", version.id.clone(), it.id.clone())
            });

        if version_info.logging.is_none() {
            let requires_logging_info = version_index
                <= minecraft
                    .versions
                    .iter()
                    .position(|x| x.id == "13w39a")
                    .unwrap_or(0);
            if requires_logging_info && !offline_mode {
                // Logging belongs to the base game; recovering it must not
                // reintroduce an online loader catalog dependency.
                let (_, vanilla) = load_managed_launch_profile(
                    &state,
                    version,
                    ModLoader::Vanilla,
                    None,
                    false,
                )
                .await?;
                version_info.logging = vanilla.logging;
            }
        }

        if let Some(profile) = &mmc_profile {
            let vanilla = if profile.replaces_minecraft()
                && metadata_loader != ModLoader::Vanilla
            {
                Some(if offline_mode {
                    download::load_local_version_info(
                        &state,
                        version,
                        ModLoader::Vanilla,
                        None,
                    )
                    .await?
                } else {
                    download::download_version_info(
                        &state,
                        version,
                        ModLoader::Vanilla,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await?
                })
            } else {
                None
            };
            profile
                .apply(
                    &mut version_info,
                    vanilla.as_ref(),
                    &state.directories.libraries_dir(),
                )
                .await?;
        }
        let version_jar =
            mmc_profile.as_ref().map_or(version_jar.clone(), |profile| {
                profile.cache_key(&version_jar)
            });

        let quick_play_version =
            QuickPlayVersion::find_version(version_index, &minecraft.versions);
        let launch_version_id = version.id.clone();
        let launch_release_time = version.release_time;

        (
            version_info,
            minecraft_updated,
            version_jar,
            launch_version_id,
            version.type_.clone(),
            quick_play_version,
            crate::launcher::local_version::HmclVersionSettings::default(),
            launch_release_time,
        )
    };

    if direct_launch.is_none() {
        if offline_mode {
            download::ensure_local_log_config(&state, &version_info)?;
        } else {
            let _ = download_log_config(
                &state,
                None,
                &version_info,
                None,
                false,
                None,
            )
            .await?;
        }
    }

    let java_version = if let Some(java) =
        context.launch_overrides.java_path.as_ref()
    {
        crate::api::jre::check_jre(std::path::PathBuf::from(java)).await?
    } else if direct_launch
        .as_ref()
        .is_some_and(|direct| direct.dialect == LinkedLauncherDialect::Hmcl)
        && let Some(java) = select_hmcl_java(
            direct_launch.as_ref().expect("checked above"),
            &hmcl_settings,
        )
        .await
    {
        // The linked launcher configured its own Java runtime for this
        // version; prefer it over Axolotl's discovery.
        java
    } else if let (Some(direct), Some(pcl)) =
        (direct_launch.as_ref(), pcl_launch.as_ref())
        && matches!(
            direct.dialect,
            LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe
        )
        && let Some(java) = select_pcl_java(direct, pcl).await
    {
        // The linked PCL/PCL-CE instance selected its own Java runtime;
        // prefer it over Axolotl's discovery.
        java
    } else {
        let key = required_java_major(&version_info);

        if let Some(java) = crate::api::jre::find_java_for_version(key).await? {
            java
        } else if let Some(java) =
            crate::api::jre::find_compatible_java_for_version(key).await?
        {
            tracing::info!(
                version = java.version,
                java = java.path,
                "Using a compatible Java runtime instead of the recommended version"
            );
            java
        } else {
            return Err(crate::ErrorKind::LauncherError(
                "Missing correct java installation".to_string(),
            )
            .into());
        }
    };

    // Test jre version
    let java_version =
        crate::api::jre::check_jre(java_version.path.clone().into()).await?;
    validate_loader_java_version(&version_info, &java_version)?;

    // Runtime-verify and fall back for GC arguments against the *actual* JVM
    // that will run Minecraft. The frontend supplies an ordered candidate
    // chain; we keep the preferred strategy only if this JVM understands it,
    // pruning unsupported tuning flags and falling back down the chain as
    // needed. A `None` keeps the args passed in untouched.
    let mut resolved_java_args = java_args.to_vec();
    // Custom JVM arguments from the linked launcher enter the same GC
    // compatibility pipeline below: unsupported tuning flags are pruned
    // against the actual JVM instead of reaching it unvalidated.
    if let Some(pcl) = &pcl_launch {
        resolved_java_args.extend(pcl.extra_jvm_args());
    }
    if let Some(gc_intent) = gc_intent {
        tracing::info!(
            java = %java_version.path,
            "Verifying GC arguments against the selected JVM",
        );
        let report = crate::launcher::jvm_args::resolve_gc_block(
            Path::new(java_version.path.as_str()),
            &mut resolved_java_args,
            &gc_intent,
        )
        .await;
        if report.fell_back() {
            tracing::info!(?report, "GC arguments adjusted for this JVM");
        }
        *gc_report = Some(report);
    }

    // Apply the mappable linked-launcher private settings for directly
    // associated instances on top of the resolved launch configuration.
    let mut effective_memory = *memory;
    let mut effective_resolution = *resolution;
    // Extra game arguments appended after the vanilla game arguments.
    let mut extra_game_args: Vec<String> = Vec::new();
    if let Some(direct) = direct_launch.as_ref()
        && direct.dialect == LinkedLauncherDialect::Hmcl
    {
        let hmcl_effective = hmcl_with_global_fallback(
            &hmcl_settings,
            direct.launcher_root.as_deref(),
        );
        extra_game_args.extend(apply_hmcl_settings(
            &hmcl_effective,
            &mut effective_memory,
            &mut effective_resolution,
            &mut resolved_java_args,
            Some(java_version.parsed_version),
        ));
    } else if let (Some(direct), Some(pcl)) =
        (direct_launch.as_ref(), pcl_launch.as_ref())
        && matches!(
            direct.dialect,
            LinkedLauncherDialect::Pcl | LinkedLauncherDialect::PclCe
        )
    {
        // PCL's RAM estimate needs the mod presence of the resolved game
        // directory and the bitness of the JVM chosen above.
        let (modable, opti_fine) =
            pcl_ram_profile(version_info.libraries.as_slice());
        let is_32_bit_java =
            crate::launcher::direct_link::linked_architecture_width(
                &java_version.architecture,
            ) == "32";
        if let Some(maximum) = pcl.resolve_ram_mb(
            &instance_path,
            modable,
            opti_fine,
            pcl_available_memory_gb(),
            is_32_bit_java,
        ) {
            tracing::info!(
                maximum,
                "Using the memory allocation configured by the linked PCL \
                 instance"
            );
            effective_memory.maximum = maximum;
        }
        extra_game_args.extend(pcl.extra_game_args());
    }

    let settings = crate::state::Settings::get(&state.data).await?;

    #[cfg(target_os = "windows")]
    if settings.auto_set_java_high_performance_mode {
        if let Err(error) =
            set_high_performance_gpu_preference(&java_version.path)
        {
            tracing::warn!(%error, java = %java_version.path, "Failed to set Java high-performance GPU preference");
        }

        match std::env::current_exe() {
            Ok(launcher_path) => {
                if let Err(error) =
                    set_high_performance_gpu_preference(&launcher_path)
                {
                    tracing::warn!(%error, launcher = %launcher_path.display(), "Failed to set launcher high-performance GPU preference");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Failed to determine launcher executable for high-performance GPU preference");
            }
        }
    }

    #[cfg(target_os = "linux")]
    let high_performance_gpu_environment =
        if settings.auto_set_java_high_performance_mode {
            high_performance_gpu_environment().await
        } else {
            Vec::new()
        };

    let client_path = if let Some(direct) = &direct_launch {
        // The game jar lives inside the linked installation.
        direct.client_jar(&version_jar)
    } else {
        let mut vanilla_client_path = state
            .directories
            .version_dir(&version_jar)
            .join(format!("{version_jar}.jar"));
        if let Some(profile) = &mmc_profile {
            vanilla_client_path = profile
                .prepare_runtime(
                    &state,
                    &vanilla_client_path,
                    &java_version.architecture,
                    minecraft_updated,
                    offline_mode,
                )
                .await?
                .assemble(&vanilla_client_path, &instance_path)
                .await?;
        }
        match crate::api::instance::assemble_for_launch(
            &instance_path,
            &content_set.game_version,
            &vanilla_client_path,
        )
        .await?
        {
            Some(assembled) => assembled,
            None => vanilla_client_path,
        }
    };

    let args = version_info.arguments.clone().unwrap_or_default();
    let mut command = match wrapper {
        Some(hook) => {
            let mut cmd = shlex::split(hook)
                .ok_or_else(|| {
                    crate::ErrorKind::LauncherError(format!(
                        "Invalid wrapper command: {hook}",
                    ))
                })?
                .into_iter();
            let mut command = Command::new(cmd.next().ok_or(
                crate::ErrorKind::LauncherError(
                    "Empty wrapper command".to_owned(),
                ),
            )?);
            command.args(cmd);
            command.arg(&java_version.path);
            command
        }
        None => Command::new(&java_version.path),
    };

    let env_args = Vec::from(env_args);

    // Check if instance has a running process, and reject running the command if it does
    // Done late so a quick double call doesn't launch two instances
    let existing_processes = process::get_by_instance_id(&instance.id).await?;
    if let Some(process) = existing_processes.first() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "Instance {} is already running as process {}",
            instance.id, process.uuid
        ))
        .as_error());
    }

    // Ensure phase (HMCL/PCL parity): before anything reads from the linked
    // installation, complete its missing libraries, assets, and logging
    // config in the standard shared locations. Version JSONs, launcher
    // private configuration, and game data are never written. Offline mode
    // forbids network fetches, so it keeps the previous strict behavior.
    if !offline_mode
        && let (Some(direct), Some(libraries)) =
            (&direct_launch, linked_libraries.as_deref())
    {
        direct_ensure::ensure_direct_launch_dependencies(
            &state,
            direct,
            libraries,
            &version_info,
            &java_version.architecture,
            minecraft_updated,
        )
        .await?;
    }

    let natives_dir = if let Some(direct) = &direct_launch {
        state
            .directories
            .version_natives_dir(&direct.natives_cache_key())
    } else {
        state.directories.version_natives_dir(&version_jar)
    };
    // Linked native archives can change outside Axolotl, so rebuild only the
    // Axolotl-owned linked cache on every launch. Never mutate linked folders.
    if direct_launch.is_some() && natives_dir.exists() {
        io::remove_dir_all(&natives_dir).await?;
    }
    if !natives_dir.exists() {
        io::create_dir_all(&natives_dir).await?;
    }
    if let (Some(direct), Some(libraries)) =
        (direct_launch.clone(), linked_libraries.clone())
    {
        // Linked instances rebuild the Axolotl-owned natives cache on every
        // launch; the managed restore path below must never touch them.
        let target = natives_dir.clone();
        let java_arch = java_version.architecture.clone();
        tokio::task::spawn_blocking(move || {
            extract_linked_natives(
                &direct,
                &libraries,
                &target,
                &java_arch,
                minecraft_updated,
            )
        })
        .await??;
    } else if direct_launch.is_none() {
        if offline_mode {
            natives::prepare_native_libraries(
                &state.directories.natives_dir(),
                &state.directories.libraries_dir(),
                &state.directories.caches_dir(),
                version_info.libraries.as_slice(),
                &version_jar,
                &java_version.architecture,
                minecraft_updated,
            )
            .await?;
        } else {
            download::download_libraries(
                &state,
                None,
                version_info.libraries.as_slice(),
                &version_jar,
                None,
                0.0,
                &java_version.architecture,
                false,
                minecraft_updated,
                None,
            )
            .await?;
        }
    }

    tracing::debug!(
        "Found QuickPlayVersion for {}: {quick_play_version:?}",
        content_set.game_version
    );
    if let QuickPlayType::Server(address) = &mut quick_play_type
        && quick_play_version.server >= QuickPlayServerVersion::BuiltinLegacy
    {
        // Record last-played for the original server address immediately so
        // recent-worlds can match without DNS/SRV resolution.
        let original = match address {
            ServerAddress::Unresolved(address) => parse_server_address(address)
                .ok()
                .map(|(h, p)| (h.to_owned(), p)),
            ServerAddress::Resolved {
                original_host,
                original_port,
                ..
            } => Some((original_host.clone(), *original_port)),
        };
        if let Some((host, port)) = original
            && let Err(e) = JoinLog::set(
                &state.data,
                &JoinLogKey {
                    instance_id: instance.id.clone(),
                    host,
                    port,
                },
                Some(&JoinLogEntry {
                    join_time: Utc::now(),
                }),
            )
            .await
        {
            tracing::warn!("Failed to write server join log entry: {e}");
        }

        address.resolve().await?;
    }

    let (main_class_keep_alive, main_class_path) =
        get_resource_file!(env "JAVA_JARS_DIR" / "theseus.jar")?;
    let yggdrasil_agent = if credentials.is_yggdrasil() {
        let account = credentials.yggdrasil.as_ref().ok_or_else(|| {
            crate::ErrorKind::LauncherError(
                "Yggdrasil account metadata is missing".to_string(),
            )
            .as_error()
        })?;
        let metadata =
            crate::state::fetch_yggdrasil_metadata(&account.api_root).await?;
        let agent_path =
            main_class_keep_alive.path().join("authlib-injector.jar");
        io::write(
            &agent_path,
            include_bytes!(concat!(
                env!("JAVA_JARS_DIR"),
                "/authlib-injector.jar"
            )),
        )
        .await?;
        Some((
            io::canonicalize(agent_path)?,
            metadata.api_root,
            BASE64_STANDARD.encode(metadata.raw),
        ))
    } else {
        None
    };

    let rpc_server = RpcServerBuilder::new().launch().await?;

    let launch_libraries_dir = runtime.libraries_dir(&state.directories);
    let launch_log_configs_dir = runtime.log_configs_dir(&state.directories);
    let class_paths = if let (Some(direct), Some(libraries)) =
        (&direct_launch, linked_libraries.as_deref())
    {
        args::get_linked_class_paths(
            direct,
            libraries,
            &[&main_class_path, &client_path],
            &java_version.architecture,
            minecraft_updated,
        )?
    } else {
        args::get_class_paths(
            &launch_libraries_dir,
            version_info.libraries.as_slice(),
            &[&main_class_path, &client_path],
            &java_version.architecture,
            minecraft_updated,
        )?
    };

    command.args(
        args::get_jvm_arguments(
            args.get(&d::minecraft::ArgumentType::Jvm)
                .map(|x| x.as_slice()),
            &natives_dir,
            &launch_libraries_dir,
            &launch_log_configs_dir,
            &class_paths,
            &main_class_path,
            &version_jar,
            effective_memory,
            resolved_java_args.clone(),
            &java_version.architecture,
            &quick_play_type,
            quick_play_version,
            version_info
                .logging
                .as_ref()
                .and_then(|x| x.get(&LoggingSide::Client)),
            rpc_server.address(),
        )?
        .into_iter(),
    );
    // Identify the launcher to Minecraft and mods consistently across every
    // loader. Append these after profile-provided JVM arguments so custom
    // arguments cannot accidentally replace the launcher's identity.
    command
        .arg("-Dminecraft.launcher.brand=Axolotl Launcher")
        .arg(format!(
            "-Dminecraft.launcher.version={}",
            env!("CARGO_PKG_VERSION")
        ));

    // PCL-style custom window title. Vanilla ignores this property, but
    // loader/mod bridges that read it get the same value as the Win32
    // rename pass applied after the window appears.
    if let Some(title) = window_title
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        command.arg(format!("-Dminecraft.window.title={title}"));
    }

    // The java launcher requires access to java.lang.reflect in order to force access in to
    // whatever module the main class is in
    if java_version.parsed_version >= 9 {
        command.arg("--add-opens=java.base/java.lang.reflect=ALL-UNNAMED");
    }

    // The java launcher code requires internal JDK code in Java 25+ in order to support JEP 512
    if java_version.parsed_version >= 25 {
        command.arg("--add-opens=java.base/jdk.internal.misc=ALL-UNNAMED");
    }

    if let Some((agent_path, api_root, prefetched_metadata)) = yggdrasil_agent {
        command
            .arg(format!(
                "-javaagent:{}={api_root}",
                agent_path.to_string_lossy()
            ))
            .arg("-Dauthlibinjector.side=client")
            .arg(format!(
                "-Dauthlibinjector.yggdrasil.prefetched={prefetched_metadata}"
            ));
    }

    let launch_assets_dir = runtime.assets_dir(&state.directories);
    let game_assets_dir = runtime
        .game_assets_dir(&state.directories, version_info.assets == "legacy");

    command
        .arg("com.modrinth.theseus.MinecraftLaunch")
        .arg(version_info.main_class.clone())
        .args(
            args::get_minecraft_arguments(
                args.get(&d::minecraft::ArgumentType::Game)
                    .map(|x| x.as_slice()),
                version_info.minecraft_arguments.as_deref(),
                credentials,
                &launch_version_id,
                &version_info.asset_index.id,
                &instance_path,
                &launch_assets_dir,
                &game_assets_dir,
                &launch_version_type,
                effective_resolution,
                &java_version.architecture,
                &quick_play_type,
                quick_play_version,
            )
            .await?
            .into_iter()
            .map(encode_game_argument),
        )
        // Linked-launcher game arguments (PCL VersionAdvanceGame, HMCL
        // auto-connect server) ride at the tail of the vanilla arguments.
        .args(extra_game_args)
        .current_dir(instance_path.clone());

    java_environment::configure_java_environment(command.as_std_mut());

    #[cfg(target_os = "linux")]
    command.envs(high_performance_gpu_environment);

    // Explicit instance choices override automatic GPU selection.
    command.envs(env_args);

    // Overwrites the minecraft options.txt file with the settings from the profile
    // Uses 'a:b' syntax which is not quite yaml
    // Directly associated instances never write into the linked folder.
    let offline_skin_pack = if direct_launch.is_some() {
        // Writing the offline skin pack would modify the linked folder.
        crate::minecraft_skins::OfflineSkinPackOptions {
            enabled_pack_id: None,
        }
    } else {
        crate::minecraft_skins::prepare_offline_skin_resource_pack(
            credentials,
            &instance_path,
            &content_set.game_version,
            &client_path,
        )
        .await?
    };

    let options_path = instance_path.join("options.txt");
    let options_existed = options_path.exists();

    if direct_launch.is_none()
        && (!mc_set_options.is_empty()
            || offline_skin_pack.enabled_pack_id.is_some()
            || options_existed
            || !settings.locale.is_empty()
            || !settings.default_game_language.is_empty())
    {
        let (mut options_string, input_encoding) = if options_existed {
            io::read_any_encoding_to_string(&options_path).await?
        } else {
            (String::new(), encoding_rs::UTF_8)
        };

        // UTF-16 encodings may be successfully detected and read, but we cannot encode
        // them back, and it's technically possible that the game client strongly expects
        // such encoding
        if input_encoding != input_encoding.output_encoding() {
            return Err(crate::ErrorKind::LauncherError(format!(
                "The instance options.txt file uses an unsupported encoding: {}. \
                Please either turn off instance options that need to modify this file, \
                or convert the file to an encoding that both the game and this app support, \
                such as UTF-8.",
                input_encoding.name()
            ))
            .into());
        }

        let language_options = language::game_language_options(
            &settings.default_game_language,
            &settings.locale,
            launch_release_time,
            &options_string,
            instance_path.join("saves").exists(),
        );

        if !mc_set_options.is_empty()
            || !language_options.is_empty()
            || offline_skin_pack.enabled_pack_id.is_some()
            || options_existed
        {
            for (key, value) in
                mc_set_options.iter().chain(language_options.iter())
            {
                let re =
                    Regex::new(&format!(r"(?m)^{}:.*$", regex::escape(key)))?;
                // check if the regex exists in the file
                if !re.is_match(&options_string) {
                    // The key was not found in the file, so append it
                    write!(&mut options_string, "\n{key}:{value}").unwrap();
                } else {
                    let replaced_string = re
                        .replace_all(&options_string, &format!("{key}:{value}"))
                        .to_string();
                    options_string = replaced_string;
                }
            }

            update_offline_skin_resource_pack_option(
                &mut options_string,
                offline_skin_pack,
            )?;

            io::write(&options_path, input_encoding.encode(&options_string).0)
                .await?;
        }
    }

    crate::state::instances::commands::set_instance_last_played(
        &instance.id,
        Utc::now(),
        &state.pool,
    )
    .await?;

    let _ = state
        .discord_rpc
        .set_activity(&format!("Playing {}", instance.name), true)
        .await;

    // The launcher log must land where the game and log browser read it. The
    // resolved directory is PCL's PathIndie equivalent for direct links.
    let logs_folder = state.directories.game_logs_dir(&instance_path);
    // Create Minecraft child by inserting it into the state
    // This also spawns the process and prepares the subsequent processes
    state
        .process_manager
        .insert_new_process(
            &instance.id,
            &instance.path,
            &instance.name,
            command,
            post_exit_hook,
            maximize_window,
            window_title,
            launch_preparation_timeout,
            instance_path.clone(),
            logs_folder,
            version_info.logging.is_some(),
            main_class_keep_alive,
            rpc_server,
            credentials.is_offline(),
            async |process: &ProcessMetadata, rpc_server| {
                let process_start_time = process.start_time.to_rfc3339();
                let instance_created_time = instance.created.to_rfc3339();
                let instance_modified_time = instance.modified.to_rfc3339();
                let (link_project_id, link_version_id) =
                    link_project_and_version(&context.link);
                let system_properties = [
                    ("modrinth.process.startTime", Some(&process_start_time)),
                    ("modrinth.profile.created", Some(&instance_created_time)),
                    ("modrinth.profile.icon", instance.icon_path.as_ref()),
                    ("modrinth.profile.link.project", link_project_id),
                    ("modrinth.profile.link.version", link_version_id),
                    (
                        "modrinth.profile.modified",
                        Some(&instance_modified_time),
                    ),
                    ("modrinth.profile.name", Some(&instance.name)),
                ];
                for (key, value) in system_properties {
                    let Some(value) = value else {
                        continue;
                    };
                    rpc_server
                        .call_method_2::<()>("set_system_property", key, value)
                        .await?;
                }
                rpc_server.call_method::<()>("launch").await?;
                Ok(())
            },
        )
        .await
}

#[cfg(test)]
mod game_argument_encoding_tests {
    use super::*;

    #[test]
    fn ascii_game_argument_stays_unchanged() {
        assert_eq!(
            encode_game_argument("--username".to_string()),
            "--username"
        );
    }

    #[test]
    fn non_ascii_game_argument_uses_ascii_transport() {
        let original = r"E:\Games\Minecraft\profiles\Prominence™ II";
        let encoded = encode_game_argument(original.to_string());

        assert!(encoded.is_ascii());
        let payload = encoded
            .strip_prefix(UTF8_GAME_ARGUMENT_PREFIX)
            .expect("non-ASCII argument should be encoded");
        assert_eq!(
            BASE64_STANDARD.decode(payload).unwrap(),
            original.as_bytes()
        );
    }

    #[test]
    fn reserved_prefix_is_escaped() {
        let original = format!("{UTF8_GAME_ARGUMENT_PREFIX}literal");
        let encoded = encode_game_argument(original.clone());
        let payload = encoded.strip_prefix(UTF8_GAME_ARGUMENT_PREFIX).unwrap();

        assert_eq!(
            BASE64_STANDARD.decode(payload).unwrap(),
            original.as_bytes()
        );
    }
}

fn update_offline_skin_resource_pack_option(
    options_string: &mut String,
    offline_skin_pack: crate::minecraft_skins::OfflineSkinPackOptions,
) -> crate::Result<()> {
    use crate::minecraft_skins::{
        OFFLINE_SKIN_PACK_LEGACY_ID, OFFLINE_SKIN_PACK_MODERN_ID,
    };

    let resource_packs = Regex::new(r"(?m)^resourcePacks:(.*)$")?;
    let existing_value = resource_packs
        .captures(options_string)
        .and_then(|captures| captures.get(1))
        .map_or("[]", |value| value.as_str().trim());
    if offline_skin_pack.enabled_pack_id.is_none()
        && !existing_value.contains(OFFLINE_SKIN_PACK_LEGACY_ID)
        && !existing_value.contains(OFFLINE_SKIN_PACK_MODERN_ID)
    {
        return Ok(());
    }
    let Ok(mut packs) = serde_json::from_str::<Vec<String>>(existing_value)
    else {
        tracing::warn!(
            "Skipping offline skin resource-pack option update because resourcePacks in options.txt is malformed"
        );
        return Ok(());
    };

    packs.retain(|pack| {
        pack != OFFLINE_SKIN_PACK_LEGACY_ID
            && pack != OFFLINE_SKIN_PACK_MODERN_ID
    });
    if let Some(pack_id) = offline_skin_pack.enabled_pack_id
        && !packs.iter().any(|pack| pack == pack_id)
    {
        if pack_id == OFFLINE_SKIN_PACK_MODERN_ID
            && !packs.iter().any(|pack| pack == "vanilla")
        {
            packs.insert(0, "vanilla".to_string());
        }
        packs.push(pack_id.to_string());
    }

    let value = serde_json::to_string(&packs)?;
    if resource_packs.is_match(options_string) {
        *options_string = resource_packs
            .replace_all(options_string, format!("resourcePacks:{value}"))
            .to_string();
    } else if options_string.is_empty() {
        write!(options_string, "resourcePacks:{value}").unwrap();
    } else {
        write!(options_string, "\nresourcePacks:{value}").unwrap();
    }

    Ok(())
}

#[cfg(test)]
mod loader_resolution_tests {
    use super::*;
    use daedalus::minecraft::RuleAction;

    #[tokio::test]
    async fn installed_launch_profile_never_calls_online_resolution() {
        let profile = prefer_local_launch_profile(Some("1.20.1-47.4.28"), || async {
            panic!("installed profile must launch even if the catalogs are unavailable")
        }).await.unwrap();
        assert_eq!(profile, "1.20.1-47.4.28");
        let mut calls = 0;
        let restored = prefer_local_launch_profile(None, || async {
            calls += 1;
            Ok("restored")
        })
        .await
        .unwrap();
        assert_eq!(restored, "restored");
        assert_eq!(calls, 1);
        let error = prefer_local_launch_profile::<(), _, _>(None, || async {
            Err(crate::ErrorKind::OtherError("recovery failed".into())
                .as_error())
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("recovery failed"));
    }

    #[tokio::test]
    async fn exact_loader_fallback_handles_successful_incomplete_catalogs_and_errors()
     {
        for primary in [
            Ok(None),
            Err(crate::ErrorKind::OtherError("official unavailable".into())
                .as_error()),
        ] {
            let result = resolve_exact_loader_fallback(primary, || async {
                Ok(Some(loader_version("47.4.28", false)))
            })
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.id, "47.4.28");
        }
        let result = resolve_exact_loader_fallback(
            Ok(Some(loader_version("47.4.28", false))),
            || async {
                panic!("found version must not consult another source")
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.id, "47.4.28");
        assert!(
            resolve_exact_loader_fallback(Ok(None), || async { Ok(None) })
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            resolve_exact_loader_fallback(Ok(None), || async {
                Err(crate::ErrorKind::OtherError("fallback unavailable".into())
                    .as_error())
            })
            .await
            .unwrap_err()
            .to_string()
            .contains("fallback unavailable")
        );
    }

    #[test]
    fn disallow_only_library_rules_are_not_downloaded() {
        let rules = [d::minecraft::Rule {
            action: RuleAction::Disallow,
            os: None,
            features: None,
        }];

        assert!(!parse_rules(&rules, "8", &QuickPlayType::None, false,));
        assert!(parse_rules(&[], "8", &QuickPlayType::None, false,));
    }

    fn loader_version(id: &str, stable: bool) -> LoaderVersion {
        LoaderVersion {
            id: id.to_string(),
            url: format!("https://example.invalid/{id}"),
            stable,
            profile_source: Default::default(),
            fallback_url: None,
        }
    }

    fn forge_manifest(ids: &[&str]) -> Manifest {
        Manifest {
            game_versions: vec![d::modded::Version {
                id: "1.20.1".into(),
                stable: true,
                version_group: None,
                loaders: ids
                    .iter()
                    .map(|id| loader_version(id, false))
                    .collect(),
            }],
            version_groups: vec![],
        }
    }

    #[tokio::test]
    async fn missing_exact_loader_refreshes_once_and_keeps_requested_version() {
        let mut calls = vec![];
        let selected = resolve_loader_version_with_refresh(
            "1.20.1",
            Some("47.4.28"),
            None,
            |behaviour| {
                calls.push(behaviour);
                let manifest = if behaviour == Some(CacheBehaviour::Bypass) {
                    forge_manifest(&["47.4.29", "47.4.28", "47.4.22"])
                } else {
                    forge_manifest(&["47.4.22"])
                };
                std::future::ready(Ok(manifest))
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(selected.id, "47.4.28");
        assert_eq!(calls, [None, Some(CacheBehaviour::Bypass)]);
    }

    #[tokio::test]
    async fn loader_refresh_respects_offline_mode_and_stops_on_missing_version()
    {
        for (behaviour, count) in [
            (None, 2),
            (Some(CacheBehaviour::CacheOnly), 1),
            (Some(CacheBehaviour::Bypass), 1),
        ] {
            let mut calls = 0;
            let result = resolve_loader_version_with_refresh(
                "1.20.1",
                Some("47.4.28"),
                behaviour,
                |_| {
                    calls += 1;
                    std::future::ready(Ok(forge_manifest(&["47.4.22"])))
                },
            )
            .await
            .unwrap();
            assert!(result.is_none());
            assert_eq!(calls, count);
        }
    }

    #[tokio::test]
    async fn matching_loader_does_not_refresh_and_refresh_errors_are_preserved()
    {
        let mut calls = 0;
        let result = resolve_loader_version_with_refresh(
            "1.20.1",
            Some("47.4.28"),
            None,
            |_| {
                calls += 1;
                std::future::ready(Ok(forge_manifest(&["47.4.28"])))
            },
        )
        .await
        .unwrap();
        assert!(result.is_some());
        assert_eq!(calls, 1);

        let result = resolve_loader_version_with_refresh(
            "1.20.1",
            Some("47.4.28"),
            None,
            |behaviour| {
                std::future::ready(
                    if behaviour == Some(CacheBehaviour::Bypass) {
                        Err(crate::ErrorKind::OtherError(
                            "metadata unavailable".into(),
                        )
                        .as_error())
                    } else {
                        Ok(forge_manifest(&[]))
                    },
                )
            },
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("metadata unavailable")
        );
    }

    #[test]
    fn exact_game_version_precedes_generic_manifest_entry() {
        let mut manifest = forge_manifest(&["47.4.28"]);
        let mut generic = manifest.game_versions[0].clone();
        generic.id = d::modded::DUMMY_REPLACE_STRING.into();
        generic.loaders = vec![loader_version("47.4.22", false)];
        manifest.game_versions.insert(0, generic);
        let loaders =
            loader_versions_for_game_version(&manifest, "1.20.1").unwrap();
        assert_eq!(
            select_loader_version(loaders, Some("47.4.28")).unwrap().id,
            "47.4.28"
        );
    }

    #[test]
    fn liteloader_runtime_detection_uses_the_maven_coordinate() {
        assert!(is_liteloader_library(
            "com.mumfrey:liteloader:1.12.2-SNAPSHOT"
        ));
        assert!(!is_liteloader_library("example:liteloader:1.12.2-SNAPSHOT"));
        assert!(!is_liteloader_library("com.mumfrey:liteloader"));
    }

    #[test]
    fn repair_exact_loader_selection_never_substitutes_or_persists_fallback() {
        let versions = vec![
            loader_version("47.4.22", true),
            loader_version("47.2.0", false),
        ];

        assert_eq!(
            select_loader_version(&versions, Some("47.2.0"))
                .map(|version| version.id.as_str()),
            Some("47.2.0")
        );
        assert!(select_loader_version(&versions, Some("47.1.0")).is_none());
        assert_eq!(
            select_loader_version(&versions, None)
                .map(|version| version.id.as_str()),
            Some("47.4.22")
        );
        assert_eq!(explicit_loader_version(Some("47.1.0")), Some("47.1.0"));
        assert_eq!(explicit_loader_version(None), None);
        assert_eq!(explicit_loader_version(Some("latest")), None);
        assert_eq!(explicit_loader_version(Some("stable")), None);
        assert_eq!(
            missing_loader_version_fallback(
                ModLoader::Forge,
                "1.20.1",
                Some("47.1.0")
            ),
            Err("Loader version 47.1.0 is not available for forge 1.20.1"
                .to_string())
        );
        assert_eq!(
            missing_loader_version_fallback(ModLoader::Forge, "1.20.1", None),
            Ok("stable")
        );
    }

    #[test]
    fn installed_instances_use_persisted_exact_loader_offline() {
        for loader in [
            ModLoader::Fabric,
            ModLoader::Forge,
            ModLoader::NeoForge,
            ModLoader::Quilt,
        ] {
            let version = installed_offline_loader_version(
                loader,
                Some("persisted-exact-version"),
            )
            .expect("installed exact loader should not require metadata");
            assert_eq!(version.id, "persisted-exact-version", "{loader:?}");
            assert!(version.url.is_empty(), "{loader:?}");
        }

        assert!(
            installed_offline_loader_version(ModLoader::Forge, Some("latest"))
                .is_none()
        );
        assert!(
            installed_offline_loader_version(ModLoader::Forge, None).is_none()
        );
    }
}

#[cfg(test)]
mod processor_output_tests {
    use super::*;
    use std::collections::HashMap;

    fn processor(
        path: impl Into<String>,
        hash: impl Into<String>,
    ) -> d::modded::Processor {
        d::modded::Processor {
            jar: "example:processor:1.0".to_string(),
            classpath: Vec::new(),
            args: Vec::new(),
            outputs: Some(HashMap::from([(path.into(), hash.into())])),
            sides: None,
        }
    }

    fn data(
        path: impl Into<String>,
        hash: impl Into<String>,
    ) -> HashMap<String, d::modded::SidedDataEntry> {
        HashMap::from([
            (
                "OUTPUT".to_string(),
                d::modded::SidedDataEntry {
                    client: path.into(),
                    server: String::new(),
                },
            ),
            (
                "HASH".to_string(),
                d::modded::SidedDataEntry {
                    client: hash.into(),
                    server: String::new(),
                },
            ),
        ])
    }

    #[test]
    fn client_processor_detection_respects_sides() {
        let mut processors = vec![d::modded::Processor {
            jar: "example:processor:1.0".to_string(),
            classpath: Vec::new(),
            args: Vec::new(),
            outputs: None,
            sides: Some(vec!["server".to_string()]),
        }];
        assert!(!has_client_processors(Some(&processors)));

        processors[0].sides = Some(vec!["client".to_string()]);
        assert!(has_client_processors(Some(&processors)));

        processors[0].sides = None;
        assert!(has_client_processors(Some(&processors)));
        assert!(!has_client_processors(None));
    }

    #[test]
    fn detects_download_mojmaps_processor_by_task_argument() {
        let mut processor = d::modded::Processor {
            jar: "net.minecraftforge:installertools:1.4.1".to_string(),
            classpath: Vec::new(),
            args: vec![
                "--task".to_string(),
                "DOWNLOAD_MOJMAPS".to_string(),
                "--output".to_string(),
                "{MOJMAPS}".to_string(),
            ],
            outputs: None,
            sides: None,
        };
        assert!(is_download_mojmaps_processor(&processor));

        processor.args[1] = "MCP_DATA".to_string();
        assert!(!is_download_mojmaps_processor(&processor));
    }

    #[tokio::test]
    async fn valid_processor_output_skips_execution() {
        let directory = tempfile::tempdir().unwrap();
        let libraries = directory.path().join("libraries");
        tokio::fs::create_dir_all(&libraries).await.unwrap();
        let output = libraries.join("output.jar");
        tokio::fs::write(&output, b"valid processor output")
            .await
            .unwrap();
        let hash = sha1_smol::Sha1::from(b"valid processor output").hexdigest();
        let data = data(output.to_string_lossy(), format!("'{hash}'"));

        assert!(
            processor_outputs_are_current(
                &processor("{OUTPUT}", "{HASH}"),
                &libraries,
                &data,
                &[&libraries],
            )
            .await
        );
    }

    #[tokio::test]
    async fn binary_patcher_uses_the_matching_output_sha() {
        let directory = tempfile::tempdir().unwrap();
        let libraries = directory.path().join("libraries");
        tokio::fs::create_dir_all(&libraries).await.unwrap();
        let output = libraries.join("patched.jar");
        tokio::fs::write(&output, b"patched minecraft")
            .await
            .unwrap();
        let hash = sha1_smol::Sha1::from(b"patched minecraft").hexdigest();
        let data = HashMap::from([
            (
                "PATCHED".to_string(),
                d::modded::SidedDataEntry {
                    client: output.to_string_lossy().into_owned(),
                    server: String::new(),
                },
            ),
            (
                "PATCHED_SHA".to_string(),
                d::modded::SidedDataEntry {
                    client: format!("'{hash}'"),
                    server: String::new(),
                },
            ),
        ]);
        let processor = d::modded::Processor {
            jar: "net.minecraftforge:binarypatcher:1.1.1".to_string(),
            classpath: Vec::new(),
            args: vec!["--output".to_string(), "{PATCHED}".to_string()],
            outputs: None,
            sides: None,
        };

        assert!(
            processor_outputs_are_current(
                &processor,
                &libraries,
                &data,
                &[&libraries],
            )
            .await
        );
    }

    #[tokio::test]
    async fn missing_processor_output_runs_execution() {
        let directory = tempfile::tempdir().unwrap();
        let libraries = directory.path().join("libraries");
        tokio::fs::create_dir_all(&libraries).await.unwrap();
        let output = libraries.join("missing.jar");
        let data = data(
            output.to_string_lossy(),
            "0000000000000000000000000000000000000000",
        );

        assert!(
            !processor_outputs_are_current(
                &processor("{OUTPUT}", "{HASH}"),
                &libraries,
                &data,
                &[&libraries],
            )
            .await
        );
    }

    #[tokio::test]
    async fn wrong_processor_output_hash_runs_execution() {
        let directory = tempfile::tempdir().unwrap();
        let libraries = directory.path().join("libraries");
        tokio::fs::create_dir_all(&libraries).await.unwrap();
        let output = libraries.join("output.jar");
        tokio::fs::write(&output, b"wrong hash").await.unwrap();
        let data = data(
            output.to_string_lossy(),
            "0000000000000000000000000000000000000000",
        );

        assert!(
            !processor_outputs_are_current(
                &processor("{OUTPUT}", "{HASH}"),
                &libraries,
                &data,
                &[&libraries],
            )
            .await
        );
    }

    #[tokio::test]
    async fn malformed_or_escaped_processor_outputs_run_execution() {
        let directory = tempfile::tempdir().unwrap();
        let libraries = directory.path().join("libraries");
        tokio::fs::create_dir_all(&libraries).await.unwrap();
        let outside = directory.path().join("outside.jar");
        tokio::fs::write(&outside, b"outside").await.unwrap();
        let hash = sha1_smol::Sha1::from(b"outside").hexdigest();
        let escaped_data = data(outside.to_string_lossy(), &hash);

        assert!(
            !processor_outputs_are_current(
                &processor("{OUTPUT}", "{HASH}"),
                &libraries,
                &escaped_data,
                &[&libraries],
            )
            .await
        );
        let malformed_data = data(outside.to_string_lossy(), "not-a-sha1");
        assert!(
            !processor_outputs_are_current(
                &processor("{OUTPUT}", "{HASH}"),
                &libraries,
                &malformed_data,
                &[directory.path()],
            )
            .await
        );
    }
}

#[cfg(test)]
mod offline_skin_resource_pack_tests {
    use super::*;

    #[test]
    fn enables_offline_skin_pack_when_options_file_is_new() {
        let mut options = String::new();

        update_offline_skin_resource_pack_option(
            &mut options,
            crate::minecraft_skins::OfflineSkinPackOptions {
                enabled_pack_id: Some(
                    crate::minecraft_skins::OFFLINE_SKIN_PACK_MODERN_ID,
                ),
            },
        )
        .unwrap();

        assert_eq!(
            options,
            "resourcePacks:[\"vanilla\",\"file/Axolotl Offline Skin.zip\"]"
        );
    }

    #[test]
    fn adds_modern_offline_skin_pack_without_removing_existing_packs() {
        let mut options =
            "resourcePacks:[\"vanilla\",\"file/user-pack.zip\"]".to_string();

        update_offline_skin_resource_pack_option(
            &mut options,
            crate::minecraft_skins::OfflineSkinPackOptions {
                enabled_pack_id: Some(
                    crate::minecraft_skins::OFFLINE_SKIN_PACK_MODERN_ID,
                ),
            },
        )
        .unwrap();

        assert_eq!(
            options,
            "resourcePacks:[\"vanilla\",\"file/user-pack.zip\",\"file/Axolotl Offline Skin.zip\"]"
        );
    }

    #[test]
    fn removes_both_offline_skin_pack_id_variants() {
        let mut options = "resourcePacks:[\"vanilla\",\"Axolotl Offline Skin.zip\",\"file/Axolotl Offline Skin.zip\"]".to_string();

        update_offline_skin_resource_pack_option(
            &mut options,
            crate::minecraft_skins::OfflineSkinPackOptions {
                enabled_pack_id: None,
            },
        )
        .unwrap();

        assert_eq!(options, "resourcePacks:[\"vanilla\"]");
    }
}

#[cfg(test)]
mod linked_rule_tests {
    use super::*;
    use d::minecraft::{FeatureRule, OsRule, Rule};

    #[test]
    fn freebsd_rules_do_not_enable_foreign_libraries_or_disable_host_libraries()
    {
        for arch in ["x86", "x86_64", "aarch64", "arm"] {
            let allowed: Vec<Rule> =
                serde_json::from_value(serde_json::json!([
                    {"action": "allow", "os": {"name": "freebsd"}}
                ]))
                .unwrap();
            assert_eq!(
                parse_rules(&allowed, arch, &QuickPlayType::None, true),
                cfg!(target_os = "freebsd")
            );
            let excluded: Vec<Rule> =
                serde_json::from_value(serde_json::json!([
                    {"action": "allow"},
                    {"action": "disallow", "os": {"name": "freebsd"}}
                ]))
                .unwrap();
            assert_eq!(
                parse_rules(&excluded, arch, &QuickPlayType::None, true),
                !cfg!(target_os = "freebsd")
            );
        }
    }

    #[test]
    fn linux_ppc64le_rules_only_apply_to_the_matching_java_architecture() {
        for arch in
            ["x86", "x86_64", "aarch64", "arm", "ppc64le", "powerpc64le"]
        {
            let allowed: Vec<Rule> =
                serde_json::from_value(serde_json::json!([
                    {"action": "allow", "os": {"name": "linux-ppc64le"}}
                ]))
                .unwrap();
            let matches = cfg!(target_os = "linux")
                && matches!(arch, "ppc64le" | "powerpc64le");
            assert_eq!(
                parse_rules(&allowed, arch, &QuickPlayType::None, true),
                matches
            );
        }
    }

    fn rule(
        action: RuleAction,
        os: Option<OsRule>,
        features: Option<FeatureRule>,
    ) -> Rule {
        Rule {
            action,
            os,
            features,
        }
    }

    #[test]
    fn ordered_rules_use_the_last_matching_action() {
        let rules = [
            rule(RuleAction::Allow, None, None),
            rule(RuleAction::Disallow, None, None),
            rule(RuleAction::Allow, None, None),
        ];
        assert!(parse_rules(
            &rules,
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true
        ));
        assert!(!parse_rules(
            &rules[..2],
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true
        ));
        assert!(parse_rules(
            &[],
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true
        ));
    }

    #[test]
    fn os_and_features_on_one_rule_are_conjunctive() {
        let matching_os = OsRule {
            name: Some(d::minecraft::Os::native_arch(std::env::consts::ARCH)),
            version: None,
            arch: Some(std::env::consts::ARCH.to_string()),
        };
        let custom_resolution = FeatureRule {
            is_demo_user: None,
            has_custom_resolution: Some(true),
            has_quick_plays_support: None,
            is_quick_play_singleplayer: None,
            is_quick_play_multiplayer: None,
            is_quick_play_realms: None,
        };
        let matching_rule = rule(
            RuleAction::Allow,
            Some(matching_os),
            Some(custom_resolution),
        );
        assert!(parse_rules(
            &[matching_rule],
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true
        ));

        let mismatched = rule(
            RuleAction::Allow,
            Some(OsRule {
                name: Some(d::minecraft::Os::native_arch(
                    std::env::consts::ARCH,
                )),
                version: None,
                arch: Some("definitely-not-this-architecture".to_string()),
            }),
            Some(FeatureRule {
                is_demo_user: None,
                has_custom_resolution: Some(true),
                has_quick_plays_support: None,
                is_quick_play_singleplayer: None,
                is_quick_play_multiplayer: None,
                is_quick_play_realms: None,
            }),
        );
        assert!(!parse_rules(
            &[mismatched],
            std::env::consts::ARCH,
            &QuickPlayType::None,
            true
        ));
    }

    #[test]
    fn symlink_profile_is_not_eligible_for_direct_link_promotion() {
        assert!(!external_link_promotion_allowed(
            false,
            Some(r"D:\Minecraft\.minecraft"),
        ));
    }

    #[test]
    fn ordinary_external_override_remains_eligible_for_promotion() {
        assert!(external_link_promotion_allowed(false, None));
        assert!(!external_link_promotion_allowed(true, None));
    }
}
