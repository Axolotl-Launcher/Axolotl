use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use daedalus::minecraft::{Library, LibraryDownload};
use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

use super::{MultiMcProfile, invalid, normalize_download};
use crate::instance::QuickPlayType;
use crate::state::State;
use crate::util::fetch::{
    self, ContentValidation, DownloadRequest, Integrity, ResourceClass,
};
use crate::util::io;

/// JAR mods are merge inputs, never classpath libraries in their own right.
pub(super) struct Artifact {
    library: Library,
    local: Option<PathBuf>,
    download: Option<LibraryDownload>,
    cache_path: PathBuf,
    always_stale: bool,
}

impl Artifact {
    pub(super) fn parse(
        mut raw: Value,
        local_directory: &str,
    ) -> crate::Result<Self> {
        if !raw.is_object() {
            return Err(invalid("Invalid MultiMC JAR artifact"));
        }
        if let Some(artifact) = raw.pointer_mut("/downloads/artifact") {
            normalize_download(artifact);
        }
        let library: Library = serde_json::from_value(raw.clone())?;
        let relative = daedalus::get_path_from_artifact(&library.name)?;
        io::join_within_root(Path::new("libraries"), &relative)?;
        if library.natives.is_some() {
            return Err(invalid(
                "MultiMC JAR artifacts cannot be native libraries",
            ));
        }
        let filename = raw
            .get("MMC-filename")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| invalid("Invalid MultiMC JAR filename"))
            })
            .transpose()?
            .unwrap_or_else(|| relative.rsplit('/').next().unwrap_or_default());
        // MultiMC's local override directories use flat filenames.
        if filename.contains(['/', '\\']) {
            return Err(invalid("Invalid MultiMC JAR filename"));
        }
        let local_path =
            io::join_within_root(Path::new(local_directory), filename)?;
        let local = (raw.get("MMC-hint").and_then(Value::as_str)
            == Some("local"))
        .then_some(local_path);
        let mut download =
            library.downloads.as_ref().and_then(|d| d.artifact.clone());
        if local.is_none() && download.as_ref().is_none_or(|d| d.url.is_empty())
        {
            let absolute_url = raw
                .get("MMC-absoluteUrl")
                .or_else(|| raw.get("MMC-absulute_url"))
                .and_then(Value::as_str);
            let repository = library
                .url
                .as_deref()
                .unwrap_or("https://libraries.minecraft.net");
            let mut storage = PathBuf::from(&relative);
            storage.set_file_name(filename);
            let fallback = LibraryDownload {
                path: None,
                sha1: String::new(),
                size: 0,
                url: absolute_url.map(str::to_string).unwrap_or_else(|| {
                    format!(
                        "{}/{}",
                        repository.trim_end_matches('/'),
                        storage.to_string_lossy().replace('\\', "/")
                    )
                }),
            };
            if let Some(download) = download.as_mut() {
                download.url = fallback.url;
            } else {
                download = Some(fallback);
            }
        }
        // A coordinate may name different payloads in different packs. Keep
        // remote artifacts separate from the normal shared Maven namespace.
        let digest =
            sha1_smol::Sha1::from(serde_json::to_vec(&raw)?).hexdigest();
        Ok(Self {
            library,
            local,
            download,
            cache_path: PathBuf::from(format!(
                "com/axolotl/multimc/artifacts/{digest}.jar"
            )),
            always_stale: raw.get("MMC-hint").and_then(Value::as_str)
                == Some("always-stale"),
        })
    }

    fn applies(&self, java_arch: &str, minecraft_updated: bool) -> bool {
        self.library.rules.as_ref().is_none_or(|rules| {
            crate::launcher::parse_rules(
                rules,
                java_arch,
                &QuickPlayType::None,
                minecraft_updated,
            )
        })
    }

    fn path(&self, root: &Path, libraries: &Path) -> crate::Result<PathBuf> {
        if let Some(relative) = &self.local {
            let path = root.join(relative);
            let canonical_root = io::canonicalize(root)?;
            let canonical_path = io::canonicalize(&path).map_err(|error| {
                invalid(format!(
                    "Missing MultiMC JAR artifact {}: {error}",
                    path.display()
                ))
            })?;
            if !canonical_path.starts_with(&canonical_root) {
                return Err(invalid(format!(
                    "MultiMC JAR artifact escapes instance: {}",
                    path.display()
                )));
            }
            Ok(canonical_path)
        } else {
            Ok(libraries.join(&self.cache_path))
        }
    }

    async fn resolve(
        &self,
        root: &Path,
        state: &State,
        offline: bool,
    ) -> crate::Result<PathBuf> {
        let path = self.path(root, &state.directories.libraries_dir())?;
        if self.local.is_none() {
            let download = self
                .download
                .as_ref()
                .ok_or_else(|| invalid("Missing MultiMC JAR download"))?;
            if (!offline && self.always_stale)
                || !artifact_is_current(&path, download).await?
            {
                if offline {
                    return Err(invalid(format!(
                        "MultiMC JAR artifact is missing or corrupt in offline mode: {}",
                        path.display()
                    )));
                }
                let request = DownloadRequest::new(
                    &download.url,
                    ResourceClass::MinecraftLibrary,
                )
                .with_integrity(Integrity {
                    sha1: (!download.sha1.is_empty())
                        .then(|| download.sha1.clone()),
                    size: (download.size > 0).then_some(download.size as u64),
                    content: ContentValidation::Jar,
                    ..Integrity::default()
                });
                // A fresh destination bypasses completed-download reuse for
                // MultiMC's explicitly mutable "always-stale" artifacts.
                let destination = if self.always_stale {
                    path.with_extension(format!(
                        "{}.refresh",
                        uuid::Uuid::new_v4()
                    ))
                } else {
                    path.clone()
                };
                let result = fetch::download_to_path(
                    request,
                    &destination,
                    &state.download_semaphore,
                    &state.pool,
                    None,
                )
                .await;
                if result.is_err() && self.always_stale {
                    let _ = tokio::fs::remove_file(&destination).await;
                }
                result?;
                if self.always_stale {
                    if let Err(error) =
                        io::atomically_replace_file(&destination, &path)
                    {
                        let _ = tokio::fs::remove_file(&destination).await;
                        return Err(error.into());
                    }
                }
            }
        }
        Ok(path)
    }
}

