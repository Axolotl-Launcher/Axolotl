use crate::instance::QuickPlayType;
use crate::launcher::{download, parse_rules};
use crc32fast::Hasher;
use daedalus::minecraft::Library;
use fs4::tokio::AsyncFileExt;
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeArchive {
    pub library_name: String,
    pub classifier: String,
    pub archive_path: PathBuf,
    pub sha1: Option<String>,
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug)]
struct NativeEntry {
    relative_path: PathBuf,
    uncompressed_size: u64,
    crc32: u32,
    archive_path: PathBuf,
    archive_entry_index: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativePreparationReport {
    pub verified: usize,
    pub restored: Vec<PathBuf>,
}

pub(crate) fn resolve_native_archives(
    libraries_dir: &Path,
    caches_dir: &Path,
    libraries: &[Library],
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<Vec<NativeArchive>> {
    let mut archives = Vec::new();
    let mut identities = HashSet::new();

    for library in libraries {
        if let Some(rules) = &library.rules
            && !parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        {
            continue;
        }
        if !library.downloadable || !download::is_native_library(library) {
            continue;
        }
        let Some(classifier) =
            download::library_native_classifier(library, java_arch)
        else {
            continue;
        };
        let native = library
            .downloads
            .as_ref()
            .and_then(|downloads| downloads.classifiers.as_ref())
            .and_then(|classifiers| classifiers.get(&classifier));
        let classified_path = libraries_dir.join(
            if let Some(path) = native.and_then(|native| native.path.as_deref())
            {
                path.to_owned()
            } else {
                download::native_library_artifact_path(library, &classifier)?
            },
        );
        let (archive_path, sha1) = if let Some(native) = native {
            let cached = caches_dir
                .join("minecraft-natives")
                .join(format!("{}.jar", native.sha1));
            // Modern native archives are content-addressed by SHA1. Do not
            // silently substitute the Maven artifact: it can be a different
            // classifier/version and produce an ABI-incompatible DLL set.
            if cached.is_file() {
                (cached, Some(native.sha1.clone()))
            } else if classified_path.is_file()
                && (native.sha1.len() != 40
                    || sha1_file(&classified_path)? == native.sha1)
            {
                // Older instances downloaded classifiers directly into
                // libraries/. Reuse that artifact when it is verifiably the
                // requested archive (or when the legacy metadata has no
                // usable SHA-1 to verify), allowing repair/import to recover
                // without a redundant network download.
                (classified_path, None)
            } else {
                (cached, Some(native.sha1.clone()))
            }
        } else {
            (classified_path, None)
        };
        let identity = sha1.clone().unwrap_or_else(|| {
            archive_path
                .canonicalize()
                .unwrap_or_else(|_| archive_path.clone())
                .to_string_lossy()
                .into_owned()
        });
        if !identities.insert(identity) {
            tracing::debug!(
                library = %library.name,
                classifier,
                "Skipped duplicate native archive"
            );
            continue;
        }
        archives.push(NativeArchive {
            library_name: library.name.clone(),
            classifier,
            archive_path,
            sha1,
            exclude: library
                .extract
                .as_ref()
                .and_then(|extract| extract.exclude.clone())
                .unwrap_or_default(),
        });
    }

    Ok(archives)
}

pub(crate) async fn prepare_native_libraries(
    natives_root: &Path,
    libraries_dir: &Path,
    caches_dir: &Path,
    libraries: &[Library],
    version: &str,
    java_arch: &str,
    minecraft_updated: bool,
) -> crate::Result<NativePreparationReport> {
    let archives = resolve_native_archives(
        libraries_dir,
        caches_dir,
        libraries,
        java_arch,
        minecraft_updated,
    )?;
    materialize_native_directory(&archives, natives_root, version).await
}

pub(crate) async fn materialize_native_directory(
    archives: &[NativeArchive],
    natives_root: &Path,
    version: &str,
) -> crate::Result<NativePreparationReport> {
    let natives_dir = natives_root.join(version);
    let lock_dir = natives_root.join(".locks");
    tokio::fs::create_dir_all(&natives_dir).await?;
    tokio::fs::create_dir_all(&lock_dir).await?;
    let lock_path = lock_dir.join(format!("{}.lock", safe_version_id(version)));
    let archives = archives.to_vec();
    let version = version.to_string();
    let lock = tokio::fs::File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .await?;
    let report = tokio::task::spawn_blocking(move || {
        lock.lock_exclusive().map_err(|error| {
            crate::ErrorKind::LauncherError(format!(
                "Failed to lock native directory for Minecraft {version} at {}: {error}",
                lock_path.display()
            ))
        })?;
        materialize_locked(&archives, &natives_dir, &version)
    })
    .await??;
    Ok(report)
}

fn materialize_locked(
    archives: &[NativeArchive],
    natives_dir: &Path,
    version: &str,
) -> crate::Result<NativePreparationReport> {
    let manifest = build_manifest(archives)?;
    let mut report = NativePreparationReport::default();

    for entry in manifest.values() {
        let target = natives_dir.join(&entry.relative_path);
        if file_matches(&target, entry.uncompressed_size, entry.crc32)? {
            report.verified += 1;
            continue;
        }
        materialize_entry(entry, &target, version)?;
        if !file_matches(&target, entry.uncompressed_size, entry.crc32)? {
            return Err(native_error(format!(
                "Native entry {} failed final validation after extraction from {}",
                target.display(),
                entry.archive_path.display()
            )));
        }
        report.restored.push(entry.relative_path.clone());
    }

    tracing::debug!(
        target = %natives_dir.display(),
        archives = archives.len(),
        entries = manifest.len(),
        verified = report.verified,
        restored = report.restored.len(),
        "Prepared Minecraft native directory"
    );
    Ok(report)
}

fn build_manifest(
    archives: &[NativeArchive],
) -> crate::Result<BTreeMap<PathBuf, NativeEntry>> {
    let mut manifest = BTreeMap::new();
    for archive in archives {
        if !archive.archive_path.is_file() {
            return Err(native_error(format!(
                "Native archive for {} ({}) is missing at {}. Repair the instance while online and try again",
                archive.library_name,
                archive.classifier,
                archive.archive_path.display()
            )));
        }
        if let Some(expected_sha1) = archive.sha1.as_deref()
            && expected_sha1.len() == 40
        {
            let actual_sha1 = sha1_file(&archive.archive_path)?;
            if actual_sha1 != expected_sha1 {
                return Err(native_error(format!(
                    "Native archive for {} ({}) at {} has SHA1 {}, expected {}. Repair the instance while online and try again",
                    archive.library_name,
                    archive.classifier,
                    archive.archive_path.display(),
                    actual_sha1,
                    expected_sha1
                )));
            }
        }
        let file = std::fs::File::open(&archive.archive_path)?;
        let mut zip = zip::ZipArchive::new(file).map_err(|error| {
            native_error(format!(
                "Failed to open native archive for {} ({}) at {}: {error}",
                archive.library_name,
                archive.classifier,
                archive.archive_path.display()
            ))
        })?;
        for index in 0..zip.len() {
            let entry = zip.by_index(index).map_err(|error| {
                native_error(format!(
                    "Failed to inspect native archive {}: {error}",
                    archive.archive_path.display()
                ))
            })?;
            if entry.is_dir() {
                continue;
            }
            let entry_name = entry.name().map_err(|error| {
                native_error(format!(
                    "Failed to read native archive entry name {}: {error}",
                    archive.archive_path.display()
                ))
            })?;
            let Some(relative_path) = safe_entry_path(&entry_name) else {
                tracing::warn!(
                    archive = %archive.archive_path.display(),
                    entry = %entry_name,
                    "Ignored unsafe native archive entry"
                );
                continue;
            };
            let normalized = relative_path.to_string_lossy().replace('\\', "/");
            if normalized == "META-INF" || normalized.starts_with("META-INF/") {
                continue;
            }
            if archive.exclude.iter().any(|excluded| {
                let excluded = excluded.replace('\\', "/");
                normalized == excluded.trim_end_matches('/')
                    || normalized.starts_with(&format!(
                        "{}/",
                        excluded.trim_end_matches('/')
                    ))
            }) {
                continue;
            }
            if let Some(previous) = manifest.insert(
                relative_path.clone(),
                NativeEntry {
                    relative_path,
                    uncompressed_size: entry.size(),
                    crc32: entry.crc32(),
                    archive_path: archive.archive_path.clone(),
                    archive_entry_index: index,
                },
            ) && (previous.uncompressed_size != entry.size()
                || previous.crc32 != entry.crc32())
            {
                tracing::debug!(
                    entry = normalized,
                    previous = %previous.archive_path.display(),
                    selected = %archive.archive_path.display(),
                    "Native entry is provided by multiple archives; using metadata order"
                );
            }
        }
    }
    Ok(manifest)
}

fn safe_entry_path(name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains('\\')
        || name.starts_with('/')
        || name.as_bytes().get(1) == Some(&b':')
    {
        return None;
    }
    let path = Path::new(name);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(path.to_path_buf())
}

fn file_matches(path: &Path, size: u64, crc32: u32) -> crate::Result<bool> {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if !metadata.is_file() || metadata.len() != size {
        return Ok(false);
    }
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize() == crc32)
}

fn sha1_file(path: &Path) -> crate::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.digest().to_string())
}

