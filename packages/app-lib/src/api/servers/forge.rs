//! Forge and NeoForge server installation: downloading the official installer and running it
//! headlessly to materialize the server launcher (run script, `@args` files, and
//! the mod loader) into a managed server directory.
//!
//! Forge ships a bootstrapper rather than a ready-to-run jar. The launcher jar is
//! produced by `java -jar forge-installer.jar --installServer <dir>`, which lays
//! down the loader, its libraries, and the `@args` launch files. The regular
//! `servers.start` flow then boots that output (see `lifecycle::forge_launch_args`).

use std::path::Path;

use crate::util::java_environment;

use crate::event::ServerPayloadType;
use crate::event::emit::emit_server;
use crate::util::io::IOError;
use crate::{ErrorKind, Result};

use super::files::{download_to_dir, safe_relative_join};
use super::manifest::{
    InstallState, read_manifest, server_path, write_manifest,
};

const FORGE_MAVEN: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge";

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
        .arg(&dir)
        .current_dir(&dir)
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
        for line in text
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
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
}
