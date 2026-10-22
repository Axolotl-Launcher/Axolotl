//! OptiFine loader support.
//!
//! OptiFine has no official metadata API, so available versions and installer
//! downloads are resolved through BMCLAPI. Standalone OptiFine instances are
//! launched through LaunchWrapper with `optifine.OptiFineTweaker`, mirroring
//! the approach used by HMCL and PCL.

use std::collections::HashMap;
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};

use daedalus::minecraft::{
    Argument, ArgumentType, Library, VersionInfo as GameVersionInfo,
};
use daedalus::modded::{LoaderVersion, PartialVersionInfo};
use reqwest::Method;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::State;
use crate::util::fetch::{
    self, ContentValidation, DownloadRequest, Integrity, ResourceClass,
    download_to_path, fetch_json,
};
use crate::util::{io, java_environment};

const BMCLAPI_OPTIFINE_BASE: &str = "https://bmclapi2.bangbang93.com/optifine";
pub const OPTIFINE_LOADER_PREFIX: &str = "OptiFine_";
const OPTIFINE_TWEAK_CLASS: &str = "optifine.OptiFineTweaker";
const LAUNCH_WRAPPER_MAIN_CLASS: &str = "net.minecraft.launchwrapper.Launch";
const FALLBACK_LAUNCH_WRAPPER: &str = "net.minecraft:launchwrapper:1.12";
const OPTIFINE_TWEAK_ENTRY: &str = "optifine/OptiFineTweaker.class";
const LAUNCH_WRAPPER_ENTRY: &str = "net/minecraft/launchwrapper/Launch.class";

#[derive(Deserialize, Debug, Clone)]
struct BmclapiOptifineEntry {
    mcversion: String,
    #[serde(rename = "type")]
    type_: String,
    patch: String,
}

impl BmclapiOptifineEntry {
    fn version_id(&self) -> String {
        format!("{}_{}", self.type_, self.patch)
    }

    fn download_url(&self) -> String {
        format!(
            "{BMCLAPI_OPTIFINE_BASE}/{}/{}/{}",
            self.mcversion, self.type_, self.patch
        )
    }

    fn is_stable(&self) -> bool {
        !self.patch.to_ascii_lowercase().contains("pre")
    }
}

async fn list_entries(
    game_version: &str,
) -> crate::Result<Vec<BmclapiOptifineEntry>> {
    let state = State::get().await?;
    let entries: Vec<BmclapiOptifineEntry> = fetch_json(
        Method::GET,
        &format!("{BMCLAPI_OPTIFINE_BASE}/{game_version}"),
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry.mcversion == game_version)
        .collect())
}

/// Lists installable OptiFine versions for a game version as loader versions,
/// ordered oldest to newest as reported by BMCLAPI.
pub async fn list_loader_versions(
    game_version: &str,
) -> crate::Result<Vec<LoaderVersion>> {
    Ok(list_entries(game_version)
        .await?
        .iter()
        .map(|entry| LoaderVersion {
            id: format!("{OPTIFINE_LOADER_PREFIX}{}", entry.version_id()),
            url: entry.download_url(),
            stable: entry.is_stable(),
            profile_source: Default::default(),
            fallback_url: None,
        })
        .collect())
}

/// Strips launcher-specific prefixes so pack-provided OptiFine version strings
/// like `OptiFine_1.12.2_HD_U_G5`, `1.12.2_HD_U_G5`, or `HD_U_G5` all resolve
/// to the same BMCLAPI entry.
fn normalize_version_id(game_version: &str, requested: &str) -> String {
    let mut value = requested.trim();
    if let Some(stripped) = value.strip_prefix(OPTIFINE_LOADER_PREFIX) {
        value = stripped;
    }
    if let Some(stripped) = value.strip_prefix(game_version)
        && let Some(stripped) = stripped.strip_prefix('_')
    {
        value = stripped;
    }
    value.to_string()
}

