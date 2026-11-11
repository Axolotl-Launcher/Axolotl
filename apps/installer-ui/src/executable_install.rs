//! Validates and publishes a complete Windows executable without truncating its predecessor.

use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
};
use windows::{
    Win32::{
        Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE},
        Storage::FileSystem::{
            FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW,
            MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS,
            ReplaceFileW,
        },
        System::Threading::{
            GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE,
        },
    },
    core::PCWSTR,
};

type Result<T> = std::result::Result<T, String>;

pub(crate) fn check(path: &Path) -> Result<()> {
    inspect(path).map(|_| ())
}

#[derive(Debug, PartialEq)]
struct Executable {
    bytes: u64,
    sha256: String,
}

pub(crate) fn dispatch(
    arguments: impl Iterator<Item = OsString>,
) -> Option<Result<()>> {
    let mut arguments = arguments;
    let command = arguments.next()?;
    let name = command.to_str()?;
    if !matches!(
        name,
        "--describe-executable" | "--install-executable" | "--check-executable"
    ) {
        return None;
    }
    Some((|| {
        let values: Vec<OsString> = arguments.collect();
        match name {
            "--describe-executable"
                if values.len() == 3 && values[1] == "--output" =>
            {
                let metadata = inspect(Path::new(&values[0]))?;
                fs::write(
                    Path::new(&values[2]),
                    format!(
                        "!define AXL_EXECUTABLE_BYTES {}\n!define AXL_EXECUTABLE_SHA256 \"{}\"\n",
                        metadata.bytes, metadata.sha256
                    ),
                )
                .map_err(|error| error.to_string())
            }
            "--check-executable" if values.len() == 1 => {
                inspect(Path::new(&values[0])).map(|_| ())
            }
            "--install-executable" if values.len() == 6 => {
                let owner_process = values[4]
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or("Invalid installer ownership process")?;
                let owner_handle = values[5]
                    .to_str()
                    .and_then(|value| value.parse::<isize>().ok())
                    .ok_or("Invalid installer ownership handle")?;
                // Duplicate before inspecting or publishing the stage. If the
                // parent already exited, fail without touching the installed EXE.
                let _ownership = inherit_ownership(
                    Path::new(&values[1]),
                    owner_process,
                    owner_handle,
                )?;
                let expected = Executable {
                    bytes: values[2]
                        .to_str()
                        .and_then(|value| value.parse().ok())
                        .ok_or("Invalid expected executable length")?,
                    sha256: values[3].to_string_lossy().to_ascii_lowercase(),
                };
                if expected.bytes == 0
                    || expected.sha256.len() != 64
                    || !expected
                        .sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err("Invalid executable integrity metadata".into());
                }
                install(Path::new(&values[0]), Path::new(&values[1]), &expected)
            }
            _ => Err("Invalid executable installation arguments".into()),
        }
    })())
}

fn inherit_ownership(
    target: &Path,
    owner_process: u32,
    owner_handle: isize,
) -> Result<File> {
    if matches!(owner_handle, 0 | -1) {
        return Err("Invalid installer ownership handle".into());
    }
    let source_process =
        unsafe { OpenProcess(PROCESS_DUP_HANDLE, false, owner_process) }
            .map_err(|error| {
                format!("Installer ownership is no longer available: {error}")
            })?;
    // Own every successfully opened/duplicated handle exactly once, including
    // errors while verifying the transferred handle's file identity.
    let source_process =
        unsafe { OwnedHandle::from_raw_handle(source_process.0) };
    let mut duplicate = HANDLE::default();
    unsafe {
        DuplicateHandle(
            HANDLE(source_process.as_raw_handle()),
            HANDLE(owner_handle as *mut _),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .map_err(|error| {
        format!("Could not retain installer ownership: {error}")
    })?;
    let ownership = unsafe { File::from_raw_handle(duplicate.0) };
    let mut name = vec![0_u16; 32_768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            HANDLE(ownership.as_raw_handle()),
            &mut name,
            FILE_NAME_NORMALIZED,
        )
    } as usize;
    if length == 0 || length >= name.len() {
        return Err("Could not verify the installer ownership file".into());
    }
    let actual = std::path::PathBuf::from(OsString::from_wide(&name[..length]));
    let parent = target
        .parent()
        .ok_or("Executable has no parent directory")?;
    let expected = fs::canonicalize(parent)
        .map_err(|error| error.to_string())?
        .join(".axolotl-install.lock");
    if !ownership
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
        || !actual
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        return Err(
            "Transferred ownership does not match this installation".into()
        );
    }
    Ok(ownership)
}

