use super::{LauncherBackend, business_error, page, serialize};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use theseus::instance::{
    self as backup, BackupOperation, BackupOperationState, BackupOperationType,
};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct RestorePreview {
    instance_id: String,
    snapshot_id: String,
    operation_id: Uuid,
}

fn operation_summary(operation: BackupOperation) -> Value {
    json!({"backup_operation_id":operation.id,"instance_id":operation.instance_id,"snapshot_id":operation.snapshot_id,
        "type":operation.operation_type,"state":operation.state,"processed_bytes":operation.processed_bytes,"total_bytes":operation.total_bytes,
        "cancellable":operation.cancellable,"cancel_requested":operation.cancel_requested,"has_error":operation.error.is_some(),
        "created_at":operation.created_at,"updated_at":operation.updated_at,"finished_at":operation.finished_at,
        "restore_preview":operation.restore_preview.map(|preview|json!({"added_files":preview.added_files,"modified_files":preview.modified_files,"deleted_files":preview.deleted_files}))})
}

async fn snapshot_exists(
    instance_id: &str,
    snapshot_id: &str,
) -> Result<(), ToolError> {
    if !backup::list_backup_snapshots(instance_id)
        .await
        .map_err(business_error)?
        .iter()
        .any(|snapshot| {
            snapshot.id == snapshot_id && snapshot.instance_id == instance_id
        })
    {
        return Err(ToolError::new(
            "snapshot_not_found",
            "Snapshot does not belong to this instance or no longer exists",
        ));
    }
    Ok(())
}

async fn operation(
    instance_id: &str,
    id: Uuid,
) -> Result<BackupOperation, ToolError> {
    backup::list_backup_operations(Some(instance_id), false)
        .await
        .map_err(business_error)?
        .into_iter()
        .find(|op| op.id == id && op.instance_id == instance_id)
        .ok_or_else(|| {
            ToolError::new(
                "backup_operation_not_found",
                "Backup operation not found for this instance",
            )
        })
}

