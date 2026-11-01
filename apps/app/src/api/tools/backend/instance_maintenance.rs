use super::{LauncherBackend, business_error, jobs};
use launcher_tools::{BackendContext, Command, ToolError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use theseus::data::InstanceMetadata;
use theseus::state::InstanceInstallStage;

#[derive(Clone)]
pub(super) struct DeletePreview {
    instance_id: String,
    fingerprint: String,
}

async fn metadata(id: &str) -> Result<InstanceMetadata, ToolError> {
    theseus::instance::get(id)
        .await
        .map_err(business_error)?
        .ok_or_else(|| ToolError::new("not_found", "Instance not found"))
}

fn ordinary(metadata: &InstanceMetadata) -> Result<(), ToolError> {
    let instance = &metadata.instance;
    if instance.is_direct_linked()
        || instance.symlink_target.is_some()
        || instance.game_dir_override.is_some()
    {
        return Err(ToolError::new(
            "external_instance_layout",
            "This operation currently supports ordinary managed instances only; external/linked/custom-directory instances must be managed separately",
        ));
    }
    Ok(())
}

fn linked(metadata: &std::fs::Metadata) -> bool {
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

async fn managed_root(
    metadata: &InstanceMetadata,
) -> Result<(PathBuf, Value), ToolError> {
    ordinary(metadata)?;
    let relative = Path::new(&metadata.instance.path);
    if !matches!(
        relative.components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    ) {
        return Err(ToolError::new(
            "invalid_instance_path",
            "Instance directory must be one managed child folder",
        ));
    }
    let state = theseus::State::get().await.map_err(business_error)?;
    let root = state.directories.instances_dir();
    let canonical_root =
        tokio::fs::canonicalize(&root).await.map_err(|_| {
            ToolError::new(
                "instance_root_missing",
                "Launcher instances directory is unavailable",
            )
        })?;
    checked_child(&canonical_root, relative).await
}

async fn checked_child(
    root: &Path,
    relative: &Path,
) -> Result<(PathBuf, Value), ToolError> {
    if !matches!(
        relative.components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    ) {
        return Err(ToolError::new(
            "invalid_instance_path",
            "Instance directory must be one managed child folder",
        ));
    }
    let path = root.join(relative);
    let entry = tokio::fs::symlink_metadata(&path).await.map_err(|_| {
        ToolError::new(
            "instance_directory_missing",
            "Instance directory is unavailable",
        )
    })?;
    if !entry.is_dir() || linked(&entry) {
        return Err(ToolError::new(
            "linked_instance_path",
            "Instance path must be a real managed directory, not a link",
        ));
    }
    let canonical = tokio::fs::canonicalize(&path).await.map_err(|_| {
        ToolError::new(
            "invalid_instance_path",
            "Unable to resolve instance directory",
        )
    })?;
    if canonical.parent() != Some(root) {
        return Err(ToolError::new(
            "invalid_instance_path",
            "Instance directory resolves outside managed root",
        ));
    }
    let created = entry
        .created()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_nanos().to_string());
    let modified = entry
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_nanos().to_string());
    #[cfg(windows)]
    let identity = {
        use std::os::windows::fs::MetadataExt;
        json!({"created":entry.creation_time(),"attributes":entry.file_attributes()})
    };
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        json!({"device":entry.dev(),"inode":entry.ino()})
    };
    #[cfg(not(any(windows, unix)))]
    let identity = Value::Null;
    Ok((
        canonical.clone(),
        json!({"canonical":canonical,"created":created,"modified":modified,"identity":identity}),
    ))
}