pub async fn resolve_loader_version(
    game_version: &str,
    requested: Option<&str>,
) -> crate::Result<Option<LoaderVersion>> {
    let resolved = match requested.unwrap_or("latest") {
        "latest" => list_loader_versions(game_version).await?.pop(),
        "stable" => {
            let versions = list_loader_versions(game_version).await?;
            versions
                .iter()
                .rev()
                .find(|version| version.stable)
                .or(versions.last())
                .cloned()
        }
        // A pinned version resolves without the network so offline launches of
        // installed instances keep working; the installer download resolves
        // the actual BMCLAPI URL itself when needed.
        id => {
            let of_id = normalize_version_id(game_version, id);
            Some(LoaderVersion {
                id: format!("{OPTIFINE_LOADER_PREFIX}{of_id}"),
                url: String::new(),
                stable: !of_id.to_ascii_lowercase().contains("pre"),
                profile_source: Default::default(),
                fallback_url: None,
            })
        }
    };
    Ok(resolved)
}

fn optifine_library_name(game_version: &str, of_id: &str) -> String {
    format!("optifine:OptiFine:{game_version}_{of_id}")
}

fn library(name: String, downloadable: bool) -> Library {
    Library {
        downloads: None,
        extract: None,
        name,
        url: None,
        natives: None,
        rules: None,
        checksums: None,
        include_in_classpath: true,
        downloadable,
    }
}

struct InstallerInfo {
    launchwrapper_library: Library,
    launchwrapper_entry: Option<String>,
    needs_patching: bool,
}

fn inspect_installer(path: &Path) -> crate::Result<InstallerInfo> {
    let file = std::fs::File::open(path)
        .map_err(|error| io::IOError::with_path(error, path))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "OptiFine installer archive is invalid: {error}"
        ))
    })?;

    let mut launchwrapper_version = None;
    if let Ok(mut entry) = archive.by_name("launchwrapper-of.txt") {
        let mut version = String::new();
        entry.read_to_string(&mut version)?;
        let version = version.trim().to_string();
        if !version.is_empty() {
            launchwrapper_version = Some(version);
        }
    }
    if launchwrapper_version.is_none() {
        launchwrapper_version = archive.file_names().find_map(|name| {
            name.strip_prefix("launchwrapper-of-")
                .and_then(|rest| rest.strip_suffix(".jar"))
                .map(str::to_string)
        });
    }

    let needs_patching = archive.by_name("optifine/Patcher.class").is_ok();

    let (launchwrapper_library, launchwrapper_entry) =
        match launchwrapper_version {
            Some(version) => (
                library(format!("optifine:launchwrapper-of:{version}"), false),
                Some(format!("launchwrapper-of-{version}.jar")),
            ),
            // Very old OptiFine builds ship without their own LaunchWrapper;
            // Mojang's launchwrapper 1.12 from libraries.minecraft.net works.
            None => (library(FALLBACK_LAUNCH_WRAPPER.to_string(), true), None),
        };

    Ok(InstallerInfo {
        launchwrapper_library,
        launchwrapper_entry,
        needs_patching,
    })
}

async fn ensure_installer(
    state: &State,
    game_version: &str,
    of_id: &str,
) -> crate::Result<PathBuf> {
    let path = state
        .directories
        .caches_dir()
        .join("optifine")
        .join(game_version)
        .join(format!("{OPTIFINE_LOADER_PREFIX}{of_id}.jar"));
    if path.is_file() {
        return Ok(path);
    }

    let entries = list_entries(game_version).await?;
    let entry = entries
        .iter()
        .find(|entry| entry.version_id() == of_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "OptiFine {of_id} is not available for Minecraft {game_version}"
            ))
        })?;

    download_to_path(
        DownloadRequest::new(entry.download_url(), ResourceClass::Loader)
            .with_integrity(Integrity {
                content: ContentValidation::Jar,
                ..Integrity::default()
            }),
        &path,
        &state.download_semaphore,
        &state.pool,
        None,
    )
    .await?;

    Ok(path)
}

