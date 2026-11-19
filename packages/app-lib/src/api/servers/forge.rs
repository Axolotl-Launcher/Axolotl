//! Forge and NeoForge server installation: downloading the official installer and running it
//! headlessly to materialize the server launcher (run script, `@args` files, and
//! the mod loader) into a managed server directory.
//!
//! Forge ships a bootstrapper rather than a ready-to-run jar. The launcher jar is
//! produced by `java -jar forge-installer.jar --installServer <dir>`, which lays
//! down the loader, its libraries, and the `@args` launch files. The regular
//! `servers.start` flow then boots that output (see `lifecycle::forge_launch_args`).

use std::collections::BTreeMap;
use std::path::Path;

use async_zip::tokio::read::fs::ZipFileReader;
use daedalus::minecraft::{LibraryDownload, LibraryDownloads};
use futures::io::AsyncReadExt;
use serde::Deserialize;

use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::util::download::constants::NEOFORGE_MAVEN;
use crate::util::fetch::{
    ContentValidation, DownloadRequest, Integrity, ResourceClass,
};
use crate::util::io::IOError;
use crate::util::java_environment;
use crate::{ErrorKind, Result, State};

use super::files::{download_server_file, download_to_dir, safe_relative_join};
use super::manifest::{
    InstallState, read_manifest, server_path, write_manifest,
};

const FORGE_MAVEN: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge";
const MAX_INSTALLER_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct InstallerManifest {
    json: Option<String>,
    #[serde(default)]
    libraries: Vec<InstallerLibrary>,
}

#[derive(Deserialize)]
struct InstallerLibrary {
    name: String,
    downloads: Option<LibraryDownloads>,
}

/// Downloads the Forge installer for `mc_version`/`build` into the server dir
/// and runs it headlessly (`--installServer`) to lay down the launcher. The
/// server is left unstarted; the caller (frontend wizard) writes `eula.txt` and
/// the regular `servers.start` flow boots it.
pub async fn install_forge(
    server_id: &str,
    mc_version: &str,
    build: &str,
    java_path: Option<String>,
) -> Result<()> {
    install_loader(server_id, "forge", mc_version, build, java_path).await
}

pub async fn install_neoforge(
    server_id: &str,
    mc_version: &str,
    build: &str,
    java_path: Option<String>,
) -> Result<()> {
    install_loader(server_id, "neoforge", mc_version, build, java_path).await
}

pub(super) fn installer_download(
    loader: &str,
    mc_version: &str,
    build: &str,
) -> Result<(String, String)> {
    for value in [mc_version, build] {
        if value.is_empty()
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
        {
            return Err(ErrorKind::InputError(
                "Invalid loader or game version".to_string(),
            )
            .as_error());
        }
    }
    let (base, artifact, version) = match loader {
        "forge" => (FORGE_MAVEN, "forge", format!("{mc_version}-{build}")),
        "neoforge" if mc_version == "1.20.1" => (
            "https://maven.neoforged.net/releases/net/neoforged/forge",
            "forge",
            format!("{mc_version}-{build}"),
        ),
        "neoforge" => (
            "https://maven.neoforged.net/releases/net/neoforged/neoforge",
            "neoforge",
            build.to_string(),
        ),
        _ => {
            return Err(ErrorKind::InputError(
                "Unsupported installer".to_string(),
            )
            .as_error());
        }
    };
    let filename = format!("{artifact}-{version}-installer.jar");
    Ok((format!("{base}/{version}/{filename}"), filename))
}

async fn install_loader(
    server_id: &str,
    loader: &str,
    mc_version: &str,
    build: &str,
    java_path: Option<String>,
) -> Result<()> {
    let dir = server_path(server_id).await?;
    let mut manifest = read_manifest(&dir).await?;
    if manifest.server_type != loader || manifest.game_version != mc_version {
        return Err(ErrorKind::InputError(
            "Installer does not match this server".to_string(),
        )
        .as_error());
    }
    let (url, filename) = installer_download(loader, mc_version, build)?;
    manifest.install_state = Some(InstallState::Incomplete);
    manifest.install_error = None;
    manifest.loader_version = Some(build.to_string());
    write_manifest(&dir, &manifest).await?;
    let result = async {
        log(
            server_id,
            &format!("Downloading {loader} installer ({filename})"),
        )
        .await?;
        download_to_dir(server_id, &dir, &url, &filename, None).await?;
        run_installer(
            server_id,
            &dir,
            &filename,
            java_path.as_deref().or(manifest.java_path.as_deref()),
            loader,
        )
        .await
    }
    .await;
    // Include download and Java launch failures in the retryable install state.
    let mut manifest = read_manifest(&dir).await?;
    manifest.install_state = if result.is_ok() {
        None
    } else {
        Some(InstallState::Failed)
    };
    manifest.install_error = result.as_ref().err().map(ToString::to_string);
    write_manifest(&dir, &manifest).await?;
    result
}