fn materialize_entry(
    entry: &NativeEntry,
    target: &Path,
    version: &str,
) -> crate::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        native_error(format!(
            "Native target {} has no parent",
            target.display()
        ))
    })?;
    std::fs::create_dir_all(parent)?;
    let file = std::fs::File::open(&entry.archive_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        native_error(format!(
            "Failed to open native archive {}: {error}",
            entry.archive_path.display()
        ))
    })?;
    let mut source =
        archive
            .by_index(entry.archive_entry_index)
            .map_err(|error| {
                native_error(format!(
                    "Failed to read native entry {} from {}: {error}",
                    entry.relative_path.display(),
                    entry.archive_path.display()
                ))
            })?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".tmp-native-{}-", safe_version_id(version)))
        .tempfile_in(parent)?;
    let mut hasher = Hasher::new();
    let mut written = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        temporary.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        written += read as u64;
    }
    if written != entry.uncompressed_size || hasher.finalize() != entry.crc32 {
        return Err(native_error(format!(
            "Native entry {} extracted from {} did not match its ZIP size and CRC32",
            entry.relative_path.display(),
            entry.archive_path.display()
        )));
    }
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    if let Ok(metadata) = std::fs::symlink_metadata(target) {
        if metadata.file_type().is_dir() {
            std::fs::remove_dir_all(target)?;
        } else if !metadata.file_type().is_file() {
            std::fs::remove_file(target)?;
        }
    }
    temporary.persist(target).map_err(|error| {
        native_error(format!(
            "Failed to atomically replace native entry {}: {}",
            target.display(),
            error.error
        ))
    })?;
    Ok(())
}

