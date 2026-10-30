use super::files::paths;
use super::{LauncherBackend, business_error, page, serialize};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
async fn idle() -> Result<(), ToolError> {
    if theseus::install::list_jobs(false)
        .await
        .map_err(business_error)?
        .iter()
        .any(|j| !j.status.is_finished())
        || theseus::instance::has_active_backup_operations()
            .await
            .map_err(business_error)?
    {
        return Err(ToolError::new(
            "launcher_busy",
            "Wait for installation and backup jobs before maintenance",
        ));
    }
    Ok(())
}
fn app(backend: &LauncherBackend) -> Result<tauri::AppHandle, ToolError> {
    backend.app.clone().ok_or_else(|| {
        ToolError::new("app_unavailable", "Desktop app handle is unavailable")
    })
}
fn flatten(node: theseus::storage::StorageNode, output: &mut Vec<Value>) {
    if output.len() >= 20000 {
        return;
    }
    output.push(json!({"node_id":format!("storage:{}",paths::hash(node.id.as_bytes())),"type":node.node_type,"name":node.name,"instance_id":node.instance_id,"size":node.size,"count":node.count}));
    for child in node.children.unwrap_or_default() {
        flatten(child, output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_nodes_hide_absolute_native_identifiers_and_paths() {
        let node = theseus::storage::StorageNode {
            id: "C:/private/launcher/cache".into(),
            node_type: theseus::storage::StorageNodeType::Cache,
            name: Some("cache".into()),
            instance_id: None,
            size: theseus::storage::StorageSize {
                actual: 12,
                symlink: 0,
            },
            count: Some(1),
            paths: vec![theseus::storage::StoragePath {
                path: "C:/private/launcher/cache".into(),
                kind: theseus::storage::StoragePathKind::Directory,
            }],
            children: None,
        };
        let mut result = Vec::new();
        flatten(node, &mut result);
        assert_eq!(result[0]["size"]["actual"], 12);
        assert!(!serde_json::to_string(&result).unwrap().contains("private"));
        assert!(
            result[0]["node_id"]
                .as_str()
                .unwrap()
                .starts_with("storage:")
        );
    }
}
pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::StorageScan {
            force,
            offset,
            limit,
        } => {
            let tree = theseus::storage::scan_storage(force.unwrap_or(false))
                .await
                .map_err(business_error)?;
            let mut nodes = Vec::new();
            for node in tree
                .categories
                .into_iter()
                .chain(tree.root_other)
                .chain(tree.external)
            {
                flatten(node, &mut nodes);
            }
            let mut result =
                page(nodes, offset.unwrap_or(0), limit.unwrap_or(50))?;
            result["node_listing_truncated"] =
                json!(result["total"].as_u64().is_some_and(|n| n >= 20000));
            result["scanned_at"] = json!(tree.scanned_at);
            result["size"] = json!(tree.total);
            result["warning_count"] = json!(tree.warnings.len());
            Ok(result)
        }
        Command::StorageClearCache {} => {
            idle().await?;
            let result = theseus::storage::clear_cache()
                .await
                .map_err(business_error)?;
            Ok(
                json!({"removed_bytes":result.removed_bytes,"remaining_bytes":result.remaining_bytes,"failure_count":result.failures.len()}),
            )
        }
        Command::StorageAnalyzeMetadata {} => {
            let result = theseus::storage::analyze_meta()
                .await
                .map_err(business_error)?;
            let handle = backend.metadata_cleanups.insert(
                &context.session_id,
                1,
                result.id,
            )?;
            Ok(
                json!({"cleanup_id":handle,"scanned_at":result.scanned_at,"groups":result.groups,"warnings":result.warnings.into_iter().take(100).map(|w|json!({"kind":w.kind})).collect::<Vec<_>>()}),
            )
        }
        Command::StorageCleanMetadata { cleanup_id, groups } => {
            idle().await?;
            let native = backend.metadata_cleanups.take(
                &context.session_id,
                cleanup_id,
                1,
            )?;
            let result = theseus::storage::clean_meta(native, groups)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"removed_files":result.removed_files,"removed_bytes":result.removed_bytes,"removed_groups":result.removed_groups,"skipped_files":result.skipped_files,"failure_count":result.failures.len()}),
            )
        }
        Command::DatabaseGetInfo {} => {
            let app = app(backend)?;
            let path = theseus::maintenance::current_database_path()
                .await
                .map_err(business_error)?;
            let metadata = tokio::fs::metadata(&path).await.map_err(|_| {
                ToolError::new(
                    "database_unavailable",
                    "Database metadata is unavailable",
                )
            })?;
            Ok(
                json!({"channel":path.parent().and_then(std::path::Path::file_name).map(|s|s.to_string_lossy().into_owned()),"configured_next_channel":crate::resolve_update_channel(&app).await.map_err(|_|ToolError::new("channel_unavailable","Database channel unavailable"))?,"bytes":metadata.len(),"beta_database_exists":theseus::beta_database_exists(&app.config().identifier).await.map_err(business_error)?}),
            )
        }
        Command::DatabaseCheckIntegrity {} => serialize(
            theseus::maintenance::check_database_integrity()
                .await
                .map_err(business_error)?,
        ),
        Command::DatabaseBackup {} => {
            let app = app(backend)?;
            let version = app.package_info().version.to_string();
            let path = theseus::maintenance::backup_open_database(&version)
                .await
                .map_err(business_error)?;
            let metadata = tokio::fs::metadata(&path).await.map_err(|_| {
                ToolError::new(
                    "backup_unavailable",
                    "Backup metadata unavailable",
                )
            })?;
            Ok(
                json!({"backup_name":path.file_name().map(|n|n.to_string_lossy().into_owned()),"bytes":metadata.len(),"channel":path.parent().and_then(std::path::Path::file_name).map(|n|n.to_string_lossy().into_owned())}),
            )
        }
        Command::DatabaseListBackups { offset, limit } => {
            let base = theseus::prelude::app_db_backup_dir()
                .map_err(business_error)?;
            let mut output = Vec::new();
            for channel in ["release", "beta", ""] {
                let root = base.join(channel);
                let mut entries = match tokio::fs::read_dir(root).await {
                    Ok(e) => e,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        continue;
                    }
                    Err(_) => {
                        return Err(ToolError::new(
                            "backup_unavailable",
                            "Backup directory unavailable",
                        ));
                    }
                };
                while let Some(entry) =
                    entries.next_entry().await.map_err(|_| {
                        ToolError::new(
                            "backup_unavailable",
                            "Backup listing failed",
                        )
                    })?
                {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let metadata = entry.metadata().await.map_err(|_| {
                        ToolError::new(
                            "backup_unavailable",
                            "Backup metadata unavailable",
                        )
                    })?;
                    if entry
                        .file_type()
                        .await
                        .map_err(|_| {
                            ToolError::new(
                                "backup_unavailable",
                                "Backup type unavailable",
                            )
                        })?
                        .is_file()
                        && name.starts_with("app-db-")
                        && name.ends_with(".db")
                    {
                        output.push(json!({"channel":channel,"backup_name":name,"bytes":metadata.len(),"modified_at":metadata.modified().ok()}));
                    }
                    if output.len() >= 1000 {
                        break;
                    }
                }
            }
            output.sort_by(|a, b| {
                b["backup_name"].as_str().cmp(&a["backup_name"].as_str())
            });
            page(output, offset.unwrap_or(0), limit.unwrap_or(50))
        }
        Command::DatabaseMaintainBackups {} => {
            idle().await?;
            theseus::State::get()
                .await
                .map_err(business_error)?
                .maintain_database_backups()
                .await
                .map_err(business_error)?;
            Ok(json!({"maintenance_finished":true}))
        }
        Command::DatabaseCopyChannel {
            source_channel,
            target_channel,
        } => {
            idle().await?;
            let app = app(backend)?;
            let active = theseus::maintenance::current_database_path()
                .await
                .map_err(business_error)?;
            if active
                .parent()
                .and_then(std::path::Path::file_name)
                .is_some_and(|s| s == std::ffi::OsStr::new(&target_channel))
            {
                return Err(ToolError::new(
                    "active_database_target",
                    "The actually opened database cannot be overwritten, even after selecting another update channel",
                ));
            }
            theseus::copy_database_between_channels(
                &app.config().identifier,
                &source_channel,
                &target_channel,
            )
            .await
            .map_err(business_error)?;
            Ok(
                json!({"copied":true,"source_channel":source_channel,"target_channel":target_channel}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a storage/database operation",
        )),
    }
}