async fn installer_manifest(
    archive: &ZipFileReader,
    name: &str,
) -> Result<InstallerManifest> {
    let (index, entry) = archive
        .file()
        .entries()
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.filename().as_str().ok() == Some(name))
        .filter(|(_, entry)| {
            entry.uncompressed_size() <= MAX_INSTALLER_MANIFEST_BYTES
        })
        .ok_or_else(|| {
            ErrorKind::InputError(format!(
                "NeoForge installer is missing a readable {name}"
            ))
            .as_error()
        })?;
    let mut json = Vec::new();
    let mut reader = archive.reader_with_entry(index).await?;
    (&mut reader)
        .take(entry.uncompressed_size() + 1)
        .read_to_end(&mut json)
        .await
        .map_err(async_zip::error::ZipError::UpstreamReadError)?;
    if json.len() as u64 != entry.uncompressed_size()
        || reader.compute_hash() != entry.crc32()
    {
        return Err(ErrorKind::InputError(format!(
            "NeoForge installer has an invalid manifest entry: {name}"
        ))
        .into());
    }
    Ok(serde_json::from_slice(&json)?)
}

fn neoforge_maven_artifact(
    library: InstallerLibrary,
) -> Option<(String, LibraryDownload)> {
    let artifact = library.downloads?.artifact?;
    let path = daedalus::get_path_from_artifact(&library.name).ok()?;
    if artifact.path.as_deref() != Some(path.as_str())
        || !(path.ends_with(".jar") || path.ends_with(".zip"))
        || !path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'+')
        })
        || safe_relative_join(Path::new(""), &path).is_err()
        || artifact.url != format!("{NEOFORGE_MAVEN}/{path}")
        || artifact.size == 0
        || artifact.sha1.len() != 40
        || !artifact
            .sha1
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some((path, artifact))
}

async fn neoforge_library_downloads(
    installer: &Path,
) -> Result<Vec<(String, DownloadRequest)>> {
    let archive = ZipFileReader::new(installer).await?;
    let profile = installer_manifest(&archive, "install_profile.json").await?;
    let version = installer_manifest(
        &archive,
        profile
            .json
            .as_deref()
            .unwrap_or("version.json")
            .trim_start_matches('/'),
    )
    .await?;
    let mut libraries = BTreeMap::new();
    for (path, artifact) in version
        .libraries
        .into_iter()
        .chain(profile.libraries)
        .filter_map(neoforge_maven_artifact)
    {
        let embedded = format!("maven/{path}");
        if archive.file().entries().iter().any(|entry| {
            entry.filename().as_str().ok() == Some(embedded.as_str())
        }) {
            continue;
        }
        libraries
            .entry(path)
            .and_modify(|existing: &mut Option<LibraryDownload>| {
                if existing.as_ref().is_some_and(|existing| {
                    existing.sha1 != artifact.sha1
                        || existing.size != artifact.size
                }) {
                    *existing = None;
                }
            })
            .or_insert(Some(artifact));
    }

    Ok(libraries
        .into_iter()
        .filter_map(|(path, artifact)| {
            let artifact = artifact?;
            let mut integrity = Integrity::sha1(artifact.sha1)
                .with_size(u64::from(artifact.size));
            if path.ends_with(".jar") {
                integrity.content = ContentValidation::Jar;
            }
            Some((
                format!("libraries/{path}"),
                DownloadRequest::new(artifact.url, ResourceClass::Loader)
                    .with_integrity(integrity),
            ))
        })
        .collect())
}

async fn prefetch_neoforge_libraries(
    server_id: &str,
    dir: &Path,
    installer: &Path,
) -> Result<()> {
    let libraries = neoforge_library_downloads(installer).await?;
    if libraries.is_empty() {
        return Ok(());
    }
    log(server_id, "Preparing NeoForge installer libraries").await?;
    let state = State::get().await?;
    for (path, request) in libraries {
        let url = request.url.clone();
        if let Err(error) = download_server_file(
            server_id, &state, dir, &path, request, None, None,
        )
        .await
        {
            log(server_id, &format!("Could not prepare {url}: {error}"))
                .await
                .ok();
        }
    }
    Ok(())
}

