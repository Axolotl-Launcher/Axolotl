//! Prepare the unattended Fabric launcher's files with the shared download engine.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::State;
use crate::api::pack::archive_util::read_archive_entry_to_string;
use crate::util::fetch::{
    ContentValidation, DownloadRequest, Integrity, ResourceClass, fetch_json,
};
use crate::util::io::IOError;
use crate::{ErrorKind, Result};

use super::files::{download_server_file, safe_relative_join};
use super::lifecycle::log_server_step;
use super::manifest::{ServerManifest, resolve_jar_name};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerProfile {
    main_class: String,
    libraries: Vec<Library>,
}

#[derive(Deserialize)]
struct Library {
    name: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}

struct FabricRuntime {
    game: String,
    loader: String,
    game_jar: String,
    launch_jar: String,
}

fn runtime(properties: &str) -> Result<FabricRuntime> {
    let property = |name: &str| -> Result<String> {
        let value = properties
            .lines()
            .filter_map(|line| line.split_once('='))
            .find_map(|(key, value)| {
                (key.trim() == name).then_some(value.trim())
            });
        let value = value
            .filter(|value| {
                !value.is_empty()
                    && value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || b"._-+".contains(&byte)
                    })
            })
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "Invalid or missing {name} in Fabric install.properties"
                ))
            })?;
        Ok(value.to_string())
    };
    let game = property("game-version")?;
    let loader = property("fabric-loader-version")?;
    Ok(FabricRuntime {
        game_jar: format!(".fabric/server/{game}-server.jar"),
        launch_jar: format!(
            ".fabric/server/fabric-loader-server-{loader}-minecraft-{game}.jar"
        ),
        game,
        loader,
    })
}

fn manifest_attribute(manifest: &str, key: &str) -> Option<String> {
    let unfolded = manifest.replace("\r\n ", "").replace("\n ", "");
    unfolded
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(key)
                .then(|| value.trim().to_string())
        })
}

fn installed(dir: &Path, runtime: &FabricRuntime) -> bool {
    let check = || -> Option<()> {
        let root = dir.canonicalize().ok()?;
        let game = safe_relative_join(dir, &runtime.game_jar).ok()?;
        zip::ZipArchive::new(std::fs::File::open(game).ok()?).ok()?;
        let launch = safe_relative_join(dir, &runtime.launch_jar).ok()?;
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&launch).ok()?).ok()?;
        let mut manifest = String::new();
        archive
            .by_name("META-INF/MANIFEST.MF")
            .ok()?
            .take(256 * 1024)
            .read_to_string(&mut manifest)
            .ok()?;
        manifest_attribute(&manifest, "Main-Class")?;
        if let Some(classpath) = manifest_attribute(&manifest, "Class-Path") {
            let base = url::Url::from_file_path(&launch).ok()?;
            for entry in classpath.split_whitespace() {
                let path = base
                    .join(entry)
                    .ok()?
                    .to_file_path()
                    .ok()?
                    .canonicalize()
                    .ok()?;
                if !path.starts_with(&root) {
                    return None;
                }
                zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
            }
        } else if archive.len() < 3 {
            return None;
        }
        Some(())
    };
    check().is_some()
}

