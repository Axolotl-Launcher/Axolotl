//! External server import: inspection, staging, installation, and lifecycle adapters.
//! Existing server entry points delegate here; source packages are never modified.
mod detect;
mod install;
mod launch;
mod operations;
mod source;
#[cfg(test)]
mod tests;

pub(super) use launch::{launch_args, prepare_fabric_launch};
pub(super) use operations::acquire as acquire_operation;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::manifest::{
    InstallState, ServerManifest, read_manifest, sanitize_folder_name,
    server_path, write_manifest,
};
use crate::{ErrorKind, Result, State};
pub use detect::{
    ServerImportPreview, inspect_configurable_server_import,
    inspect_server_import,
};
use source::safe_source_path;

use crate::api::drop_classifier::{
    MAX_ZIP_EXTRACTION_BYTES as MAX_BYTES,
    MAX_ZIP_EXTRACTION_ENTRIES as MAX_FILES,
};
fn input(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).as_error()
}

fn copy_tree(
    source: &Path,
    dest: &Path,
    depth: usize,
    count: &mut usize,
    bytes: &mut u64,
    locks: &mut Vec<std::fs::File>,
) -> std::result::Result<(), String> {
    if depth > 64 {
        return Err("Folder nesting exceeds the import limit".into());
    }
    let session = source.join("session.lock");
    if session.is_symlink() {
        return Err(
            "Symbolic links and special files cannot be imported".into()
        );
    }
    if session.is_file() {
        let file = std::fs::File::open(&session).map_err(|e| e.to_string())?;
        file.try_lock_shared().map_err(|_| "The source world is in use. Stop the source server before importing".to_string())?;
        locks.push(file);
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if crate::api::drop_classifier::is_noise_entry(&name.to_string_lossy())
            || (depth == 0 && name == "axolotl-server.json")
        {
            continue;
        }
        let meta = entry.file_type().map_err(|e| e.to_string())?;
        if meta.is_symlink() || (!meta.is_dir() && !meta.is_file()) {
            return Err(
                "Symbolic links and special files cannot be imported".into()
            );
        }
        *count += 1;
        if *count > MAX_FILES {
            return Err("Server contains too many files".into());
        }
        let target =
            super::files::safe_relative_join(dest, &name.to_string_lossy())
                .map_err(|e| e.to_string())?;
        if meta.is_dir() {
            copy_tree(&entry.path(), &target, depth + 1, count, bytes, locks)?;
        } else {
            let mut reader =
                std::fs::File::open(entry.path()).map_err(|e| e.to_string())?;
            let mut writer = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(target)
                .map_err(|e| e.to_string())?;
            let mut buffer = vec![0; 64 * 1024];
            loop {
                let read =
                    reader.read(&mut buffer).map_err(|e| e.to_string())?;
                if read == 0 {
                    break;
                }
                *bytes = bytes
                    .checked_add(read as u64)
                    .ok_or("Server size exceeds import limit")?;
                if *bytes > MAX_BYTES {
                    return Err("Server exceeds the 32 GiB import limit".into());
                }
                writer
                    .write_all(&buffer[..read])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn prepare_source(
    source: &Path,
    content: &Path,
    expected: &ServerImportPreview,
) -> std::result::Result<(), String> {
    let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
    let destination_parent = dunce::canonicalize(
        content.parent().ok_or("Missing staging directory")?,
    )
    .map_err(|e| e.to_string())?;
    if source.is_dir() && destination_parent.starts_with(&source) {
        return Err(
            "The import destination must be outside the source directory"
                .into(),
        );
    }
    let selected = source::Source::select(&source, &expected.archives)?;
    match selected {
        source::Source::Folder(root) => {
            let root = if expected.inner_base.is_empty() {
                root
            } else {
                safe_source_path(&root, &expected.inner_base)?
            };
            copy_tree(&root, content, 0, &mut 0, &mut 0, &mut Vec::new())?;
        }
        source::Source::Zip(mut archive, _) => {
            source::validate_archive(&mut archive)?;
            let extracted = content.parent().unwrap().join("extracted");
            std::fs::create_dir(&extracted).map_err(|e| e.to_string())?;
            crate::api::drop_classifier::extract_archive_to_dir(
                &mut archive,
                &extracted,
                None,
            )?;
            let root = if expected.inner_base.is_empty() {
                extracted
            } else {
                safe_source_path(&extracted, &expected.inner_base)?
            };
            std::fs::rename(&root, content).map_err(|e| e.to_string())?;
            let source_manifest = content.join("axolotl-server.json");
            if source_manifest.exists() {
                std::fs::remove_file(source_manifest)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    let actual = inspect_configurable_server_import(content)?
        .ok_or("The copied package no longer contains a recognized server")?;
    let mut expected = expected.clone();
    expected.source_is_directory = true;
    expected.inner_base.clear();
    expected.archives.clear();
    if actual != expected {
        return Err("Server files changed during import. Inspect the source and try again".into());
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerImportConfiguration {
    pub server_type: String,
    pub game_version: String,
    pub loader_version: Option<String>,
}

fn apply_configuration(
    mut preview: ServerImportPreview,
    configuration: Option<ServerImportConfiguration>,
) -> Result<ServerImportPreview> {
    if preview.problem.as_deref() == Some("configuration-required") {
        let configuration = configuration.ok_or_else(|| {
            input("Choose the server type and exact versions before importing")
        })?;
        if !matches!(
            configuration.server_type.as_str(),
            "forge" | "fabric" | "vanilla"
        ) || !detect::version(&configuration.game_version)
            || (configuration.server_type != "vanilla"
                && !configuration
                    .loader_version
                    .as_deref()
                    .is_some_and(detect::version))
        {
            return Err(input(
                "Choose a supported server type and valid exact versions",
            ));
        }
        preview.server_type = configuration.server_type;
        preview.game_version = configuration.game_version;
        preview.loader_version = if preview.server_type == "vanilla" {
            None
        } else {
            configuration.loader_version
        };
        preview.problem = None;
        preview.needs_install = true;
        preview.jar_name = None;
    } else if configuration.is_some() || preview.problem.is_some() {
        return Err(input(
            "Resolve the detected server package problem before importing; detected versions cannot be overridden",
        ));
    }
    Ok(preview)
}

pub async fn import_external_server(
    source_path: String,
    expected: ServerImportPreview,
    name: String,
    java_path: String,
    memory_mb: u32,
    configuration: Option<ServerImportConfiguration>,
    source_stopped: bool,
) -> Result<ServerManifest> {
    let configured = apply_configuration(expected.clone(), configuration)?;
    if name.trim().is_empty() || java_path.trim().is_empty() || memory_mb < 512
    {
        return Err(input(
            "Choose a name, Java installation and at least 512 MB of memory",
        ));
    }
    let source = PathBuf::from(source_path);
    if source.is_dir() && !source_stopped {
        return Err(input(
            "Confirm that the source server is stopped before importing its files",
        ));
    }
    let probe = source.clone();
    let actual = tokio::task::spawn_blocking(move || {
        inspect_configurable_server_import(&probe)
    })
    .await
    .map_err(|e| input(e.to_string()))?
    .map_err(input)?
    .ok_or_else(|| input("No supported server found"))?;
    if actual != expected {
        return Err(input(
            "Server files changed. Inspect the source again before importing",
        ));
    }
    let state = State::get().await?;
    let servers_dir = state.directories.servers_dir();
    tokio::fs::create_dir_all(&servers_dir)
        .await
        .map_err(|e| input(e.to_string()))?;
    let staging = tempfile::Builder::new()
        .prefix(".server-import-")
        .tempdir_in(&servers_dir)
        .map_err(|e| input(e.to_string()))?;
    let content = staging.path().join("content");
    let copy_dest = content.clone();
    let preview = expected.clone();
    tokio::task::spawn_blocking(move || {
        prepare_source(&source, &copy_dest, &preview)
    })
    .await
    .map_err(|e| input(e.to_string()))?
    .map_err(input)?;
    let expected = configured;
    let uuid = Uuid::new_v4().to_string();
    let id = format!("{}-{}", sanitize_folder_name(name.trim()), uuid);
    let dir = servers_dir.join(&id);
    let mut manifest = ServerManifest {
        external_import: true,
        id,
        name: name.trim().into(),
        server_type: expected.server_type.clone(),
        game_version: expected.game_version.clone(),
        loader_version: expected.loader_version,
        jar_name: expected.jar_name,
        java_path: Some(java_path),
        memory_mb: Some(memory_mb),
        icon_path: None,
        modpack: None,
        install_state: Some(InstallState::Incomplete),
        install_error: None,
        jvm_args: Vec::new(),
        pre_launch_hook: None,
        stop_command: super::manifest::default_stop_command(),
        home_pinned_at: None,
        linked_world: None,
        created_at: chrono::Utc::now(),
        last_started_at: None,
        last_exit_crashed: false,
    };
    for icon in ["icon.png", "server-icon.png"] {
        if content.join(icon).is_file() {
            manifest.icon_path =
                Some(dir.join(icon).to_string_lossy().into_owned());
            break;
        }
    }
    let _operation = acquire_operation(&manifest.id)?;
    // Stage metadata before publishing; the list never sees a copied source manifest or a ready half-install.
    write_manifest(&content, &manifest).await?;
    tokio::fs::rename(&content, &dir)
        .await
        .map_err(|e| input(e.to_string()))?;
    drop(staging);
    if expected.needs_install {
        install::finish_import_install_locked(&manifest.id).await?;
    } else {
        match launch::imported_launch_args(&dir, &manifest) {
            Ok(_) => {
                manifest.install_state = None;
                write_manifest(&dir, &manifest).await?;
            }
            Err(error) => {
                manifest.install_state = Some(InstallState::Failed);
                manifest.install_error = Some(error);
                write_manifest(&dir, &manifest).await?;
            }
        }
    }
    read_manifest(&dir).await
}

pub async fn retry_external_server(id: &str) -> Result<ServerManifest> {
    install::finish_import_install(id).await?;
    let dir = server_path(id).await?;
    read_manifest(&dir).await
}

/// Preserve the classifier's precedence and nested-archive consent at one boundary.
pub(crate) fn classify_drop(
    path: &Path,
    allow_nested_unpack: bool,
) -> Option<crate::api::drop_classifier::DroppedItemType> {
    use crate::api::drop_classifier::DroppedItemType;
    match detect::inspect_server_drop(path, allow_nested_unpack) {
        Ok(Some(preview)) => Some(DroppedItemType::Server {
            file_path: path.to_path_buf(),
            preview,
        }),
        Ok(None) => None,
        Err(reason) => Some(DroppedItemType::Unknown { reason }),
    }
}
