//! Offline cleanup of superseded, journaled sync generations.

use super::{ensure_supported_generation, journal, manifest};
use crate::ErrorKind;
use fs4::tokio::AsyncFileExt;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::ErrorKind::NotFound;
use std::path::{Path, PathBuf};
use tokio::fs;

const BATCH: usize = 128;

#[derive(Debug, Default, Serialize)]
pub struct SyncCleanupReport {
    pub journals_scanned: usize,
    pub retained_for_recovery: usize,
    pub retained_recent_or_uncommitted: usize,
    pub failed: usize,
    pub candidates: usize,
    pub candidate_files: usize,
    pub candidate_bytes: u64,
    pub removed: usize,
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub limit_reached: bool,
}

fn invalid(message: impl Into<String>) -> crate::Error {
    ErrorKind::InputError(message.into()).into()
}

pub(crate) async fn checked_path(
    path: &Path,
    directory: bool,
) -> crate::Result<Option<u64>> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if reparse
        || !(if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        })
    {
        return Err(invalid(format!(
            "Unexpected sync maintenance path: {}",
            path.display()
        )));
    }
    Ok(Some(metadata.len()))
}

pub(crate) async fn require_path(
    path: &Path,
    directory: bool,
) -> crate::Result<()> {
    checked_path(path, directory).await?.ok_or_else(|| {
        invalid(format!("Missing sync maintenance path: {}", path.display()))
    })?;
    Ok(())
}

pub(crate) async fn lock(root: &Path) -> crate::Result<fs::File> {
    require_path(&root.join("state"), true).await?;
    let path = root.join("state/transaction.lock");
    require_path(&path, false).await?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .await?;
    if !lock.try_lock_exclusive()? {
        return Err(invalid("Sync publication is in progress"));
    }
    Ok(lock)
}

fn is_token(token: &str) -> bool {
    uuid::Uuid::parse_str(token).is_ok_and(|uuid| uuid.to_string() == token)
}

fn writer_lock(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".writer.lock");
    name.into()
}

fn generation_paths(head: &manifest::Head) -> crate::Result<HashSet<PathBuf>> {
    if !is_token(&head.transaction)
        || head.legacy_extensions_present
        || !head.extra.is_empty()
    {
        return Err(invalid("Unknown sync generation metadata"));
    }
    let prefix = format!("state/generations/{}/", head.transaction);
    let mut paths = HashSet::new();
    for (index, shard) in head.shards.iter().enumerate() {
        let name = format!("{index}.json");
        if !shard.extra.is_empty() || !is_sha256(&shard.sha256) {
            return Err(invalid("Unknown sync shard reference"));
        }
        if shard.file == format!("{prefix}{name}") {
            paths.insert(PathBuf::from(name));
        } else if shard.file
            != format!("{}/{}.json", manifest::SHARED_SHARDS, shard.sha256)
        {
            return Err(invalid("Unknown sync shard reference"));
        }
    }
    for (name, native) in &head.native {
        let file = match name.as_str() {
            "servers" => "native/servers.dat",
            "hotbars" => "native/hotbars.nbt",
            "command_history" => "native/command_history.txt",
            _ => return Err(invalid("Unknown native sync kind")),
        };
        if !native.extra.is_empty() || native.file != format!("{prefix}{file}")
        {
            return Err(invalid("Unknown native sync reference"));
        }
        paths.insert(PathBuf::from(file));
    }
    Ok(paths)
}

async fn validate_root(
    root: &Path,
    head: &manifest::Head,
) -> crate::Result<()> {
    let paths = generation_paths(head)?;
    if !paths.is_empty() {
        let generation = root.join("state/generations").join(&head.transaction);
        require_path(&generation, true).await?;
        if !head.native.is_empty() {
            require_path(&generation.join("native"), true).await?;
        }
        for path in paths {
            require_path(&generation.join(path), false).await?;
        }
    }
    let mut objects = HashSet::new();
    for shard in &head.shards {
        if shard.file.starts_with(manifest::SHARED_SHARDS) {
            require_path(&root.join(manifest::SHARED_SHARDS), true).await?;
            require_path(&root.join(&shard.file), false).await?;
        }
        let body =
            manifest::read_bounded(&root.join(&shard.file), 1 << 20).await?;
        manifest::collect_blob_paths(
            &serde_json::from_slice(&body)?,
            &mut objects,
        )?;
    }
    if !objects.is_empty() {
        require_path(&root.join("state/objects"), true).await?;
        for object in objects {
            let name = object
                .strip_prefix("state/objects/")
                .expect("object prefix");
            if !name.strip_suffix(".bin").is_some_and(is_sha256) {
                return Err(invalid("Unknown sync object reference"));
            }
            require_path(&root.join(object), false).await?;
        }
    }
    let collections = manifest::validate(root, head).await?;
    ensure_supported_generation(head, &collections)
}