pub(super) async fn prepare(
    server_id: &str,
    dir: &Path,
    manifest: &ServerManifest,
    cancellation: &CancellationToken,
) -> Result<()> {
    let launcher = safe_relative_join(dir, &resolve_jar_name(manifest))?;
    let properties = read_archive_entry_to_string(
        launcher,
        "install.properties".to_string(),
    )
    .await?;
    let runtime = runtime(&properties)?;
    let ready = {
        let dir = dir.to_path_buf();
        let properties = properties.clone();
        tokio::task::spawn_blocking(move || -> Result<bool> {
            Ok(installed(&dir, &self::runtime(&properties)?))
        })
        .await??
    };
    if ready {
        return Ok(());
    }

    log_server_step(server_id, "Preparing Fabric server files").await;
    let state = State::get().await?;
    let versions =
        crate::api::metadata::get_minecraft_versions_with_cache(None).await?;
    let version = versions
        .versions
        .iter()
        .find(|version| version.id == runtime.game)
        .ok_or_else(|| {
            ErrorKind::InputError(format!(
                "Unknown Minecraft version: {}",
                runtime.game
            ))
        })?;
    let game = crate::launcher::download::download_version_info(
        &state,
        version,
        crate::data::ModLoader::Vanilla,
        None,
        None,
        None,
        None,
    )
    .await?;
    let server = game
        .downloads
        .get(&daedalus::minecraft::DownloadType::Server)
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Minecraft version has no server download".to_string(),
            )
        })?;
    let request =
        DownloadRequest::new(&server.url, ResourceClass::MinecraftLibrary)
            .with_integrity(
                Integrity::sha1(&server.sha1)
                    .with_size(u64::from(server.size))
                    .with_content_validation(ContentValidation::Jar),
            );
    log_server_step(server_id, "Downloading or verifying Minecraft server")
        .await;
    download_server_file(
        server_id,
        &state,
        dir,
        &runtime.game_jar,
        request,
        Some(cancellation),
        Some("Minecraft server"),
    )
    .await?;

    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/server/json",
        runtime.game, runtime.loader
    );
    let profile: ServerProfile = fetch_json(
        reqwest::Method::GET,
        &url,
        None,
        None,
        None,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;
    let mut libraries = Vec::new();
    for library in &profile.libraries {
        let artifact = daedalus::get_path_from_artifact(&library.name)?;
        let relative = format!("libraries/{artifact}");
        let destination = safe_relative_join(dir, &relative)?;
        let url = url::Url::parse(&library.url)?.join(&artifact)?.to_string();
        let integrity = Integrity {
            sha1: library.sha1.clone(),
            size: library.size,
            content: ContentValidation::Jar,
            ..Integrity::default()
        };
        libraries.push((
            library.name.clone(),
            relative,
            destination,
            DownloadRequest::new(url, ResourceClass::Loader)
                .with_integrity(integrity),
        ));
    }
    log_server_step(
        server_id,
        &format!("Preparing {} Fabric libraries", libraries.len()),
    )
    .await;
    stream::iter(libraries.clone().into_iter().map(
        |(name, relative, _, request)| {
            let state = &state;
            async move {
                download_server_file(
                    server_id,
                    state,
                    dir,
                    &relative,
                    request,
                    Some(cancellation),
                    Some(&name),
                )
                .await
            }
        },
    ))
    .buffer_unordered(state.download_concurrency().clamp(1, 8))
    .try_collect::<Vec<_>>()
    .await?;

    let launch = safe_relative_join(dir, &runtime.launch_jar)?;
    let loader = libraries
        .iter()
        .find(|(name, ..)| name.starts_with("net.fabricmc:fabric-loader:"))
        .ok_or_else(|| {
            ErrorKind::InputError(
                "Fabric profile has no loader library".to_string(),
            )
        })?;
    let loader_manifest = read_archive_entry_to_string(
        loader.2.clone(),
        "META-INF/MANIFEST.MF".to_string(),
    )
    .await?;
    let main = manifest_attribute(&loader_manifest, "Main-Class").ok_or_else(
        || ErrorKind::InputError("Fabric loader has no Main-Class".to_string()),
    )?;
    let library_files = libraries
        .into_iter()
        .map(|(_, relative, path, _)| (relative, path))
        .collect();
    let old_loader = runtime
        .loader
        .split('.')
        .take(3)
        .map(|part| part.parse::<u32>().unwrap_or(u32::MAX))
        .collect::<Vec<_>>()
        <= vec![0, 12, 5];
    tokio::task::spawn_blocking(move || {
        write_launch_jar(
            &launch,
            &main,
            &profile.main_class,
            library_files,
            old_loader,
        )
    })
    .await??;
    log_server_step(server_id, "Fabric server files are ready").await;
    Ok(())
}

fn header(output: &mut String, name: &str, value: &str) -> Result<()> {
    if value.contains(['\r', '\n']) {
        return Err(ErrorKind::InputError(
            "Invalid JAR manifest value".to_string(),
        )
        .into());
    }
    let line = format!("{name}: {value}");
    let mut remaining = line.as_str();
    while !remaining.is_empty() {
        let mut length = remaining.len().min(70);
        while !remaining.is_char_boundary(length) {
            length -= 1;
        }
        output.push_str(&remaining[..length]);
        output.push_str("\r\n");
        remaining = &remaining[length..];
        if !remaining.is_empty() {
            output.push(' ');
        }
    }
    Ok(())
}

