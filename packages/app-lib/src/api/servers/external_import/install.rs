//! Materializes missing launchers using official downloads and existing Forge installation.
use super::super::manifest::{
    InstallState, ServerManifest, read_manifest, server_path, write_manifest,
};
use super::super::{files, forge};
use super::launch::imported_launch_args;
use super::{input, inspect_server_import};
use crate::{Result, State};
use std::path::Path;
use uuid::Uuid;

pub(super) async fn finish_import_install(id: &str) -> Result<()> {
    let _operation = super::acquire_operation(id)?;
    finish_import_install_locked(id).await
}

/// Caller retains the operation guard, including while publishing a new import.
pub(super) async fn finish_import_install_locked(id: &str) -> Result<()> {
    let result = async {
        let dir = server_path(id).await?;
        let mut manifest = read_manifest(&dir).await?;
        if !manifest.external_import || manifest.install_state.is_none() || super::super::lifecycle::is_running(id) {
            return Err(input("This server does not need an import repair"));
        }
        manifest.install_state = Some(InstallState::Incomplete);
        manifest.install_error = None;
        write_manifest(&dir, &manifest).await?;
        let result = match manifest.server_type.as_str() {
            "forge" => ensure_forge_launcher(id, &dir, &manifest).await,
            "fabric" if manifest.jar_name.is_none() => {
                install_fabric_launcher(id, &dir, &mut manifest).await
            }
            "vanilla" if manifest.jar_name.is_none() => {
                install_vanilla_launcher(id, &dir, &mut manifest).await
            }
            _ => Ok(()),
        };
        let result = if result.is_ok() && manifest.server_type == "forge" {
            async {
                let installed = inspect_server_import(&dir).map_err(input)?.ok_or_else(|| input("Forge installation did not produce a recognized server"))?;
                if installed.problem.is_some() || installed.server_type != "forge" || installed.game_version != manifest.game_version || installed.loader_version != manifest.loader_version || installed.needs_install {
                    return Err(input("Installed Forge files do not match the configured server"));
                }
                manifest.jar_name = installed.jar_name;
                write_manifest(&dir, &manifest).await
            }.await
        } else { result };
        let result = result.and_then(|()| {
            imported_launch_args(&dir, &manifest)
                .map(|_| ())
                .map_err(input)
        });
        let mut manifest = read_manifest(&dir).await?;
        match result {
            Ok(()) => {
                manifest.install_state = None;
                manifest.install_error = None;
            }
            Err(error) => {
                manifest.install_state = Some(InstallState::Failed);
                manifest.install_error = Some(error.to_string());
            }
        }
        write_manifest(&dir, &manifest).await
    }
    .await;
    result
}

async fn ensure_forge_launcher(
    id: &str,
    dir: &Path,
    manifest: &ServerManifest,
) -> Result<()> {
    if let Some(preview) = inspect_server_import(dir).map_err(input)? {
        if preview.problem.is_some()
            || preview.server_type != manifest.server_type
            || preview.game_version != manifest.game_version
            || preview.loader_version != manifest.loader_version
        {
            return Err(input(
                "Existing server files conflict with the configured Forge version",
            ));
        }
        if !preview.needs_install {
            return Ok(());
        }
    }
    install_forge_launcher(id, dir, manifest).await
}

async fn install_forge_launcher(
    id: &str,
    dir: &Path,
    manifest: &ServerManifest,
) -> Result<()> {
    let build = manifest
        .loader_version
        .as_deref()
        .ok_or_else(|| input("Missing Forge version"))?;
    let (url, filename) =
        forge::installer_download("forge", &manifest.game_version, build)?;
    files::download_to_dir(id, dir, &url, &filename, None).await?;
    // The import owns readiness; reuse the installer's caller-owned-state API.
    forge::run_installer_with_timeout(
        id,
        dir,
        &filename,
        manifest.java_path.as_deref(),
        "forge",
        Some(std::time::Duration::from_secs(15 * 60)),
    )
    .await
}