fn is_sha256(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(crate) async fn read_envelope(
    path: &Path,
    protocol: &str,
) -> crate::Result<Option<Value>> {
    if checked_path(path, false).await?.is_none() {
        return Ok(None);
    }
    let mut raw: Value =
        serde_json::from_slice(&manifest::read_bounded(path, 16 << 20).await?)?;
    if raw.as_object().is_none_or(|object| object.len() != 4)
        || raw["protocol"] != protocol
        || raw["schema_version"].as_u64() != Some(1)
        || raw["revision"].as_u64().is_none()
        || !raw["data"].is_object()
    {
        return Err(invalid(format!(
            "Unsupported sync document: {}",
            path.display()
        )));
    }
    Ok(Some(raw["data"].take()))
}

async fn read_head(path: &Path) -> crate::Result<Option<manifest::Head>> {
    Ok(read_envelope(path, "axolotl-sync-state")
        .await?
        .map(serde_json::from_value)
        .transpose()?)
}

fn journal_path(root: &Path, token: &str) -> PathBuf {
    root.join("state/journals").join(format!("{token}.json"))
}

async fn roots(
    root: &Path,
    decision: Option<&str>,
) -> crate::Result<(u64, HashSet<String>)> {
    let current = read_head(&root.join("state/head.json"))
        .await?
        .ok_or_else(|| invalid("Current sync head is missing"))?;
    let previous = read_head(&root.join("state/head.json.previous")).await?;
    if previous.as_ref().is_some_and(|head| {
        head.revision >= current.revision && head != &current
    }) {
        return Err(invalid("Current and previous sync heads disagree"));
    }
    let mut tokens = HashSet::new();
    let mut oldest = current.revision;
    for head in std::iter::once(&current).chain(previous.as_ref()) {
        validate_root(root, head).await?;
        oldest = oldest.min(head.revision);
        tokens.insert(head.transaction.clone());
        if checked_path(&journal_path(root, &head.transaction), false)
            .await?
            .is_some()
            && journal::read_intent(root, &head.transaction, true)
                .await?
                .desired
                != *head
        {
            return Err(invalid("Sync head and journal disagree"));
        }
    }
    if let Some(token) = decision {
        if !is_token(token) {
            return Err(invalid("Unknown coordinator sync decision"));
        }
        require_path(&journal_path(root, token), false).await?;
        let intent = journal::read_intent(root, token, true).await?;
        if intent.desired != current && intent.expected != current.revision {
            return Err(invalid(
                "Coordinator decision cannot be replayed against sync head",
            ));
        }
        validate_root(root, &intent.desired).await?;
        tokens.insert(token.to_owned());
    }
    Ok((oldest, tokens))
}

struct Candidate {
    files: Vec<(PathBuf, u64)>,
    directories: Vec<PathBuf>,
    journal: PathBuf,
    journal_bytes: u64,
}

impl Candidate {
    async fn inspect(
        root: &Path,
        token: &str,
        head: &manifest::Head,
    ) -> crate::Result<Self> {
        let mut allowed = generation_paths(head)?;
        allowed.extend(
            allowed
                .iter()
                .map(|path| writer_lock(path))
                .collect::<Vec<_>>(),
        );
        let generation = root.join("state/generations").join(token);
        let mut files = Vec::new();
        let mut directories = Vec::new();
        if checked_path(&generation, true).await?.is_some() {
            let mut entries = fs::read_dir(&generation).await?;
            while let Some(entry) = entries.next_entry().await? {
                let name = entry.file_name();
                if name == "native" && !head.native.is_empty() {
                    let native = entry.path();
                    require_path(&native, true).await?;
                    let mut entries = fs::read_dir(&native).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        let relative =
                            Path::new("native").join(entry.file_name());
                        files.push(
                            inspect_file(&allowed, &relative, entry.path())
                                .await?,
                        );
                    }
                    directories.push(native);
                } else {
                    files.push(
                        inspect_file(&allowed, Path::new(&name), entry.path())
                            .await?,
                    );
                }
            }
            directories.push(generation);
        }
        let journal = journal_path(root, token);
        let mut previous = journal.as_os_str().to_owned();
        previous.push(".previous");
        if checked_path(Path::new(&previous), false).await?.is_some() {
            return Err(invalid("Unexpected previous copy of sync journal"));
        }
        let lock = writer_lock(&journal);
        if let Some(bytes) = checked_path(&lock, false).await? {
            files.push((lock, bytes));
        }
        let journal_bytes =
            checked_path(&journal, false).await?.ok_or_else(|| {
                invalid("Sync journal disappeared during maintenance")
            })?;
        Ok(Self {
            files,
            directories,
            journal,
            journal_bytes,
        })
    }

    async fn remove(self, report: &mut SyncCleanupReport) -> crate::Result<()> {
        // Leave the validated intent in place until all its files are gone so
        // a restart can resume after any interrupted removal.
        for (file, bytes) in self.files {
            fs::remove_file(file).await?;
            report.removed_files += 1;
            report.removed_bytes += bytes;
        }
        for directory in self.directories {
            fs::remove_dir(directory).await?;
        }
        fs::remove_file(self.journal).await?;
        report.removed_files += 1;
        report.removed_bytes += self.journal_bytes;
        report.removed += 1;
        Ok(())
    }
}