fn strip_loader_prefix(loader_version_id: &str) -> &str {
    loader_version_id
        .strip_prefix(OPTIFINE_LOADER_PREFIX)
        .unwrap_or(loader_version_id)
}

/// Builds the loader profile for a standalone OptiFine version locally, taking
/// the place of the Daedalus partial version metadata used by other loaders.
pub(crate) async fn build_partial_version_info(
    state: &State,
    vanilla: &GameVersionInfo,
    game_version: &str,
    loader_version_id: &str,
) -> crate::Result<PartialVersionInfo> {
    let of_id = strip_loader_prefix(loader_version_id).to_string();
    let installer = ensure_installer(state, game_version, &of_id).await?;
    let installer_for_inspect = installer.clone();
    let info = tokio::task::spawn_blocking(move || {
        inspect_installer(&installer_for_inspect)
    })
    .await??;

    let libraries = vec![
        library(optifine_library_name(game_version, &of_id), false),
        info.launchwrapper_library,
    ];

    let mut minecraft_arguments = None;
    let mut arguments = None;
    if let Some(vanilla_arguments) = &vanilla.minecraft_arguments {
        minecraft_arguments = Some(format!(
            "{vanilla_arguments} --tweakClass {OPTIFINE_TWEAK_CLASS}"
        ));
    } else {
        arguments = Some(HashMap::from([(
            ArgumentType::Game,
            vec![
                Argument::Normal("--tweakClass".to_string()),
                Argument::Normal(OPTIFINE_TWEAK_CLASS.to_string()),
            ],
        )]));
    }

    Ok(PartialVersionInfo {
        id: format!("{game_version}-{loader_version_id}"),
        inherits_from: game_version.to_string(),
        release_time: vanilla.release_time,
        time: vanilla.time,
        main_class: Some(LAUNCH_WRAPPER_MAIN_CLASS.to_string()),
        minecraft_arguments,
        arguments,
        libraries,
        java_version: None,
        type_: vanilla.type_.clone(),
        data: None,
        processors: None,
    })
}

fn extract_installer_entry(
    installer: &Path,
    entry_name: &str,
    target: &Path,
) -> crate::Result<()> {
    let file = std::fs::File::open(installer)
        .map_err(|error| io::IOError::with_path(error, installer))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        crate::ErrorKind::InputError(format!(
            "OptiFine installer archive is invalid: {error}"
        ))
    })?;
    let mut entry = archive.by_name(entry_name).map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "OptiFine installer is missing {entry_name}"
        ))
    })?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| io::IOError::with_path(error, parent))?;
    }
    let mut output = std::fs::File::create(target)
        .map_err(|error| io::IOError::with_path(error, target))?;
    std::io::copy(&mut entry, &mut output)
        .map_err(|error| io::IOError::with_path(error, target))?;
    Ok(())
}

async fn validate_runtime_jar(
    path: &Path,
    required_entry: &str,
) -> crate::Result<()> {
    fetch::validate_file_content(path, ContentValidation::Jar).await?;
    let path = path.to_path_buf();
    let required_entry = required_entry.to_string();
    tokio::task::spawn_blocking(move || -> crate::Result<()> {
        let file = std::fs::File::open(&path)
            .map_err(|error| io::IOError::with_path(error, &path))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Invalid runtime JAR {}: {error}",
                path.display()
            ))
        })?;
        let mut entry = archive.by_name(&required_entry).map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Runtime JAR {} is missing {required_entry}: {error}",
                path.display()
            ))
        })?;
        if entry.size() == 0 {
            return Err(crate::ErrorKind::LauncherError(format!(
                "Runtime JAR {} contains an empty {required_entry}",
                path.display()
            ))
            .into());
        }
        std::io::copy(&mut entry, &mut std::io::sink())?;
        Ok(())
    })
    .await?
}