/// Acquire the official executable launcher for the declared versions. Its
/// first manual start downloads runtime dependencies; no package script runs.
async fn install_fabric_launcher(
    id: &str,
    dir: &Path,
    manifest: &mut ServerManifest,
) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Installer {
        version: String,
        stable: bool,
    }
    let state = State::get().await?;
    let installers: Vec<Installer> = crate::util::fetch::fetch_json(
        reqwest::Method::GET,
        "https://meta.fabricmc.net/v2/versions/installer",
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    let installer = installers
        .into_iter()
        .find(|entry| entry.stable && super::detect::version(&entry.version))
        .ok_or_else(|| input("No stable Fabric installer is available"))?;
    let loader = manifest
        .loader_version
        .as_deref()
        .ok_or_else(|| input("Missing Fabric version"))?;
    if !super::detect::version(&manifest.game_version)
        || !super::detect::version(loader)
    {
        return Err(input("Invalid Fabric version"));
    }
    // A unique managed name avoids replacing an unrecognized launcher supplied by the package.
    let name = format!("axolotl-fabric-{}.jar", Uuid::new_v4());
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
        manifest.game_version, loader, installer.version
    );
    files::download_to_dir(id, dir, &url, &name, None).await?;
    let actual =
        inspect_server_import(dir).map_err(input)?.ok_or_else(|| {
            input("Downloaded Fabric launcher could not be verified")
        })?;
    if actual.problem.is_some()
        || actual.server_type != "fabric"
        || actual.game_version != manifest.game_version
        || actual.loader_version != manifest.loader_version
        || actual.jar_name.as_deref() != Some(&name)
    {
        return Err(input(
            "Downloaded Fabric launcher does not match the declared server",
        ));
    }
    manifest.jar_name = Some(name);
    write_manifest(dir, manifest).await
}

async fn install_vanilla_launcher(
    id: &str,
    dir: &Path,
    manifest: &mut ServerManifest,
) -> Result<()> {
    let state = State::get().await?;
    let versions: daedalus::minecraft::VersionManifest =
        crate::util::fetch::fetch_json(
            reqwest::Method::GET,
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            None,
            None,
            None,
            &state.fetch_semaphore,
            &state.pool,
        )
        .await?;
    let url = &versions
        .versions
        .iter()
        .find(|entry| entry.id == manifest.game_version)
        .ok_or_else(|| input("Minecraft server version was not found"))?
        .url;
    let version: serde_json::Value = crate::util::fetch::fetch_json(
        reqwest::Method::GET,
        url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    let server = &version["downloads"]["server"];
    let url = server["url"].as_str().ok_or_else(|| {
        input("This Minecraft version has no server download")
    })?;
    let sha1 = server["sha1"]
        .as_str()
        .ok_or_else(|| input("Missing Minecraft server checksum"))?;
    let name = format!("axolotl-vanilla-{}.jar", Uuid::new_v4());
    files::download_to_dir(id, dir, url, &name, Some(sha1.into())).await?;
    manifest.jar_name = Some(name);
    write_manifest(dir, manifest).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn installed_forge_revalidates_without_downloading_or_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let libs = root
            .path()
            .join("libraries/net/minecraftforge/forge/1.20.1-47.4.16");
        std::fs::create_dir_all(&libs).unwrap();
        let args = "--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.4.16";
        std::fs::write(libs.join("unix_args.txt"), args).unwrap();
        std::fs::write(libs.join("win_args.txt"), args).unwrap();
        let mut manifest: ServerManifest = serde_json::from_value(serde_json::json!({
            "id": "forge-retry-fixture", "name": "Forge retry", "serverType": "forge",
            "gameVersion": "1.20.1", "loaderVersion": "47.4.16", "createdAt": "2026-10-23T00:00:00Z"
        })).unwrap();
        // No State, Java or network is configured: a needless installer would fail.
        assert!(manifest.jar_name.is_none());
        ensure_forge_launcher("fixture", root.path(), &manifest)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(libs.join("unix_args.txt")).unwrap(),
            args
        );
        manifest.loader_version = Some("47.0.0".into());
        assert!(
            ensure_forge_launcher("fixture", root.path(), &manifest)
                .await
                .unwrap_err()
                .to_string()
                .contains("conflict")
        );
        assert_eq!(
            std::fs::read_to_string(libs.join("unix_args.txt")).unwrap(),
            args
        );
    }
}