fn write_launch_jar(
    path: &Path,
    main: &str,
    launch_main: &str,
    libraries: Vec<(String, PathBuf)>,
    shade: bool,
) -> Result<()> {
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent)
        .map_err(|error| IOError::with_path(error, parent))?;
    let mut manifest = String::new();
    header(&mut manifest, "Manifest-Version", "1.0")?;
    header(&mut manifest, "Main-Class", main)?;
    if !shade {
        let classpath = libraries
            .iter()
            .map(|(relative, _)| {
                let encoded =
                    url::Url::parse("file:///").unwrap().join(relative)?;
                Ok(format!("../../{}", encoded.path().trim_start_matches('/')))
            })
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        header(&mut manifest, "Class-Path", &classpath)?;
    }
    manifest.push_str("\r\n");
    if launch_main.contains(['\r', '\n']) {
        return Err(ErrorKind::InputError(
            "Invalid Fabric main class".to_string(),
        )
        .into());
    }
    // Build beside the target and publish only a complete archive, so cancellation
    // and failed downloads cannot leave a launcher that looks installed.
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| IOError::with_path(error, parent))?;
    {
        let mut archive = zip::ZipWriter::new(temporary.as_file_mut());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        archive
            .start_file("META-INF/MANIFEST.MF", options)
            .map_err(archive_error)?;
        archive
            .write_all(manifest.as_bytes())
            .map_err(|error| IOError::with_path(error, path))?;
        archive
            .start_file("fabric-server-launch.properties", options)
            .map_err(archive_error)?;
        archive
            .write_all(format!("launch.mainClass={launch_main}\n").as_bytes())
            .map_err(|error| IOError::with_path(error, path))?;
        if shade {
            let mut added = HashSet::from([
                "META-INF/MANIFEST.MF".to_string(),
                "fabric-server-launch.properties".to_string(),
            ]);
            let mut services: BTreeMap<String, BTreeSet<String>> =
                BTreeMap::new();
            for (_, library) in libraries {
                let mut source = zip::ZipArchive::new(
                    std::fs::File::open(&library)
                        .map_err(|error| IOError::with_path(error, &library))?,
                )
                .map_err(archive_error)?;
                for index in 0..source.len() {
                    let mut entry =
                        source.by_index(index).map_err(archive_error)?;
                    let name = entry.name().to_string();
                    if entry.is_dir() {
                        continue;
                    }
                    if name.starts_with("META-INF/services/") {
                        let mut text = String::new();
                        entry.read_to_string(&mut text).map_err(|error| {
                            IOError::with_path(error, &library)
                        })?;
                        services.entry(name).or_default().extend(
                            text.lines().filter_map(|line| {
                                let line =
                                    line.split('#').next().unwrap_or("").trim();
                                (!line.is_empty()).then(|| line.to_string())
                            }),
                        );
                    } else if !(name.starts_with("META-INF/")
                        && [".SF", ".DSA", ".RSA", ".EC"]
                            .iter()
                            .any(|suffix| name.ends_with(suffix)))
                        && added.insert(name.clone())
                    {
                        archive
                            .start_file(name, options)
                            .map_err(archive_error)?;
                        std::io::copy(&mut entry, &mut archive).map_err(
                            |error| IOError::with_path(error, &library),
                        )?;
                    }
                }
            }
            for (name, providers) in services {
                archive.start_file(name, options).map_err(archive_error)?;
                archive
                    .write_all(
                        providers
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join("\n")
                            .as_bytes(),
                    )
                    .map_err(|error| IOError::with_path(error, path))?;
            }
        }
        archive.finish().map_err(archive_error)?;
    }
    temporary
        .persist(path)
        .map_err(|error| IOError::with_path(error.error, path))?;
    Ok(())
}