async fn inspect_file(
    allowed: &HashSet<PathBuf>,
    relative: &Path,
    path: PathBuf,
) -> crate::Result<(PathBuf, u64)> {
    if !allowed.contains(relative) {
        return Err(invalid(format!(
            "Unexpected generation entry: {}",
            path.display()
        )));
    }
    let size = checked_path(&path, false).await?.ok_or_else(|| {
        invalid("Generation file disappeared during maintenance")
    })?;
    Ok((path, size))
}

/// The caller holds coordinator ownership, its SQL writer reservation, and the
/// sync transaction lock throughout enumeration and removal.
pub(crate) async fn scan(
    root: &Path,
    decision: Option<&str>,
    apply: bool,
    limit: Option<usize>,
    mut progress: impl FnMut(&SyncCleanupReport),
    mut failure: impl FnMut(&str, &crate::Error),
) -> crate::Result<SyncCleanupReport> {
    let mut report = SyncCleanupReport::default();
    checked_path(&root.join("state/generations"), true).await?;
    let journals = root.join("state/journals");
    let has_journals = checked_path(&journals, true).await?.is_some();
    let (oldest, keep) = roots(root, decision).await?;
    if !has_journals {
        return Ok(report);
    }
    let mut entries = fs::read_dir(journals).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let Some(token) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|name| is_token(name))
        else {
            continue;
        };
        report.journals_scanned += 1;
        if keep.contains(token) {
            report.retained_for_recovery += 1;
        } else {
            let result = async {
                require_path(&entry.path(), false).await?;
                let intent = journal::read_intent(root, token, true).await?;
                if intent.desired.revision >= oldest {
                    report.retained_recent_or_uncommitted += 1;
                    return Ok(());
                }
                let candidate =
                    Candidate::inspect(root, token, &intent.desired).await?;
                report.candidates += 1;
                report.candidate_files += candidate.files.len() + 1;
                report.candidate_bytes += candidate.journal_bytes
                    + candidate
                        .files
                        .iter()
                        .map(|(_, bytes)| bytes)
                        .sum::<u64>();
                if apply {
                    candidate.remove(&mut report).await?;
                }
                Ok(())
            }
            .await;
            if let Err(error) = result {
                report.failed += 1;
                failure(token, &error);
            }
        }
        if report.journals_scanned % BATCH == 0 {
            progress(&report);
        }
        if limit.is_some_and(|limit| report.candidates >= limit) {
            report.limit_reached = true;
            break;
        }
    }
    Ok(report)
}
