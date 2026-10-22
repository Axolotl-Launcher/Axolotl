use super::categories;
use crate::resources::{
    ArtifactIntegrity, ExportCategory, ExportDirectorySnapshot,
    ExportFileSnapshot, ExportGameRequirements, ResourceError,
    ResourceExportScan, ResourceResult, SessionId,
};
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tokio_util::sync::CancellationToken;

pub async fn scan(
    instance_id: String,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceExportScan> {
    let state = crate::State::get().await?;
    let metadata = crate::state::get_instance(&instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ResourceError::InvalidRequest("unknown instance".into())
        })?;
    if metadata.instance.is_direct_linked() {
        return Err(ResourceError::Unsupported(
            "directly associated instances cannot be exported".into(),
        ));
    }
    let revision =
        crate::resources::plans::instance_revision(&state.pool, &instance_id)
            .await?;
    let root = crate::api::instance::get_full_path(&instance_id).await?;
    let requirements = ExportGameRequirements {
        game_version: metadata.applied_content_set.game_version,
        loader: (metadata.applied_content_set.loader
            != crate::state::ModLoader::Vanilla)
            .then(|| metadata.applied_content_set.loader.as_str().to_string()),
        loader_version: metadata.applied_content_set.loader_version,
    };
    let mut scan = scan_directory(&root, cancel).await?;
    if crate::resources::plans::instance_revision(&state.pool, &instance_id)
        .await?
        != revision
    {
        return Err(ResourceError::InvalidRequest(
            "instance changed during export scan; retry the scan".into(),
        ));
    }
    scan.instance_id = instance_id;
    scan.instance_revision = revision.try_into().map_err(|_| {
        ResourceError::InvalidRequest("instance revision overflow".into())
    })?;
    scan.requirements = requirements;
    Ok(scan)
}

fn filesystem(path: &Path, error: impl std::fmt::Display) -> ResourceError {
    ResourceError::InvalidRequest(format!(
        "cannot scan {}: {error}",
        path.display()
    ))
}

