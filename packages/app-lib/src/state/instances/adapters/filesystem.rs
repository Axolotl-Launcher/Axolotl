use crate::state::ProjectType;
use crate::util::io::IOError;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct ScannedContentFile {
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hash_cache_key: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ScannedBackupFile {
    pub relative_path: String,
    pub file_name: String,
    /// Seconds since the UNIX epoch; used to pick the oldest backup when
    /// several updates of the same file have accumulated.
    pub modified: i64,
}

pub(crate) fn scan_content_files(
    instances_dir: &Path,
    instance_path: &str,
) -> crate::Result<Vec<ScannedContentFile>> {
    let instance_dir =
        crate::util::io::canonicalize(instances_dir.join(instance_path))?;
    scan_content_files_from(&instance_dir, instance_path)
}

/// Variant of [`scan_content_files`] that takes an already-resolved game
/// directory, so callers can pass a `game_dir_override` target as-is.
/// `cache_key_path` is the *relative* instance path used as the hash-cache-key
/// prefix (it must stay the launcher's relative `instance.path`, not the
/// resolved override path, so content hashes remain stable across dirs).
pub(crate) fn scan_content_files_from(
    instance_dir: &Path,
    cache_key_path: &str,
) -> crate::Result<Vec<ScannedContentFile>> {
    let mut files = Vec::new();

    for_each_content_folder(
        instance_dir,
        |folder_path, relative_dir, project_type| {
            scan_content_folder(
                folder_path,
                relative_dir,
                project_type,
                cache_key_path,
                &mut files,
            )
        },
    )?;

    Ok(files)
}

/// Collects update backup files (`*.old`) across every content folder. Backups
/// are never hashed or listed as content; they are matched back to their
/// active file by the `{active}_{previous}.old` naming convention.
#[cfg(test)]
pub(crate) fn scan_content_backups(
    instances_dir: &Path,
    instance_path: &str,
) -> crate::Result<Vec<ScannedBackupFile>> {
    scan_content_backups_from(&crate::util::io::canonicalize(
        instances_dir.join(instance_path),
    )?)
}

/// Variant of [`scan_content_backups`] that takes an already-resolved game
/// directory, so callers can pass a `game_dir_override` target as-is.
pub(crate) fn scan_content_backups_from(
    instance_dir: &Path,
) -> crate::Result<Vec<ScannedBackupFile>> {
    let mut backups = Vec::new();

    for_each_content_folder(instance_dir, |folder_path, relative_dir, _| {
        for entry in std::fs::read_dir(folder_path)
            .map_err(|err| IOError::with_path(err, folder_path))?
        {
            let path = entry.map_err(IOError::from)?.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_name) =
                path.file_name().and_then(|value| value.to_str())
            else {
                continue;
            };
            if !file_name.ends_with(".old") {
                continue;
            }
            let modified = path
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or_default();
            backups.push(ScannedBackupFile {
                relative_path: format!("{relative_dir}/{file_name}"),
                file_name: file_name.to_string(),
                modified,
            });
        }
        Ok(())
    })?;

    Ok(backups)
}

/// Walks every content folder (and, for schematics, nested subfolders) and
/// invokes `visit` for each folder that may hold project files.
fn for_each_content_folder(
    instance_dir: &Path,
    mut visit: impl FnMut(&Path, &str, ProjectType) -> crate::Result<()>,
) -> crate::Result<()> {
    for project_type in ProjectType::iterator() {
        let folder = project_type.get_folder();
        let folder_path = instance_dir.join(folder);

        if !folder_path.exists() {
            continue;
        }

        walk_content_folder(&folder_path, folder, project_type, &mut visit)?;
    }

    let saves = instance_dir.join("saves");
    if saves.is_dir() {
        for world in std::fs::read_dir(&saves)
            .map_err(|error| IOError::with_path(error, &saves))?
        {
            let world = world.map_err(IOError::from)?;
            if !world.file_type().map_err(IOError::from)?.is_dir() {
                continue;
            }
            let Some(name) = world.file_name().to_str().map(str::to_owned)
            else {
                continue;
            };
            let datapacks = world.path().join("datapacks");
            if datapacks.is_dir() {
                visit(
                    &datapacks,
                    &format!("saves/{name}/datapacks"),
                    ProjectType::DataPack,
                )?;
            }
        }
    }

    Ok(())
}

