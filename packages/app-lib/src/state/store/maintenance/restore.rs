//! Explicit offline restore, with a durable roll-forward receipt before replacing
//! the first authority. Prior files remain in the restore workspace.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Executor, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    source: PathBuf,
    target: PathBuf,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    protocol: String,
    version: u32,
    root: PathBuf,
    content_root: PathBuf,
    workspace: PathBuf,
    files: Vec<File>,
}
fn invalid(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}
fn receipt(root: &Path) -> PathBuf {
    root.join("databases/storage-restore.json")
}
async fn digest(path: &Path) -> crate::Result<String> {
    if !tokio::fs::symlink_metadata(path)
        .await?
        .file_type()
        .is_file()
    {
        return Err(invalid("Restore requires regular files"));
    }
    let mut input = tokio::fs::File::open(path).await?;
    let mut hash = Sha256::new();
    let mut bytes = vec![0; 65536];
    loop {
        let n = input.read(&mut bytes).await?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
async fn bounded_json(path: &Path) -> crate::Result<serde_json::Value> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.is_file() || metadata.len() > 16 << 20 {
        return Err(invalid("Restore document exceeds bounds"));
    }
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new();
    file.take((16 << 20) + 1).read_to_end(&mut bytes).await?;
    if bytes.len() > 16 << 20 {
        return Err(invalid("Restore document grew beyond bounds"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
async fn regular_copy(source: &Path, destination: &Path) -> crate::Result<()> {
    if !tokio::fs::symlink_metadata(source)
        .await?
        .file_type()
        .is_file()
    {
        return Err(invalid("Restore refuses linked snapshot files"));
    }
    tokio::fs::create_dir_all(
        destination
            .parent()
            .ok_or_else(|| invalid("Restore destination has no parent"))?,
    )
    .await?;
    tokio::fs::copy(source, destination).await?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .open(destination)
        .await?
        .sync_all()
        .await?;
    Ok(())
}
async fn tree(source: &Path) -> crate::Result<Vec<PathBuf>> {
    let mut pending = vec![PathBuf::new()];
    let mut output = Vec::new();
    while let Some(relative) = pending.pop() {
        let directory = source.join(&relative);
        if !tokio::fs::symlink_metadata(&directory).await?.is_dir() {
            return Err(invalid("Restore snapshot contains linked directory"));
        }
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = relative.join(entry.file_name());
            let kind = entry.file_type().await?;
            if kind.is_dir() {
                pending.push(path)
            } else if kind.is_file() {
                output.push(path)
            } else {
                return Err(invalid(
                    "Restore snapshot contains a nonregular member",
                ));
            }
        }
    }
    Ok(output)
}
async fn safe_destination(root: &Path, target: &Path) -> crate::Result<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| invalid("Restore target escapes its selected root"))?;
    if relative
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(invalid("Restore target is not a normalized child path"));
    }
    let mut parent = root.to_owned();
    for component in relative.parent().into_iter().flat_map(Path::components) {
        parent.push(component.as_os_str());
        if tokio::fs::try_exists(&parent).await? {
            if !tokio::fs::symlink_metadata(&parent).await?.is_dir() {
                return Err(invalid("Restore refuses linked parent directory"));
            }
        } else {
            tokio::fs::create_dir(&parent).await?;
        }
    }
    if tokio::fs::try_exists(target).await?
        && !tokio::fs::symlink_metadata(target)
            .await?
            .file_type()
            .is_file()
    {
        return Err(invalid("Restore refuses a linked target"));
    }
    Ok(())
}

/// A native maintenance caller must close the launcher first. This API is an
/// explicit operation: startup only resumes a previously published receipt.
/// It restores launcher state; game files and account credentials are separate.
pub(crate) async fn restore_snapshot(
    root: &Path,
    bundle: &Path,
    content_root: &Path,
) -> crate::Result<PathBuf> {
    if crate::State::get_if_initialized().is_some() {
        return Err(invalid("Close the launcher before restoring its storage"));
    }
    tokio::fs::create_dir_all(root.join("databases")).await?;
    tokio::fs::create_dir_all(content_root).await?;
    let root = tokio::fs::canonicalize(root).await?;
    let content_root = tokio::fs::canonicalize(content_root).await?;
    let bundle = tokio::fs::canonicalize(bundle).await?;
    let _ownership = crate::state::db::acquire_domain_ownership(
        &root.join("databases/system.db"),
    )
    .await?;
    finish_restore(&root).await?;
    let metadata: serde_json::Value =
        bounded_json(&bundle.join("snapshot.json")).await?;
    if metadata["protocol"] != "axolotl-storage-snapshot"
        || metadata["version"] != 1
        || !tokio::fs::try_exists(bundle.join("complete")).await?
    {
        return Err(invalid(
            "Snapshot is incomplete or uses an unsupported contract",
        ));
    }
    if metadata["core"]
        != serde_json::json!(["system.db", "instances.db", "storage.db"])
    {
        return Err(invalid("Snapshot core set is not supported"));
    }
    let workspace =
        root.join(format!(".storage-restore-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&workspace).await?;
    let staged = workspace.join("next");
    tokio::fs::create_dir(&staged).await?;
    let mut files = Vec::new();
    for relative in tree(&bundle).await? {
        if !["settings", "state", "media", "synced-options"]
            .iter()
            .any(|name| relative.starts_with(name))
            && !matches!(
                relative.to_str(),
                Some(
                    "system.db"
                        | "instances.db"
                        | "storage.db"
                        | "records.db"
                        | "operations-history.db"
                        | "accounts.json"
                        | "instance_groups.json"
                        | "favorite_groups.json"
                        | "axolotl_config.json"
                )
            )
        {
            continue;
        }
        if relative.starts_with("operation-records")
            || relative.starts_with("unavailable")
            || matches!(
                relative.to_str(),
                Some(
                    "snapshot.json"
                        | "complete"
                        | "backup-status.json"
                        | "sync-backup-status.json"
                )
            )
        {
            continue;
        }
        let target = if matches!(
            relative.to_str(),
            Some(
                "system.db"
                    | "instances.db"
                    | "storage.db"
                    | "records.db"
                    | "operations-history.db"
            )
        ) {
            root.join("databases").join(&relative)
        } else if relative.starts_with("synced-options") {
            content_root.join(&relative)
        } else {
            root.join(&relative)
        };
        let source = staged.join(&relative);
        regular_copy(&bundle.join(&relative), &source).await?;
        files.push(File {
            source,
            target,
            sha256: String::new(),
        });
    }
    for name in ["system.db", "instances.db", "storage.db"] {
        if !staged.join(name).is_file() {
            return Err(invalid("Snapshot lacks a required core file"));
        }
    }
    let mut connection = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(staged.join("system.db"))
            .foreign_keys(true),
    )
    .await?;
    for name in ["instances", "storage"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "ATTACH DATABASE ? AS {name}"
        )))
        .bind(
            staged
                .join(format!("{name}.db"))
                .to_string_lossy()
                .into_owned(),
        )
        .execute(&mut connection)
        .await?;
    }
    super::super::databases::validate_snapshot(&mut connection).await?;
    let sync_available =
        staged.join("synced-options/state/head.json").is_file();
    connection.execute("BEGIN IMMEDIATE").await?;
    if !sync_available {
        connection
            .execute("DELETE FROM main.sync_decisions")
            .await?;
    }
    super::super::recovery_documents::restore_references(
        &mut connection,
        &root,
        &content_root,
        &bundle,
    )
    .await?;
    connection.execute("COMMIT").await?;
    connection.close().await?;
    // A requested content-root selection is explicit; preserve all other keys.
    let bootstrap = staged.join("settings/bootstrap.json");
    if bootstrap.is_file() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&bootstrap).await?)?;
        if value["schema_version"] != 1 || !value["data"].is_object() {
            return Err(invalid("Cannot rebind newer bootstrap document"));
        }
        value["data"]["custom_dir"] = serde_json::json!(content_root);
        value["data"]["prev_custom_dir"] = serde_json::json!(content_root);
        crate::state::persistence::document::write_json(
            &bootstrap,
            serde_json::to_vec(&value)?,
        )
        .await?;
    } else if content_root != root {
        return Err(invalid(
            "Snapshot has no bootstrap document for the selected content directory",
        ));
    }
    let descriptor = staged.join("state/sync.json");
    if sync_available {
        let mut value = bounded_json(&descriptor).await?;
        if value["protocol"] != "axolotl-sync-authority"
            || value["schema_version"] != 1
            || value["data"]["contract"] != 1
        {
            return Err(invalid("Cannot rebind unsupported sync descriptor"));
        }
        crate::state::sync_state::manifest::load(
            &staged.join("synced-options"),
        )
        .await?;
        value["data"]["root"] =
            serde_json::json!(content_root.join("synced-options"));
        crate::state::persistence::document::write_json(
            &descriptor,
            serde_json::to_vec(&value)?,
        )
        .await?;
    } else {
        // Missing optional state is an explicit unavailable authority. This
        // blocks old-state import or an accidental mix with the current head.
        crate::state::persistence::document::write_json(&descriptor,serde_json::to_vec(&serde_json::json!({"protocol":"axolotl-sync-unavailable","schema_version":1,"data":{"reason":"excluded-from-restored-snapshot"}}))?).await?;
        if !files.iter().any(|file| file.source == descriptor) {
            files.push(File {
                source: descriptor.clone(),
                target: root.join("state/sync.json"),
                sha256: String::new(),
            });
        }
    }
    // An excluded document must not retain a current authority or resurrect
    // its old database when restored into a fresh or partially populated root.
    // The valid receipt records takeover; its deliberately unsupported target
    // envelope records unavailability rather than fabricated empty user data.
    if let Some(exclusions) = metadata["optional_exclusions"].as_array() {
        for excluded in exclusions {
            let name = excluded
                .as_str()
                .ok_or_else(|| invalid("Invalid excluded domain name"))?;
            if matches!(name, "instance-recovery-copies" | "operation-history")
            {
                continue;
            }
            if name.is_empty()
                || !name.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'-' | b'_')
                })
            {
                return Err(invalid("Invalid excluded domain name"));
            }
            let target = match name {
                "accounts" => "accounts.json".to_owned(),
                "telemetry" => "state/telemetry.json".to_owned(),
                "media" => "state/media.json".to_owned(),
                "sync" => "state/sync.json".to_owned(),
                "worlds" | "joins" | "shared_logs" | "resource_favorites"
                | "instance_pins" => format!("state/{name}.json"),
                "daily-playtime" => "state/records.json".to_owned(),
                "sync-pending" => "state/sync-pending.json".to_owned(),
                _ => format!("state/unavailable/{name}.json"),
            };
            let relative_receipt =
                format!("state/persistence/imports/{name}.json");
            let unavailable = serde_json::json!({"protocol":"axolotl-restored-unavailable","schema_version":1,"revision":1,"data":{"domain":name,"reason":"excluded-from-restored-snapshot"}});
            let takeover = serde_json::json!({"protocol":"axolotl-import","schema_version":1,"revision":1,"data":{"name":name,"target":target,"complete":true,"restored_unavailable":true}});
            for (relative, value) in
                [(&target, unavailable), (&relative_receipt, takeover)]
            {
                let source = staged.join(relative);
                crate::state::persistence::document::write_json(
                    &source,
                    serde_json::to_vec(&value)?,
                )
                .await?;
                if !files.iter().any(|file| file.source == source) {
                    files.push(File {
                        source,
                        target: root.join(relative),
                        sha256: String::new(),
                    });
                }
            }
        }
    }
    for file in &mut files {
        file.sha256 = digest(&file.source).await?;
    }
    // The commit receipt is durable before the first old authority is moved.
    let intent = Intent {
        protocol: "axolotl-storage-restore".into(),
        version: 1,
        root: root.clone(),
        content_root,
        workspace: workspace.clone(),
        files,
    };
    crate::state::persistence::document::write_json(
        &receipt(&root),
        serde_json::to_vec(&intent)?,
    )
    .await?;
    finish_restore(&root).await?;
    Ok(workspace)
}