fn safe_version_id(version: &str) -> String {
    let sanitized: String = version
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_')
            {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn native_error(message: String) -> crate::Error {
    crate::ErrorKind::LauncherError(message).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    fn archive(path: PathBuf) -> NativeArchive {
        NativeArchive {
            library_name: "org.lwjgl:lwjgl:3.2.2".to_string(),
            classifier: "natives-windows".to_string(),
            archive_path: path,
            sha1: None,
            exclude: Vec::new(),
        }
    }

    #[tokio::test]
    async fn repairs_missing_truncated_same_size_and_directory_entries() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("natives.jar");
        write_archive(
            &archive_path,
            &[
                ("missing.dll", b"missing"),
                ("bad.dll", b"correct"),
                ("folder.dll", b"file"),
            ],
        );
        let target = root.path().join("target");
        std::fs::create_dir_all(target.join("version/folder.dll")).unwrap();
        std::fs::write(target.join("version/bad.dll"), b"xxxxxxx").unwrap();

        let report = materialize_native_directory(
            &[archive(archive_path)],
            &target,
            "version",
        )
        .await
        .unwrap();

        assert_eq!(report.restored.len(), 3);
        assert_eq!(
            std::fs::read(target.join("version/missing.dll")).unwrap(),
            b"missing"
        );
        assert_eq!(
            std::fs::read(target.join("version/bad.dll")).unwrap(),
            b"correct"
        );
        assert_eq!(
            std::fs::read(target.join("version/folder.dll")).unwrap(),
            b"file"
        );
    }

    #[tokio::test]
    async fn preparation_is_idempotent_and_leaves_no_temporary_files() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("natives.jar");
        write_archive(&archive_path, &[("lwjgl.dll", b"native")]);
        let archives = [archive(archive_path)];
        let first =
            materialize_native_directory(&archives, root.path(), "1.18.2")
                .await
                .unwrap();
        let second =
            materialize_native_directory(&archives, root.path(), "1.18.2")
                .await
                .unwrap();

        assert_eq!(first.restored, [PathBuf::from("lwjgl.dll")]);
        assert_eq!(second.verified, 1);
        assert!(second.restored.is_empty());
        assert!(
            std::fs::read_dir(root.path().join("1.18.2"))
                .unwrap()
                .flatten()
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".tmp-native-"))
        );
    }

    #[test]
    fn modern_native_resolution_does_not_fallback_to_unverified_maven_artifact()
    {
        let root = tempfile::tempdir().unwrap();
        let libraries_dir = root.path().join("libraries");
        let caches_dir = root.path().join("caches");
        let classified = libraries_dir
            .join("org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-windows.jar");
        std::fs::create_dir_all(classified.parent().unwrap()).unwrap();
        write_archive(&classified, &[("lwjgl.dll", b"wrong")]);
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.2.2",
            "natives": {"windows": "natives-windows"},
            "downloads": {"classifiers": {"natives-windows": {
                "sha1": "05359f3aa50d36352815fc662ea73e1c00d22170",
                "size": 279593,
                "url": "https://libraries.minecraft.net/native.jar"
            }}}
        }))
        .unwrap();

        let archives = resolve_native_archives(
            &libraries_dir,
            &caches_dir,
            &[library],
            "x86_64",
            true,
        )
        .unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(
            archives[0].archive_path,
            caches_dir.join(
                "minecraft-natives/05359f3aa50d36352815fc662ea73e1c00d22170.jar"
            )
        );
    }

    #[test]
    fn classifier_native_library_is_selected_for_preparation() {
        let root = tempfile::tempdir().unwrap();
        let library: Library = serde_json::from_value(serde_json::json!({
            "name": "org.lwjgl:lwjgl:3.2.2:natives-windows",
            "downloads": {
                "artifact": {
                    "sha1": "abc",
                    "size": 1,
                    "url": "https://example.com/native.jar"
                }
            }
        }))
        .unwrap();

        let archives = resolve_native_archives(
            &root.path().join("libraries"),
            &root.path().join("caches"),
            &[library],
            "x86_64",
            true,
        )
        .unwrap();

        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].classifier, "natives-windows");
        assert_eq!(
            archives[0].archive_path,
            root.path().join(
                "libraries/org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2-natives-windows.jar"
            )
        );
    }

    #[tokio::test]
    async fn rejects_modern_archives_with_the_wrong_sha1() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("natives.jar");
        write_archive(&archive_path, &[("lwjgl.dll", b"native")]);
        let mut archive = archive(archive_path);
        archive.sha1 =
            Some("0000000000000000000000000000000000000000".to_string());

        let error = materialize_native_directory(
            &[archive],
            root.path(),
            "1.18.2-forge-40.2.34",
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("has SHA1"));
        assert!(!root.path().join("1.18.2-forge-40.2.34/lwjgl.dll").exists());
    }

    #[tokio::test]
    async fn concurrent_preparation_is_serialized() {
        let root = tempfile::tempdir().unwrap();
        let archive_path = root.path().join("natives.jar");
        write_archive(&archive_path, &[("lwjgl.dll", b"native")]);
        let archives = vec![archive(archive_path)];
        let root_path = root.path().to_path_buf();
        let (left, right) = tokio::join!(
            materialize_native_directory(&archives, &root_path, "1.18.2"),
            materialize_native_directory(&archives, &root_path, "1.18.2"),
        );

        let reports = [left.unwrap(), right.unwrap()];
        assert_eq!(
            reports
                .iter()
                .map(|report| report.restored.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(
            reports.iter().map(|report| report.verified).sum::<usize>(),
            1
        );
    }

    #[tokio::test]
    async fn unsafe_meta_inf_and_duplicate_entries_are_deterministic() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first.jar");
        let second = root.path().join("second.jar");
        write_archive(
            &first,
            &[
                ("same.dll", b"first"),
                ("META-INF/MANIFEST.MF", b"meta"),
                ("excluded/skip.dll", b"skip"),
            ],
        );
        write_archive(
            &second,
            &[
                ("../evil.dll", b"evil"),
                ("C:/evil.dll", b"evil"),
                ("same.dll", b"later"),
            ],
        );

        let mut first = archive(first);
        first.exclude = vec!["excluded/".to_string()];
        materialize_native_directory(
            &[first, archive(second)],
            root.path(),
            "version",
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read(root.path().join("version/same.dll")).unwrap(),
            b"later"
        );
        assert!(!root.path().join("evil.dll").exists());
        assert!(!root.path().join("version/META-INF").exists());
        assert!(!root.path().join("version/excluded").exists());
    }
}