fn walk_content_folder(
    folder_path: &Path,
    relative_dir: &str,
    project_type: ProjectType,
    visit: &mut impl FnMut(&Path, &str, ProjectType) -> crate::Result<()>,
) -> crate::Result<()> {
    visit(folder_path, relative_dir, project_type)?;

    for entry in std::fs::read_dir(folder_path)
        .map_err(|err| IOError::with_path(err, folder_path))?
    {
        let path = entry.map_err(IOError::from)?.path();
        if path.is_dir() {
            // Only schematics may live in nested folders; other content
            // folders are scanned at the top level only.
            if project_type == ProjectType::Schematic {
                let Some(dir_name) =
                    path.file_name().and_then(|value| value.to_str())
                else {
                    continue;
                };
                walk_content_folder(
                    &path,
                    &format!("{relative_dir}/{dir_name}"),
                    project_type,
                    visit,
                )?;
            }
        }
    }

    Ok(())
}

fn scan_content_folder(
    folder_path: &Path,
    relative_dir: &str,
    project_type: ProjectType,
    instance_path: &str,
    files: &mut Vec<ScannedContentFile>,
) -> crate::Result<()> {
    for entry in std::fs::read_dir(folder_path)
        .map_err(|err| IOError::with_path(err, folder_path))?
    {
        let path = entry.map_err(IOError::from)?.path();
        if path.is_dir() || !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str())
        else {
            continue;
        };

        if !is_scannable_project_path(project_type, file_name) {
            continue;
        }

        let metadata = path.metadata().map_err(IOError::from)?;
        let size = metadata.len();
        let relative_path = format!("{relative_dir}/{file_name}");
        let hash_cache_key = format!("{size}-{instance_path}/{relative_path}");

        files.push(ScannedContentFile {
            relative_path,
            file_name: file_name.to_string(),
            enabled: !file_name.ends_with(".disabled"),
            size,
            modified: metadata.modified().ok(),
            hash_cache_key,
        });
    }

    Ok(())
}

pub(crate) fn project_type_from_relative_path(
    relative_path: &str,
) -> Option<ProjectType> {
    let mut current = Path::new(relative_path).parent();
    while let Some(parent) = current {
        let folder_name = parent
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if let Some(project_type) = ProjectType::from_folder_name(folder_name) {
            return Some(project_type);
        }
        current = parent.parent();
    }
    None
}

