//! Materializes missing launchers using official downloads and existing Forge installation.
use super::super::manifest::{
    InstallState, ServerManifest, read_manifest, server_path, write_manifest,
};
use super::super::{files, forge};
use super::launch::imported_launch_args;
use super::{input, inspect_server_import};
use crate::{Result, State};
use dashmap::DashSet;
use std::path::Path;
use std::sync::LazyLock;
use uuid::Uuid;

static INSTALLING: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);

pub(super) async fn finish_import_install(id: &str) -> Result<()> {
    if !INSTALLING.insert(id.into()) {
        return Err(input("This server is already being installed"));
    }
    let result = async {
        let dir = server_path(id).await?;
        let mut manifest = read_manifest(&dir).await?;
        manifest.install_state = Some(InstallState::Incomplete);
        manifest.install_error = None;
        write_manifest(&dir, &manifest).await?;
        let result = match manifest.server_type.as_str() {
            "forge" => {
                if let Some(build) = manifest.loader_version.as_deref() {
                    forge::install_forge(
                        id,
                        &manifest.game_version,
                        build,
                        manifest.java_path.clone(),
                    )
                    .await
                } else {
                    Err(input("Missing Forge version"))
                }
            }
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
    INSTALLING.remove(id);
    result
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