fn check_cancellation(cancellation: &CancellationToken) -> crate::Result<()> {
    if cancellation.is_cancelled() {
        return Err(crate::ErrorKind::LauncherError(
            "OptiFine installation canceled".to_string(),
        )
        .into());
    }
    Ok(())
}

/// Publishes a completed runtime JAR while keeping failed builds outside the cache.
async fn publish_runtime_jar<F, Fut>(
    target: &Path,
    repairing: bool,
    required_entry: &str,
    cancellation: &CancellationToken,
    build: F,
) -> crate::Result<()>
where
    F: FnOnce(PathBuf) -> Fut + Send,
    Fut: Future<Output = crate::Result<()>> + Send,
{
    let lock = fetch::destination_download_lock(target);
    let _guard = lock.lock().await;
    check_cancellation(cancellation)?;
    if !repairing && validate_runtime_jar(target, required_entry).await.is_ok()
    {
        check_cancellation(cancellation)?;
        return Ok(());
    }
    let parent = target.parent().ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "Runtime JAR has no parent directory: {}",
            target.display()
        ))
    })?;
    io::create_dir_all(parent).await?;
    let staging = tempfile::tempdir_in(parent)
        .map_err(|error| io::IOError::with_path(error, parent))?;
    let output = staging.path().join("output.jar");
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => { check_cancellation(cancellation)?; }
        result = build(output.clone()) => { result?; }
    }
    check_cancellation(cancellation)?;
    validate_runtime_jar(&output, required_entry).await?;
    let bytes = io::read(&output).await?;
    check_cancellation(cancellation)?;
    io::write(target, bytes).await?;
    check_cancellation(cancellation)?;
    Ok(())
}

