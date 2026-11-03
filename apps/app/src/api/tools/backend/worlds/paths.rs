use super::super::business_error;
use launcher_tools::ToolError;
use std::path::{Path, PathBuf};

fn is_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

async fn no_link(path: &Path) -> Result<(), ToolError> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if is_link(&metadata) => Err(ToolError::new(
            "linked_world_path",
            "World operations cannot traverse linked files or directories",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ToolError::new(
            "world_path_unavailable",
            "Unable to inspect world path",
        )),
    }
}

async fn metadata_targets(
    root: &Path,
    relative: &str,
) -> Result<(), ToolError> {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        no_link(&path).await?;
    }
    for suffix in ["_old", ".axolotl-tmp"] {
        let mut candidate = path.as_os_str().to_owned();
        candidate.push(suffix);
        no_link(Path::new(&candidate)).await?;
    }
    Ok(())
}

pub(super) async fn contained(
    root: &Path,
    id: &str,
) -> Result<PathBuf, ToolError> {
    if !launcher_tools::world_input::valid_world_id(id) {
        return Err(ToolError::new(
            "invalid_world_id",
            "Use a save-folder name returned by world_list",
        ));
    }
    let root = tokio::fs::canonicalize(root).await.map_err(|_| {
        ToolError::new("not_found", "Instance game directory not found")
    })?;
    let saves = root.join("saves");
    no_link(&saves).await?;
    let world = saves.join(id);
    no_link(&world).await?;
    let canonical = tokio::fs::canonicalize(&world)
        .await
        .map_err(|_| ToolError::new("not_found", "World not found"))?;
    if canonical.parent() != Some(saves.as_path()) {
        return Err(ToolError::new(
            "invalid_world_path",
            "World is outside this instance's saves directory",
        ));
    }
    // Native modern-world editing can write sibling data/player files as well
    // as level.dat. Check their parents and backup/temp targets before use.
    for relative in [
        "level.dat",
        "session.lock",
        "icon.png",
        "data/minecraft/game_rules.dat",
        "data/minecraft/world_gen_settings.dat",
    ] {
        metadata_targets(&canonical, relative).await?;
    }
    let players = canonical.join("players");
    no_link(&players).await?;
    let data = players.join("data");
    no_link(&data).await?;
    let mut entries = match tokio::fs::read_dir(&data).await {
        Ok(entries) => Some(entries),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            return Err(ToolError::new(
                "world_path_unavailable",
                "Cannot inspect player metadata",
            ));
        }
    };
    if let Some(entries) = entries.as_mut() {
        while let Some(entry) = entries.next_entry().await.map_err(|_| {
            ToolError::new(
                "world_path_unavailable",
                "Cannot inspect player metadata",
            )
        })? {
            if entry.file_name().to_string_lossy().ends_with(".dat") {
                metadata_targets(&data, &entry.file_name().to_string_lossy())
                    .await?;
            }
        }
    }
    Ok(root)
}

pub(super) async fn world(
    instance_id: &str,
    id: &str,
) -> Result<PathBuf, ToolError> {
    let root = theseus::instance::get_full_path(instance_id)
        .await
        .map_err(business_error)?;
    contained(&root, id).await
}

pub(super) async fn server_list(instance_id: &str) -> Result<(), ToolError> {
    let root = theseus::instance::get_full_path(instance_id)
        .await
        .map_err(business_error)?;
    no_link(&root.join("servers.dat")).await
}

pub(super) async fn backup_destination(root: &Path) -> Result<(), ToolError> {
    let directory = root.join("backups");
    no_link(&directory).await?;
    match tokio::fs::read_dir(&directory).await {
        Ok(mut entries) => {
            while let Some(entry) =
                entries.next_entry().await.map_err(|_| {
                    ToolError::new(
                        "backup_path_unavailable",
                        "Cannot inspect backup directory",
                    )
                })?
            {
                no_link(&entry.path()).await?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ToolError::new(
            "backup_path_unavailable",
            "Cannot inspect backup directory",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn world_paths_must_be_one_existing_child_of_saves() {
        let root = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(root.path().join("saves").join("世界"))
            .await
            .unwrap();
        assert!(contained(root.path(), "世界").await.is_ok());
        assert_eq!(
            contained(root.path(), "../outside").await.unwrap_err().code,
            "invalid_world_id"
        );
        assert_eq!(
            contained(root.path(), "absent").await.unwrap_err().code,
            "not_found"
        );
    }
}
