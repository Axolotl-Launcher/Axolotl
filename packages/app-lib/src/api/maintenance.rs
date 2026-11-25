//! Launcher maintenance entry points; callers do not name a database backend.
pub use crate::state::store::maintenance::IntegrityReport;
pub async fn check_database_integrity() -> crate::Result<IntegrityReport> {
    let state = crate::State::get().await?;
    crate::state::store::maintenance::check(&state.data).await
}

/// Actual opened database, rather than the channel selected for next startup.
pub async fn current_database_path() -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    Ok(crate::state::store::maintenance::database_path(&state.data))
}

pub async fn backup_open_database(
    version: &str,
) -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    let path = crate::state::store::maintenance::database_path(&state.data);
    if path.file_name().is_some_and(|name| name == "system.db") {
        crate::state::store::databases::backup(
            &state.pool,
            &state.directories.settings_dir,
        )
        .await
    } else {
        crate::state::db_backup::backup_app_db_for_update(&path, version).await
    }
}

#[derive(serde::Serialize)]
pub struct DatabaseMember {
    pub name: String,
    pub role: &'static str,
    pub bytes: u64,
}
#[derive(serde::Serialize)]
pub struct DatabaseInfo {
    pub layout: &'static str,
    pub channel: Option<String>,
    pub bytes: u64,
    pub databases: Vec<DatabaseMember>,
}

/// Current database authorities and optional stores, excluding retired import
/// sources. Sizes include existing SQLite sidecars and contain no native paths.
pub async fn database_info() -> crate::Result<DatabaseInfo> {
    database_info_at(&current_database_path().await?).await
}

async fn database_info_at(
    path: &std::path::Path,
) -> crate::Result<DatabaseInfo> {
    let shared = path.file_name().is_some_and(|name| name == "system.db");
    let directory = path.parent().ok_or_else(|| {
        crate::ErrorKind::InputError("Database has no directory".into())
    })?;
    let mut databases = Vec::new();
    let members = if shared {
        vec![
            ("system.db", "core"),
            ("instances.db", "core"),
            ("storage.db", "core"),
            ("records.db", "history"),
            ("operations-history.db", "history"),
            ("derived.db", "derived"),
            ("resource-cache-release.db", "derived"),
            ("resource-cache-beta.db", "derived"),
        ]
    } else {
        vec![("app.db", "legacy")]
    };
    for (name, role) in members {
        let file = directory.join(name);
        let mut bytes = match tokio::fs::symlink_metadata(&file).await {
            Ok(metadata) if metadata.is_file() => metadata.len(),
            Ok(_) => {
                return Err(crate::ErrorKind::InputError(
                    "Database is not a regular file".into(),
                )
                .into());
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && role != "core"
                    && shared =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        for suffix in ["-wal", "-shm", "-journal"] {
            match tokio::fs::symlink_metadata(
                directory.join(format!("{name}{suffix}")),
            )
            .await
            {
                Ok(metadata) if metadata.is_file() => bytes += metadata.len(),
                Ok(_) => {
                    return Err(crate::ErrorKind::InputError(
                        "Database sidecar is not a regular file".into(),
                    )
                    .into());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        databases.push(DatabaseMember {
            name: name.into(),
            role,
            bytes,
        });
    }
    Ok(DatabaseInfo {
        layout: if shared { "shared" } else { "legacy_channel" },
        channel: if shared {
            None
        } else {
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        },
        bytes: databases.iter().map(|member| member.bytes).sum(),
        databases,
    })
}

#[derive(serde::Serialize)]
pub struct DatabaseBackupInfo {
    pub layout: &'static str,
    pub channel: Option<String>,
    pub backup_name: String,
    pub bytes: u64,
    pub modified_at: Option<std::time::SystemTime>,
}

/// Measure only a complete native snapshot or a legacy backup file. Directory
/// lengths are not byte sizes, and staging bundles are never valid backups.
pub async fn database_backup_info(
    path: &std::path::Path,
) -> crate::Result<Option<DatabaseBackupInfo>> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let metadata = tokio::fs::symlink_metadata(path).await?;
    let (layout, channel, bytes) = if metadata.is_file()
        && name.starts_with("app-db-")
        && name.ends_with(".db")
    {
        (
            "legacy_channel",
            path.parent()
                .and_then(std::path::Path::file_name)
                .and_then(|name| name.to_str())
                .filter(|name| matches!(*name, "release" | "beta"))
                .map(str::to_owned),
            metadata.len(),
        )
    } else if metadata.is_dir() && uuid::Uuid::parse_str(&name).is_ok() {
        if !crate::state::store::maintenance::complete_bundle(path).await? {
            return Ok(None);
        }
        ("shared", None, snapshot_bytes(path).await?)
    } else {
        return Ok(None);
    };
    Ok(Some(DatabaseBackupInfo {
        layout,
        channel,
        backup_name: name,
        bytes,
        modified_at: metadata.modified().ok(),
    }))
}

async fn snapshot_bytes(path: &std::path::Path) -> crate::Result<u64> {
    let mut directories = vec![path.to_owned()];
    let mut bytes = 0;
    while let Some(directory) = directories.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = tokio::fs::symlink_metadata(entry.path()).await?;
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                bytes += metadata.len();
            } else {
                return Err(crate::ErrorKind::InputError(
                    "Snapshot contains a linked or nonregular member".into(),
                )
                .into());
            }
        }
    }
    Ok(bytes)
}

pub async fn list_database_backups() -> crate::Result<Vec<DatabaseBackupInfo>> {
    let path = current_database_path().await?;
    let root =
        path.parent()
            .and_then(std::path::Path::parent)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Database has no settings root".into(),
                )
            })?;
    let bases = match std::env::var_os("THESEUS_DB_BACKUP_DIR") {
        Some(base) => vec![std::path::PathBuf::from(base)],
        None => {
            vec![root.join("Backups/databases"), root.join("Backups/app-db")]
        }
    };
    list_database_backups_at(&bases).await
}