pub(super) fn parse_jar_mods(patch: &Value) -> crate::Result<Vec<Artifact>> {
    let (key, legacy) = if patch.get("jarMods").is_some() {
        ("jarMods", false)
    } else {
        ("+jarMods", true)
    };
    let Some(raw) = patch.get(key) else {
        return Ok(Vec::new());
    };
    raw.as_array().ok_or_else(|| invalid(format!("Invalid MultiMC {key} list")))?
        .iter().map(|raw| {
            let raw = if legacy {
                let filename = raw.get("name").and_then(Value::as_str)
                    .ok_or_else(|| invalid("Legacy MultiMC JAR mod is missing its filename"))?;
                json!({"name":"org.multimc.jarmods:legacy:1", "MMC-hint":"local", "MMC-filename":filename})
            } else { raw.clone() };
            Artifact::parse(raw, "jarmods")
        }).collect()
}

async fn artifact_is_current(
    path: &Path,
    download: &LibraryDownload,
) -> crate::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let path = path.to_path_buf();
    let sha1 = download.sha1.clone();
    let size = download.size;
    tokio::task::spawn_blocking(move || -> crate::Result<bool> {
        let mut file = std::fs::File::open(&path)?;
        if size > 0 && file.metadata()?.len() != size as u64 {
            return Ok(false);
        }
        if !sha1.is_empty() {
            let mut hash = sha1_smol::Sha1::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if !hash.hexdigest().eq_ignore_ascii_case(&sha1) {
                return Ok(false);
            }
        }
        Ok(zip::ZipArchive::new(file).is_ok())
    })
    .await?
}

pub(crate) struct PreparedRuntime {
    main_jar: Option<PathBuf>,
    jar_mods: Vec<PathBuf>,
}