fn inspect(path: &Path) -> Result<Executable> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err(format!(
            "Executable is not a regular file: {}",
            path.display()
        ));
    }
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let bytes = metadata.len();
    validate_pe(&mut file, bytes)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count =
            file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(Executable {
        bytes,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn validate_pe(file: &mut File, bytes: u64) -> Result<()> {
    let invalid = || {
        "Executable is empty, truncated or has an invalid PE header".to_owned()
    };
    let mut dos = [0_u8; 64];
    file.read_exact(&mut dos).map_err(|_| invalid())?;
    if &dos[..2] != b"MZ" {
        return Err(invalid());
    }
    let offset = u64::from(u32::from_le_bytes(dos[60..64].try_into().unwrap()));
    if offset < 64 || offset.checked_add(24).is_none_or(|end| end > bytes) {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(offset)).map_err(|_| invalid())?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header).map_err(|_| invalid())?;
    let sections = u16::from_le_bytes(header[6..8].try_into().unwrap());
    let optional = u16::from_le_bytes(header[20..22].try_into().unwrap());
    if &header[..4] != b"PE\0\0"
        || !(1..=96).contains(&sections)
        || optional < 96
        || offset + 24 + u64::from(optional) + u64::from(sections) * 40 > bytes
    {
        return Err(invalid());
    }
    let mut magic = [0_u8; 2];
    file.read_exact(&mut magic).map_err(|_| invalid())?;
    if !matches!(u16::from_le_bytes(magic), 0x10b | 0x20b) {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(offset + 24 + u64::from(optional)))
        .map_err(|_| invalid())?;
    for _ in 0..sections {
        let mut section = [0_u8; 40];
        file.read_exact(&mut section).map_err(|_| invalid())?;
        let size =
            u64::from(u32::from_le_bytes(section[16..20].try_into().unwrap()));
        let start =
            u64::from(u32::from_le_bytes(section[20..24].try_into().unwrap()));
        if start.checked_add(size).is_none_or(|end| end > bytes) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn install(stage: &Path, target: &Path, expected: &Executable) -> Result<()> {
    let parent = target
        .parent()
        .ok_or("Executable has no parent directory")?;
    let parent = fs::canonicalize(parent).map_err(|error| error.to_string())?;
    let stage_parent = fs::canonicalize(
        stage.parent().ok_or("Staged executable has no parent")?,
    )
    .map_err(|error| error.to_string())?;
    if parent != stage_parent || stage.file_name() == target.file_name() {
        return Err("The executable must be staged beside a distinct installation target".into());
    }
    record(
        &parent,
        &format!("Verifying staged executable {}", stage.display()),
    );
    let result = (|| {
        let actual = inspect(stage)?;
        if &actual != expected {
            return Err(format!(
                "Staged executable integrity mismatch: {}",
                stage.display()
            ));
        }
        OpenOptions::new()
            .write(true)
            .open(stage)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
        let target_exists =
            target.try_exists().map_err(|error| error.to_string())?;
        if target_exists {
            let metadata = fs::symlink_metadata(target)
                .map_err(|error| error.to_string())?;
            if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                return Err("Installed executable is not a regular file".into());
            }
            let backup = tempfile::Builder::new()
                .prefix(".axolotl-previous-")
                .suffix(".exe")
                .tempfile_in(&parent)
                .map_err(|error| error.to_string())?;
            let backup_path = backup.path().to_path_buf();
            backup.close().map_err(|error| error.to_string())?;
            record(
                &parent,
                &format!(
                    "Replacing executable; previous file: {}",
                    backup_path.display()
                ),
            );
            let replace = unsafe {
                ReplaceFileW(
                    PCWSTR(wide(target).as_ptr()),
                    PCWSTR(wide(stage).as_ptr()),
                    PCWSTR(wide(&backup_path).as_ptr()),
                    REPLACE_FILE_FLAGS(0),
                    None,
                    None,
                )
            };
            if let Err(error) = replace {
                // Some ReplaceFile failure states leave the old file at its
                // backup name. Restore only an absent target, never overwrite
                // a file published by another operation.
                if !target.try_exists().unwrap_or(true) && backup_path.is_file()
                {
                    let restored = unsafe {
                        MoveFileExW(
                            PCWSTR(wide(&backup_path).as_ptr()),
                            PCWSTR(wide(target).as_ptr()),
                            MOVEFILE_WRITE_THROUGH,
                        )
                    };
                    if let Err(rollback) = restored {
                        return Err(format!(
                            "Executable replacement failed: {error}; previous executable remains at {}: {rollback}",
                            backup_path.display()
                        ));
                    }
                }
                return Err(format!("Executable replacement failed: {error}"));
            }
        } else {
            unsafe {
                MoveFileExW(
                    PCWSTR(wide(stage).as_ptr()),
                    PCWSTR(wide(target).as_ptr()),
                    MOVEFILE_WRITE_THROUGH,
                )
            }
            .map_err(|error| {
                format!("Could not publish executable: {error}")
            })?;
        }
        let actual = inspect(target)?;
        if &actual != expected {
            return Err("Installed executable failed its final integrity check; previous file was retained".into());
        }
        record(&parent, "Executable replacement completed and verified");
        Ok(())
    })();
    if let Err(error) = &result {
        record(&parent, error);
    }
    result
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn record(parent: &Path, message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(parent.join(".axolotl-install.log"))
    {
        let _ = writeln!(file, "{}: {message}", std::process::id());
        let _ = file.sync_data();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

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
    fn transferred_ownership_outlives_the_original_process() {
        const ROLE: &str = "AXOLOTL_INSTALL_OWNERSHIP_TEST_ROLE";
        const DIRECTORY: &str = "AXOLOTL_INSTALL_OWNERSHIP_TEST_DIRECTORY";
        if let Some(role) = std::env::var_os(ROLE) {
            let directory =
                std::path::PathBuf::from(std::env::var_os(DIRECTORY).unwrap());
            let target = directory.join("app.exe");
            if role == "original" {
                let ownership = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .share_mode(0)
                    .open(directory.join(".axolotl-install.lock"))
                    .unwrap();
                let descriptor =
                    (std::process::id(), ownership.as_raw_handle() as isize);
                fs::write(
                    directory.join("owner.json"),
                    serde_json::to_vec(&descriptor).unwrap(),
                )
                .unwrap();
                wait_for(&directory.join("original-release"));
                drop(ownership);
            } else {
                let (process, handle): (u32, isize) = serde_json::from_slice(
                    &fs::read(directory.join("owner.json")).unwrap(),
                )
                .unwrap();
                let ownership =
                    inherit_ownership(&target, process, handle).unwrap();
                fs::write(directory.join("retained"), b"ready").unwrap();
                wait_for(&directory.join("helper-release"));
                drop(ownership);
            }
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let child = |role: &str| {
            OwnedChild(Command::new(&executable)
            .args(["--exact", "executable_install::tests::transferred_ownership_outlives_the_original_process", "--nocapture"])
            .env(ROLE, role).env(DIRECTORY, directory.path())
            .stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap())
        };
        let mut original = child("original");
        wait_for(&directory.path().join("owner.json"));
        let mut retained = child("retained");
        wait_for(&directory.path().join("retained"));
        original.0.kill().unwrap();
        original.0.wait().unwrap();
        let try_lock = || {
            OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(directory.path().join(".axolotl-install.lock"))
        };
        assert!(
            try_lock().is_err(),
            "The surviving helper must retain exclusive ownership"
        );
        fs::write(directory.path().join("helper-release"), b"finish").unwrap();
        assert!(retained.0.wait().unwrap().success());
        assert!(
            try_lock().is_ok(),
            "The last owner must release ownership on exit"
        );
    }

    #[test]
    fn unrelated_or_invalid_transferred_handles_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("app.exe");
        let unrelated = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join("different.lock"))
            .unwrap();
        assert!(
            inherit_ownership(
                &target,
                std::process::id(),
                unrelated.as_raw_handle() as isize
            )
            .is_err()
        );
        assert!(inherit_ownership(&target, std::process::id(), 0).is_err());
        assert!(inherit_ownership(&target, std::process::id(), -1).is_err());
    }

    fn fixture(path: &Path, marker: u8) {
        let mut bytes = vec![marker; 1024];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[70..72].copy_from_slice(&1_u16.to_le_bytes());
        bytes[84..86].copy_from_slice(&96_u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[200..208].fill(0);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn invalid_or_mismatched_stage_never_changes_installed_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let stage = directory.path().join("next.exe");
        let target = directory.path().join("app.exe");
        fixture(&stage, 2);
        let expected = inspect(&stage).unwrap();
        fixture(&target, 1);
        let original = fs::read(&target).unwrap();
        for bad in [vec![], vec![0; 1024], b"MZ".to_vec()] {
            fs::write(&stage, bad).unwrap();
            assert!(install(&stage, &target, &expected).is_err());
            assert_eq!(fs::read(&target).unwrap(), original);
        }
        fixture(&stage, 3);
        assert!(install(&stage, &target, &expected).is_err());
        assert_eq!(fs::read(&target).unwrap(), original);
    }

    #[test]
    fn atomic_replacement_keeps_a_byte_exact_previous_executable() {
        let directory = tempfile::tempdir().unwrap();
        let stage = directory.path().join("下一版.exe");
        let target = directory.path().join("launcher with spaces.exe");
        fixture(&stage, 2);
        fixture(&target, 1);
        let original = fs::read(&target).unwrap();
        let replacement = fs::read(&stage).unwrap();
        install(&stage, &target, &inspect(&stage).unwrap()).unwrap();
        assert_eq!(fs::read(&target).unwrap(), replacement);
        assert!(!stage.exists());
        let backup = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|entry| {
                entry
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".axolotl-previous-")
            })
            .unwrap();
        assert_eq!(fs::read(backup).unwrap(), original);
    }

    #[test]
    fn locked_destination_is_preserved_and_a_fresh_install_is_verified() {
        let directory = tempfile::tempdir().unwrap();
        let stage = directory.path().join("next.exe");
        let target = directory.path().join("app.exe");
        fixture(&stage, 2);
        fixture(&target, 1);
        let expected = inspect(&stage).unwrap();
        let original = fs::read(&target).unwrap();
        let locked = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&target)
            .unwrap();
        assert!(install(&stage, &target, &expected).is_err());
        drop(locked);
        assert_eq!(fs::read(&target).unwrap(), original);
        fs::remove_file(&target).unwrap();
        install(&stage, &target, &expected).unwrap();
        assert_eq!(inspect(&target).unwrap(), expected);
    }
}
