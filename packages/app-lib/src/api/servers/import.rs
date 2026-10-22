//! Copies external server packages into new managed directories without touching sources.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use dashmap::DashSet;
use uuid::Uuid;

use super::detect::{
    ServerImportPreview, inspect_server_import, read_bounded, safe_source_path,
};
use super::manifest::{
    InstallState, ServerManifest, read_manifest, sanitize_folder_name,
    server_path, write_manifest,
};
use crate::{ErrorKind, Result, State};

use crate::api::drop_classifier::{
    MAX_ZIP_EXTRACTION_BYTES as MAX_BYTES,
    MAX_ZIP_EXTRACTION_ENTRIES as MAX_FILES,
};
static INSTALLING: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);
fn input(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).as_error()
}

fn copy_tree(
    source: &Path,
    dest: &Path,
    depth: usize,
    count: &mut usize,
    bytes: &mut u64,
) -> std::result::Result<(), String> {
    if depth > 64 {
        return Err("Folder nesting exceeds the import limit".into());
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
            copy_tree(&entry.path(), &target, depth + 1, count, bytes)?;
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
    let root = if source.is_dir() {
        if expected.inner_base.is_empty() {
            source.clone()
        } else {
            safe_source_path(&source, &expected.inner_base)?
        }
    } else {
        // A strict preflight supplements the shared extractor: fail rather than silently dropping unsafe content.
        let mut archive = zip::ZipArchive::new(
            std::fs::File::open(&source).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let mut paths = std::collections::HashSet::new();
        for index in 0..archive.len() {
            let entry =
                archive.by_index_raw(index).map_err(|e| e.to_string())?;
            let name = crate::api::pack::detect::decode_zip_entry_name(
                entry.name_raw(),
            )
            .replace('\\', "/");
            if name.is_empty() {
                continue;
            }
            super::files::safe_relative_join(
                content,
                name.trim_end_matches('/'),
            )
            .map_err(|_| format!("Unsafe archive path: {name}"))?;
            if !entry.is_dir() {
                let collision_key = if cfg!(any(windows, target_os = "macos")) {
                    name.to_lowercase()
                } else {
                    name.clone()
                };
                if !paths.insert(collision_key) {
                    return Err(format!("Duplicate archive file: {name}"));
                }
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err("Symbolic links cannot be imported".into());
            }
        }
        drop(archive);
        let extracted = content.parent().unwrap().join("extracted");
        std::fs::create_dir(&extracted).map_err(|e| e.to_string())?;
        crate::api::drop_classifier::extract_zip_to_dir(&source, &extracted)?;
        if expected.inner_base.is_empty() {
            extracted
        } else {
            safe_source_path(&extracted, &expected.inner_base)?
        }
    };
    if source.is_dir() {
        copy_tree(&root, content, 0, &mut 0, &mut 0)?;
    } else {
        // Extraction is already private and bounded; move it instead of copying every world twice.
        std::fs::rename(&root, content).map_err(|e| e.to_string())?;
        let source_manifest = content.join("axolotl-server.json");
        if source_manifest.exists() {
            std::fs::remove_file(source_manifest).map_err(|e| e.to_string())?;
        }
    }
    let actual = inspect_server_import(content)?
        .ok_or("The copied package no longer contains a recognized server")?;
    let mut expected = expected.clone();
    expected.inner_base.clear();
    if actual != expected {
        return Err("Server files changed during import. Inspect the source and try again".into());
    }
    Ok(())
}

pub async fn import_external_server(
    source_path: String,
    expected: ServerImportPreview,
    name: String,
    java_path: String,
    memory_mb: u32,
) -> Result<ServerManifest> {
    if expected.problem.is_some() {
        return Err(input(
            "Resolve the detected server package problem before importing",
        ));
    }
    if name.trim().is_empty() || java_path.trim().is_empty() || memory_mb < 512
    {
        return Err(input(
            "Choose a name, Java installation and at least 512 MB of memory",
        ));
    }
    let source = PathBuf::from(source_path);
    let probe = source.clone();
    let actual =
        tokio::task::spawn_blocking(move || inspect_server_import(&probe))
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
    let uuid = Uuid::new_v4().to_string();
    let id = format!("{}-{}", sanitize_folder_name(name.trim()), uuid);
    let dir = servers_dir.join(&id);
    let mut manifest = ServerManifest::new(
        id,
        name.trim(),
        &expected.server_type,
        &expected.game_version,
        expected.loader_version,
        Some(java_path),
        Some(memory_mb),
    );
    manifest.external_import = true;
    manifest.jar_name = expected.jar_name;
    manifest.install_state = Some(InstallState::Incomplete);
    for icon in ["icon.png", "server-icon.png"] {
        if content.join(icon).is_file() {
            manifest.icon_path =
                Some(dir.join(icon).to_string_lossy().into_owned());
            break;
        }
    }
    // Stage metadata before publishing; the list never sees a copied source manifest or a ready half-install.
    write_manifest(&content, &manifest).await?;
    tokio::fs::rename(&content, &dir)
        .await
        .map_err(|e| input(e.to_string()))?;
    drop(staging);
    if expected.needs_install {
        finish_import_install(&manifest.id).await?;
    } else {
        match imported_launch_args(&dir, &manifest) {
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

async fn finish_import_install(id: &str) -> Result<()> {
    if !INSTALLING.insert(id.into()) {
        return Err(input("This server is already being installed"));
    }
    let result = async {
        let dir = server_path(id).await?;
        let mut manifest = read_manifest(&dir).await?;
        manifest.install_state = Some(InstallState::Incomplete);
        manifest.install_error = None;
        write_manifest(&dir, &manifest).await?;
        let result = if manifest.server_type == "forge" {
            if let Some(build) = manifest.loader_version.as_deref() {
                super::forge::install_forge(
                    id,
                    &manifest.game_version,
                    build,
                    manifest.java_path.clone(),
                )
                .await
            } else {
                Err(input("Missing Forge version"))
            }
        } else {
            Ok(())
        };
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

pub async fn retry_external_server(id: &str) -> Result<ServerManifest> {
    let dir = server_path(id).await?;
    let manifest = read_manifest(&dir).await?;
    if !manifest.external_import
        || manifest.install_state.is_none()
        || super::lifecycle::is_running(id)
    {
        return Err(input("This server does not need an import repair"));
    }
    finish_import_install(id).await?;
    read_manifest(&dir).await
}

pub(super) fn imported_launch_args(
    dir: &Path,
    manifest: &ServerManifest,
) -> std::result::Result<Vec<String>, String> {
    if manifest.server_type == "forge" {
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

fn validate_args(dir: &Path, text: &str) -> std::result::Result<(), String> {
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

fn validate_jar_paths(
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
        crate::mod_metadata::manifest::manifest_attribute(&text, "Class-Path")
    {
        for relative in classpath.split_whitespace() {
            safe_source_path(dir, relative).map_err(|_| format!("A launcher dependency is missing or outside the server folder: {relative}"))?;
        }
    }
    for key in ["Launcher-Agent-Class", "Premain-Class"] {
        if crate::mod_metadata::manifest::manifest_attribute(&text, key)
            .is_some()
        {
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

#[cfg(test)]
mod tests {
    use super::super::detect::tests::{forge, vanilla};
    use super::*;
    fn source() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("launch.jar"), vanilla()).unwrap();
        dir
    }
    #[test]
    fn external_server_copy_preserves_world_and_settings_without_source_manifest()
     {
        let src = source();
        let stage = tempfile::tempdir().unwrap();
        std::fs::create_dir(src.path().join("world")).unwrap();
        std::fs::write(src.path().join("world/level.dat"), b"saved-world")
            .unwrap();
        std::fs::write(
            src.path().join("server.properties"),
            b"server-port=25570\nonline-mode=false\n",
        )
        .unwrap();
        std::fs::write(
            src.path().join("eula.txt"),
            b"# accepted by owner\neula=true\n",
        )
        .unwrap();
        std::fs::write(src.path().join("axolotl-server.json"), br#"{"preLaunchHook":"touch /tmp/no","id":"original","javaPath":"/private/java"}"#).unwrap();
        let preview = inspect_server_import(src.path()).unwrap().unwrap();
        let dest = stage.path().join("content");
        prepare_source(src.path(), &dest, &preview).unwrap();
        for file in [
            "world/level.dat",
            "server.properties",
            "eula.txt",
            "launch.jar",
        ] {
            assert_eq!(
                std::fs::read(src.path().join(file)).unwrap(),
                std::fs::read(dest.join(file)).unwrap()
            );
        }
        assert!(!dest.join("axolotl-server.json").exists());
        assert!(src.path().join("axolotl-server.json").exists());
    }
    #[test]
    fn external_server_copy_rejects_recursive_destination() {
        let src = source();
        let stage = tempfile::tempdir_in(src.path()).unwrap();
        let mut preview = inspect_server_import(src.path()).unwrap().unwrap();
        preview.inner_base.clear();
        assert!(
            prepare_source(src.path(), &stage.path().join("content"), &preview)
                .unwrap_err()
                .contains("outside")
        );
        assert!(src.path().join("launch.jar").exists());
    }
    #[test]
    fn external_server_copy_detects_changed_metadata() {
        let src = source();
        let stage = tempfile::tempdir().unwrap();
        let preview = inspect_server_import(src.path()).unwrap().unwrap();
        std::fs::write(src.path().join("launch.jar"), forge()).unwrap();
        assert!(
            prepare_source(src.path(), &stage.path().join("content"), &preview)
                .is_err()
        );
    }
    #[test]
    fn external_server_zip_rejects_traversal_and_links() {
        for link in [false, true] {
            let src = tempfile::tempdir().unwrap();
            let stage = tempfile::tempdir().unwrap();
            let path = src.path().join("server.zip");
            let mut out =
                zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            out.start_file("launch.jar", opts).unwrap();
            out.write_all(&vanilla()).unwrap();
            if link {
                out.add_symlink("world-link", "/private/user-world", opts)
                    .unwrap();
            } else {
                out.start_file("../escape", opts).unwrap();
                out.write_all(b"bad").unwrap();
            }
            out.finish().unwrap();
            let preview = inspect_server_import(&path).unwrap().unwrap();
            assert!(
                prepare_source(&path, &stage.path().join("content"), &preview)
                    .is_err()
            );
            assert!(!stage.path().join("escape").exists());
        }
    }
    #[cfg(unix)]
    #[test]
    fn external_server_folder_links_are_never_followed() {
        let src = source();
        let stage = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("world"), b"private").unwrap();
        std::os::unix::fs::symlink(other.path(), src.path().join("world"))
            .unwrap();
        let preview = inspect_server_import(src.path()).unwrap().unwrap();
        assert!(
            prepare_source(src.path(), &stage.path().join("content"), &preview)
                .is_err()
        );
        assert_eq!(
            std::fs::read(other.path().join("world")).unwrap(),
            b"private"
        );
    }
    #[test]
    fn external_server_launch_rejects_hooks_and_missing_dependencies() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("libraries")).unwrap();
        std::fs::write(root.path().join("libraries/ok.jar"), b"library")
            .unwrap();
        let baseline = "--module-path libraries/ok.jar cpw.mods.bootstraplauncher.BootstrapLauncher --add-opens java.base/java.util.jar=cpw.mods.securejarhandler --launchTarget forge_server";
        assert!(validate_args(root.path(), baseline).is_ok());
        for args in [
            "@/tmp/user.args",
            "-javaagent:evil.jar",
            "-XX:OnError=exec",
            "-Djava.system.class.loader=Evil",
            "--class-path /private/lib.jar",
            "--class-path libraries/missing.jar",
        ] {
            assert!(
                validate_args(root.path(), &format!("{baseline} {args}"))
                    .is_err(),
                "{args}"
            );
        }
    }
    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    #[ignore = "Run explicitly with a fresh temporary THESEUS_CONFIG_DIR"]
    async fn external_server_managed_import_and_repair_are_isolated() {
        // Refuse to initialize State unless the caller explicitly supplies a clean temporary data directory.
        let config = std::env::var_os("THESEUS_CONFIG_DIR")
            .expect("Run this test with a fresh temporary THESEUS_CONFIG_DIR");
        let config = std::fs::canonicalize(config)
            .expect("Create the isolated data directory before testing");
        let system_temp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(
            config.starts_with(system_temp)
                || config.starts_with("/private/tmp")
        );
        assert!(
            ["release", "beta"]
                .iter()
                .all(|channel| !config.join(channel).join("app.db").exists()),
            "This test requires a clean database"
        );
        crate::event::EventState::init().await.unwrap();
        let state = State::init_for_test("red.ghs.axolotl.import-test".into())
            .await
            .unwrap();
        assert!(state.directories.settings_dir.starts_with(&config));
        let src = source();
        std::fs::write(src.path().join("eula.txt"), b"eula=false\n").unwrap();
        std::fs::write(src.path().join("icon.png"), b"fixture-icon").unwrap();
        std::fs::write(
            src.path().join("server.properties"),
            b"server-port=25577\n",
        )
        .unwrap();
        std::fs::write(src.path().join("axolotl-server.json"), br#"{"id":"private","preLaunchHook":"never-run","javaPath":"/private/java"}"#).unwrap();
        let preview = inspect_server_import(src.path()).unwrap().unwrap();
        let imported = import_external_server(
            src.path().to_string_lossy().into_owned(),
            preview.clone(),
            "External".into(),
            "/fixture/java17".into(),
            2048,
        )
        .await
        .unwrap();
        assert!(imported.external_import);
        assert_eq!(imported.install_state, None);
        assert_eq!(imported.pre_launch_hook, None);
        assert_eq!(imported.java_path.as_deref(), Some("/fixture/java17"));
        assert_ne!(imported.id, "private");
        let info = super::super::manage::get(&imported.id).await.unwrap();
        assert!(!info.running);
        assert!(!info.eula_accepted);
        assert_eq!(info.port, Some(25577));
        assert!(
            Path::new(imported.icon_path.as_ref().unwrap())
                .starts_with(&info.path)
        );
        assert!(src.path().join("axolotl-server.json").exists());
        let second = import_external_server(
            src.path().to_string_lossy().into_owned(),
            preview,
            "External".into(),
            "/fixture/java17".into(),
            2048,
        )
        .await
        .unwrap();
        assert_ne!(imported.id, second.id);
        // A missing dependency stays failed and cannot launch; restoring it allows revalidation without replacing the jar.
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        out.start_file("META-INF/MANIFEST.MF", opts).unwrap();
        out.write_all(b"Manifest-Version: 1.0\nMain-Class: net.minecraft.bundler.Main\nClass-Path: libraries/missing.jar\n").unwrap();
        out.start_file("version.json", opts).unwrap();
        out.write_all(br#"{"id":"1.20.1"}"#).unwrap();
        std::fs::write(
            src.path().join("launch.jar"),
            out.finish().unwrap().into_inner(),
        )
        .unwrap();
        let preview = inspect_server_import(src.path()).unwrap().unwrap();
        let failed = import_external_server(
            src.path().to_string_lossy().into_owned(),
            preview,
            "Broken".into(),
            "/fixture/no-java".into(),
            2048,
        )
        .await
        .unwrap();
        assert_eq!(failed.install_state, Some(InstallState::Failed));
        assert!(
            failed
                .install_error
                .as_deref()
                .unwrap()
                .contains("dependency")
        );
        assert!(
            super::super::lifecycle::start(&failed.id, None, None, None)
                .await
                .is_err()
        );
        let dest = server_path(&failed.id).await.unwrap();
        std::fs::create_dir(dest.join("libraries")).unwrap();
        std::fs::write(dest.join("libraries/missing.jar"), b"restored")
            .unwrap();
        let repaired = retry_external_server(&failed.id).await.unwrap();
        assert_eq!(repaired.install_state, None);
        assert_eq!(repaired.install_error, None);
        assert!(!super::super::manage::get(&failed.id).await.unwrap().running);
    }
    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    #[ignore = "Requires network, AXOLOTL_TEST_JAVA and a fresh temporary THESEUS_CONFIG_DIR"]
    async fn external_server_official_forge_installs_without_starting() {
        let config = std::fs::canonicalize(
            std::env::var_os("THESEUS_CONFIG_DIR")
                .expect("Use a new temporary data directory"),
        )
        .unwrap();
        let temporary = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(
            config.starts_with(temporary) || config.starts_with("/private/tmp")
        );
        assert!(
            ["release", "beta"]
                .iter()
                .all(|channel| !config.join(channel).join("app.db").exists())
        );
        let java = std::env::var("AXOLOTL_TEST_JAVA")
            .expect("Select an isolated-test Java 17 executable");
        crate::event::EventState::init().await.unwrap();
        let state = State::init_for_test("red.ghs.axolotl.import-test".into())
            .await
            .unwrap();
        assert!(state.directories.settings_dir.starts_with(&config));
        let source = tempfile::tempdir().unwrap();
        // The source is a metadata-only fixture. Only the official installer downloaded by Axolotl is executed.
        std::fs::write(source.path().join("forge.jar"), forge()).unwrap();
        std::fs::write(
            source.path().join("eula.txt"),
            b"# owner status\neula=false\n",
        )
        .unwrap();
        std::fs::write(source.path().join("start.sh"), b"exit 88\n").unwrap();
        let preview = inspect_server_import(source.path()).unwrap().unwrap();
        let began = std::time::Instant::now();
        let result = import_external_server(
            source.path().to_string_lossy().into_owned(),
            preview,
            "Forge install verification".into(),
            java,
            2048,
        )
        .await
        .unwrap();
        eprintln!("Official Forge import took {:?}", began.elapsed());
        assert_eq!(result.install_state, None, "{:?}", result.install_error);
        assert_eq!(result.game_version, "1.20.1");
        assert_eq!(result.loader_version.as_deref(), Some("47.4.16"));
        let dest = server_path(&result.id).await.unwrap();
        assert!(dest.join("libraries/net/minecraftforge/forge/1.20.1-47.4.16/unix_args.txt").exists());
        assert_eq!(
            std::fs::read(dest.join("eula.txt")).unwrap(),
            b"# owner status\neula=false\n"
        );
        assert!(!dest.join("world").exists());
        assert!(!super::super::manage::get(&result.id).await.unwrap().running);
        let installed_preview = inspect_server_import(&dest).unwrap().unwrap();
        assert!(!installed_preview.needs_install);
        assert_eq!(installed_preview.problem, None);
        let copied = import_external_server(
            dest.to_string_lossy().into_owned(),
            installed_preview,
            "Installed Forge verification".into(),
            result.java_path.clone().unwrap(),
            2048,
        )
        .await
        .unwrap();
        assert_ne!(copied.id, result.id);
        assert_eq!(copied.install_state, None, "{:?}", copied.install_error);
        let copied_dest = server_path(&copied.id).await.unwrap();
        assert!(dest.join("axolotl-server.json").exists());
        assert_eq!(
            std::fs::read(copied_dest.join("eula.txt")).unwrap(),
            b"# owner status\neula=false\n"
        );
        assert!(!copied_dest.join("world").exists());
        assert!(!super::super::manage::get(&copied.id).await.unwrap().running);
    }
}