fn archive_error(error: zip::result::ZipError) -> ErrorKind {
    ErrorKind::OtherError(format!("Failed to prepare Fabric launcher: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(path: &Path, entries: &[(&str, &str)]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut jar = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, content) in entries {
            jar.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            jar.write_all(content.as_bytes()).unwrap();
        }
        jar.finish().unwrap();
    }

    #[test]
    fn uses_the_versions_embedded_in_the_downloaded_launcher_and_real_cache_paths()
     {
        let runtime = runtime("# unattended launcher\ngame-version=1.21.1\nfabric-loader-version=0.16.14\n").unwrap();
        assert_eq!(runtime.game_jar, ".fabric/server/1.21.1-server.jar");
        assert_eq!(
            runtime.launch_jar,
            ".fabric/server/fabric-loader-server-0.16.14-minecraft-1.21.1.jar"
        );
        assert!(
            self::runtime(
                "game-version=../../secret\nfabric-loader-version=0.16.14"
            )
            .is_err()
        );
        assert!(self::runtime("game-version=1.21.1").is_err());
    }

    #[test]
    fn folds_manifest_headers_without_breaking_unicode_or_classpath_values() {
        let classpath = "../../libraries/中文.jar ".repeat(20);
        let mut manifest = String::new();
        header(&mut manifest, "Manifest-Version", "1.0").unwrap();
        header(&mut manifest, "Class-Path", &classpath).unwrap();
        manifest.push_str("\r\n");
        assert!(manifest.lines().all(|line| line.len() <= 72));
        assert_eq!(
            manifest_attribute(&manifest, "Class-Path"),
            Some(classpath.trim().to_string())
        );
        assert!(header(&mut manifest, "Main-Class", "foo\r\nBad: x").is_err());
    }

    #[test]
    fn reuses_a_complete_install_and_detects_missing_or_corrupt_dependencies() {
        let root = tempfile::tempdir().unwrap();
        let runtime =
            runtime("game-version=1.21.1\nfabric-loader-version=0.16.14")
                .unwrap();
        let library =
            root.path().join("libraries/net/fabricmc/test/1/test-1.jar");
        jar(
            &root.path().join(&runtime.game_jar),
            &[("version.json", "{}")],
        );
        jar(&library, &[("example/Server.class", "test")]);
        let launch = root.path().join(&runtime.launch_jar);
        let libraries = vec![(
            "libraries/net/fabricmc/test/1/test-1.jar".to_string(),
            library.clone(),
        )];
        write_launch_jar(
            &launch,
            "loader.Server",
            "loader.KnotServer",
            libraries.clone(),
            false,
        )
        .unwrap();
        assert!(installed(root.path(), &runtime));
        // Rebuilding an existing launch jar must also work on Windows.
        write_launch_jar(
            &launch,
            "loader.Server",
            "loader.KnotServer",
            libraries,
            false,
        )
        .unwrap();
        assert!(installed(root.path(), &runtime));
        std::fs::write(&library, "interrupted download").unwrap();
        assert!(!installed(root.path(), &runtime));
        std::fs::remove_file(&library).unwrap();
        assert!(!installed(root.path(), &runtime));
    }

    #[test]
    fn supports_legacy_shaded_loaders_merges_services_and_omits_invalid_signatures()
     {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("one.jar");
        let second = root.path().join("two.jar");
        jar(
            &first,
            &[
                ("META-INF/MANIFEST.MF", "Main-Class: old.Server\r\n\r\n"),
                ("META-INF/services/service.Type", "provider.One\n"),
                ("META-INF/SIGNATURE.SF", "signature"),
                ("old/Server.class", "class"),
            ],
        );
        jar(
            &second,
            &[
                (
                    "META-INF/services/service.Type",
                    "provider.Two\nprovider.One # duplicated\n",
                ),
                ("other/Type.class", "class"),
            ],
        );
        let path = root.path().join("launch.jar");
        write_launch_jar(
            &path,
            "old.Server",
            "old.KnotServer",
            vec![
                ("one.jar".to_string(), first),
                ("two.jar".to_string(), second),
            ],
            true,
        )
        .unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        assert!(zip.by_name("old/Server.class").is_ok());
        assert!(zip.by_name("other/Type.class").is_ok());
        assert!(zip.by_name("META-INF/SIGNATURE.SF").is_err());
        let mut providers = String::new();
        zip.by_name("META-INF/services/service.Type")
            .unwrap()
            .read_to_string(&mut providers)
            .unwrap();
        assert_eq!(providers, "provider.One\nprovider.Two");
    }
}
