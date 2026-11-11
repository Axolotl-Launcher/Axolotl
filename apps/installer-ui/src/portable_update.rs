//! A signed portable package is staged before exit and activated by a headless worker.
//! The main executable is the commit point; startup rolls back an interrupted resource move.

use crate::executable_install;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    },
};

type Result<T> = std::result::Result<T, String>;
const MAIN: &str = "Axolotl Launcher.exe";
const HELPER: &str = "axolotl-updater.exe";
const MARKER: &str = ".axolotl-portable-update.json";
const PREFIX: &str = ".axolotl-update-";
const MAX_FILES: usize = 20_000;
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct DigestInfo {
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Item {
    path: String,
    next: DigestInfo,
    previous: Option<DigestInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Journal {
    format: u32,
    stage: String,
    executable: String,
    previous_main: DigestInfo,
    next_main: DigestInfo,
    items: Vec<Item>,
}

/// Owns an unpublished stage. Removing an enqueued update removes its stage too.
pub(crate) struct PreparedUpdate {
    stage: tempfile::TempDir,
    executable: PathBuf,
}

fn digest(path: &Path) -> Result<DigestInfo> {
    regular(path)?;
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count =
            file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(DigestInfo {
        size,
        sha256: format!("{:x}", hash.finalize()),
    })
}

fn regular(path: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err(format!(
            "Not a regular portable program file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.ends_with(['.', ' '])
        && !value.chars().any(|c| c < ' ' || "\\/:*?\"<>|".contains(c))
        && !matches!(
            value
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
}

fn program_path(path: &str) -> bool {
    (path == HELPER
        || path.starts_with("resources/blockbench-skin/")
        || path.starts_with("resources/seed-map/"))
        && path.split('/').all(component)
}

fn safe_parents(root: &Path, relative: &str) -> Result<()> {
    let metadata =
        fs::symlink_metadata(root).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(format!("Unsafe portable root: {}", root.display()));
    }
    let mut path = root.to_path_buf();
    let parts: Vec<_> = relative.split('/').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.is_dir()
                    && metadata.file_attributes() & 0x400 == 0 => {}
            Ok(_) => {
                return Err(format!(
                    "Unsafe portable directory: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|error| error.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

/// Called only after the package's own minisign signature has been verified.
pub(crate) fn prepare(
    bytes: &[u8],
    executable: PathBuf,
) -> Result<PreparedUpdate> {
    let root = executable
        .parent()
        .ok_or("Portable executable has no directory")?;
    let stage = tempfile::Builder::new()
        .prefix(PREFIX)
        .tempdir_in(root)
        .map_err(|error| error.to_string())?;
    let payload = stage.path().join("payload");
    fs::create_dir(&payload).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| error.to_string())?;
    if archive.len() > MAX_FILES {
        return Err("Portable archive has too many entries".into());
    }
    let mut names = HashSet::new();
    let mut spellings = HashMap::new();
    let mut total = 0_u64;
    let mut files = HashSet::new();
    for index in 0..archive.len() {
        let mut entry =
            archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry
            .name()
            .map_err(|error| error.to_string())?
            .into_owned();
        let trimmed = name.strip_suffix('/').unwrap_or(&name);
        if !trimmed.split('/').all(component)
            || !names.insert(trimmed.to_ascii_lowercase())
        {
            return Err(format!(
                "Unsafe or duplicate portable archive path: {name}"
            ));
        }
        let mut prefix = String::new();
        for part in trimmed.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if let Some(previous) =
                spellings.insert(prefix.to_ascii_lowercase(), prefix.clone())
            {
                if previous != prefix {
                    return Err(
                        "Portable archive contains case-colliding paths".into(),
                    );
                }
            }
        }
        if entry.unix_mode().is_some_and(|mode| {
            mode & 0o170000 != 0
                && !matches!(mode & 0o170000, 0o100000 | 0o040000)
        }) {
            return Err(
                "Portable archive contains a link or special file".into()
            );
        }
        if trimmed == "Axolotl" && entry.is_dir() {
            continue;
        }
        let relative = trimmed
            .strip_prefix("Axolotl/")
            .ok_or("Unexpected portable archive root")?;
        if relative == ".Axolotl" && entry.is_dir() && entry.size() == 0 {
            continue;
        }
        if entry.is_dir() {
            if relative == "resources"
                || relative == "resources/blockbench-skin"
                || relative == "resources/seed-map"
                || program_path(&format!("{relative}/file"))
            {
                continue;
            }
            return Err(format!(
                "Unexpected portable archive directory: {name}"
            ));
        }
        if relative != MAIN && !program_path(relative) {
            return Err(format!(
                "Portable archive contains a non-program file: {name}"
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Portable archive size overflow")?;
        if total > MAX_BYTES {
            return Err("Portable archive expands beyond its size limit".into());
        }
        safe_parents(&payload, relative)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(payload.join(relative))
            .map_err(|error| error.to_string())?;
        let expected = entry.size();
        let written = std::io::copy(
            &mut (&mut entry).take(expected.saturating_add(1)),
            &mut output,
        )
        .map_err(|error| error.to_string())?;
        if written != expected {
            return Err("Portable archive entry size mismatch".into());
        }
        output.sync_all().map_err(|error| error.to_string())?;
        files.insert(relative.to_owned());
    }
    if !files.contains(MAIN)
        || !files.contains(HELPER)
        || !files
            .iter()
            .any(|name| name.starts_with("resources/blockbench-skin/"))
        || !files.contains("resources/seed-map/axolotl-cubiomes.dll")
    {
        return Err("Portable archive is missing required program files".into());
    }
    executable_install::check(&payload.join(MAIN))?;
    executable_install::check(&payload.join(HELPER))?;
    let mut inventory: Vec<_> = files
        .into_iter()
        .map(|path| {
            digest(&payload.join(&path)).map(|next| Item {
                path,
                next,
                previous: None,
            })
        })
        .collect::<Result<_>>()?;
    inventory.sort_by(|left, right| left.path.cmp(&right.path));
    write_new(&stage.path().join("inventory.json"), &inventory)?;
    fs::copy(payload.join(HELPER), stage.path().join("worker.exe"))
        .map_err(|error| error.to_string())?;
    Ok(PreparedUpdate { stage, executable })
}

fn write_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    regular(path)?;
    let file = File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len()
        > 16 * 1024 * 1024
    {
        return Err("Portable update journal is too large".into());
    }
    serde_json::from_reader(file).map_err(|error| error.to_string())
}

fn ownership(root: &Path) -> Result<File> {
    OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0)
        .open(root.join(".axolotl-install.lock"))
        .map_err(|error| format!("Another installation or portable update is in progress: {error}"))
}

impl PreparedUpdate {
    /// The worker must hold the installation lock and the actual parent process handle
    /// before the launcher lets its database and process finish shutting down.
    pub(crate) fn start(self, restart: bool) -> Result<()> {
        let mut child = Command::new(self.stage.path().join("worker.exe"))
            .arg("--portable-update")
            .arg(self.stage.path())
            .arg(&self.executable)
            .arg(std::process::id().to_string())
            .arg(if restart { "restart" } else { "quit" })
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|error| error.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if self.stage.path().join("ready").is_file() {
                let _stage = self.stage.keep();
                return Ok(());
            }
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                let error =
                    fs::read_to_string(self.stage.path().join("error.txt"))
                        .unwrap_or_else(|_| {
                            "Portable updater stopped before it became ready"
                                .into()
                        });
                return Err(error);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Portable updater did not become ready".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn validate_journal(root: &Path, journal: &Journal) -> Result<PathBuf> {
    if journal.format != 1
        || !component(&journal.stage)
        || !journal.stage.starts_with(PREFIX)
        || !component(&journal.executable)
        || !journal.executable.to_ascii_lowercase().ends_with(".exe")
        || journal.executable.eq_ignore_ascii_case(HELPER)
        || journal.items.len() > MAX_FILES
    {
        return Err("Invalid portable update journal".into());
    }
    let mut names = HashSet::new();
    for item in &journal.items {
        if !program_path(&item.path)
            || !names.insert(item.path.to_ascii_lowercase())
        {
            return Err("Invalid portable update program path".into());
        }
    }
    let stage = root.join(&journal.stage);
    if !journal.items.iter().any(|item| item.path == HELPER)
        || !journal
            .items
            .iter()
            .any(|item| item.path == "resources/seed-map/axolotl-cubiomes.dll")
        || !journal
            .items
            .iter()
            .any(|item| item.path.starts_with("resources/blockbench-skin/"))
    {
        return Err("Portable journal is missing required program files".into());
    }
    let metadata =
        fs::symlink_metadata(&stage).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err("Unsafe portable update staging directory".into());
    }
    Ok(stage)
}

fn matches(path: &Path, expected: &DigestInfo) -> bool {
    digest(path).is_ok_and(|actual| actual == *expected)
}

fn recover_owned(root: &Path) -> Result<()> {
    let marker = root.join(MARKER);
    if !marker.try_exists().map_err(|error| error.to_string())? {
        return Ok(());
    }
    let journal: Journal = read_json(&marker)?;
    let stage = validate_journal(root, &journal)?;
    let target_main = root.join(&journal.executable);
    if matches(&target_main, &journal.next_main) {
        // The executable was atomically published; a stale phase marker must never
        // undo the resources belonging to this new executable.
        for item in &journal.items {
            safe_parents(root, &item.path)?;
            if !matches(&root.join(&item.path), &item.next) {
                return Err(format!(
                    "Committed portable resource failed verification: {}",
                    item.path
                ));
            }
        }
    } else {
        if !matches(&target_main, &journal.previous_main) {
            return Err("Portable executable matches neither side of the interrupted update; backups were retained".into());
        }
        for item in journal.items.iter().rev() {
            safe_parents(root, &item.path)?;
            safe_parents(&stage.join("previous"), &item.path)?;
            let target = root.join(&item.path);
            let backup = stage.join("previous").join(&item.path);
            if let Some(previous) = &item.previous {
                if backup.try_exists().map_err(|error| error.to_string())? {
                    if !matches(&backup, previous) {
                        return Err(
                            "Portable resource backup failed verification"
                                .into(),
                        );
                    }
                    if target.try_exists().map_err(|error| error.to_string())? {
                        if !matches(&target, &item.next) {
                            return Err(
                                "Portable resource changed during recovery"
                                    .into(),
                            );
                        }
                        fs::remove_file(&target)
                            .map_err(|error| error.to_string())?;
                    }
                    fs::rename(&backup, &target)
                        .map_err(|error| error.to_string())?;
                } else if !matches(&target, previous) {
                    return Err(
                        "Portable resource and its backup are unavailable"
                            .into(),
                    );
                }
            } else if target.try_exists().map_err(|error| error.to_string())? {
                if !matches(&target, &item.next) {
                    return Err(
                        "Unexpected portable resource during recovery".into()
                    );
                }
                fs::remove_file(&target).map_err(|error| error.to_string())?;
            }
        }
    }
    fs::remove_file(marker).map_err(|error| error.to_string())?;
    // Retain the previous resource files as well as the EXE backup. Cleanup of
    // historical backups is deliberately separate from the activation transaction.
    Ok(())
}

/// Run before any bundled resources are loaded. A live worker excludes startup.
pub(crate) fn recover(executable: &Path) -> Result<()> {
    let root = executable
        .parent()
        .ok_or("Portable executable has no directory")?;
    if !root.join(".Axolotl").is_dir() && !root.join(MARKER).exists() {
        return Ok(());
    }
    let _ownership = ownership(root)?;
    recover_owned(root)
}

fn activate(root: &Path, stage: &Path, executable: &Path) -> Result<()> {
    recover_owned(root)?;
    let mut inventory: Vec<Item> = read_json(&stage.join("inventory.json"))?;
    let main_index = inventory
        .iter()
        .position(|item| item.path == MAIN)
        .ok_or("Missing portable main executable")?;
    let main = inventory.remove(main_index);
    let mut journal = Journal {
        format: 1,
        stage: stage
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid staging name")?
            .into(),
        executable: executable
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid executable name")?
            .into(),
        previous_main: digest(executable)?,
        next_main: main.next,
        items: inventory,
    };
    validate_journal(root, &journal)?;
    if journal.previous_main == journal.next_main {
        return Err("Portable package contains the current executable".into());
    }
    executable_install::check(&stage.join("payload").join(MAIN))?;
    fs::create_dir(stage.join("previous"))
        .map_err(|error| error.to_string())?;
    for item in &mut journal.items {
        safe_parents(root, &item.path)?;
        safe_parents(&stage.join("payload"), &item.path)?;
        if !matches(&stage.join("payload").join(&item.path), &item.next) {
            return Err("Portable stage failed verification".into());
        }
        let target = root.join(&item.path);
        if target.try_exists().map_err(|error| error.to_string())? {
            item.previous = Some(digest(&target)?);
        }
    }
    // Persist the complete undo description before the first installed file moves.
    let mut marker = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(marker.as_file_mut(), &journal)
        .map_err(|error| error.to_string())?;
    marker
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    marker
        .persist_noclobber(root.join(MARKER))
        .map_err(|error| error.to_string())?;
    checkpoint("journal");
    let result = (|| {
        for item in &journal.items {
            let target = root.join(&item.path);
            if item.previous.is_some() {
                safe_parents(&stage.join("previous"), &item.path)?;
                fs::rename(&target, stage.join("previous").join(&item.path))
                    .map_err(|error| error.to_string())?;
                checkpoint("previous");
            }
            fs::rename(stage.join("payload").join(&item.path), &target)
                .map_err(|error| error.to_string())?;
            checkpoint("resource");
        }
        let mut staged_main = tempfile::Builder::new()
            .prefix(".axolotl-next-")
            .suffix(".exe")
            .tempfile_in(root)
            .map_err(|error| error.to_string())?;
        let mut source = File::open(stage.join("payload").join(MAIN))
            .map_err(|error| error.to_string())?;
        std::io::copy(&mut source, staged_main.as_file_mut())
            .map_err(|error| error.to_string())?;
        staged_main
            .as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        // Close the temporary file handle before ReplaceFileW, which needs
        // delete access to consume the staged path on Windows.
        let staged_main = staged_main.into_temp_path();
        checkpoint("before-main");
        executable_install::install(
            &staged_main,
            executable,
            &executable_install::Executable {
                bytes: journal.next_main.size,
                sha256: journal.next_main.sha256.clone(),
            },
        )?;
        checkpoint("after-main");
        Ok(())
    })();
    let recovery = recover_owned(root);
    result.and(recovery)
}

#[inline]
fn checkpoint(_name: &str) {
    #[cfg(test)]
    if std::env::var("AXOLOTL_PORTABLE_TEST_STOP").as_deref() == Ok(_name) {
        std::process::exit(91);
    }
}

fn worker(
    stage: &Path,
    executable: &Path,
    parent: u32,
    restart: bool,
) -> Result<()> {
    // Open before readiness: waiting on a handle cannot accidentally wait for a
    // recycled PID or permit writes while the original launcher is still alive.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, parent) }
        .map_err(|error| error.to_string())?;
    let process = unsafe { OwnedHandle::from_raw_handle(process.0) };
    let root = executable
        .parent()
        .ok_or("Portable executable has no directory")?;
    if fs::canonicalize(stage.parent().ok_or("Invalid portable stage")?)
        .map_err(|error| error.to_string())?
        != fs::canonicalize(root).map_err(|error| error.to_string())?
        || !stage
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(PREFIX) && component(name))
    {
        return Err(
            "Portable stage must be beside the current executable".into()
        );
    }
    let _ownership = ownership(root)?;
    write_new(&stage.join("ready"), &true)?;
    if unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), INFINITE) }
        != WAIT_OBJECT_0
    {
        return Err("Could not wait for the launcher to exit".into());
    }
    activate(root, stage, executable)?;
    if restart {
        drop(_ownership);
        Command::new(executable)
            .current_dir(root)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn dispatch(
    mut arguments: impl Iterator<Item = OsString>,
) -> Option<Result<()>> {
    if arguments.next()? != "--portable-update" {
        return None;
    }
    Some((|| {
        let args: Vec<_> = arguments.collect();
        if args.len() != 4 {
            return Err("Invalid portable updater arguments".into());
        }
        let stage = Path::new(&args[0]);
        let result = (|| {
            let parent = args[2]
                .to_str()
                .and_then(|value| value.parse().ok())
                .ok_or("Invalid portable parent")?;
            if args[3] != "restart" && args[3] != "quit" {
                return Err("Invalid portable restart intent".into());
            }
            worker(stage, Path::new(&args[1]), parent, args[3] == "restart")
        })();
        if let Err(error) = &result {
            let _ = fs::write(stage.join("error.txt"), error);
        }
        result
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Stdio};
    use zip::write::SimpleFileOptions;

    fn pe(marker: u8) -> Vec<u8> {
        let mut bytes = vec![marker; 1024];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[70..72].copy_from_slice(&1_u16.to_le_bytes());
        bytes[84..86].copy_from_slice(&96_u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[200..208].fill(0);
        bytes
    }

    fn archive(extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let executable = pe(2);
        for (name, bytes) in [
            ("Axolotl/Axolotl Launcher.exe", executable.as_slice()),
            ("Axolotl/axolotl-updater.exe", executable.as_slice()),
            (
                "Axolotl/resources/blockbench-skin/index.html",
                b"new resources".as_slice(),
            ),
            (
                "Axolotl/resources/seed-map/axolotl-cubiomes.dll",
                b"new native".as_slice(),
            ),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn installation() -> tempfile::TempDir {
        let root = tempfile::Builder::new()
            .prefix("Axolotl portable 中文 ")
            .tempdir()
            .unwrap();
        fs::write(root.path().join(MAIN), pe(1)).unwrap();
        fs::write(root.path().join(HELPER), pe(1)).unwrap();
        fs::create_dir(root.path().join(".Axolotl")).unwrap();
        fs::write(root.path().join(".Axolotl/app.db"), b"user database")
            .unwrap();
        fs::write(root.path().join("user-file.txt"), b"unrelated").unwrap();
        fs::create_dir_all(root.path().join("resources/blockbench-skin"))
            .unwrap();
        fs::write(
            root.path().join("resources/blockbench-skin/index.html"),
            b"old resources",
        )
        .unwrap();
        fs::write(
            root.path().join("resources/blockbench-skin/user.txt"),
            b"user resource",
        )
        .unwrap();
        root
    }

    fn assert_user_data(root: &Path) {
        assert_eq!(
            fs::read(root.join(".Axolotl/app.db")).unwrap(),
            b"user database"
        );
        assert_eq!(fs::read(root.join("user-file.txt")).unwrap(), b"unrelated");
        assert_eq!(
            fs::read(root.join("resources/blockbench-skin/user.txt")).unwrap(),
            b"user resource"
        );
    }

    #[test]
    fn archive_rejects_data_paths_aliases_and_unexpected_roots() {
        let root = installation();
        let executable = root.path().join(MAIN);
        for name in [
            "Axolotl/.Axolotl/app.db",
            "Axolotl/../outside",
            "Axolotl/resources/seed-map/../file",
            "Axolotl/resources/seed-map/CON.txt",
            "Axolotl/resources/seed-map/file.",
            "Axolotl/resources/seed-map/file:stream",
            "Axolotl\\resources\\evil",
            "/Axolotl/evil",
            "Other/file",
            "Axolotl/resources/seed-map/AXOLOTL-CUBIOMES.DLL",
        ] {
            assert!(
                prepare(&archive(&[(name, b"malicious")]), executable.clone())
                    .is_err(),
                "accepted {name}"
            );
        }
        assert!(
            prepare(
                &archive(&[
                    ("Axolotl/resources/seed-map/Extra/file", b"one"),
                    ("Axolotl/resources/seed-map/extra/other", b"two"),
                ]),
                executable.clone()
            )
            .is_err()
        );
        assert_user_data(root.path());
        assert_eq!(fs::read(executable).unwrap(), pe(1));
    }

    #[test]
    fn archive_rejects_links_and_incomplete_packages() {
        let root = installation();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_symlink(
            "Axolotl/resources/seed-map/link",
            "../../../.Axolotl",
            SimpleFileOptions::default(),
        )
        .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(prepare(&bytes, root.path().join(MAIN)).is_err());
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            "Axolotl/Axolotl Launcher.exe",
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&pe(2)).unwrap();
        assert!(
            prepare(
                &zip.finish().unwrap().into_inner(),
                root.path().join(MAIN)
            )
            .is_err()
        );
    }

    #[test]
    fn activation_preserves_user_files_and_keeps_previous_program_bytes() {
        let root = installation();
        let target = root.path().join(MAIN);
        let prepared = prepare(&archive(&[]), target.clone()).unwrap();
        let _lock = ownership(root.path()).unwrap();
        activate(root.path(), prepared.stage.path(), &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), pe(2));
        assert_eq!(
            fs::read(root.path().join("resources/blockbench-skin/index.html"))
                .unwrap(),
            b"new resources"
        );
        assert_eq!(
            fs::read(
                prepared
                    .stage
                    .path()
                    .join("previous/resources/blockbench-skin/index.html")
            )
            .unwrap(),
            b"old resources"
        );
        assert!(!root.path().join(MARKER).exists());
        assert_user_data(root.path());
    }

    #[test]
    fn tampered_stage_is_rejected_before_moving_installed_files() {
        let root = installation();
        let target = root.path().join(MAIN);
        let prepared = prepare(&archive(&[]), target.clone()).unwrap();
        fs::write(
            prepared
                .stage
                .path()
                .join("payload/resources/blockbench-skin/index.html"),
            b"tampered",
        )
        .unwrap();
        assert!(activate(root.path(), prepared.stage.path(), &target).is_err());
        assert_eq!(fs::read(target).unwrap(), pe(1));
        assert_eq!(
            fs::read(root.path().join("resources/blockbench-skin/index.html"))
                .unwrap(),
            b"old resources"
        );
        assert!(!root.path().join(MARKER).exists());
        assert_user_data(root.path());
    }

    #[test]
    fn install_ownership_excludes_competing_updates_and_startup() {
        let root = installation();
        let lock = ownership(root.path()).unwrap();
        assert!(ownership(root.path()).is_err());
        assert!(recover(&root.path().join(MAIN)).is_err());
        drop(lock);
        recover(&root.path().join(MAIN)).unwrap();
    }

    #[test]
    fn interrupted_activation_recovers_the_executable_resource_family() {
        const DIRECTORY: &str = "AXOLOTL_PORTABLE_TEST_ROOT";
        if let Some(directory) = std::env::var_os(DIRECTORY) {
            let root = PathBuf::from(directory);
            let stage = PathBuf::from(
                std::env::var_os("AXOLOTL_PORTABLE_TEST_STAGE").unwrap(),
            );
            let _lock = ownership(&root).unwrap();
            activate(&root, &stage, &root.join(MAIN)).unwrap();
            panic!("The requested cut point was not reached");
        }
        for point in [
            "journal",
            "previous",
            "resource",
            "before-main",
            "after-main",
        ] {
            let root = installation();
            let target = root.path().join(MAIN);
            let prepared = prepare(&archive(&[]), target.clone()).unwrap();
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "portable_update::tests::interrupted_activation_recovers_the_executable_resource_family", "--nocapture"])
                .env(DIRECTORY, root.path()).env("AXOLOTL_PORTABLE_TEST_STAGE", prepared.stage.path())
                .env("AXOLOTL_PORTABLE_TEST_STOP", point).stdout(Stdio::null()).status().unwrap();
            assert_eq!(status.code(), Some(91), "cut point {point}");
            recover(&target).unwrap();
            recover(&target).unwrap();
            let committed = point == "after-main";
            assert_eq!(
                fs::read(&target).unwrap(),
                pe(if committed { 2 } else { 1 }),
                "{point}"
            );
            assert_eq!(
                fs::read(
                    root.path().join("resources/blockbench-skin/index.html")
                )
                .unwrap(),
                if committed {
                    b"new resources"
                } else {
                    b"old resources"
                },
                "{point}"
            );
            assert_eq!(
                root.path()
                    .join("resources/seed-map/axolotl-cubiomes.dll")
                    .exists(),
                committed
            );
            assert!(!root.path().join(MARKER).exists());
            assert_user_data(root.path());
        }
    }

    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn wait_for(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "Timed out waiting for {}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn worker_waits_for_actual_parent_and_quit_does_not_relaunch() {
        const ROLE: &str = "AXOLOTL_PORTABLE_WORKER_TEST_ROLE";
        const ROOT: &str = "AXOLOTL_PORTABLE_WORKER_TEST_ROOT";
        if let Some(role) = std::env::var_os(ROLE) {
            let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
            if role == "parent" {
                fs::write(root.join("parent-ready"), b"ready").unwrap();
                wait_for(&root.join("parent-release"));
            } else {
                let parent: u32 =
                    std::env::var("AXOLOTL_PORTABLE_WORKER_TEST_PARENT")
                        .unwrap()
                        .parse()
                        .unwrap();
                let stage = PathBuf::from(
                    std::env::var_os("AXOLOTL_PORTABLE_WORKER_TEST_STAGE")
                        .unwrap(),
                );
                worker(&stage, &root.join(MAIN), parent, false).unwrap();
            }
            return;
        }
        let root = installation();
        let target = root.path().join(MAIN);
        let prepared = prepare(&archive(&[]), target.clone()).unwrap();
        let spawn = |role: &str, parent: u32| {
            OwnedChild(Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "portable_update::tests::worker_waits_for_actual_parent_and_quit_does_not_relaunch", "--nocapture"])
                .env(ROLE, role).env(ROOT, root.path()).env("AXOLOTL_PORTABLE_WORKER_TEST_PARENT", parent.to_string())
                .env("AXOLOTL_PORTABLE_WORKER_TEST_STAGE", prepared.stage.path())
                .stdout(Stdio::null()).spawn().unwrap())
        };
        let mut parent = spawn("parent", 0);
        wait_for(&root.path().join("parent-ready"));
        let mut worker = spawn("worker", parent.0.id());
        wait_for(&prepared.stage.path().join("ready"));
        assert_eq!(fs::read(&target).unwrap(), pe(1));
        assert!(!root.path().join(MARKER).exists());
        assert!(ownership(root.path()).is_err());
        parent.0.kill().unwrap();
        parent.0.wait().unwrap();
        assert!(worker.0.wait().unwrap().success());
        assert_eq!(fs::read(&target).unwrap(), pe(2));
        assert!(ownership(root.path()).is_ok());
        assert_user_data(root.path());
    }
}