fn installer_log_lines(text: &str, failed: bool) -> impl Iterator<Item = &str> {
    let lines = text.lines();
    let skipped = lines.clone().count().saturating_sub(20);
    let exception = failed
        .then(|| {
            lines.clone().take(skipped).find(|line| {
                let line = line.trim_start();
                line.starts_with("Exception in thread ")
                    || line.split_once(':').is_some_and(|(kind, _)| {
                        kind.contains('.')
                            && (kind.ends_with("Exception")
                                || kind.ends_with("Error"))
                    })
            })
        })
        .flatten();
    exception.into_iter().chain(lines.skip(skipped))
}

/// Runs an already downloaded installer, also used after unpacking a modpack.
/// The caller owns install state so a partially installed pack never looks ready.
pub(super) async fn run_installer(
    server_id: &str,
    dir: &Path,
    installer_name: &str,
    java_path: Option<&str>,
    loader: &str,
) -> Result<()> {
    let installer_path = safe_relative_join(dir, installer_name)?;
    let java = java_path.unwrap_or("java");
    let java = java_environment::console_java_executable(java)?;

    if loader == "neoforge"
        && let Err(error) =
            prefetch_neoforge_libraries(server_id, dir, &installer_path).await
    {
        log(
            server_id,
            &format!("Could not prepare NeoForge installer libraries: {error}"),
        )
        .await
        .ok();
    }

    log(
        server_id,
        &format!("Running {loader} installer (this may take a while)"),
    )
    .await?;
    let mut command = java_environment::tokio_command(&java);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let output = command
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installServer")
        .arg(dir)
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            ErrorKind::LauncherError(format!(
                "Failed to run {loader} installer: {e}"
            ))
            .as_error()
        })?;

    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        for line in installer_log_lines(&text, !output.status.success()) {
            log(server_id, line).await.ok();
        }
    }
    if !output.status.success() {
        return Err(ErrorKind::LauncherError(format!(
            "{loader} installer failed. Check the installer output and selected Java version."
        )).as_error());
    }
    if loader == "neoforge" {
        let manifest = read_manifest(dir).await?;
        super::lifecycle::neoforge_launch_args(
            dir,
            &manifest.game_version,
            manifest.loader_version.as_deref(),
        )?;
    }

    // Ensure eula.txt exists (eula=false) so the manual-start gate can offer it
    // without booting the jar.
    let eula_path = dir.join("eula.txt");
    if !eula_path.exists() {
        tokio::fs::write(&eula_path, "eula=false\n")
            .await
            .map_err(|e| IOError::with_path(e, &eula_path))?;
    }

    log(server_id, &format!("{loader} server files installed"))
        .await
        .ok();
    Ok(())
}