fn preview_ready<'a>(
    preview: &RestorePreview,
    operation: &'a BackupOperation,
) -> Result<&'a backup::BackupRestorePreview, ToolError> {
    if operation.id != preview.operation_id
        || operation.instance_id != preview.instance_id
        || operation.snapshot_id.as_deref() != Some(&preview.snapshot_id)
        || operation.operation_type != BackupOperationType::RestorePreview
    {
        return Err(ToolError::new(
            "invalid_preview",
            "Operation does not match this restoration preview",
        ));
    }
    if operation.state != BackupOperationState::Completed {
        return Err(ToolError::new(
            "preview_not_ready",
            "Wait for restoration preview to complete successfully",
        ));
    }
    operation
        .restore_preview
        .as_ref()
        .filter(|result| !result.plan_token.is_empty())
        .ok_or_else(|| {
            ToolError::new(
                "invalid_preview",
                "Completed preview has no valid restore plan",
            )
        })
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::BackupRepositoryStatus {} => {
            let status = backup::get_backup_repository_status()
                .await
                .map_err(business_error)?;
            Ok(
                json!({"path":status.path,"initialized":status.initialized,"available":status.available,"stored_size":status.stored_size,
                "logical_size":status.logical_size,"snapshot_count":status.snapshot_count,"has_error":status.error.is_some()}),
            )
        }
        Command::BackupGetConfig { instance_id } => serialize(
            backup::get_backup_config(&instance_id)
                .await
                .map_err(business_error)?,
        ),
        Command::BackupEnable { instance_id } => {
            let current = backup::get_backup_config(&instance_id)
                .await
                .map_err(business_error)?;
            serialize(
                backup::enable_backups(&instance_id, current.excluded_paths)
                    .await
                    .map_err(business_error)?,
            )
        }
        Command::BackupDisable { instance_id } => {
            backup::disable_backups(&instance_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"enabled":false,"instance_id":instance_id}))
        }
        Command::BackupSetExclusions {
            instance_id,
            files,
            directories,
        } => {
            let exclusions = files
                .into_iter()
                .map(|path| backup::BackupExclusion {
                    path,
                    kind: backup::BackupExclusionKind::File,
                })
                .chain(directories.into_iter().map(|path| {
                    backup::BackupExclusion {
                        path,
                        kind: backup::BackupExclusionKind::Directory,
                    }
                }))
                .collect();
            serialize(
                backup::update_backup_exclusions(&instance_id, exclusions)
                    .await
                    .map_err(business_error)?,
            )
        }
        Command::BackupCreate { instance_id } => {
            let id = backup::start_backup_snapshot(instance_id)
                .await
                .map_err(business_error)?;
            Ok(json!({"backup_operation_id":id}))
        }
        Command::BackupListSnapshots {
            instance_id,
            offset,
            limit,
        } => page(
            backup::list_backup_snapshots(&instance_id)
                .await
                .map_err(business_error)?,
            offset,
            limit,
        ),
        Command::BackupListOperations {
            instance_id,
            active_only,
            offset,
            limit,
        } => {
            let mut operations =
                backup::list_backup_operations(Some(&instance_id), active_only)
                    .await
                    .map_err(business_error)?;
            operations.sort_by(|a, b| {
                b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id))
            });
            page(
                operations.into_iter().map(operation_summary).collect(),
                offset,
                limit,
            )
        }
        Command::BackupGetOperation {
            instance_id,
            backup_operation_id,
        } => Ok(operation_summary(
            operation(&instance_id, backup_operation_id).await?,
        )),
        Command::BackupCancel {
            instance_id,
            backup_operation_id,
        } => {
            operation(&instance_id, backup_operation_id).await?;
            Ok(
                json!({"accepted":backup::cancel_backup(backup_operation_id).await.map_err(business_error)?}),
            )
        }
        Command::BackupPreviewRestore {
            instance_id,
            snapshot_id,
        } => {
            snapshot_exists(&instance_id, &snapshot_id).await?;
            let operation_id = backup::get_backup_restore_preview(&snapshot_id)
                .await
                .map_err(business_error)?;
            let handle = backend.restores.insert(
                &context.session_id,
                0,
                RestorePreview {
                    instance_id,
                    snapshot_id,
                    operation_id,
                },
            );
            let handle = match handle {
                Ok(handle) => handle,
                Err(error) => {
                    let _ = backup::cancel_backup(operation_id).await;
                    return Err(error);
                }
            };
            Ok(json!({"preview_id":handle,"backup_operation_id":operation_id}))
        }
        Command::BackupGetRestorePreview { preview_id } => {
            let preview =
                backend.restores.get(&context.session_id, preview_id, 0)?;
            let operation =
                operation(&preview.instance_id, preview.operation_id).await?;
            let ready = preview_ready(&preview, &operation).is_ok();
            Ok(
                json!({"preview_id":preview_id,"instance_id":preview.instance_id,"snapshot_id":preview.snapshot_id,"ready":ready,"operation":operation_summary(operation)}),
            )
        }
        Command::BackupRestore { preview_id } => {
            let preview =
                backend.restores.get(&context.session_id, preview_id, 0)?;
            let operation =
                operation(&preview.instance_id, preview.operation_id).await?;
            let token = preview_ready(&preview, &operation)?.plan_token.clone();
            snapshot_exists(&preview.instance_id, &preview.snapshot_id).await?;
            let preview =
                backend.restores.take(&context.session_id, preview_id, 0)?;
            let id = backup::restore_snapshot(&preview.snapshot_id, &token)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"instance_id":preview.instance_id,"backup_operation_id":id}),
            )
        }
        Command::BackupDeleteSnapshot {
            instance_id,
            snapshot_id,
        } => {
            snapshot_exists(&instance_id, &snapshot_id).await?;
            backup::delete_backup_snapshot(&snapshot_id)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"deleted":true,"instance_id":instance_id,"snapshot_id":snapshot_id}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a backup operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview() -> RestorePreview {
        RestorePreview {
            instance_id: "instance".into(),
            snapshot_id: "snapshot".into(),
            operation_id: Uuid::new_v4(),
        }
    }

    fn operation(preview: &RestorePreview) -> BackupOperation {
        BackupOperation {
            id: preview.operation_id,
            operation_type: BackupOperationType::RestorePreview,
            instance_id: preview.instance_id.clone(),
            snapshot_id: Some(preview.snapshot_id.clone()),
            state: BackupOperationState::Completed,
            processed_bytes: 100,
            total_bytes: 100,
            cancellable: false,
            cancel_requested: false,
            error: Some("private native diagnostic".into()),
            created_at: 1,
            updated_at: 2,
            finished_at: Some(2),
            restore_preview: Some(backup::BackupRestorePreview {
                added_files: 1,
                modified_files: 2,
                deleted_files: 3,
                plan_token: "private-native-token".into(),
            }),
        }
    }

    #[test]
    fn operation_summaries_never_expose_restore_tokens_or_raw_errors() {
        let preview = preview();
        let summary = operation_summary(operation(&preview));
        assert_eq!(summary["restore_preview"]["deleted_files"], 3);
        assert_eq!(summary["has_error"], true);
        assert!(!summary.to_string().contains("private-native-token"));
        assert!(!summary.to_string().contains("private native diagnostic"));
        assert!(summary["restore_preview"].get("plan_token").is_none());
    }

    #[test]
    fn restore_preview_must_match_instance_snapshot_operation_and_type() {
        let preview = preview();
        assert_eq!(
            preview_ready(&preview, &operation(&preview))
                .unwrap()
                .plan_token,
            "private-native-token"
        );
        for invalid in 0..4 {
            let mut operation = operation(&preview);
            match invalid {
                0 => operation.instance_id = "other".into(),
                1 => operation.snapshot_id = Some("other".into()),
                2 => operation.id = Uuid::new_v4(),
                _ => operation.operation_type = BackupOperationType::Create,
            }
            assert_eq!(
                preview_ready(&preview, &operation).unwrap_err().code,
                "invalid_preview"
            );
        }
    }

    #[test]
    fn incomplete_failed_or_empty_previews_cannot_restore() {
        let preview = preview();
        for state in [
            BackupOperationState::Queued,
            BackupOperationState::Scanning,
            BackupOperationState::Failed,
            BackupOperationState::Cancelled,
            BackupOperationState::Interrupted,
        ] {
            let mut operation = operation(&preview);
            operation.state = state;
            assert_eq!(
                preview_ready(&preview, &operation).unwrap_err().code,
                "preview_not_ready"
            );
        }
        let mut operation = operation(&preview);
        operation
            .restore_preview
            .as_mut()
            .unwrap()
            .plan_token
            .clear();
        assert_eq!(
            preview_ready(&preview, &operation).unwrap_err().code,
            "invalid_preview"
        );
    }

    #[test]
    fn preview_queries_are_repeatable_but_restore_consumes_the_handle() {
        let backend = LauncherBackend::default();
        let handle = backend.restores.insert("owner", 0, preview()).unwrap();
        for _ in 0..3 {
            assert_eq!(
                backend
                    .restores
                    .get("owner", handle, 0)
                    .unwrap()
                    .snapshot_id,
                "snapshot"
            );
        }
        assert!(backend.restores.get("other", handle, 0).is_err());
        backend.restores.take("owner", handle, 0).unwrap();
        assert!(backend.restores.get("owner", handle, 0).is_err());
    }
}
