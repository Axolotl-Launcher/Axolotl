//! Validation and launch arguments specific to externally imported servers.
use super::super::manifest::ServerManifest;
use super::source::{read_bounded, safe_source_path};
use super::{input, inspect_server_import};
use crate::Result;
use std::path::Path;

/// Returns no override for existing managed servers, preserving their launch flow.
pub(in crate::api::servers) fn launch_args(
    dir: &Path,
    manifest: &ServerManifest,
) -> Result<Option<Vec<String>>> {
    if !manifest.external_import {
        return Ok(None);
    }
    if manifest.install_state.is_some() {
        return Err(input("Finish installing this server before starting it"));
    }
    imported_launch_args(dir, manifest).map(Some).map_err(input)
}

/// Use upstream preparation for unattended Fabric launchers while retaining
/// installed launchers which manage their dependencies through their classpath.
pub(in crate::api::servers) async fn prepare_fabric_launch(
    id: &str,
    dir: &Path,
    manifest: &ServerManifest,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<()> {
    if manifest.external_import {
        let path = safe_source_path(
            dir,
            &super::super::manifest::resolve_jar_name(manifest),
        )
        .map_err(input)?;
        let jar = zip::ZipArchive::new(
            std::fs::File::open(path).map_err(|e| input(e.to_string()))?,
        )
        .map_err(|e| input(e.to_string()))?;
        if jar.index_for_name("install.properties").is_none() {
            return Ok(());
        }
    }
    super::super::fabric::prepare(id, dir, manifest, cancellation).await
}

pub(super) fn imported_launch_args(
    dir: &Path,
    manifest: &ServerManifest,
) -> std::result::Result<Vec<String>, String> {
    if manifest.server_type == "forge" && manifest.jar_name.is_some() {
        let actual = inspect_server_import(dir)?
            .ok_or("Forge launch JAR could not be verified")?;
        if actual.problem.is_some()
            || actual.server_type != "forge"
            || actual.jar_name != manifest.jar_name
            || actual.game_version != manifest.game_version
            || actual.loader_version != manifest.loader_version
        {
            return Err(
                "Forge launch JAR does not match the managed server".into()
            );
        }
    }
    if manifest.server_type == "forge" && manifest.jar_name.is_none() {
        let build = manifest
            .loader_version
            .as_deref()
            .ok_or("Missing Forge version")?;
        let args_name = if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        let relative = format!(
            "libraries/net/minecraftforge/forge/{}-{build}/{args_name}",
            manifest.game_version
        );
        let file = safe_source_path(dir, &relative).map_err(|_| {
            "Forge launch files are missing. Retry installation".to_string()
        })?;
        let text = String::from_utf8(read_bounded(
            std::fs::File::open(file).map_err(|e| e.to_string())?,
            2 * 1024 * 1024,
        )?)
        .map_err(|e| e.to_string())?;
        validate_args(dir, &text)?;
        // Never inherit user_jvm_args.txt: memory/Java/JVM settings belong to the new managed manifest.
        return Ok(vec![format!("@{relative}"), "nogui".into()]);
    }
    let jar = manifest
        .jar_name
        .as_deref()
        .ok_or("Missing server launcher")?;
    let path = safe_source_path(dir, jar)?;
    if !path.is_file() {
        return Err("Server launcher is missing".into());
    }
    validate_jar_paths(dir, &path)?;
    Ok(vec!["-jar".into(), jar.into(), "nogui".into()])
}

pub(super) fn validate_args(
    dir: &Path,
    text: &str,
) -> std::result::Result<(), String> {
    if !text
        .split_whitespace()
        .any(|token| token == "cpw.mods.bootstraplauncher.BootstrapLauncher")
        || !text
            .split_whitespace()
            .any(|token| token == "--module-path" || token == "-p")
    {
        return Err(
            "Forge launch files are incomplete. Retry installation".into()
        );
    }
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        for token in line.split_whitespace() {
            let token = token.trim_matches(['\'', '"']);
            if token.starts_with('@')
                || token.starts_with("-X")
                || token.starts_with("-javaagent")
                || token.starts_with("-agent")
                || token.starts_with("-jar")
                || (token.starts_with("-D")
                    && ![
                        "-DlibraryDirectory=",
                        "-DlegacyClassPath=",
                        "-DignoreList=",
                        "-DmergeModules=",
                        "-Dfml.",
                        "-Djava.net.preferIPv6Addresses=",
                    ]
                    .iter()
                    .any(|prefix| token.starts_with(prefix)))
            {
                return Err("The package contains custom JVM launch instructions. Review them before importing".into());
            }
            let value = token.split_once('=').map_or(token, |(_, v)| v);
            for component in value.split(if cfg!(windows) { ';' } else { ':' })
            {
                if component.contains('/') || component.contains('\\') {
                    // Module names (java.base/java.util=...) are not file paths.
                    if token.starts_with("--add-")
                        || component.starts_with("java.")
                        || component == "ALL-MODULE-PATH"
                    {
                        continue;
                    }
                    safe_source_path(dir, component).map_err(|_| format!("A launch dependency is missing or outside the server folder: {component}"))?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_jar_paths(
    dir: &Path,
    path: &Path,
) -> std::result::Result<(), String> {
    let mut jar = zip::ZipArchive::new(
        std::fs::File::open(path).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let bytes = read_bounded(
        jar.by_name("META-INF/MANIFEST.MF")
            .map_err(|e| e.to_string())?,
        2 * 1024 * 1024,
    )?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    if let Some(classpath) =
        super::detect::manifest_attribute(&text, "Class-Path")
    {
        for relative in classpath.split_whitespace() {
            safe_source_path(dir, relative).map_err(|_| format!("A launcher dependency is missing or outside the server folder: {relative}"))?;
        }
    }
    for key in ["Launcher-Agent-Class", "Premain-Class"] {
        if super::detect::manifest_attribute(&text, key).is_some() {
            return Err(
                "Custom Java agents are not supported during server import"
                    .into(),
            );
        }
    }
    let props = dir.join("fabric-server-launcher.properties");
    if props.exists() {
        let text = String::from_utf8(read_bounded(
            std::fs::File::open(props).map_err(|e| e.to_string())?,
            2 * 1024 * 1024,
        )?)
        .map_err(|e| e.to_string())?;
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=')
                && key.trim() == "serverJar"
            {
                safe_source_path(dir, value.trim())?;
            }
        }
    }
    Ok(())
}