/// Called while the coordinator ownership lock is held, before opening SQLite.
pub(crate) async fn finish_restore(root: &Path) -> crate::Result<()> {
    let canonical = tokio::fs::canonicalize(root).await?;
    let root = canonical.as_path();
    let marker = receipt(root);
    if !tokio::fs::try_exists(&marker).await? {
        return Ok(());
    }
    let intent: Intent = serde_json::from_value(bounded_json(&marker).await?)?;
    if intent.protocol != "axolotl-storage-restore"
        || intent.version != 1
        || intent.root != root
        || intent.workspace.parent() != Some(root)
    {
        return Err(invalid("Unsupported restore receipt retained"));
    }
    // Validate the whole staged set before touching any current authority.
    for file in &intent.files {
        if file
            .source
            .strip_prefix(intent.workspace.join("next"))
            .is_err()
            || file
                .source
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            || digest(&file.source).await? != file.sha256
        {
            return Err(invalid(
                "Restore staged member failed integrity validation",
            ));
        }
    }
    for (index, file) in intent.files.iter().enumerate() {
        let boundary = if file.target.starts_with(root) {
            root
        } else {
            intent.content_root.as_path()
        };
        safe_destination(boundary, &file.target).await?;
        // Old SQLite sidecars must be removed even if the database image already
        // matches; replaying another generation's journal would corrupt it.
        if file.target.extension().and_then(|value| value.to_str())
            == Some("db")
        {
            for suffix in ["-wal", "-shm", "-journal"] {
                let mut sidecar = file.target.as_os_str().to_owned();
                sidecar.push(suffix);
                let sidecar = PathBuf::from(sidecar);
                if tokio::fs::try_exists(&sidecar).await? {
                    let archived = intent
                        .workspace
                        .join("previous")
                        .join(format!("{index}{suffix}"));
                    if !archived.is_file() {
                        regular_copy(&sidecar, &archived).await?;
                    } else if digest(&sidecar).await?
                        != digest(&archived).await?
                    {
                        regular_copy(
                            &sidecar,
                            &intent.workspace.join("previous").join(format!(
                                "{index}{suffix}-attempt-{}",
                                uuid::Uuid::new_v4()
                            )),
                        )
                        .await?;
                    }
                    tokio::fs::remove_file(sidecar).await?;
                }
            }
        }
        if tokio::fs::try_exists(&file.target).await?
            && digest(&file.target).await? == file.sha256
        {
            continue;
        }
        let previous =
            intent.workspace.join("previous").join(index.to_string());
        if tokio::fs::try_exists(&file.target).await? {
            if !tokio::fs::try_exists(&previous).await? {
                regular_copy(&file.target, &previous).await?;
            } else if digest(&file.target).await? != digest(&previous).await? {
                let unexpected = intent
                    .workspace
                    .join("previous")
                    .join(format!("{index}-attempt-{}", uuid::Uuid::new_v4()));
                regular_copy(&file.target, &unexpected).await?;
            }
            tokio::fs::remove_file(&file.target).await?;
        }
        let mut temporary = file.target.as_os_str().to_owned();
        temporary.push(format!(".restore-{index}.next"));
        let temporary = PathBuf::from(temporary);
        regular_copy(&file.source, &temporary).await?;
        if digest(&temporary).await? != file.sha256 {
            return Err(invalid("Restore copy failed integrity validation"));
        }
        tokio::fs::rename(temporary, &file.target).await?;
    }
    tokio::fs::rename(marker, intent.workspace.join("committed.json")).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ownership_refuses_restore_while_core_is_open() {
        let directory = tempfile::tempdir().unwrap();
        let pool = crate::state::store::databases::open(directory.path())
            .await
            .unwrap();
        let snapshot =
            crate::state::store::databases::backup(&pool, directory.path())
                .await
                .unwrap();
        assert!(
            restore_snapshot(directory.path(), &snapshot, directory.path())
                .await
                .is_err()
        );
        pool.close().await;
    }
    #[tokio::test]
    async fn restore_replaces_core_and_preserves_previous_authorities() {
        let directory = tempfile::tempdir().unwrap();
        let pool = crate::state::store::databases::open(directory.path())
            .await
            .unwrap();
        sqlx::query("INSERT INTO main.database_imports(source,completed) VALUES('restore-fixture',1)").execute(&pool).await.unwrap();
        let snapshot =
            crate::state::store::databases::backup(&pool, directory.path())
                .await
                .unwrap();
        sqlx::query("UPDATE main.database_imports SET completed=0 WHERE source='restore-fixture'").execute(&pool).await.unwrap();
        pool.close().await;
        drop(pool);
        let archived =
            restore_snapshot(directory.path(), &snapshot, directory.path())
                .await
                .unwrap();
        assert!(archived.join("committed.json").is_file());
        assert!(!receipt(directory.path()).is_file());
        let pool = crate::state::store::databases::open(directory.path())
            .await
            .unwrap();
        let value:i64=sqlx::query_scalar("SELECT completed FROM main.database_imports WHERE source='restore-fixture'").fetch_one(&pool).await.unwrap();
        assert_eq!(value, 1);
        assert!(archived.join("previous").is_dir());
        pool.close().await;
    }
    #[tokio::test]
    async fn published_restore_receipt_resumes_after_original_was_archived() {
        let directory = tempfile::tempdir().unwrap();
        let root = tokio::fs::canonicalize(directory.path()).await.unwrap();
        let workspace = root.join(".storage-restore-fixture");
        let source = workspace.join("next/state/item.json");
        let target = root.join("state/item.json");
        tokio::fs::create_dir_all(source.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&source, b"restored").await.unwrap();
        tokio::fs::create_dir_all(workspace.join("previous"))
            .await
            .unwrap();
        tokio::fs::write(workspace.join("previous/0"), b"old")
            .await
            .unwrap();
        let intent = Intent {
            protocol: "axolotl-storage-restore".into(),
            version: 1,
            root: root.clone(),
            content_root: root.clone(),
            workspace: workspace.clone(),
            files: vec![File {
                source: source.clone(),
                target: target.clone(),
                sha256: digest(&source).await.unwrap(),
            }],
        };
        crate::state::persistence::document::write_json(
            &receipt(&root),
            serde_json::to_vec(&intent).unwrap(),
        )
        .await
        .unwrap();
        finish_restore(&root).await.unwrap();
        assert_eq!(tokio::fs::read(target).await.unwrap(), b"restored");
        assert_eq!(
            tokio::fs::read(workspace.join("previous/0")).await.unwrap(),
            b"old"
        );
    }
}