async fn list_database_backups_at(
    bases: &[std::path::PathBuf],
) -> crate::Result<Vec<DatabaseBackupInfo>> {
    let mut output = Vec::new();
    for base in bases {
        for channel in ["", "release", "beta"] {
            let mut entries = match tokio::fs::read_dir(base.join(channel))
                .await
            {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            while let Some(entry) = entries.next_entry().await? {
                match database_backup_info(&entry.path()).await {
                    Ok(Some(info)) => output.push(info),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(%error, backup = %entry.file_name().to_string_lossy(), "Skipping unavailable database backup");
                    }
                }
            }
        }
    }
    output.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| b.backup_name.cmp(&a.backup_name))
    });
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn shared_info_counts_core_history_and_sidecars_without_legacy_sources()
     {
        let root = tempfile::tempdir().unwrap();
        for (name, size) in [
            ("system.db", 3),
            ("instances.db", 5),
            ("storage.db", 7),
            ("records.db", 11),
            ("system.db-journal", 13),
            ("accounts.db", 100),
        ] {
            tokio::fs::write(root.path().join(name), vec![0; size])
                .await
                .unwrap();
        }
        let info = database_info_at(&root.path().join("system.db"))
            .await
            .unwrap();
        assert_eq!(info.layout, "shared");
        assert!(info.channel.is_none());
        assert_eq!(info.bytes, 39);
        assert_eq!(info.databases.len(), 4);
        assert!(
            !serde_json::to_string(&info)
                .unwrap()
                .contains(root.path().to_str().unwrap())
        );
    }
    #[tokio::test]
    async fn lists_complete_bundles_and_legacy_files_with_recursive_byte_sizes()
    {
        let root = tempfile::tempdir().unwrap();
        let shared = crate::state::layout::data(root.path());
        let old = root.path().join("app-db");
        let bundle = shared.join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(bundle.join("state"))
            .await
            .unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({"protocol":"axolotl-storage-snapshot","version":crate::state::layout::SNAPSHOT_VERSION,"core":["system.db","instances.db","storage.db"]})).unwrap();
        tokio::fs::write(bundle.join("snapshot.json"), &manifest)
            .await
            .unwrap();
        for name in ["system.db", "instances.db", "storage.db"] {
            tokio::fs::write(bundle.join(name), b"core").await.unwrap();
        }
        tokio::fs::write(bundle.join("complete"), b"done")
            .await
            .unwrap();
        tokio::fs::write(bundle.join("state/user.json"), b"user-data")
            .await
            .unwrap();
        let partial = shared.join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(&partial).await.unwrap();
        tokio::fs::write(partial.join("system.db"), b"partial")
            .await
            .unwrap();
        let damaged = shared.join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(&damaged).await.unwrap();
        tokio::fs::write(damaged.join("complete"), b"done")
            .await
            .unwrap();
        tokio::fs::write(damaged.join("snapshot.json"), b"{")
            .await
            .unwrap();
        tokio::fs::create_dir_all(old.join("beta")).await.unwrap();
        tokio::fs::write(old.join("beta/app-db-before-test.db"), b"legacy")
            .await
            .unwrap();
        let listing = list_database_backups_at(&[shared, old]).await.unwrap();
        assert_eq!(listing.len(), 2);
        let snapshot =
            listing.iter().find(|info| info.layout == "shared").unwrap();
        assert_eq!(snapshot.bytes, manifest.len() as u64 + 12 + 4 + 9);
        assert!(snapshot.channel.is_none());
        assert_eq!(
            listing
                .iter()
                .find(|info| info.layout == "legacy_channel")
                .unwrap()
                .channel
                .as_deref(),
            Some("beta")
        );
        tokio::fs::remove_file(bundle.join("instances.db"))
            .await
            .unwrap();
        assert!(database_backup_info(&bundle).await.unwrap().is_none());
        tokio::fs::write(bundle.join("instances.db"), b"core")
            .await
            .unwrap();
        tokio::fs::remove_file(bundle.join("complete"))
            .await
            .unwrap();
        assert!(database_backup_info(&bundle).await.unwrap().is_none());
    }
}
pub use crate::state::sync_state::gc::SyncCleanupReport;

/// Inspect or remove superseded generations while the launcher is closed.
/// An explicitly named source lets a copied authority be checked in isolation.
pub async fn cleanup_sync_history(
    settings_root: &std::path::Path,
    copied_from: Option<&std::path::Path>,
    apply: bool,
    limit: Option<usize>,
    progress: impl FnMut(&SyncCleanupReport),
    failure: impl FnMut(&str, &crate::Error),
) -> crate::Result<SyncCleanupReport> {
    crate::state::store::sync_documents::cleanup_history(
        settings_root,
        copied_from,
        apply,
        limit,
        progress,
        failure,
    )
    .await
}