impl MultiMcProfile {
    pub(crate) async fn prepare_runtime(
        &self,
        state: &State,
        base_jar: &Path,
        java_arch: &str,
        minecraft_updated: bool,
        offline: bool,
    ) -> crate::Result<PreparedRuntime> {
        let mut jar_mods = Vec::new();
        for artifact in &self.jar_mods {
            if artifact.applies(java_arch, minecraft_updated) {
                jar_mods
                    .push(artifact.resolve(&self.root, state, offline).await?);
            }
        }
        let main_jar = if let Some(artifact) = &self.main_jar
            && artifact.applies(java_arch, minecraft_updated)
        {
            let metadata =
                artifact.download.clone().unwrap_or(LibraryDownload {
                    path: None,
                    sha1: String::new(),
                    size: 0,
                    url: String::new(),
                });
            let path = if artifact.local.is_none()
                && !artifact.always_stale
                && !metadata.sha1.is_empty()
                && artifact_is_current(base_jar, &metadata).await?
            {
                // The install pipeline may already have downloaded this exact
                // mainJar as the profile's client. Reuse it, including offline.
                base_jar.to_path_buf()
            } else {
                artifact.resolve(&self.root, state, offline).await?
            };
            if !artifact_is_current(&path, &metadata).await? {
                return Err(invalid(format!(
                    "Invalid MultiMC main JAR: {}",
                    path.display()
                )));
            }
            Some(path)
        } else {
            None
        };
        Ok(PreparedRuntime { main_jar, jar_mods })
    }
}

impl PreparedRuntime {
    pub(crate) async fn assemble(
        &self,
        base: &Path,
        game_dir: &Path,
    ) -> crate::Result<PathBuf> {
        let base = self.main_jar.as_deref().unwrap_or(base);
        if self.jar_mods.is_empty() {
            return Ok(base.to_path_buf());
        }
        let base = base.to_path_buf();
        let mods = self.jar_mods.clone();
        let output = game_dir.join(".axolotl/multimc-minecraft.jar");
        let target = output.clone();
        let lock = fetch::destination_download_lock(&output);
        let _guard = lock.lock().await;
        tokio::task::spawn_blocking(move || {
            assemble_archive(&base, &mods, &target)
        })
        .await??;
        Ok(output)
    }
}

fn assemble_archive(
    base: &Path,
    mods: &[PathBuf],
    output: &Path,
) -> crate::Result<()> {
    std::fs::create_dir_all(
        output
            .parent()
            .ok_or_else(|| invalid("Invalid MultiMC output path"))?,
    )?;
    let temporary =
        output.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> crate::Result<()> {
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(&temporary)?);
        let mut written = HashSet::new();
        // MultiMC walks the final mod list backwards, then fills missing
        // entries from the original client. Later mods therefore win.
        for input in mods.iter().rev() {
            merge_input(input, false, &mut writer, &mut written)?;
        }
        merge_input(base, true, &mut writer, &mut written)?;
        writer
            .finish()
            .map_err(|error| {
                invalid(format!("Cannot finalize MultiMC JAR: {error}"))
            })?
            .sync_all()?;
        io::atomically_replace_file(&temporary, output)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn entry_name(name: &str) -> crate::Result<String> {
    let normalized = name.replace('\\', "/");
    let path = io::join_within_root(
        Path::new("archive"),
        normalized.trim_end_matches('/'),
    )?;
    Ok(path
        .strip_prefix("archive")?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn merge_input(
    input: &Path,
    base: bool,
    writer: &mut zip::ZipWriter<std::fs::File>,
    written: &mut HashSet<String>,
) -> crate::Result<()> {
    if !base && input.is_dir() {
        return merge_directory(
            input.parent().unwrap_or(input),
            input,
            writer,
            written,
        );
    }
    if !base
        && !input.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("jar") || ext.eq_ignore_ascii_case("zip")
        })
    {
        let name = entry_name(
            &input.file_name().unwrap_or_default().to_string_lossy(),
        )?;
        return write_entry(
            name,
            &mut std::fs::File::open(input)?,
            writer,
            written,
        );
    }
    let mut archive = zip::ZipArchive::new(std::fs::File::open(input)?)
        .map_err(|error| {
            invalid(format!(
                "Cannot open MultiMC JAR {}: {error}",
                input.display()
            ))
        })?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            invalid(format!("Cannot read MultiMC JAR entry: {error}"))
        })?;
        let name = entry_name(entry.name())?;
        // Match MultiMC: filter META-INF from the base, retaining metadata
        // supplied by a mod (for example service provider registrations).
        if entry.is_dir() || (base && name.contains("META-INF")) {
            continue;
        }
        write_entry(name, &mut entry, writer, written)?;
    }
    Ok(())
}