fn modified(metadata: &std::fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

pub(crate) fn linked(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

async fn scan_directory(
    root: &Path,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceExportScan> {
    let root = root.to_owned();
    let cancel = cancel.clone();
    tokio::task::spawn_blocking(move || scan_directory_blocking(&root, &cancel))
        .await
        .map_err(|error| filesystem(Path::new("export"), error))?
}

fn scan_directory_blocking(
    root: &Path,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceExportScan> {
    use cap_fs_ext::DirExt;
    let root_handle =
        super::files::root(root).map_err(|error| filesystem(root, error))?;
    let mut pending = vec![PathBuf::new()];
    let mut files = vec![];
    let mut directories = vec![];
    let mut excluded = vec![];
    while let Some(relative) = pending.pop() {
        let path = root.join(&relative);
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let unix_directory = if relative.as_os_str().is_empty() {
            String::new()
        } else {
            super::files::relative_path(&relative)
                .map_err(|error| filesystem(&relative, error))?
        };
        let directory = super::files::directory(&root_handle, &unix_directory)
            .map_err(|error| filesystem(&relative, error))?;
        let entries = directory
            .entries()
            .map_err(|error| filesystem(&path, error))?;
        for entry in entries {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            let entry = entry.map_err(|error| filesystem(&path, error))?;
            let entry_relative = relative.join(entry.file_name());
            let unix = super::files::relative_path(&entry_relative)
                .map_err(|error| filesystem(&entry_relative, error))?;
            path_util::SafeRelativeUtf8UnixPathBuf::try_from(unix.clone())
                .map_err(|error| filesystem(&entry_relative, error))?;
            if categories::forbidden(&unix) {
                excluded.push(unix);
                continue;
            }
            let file_type = entry
                .file_type()
                .map_err(|error| filesystem(&entry_relative, error))?;
            if file_type.is_symlink()
                || (!file_type.is_dir() && !file_type.is_file())
            {
                excluded.push(unix);
                continue;
            }
            let metadata = if file_type.is_dir() {
                directory
                    .open_dir_nofollow(entry.file_name())
                    .and_then(|dir| dir.into_std_file().metadata())
            } else {
                super::files::open(
                    &directory,
                    entry.file_name().to_str().ok_or_else(|| {
                        filesystem(&entry_relative, "filename is not UTF-8")
                    })?,
                )
                .and_then(|file| file.metadata())
            }
            .map_err(|error| filesystem(&entry_relative, error))?;
            if metadata.is_dir() {
                directories.push(ExportDirectorySnapshot {
                    relative_path: unix,
                    modified_at: modified(&metadata),
                });
                pending.push(entry_relative);
            } else {
                let (category, eligible) = categories::classify(&unix);
                let identifiable = matches!(
                    unix.rsplit('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase()
                        .as_str(),
                    "jar" | "zip" | "rar" | "disabled" | "old"
                );
                files.push(ExportFileSnapshot {
                    relative_path: unix,
                    integrity: ArtifactIntegrity {
                        size: Some(metadata.len()),
                        hashes: vec![],
                    },
                    revision: 1,
                    identifiable,
                    modified_at: modified(&metadata),
                    category_id: category.into(),
                    category_selected: eligible,
                });
            }
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    directories.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    excluded.sort();
    let mut categories = categories::categories();
    for category in &mut categories {
        if !matches!(
            category.id.as_str(),
            "resourcePacks" | "shaders" | "worlds"
        ) {
            continue;
        }
        let mut objects = std::collections::BTreeSet::new();
        for path in directories
            .iter()
            .map(|d| &d.relative_path)
            .chain(files.iter().map(|f| &f.relative_path))
        {
            let parts = path.split('/').collect::<Vec<_>>();
            if parts.len() >= 2
                && category.paths.iter().any(|prefix| path.starts_with(prefix))
            {
                objects.insert(format!("{}/{}", parts[0], parts[1]));
            }
        }
        for object in objects {
            let id = format!("{}:{object}", category.id);
            let mut modified_at = None;
            for file in &mut files {
                if file.category_id == category.id
                    && (file.relative_path == object
                        || file
                            .relative_path
                            .starts_with(&format!("{object}/")))
                {
                    file.category_id = id.clone();
                    modified_at = modified_at.max(file.modified_at);
                }
            }
            let title = if category.id == "worlds" {
                super::files::open(&root_handle, &format!("{object}/level.dat"))
                    .ok()
                    .and_then(world_name_from_file)
            } else {
                None
            };
            category.children.push(ExportCategory {
                id,
                default_selected: category.default_selected,
                paths: vec![object],
                children: vec![],
                title,
                modified_at,
            });
        }
    }
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    Ok(ResourceExportScan {
        session_id: SessionId::new(uuid::Uuid::new_v4().to_string())?,
        instance_id: String::new(),
        instance_revision: 0,
        files,
        categories,
        excluded_paths: excluded,
        directories,
        requirements: ExportGameRequirements {
            game_version: String::new(),
            loader: None,
            loader_version: None,
        },
    })
}

#[cfg(test)]
fn world_name(path: &Path) -> Option<String> {
    let path = path.join("level.dat");
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if linked(&metadata)
        || !metadata.is_file()
        || metadata.len() > 4 * 1024 * 1024
    {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    world_name_from_file(file)
}

fn world_name_from_file(file: std::fs::File) -> Option<String> {
    use std::io::Read;
    if file.metadata().ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let mut bytes = vec![];
    flate2::read::GzDecoder::new(file)
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut std::io::Cursor::new(bytes),
        quartz_nbt::io::Flavor::Uncompressed,
    )
    .ok()?;
    let data = root.get::<_, &quartz_nbt::NbtCompound>("Data").ok()?;
    let name = data.get::<_, &str>("LevelName").ok()?.trim();
    (!name.is_empty()).then(|| name.into())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[tokio::test]
    async fn literal_backslashes_never_alias_nested_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a\\b.jar"), b"literal").unwrap();
        std::fs::create_dir(root.path().join("a")).unwrap();
        std::fs::write(root.path().join("a/b.jar"), b"nested").unwrap();
        assert!(
            scan_directory(root.path(), &CancellationToken::new())
                .await
                .is_err()
        );
    }

    use super::*;

    #[tokio::test]
    async fn world_display_name_comes_from_bounded_local_nbt() {
        use std::io::Write;
        let root = tempfile::TempDir::new().unwrap();
        let world = root.path().join("saves/folder");
        std::fs::create_dir_all(&world).unwrap();
        let mut data = quartz_nbt::NbtCompound::new();
        data.insert("LevelName", "我的存档");
        let mut nbt = quartz_nbt::NbtCompound::new();
        nbt.insert("Data", data);
        let mut bytes = vec![];
        quartz_nbt::io::write_nbt(
            &mut bytes,
            None,
            &nbt,
            quartz_nbt::io::Flavor::Uncompressed,
        )
        .unwrap();
        let mut gzip = flate2::write::GzEncoder::new(
            std::fs::File::create(world.join("level.dat")).unwrap(),
            flate2::Compression::default(),
        );
        gzip.write_all(&bytes).unwrap();
        gzip.finish().unwrap();
        let scan = scan_directory(root.path(), &CancellationToken::new())
            .await
            .unwrap();
        let category = &scan
            .categories
            .iter()
            .find(|c| c.id == "worlds")
            .unwrap()
            .children[0];
        assert_eq!(category.title.as_deref(), Some("我的存档"));
        assert!(category.modified_at.is_some());
    }

    #[tokio::test]
    async fn failed_directory_read_is_an_error_instead_of_a_partial_scan() {
        let root = tempfile::TempDir::new().unwrap();
        assert!(
            scan_directory(
                &root.path().join("missing"),
                &CancellationToken::new()
            )
            .await
            .is_err()
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn junctions_cannot_escape_or_cycle() {
        let root = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret"), b"secret").unwrap();
        let link = root.path().join("outside");
        junction::create(outside.path(), &link).unwrap();
        let scan = scan_directory(root.path(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(scan.excluded_paths, vec!["outside"]);
        assert!(scan.files.is_empty());
        junction::delete(&link).unwrap();
        assert!(outside.path().join("secret").exists());
    }

    #[tokio::test]
    async fn complete_scan_keeps_worlds_packs_and_shader_settings_independent()
    {
        let root = tempfile::TempDir::new().unwrap();
        for path in [
            "mods/deep/a.jar",
            "mods/deep/b.jar.disabled",
            "resourcepacks/folder/assets/a",
            "resourcepacks/pack.zip",
            "shaderpacks/a.zip",
            "shaderpacks/a.zip.txt",
            "datapacks/root.zip",
            "saves/world/datapacks/world.zip",
            "config/jei/world/a",
            "assets/excluded/a",
            "essential/microsoft_accounts.json",
            "unknown/deep/file",
        ] {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"data").unwrap();
        }
        std::fs::create_dir_all(root.path().join("empty/deep")).unwrap();
        let scan = scan_directory(root.path(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(scan.files.len(), 10);
        assert!(
            scan.files
                .iter()
                .any(|f| f.relative_path == "mods/deep/a.jar"
                    && f.integrity.size == Some(4))
        );
        assert!(
            scan.directories
                .iter()
                .any(|d| d.relative_path == "empty/deep")
        );
        assert!(scan.excluded_paths.contains(&"assets".into()));
        assert!(
            scan.files
                .iter()
                .any(|f| f.category_id == "worlds:saves/world")
        );
        assert_eq!(
            scan.categories
                .iter()
                .find(|c| c.id == "shaders")
                .unwrap()
                .children
                .len(),
            2
        );
        assert_eq!(
            scan.categories
                .iter()
                .find(|c| c.id == "resourcePacks")
                .unwrap()
                .children
                .len(),
            2
        );
        assert!(
            scan.files
                .iter()
                .any(|f| f.relative_path == "resourcepacks/folder/assets/a")
        );
    }

    #[tokio::test]
    async fn cancellation_never_returns_an_incomplete_snapshot() {
        let root = tempfile::TempDir::new().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            scan_directory(root.path(), &cancel).await,
            Err(ResourceError::Canceled)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_directories_cannot_escape_or_cycle() {
        let root = tempfile::TempDir::new().unwrap();
        std::os::unix::fs::symlink(root.path(), root.path().join("loop"))
            .unwrap();
        let scan = scan_directory(root.path(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(scan.excluded_paths, vec!["loop"]);
        assert!(scan.files.is_empty());
    }
}