/// Materializes the OptiFine libraries after the vanilla client jar has been
/// downloaded: extracts the bundled LaunchWrapper and produces the OptiFine
/// library jar, running the installer's patcher against the client jar when
/// the installer only ships patch data.
pub(crate) async fn install_optifine_libraries(
    state: &State,
    java_path: &Path,
    game_version: &str,
    loader_version_id: &str,
    client_jar_path: &Path,
    repairing: bool,
    cancellation: &CancellationToken,
) -> crate::Result<()> {
    let of_id = strip_loader_prefix(loader_version_id).to_string();
    let installer = ensure_installer(state, game_version, &of_id).await?;
    let installer_for_inspect = installer.clone();
    let info = tokio::task::spawn_blocking(move || {
        inspect_installer(&installer_for_inspect)
    })
    .await??;
    let libraries_dir = state.directories.libraries_dir();

    if let Some(entry_name) = info.launchwrapper_entry {
        let target = libraries_dir.join(daedalus::get_path_from_artifact(
            &info.launchwrapper_library.name,
        )?);
        let embedded_installer = installer.clone();
        publish_runtime_jar(
            &target,
            repairing,
            LAUNCH_WRAPPER_ENTRY,
            cancellation,
            |output| async move {
                tokio::task::spawn_blocking(move || {
                    extract_installer_entry(
                        &embedded_installer,
                        &entry_name,
                        &output,
                    )
                })
                .await??;
                Ok(())
            },
        )
        .await?;
    }

    let optifine_target = libraries_dir.join(daedalus::get_path_from_artifact(
        &optifine_library_name(game_version, &of_id),
    )?);
    publish_runtime_jar(
        &optifine_target,
        repairing,
        OPTIFINE_TWEAK_ENTRY,
        cancellation,
        |output_path| async move {
            if info.needs_patching {
                let mut command = java_environment::tokio_command(java_path);
                command
                    .kill_on_drop(true)
                    .arg("-cp")
                    .arg(&installer)
                    .arg("optifine.Patcher")
                    .arg(client_jar_path)
                    .arg(&installer)
                    .arg(&output_path);
                let output = command.output().await.map_err(|error| {
                    crate::ErrorKind::LauncherError(format!(
                        "Error running OptiFine patcher: {error}"
                    ))
                })?;
                if !output.status.success() {
                    return Err(crate::ErrorKind::LauncherError(format!(
                        "OptiFine patcher error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                    .as_error());
                }
            } else {
                io::copy(&installer, &output_path).await?;
            }
            Ok(())
        },
    )
    .await
}

/// Downloads the OptiFine jar usable as a Forge/NeoForge mod into the given
/// directory, patching it against the client jar when required. Returns the
/// target file path.
pub async fn install_optifine_as_mod(
    state: &State,
    instance_id: &str,
    cancellation: tokio_util::sync::CancellationToken,
    java_path: &Path,
    game_version: &str,
    requested_version: &str,
    client_jar_path: &Path,
    mods_dir: &Path,
) -> crate::Result<PathBuf> {
    let of_id = normalize_version_id(game_version, requested_version);
    let installer = ensure_installer(state, game_version, &of_id).await?;
    let installer_for_inspect = installer.clone();
    let info = tokio::task::spawn_blocking(move || {
        inspect_installer(&installer_for_inspect)
    })
    .await??;

    let target = mods_dir.join(format!(
        "{OPTIFINE_LOADER_PREFIX}{game_version}_{of_id}.jar"
    ));
    io::create_dir_all(mods_dir).await?;
    let process_cancellation = cancellation.clone();

    publish_runtime_jar(
        &target,
        true,
        OPTIFINE_TWEAK_ENTRY,
        &cancellation,
        |output_path| async move {
            if info.needs_patching {
                let mut command = java_environment::tokio_command(java_path);
                command
                    .arg("-cp")
                    .arg(&installer)
                    .arg("optifine.Patcher")
                    .arg(client_jar_path)
                    .arg(&installer)
                    .arg(&output_path);
                let output = super::run_instance_install_command(
                    instance_id.to_string(),
                    process_cancellation,
                    command,
                )
                .await
                .map_err(|error| {
                    crate::ErrorKind::LauncherError(format!(
                        "Error running OptiFine patcher: {error}"
                    ))
                })?;
                if !output.status.success() {
                    return Err(crate::ErrorKind::LauncherError(format!(
                        "OptiFine patcher error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                    .as_error());
                }
            } else {
                io::copy(&installer, &output_path).await?;
            }
            Ok(())
        },
    )
    .await?;

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn runtime_jar(entry: &str, payload: &[u8]) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(payload).unwrap();
        archive.finish().unwrap().into_inner()
    }

    async fn publish(
        target: &Path,
        repairing: bool,
        bytes: Vec<u8>,
    ) -> crate::Result<()> {
        publish_runtime_jar(
            target,
            repairing,
            OPTIFINE_TWEAK_ENTRY,
            &CancellationToken::new(),
            |output| async move {
                io::write(output, bytes).await?;
                Ok(())
            },
        )
        .await
    }

    #[tokio::test]
    async fn healthy_cache_is_reused_and_explicit_repair_rebuilds_it() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        let old = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"old class");
        io::write(&target, &old).await.unwrap();
        publish_runtime_jar(
            &target,
            false,
            OPTIFINE_TWEAK_ENTRY,
            &CancellationToken::new(),
            |_| async {
                Err(crate::ErrorKind::LauncherError(
                    "healthy cache must not rebuild".to_string(),
                )
                .into())
            },
        )
        .await
        .unwrap();
        assert_eq!(io::read(&target).await.unwrap(), old);
        let repaired = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"repaired class");
        publish(&target, true, repaired.clone()).await.unwrap();
        assert_eq!(io::read(&target).await.unwrap(), repaired);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn incomplete_cache_is_rebuilt_without_an_explicit_repair() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        for invalid in [
            b"interrupted patch".to_vec(),
            runtime_jar("unrelated.class", b"unrelated"),
            runtime_jar(OPTIFINE_TWEAK_ENTRY, b""),
        ] {
            io::write(&target, invalid).await.unwrap();
            let rebuilt = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"complete class");
            publish(&target, false, rebuilt.clone()).await.unwrap();
            assert_eq!(io::read(&target).await.unwrap(), rebuilt);
        }
    }

    #[tokio::test]
    async fn failed_patch_never_replaces_the_cache_and_retry_rebuilds() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        for existing in [
            None,
            Some(runtime_jar(OPTIFINE_TWEAK_ENTRY, b"previous class")),
        ] {
            if let Some(bytes) = &existing {
                io::write(&target, bytes).await.unwrap();
            }
            let error = publish_runtime_jar(
                &target,
                true,
                OPTIFINE_TWEAK_ENTRY,
                &CancellationToken::new(),
                |output| async move {
                    io::write(output, b"partial output").await?;
                    Err(crate::ErrorKind::LauncherError(
                        "patcher failed".to_string(),
                    )
                    .into())
                },
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("patcher failed"));
            assert_eq!(tokio::fs::read(&target).await.ok(), existing);
            assert_eq!(
                std::fs::read_dir(directory.path()).unwrap().count(),
                usize::from(existing.is_some())
            );
            let rebuilt = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"complete class");
            publish(&target, existing.is_some(), rebuilt.clone())
                .await
                .unwrap();
            assert_eq!(io::read(&target).await.unwrap(), rebuilt);
        }
    }

    #[tokio::test]
    async fn invalid_successful_patch_output_is_not_published() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        let old = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"previous class");
        io::write(&target, &old).await.unwrap();
        for invalid in
            [b"truncated".to_vec(), runtime_jar("other.class", b"other")]
        {
            assert!(publish(&target, true, invalid).await.is_err());
            assert_eq!(io::read(&target).await.unwrap(), old);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }

    #[tokio::test]
    async fn cancellation_after_writing_partial_output_preserves_the_cache() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        let old = runtime_jar(OPTIFINE_TWEAK_ENTRY, b"previous class");
        io::write(&target, &old).await.unwrap();
        let cancellation = CancellationToken::new();
        let trigger = cancellation.clone();
        let error = publish_runtime_jar(
            &target,
            true,
            OPTIFINE_TWEAK_ENTRY,
            &cancellation,
            |output| async move {
                io::write(output, b"partial output").await?;
                trigger.cancel();
                std::future::pending::<crate::Result<()>>().await
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("canceled"));
        assert_eq!(io::read(&target).await.unwrap(), old);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn concurrent_installs_share_one_validated_cache_build() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("OptiFine.jar");
        let builds = AtomicUsize::new(0);
        let cancellation = CancellationToken::new();
        let install = || {
            publish_runtime_jar(
                &target,
                false,
                OPTIFINE_TWEAK_ENTRY,
                &cancellation,
                |output| async {
                    builds.fetch_add(1, Ordering::SeqCst);
                    io::write(
                        output,
                        runtime_jar(OPTIFINE_TWEAK_ENTRY, b"complete class"),
                    )
                    .await?;
                    Ok(())
                },
            )
        };
        let (first, second) = tokio::join!(install(), install());
        first.unwrap();
        second.unwrap();
        assert_eq!(builds.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn embedded_launchwrapper_is_validated_before_cache_reuse() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("launchwrapper.jar");
        io::write(&target, b"partial extraction").await.unwrap();
        let jar = runtime_jar(LAUNCH_WRAPPER_ENTRY, b"launch class");
        publish_runtime_jar(
            &target,
            false,
            LAUNCH_WRAPPER_ENTRY,
            &CancellationToken::new(),
            |output| async {
                io::write(output, &jar).await?;
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(io::read(&target).await.unwrap(), jar);
    }
}
