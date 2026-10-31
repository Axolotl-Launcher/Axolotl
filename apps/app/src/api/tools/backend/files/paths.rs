use super::super::{business_error, reviewed, servers};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use launcher_tools::ToolError;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) fn error(_: impl std::fmt::Display) -> ToolError {
    ToolError::new(
        "file_operation_failed",
        "File is unavailable, linked, outside the target, too large or changed; refresh metadata and retry",
    )
}
pub(in super::super) fn valid(
    path: &str,
    root_allowed: bool,
) -> Result<(), ToolError> {
    if root_allowed && path.is_empty() {
        return Ok(());
    }
    if path.is_empty()
        || path.len() > 1024
        || path.contains(['\\', ':', '*', '?', '"', '<', '>', '|'])
        || path.chars().any(char::is_control)
        || path.split('/').any(|s| {
            let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
            s.is_empty()
                || matches!(s, "." | "..")
                || s.ends_with([' ', '.'])
                || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
    {
        return Err(ToolError::new(
            "invalid_path",
            "Use a relative slash-separated path without links or parent traversal",
        ));
    }
    Ok(())
}
pub(in super::super) fn directory(
    root: &Dir,
    relative: &str,
) -> Result<Dir, ToolError> {
    valid(relative, true)?;
    let mut current = root.try_clone().map_err(error)?;
    for segment in relative.split('/').filter(|s| !s.is_empty()) {
        current = current.open_dir_nofollow(segment).map_err(error)?;
    }
    Ok(current)
}
pub(in super::super) fn parent(
    root: &Dir,
    relative: &str,
) -> Result<(Dir, String), ToolError> {
    valid(relative, false)?;
    let (dir, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    Ok((directory(root, dir)?, name.into()))
}
pub(in super::super) fn read(
    root: &Dir,
    relative: &str,
    max: u64,
) -> Result<Vec<u8>, ToolError> {
    let (dir, name) = parent(root, relative)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir.open_with(name, &options).map_err(error)?;
    let metadata = file.metadata().map_err(error)?;
    if !metadata.is_file() || metadata.len() > max {
        return Err(error("size"));
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > max {
        return Err(error("size"));
    }
    Ok(bytes)
}
pub(in super::super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(in super::super) fn check(
    bytes: &[u8],
    expected: &str,
) -> Result<(), ToolError> {
    if !hash(bytes).eq_ignore_ascii_case(expected) {
        return Err(ToolError::new(
            "file_changed",
            "Source hash changed; read and review again",
        ));
    }
    Ok(())
}
pub(in super::super) fn write(
    root: &Dir,
    path: &str,
    bytes: &[u8],
    expected: Option<&str>,
    create: bool,
) -> Result<(), ToolError> {
    let (dir, name) = parent(root, path)?;
    if create {
        if expected.is_some() {
            return Err(ToolError::new(
                "invalid_write",
                "New file cannot have expected_sha256",
            ));
        }
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = dir.open_with(&name, &options).map_err(error)?;
        if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = dir.remove_file(&name);
            return Err(error(e));
        }
    } else {
        let expected = expected.ok_or_else(|| {
            ToolError::new(
                "hash_required",
                "Existing files require expected_sha256",
            )
        })?;
        check(&read(root, path, 2 * 1024 * 1024)?, expected)?;
        let temp = format!(".mcp-{}.tmp", uuid::Uuid::new_v4());
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = dir.open_with(&temp, &options).map_err(error)?;
        let result = (|| {
            file.write_all(bytes).map_err(error)?;
            file.sync_all().map_err(error)?;
            drop(file);
            check(&read(root, path, 2 * 1024 * 1024)?, expected)?;
            dir.rename(&temp, &dir, &name).map_err(error)
        })();
        if result.is_err() {
            let _ = dir.remove_file(&temp);
        }
        result?;
    }
    Ok(())
}
pub(in super::super) async fn root(
    kind: &str,
    id: &str,
    write: bool,
) -> Result<PathBuf, ToolError> {
    let path = match kind {
        "instance" => {
            let metadata = if write {
                reviewed::instance(id).await?
            } else {
                theseus::instance::get(id)
                    .await
                    .map_err(business_error)?
                    .ok_or_else(|| {
                        ToolError::new("not_found", "Instance not found")
                    })?
            };
            if metadata.instance.is_direct_linked()
                || metadata.instance.symlink_target.is_some()
                || metadata.instance.game_dir_override.is_some()
            {
                return Err(ToolError::new(
                    "external_instance_layout",
                    "File tools require an ordinary launcher-owned target",
                ));
            }
            valid(&metadata.instance.path, false)?;
            if metadata.instance.path.contains('/') {
                return Err(error("instance root"));
            }
            theseus::instance::get_full_path(id)
                .await
                .map_err(business_error)?
        }
        "server" => {
            let info = servers::info(id).await?;
            if write {
                servers::require_stopped(&info)?;
            }
            PathBuf::from(info.path)
        }
        _ => {
            return Err(ToolError::new(
                "invalid_target",
                "Use instance or server",
            ));
        }
    };
    let metadata = tokio::fs::symlink_metadata(&path).await.map_err(error)?;
    if !metadata.is_dir() || reviewed::linked(&metadata) {
        return Err(error("linked root"));
    }
    Ok(path)
}
pub(in super::super) fn open_root(path: &Path) -> Result<Dir, ToolError> {
    let parent = path.parent().ok_or_else(|| error("root parent"))?;
    let name = path.file_name().ok_or_else(|| error("root name"))?;
    Dir::open_ambient_dir(parent, cap_std::ambient_authority())
        .map_err(error)?
        .open_dir_nofollow(name)
        .map_err(error)
}

pub(in super::super) struct Payload {
    _directory: tempfile::TempDir,
    path: PathBuf,
}
impl Payload {
    pub(in super::super) fn path(&self) -> &Path {
        &self.path
    }
}

pub(in super::super) async fn payload(
    instance: &str,
    path: &str,
    expected: &str,
) -> Result<Payload, ToolError> {
    let root = root("instance", instance, true).await?;
    let path = path.to_owned();
    let expected = expected.to_owned();
    tokio::task::spawn_blocking(move || {
        let bytes = read(&open_root(&root)?, &path, 64 * 1024 * 1024)?;
        check(&bytes, &expected)?;
        let name = path.rsplit('/').next().ok_or_else(|| error("name"))?;
        let directory = tempfile::tempdir().map_err(error)?;
        let destination = directory.path().join(name);
        std::fs::write(&destination, bytes).map_err(error)?;
        Ok(Payload {
            _directory: directory,
            path: destination,
        })
    })
    .await
    .map_err(error)?
}