fn merge_directory(
    root: &Path,
    directory: &Path,
    writer: &mut zip::ZipWriter<std::fs::File>,
    written: &mut HashSet<String>,
) -> crate::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if io::is_symlink_or_reparse(&metadata) {
            return Err(invalid(format!(
                "MultiMC JAR mod contains a symlink: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            merge_directory(root, &path, writer, written)?;
        } else if metadata.is_file() {
            let name = entry_name(&path.strip_prefix(root)?.to_string_lossy())?;
            write_entry(
                name,
                &mut std::fs::File::open(&path)?,
                writer,
                written,
            )?;
        }
    }
    Ok(())
}

fn write_entry(
    name: String,
    reader: &mut impl Read,
    writer: &mut zip::ZipWriter<std::fs::File>,
    written: &mut HashSet<String>,
) -> crate::Result<()> {
    if written.insert(name.clone()) {
        writer
            .start_file(
                name,
                SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|error| {
                invalid(format!("Cannot write MultiMC JAR entry: {error}"))
            })?;
        std::io::copy(reader, writer)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DirectoryInfo, test_state};
    use std::io::Write;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn write_jar(path: &Path, entries: &[(&str, &[u8])]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn read_entry(path: &Path, name: &str) -> Vec<u8> {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    async fn state() -> (tempfile::TempDir, Arc<State>) {
        let root = tempfile::tempdir().unwrap();
        let directories = DirectoryInfo {
            settings_dir: root.path().join("settings"),
            config_dir: root.path().join("config"),
            app_identifier: "test".to_string(),
        };
        std::fs::create_dir_all(directories.instances_dir()).unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        (root, test_state(directories, pool).await.unwrap())
    }

    #[test]
    fn modern_and_legacy_formats_use_distinct_filename_semantics() {
        let modern = parse_jar_mods(&json!({"jarMods":[{
            "name":"example:patch:1", "MMC-hint":"local", "MMC-filename":"custom.jar"
        }], "+jarMods":[{"name":"ignored.jar"}]})).unwrap();
        assert_eq!(modern.len(), 1);
        assert_eq!(
            modern[0].local.as_deref(),
            Some(Path::new("jarmods/custom.jar"))
        );
        let legacy = parse_jar_mods(
            &json!({"+jarMods":[{"name":"bta.zip", "originalName":"BTA"}]}),
        )
        .unwrap();
        assert_eq!(
            legacy[0].local.as_deref(),
            Some(Path::new("jarmods/bta.zip"))
        );
        for patch in [
            json!({"jarMods":{}}),
            json!({"+jarMods":true}),
            json!({"+jarMods":[{}]}),
            json!({"jarMods":[null]}),
        ] {
            assert!(parse_jar_mods(&patch).is_err());
        }
        for filename in [
            "../outside.jar",
            "C:/outside.jar",
            "C:\\outside.jar",
            "\\\\server\\share.jar",
            "a/b.jar",
            "bad:stream",
            "bad\0.jar",
        ] {
            assert!(
                parse_jar_mods(&json!({"+jarMods":[{"name":filename}]}))
                    .is_err(),
                "{filename:?}"
            );
        }
    }

    #[test]
    fn merge_order_matches_multimc_and_preserves_mod_metadata() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().join("base.jar");
        let first = root.path().join("first.jar");
        let last = root.path().join("last.jar");
        let output = root.path().join("assembled.jar");
        write_jar(
            &base,
            &[
                ("overlap.class", b"base"),
                ("original.txt", b"original"),
                ("META-INF/MANIFEST.MF", b"signed"),
                ("META-INF/OLD.SF", b"signature"),
            ],
        );
        write_jar(
            &first,
            &[("overlap.class", b"first"), ("first.txt", b"first")],
        );
        write_jar(
            &last,
            &[
                ("overlap.class", b"last"),
                ("META-INF/services/example.Service", b"provider"),
            ],
        );
        let original = std::fs::read(&base).unwrap();
        assemble_archive(&base, &[first.clone(), last.clone()], &output)
            .unwrap();
        assert_eq!(read_entry(&output, "overlap.class"), b"last");
        assert_eq!(read_entry(&output, "original.txt"), b"original");
        assert_eq!(read_entry(&output, "first.txt"), b"first");
        assert_eq!(
            read_entry(&output, "META-INF/services/example.Service"),
            b"provider"
        );
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&output).unwrap())
                .unwrap();
        assert!(archive.by_name("META-INF/OLD.SF").is_err());
        assert!(archive.by_name("META-INF/MANIFEST.MF").is_err());
        drop(archive);
        write_jar(&last, &[("overlap.class", b"updated")]);
        assemble_archive(&base, &[first, last], &output).unwrap();
        assert_eq!(read_entry(&output, "overlap.class"), b"updated");
        assert_eq!(std::fs::read(base).unwrap(), original);
    }

    #[test]
    fn failed_assembly_preserves_previous_output_and_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().join("base.jar");
        let bad = root.path().join("bad.jar");
        let output = root.path().join("assembled.jar");
        write_jar(&base, &[("value", b"base")]);
        std::fs::write(&output, b"previous output").unwrap();
        for name in [
            "../escape.class",
            "/absolute.class",
            "C:/absolute.class",
            "..\\escape.class",
        ] {
            write_jar(&bad, &[(name, b"bad")]);
            assert!(
                assemble_archive(&base, std::slice::from_ref(&bad), &output)
                    .is_err()
            );
            assert_eq!(std::fs::read(&output).unwrap(), b"previous output");
        }
        std::fs::write(&bad, b"corrupt jar").unwrap();
        assert!(assemble_archive(&base, &[bad], &output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"previous output");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
    }

    #[test]
    fn directory_and_single_file_mods_follow_the_same_precedence() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().join("base.jar");
        let directory = root.path().join("directory");
        let single = root.path().join("value.txt");
        let output = root.path().join("assembled.jar");
        write_jar(&base, &[("value.txt", b"base")]);
        std::fs::create_dir_all(directory.join("nested")).unwrap();
        std::fs::write(directory.join("nested/resource.txt"), b"resource")
            .unwrap();
        std::fs::write(directory.join("value.txt"), b"directory").unwrap();
        std::fs::write(&single, b"single").unwrap();
        assemble_archive(&base, &[directory, single], &output).unwrap();
        assert_eq!(read_entry(&output, "value.txt"), b"single");
        assert_eq!(
            read_entry(&output, "directory/nested/resource.txt"),
            b"resource"
        );
    }

    #[tokio::test]
    async fn main_jar_without_integrity_preserves_the_standard_client_download()
    {
        let root = tempfile::tempdir().unwrap();
        super::super::tests::write_profile(
            root.path(),
            json!([{"uid":"net.minecraft", "version":"1.7.10"}]),
            &[(
                "net.minecraft",
                json!({"mainJar":{"name":"example:client:1", "downloads":{"artifact":{"url":"https://example.com/client.jar"}}}}),
            )],
        );
        let profile = MultiMcProfile::from_root(root.path().to_path_buf())
            .await
            .unwrap();
        let mut info = super::super::tests::baseline();
        profile.apply(&mut info, None, root.path()).await.unwrap();
        assert!(
            info.downloads.is_empty(),
            "incomplete metadata must not become a zero-byte client download"
        );
        assert_eq!(
            profile
                .main_jar
                .as_ref()
                .unwrap()
                .download
                .as_ref()
                .unwrap()
                .url,
            "https://example.com/client.jar"
        );
    }

    #[tokio::test]
    async fn local_profile_uses_main_jar_and_ordered_mods_offline() {
        let (_state_root, state) = state().await;
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("minecraft");
        std::fs::create_dir_all(&game).unwrap();
        let foreign_os = if cfg!(windows) { "linux" } else { "windows" };
        super::super::tests::write_profile(
            root.path(),
            json!([
                {"uid":"net.minecraft", "version":"1.7.10"},
                {"uid":"first"}, {"uid":"last"}, {"uid":"disabled", "disabled":true}
            ]),
            &[
                (
                    "net.minecraft",
                    json!({"+traits":["legacyLaunch", "noapplet"], "compatibleJavaMajors":[17]}),
                ),
                (
                    "first",
                    json!({"mainJar":{"name":"example:client:1", "MMC-hint":"local"}, "jarMods":[
                        {"name":"example:first:1", "MMC-hint":"local", "MMC-filename":"first.jar"},
                        {"name":"example:missing:1", "MMC-hint":"local", "rules":[{"action":"allow", "os":{"name":foreign_os}}]}
                    ]}),
                ),
                ("last", json!({"+jarMods":[{"name":"last.zip"}]})),
                ("disabled", json!({"+jarMods":[{"name":"missing.jar"}]})),
            ],
        );
        write_jar(
            &root.path().join("libraries/client-1.jar"),
            &[("value", b"custom main"), ("client-only", b"custom")],
        );
        write_jar(
            &root.path().join("jarmods/first.jar"),
            &[("value", b"first")],
        );
        write_jar(&root.path().join("jarmods/last.zip"), &[("value", b"last")]);
        let profile = MultiMcProfile::load(
            &game,
            "1.7.10",
            crate::state::ModLoader::Vanilla,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        let mut info = super::super::tests::baseline();
        info.main_class = "net.minecraft.launchwrapper.Launch".to_string();
        profile
            .apply(&mut info, None, &state.directories.libraries_dir())
            .await
            .unwrap();
        assert_eq!(
            info.libraries.len(),
            1,
            "JAR mods must not be added to classpath"
        );
        let arguments = serde_json::to_string(&info.arguments).unwrap();
        assert_eq!(info.main_class, "net.minecraft.client.Minecraft");
        assert_eq!(info.java_version.as_ref().unwrap().major_version, 17);
        assert!(
            arguments.contains("-Dfml.ignoreInvalidMinecraftCertificates=true")
        );
        let runtime = profile
            .prepare_runtime(
                &state,
                &game.join("unused.jar"),
                std::env::consts::ARCH,
                false,
                true,
            )
            .await
            .unwrap();
        assert_eq!(runtime.jar_mods.len(), 2);
        let output = runtime
            .assemble(&game.join("unused.jar"), &game)
            .await
            .unwrap();
        assert_eq!(read_entry(&output, "value"), b"last");
        assert_eq!(read_entry(&output, "client-only"), b"custom");
        std::fs::remove_file(root.path().join("jarmods/last.zip")).unwrap();
        assert!(
            profile
                .prepare_runtime(
                    &state,
                    &game.join("unused.jar"),
                    std::env::consts::ARCH,
                    false,
                    true
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn remote_download_repairs_corruption_and_offline_uses_only_cache() {
        let (_root, state) = state().await;
        let root = tempfile::tempdir().unwrap();
        let payload = root.path().join("payload.jar");
        write_jar(&payload, &[("value", b"downloaded")]);
        let bytes = std::fs::read(payload).unwrap();
        let hash = sha1_smol::Sha1::from(&bytes).hexdigest();
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url =
            format!("http://{}/patch.jar", listener.local_addr().unwrap());
        let server_bytes = bytes.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let bytes = server_bytes.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request
                        .windows(4)
                        .any(|window| window == b"\r\n\r\n")
                    {
                        let count = stream.read(&mut buffer).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..count]);
                    }
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).as_bytes()).await.unwrap();
                    stream.write_all(&bytes).await.unwrap();
                });
            }
        });
        let artifact = Artifact::parse(json!({"name":"example:patch:1", "downloads":{"artifact":{"url":url, "sha1":hash, "size":bytes.len()}}}), "jarmods").unwrap();
        assert!(artifact.resolve(root.path(), &state, true).await.is_err());
        let path = artifact.resolve(root.path(), &state, false).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(artifact.resolve(root.path(), &state, true).await.is_err());
        artifact.resolve(root.path(), &state, false).await.unwrap();
        let mutable = Artifact::parse(json!({"name":"example:mutable:1", "MMC-hint":"always-stale", "MMC-absoluteUrl":url}), "jarmods").unwrap();
        let mutable_path =
            mutable.resolve(root.path(), &state, false).await.unwrap();
        write_jar(&mutable_path, &[("value", b"older cached version")]);
        mutable.resolve(root.path(), &state, true).await.unwrap();
        assert_eq!(read_entry(&mutable_path, "value"), b"older cached version");
        mutable.resolve(root.path(), &state, false).await.unwrap();
        assert_eq!(std::fs::read(&mutable_path).unwrap(), bytes);
        server.abort();
        assert_eq!(
            artifact.resolve(root.path(), &state, true).await.unwrap(),
            path
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let absolute = Artifact::parse(json!({"name":"example:patch:1", "MMC-absoluteUrl":"https://example.com/other.jar"}), "jarmods").unwrap();
        assert_eq!(
            absolute.download.as_ref().unwrap().url,
            "https://example.com/other.jar"
        );
        assert_ne!(absolute.cache_path, artifact.cache_path);
    }

    /// Verifies a real export without starting the bundled game or installer.
    #[tokio::test]
    #[ignore = "requires AXOLOTL_TEST_MULTIMC_ARCHIVE and AXOLOTL_TEST_MULTIMC_FIXTURE"]
    async fn multimc_real_bta_jar_mod_archive() {
        let archive_path =
            std::env::var_os("AXOLOTL_TEST_MULTIMC_ARCHIVE").unwrap();
        let fixture = PathBuf::from(
            std::env::var_os("AXOLOTL_TEST_MULTIMC_FIXTURE").unwrap(),
        );
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("minecraft");
        std::fs::create_dir_all(&game).unwrap();
        zip::ZipArchive::new(std::fs::File::open(archive_path).unwrap())
            .unwrap()
            .extract(root.path())
            .unwrap();
        let (_state_root, state) = state().await;
        let profile = MultiMcProfile::from_root(root.path().to_path_buf())
            .await
            .unwrap();
        let mut info: daedalus::minecraft::VersionInfo =
            serde_json::from_slice(
                &std::fs::read(fixture.join("vanilla.json")).unwrap(),
            )
            .unwrap();
        let library_count = info.libraries.len();
        profile
            .apply(&mut info, None, &state.directories.libraries_dir())
            .await
            .unwrap();
        assert_eq!(info.main_class, "net.minecraft.client.Minecraft");
        assert_eq!(info.java_version.as_ref().unwrap().major_version, 17);
        assert_eq!(info.libraries.len(), library_count);
        let artifact = profile.main_jar.as_ref().unwrap();
        let cache = artifact
            .path(root.path(), &state.directories.libraries_dir())
            .unwrap();
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::copy(fixture.join("client.jar"), cache).unwrap();
        let runtime = profile
            .prepare_runtime(
                &state,
                &fixture.join("client.jar"),
                std::env::consts::ARCH,
                false,
                true,
            )
            .await
            .unwrap();
        assert_eq!(runtime.jar_mods.len(), 1);
        let output = runtime
            .assemble(&fixture.join("client.jar"), &game)
            .await
            .unwrap();
        assert_eq!(
            read_entry(&output, "net/minecraft/client/Minecraft.class"),
            read_entry(
                &runtime.jar_mods[0],
                "net/minecraft/client/Minecraft.class"
            )
        );
        assert!(!read_entry(&output, "b100/installer/Main.class").is_empty());
        println!(
            "BTA official export: {} libraries, Java 17, direct main {}, merged JAR {} bytes",
            info.libraries.len(),
            info.main_class,
            std::fs::metadata(output).unwrap().len()
        );
    }
}