async fn log(server_id: &str, line: &str) -> Result<()> {
    emit_server(
        server_id,
        ServerPayloadType::Log {
            line: line.to_string(),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use crate::state::DownloadSourceMode;
    use crate::util::download::constants::BMCLAPI_BASE_URL;
    use crate::util::fetch::{
        DownloadRouteSource, resolve_download_routes_for,
    };

    use super::*;

    #[test]
    fn neoforge_installer_uses_official_modern_and_legacy_coordinates() {
        let (url, filename) =
            installer_download("neoforge", "1.21.1", "21.1.172").unwrap();
        assert_eq!(filename, "neoforge-21.1.172-installer.jar");
        assert_eq!(
            url,
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar"
        );
        let (url, filename) =
            installer_download("neoforge", "1.20.1", "47.1.106").unwrap();
        assert_eq!(filename, "forge-1.20.1-47.1.106-installer.jar");
        assert_eq!(
            url,
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"
        );
        assert!(
            installer_download("forge", "1.20.1", "47.4.22")
                .unwrap()
                .0
                .starts_with(FORGE_MAVEN)
        );
    }

    #[test]
    fn installer_rejects_invalid_version_paths() {
        for build in ["", "../../server", "21.1.1/other", "21.1.1?other"] {
            assert!(installer_download("neoforge", "1.21.1", build).is_err());
        }
        assert!(
            installer_download("neoforge", "26.1", "26.1.0.19-beta").is_ok()
        );
    }

    const UNIVERSAL_PATH: &str =
        "net/neoforged/neoforge/26.3.0.113/neoforge-26.3.0.113-universal.jar";
    const UNIVERSAL_SHA1: &str = "7aef686b46a81426526f936f11c15b11962a3877";

    fn installer_library(
        name: &str,
        path: &str,
        sha1: &str,
        size: u32,
    ) -> serde_json::Value {
        json!({
            "name": name,
            "downloads": {"artifact": {
                "path": path,
                "url": format!("{NEOFORGE_MAVEN}/{path}"),
                "size": size,
                "sha1": sha1,
            }},
        })
    }

    #[tokio::test]
    async fn neoforge_prefetch_plans_verified_external_libraries_once() {
        let universal = installer_library(
            "net.neoforged:neoforge:26.3.0.113:universal",
            UNIVERSAL_PATH,
            UNIVERSAL_SHA1,
            7_183_562,
        );
        let embedded_path = "net/neoforged/test/1/test-1.jar";
        let embedded = installer_library(
            "net.neoforged:test:1",
            embedded_path,
            UNIVERSAL_SHA1,
            1,
        );
        let processor_path = "net/neoforged/tool/1/tool-1.zip";
        let processor = installer_library(
            "net.neoforged:tool:1@zip",
            processor_path,
            UNIVERSAL_SHA1,
            2,
        );
        let mut installer = tempfile::NamedTempFile::new().unwrap();
        let mut archive = zip::ZipWriter::new(installer.as_file_mut());
        for (name, manifest) in [
            (
                "install_profile.json",
                json!({"json": "/launcher.json", "libraries": [universal, processor, embedded]}),
            ),
            ("launcher.json", json!({"libraries": [universal]})),
        ] {
            archive
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(manifest.to_string().as_bytes()).unwrap();
        }
        archive
            .start_file(
                format!("maven/{embedded_path}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"embedded").unwrap();
        archive.finish().unwrap();

        let downloads =
            neoforge_library_downloads(installer.path()).await.unwrap();
        assert_eq!(downloads.len(), 2);
        let (path, request) = &downloads[0];
        assert_eq!(path, &format!("libraries/{UNIVERSAL_PATH}"));
        assert_eq!(request.url, format!("{NEOFORGE_MAVEN}/{UNIVERSAL_PATH}"));
        assert_eq!(request.resource, ResourceClass::Loader);
        assert_eq!(request.integrity.sha1.as_deref(), Some(UNIVERSAL_SHA1));
        assert_eq!(request.integrity.size, Some(7_183_562));
        assert_eq!(request.integrity.content, ContentValidation::Jar);
        assert_eq!(downloads[1].0, format!("libraries/{processor_path}"));
        assert_eq!(downloads[1].1.integrity.content, ContentValidation::None);
        let routes = resolve_download_routes_for(
            &request.url,
            request.resource,
            DownloadSourceMode::MirrorPreferred,
        );
        assert_eq!(routes[0].source, DownloadRouteSource::Bmclapi);
        assert_eq!(
            routes[0].url,
            format!("{BMCLAPI_BASE_URL}/maven/{UNIVERSAL_PATH}")
        );
        assert_eq!(routes[1].source, DownloadRouteSource::Official);
    }

    #[test]
    fn neoforge_prefetch_rejects_unverified_or_noncanonical_artifacts() {
        let library = installer_library(
            "net.neoforged:neoforge:26.3.0.113:universal",
            UNIVERSAL_PATH,
            UNIVERSAL_SHA1,
            7_183_562,
        );
        for (field, value) in [
            ("path", json!("../../other.jar")),
            ("sha1", json!("not-a-checksum")),
            ("size", json!(0)),
            (
                "url",
                json!(format!(
                    "https://maven.neoforged.net.example.com/releases/{UNIVERSAL_PATH}"
                )),
            ),
        ] {
            let mut invalid = library.clone();
            invalid["downloads"]["artifact"][field] = value;
            assert!(
                neoforge_maven_artifact(
                    serde_json::from_value(invalid).unwrap()
                )
                .is_none(),
                "accepted invalid {field}"
            );
        }
    }

    #[test]
    fn failed_installer_output_keeps_the_exception_before_its_tail() {
        let exception = "java.net.ConnectException: Connection refused";
        let failure = "There was an error during installation";
        let output =
            format!("{exception}\n{}{failure}", "\tat example\n".repeat(25));
        let lines = installer_log_lines(&output, true).collect::<Vec<_>>();
        assert_eq!(lines.len(), 21);
        assert_eq!(lines.first(), Some(&exception));
        assert_eq!(lines.last(), Some(&failure));
        assert_eq!(installer_log_lines(&output, false).count(), 20);
        assert_eq!(installer_log_lines(exception, true).count(), 1);
    }
}
