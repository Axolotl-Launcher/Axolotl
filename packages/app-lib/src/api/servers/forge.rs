//! Forge server installation: downloading the official installer and running it
//! headlessly to materialize the server launcher (run script, `@args` files, and
//! the mod loader) into a managed server directory.
//!
//! Forge ships a bootstrapper rather than a ready-to-run jar. The launcher jar is
//! produced by `java -jar forge-installer.jar --installServer <dir>`, which lays
//! down the loader, its libraries, and the `@args` launch files. The regular
//! `servers.start` flow then boots that output (see `lifecycle::forge_launch_args`).

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
    let dir = server_path(server_id).await?;

    let mut manifest = read_manifest(&dir).await?;
    manifest.install_state = Some(InstallState::Incomplete);
    manifest.install_error = None;
    write_manifest(&dir, &manifest).await?;
    drop(manifest);

    let installer_name = format!("forge-{mc_version}-{build}-installer.jar");
    let installer_path = safe_relative_join(&dir, &installer_name)?;
    let installer_url =
        format!("{FORGE_MAVEN}/{mc_version}-{build}/{installer_name}");

    log(
        server_id,
        &format!("Downloading Forge installer ({installer_name})"),
    )
    .await?;
    download_to_dir(server_id, &dir, &installer_url, &installer_name, None)
        .await?;

    let java = java_path.clone().unwrap_or_else(|| "java".to_string());

    log(server_id, "Running Forge installer (this may take a while)").await?;
    let output = java_environment::tokio_command(&java)
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
                "Failed to run Forge installer: {e}"
            ))
            .as_error()
        })?;

    // The installer is verbose; surface a condensed tail so failures are diagnosable.
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().rev().take(20) {
        log(server_id, line).await.ok();
    }

    if !output.status.success() {
        let mut manifest = read_manifest(&dir).await?;
        manifest.install_state = Some(InstallState::Failed);
        manifest.install_error =
            Some("Forge installer exited with an error".to_string());
        write_manifest(&dir, &manifest).await?;
        return Err(ErrorKind::LauncherError(
			"Forge installer failed. Check that the selected Java version supports this game version."
				.to_string(),
		)
		.as_error());
    }

    // Ensure eula.txt exists (eula=false) so the manual-start gate can offer it
    // without booting the jar.
    let eula_path = dir.join("eula.txt");
    if !eula_path.exists() {
        tokio::fs::write(&eula_path, "eula=false\n")
            .await
            .map_err(|e| IOError::with_path(e, &eula_path))?;
    }

    let mut manifest = read_manifest(&dir).await?;
    manifest.install_state = None;
    manifest.install_error = None;
    write_manifest(&dir, &manifest).await?;
    log(server_id, "Forge server files installed").await.ok();
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