async fn activity(id: &str) -> Result<Value, ToolError> {
    let processes = theseus::process::get_by_instance_id(id)
        .await
        .map_err(business_error)?;
    let jobs = theseus::install::list_jobs(false)
        .await
        .map_err(business_error)?;
    let jobs:Vec<_>=jobs.into_iter().filter(|job|!job.status.is_finished() && (job.instance_id.as_deref()==Some(id)||job.source_instance_id.as_deref()==Some(id))).map(|job|json!({"job_id":job.job_id,"kind":job.kind,"status":job.status})).collect();
    let backups = theseus::instance::list_backup_operations(Some(id), true)
        .await
        .map_err(business_error)?;
    Ok(
        json!({"running_process_count":processes.len(),"active_job_count":jobs.len(),"active_jobs":jobs.into_iter().take(100).collect::<Vec<_>>(),"active_backup_count":backups.len()}),
    )
}
fn idle(activity: &Value) -> Result<(), ToolError> {
    if [
        "running_process_count",
        "active_job_count",
        "active_backup_count",
    ]
    .iter()
    .any(|field| activity[*field].as_u64().unwrap_or(0) > 0)
    {
        return Err(ToolError::new(
            "instance_busy",
            "Stop the game and wait for active installation/copy/backup jobs before maintenance",
        ));
    }
    Ok(())
}
fn fingerprint(
    metadata: &InstanceMetadata,
    directory: Value,
    mut snapshots: Vec<theseus::instance::BackupSnapshot>,
) -> Result<String, ToolError> {
    snapshots.sort_by(|a, b| a.id.cmp(&b.id));
    let value = json!({"metadata":metadata,"directory":directory,"snapshots":snapshots});
    let bytes = serde_json::to_vec(&value).map_err(|_| {
        ToolError::new("preview_failed", "Cannot encode deletion preview state")
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

async fn deletion_state(id: &str) -> Result<(DeletePreview, Value), ToolError> {
    let metadata = metadata(id).await?;
    let (path, directory) = managed_root(&metadata).await?;
    idle(&activity(id).await?)?;
    let snapshots = theseus::instance::list_backup_snapshots(id)
        .await
        .map_err(business_error)?;
    let count = snapshots.len();
    let logical_size = snapshots.iter().fold(0u64, |size, snapshot| {
        size.saturating_add(snapshot.logical_size)
    });
    let preview = DeletePreview {
        instance_id: id.into(),
        fingerprint: fingerprint(&metadata, directory, snapshots)?,
    };
    Ok((
        preview,
        json!({"instance_id":id,"name":metadata.instance.name,"target_directory":path,"snapshot_count":count,"snapshot_logical_size_bytes":logical_size,
        "removes_all_game_files":true,"removes_worlds_and_zip_backups_in_instance":true,"removes_associated_snapshots":true,"automatic_backup_created":false,
        "filesystem_contents_frozen":false,"external_concurrent_changes_possible":true}),
    ))
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::InstanceDuplicate { instance_id } => {
            let metadata = metadata(&instance_id).await?;
            if metadata.instance.is_direct_linked() {
                return Err(ToolError::new(
                    "external_instance_layout",
                    "Direct-linked instances cannot be duplicated",
                ));
            }
            idle(&activity(&instance_id).await?)?;
            Ok(jobs::summary(
                theseus::install::duplicate_instance(instance_id)
                    .await
                    .map_err(business_error)?,
            ))
        }
        Command::InstanceReinstall { instance_id, force } => {
            let metadata = metadata(&instance_id).await?;
            ordinary(&metadata)?;
            idle(&activity(&instance_id).await?)?;
            Ok(jobs::summary(
                theseus::install::install_existing_instance(instance_id, force)
                    .await
                    .map_err(business_error)?,
            ))
        }
        Command::InstanceCheckInstallation { instance_id } => {
            let metadata = metadata(&instance_id).await?;
            let directory =
                theseus::instance::get_full_path(&instance_id).await;
            let directory_exists = match &directory {
                Ok(path) => tokio::fs::metadata(path)
                    .await
                    .is_ok_and(|metadata| metadata.is_dir()),
                Err(_) => false,
            };
            let active = activity(&instance_id).await?;
            let installed = metadata.instance.install_stage
                == InstanceInstallStage::Installed;
            Ok(
                json!({"instance_id":instance_id,"install_stage":metadata.instance.install_stage,"installed":installed,"directory_exists":directory_exists,
                "ready_for_launch":installed&&directory_exists&&idle(&active).is_ok(),"game_version":metadata.applied_content_set.game_version,"loader":metadata.applied_content_set.loader,
                "direct_linked":metadata.instance.is_direct_linked(),"symbolic_link":metadata.instance.symlink_target.is_some(),"custom_game_directory":metadata.instance.game_dir_override.is_some(),
                "activity":active,"file_integrity_checked":false,"account_java_compatibility_checked":false}),
            )
        }
        Command::InstanceDeletePreview { instance_id } => {
            let (preview, mut value) = deletion_state(&instance_id).await?;
            let handle =
                backend.deletions.insert(&context.session_id, 0, preview)?;
            value["preview_id"] = json!(handle);
            value["expires_in_seconds"] = json!(600);
            Ok(value)
        }
        Command::InstanceDelete { preview_id } => {
            let previous =
                backend.deletions.get(&context.session_id, preview_id, 0)?;
            let (current, _) = deletion_state(&previous.instance_id).await?;
            if current.fingerprint != previous.fingerprint {
                return Err(ToolError::new(
                    "stale_delete_preview",
                    "Instance configuration/directory or snapshot list changed; create a new deletion preview",
                ));
            }
            let preview =
                backend.deletions.take(&context.session_id, preview_id, 0)?;
            theseus::instance::remove(&preview.instance_id)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"instance_id":preview.instance_id,"removed":true,"associated_snapshots_removed":true}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not an instance maintenance operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> InstanceMetadata {
        serde_json::from_value(json!({
            "instance":{"id":"instance","path":"profile","install_stage":"installed","launcher_feature_version":"none","update_channel":"release",
                "name":"Instance","created":"2026-01-01T00:00:00Z","modified":"2026-01-01T00:00:00Z","submitted_time_played":0,"recent_time_played":0},
            "applied_content_set":{"id":"content","instance_id":"instance","name":"Content","source_kind":"local","status":"available","game_version":"1.21.1","loader":"fabric",
                "created":"2026-01-01T00:00:00Z","modified":"2026-01-01T00:00:00Z"},
            "link":"unmanaged","groups":[],"launch_overrides":{"instance_id":"instance","hooks":{"pre_launch":null,"wrapper":null,"post_exit":null}}
        })).unwrap()
    }
    fn snapshot(id: &str) -> theseus::instance::BackupSnapshot {
        theseus::instance::BackupSnapshot {
            id: id.into(),
            instance_id: "instance".into(),
            instance_name: "Instance".into(),
            created_at: 1,
            file_count: 1,
            symlink_count: 0,
            logical_size: 100,
            added_size: 100,
        }
    }
    #[test]
    fn external_layouts_are_not_treated_as_ordinary_managed_deletions() {
        let mut metadata = metadata();
        assert!(ordinary(&metadata).is_ok());
        metadata.instance.game_dir_override = Some("C:/external".into());
        assert!(ordinary(&metadata).is_err());
        metadata.instance.game_dir_override = None;
        metadata.instance.symlink_target = Some("C:/external".into());
        assert!(ordinary(&metadata).is_err());
        metadata.instance.symlink_target = None;
        metadata.instance.linked_dot_minecraft = Some("C:/external".into());
        assert!(ordinary(&metadata).is_err());
    }
    #[test]
    fn deletion_fingerprint_changes_with_configuration_directory_and_snapshots()
    {
        let mut metadata = metadata();
        let dir =
            json!({"canonical":"managed/profile","created":1,"identity":1});
        let expected = fingerprint(
            &metadata,
            dir.clone(),
            vec![snapshot("b"), snapshot("a")],
        )
        .unwrap();
        assert_eq!(
            expected,
            fingerprint(
                &metadata,
                dir.clone(),
                vec![snapshot("a"), snapshot("b")]
            )
            .unwrap()
        );
        metadata.instance.name = "Changed".into();
        assert_ne!(
            expected,
            fingerprint(
                &metadata,
                dir.clone(),
                vec![snapshot("a"), snapshot("b")]
            )
            .unwrap()
        );
        metadata.instance.name = "Instance".into();
        assert_ne!(
            expected,
            fingerprint(&metadata, dir.clone(), vec![snapshot("a")]).unwrap()
        );
        assert_ne!(
            expected,
            fingerprint(
                &metadata,
                json!({"canonical":"other"}),
                vec![snapshot("a"), snapshot("b")]
            )
            .unwrap()
        );
    }
    #[test]
    fn running_game_install_and_backup_each_prevent_maintenance() {
        let mut value = json!({"running_process_count":0,"active_job_count":0,"active_backup_count":0});
        assert!(idle(&value).is_ok());
        for field in [
            "running_process_count",
            "active_job_count",
            "active_backup_count",
        ] {
            value[field] = json!(1);
            assert_eq!(idle(&value).unwrap_err().code, "instance_busy");
            value[field] = json!(0);
        }
    }
    #[tokio::test]
    async fn deletion_targets_are_existing_direct_children_only() {
        let root = tempfile::tempdir().unwrap();
        let canonical = tokio::fs::canonicalize(root.path()).await.unwrap();
        tokio::fs::create_dir(canonical.join("profile"))
            .await
            .unwrap();
        assert!(
            checked_child(&canonical, Path::new("profile"))
                .await
                .is_ok()
        );
        assert_eq!(
            checked_child(&canonical, Path::new("../outside"))
                .await
                .unwrap_err()
                .code,
            "invalid_instance_path"
        );
        assert_eq!(
            checked_child(&canonical, Path::new("absent"))
                .await
                .unwrap_err()
                .code,
            "instance_directory_missing"
        );
    }
    #[test]
    fn deletion_preview_handles_are_session_bound_and_consumed_once() {
        let backend = LauncherBackend::default();
        let handle = backend
            .deletions
            .insert(
                "owner",
                0,
                DeletePreview {
                    instance_id: "instance".into(),
                    fingerprint: "fingerprint".into(),
                },
            )
            .unwrap();
        assert!(backend.deletions.get("other", handle, 0).is_err());
        assert_eq!(
            backend
                .deletions
                .get("owner", handle, 0)
                .unwrap()
                .instance_id,
            "instance"
        );
        backend.deletions.take("owner", handle, 0).unwrap();
        assert!(backend.deletions.get("owner", handle, 0).is_err());
    }
}
