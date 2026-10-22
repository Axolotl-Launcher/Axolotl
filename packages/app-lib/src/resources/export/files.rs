use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::path::{Component, Path};

pub fn relative_path(path: &Path) -> std::io::Result<String> {
    path.components()
        .map(|component| match component {
            Component::Normal(name) => name
                .to_str()
                .filter(|name| !name.contains('\\'))
                .map(str::to_owned)
                .ok_or_else(|| {
                    std::io::Error::other(
                        "filename cannot be represented in an export",
                    )
                }),
            _ => Err(std::io::Error::other("invalid export path component")),
        })
        .collect::<std::io::Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

pub fn root(path: &Path) -> std::io::Result<Dir> {
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
}

pub fn directory(root: &Dir, relative: &str) -> std::io::Result<Dir> {
    let mut current = root.try_clone()?;
    if relative.is_empty() {
        return Ok(current);
    }
    let safe =
        path_util::SafeRelativeUtf8UnixPathBuf::try_from(relative.to_owned())
            .map_err(std::io::Error::other)?;
    for segment in safe.as_str().split('/') {
        current = current.open_dir_nofollow(segment)?;
    }
    Ok(current)
}

pub fn open(root: &Dir, relative: &str) -> std::io::Result<std::fs::File> {
    let safe =
        path_util::SafeRelativeUtf8UnixPathBuf::try_from(relative.to_owned())
            .map_err(std::io::Error::other)?;
    let (parent, name) = safe
        .as_str()
        .rsplit_once('/')
        .unwrap_or(("", safe.as_str()));
    let directory = directory(root, parent)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = directory.open_with(name, &options)?.into_std();
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("export path is not a regular file"));
    }
    Ok(file)
}

pub async fn open_async(
    root: &Path,
    relative: &str,
) -> std::io::Result<tokio::fs::File> {
    let root = root.to_owned();
    let relative = relative.to_owned();
    tokio::task::spawn_blocking(move || open(&self::root(&root)?, &relative))
        .await
        .map_err(std::io::Error::other)?
        .map(tokio::fs::File::from_std)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opened_handles_keep_the_original_directory_after_parent_replacement() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("mods")).unwrap();
        std::fs::write(temp.path().join("mods/a.jar"), b"inside").unwrap();
        let dir = root(temp.path()).unwrap();
        let child = directory(&dir, "mods").unwrap();
        let rename = std::fs::rename(
            temp.path().join("mods"),
            temp.path().join("original"),
        );
        if cfg!(windows) && rename.is_err() {
            assert!(temp.path().join("mods/a.jar").exists());
            return;
        }
        rename.unwrap();
        std::fs::create_dir(temp.path().join("mods")).unwrap();
        std::fs::write(temp.path().join("mods/a.jar"), b"replacement").unwrap();
        use std::io::Read;
        let mut bytes = Vec::new();
        open(&child, "a.jar")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"inside");
    }

    #[cfg(unix)]
    #[test]
    fn parent_and_final_symlinks_never_escape_the_root() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("a.jar"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("mods"))
            .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("a.jar"),
            temp.path().join("a.jar"),
        )
        .unwrap();
        let root = root(temp.path()).unwrap();
        assert!(open(&root, "mods/a.jar").is_err());
        assert!(open(&root, "a.jar").is_err());
    }
}