pub(crate) fn is_scannable_project_path(
    project_type: ProjectType,
    relative_path: &str,
) -> bool {
    let Some(extension) =
        Path::new(relative_path.trim_end_matches(".disabled"))
            .extension()
            .and_then(|ext| ext.to_str())
    else {
        return false;
    };

    match project_type {
        ProjectType::Mod => extension.eq_ignore_ascii_case("jar"),
        ProjectType::DataPack
        | ProjectType::ResourcePack
        | ProjectType::ShaderPack => extension.eq_ignore_ascii_case("zip"),
        ProjectType::Schematic => {
            extension.eq_ignore_ascii_case("litematic")
                || extension.eq_ignore_ascii_case("schematic")
                || extension.eq_ignore_ascii_case("schem")
        }
        // WorldSave folders (saves/) are handled separately via worlds.rs,
        // not scanned as regular project files.
        ProjectType::WorldSave => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_world_datapack_archives_without_recursing_through_world_data() {
        let root = tempdir().unwrap();
        let packs = root.path().join("saves/World One/datapacks");
        fs::create_dir_all(packs.join("folder-pack/data")).unwrap();
        fs::create_dir_all(root.path().join("saves/World One/region")).unwrap();
        fs::write(packs.join("one.zip"), b"one").unwrap();
        fs::write(packs.join("two.zip.disabled"), b"two").unwrap();
        fs::write(packs.join("ignored.txt"), b"text").unwrap();
        fs::write(packs.join("folder-pack/data/ignored.zip"), b"nested")
            .unwrap();
        fs::write(
            root.path().join("saves/World One/region/ignored.zip"),
            b"region",
        )
        .unwrap();
        let files = scan_content_files_from(root.path(), "").unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|file| file.relative_path
            == "saves/World One/datapacks/one.zip"
            && file.enabled));
        assert!(files.iter().any(|file| file.relative_path
            == "saves/World One/datapacks/two.zip.disabled"
            && !file.enabled));
    }

    #[test]
    fn scan_content_files_finds_nested_schematics() {
        let root = tempdir().unwrap();
        let instance_dir = root.path().join("inst");
        let schematics = instance_dir.join("schematics");
        fs::create_dir_all(schematics.join("redstone/contraptions")).unwrap();
        fs::create_dir_all(instance_dir.join("mods")).unwrap();
        fs::write(schematics.join("house.litematic"), "a").unwrap();
        fs::write(schematics.join("redstone/clock.litematic"), "b").unwrap();
        fs::write(schematics.join("redstone/contraptions/gear.schem"), "c")
            .unwrap();
        fs::write(schematics.join("redstone/notes.txt"), "d").unwrap();
        fs::write(instance_dir.join("mods/example.jar"), "e").unwrap();

        let files = scan_content_files(root.path(), "inst").unwrap();

        let paths: Vec<&str> = files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"schematics/house.litematic"));
        assert!(paths.contains(&"schematics/redstone/clock.litematic"));
        assert!(paths.contains(&"schematics/redstone/contraptions/gear.schem"));
        assert!(!paths.contains(&"schematics/redstone/notes.txt"));
        assert!(paths.contains(&"mods/example.jar"));
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn project_type_from_relative_path_matches_nested_folders() {
        assert_eq!(
            project_type_from_relative_path("schematics/house.litematic"),
            Some(ProjectType::Schematic)
        );
        assert_eq!(
            project_type_from_relative_path(
                "schematics/redstone/clock.litematic"
            ),
            Some(ProjectType::Schematic)
        );
        assert_eq!(
            project_type_from_relative_path("schematics/a/b/c/tower.litematic"),
            Some(ProjectType::Schematic)
        );
        assert_eq!(
            project_type_from_relative_path("mods/example.jar"),
            Some(ProjectType::Mod)
        );
        assert_eq!(
            project_type_from_relative_path("config/example.json"),
            None
        );
    }

    #[test]
    fn scannable_project_paths_exclude_shader_configuration_sidecars() {
        assert!(is_scannable_project_path(
            ProjectType::ShaderPack,
            "shaderpacks/BSL_v10.1.3.zip"
        ));
        assert!(!is_scannable_project_path(
            ProjectType::ShaderPack,
            "shaderpacks/BSL_v10.1.3.zip.txt"
        ));
        assert!(!is_scannable_project_path(
            ProjectType::ShaderPack,
            "shaderpacks/ComplementaryReimagined_r5.3.txt"
        ));
    }

    #[test]
    fn scan_content_backups_finds_only_backup_files() {
        let root = tempdir().unwrap();
        let instance_dir = root.path().join("inst");
        fs::create_dir_all(instance_dir.join("schematics/redstone")).unwrap();
        fs::create_dir_all(instance_dir.join("mods")).unwrap();
        fs::write(instance_dir.join("mods/Mod-2.jar"), "new").unwrap();
        fs::write(instance_dir.join("mods/Mod-2.jar_Mod-1.jar.old"), "old")
            .unwrap();
        fs::write(
            instance_dir
                .join("schematics/redstone/New.litematic_Old.litematic.old"),
            "x",
        )
        .unwrap();
        fs::write(
            instance_dir.join("schematics/redstone/house.litematic"),
            "y",
        )
        .unwrap();

        let backups = scan_content_backups(root.path(), "inst").unwrap();
        let mut paths = backups
            .iter()
            .map(|backup| backup.relative_path.as_str())
            .collect::<Vec<_>>();
        paths.sort_unstable();

        assert_eq!(
            paths,
            vec![
                "mods/Mod-2.jar_Mod-1.jar.old",
                "schematics/redstone/New.litematic_Old.litematic.old",
            ]
        );
    }
}
